use actix_web::{HttpResponse, Responder, get, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

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

#[get("/")]
pub async fn listing_page(
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let archives =
        WabbajackArchive::get_all(&conn).map_err(actix_web::error::ErrorInternalServerError)?;

    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Modlists" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    h1 { "Wabbajack Modlists" }
                    @if archives.is_empty() {
                        p.empty-state { "No modlists found." }
                    } @else {
                        table.modlist-table {
                            thead {
                                tr {
                                    th { "Name" }
                                    th { "Version" }
                                    th { "Filename" }
                                    th { "Size" }
                                    th { "Hash" }
                                    th { "Status" }
                                }
                            }
                            tbody {
                                @for archive in &archives {
                                    tr {
                                        td.name {
                                            a href={"/modlists/" (archive.id)} {
                                                (archive.name)
                                            }
                                        }
                                        td.version { (archive.version) }
                                        td.filename { (archive.filename) }
                                        td.size { (format_size(archive.size)) }
                                        td.hash {
                                            code { (format_hash(&archive.xxhash64)) }
                                        }
                                        td.status {
                                            @if archive.available {
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
