use actix_web::{HttpResponse, Responder, get, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::db::mod_association::ModAssociation;
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
    let all_modlists =
        Modlist::get_all(&conn).map_err(actix_web::error::ErrorInternalServerError)?;

    // Filter out muted modlists
    let modlists: Vec<_> = all_modlists.iter().filter(|m| !m.muted).collect();

    // Compute mod counts for each modlist
    let modlists_with_counts: Vec<_> = modlists
        .iter()
        .map(|modlist| {
            let mods_total = modlist.count_mods_total(&conn).unwrap_or(0);
            let mods_available = modlist.count_mods_available(&conn).unwrap_or(0);
            let has_lost_forever = modlist.has_lost_forever_mods(&conn).unwrap_or(false);
            (modlist, mods_total, mods_available, has_lost_forever)
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
                        div.nav-links {
                            a.nav-link href="/mods" { "View All Mods" }
                            a.nav-link href="/modlists/muted" { "View Muted Modlists" }
                        }
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
                                @for (modlist, mods_total, mods_available, has_lost_forever) in &modlists_with_counts {
                                    tr class=(
                                        if *has_lost_forever {
                                            "uninstallable-row"
                                        } else if *mods_total > 0 && *mods_available < *mods_total {
                                            "unavailable-row"
                                        } else {
                                            ""
                                        }
                                    ) {
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
                                            @if *has_lost_forever {
                                                span.status-badge.missing { "Uninstallable" }
                                            } @else if *mods_total == 0 || *mods_available == *mods_total {
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

#[get("/modlists/muted")]
pub async fn muted_modlists_page(
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let modlists = Modlist::get_muted(&conn).map_err(actix_web::error::ErrorInternalServerError)?;

    // Compute mod counts for each modlist
    let modlists_with_counts: Vec<_> = modlists
        .iter()
        .map(|modlist| {
            let mods_total = modlist.count_mods_total(&conn).unwrap_or(0);
            let mods_available = modlist.count_mods_available(&conn).unwrap_or(0);
            let has_lost_forever = modlist.has_lost_forever_mods(&conn).unwrap_or(false);
            (modlist, mods_total, mods_available, has_lost_forever)
        })
        .collect();

    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Muted Modlists" }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-listing {
                div.container {
                    div.header-nav {
                        h1 { "Muted Modlists" }
                        div.nav-links {
                            a.nav-link href="/" { "View All Modlists" }
                            a.nav-link href="/mods" { "View All Mods" }
                        }
                    }
                    @if modlists_with_counts.is_empty() {
                        p.empty-state { "No muted modlists found." }
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
                                @for (modlist, mods_total, mods_available, has_lost_forever) in &modlists_with_counts {
                                    tr class=(
                                        if *has_lost_forever {
                                            "uninstallable-row"
                                        } else if *mods_total > 0 && *mods_available < *mods_total {
                                            "unavailable-row"
                                        } else {
                                            "muted-row"
                                        }
                                    ) {
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
                                            @if *has_lost_forever {
                                                span.status-badge.missing { "Uninstallable" }
                                            } @else if *mods_total == 0 || *mods_available == *mods_total {
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

    // Get associations for all mods to display modlist-specific metadata
    // For the listing, we'll use the first association's metadata (if any)
    let mods_with_metadata: Vec<_> = mods
        .iter()
        .map(|mod_item| {
            let modlists_count = mod_item.count_modlists(&conn).unwrap_or(0);
            let associations =
                ModAssociation::get_by_mod_id(mod_item.id, &conn).unwrap_or_default();
            let first_assoc = associations.first().cloned();
            (mod_item, modlists_count, first_assoc)
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
                    @if mods_with_metadata.is_empty() {
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
                                @for (mod_item, modlists_count, first_assoc) in &mods_with_metadata {
                                    tr {
                                        td.filename {
                                            a href=(format!("/mod/{}", mod_item.id)) {
                                                @match &mod_item.disk_filename {
                                                    Some(disk_filename) => {
                                                        (disk_filename)
                                                    }
                                                    None => {
                                                        @match first_assoc {
                                                            Some(assoc) => {
                                                                (assoc.filename.clone())
                                                            }
                                                            None => {
                                                                em { "Unknown" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        td.name {
                                            a href=(format!("/mod/{}", mod_item.id)) {
                                                @match first_assoc {
                                                    Some(assoc) => {
                                                        @match &assoc.name {
                                                            Some(name) => {
                                                                (name.clone())
                                                            }
                                                            None => {
                                                                em { "Unknown" }
                                                            }
                                                        }
                                                    }
                                                    None => {
                                                        em { "Unknown" }
                                                    }
                                                }
                                            }
                                        }
                                        td.version {
                                            @match first_assoc {
                                                Some(assoc) => {
                                                    @match &assoc.version {
                                                        Some(version) => {
                                                            (version.clone())
                                                        }
                                                        None => {
                                                            em { "-" }
                                                        }
                                                    }
                                                }
                                                None => {
                                                    em { "-" }
                                                }
                                            }
                                        }
                                        td.size { (format_size(mod_item.size)) }
                                        td.hash {
                                            code { (format_hash(&mod_item.xxhash64)) }
                                        }
                                        td { (modlists_count) }
                                        td.status {
                                            @if mod_item.is_available() {
                                                span.status-badge.available { "Available" }
                                            } @else if mod_item.lost_forever {
                                                span.status-badge.missing { "Lost Forever" }
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
