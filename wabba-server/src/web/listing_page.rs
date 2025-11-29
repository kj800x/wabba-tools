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
                    div.header-nav {
                        h1 { "Wabbajack Modlists" }
                        a.nav-link href="/mods" { "View All Mods" }
                    }
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

#[get("/mods")]
pub async fn mods_listing_page(
    query: web::Query<std::collections::HashMap<String, String>>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let show_unavailable_only = query
        .get("filter")
        .map(|s| s == "unavailable")
        .unwrap_or(false);

    let mods = if show_unavailable_only {
        Mod::get_unavailable(&conn).map_err(actix_web::error::ErrorInternalServerError)?
    } else {
        Mod::get_all(&conn).map_err(actix_web::error::ErrorInternalServerError)?
    };

    // Compute modlist counts for each mod
    let mods_with_counts: Vec<_> = mods
        .iter()
        .map(|mod_item| {
            let modlists_count = mod_item.count_modlists(&conn).unwrap_or(0);
            (mod_item, modlists_count)
        })
        .collect();

    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title {
                    @if show_unavailable_only {
                        "Missing Mods"
                    } @else {
                        "Mods"
                    }
                }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    div.header-nav {
                        h1 {
                            @if show_unavailable_only {
                                "Missing Mods"
                            } @else {
                                "Mods"
                            }
                        }
                        div.nav-links {
                            a.nav-link href="/" { "View Modlists" }
                            @if show_unavailable_only {
                                a.nav-link href="/mods" { "View All Mods" }
                            } @else {
                                a.nav-link href="/mods?filter=unavailable" { "View Missing Mods" }
                            }
                        }
                    }
                    @if mods_with_counts.is_empty() {
                        p.empty-state {
                            @if show_unavailable_only {
                                "No missing mods found."
                            } @else {
                                "No mods found."
                            }
                        }
                    } @else {
                        table.modlist-table.mods-table {
                            thead {
                                tr {
                                    th { "Filename" }
                                    th { "Name" }
                                    th { "Version" }
                                    th { "Size" }
                                    th { "Hash" }
                                    th { "Modlists" }
                                    th { "Status" }
                                }
                            }
                            tbody {
                                @for (mod_item, modlists_count) in &mods_with_counts {
                                    tr {
                                        td.filename {
                                            a href=(format!("/mod/{}", mod_item.id)) {
                                                (mod_item.filename.clone())
                                            }
                                        }
                                        td.name {
                                            a href=(format!("/mod/{}", mod_item.id)) {
                                                @if let Some(ref name) = mod_item.name {
                                                    (name)
                                                } @else {
                                                    em { "Unknown" }
                                                }
                                            }
                                        }
                                        td.version {
                                            @if let Some(ref version) = mod_item.version {
                                                (version)
                                            } @else {
                                                em { "-" }
                                            }
                                        }
                                        td.size { (format_size(mod_item.size)) }
                                        td.hash {
                                            code { (format_hash(&mod_item.xxhash64)) }
                                        }
                                        td { (modlists_count) }
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
