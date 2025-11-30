pub mod bootstrap;
pub mod ingest;
pub mod upload_validation;

use actix_web::HttpRequest;
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
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
use crate::resources::ingest::{ingest_mod, ingest_modlist};
use crate::resources::upload_validation::{UploadValidationResult, validate_upload_request};

/// Converts a base64 hash to base64url encoding for use in filenames
fn base64_to_base64url(base64_hash: &str) -> String {
    base64_hash
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_string()
}

/// Determines the final filename, handling collisions by appending hash and/or incrementing numbers
fn determine_final_filename(
    requested_filename: &str,
    hash_base64url: &str,
    downloads_dir: &Path,
) -> String {
    // Check if requested filename is available
    let requested_path = downloads_dir.join(requested_filename);
    if !requested_path.exists() {
        return requested_filename.to_string();
    }

    // Filename is taken, append hash
    let (name, ext) = match requested_filename.rfind('.') {
        Some(dot_idx) => {
            let (n, e) = requested_filename.split_at(dot_idx);
            (n, &e[1..]) // Remove the dot from extension
        }
        None => (requested_filename, ""),
    };

    let mut candidate = if ext.is_empty() {
        format!("{}-{}", name, hash_base64url)
    } else {
        format!("{}-{}.{}", name, hash_base64url, ext)
    };

    // Check if hash-appended filename exists, if so append incrementing number
    let mut counter = 0;
    while downloads_dir.join(&candidate).exists() {
        counter += 1;
        candidate = if ext.is_empty() {
            format!("{}-{}_{}", name, hash_base64url, counter)
        } else {
            format!("{}-{}_{}.{}", name, hash_base64url, counter, ext)
        };
    }

    candidate
}

/// Streams the upload payload to a temporary file, with progress logging every 5 seconds.
/// Returns the path to the temporary file and the total number of bytes written.
async fn stream_upload_to_temp_file(
    temp_dir: &Path,
    body: web::Payload,
) -> Result<(PathBuf, usize), actix_web::Error> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Create unique temp filename
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // TODO: Is nanos safe enough that we won't have fs collisions?
    let temp_filename = format!("upload_{}.tmp", timestamp);
    let temp_path = temp_dir.join(&temp_filename);

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Failed to create temp file: {}", e))
        })?;
    let mut writer = BufWriter::new(file);

    log::info!("Uploading to temp file: {:?}", temp_path);

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

    log::info!("Upload complete, {} bytes written", total_written);

    Ok((temp_path, total_written))
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
    let requested_filename = filename.into_inner();
    let data_dir = data_dir.into_inner();

    log::info!("Request to upload modlist file {}", requested_filename);

    // Validate the upload request (check by hash)
    let validation_result = validate_upload_request::<Modlist>(&req, &conn).map_err(|e| {
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
        UploadValidationResult::AcceptUpload => {
            // Continue with upload
        }
    }

    // Get hash from If-None-Match header
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|x| x.to_str().ok())
        .expect("If-None-Match header should have been validated earlier");

    // Upload to temporary file
    let modlist_dir = data_dir.get_modlist_dir();
    let (temp_path, _size) = stream_upload_to_temp_file(&modlist_dir, body).await?;

    // Compute hash from uploaded file
    let computed_hash = Hash::compute(&std::fs::read(&temp_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        actix_web::error::ErrorInternalServerError(format!("Failed to read temp file: {}", e))
    })?);

    // Verify hash matches
    if computed_hash != if_none_match {
        let _ = std::fs::remove_file(&temp_path);
        return Err(actix_web::error::ErrorBadRequest(format!(
            "File hash mismatch: user provided {}, we computed {}",
            if_none_match, computed_hash
        )));
    }

    // Determine final filename (handle collisions same as mods)
    let hash_base64url = base64_to_base64url(if_none_match);
    let final_filename =
        determine_final_filename(&requested_filename, &hash_base64url, &modlist_dir);
    let final_path = modlist_dir.join(&final_filename);

    // Move temp file to final location
    std::fs::rename(&temp_path, &final_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to move file to final location: {}",
            e
        ))
    })?;

    log::info!("File moved to final location: {}", final_filename);

    // Update database
    ingest_modlist(&final_filename, if_none_match, &final_path, &conn).map_err(|e| {
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
    let requested_filename = filename.into_inner();
    let data_dir = data_dir.into_inner();

    log::info!("Request to upload mod file {}", requested_filename);

    // Validate the upload request (check by hash)
    let validation_result = validate_upload_request::<Mod>(&req, &conn).map_err(|e| {
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
        UploadValidationResult::AcceptUpload => {
            // Continue with upload
        }
    }

    // Get hash from If-None-Match header
    let if_none_match = req
        .headers()
        .get("If-None-Match")
        .and_then(|x| x.to_str().ok())
        .expect("If-None-Match header should have been validated earlier");

    // Upload to temporary file
    let downloads_dir = data_dir.get_mod_dir();
    let (temp_path, _size) = stream_upload_to_temp_file(&downloads_dir, body).await?;

    // Compute hash from uploaded file
    let computed_hash = Hash::compute(&std::fs::read(&temp_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        actix_web::error::ErrorInternalServerError(format!("Failed to read temp file: {}", e))
    })?);

    // Verify hash matches
    if computed_hash != if_none_match {
        let _ = std::fs::remove_file(&temp_path);
        return Err(actix_web::error::ErrorBadRequest(format!(
            "File hash mismatch: user provided {}, we computed {}",
            if_none_match, computed_hash
        )));
    }

    // Determine final filename
    let hash_base64url = base64_to_base64url(if_none_match);
    let final_filename =
        determine_final_filename(&requested_filename, &hash_base64url, &downloads_dir);
    let final_path = downloads_dir.join(&final_filename);

    // Move temp file to final location
    std::fs::rename(&temp_path, &final_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        actix_web::error::ErrorInternalServerError(format!(
            "Failed to move file to final location: {}",
            e
        ))
    })?;

    log::info!("File moved to final location: {}", final_filename);

    // Update database
    ingest_mod(&final_filename, if_none_match, &final_path, &conn).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;

    Ok(HttpResponse::Ok().body("ok"))
}
