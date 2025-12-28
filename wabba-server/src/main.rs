pub mod prelude {
    pub use chrono::prelude::*;

    pub use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
    pub use actix_web::{
        App, HttpResponse, HttpServer, Responder,
        cookie::Key,
        delete, error, get, middleware, post, put,
        web::{self, Data, Json, get as web_get, resource},
    };
    pub use futures_util::future::join_all;
    pub use r2d2::Pool;
    pub use r2d2_sqlite::SqliteConnectionManager;
    pub use serde::{Deserialize, Serialize};

    pub use actix_web::Error;
    pub use actix_web::{Result, guard};
    pub use maud::{DOCTYPE, Markup, html};
    pub use r2d2::PooledConnection;
    pub use rusqlite::Connection;
    pub use rusqlite::{OptionalExtension, params};
    pub use rusqlite_migration::{M, Migrations};
    pub use std::time::{SystemTime, UNIX_EPOCH};
}

mod data_dir;
mod db;
mod resources;
mod web;
use std::path::PathBuf;

use crate::data_dir::DataDir;
use crate::db::migrations::migrate;
use crate::prelude::*;
use crate::resources::bootstrap::{bootstrap, bootstrap_modlists, bootstrap_mods};
use crate::resources::{hello_world, upload_mod, upload_modlist};
use crate::web::details_page::{
    details_page, mod_details_page, mod_image, rename_modlist, toggle_lost_forever, toggle_muted,
};
use crate::web::listing_page::{listing_page, mods_listing_page, muted_modlists_page};
use crate::web::upload_page::{upload_page, upload_post};
use wabba_server::serve_static_file;

async fn start_http(
    pool: Pool<SqliteConnectionManager>,
    data_dir: DataDir,
) -> Result<(), std::io::Error> {
    log::info!("Starting HTTP server at http://localhost:8080/api");

    HttpServer::new(move || {
        App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::from(&[0; 64]))
                    .cookie_secure(false)
                    .build(),
            )
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(data_dir.clone()))
            .wrap(middleware::Logger::default())
            .service(hello_world)
            .service(upload_modlist)
            .service(upload_mod)
            .service(listing_page)
            .service(mods_listing_page)
            .service(muted_modlists_page)
            .service(details_page)
            .service(mod_details_page)
            .service(mod_image)
            .service(toggle_lost_forever)
            .service(toggle_muted)
            .service(rename_modlist)
            .service(bootstrap)
            .service(bootstrap_modlists)
            .service(bootstrap_mods)
            .service(upload_page)
            .service(upload_post)
            .service(serve_static_file!("htmx.min.js"))
            .service(serve_static_file!("idiomorph.min.js"))
            .service(serve_static_file!("idiomorph-ext.min.js"))
            .service(serve_static_file!("styles.css"))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[actix_web::main]
#[allow(clippy::expect_used)]
async fn main() -> std::io::Result<()> {
    // Configure logger with custom filter to prioritize Discord logs
    env_logger::builder()
        .filter_level(log::LevelFilter::Info) // Set default level to Info for most modules
        .filter_module("actix_web::middleware::logger", log::LevelFilter::Warn) // Actix web middleware logs every request at info
        .parse_default_env()
        .init();

    let data_dir = DataDir::new(&PathBuf::from(
        std::env::var("DATA_DIR").expect("DATA_DIR environment variable is not set"),
    ))
    .expect("Failed to open data directory");

    log::info!("Data directory: {:?}", data_dir.get_path());

    // connect to SQLite DB
    let manager = SqliteConnectionManager::file(data_dir.get_db_path());
    let pool = Pool::new(manager).expect("Failed to create database pool");
    {
        let conn = pool.get().expect("Failed to get database connection");
        migrate(conn).expect("Failed to run database migrations");
    }

    start_http(pool.clone(), data_dir).await?;

    Ok(())
}
