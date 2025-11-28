use actix_web::{HttpResponse, post, web};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use wabba_protocol::hash::Hash;

use crate::{
    data_dir::DataDir,
    resources::ingest::{ingest_mod, ingest_modlist},
};

fn bootstrap_modlists_impl(
    conn: &PooledConnection<SqliteConnectionManager>,
    data_dir: &DataDir,
) -> Result<(), actix_web::Error> {
    // Read all modlist files in the modlist directory
    let modlist_files = std::fs::read_dir(data_dir.get_modlist_dir()).unwrap();
    for modlist_file in modlist_files.filter_map(Result::ok) {
        let path = modlist_file.path();
        if path.extension().unwrap_or_default() != "wabbajack" {
            log::info!("Skipping non-wabbajack file: {:?}", path);
            continue;
        }
        log::info!("Processing modlist file: {:?}", path.file_name());
        let file_name_os = modlist_file.file_name();
        let filename = file_name_os.to_str().unwrap();
        let hash = Hash::compute(&std::fs::read(&path).unwrap());
        ingest_modlist(&filename, &hash, &path, &conn)?;
    }

    Ok(())
}

fn bootstrap_mods_impl(
    conn: &PooledConnection<SqliteConnectionManager>,
    data_dir: &DataDir,
) -> Result<(), actix_web::Error> {
    // Read all mod files in the mod directory
    let mod_files = std::fs::read_dir(data_dir.get_mod_dir()).unwrap();
    for mod_file in mod_files.filter_map(Result::ok) {
        let path = mod_file.path();
        if path.extension().unwrap_or_default() == "meta" {
            log::info!("Skipping meta file: {:?}", path.file_name());
            continue;
        }
        if path.is_dir() {
            log::info!("Skipping directory: {:?}", path.file_name());
            continue;
        }
        let file_name_os = mod_file.file_name();
        let filename = file_name_os
            .to_str()
            .expect("Failed to convert file name to string");
        log::info!("Processing mod file: {:?}", filename);
        let hash = Hash::compute(&std::fs::read(&path).expect("Failed to read mod file"));
        ingest_mod(&filename, &hash, &path, &conn)?;
    }

    Ok(())
}

#[post("/bootstrap/modlists")]
pub async fn bootstrap_modlists(
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
) -> Result<HttpResponse, actix_web::Error> {
    tokio::task::spawn_blocking(move || {
        let conn = pool.into_inner().get().unwrap();
        let data_dir = data_dir.into_inner();

        log::info!(
            "Bootstrapping modlists from data directory: {:?}",
            data_dir.get_path()
        );

        bootstrap_modlists_impl(&conn, &data_dir).expect("Failed to bootstrap modlists");

        log::info!("Modlists bootstrap complete");
    });

    Ok(HttpResponse::Ok().body("modlists bootstrap started"))
}

#[post("/bootstrap/mods")]
pub async fn bootstrap_mods(
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
) -> Result<HttpResponse, actix_web::Error> {
    tokio::task::spawn_blocking(move || {
        let conn = pool.into_inner().get().unwrap();
        let data_dir = data_dir.into_inner();

        log::info!(
            "Bootstrapping mods from data directory: {:?}",
            data_dir.get_path()
        );

        bootstrap_mods_impl(&conn, &data_dir).expect("Failed to bootstrap mods");

        log::info!("Mods bootstrap complete");
    });

    Ok(HttpResponse::Ok().body("mods bootstrap started"))
}

#[post("/bootstrap")]
pub async fn bootstrap(
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
) -> Result<HttpResponse, actix_web::Error> {
    tokio::task::spawn_blocking(move || {
        let conn = pool.into_inner().get().unwrap();
        let data_dir = data_dir.into_inner();

        log::info!(
            "Bootstrapping all from data directory: {:?}",
            data_dir.get_path()
        );

        bootstrap_modlists_impl(&conn, &data_dir).expect("Failed to bootstrap modlists");
        bootstrap_mods_impl(&conn, &data_dir).expect("Failed to bootstrap mods");

        log::info!("Bootstrapping complete");
    });

    Ok(HttpResponse::Ok().body("bootstrap started"))
}
