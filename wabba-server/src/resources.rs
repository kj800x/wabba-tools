use actix_web::HttpRequest;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use wabba_protocol::hash::Hash;
use wabba_protocol::wabbajack::WabbajackMetadata;

use actix_web::{HttpResponse, Responder, get, post, web};
use futures_util::StreamExt;
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::data_dir::DataDir;
use crate::db::mod_archive::ModArchive;
use crate::db::mod_archive::ModArchiveEgg;
use crate::db::wabbajack_archive::WabbajackArchive;
use crate::db::wabbajack_archive::WabbajackArchiveEgg;

#[get("/hello")]
pub async fn hello_world() -> impl Responder {
    html! {
        div {
          "Hello, world!"
        }
    }
}

#[post("/submit/wabbajack/{filename}")]
pub async fn upload_wabbajack_file(
    filename: web::Path<String>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
    req: HttpRequest,
    mut body: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let headers = req.headers();
    let pool = pool.into_inner().get().unwrap();
    let filename = filename.into_inner();
    let data_dir = data_dir.into_inner();
    let path = data_dir.get_modlist_path(&filename);

    log::info!("Request to upload modlist file {}", filename);

    let if_none_match = headers
        .get("If-None-Match")
        .map(|x| x.to_str().unwrap_or(""));

    match if_none_match {
        Some(if_none_match) => {
            if let Some(stored_archive) =
                WabbajackArchive::get_by_hash(if_none_match, &pool).unwrap()
            {
                if stored_archive.filename == filename {
                    return Ok(HttpResponse::NotModified().finish());
                } else {
                    return Err(actix_web::error::ErrorBadRequest(
                        "Content hash already stored in db under a different filename",
                    ));
                }
            }

            if let Some(stored_archive) =
                WabbajackArchive::get_by_filename(&filename, &pool).unwrap()
            {
                if stored_archive.xxhash64 == if_none_match {
                    return Ok(HttpResponse::NotModified().finish());
                } else {
                    return Err(actix_web::error::ErrorBadRequest(
                        "File already exists in db with different hash",
                    ));
                }
            }

            if path.exists() {
                let existing_hash = Hash::compute(&std::fs::read(&path).unwrap());
                if if_none_match == existing_hash {
                    log::warn!(
                        "User tried to upload a file which already existed on disk and matched the hash supplied, but it was not in the db"
                    );
                    return Ok(HttpResponse::NotModified().finish());
                } else {
                    return Err(actix_web::error::ErrorBadRequest(
                        "File already exists on disk (but not db) and does not match the hash supplied",
                    ));
                }
            }
        }

        None => {
            if path.exists() {
                return Err(actix_web::error::ErrorBadRequest(
                    "File already exists on disk (but not db) and you did not supply a hash",
                ));
            }
        }
    }

    let file = File::create(&path)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let mut writer = BufWriter::new(file);

    log::info!("Uploading modlist file {}", filename);

    let mut last_log_time = SystemTime::now();
    let mut total_written = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;

        writer
            .write_all(&chunk)
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;

        total_written += chunk.len();
        if last_log_time.elapsed().unwrap().as_secs() > 5 {
            last_log_time = SystemTime::now();
            log::info!(
                "...{:0.2} MB written so far",
                total_written as f64 / 1024.0 / 1024.0
            );
        }
    }

    log::info!("Uploaded modlist file {}", filename);

    writer
        .flush()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let hash = Hash::compute(&std::fs::read(&path).unwrap());

    let metadata = WabbajackMetadata::load(&path).expect("Failed to load Wabbajack metadata");

    let wabbajack_archive = WabbajackArchiveEgg {
        filename: filename.clone(),
        name: metadata.name.clone(),
        version: metadata.version.clone(),
        xxhash64: hash.clone(),
        size: std::fs::metadata(&path).unwrap().len() as u64,
        available: true,
    };

    let created_wabbajack_archive = wabbajack_archive.create(&pool).unwrap();

    log::info!(
        "created_wabbajack_archive: {:#?}",
        created_wabbajack_archive
    );

    for archive in metadata.required_archives() {
        let mod_archive = ModArchiveEgg {
            filename: archive.filename.clone(),
            name: archive.name(),
            version: archive.version(),
            xxhash64: archive.hash.clone(),
            size: archive.size,
            available: false,
        };

        let created_mod_archive = mod_archive.create(&pool).unwrap();
        created_mod_archive
            .associate(&created_wabbajack_archive, &pool)
            .unwrap();

        log::info!("created_mod_archive: {:#?}", created_mod_archive);
    }

    Ok(HttpResponse::Ok().body("ok"))
}

#[post("/submit/mod-archive/{filename}")]
pub async fn upload_mod_archive(
    filename: web::Path<String>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
    req: HttpRequest,
    mut body: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let headers = req.headers();
    let pool = pool.into_inner().get().unwrap();
    let filename = filename.into_inner();
    let data_dir = data_dir.into_inner();
    let path = data_dir.get_mod_archive_path(&filename);

    log::info!("Request to upload mod archive file {}", filename);

    let if_none_match = headers
        .get("If-None-Match")
        .map(|x| x.to_str().unwrap_or(""));

    match if_none_match {
        Some(if_none_match) => {
            if let Some(stored_archive) = ModArchive::get_by_hash(if_none_match, &pool)
                .unwrap()
                .filter(|x| x.available)
            {
                if stored_archive.filename == filename {
                    return Ok(HttpResponse::NotModified().finish());
                } else {
                    return Err(actix_web::error::ErrorBadRequest(
                        "Content hash already stored in db under a different filename",
                    ));
                }
            }

            if let Some(stored_archive) = ModArchive::get_by_filename(&filename, &pool)
                .unwrap()
                .filter(|x| x.available)
            {
                if stored_archive.xxhash64 == if_none_match {
                    return Ok(HttpResponse::NotModified().finish());
                } else {
                    return Err(actix_web::error::ErrorBadRequest(
                        "File already exists in db with different hash",
                    ));
                }
            }

            if path.exists() {
                let existing_hash = Hash::compute(&std::fs::read(&path).unwrap());
                if if_none_match == existing_hash {
                    log::warn!(
                        "User tried to upload a file which already existed on disk and matched the hash supplied, but it was not in the db"
                    );
                    return Ok(HttpResponse::NotModified().finish());
                } else {
                    return Err(actix_web::error::ErrorBadRequest(
                        "File already exists on disk (but not db) and does not match the hash supplied",
                    ));
                }
            }
        }

        None => {
            if path.exists() {
                return Err(actix_web::error::ErrorBadRequest(
                    "File already exists on disk (but not db) and you did not supply a hash",
                ));
            }
        }
    }

    let file = File::create(&path)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let mut writer = BufWriter::new(file);

    log::info!("Uploading mod archive file {}", filename);

    let mut last_log_time = SystemTime::now();
    let mut total_written = 0;
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;

        writer
            .write_all(&chunk)
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;

        total_written += chunk.len();
        if last_log_time.elapsed().unwrap().as_secs() > 5 {
            last_log_time = SystemTime::now();
            log::info!(
                "...{:0.2} MB written so far",
                total_written as f64 / 1024.0 / 1024.0
            );
        }
    }

    log::info!("Uploaded mod archive file {}", filename);

    writer
        .flush()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let hash = Hash::compute(&std::fs::read(&path).unwrap());

    match ModArchive::get_by_hash(&hash, &pool).unwrap() {
        Some(stored_archive) => {
            log::info!("Mod archive present in db, marking as available");
            stored_archive.mark_available(&pool).unwrap();
        }

        None => {
            log::info!("Mod archive not found in db, creating new one");
            let mod_archive = ModArchiveEgg {
                filename: filename,
                name: None,
                version: None,
                xxhash64: hash,
                size: std::fs::metadata(&path).unwrap().len() as u64,
                available: true,
            };

            mod_archive.create(&pool).unwrap();
        }
    }

    Ok(HttpResponse::Ok().body("ok"))
}
