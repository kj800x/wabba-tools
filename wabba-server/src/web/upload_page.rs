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

use crate::{data_dir::DataDir, db::mod_data::Mod, resources::ingest::ingest_mod};

#[get("/upload/mod")]
pub async fn upload_mod_page() -> impl Responder {
    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Upload Mod" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    div.header-nav {
                        h1 { "Upload Mod" }
                        a.nav-link href="/mods" { "Back to Mods" }
                    }
                    div.upload-section {
                        h2 { "Upload a Mod File" }
                        form method="post" action="/upload/mod" enctype="multipart/form-data" {
                            div.form-group {
                                label for="file-input" {
                                    "Select Mod File:"
                                }
                                input type="file" id="file-input" name="file" accept=".zip,.7z,.rar" required {}
                            }
                            div.form-group {
                                button.upload-button type="submit" {
                                    "Upload Mod"
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

#[post("/upload/mod")]
pub async fn upload_mod_page_post(
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
            let path = data_dir.get_mod_path(&filename_str);

            log::info!(
                "Request to upload mod file {} (simple upload, hash computed server-side)",
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

    // Compute hash server-side
    let hash = Hash::compute(&std::fs::read(&path).unwrap());

    log::info!("Computed hash {} for uploaded file {}", hash, filename);

    // Check if a mod with this hash already exists
    if let Ok(Some(_existing_mod)) = Mod::get_by_filename_and_hash(&filename, &hash, &conn) {
        // File already exists with same hash, delete the uploaded file
        let _ = std::fs::remove_file(&path);
        return Ok(render_upload_result(
            false,
            format!(
                "Mod with filename {} and hash {} already exists",
                filename, hash
            ),
            Some(hash),
        ));
    }

    // Ingest the mod
    match ingest_mod(&filename, &hash, &path, &conn) {
        Ok(_) => Ok(render_upload_result(
            true,
            format!("Upload successful! Hash: {}", hash),
            Some(hash),
        )),
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

fn render_upload_result(success: bool, message: String, hash: Option<String>) -> HttpResponse {
    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Upload Mod" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    div.header-nav {
                        h1 { "Upload Mod" }
                        a.nav-link href="/mods" { "Back to Mods" }
                    }
                    div.upload-section {
                        h2 { "Upload a Mod File" }
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
                        form method="post" action="/upload/mod" enctype="multipart/form-data" {
                            div.form-group {
                                label for="file-input" {
                                    "Select Mod File:"
                                }
                                input type="file" id="file-input" name="file" accept=".zip,.7z,.rar" required {}
                            }
                            div.form-group {
                                button.upload-button type="submit" {
                                    "Upload Mod"
                                }
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
