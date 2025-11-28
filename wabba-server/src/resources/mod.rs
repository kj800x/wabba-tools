pub mod ingest;
pub mod upload_validation;

use actix_web::HttpRequest;
use std::path::Path;
use std::time::SystemTime;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use wabba_protocol::hash::Hash;

use actix_web::{HttpResponse, Responder, get, post, web};
use futures_util::StreamExt;
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::data_dir::DataDir;
use crate::db::mod_data::Mod;
use crate::db::modlist::Modlist;
use crate::resources::ingest::ingest_mod;
use crate::resources::ingest::ingest_modlist;
use crate::resources::upload_validation::{UploadValidationResult, validate_upload_request};

/// Streams the upload payload to a file, with progress logging every 5 seconds.
/// Returns the total number of bytes written.
async fn stream_upload_to_file(
    path: &Path,
    filename: &str,
    body: web::Payload,
) -> Result<usize, actix_web::Error> {
    let file = File::create(path)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let mut writer = BufWriter::new(file);

    log::info!("Uploading file {}", filename);

    let mut last_log_time = SystemTime::now();
    let mut total_written = 0;
    let mut body = body;
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

    writer
        .flush()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log::info!("Uploaded file {}", filename);

    Ok(total_written)
}

#[get("/hello")]
pub async fn hello_world() -> impl Responder {
    html! {
        div {
          "Hello, world!"
        }
    }
}

#[post("/submit/modlist/{filename}")]
pub async fn upload_modlist(
    filename: web::Path<String>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
    req: HttpRequest,
    body: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool.into_inner().get().unwrap();
    let filename = filename.into_inner();
    let data_dir = data_dir.into_inner();
    let path = data_dir.get_modlist_path(&filename);

    log::info!("Request to upload modlist file {}", filename);

    if Modlist::get_by_filename(&filename, &conn)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Database error: {}", e)))?
        .is_some_and(|x| !x.available)
    {
        log::error!("invariant violation: modlists in database should always be available");
        return Err(actix_web::error::ErrorInternalServerError(
            "invariant violation: modlists in database should always be available",
        ));
    }

    // Validate the upload request
    let validation_result = validate_upload_request::<Modlist>(&req, &filename, &path, &conn)
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        })?;

    match validation_result {
        UploadValidationResult::NotModified => {
            return Ok(HttpResponse::NotModified().finish());
        }
        UploadValidationResult::RejectUserError(reason) => {
            let message = format!("User error: {}", reason);
            log::info!("{}", message);
            return Err(actix_web::error::ErrorBadRequest(message));
        }
        UploadValidationResult::RejectNeedsBootstrap(reason) => {
            let message = format!(
                "Data directory is inconsistent, fix with bootstrap endpoint: {}",
                reason
            );
            log::error!("{}", message);
            return Err(actix_web::error::ErrorBadRequest(message));
        }
        UploadValidationResult::AcceptUpload => {
            // Continue with upload
        }
    }

    // Stream the upload to disk
    stream_upload_to_file(&path, &filename, body).await?;

    // Compute hash and load metadata
    let hash = Hash::compute(&std::fs::read(&path).unwrap());

    // Verify that the computed hash matches the If-None-Match header
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|x| x.to_str().ok())
        .expect("If-None-Match header should have been validated earlier");

    if hash != if_none_match {
        // Delete the uploaded file since it doesn't match
        let _ = std::fs::remove_file(&path);
        return Err(actix_web::error::ErrorBadRequest(format!(
            "File hash mismatch: expected {}, got {}",
            if_none_match, hash
        )));
    }

    ingest_modlist(&filename, &hash, &path, &conn).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    Ok(HttpResponse::Ok().body("ok"))
}

#[post("/submit/mod/{filename}")]
pub async fn upload_mod(
    filename: web::Path<String>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
    req: HttpRequest,
    body: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool.into_inner().get().unwrap();
    let filename = filename.into_inner();
    let data_dir = data_dir.into_inner();
    let path = data_dir.get_mod_path(&filename);

    log::info!("Request to upload mod file {}", filename);

    // Validate the upload request
    let validation_result =
        validate_upload_request::<Mod>(&req, &filename, &path, &conn).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        })?;

    match validation_result {
        UploadValidationResult::NotModified => {
            return Ok(HttpResponse::NotModified().finish());
        }
        UploadValidationResult::RejectUserError(reason) => {
            return Err(actix_web::error::ErrorBadRequest(format!(
                "User error: {}",
                reason
            )));
        }
        UploadValidationResult::RejectNeedsBootstrap(reason) => {
            return Err(actix_web::error::ErrorBadRequest(format!(
                "Data directory is inconsistent, fix with bootstrap endpoint: {}",
                reason
            )));
        }
        UploadValidationResult::AcceptUpload => {
            // Continue with upload
        }
    }

    // Stream the upload to disk
    stream_upload_to_file(&path, &filename, body).await?;

    // Compute hash
    let hash = Hash::compute(&std::fs::read(&path).unwrap());

    // Verify that the computed hash matches the If-None-Match header
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|x| x.to_str().ok())
        .expect("If-None-Match header should have been validated earlier");

    if hash != if_none_match {
        // Delete the uploaded file since it doesn't match
        let _ = std::fs::remove_file(&path);
        return Err(actix_web::error::ErrorBadRequest(format!(
            "File hash mismatch: expected {}, got {}",
            if_none_match, hash
        )));
    }

    ingest_mod(&filename, &hash, &path, &conn).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    Ok(HttpResponse::Ok().body("ok"))
}
