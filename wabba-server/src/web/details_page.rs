use actix_web::{HttpResponse, Responder, get, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::db::mod_archive::ModArchive;
use crate::db::wabbajack_archive::WabbajackArchive;

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_hash(hash: &str) -> String {
    if hash.len() > 16 {
        format!("{}...", &hash[..16])
    } else {
        hash.to_string()
    }
}

#[get("/modlists/{id}")]
pub async fn details_page(
    id: web::Path<u64>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let archive_id = id.into_inner();

    let archive = WabbajackArchive::get_by_id(archive_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Modlist not found"))?;

    // Get mod archives via association table
    let mod_archives = ModArchive::get_by_wabbajack_archive_id(archive_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (archive.name.clone()) " - Modlist Details" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-details {
                div.container {
                    div.header {
                        a.back-link href="/" { "← Back to Modlists" }
                        h1 { (archive.name.clone()) }
                        div.metadata {
                            p { strong { "Version: " } (archive.version.clone()) }
                            p { strong { "Filename: " } (archive.filename.clone()) }
                            p { strong { "Size: " } (format_size(archive.size)) }
                            p { strong { "Hash: " } (format_hash(&archive.xxhash64)) }
                        }
                    }

                    h2 { "Required Mod Archives" }
                    @if mod_archives.is_empty() {
                        p.empty-state { "No mod archives found." }
                    } @else {
                        table.mod-archive-table {
                            thead {
                                tr {
                                    th { "Filename" }
                                    th { "Name" }
                                    th { "Version" }
                                    th { "Size" }
                                    th { "Hash" }
                                    th { "Status" }
                                }
                            }
                            tbody {
                                @for mod_archive in &mod_archives {
                                    tr {
                                        td.filename { (mod_archive.filename.clone()) }
                                        td.name {
                                            @if let Some(ref name) = mod_archive.name {
                                                (name)
                                            } @else {
                                                em { "Unknown" }
                                            }
                                        }
                                        td.version {
                                            @if let Some(ref version) = mod_archive.version {
                                                (version)
                                            } @else {
                                                em { "-" }
                                            }
                                        }
                                        td.size {
                                            (format_size(mod_archive.size))
                                        }
                                        td.hash {
                                            code { (format_hash(&mod_archive.xxhash64)) }
                                        }
                                        td.status {
                                            @if mod_archive.available {
                                                span.status-badge.available { "Available" }
                                            } @else {
                                                span.status-badge.unavailable { "Unavailable" }
                                            }
                                        }
                                    }
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
        .body(page.into_string()))
}
