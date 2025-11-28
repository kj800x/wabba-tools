use actix_web::{HttpResponse, Responder, get, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

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

#[get("/")]
pub async fn listing_page(
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let modlists = Modlist::get_all(&conn).map_err(actix_web::error::ErrorInternalServerError)?;

    // Compute mod counts for each modlist
    let modlists_with_counts: Vec<_> = modlists
        .iter()
        .map(|modlist| {
            let mods_total = modlist.count_mods_total(&conn).unwrap_or(0);
            let mods_available = modlist.count_mods_available(&conn).unwrap_or(0);
            (modlist, mods_total, mods_available)
        })
        .collect();

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
                    @if modlists_with_counts.is_empty() {
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
                                    th { "Mods total" }
                                    th { "Mods available" }
                                    th { "Status" }
                                }
                            }
                            tbody {
                                @for (modlist, mods_total, mods_available) in &modlists_with_counts {
                                    tr {
                                        td.name {
                                            a href={"/modlists/" (modlist.id)} {
                                                (modlist.name)
                                            }
                                        }
                                        td.version { (modlist.version) }
                                        td.filename { (modlist.filename) }
                                        td.size { (format_size(modlist.size)) }
                                        td.hash {
                                            code { (format_hash(&modlist.xxhash64)) }
                                        }
                                        td { (mods_total) }
                                        td { (mods_available) }
                                        td.status {
                                            @if *mods_total == 0 || *mods_available == *mods_total {
                                                span.status-badge.available { "Ready" }
                                            } @else {
                                                span.status-badge.missing { "Missing files" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div.bootstrap-section {
                        h2 { "Bootstrap Database" }
                        p {
                            "Scan the data directory and update the database with all modlists and mods found on disk."
                        }
                        form method="post" action="/bootstrap" {
                            button.bootstrap-button type="submit" {
                                "Run Bootstrap"
                            }
                        }
                        form method="post" action="/bootstrap/modlists" {
                            button.bootstrap-button type="submit" {
                                "Run Modlists Bootstrap"
                            }
                        }
                        form method="post" action="/bootstrap/mods" {
                            button.bootstrap-button type="submit" {
                                "Run Mods Bootstrap"
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
