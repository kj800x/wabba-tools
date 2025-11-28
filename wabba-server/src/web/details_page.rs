use actix_web::{HttpResponse, Responder, get, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::db::mod_data::Mod;
use crate::db::modlist::Modlist;

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

    let modlist = Modlist::get_by_id(archive_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Modlist not found"))?;

    // Get mods via association table
    let mods = Mod::get_by_modlist_id(archive_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Separate unavailable mods for the missing mods table
    let unavailable_mods: Vec<_> = mods.iter().filter(|m| !m.available).cloned().collect();
    let show_missing_table = !unavailable_mods.is_empty() && unavailable_mods.len() < 25;

    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (modlist.name.clone()) " - Modlist Details" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-details {
                div.container {
                    div.header {
                        a.back-link href="/" { "← Back to Modlists" }
                        h1 { (modlist.name.clone()) }
                        div.metadata {
                            p { strong { "Version: " } (modlist.version.clone()) }
                            p { strong { "Filename: " } (modlist.filename.clone()) }
                            p { strong { "Size: " } (format_size(modlist.size)) }
                            p { strong { "Hash: " } (format_hash(&modlist.xxhash64)) }
                        }
                    }

                    @if show_missing_table {
                        h2 { "Missing Mods" }
                        table.mod-table {
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
                                @for mod_item in &unavailable_mods {
                                    tr {
                                        td.filename { (mod_item.filename.clone()) }
                                        td.name {
                                            @if let Some(ref name) = mod_item.name {
                                                (name)
                                            } @else {
                                                em { "Unknown" }
                                            }
                                        }
                                        td.version {
                                            @if let Some(ref version) = mod_item.version {
                                                (version)
                                            } @else {
                                                em { "-" }
                                            }
                                        }
                                        td.size {
                                            (format_size(mod_item.size))
                                        }
                                        td.hash {
                                            code { (format_hash(&mod_item.xxhash64)) }
                                        }
                                        td.status {
                                            span.status-badge.unavailable { "Unavailable" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    h2 { "Required Mods" }
                    @if mods.is_empty() {
                        p.empty-state { "No mods found." }
                    } @else {
                        table.mod-table {
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
                                @for mod_item in &mods {
                                    tr {
                                        td.filename { (mod_item.filename.clone()) }
                                        td.name {
                                            @if let Some(ref name) = mod_item.name {
                                                (name)
                                            } @else {
                                                em { "Unknown" }
                                            }
                                        }
                                        td.version {
                                            @if let Some(ref version) = mod_item.version {
                                                (version)
                                            } @else {
                                                em { "-" }
                                            }
                                        }
                                        td.size {
                                            (format_size(mod_item.size))
                                        }
                                        td.hash {
                                            code { (format_hash(&mod_item.xxhash64)) }
                                        }
                                        td.status {
                                            @if mod_item.available {
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
