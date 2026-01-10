use std::time::SystemTime;

use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, get, post, web};
use futures_util::TryStreamExt;
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tokio::{
    fs::OpenOptions,
    io::{AsyncWriteExt, BufWriter},
};
use wabba_protocol::hash::Hash;

use crate::{
    data_dir::DataDir,
    db::mod_data::Mod,
    db::modlist::Modlist,
    resources::ingest::{ingest_mod, ingest_modlist},
};

#[get("/upload")]
pub async fn upload_page() -> impl Responder {
    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Upload File" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    div.header-nav {
                        h1 { "Upload File" }
                        p { "Upload a modlist or mod file to the server" }
                    }
                    div.upload-section {
                        h2 { "Upload a file" }
                        form method="post" action="/upload" enctype="multipart/form-data" {
                            div.form-group {
                                label for="file-input" {
                                    "Select File:"
                                }
                                input type="file" id="file-input" name="file" accept=".zip,.7z,.rar,.wabbajack" required {}
                            }
                            div.form-group {
                                button.upload-button type="submit" {
                                    "Upload"
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(page.into_string())) as Result<HttpResponse, actix_web::Error>
}

#[post("/upload")]
pub async fn upload_post(
    pool: web::Data<Pool<SqliteConnectionManager>>,
    data_dir: web::Data<DataDir>,
    mut payload: Multipart,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let data_dir = data_dir.into_inner();

    let mut filename: Option<String> = None;
    let mut file_path: Option<std::path::PathBuf> = None;

    // Extract file from multipart form
    while let Some(mut field) = payload
        .try_next()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?
    {
        if field.name() == "file" {
            let content_disposition = field.content_disposition();
            let uploaded_filename = content_disposition
                .get_filename()
                .ok_or_else(|| actix_web::error::ErrorBadRequest("No filename in upload"))?;

            let filename_str = uploaded_filename.to_string();

            // Determine if this is a modlist (.wabbajack) or mod archive
            let is_modlist = filename_str.to_lowercase().ends_with(".wabbajack");
            let path = if is_modlist {
                data_dir.get_modlist_path(&filename_str)
            } else {
                data_dir.get_mod_path(&filename_str)
            };

            log::info!(
                "Request to upload {} file {} (simple upload, hash computed server-side)",
                if is_modlist { "modlist" } else { "mod" },
                filename_str
            );

            // Check if file already exists by filename
            if path.exists() {
                return Ok(render_upload_result(
                    false,
                    format!("File already exists: {}", filename_str),
                    None,
                ));
            }

            filename = Some(filename_str.clone());
            file_path = Some(path.clone());

            // Stream the upload to disk
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::AlreadyExists {
                        actix_web::error::ErrorBadRequest(format!(
                            "File already exists: {}",
                            filename_str
                        ))
                    } else {
                        actix_web::error::ErrorInternalServerError(format!(
                            "Failed to create file {}: {}",
                            filename_str, e
                        ))
                    }
                })?;
            let mut writer = BufWriter::new(file);

            let mut total_written = 0;
            let mut last_log_time = SystemTime::now();
            while let Some(chunk) = field
                .try_next()
                .await
                .map_err(actix_web::error::ErrorInternalServerError)?
            {
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

            log::info!("Uploaded file {}", filename_str);
            break;
        }
    }

    let filename =
        filename.ok_or_else(|| actix_web::error::ErrorBadRequest("No file field in form"))?;
    let path = file_path.unwrap();
    let is_modlist = filename.to_lowercase().ends_with(".wabbajack");

    // Compute hash server-side
    let hash = Hash::compute(&std::fs::read(&path).unwrap());

    log::info!("Computed hash {} for uploaded file {}", hash, filename);

    if is_modlist {
        // Handle modlist upload
        // Check if a modlist with this hash already exists
        if let Ok(Some(existing_modlist)) = Modlist::get_by_hash(&hash, &conn) {
            // If modlist exists and is available, redirect to its details page
            if existing_modlist.available {
                let _ = std::fs::remove_file(&path);
                return Ok(HttpResponse::SeeOther()
                    .append_header(("Location", format!("/modlists/{}", existing_modlist.id)))
                    .finish());
            }
            // If modlist exists but is unavailable, allow the upload to proceed
            // and ingest_modlist will mark it as available
        }

        // Ingest the modlist
        match ingest_modlist(&filename, &hash, &path, &conn) {
            Ok(_) => {
                // Get the modlist ID to redirect
                match Modlist::get_by_filename(&filename, &conn) {
                    Ok(Some(modlist)) => {
                        // Redirect to modlist details page
                        Ok(HttpResponse::SeeOther()
                            .append_header(("Location", format!("/modlists/{}", modlist.id)))
                            .finish())
                    }
                    Ok(None) => {
                        // This shouldn't happen, but handle it gracefully
                        Ok(render_upload_result(
                            true,
                            format!("Upload successful! Hash: {}", hash),
                            Some(hash),
                        ))
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&path);
                        Ok(render_upload_result(
                            false,
                            format!("Database error: {}", e),
                            Some(hash),
                        ))
                    }
                }
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Ok(render_upload_result(
                    false,
                    format!("Database error: {}", e),
                    Some(hash),
                ))
            }
        }
    } else {
        // Handle mod archive upload
        // Check if a mod with this hash already exists
        let file_size = std::fs::metadata(&path)
            .map_err(actix_web::error::ErrorInternalServerError)?
            .len() as u64;

        if let Ok(Some(existing_mod)) = Mod::get_by_size_and_hash(file_size, &hash, &conn) {
            // If mod exists and is available, reject the upload
            if existing_mod.is_available() {
                let _ = std::fs::remove_file(&path);
                return Ok(render_upload_result(
                    false,
                    format!(
                        "Mod with size {} and hash {} already exists",
                        file_size, hash
                    ),
                    Some(hash),
                ));
            }
            // If mod exists but is unavailable, allow the upload to proceed
            // and ingest_mod will mark it as available
        }

        // Ingest the mod
        match ingest_mod(&filename, &hash, &path, &conn) {
            Ok(_) => {
                // Get the mod ID to redirect
                match Mod::get_by_disk_filename(&filename, &conn) {
                    Ok(Some(mod_item)) => {
                        // Redirect to mod details page
                        Ok(HttpResponse::SeeOther()
                            .append_header(("Location", format!("/mod/{}", mod_item.id)))
                            .finish())
                    }
                    Ok(None) => {
                        // Try by hash as fallback
                        match Mod::get_by_hash(&hash, &conn) {
                            Ok(Some(mod_item)) => Ok(HttpResponse::SeeOther()
                                .append_header(("Location", format!("/mod/{}", mod_item.id)))
                                .finish()),
                            _ => {
                                // This shouldn't happen, but handle it gracefully
                                Ok(render_upload_result(
                                    true,
                                    format!("Upload successful! Hash: {}", hash),
                                    Some(hash),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&path);
                        Ok(render_upload_result(
                            false,
                            format!("Database error: {}", e),
                            Some(hash),
                        ))
                    }
                }
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Ok(render_upload_result(
                    false,
                    format!("Database error: {}", e),
                    Some(hash),
                ))
            }
        }
    }
}

fn render_upload_result(success: bool, message: String, hash: Option<String>) -> HttpResponse {
    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Upload File" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    div.header-nav {
                        h1 { "Upload File" }
                    }
                    div.upload-section {
                        h2 { "Result" }
                        @if success {
                            div.success-message {
                                p { (message) }
                            }
                        } @else {
                            div.error-message {
                                p { (message) }
                            }
                        }
                        @if let Some(ref hash_value) = hash {
                            p {
                                strong { "Hash: " }
                                code { (hash_value) }
                            }
                        }

                    }
                }
            }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(page.into_string())
}
