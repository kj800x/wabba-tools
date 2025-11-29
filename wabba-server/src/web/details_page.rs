use actix_web::{HttpResponse, Responder, get, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::db::mod_data::Mod;
use crate::db::modlist::Modlist;
use wabba_protocol::archive_state::ArchiveState;

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

fn render_source(source: &ArchiveState) -> maud::Markup {
    html! {
        @match source {
            ArchiveState::NexusDownloader {
                name,
                mod_id,
                file_id,
                game_name,
                author,
                description,
                version,
                image_url,
                is_nsfw,
                ..
            } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "Nexus Mods" }
                        @if *is_nsfw {
                            span.nsfw-badge { "NSFW" }
                        }
                    }
                    @if let Some(img_url) = image_url {
                        div.source-image {
                            img src=(img_url) alt="Mod image" {}
                        }
                    }
                    div.source-details {
                        @if let Some(author_name) = author {
                            div.source-field {
                                strong { "Author: " }
                                (author_name)
                            }
                        }
                        div.source-field {
                            strong { "Name: " }
                            (name)
                        }
                        div.source-field {
                            strong { "Version: " }
                            (version)
                        }
                        div.source-field {
                            strong { "Game: " }
                            (game_name)
                        }
                        div.source-field {
                            strong { "Mod ID: " }
                            code { (mod_id) }
                        }
                        div.source-field {
                            strong { "File ID: " }
                            code { (file_id) }
                        }
                        @if !description.is_empty() {
                            div.source-field {
                                strong { "Description: " }
                                p.source-description { (description) }
                            }
                        }
                    }
                }
            }
            ArchiveState::HttpDownloader { url, headers } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "HTTP Download" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "URL: " }
                            a href=(url) target="_blank" { (url) }
                        }
                        @if !headers.is_null() {
                            div.source-field {
                                strong { "Headers: " }
                                code.source-headers {
                                    (serde_json::to_string_pretty(headers).unwrap_or_default())
                                }
                            }
                        }
                    }
                }
            }
            ArchiveState::WabbajackCDNDownloader { url } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "Wabbajack CDN" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "URL: " }
                            a href=(url) target="_blank" { (url) }
                        }
                    }
                }
            }
            ArchiveState::ManualDownloader { url, prompt } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "Manual Download" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "URL: " }
                            a href=(url) target="_blank" { (url) }
                        }
                        div.source-field {
                            strong { "Prompt: " }
                            (prompt)
                        }
                    }
                }
            }
            ArchiveState::MegaDownloader { url } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "MEGA" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "URL: " }
                            a href=(url) target="_blank" { (url) }
                        }
                    }
                }
            }
            ArchiveState::GoogleDriveDownloader { id } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "Google Drive" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "File ID: " }
                            code { (id) }
                        }
                    }
                }
            }
            ArchiveState::MediaFireDownloader { url } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "MediaFire" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "URL: " }
                            a href=(url) target="_blank" { (url) }
                        }
                    }
                }
            }
            ArchiveState::LoversLabOAuthDownloader {
                name,
                ips4_mod,
                url,
                author,
                description,
                version,
                image_url,
                is_nsfw,
                ..
            } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "LoversLab" }
                        @if *is_nsfw {
                            span.nsfw-badge { "NSFW" }
                        }
                    }
                    @if let Some(img_url) = image_url {
                        div.source-image {
                            img src=(img_url) alt="Mod image" {}
                        }
                    }
                    div.source-details {
                        @if let Some(author_name) = author {
                            div.source-field {
                                strong { "Author: " }
                                (author_name)
                            }
                        }
                        @if let Some(mod_name) = name {
                            div.source-field {
                                strong { "Name: " }
                                (mod_name)
                            }
                        }
                        @if let Some(mod_version) = version {
                            div.source-field {
                                strong { "Version: " }
                                (mod_version)
                            }
                        }
                        div.source-field {
                            strong { "Mod ID: " }
                            code { (ips4_mod) }
                        }
                        div.source-field {
                            strong { "URL: " }
                            a href=(url) target="_blank" { (url) }
                        }
                        @if let Some(desc) = description {
                            @if !desc.is_empty() {
                                div.source-field {
                                    strong { "Description: " }
                                    p.source-description { (desc) }
                                }
                            }
                        }
                    }
                }
            }
            ArchiveState::GameFileSourceDownloader {
                game,
                game_file,
                game_version,
                hash,
            } => {
                div.source-info {
                    div.source-header {
                        span.source-type { "Game File" }
                    }
                    div.source-details {
                        div.source-field {
                            strong { "Game: " }
                            (game)
                        }
                        div.source-field {
                            strong { "File: " }
                            code { (game_file) }
                        }
                        div.source-field {
                            strong { "Game Version: " }
                            (game_version)
                        }
                        div.source-field {
                            strong { "Hash: " }
                            code { (hash) }
                        }
                    }
                }
            }
            ArchiveState::UnknownDownloader => {
                div.source-info {
                    div.source-header {
                        span.source-type { "Unknown Source" }
                    }
                    div.source-details {
                        p { "Source type is not recognized or not available." }
                    }
                }
            }
        }
    }
}

#[get("/mod/{id}")]
pub async fn mod_details_page(
    id: web::Path<u64>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let mod_id = id.into_inner();

    let mod_item = Mod::get_by_id(mod_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Mod not found"))?;

    // Get modlists via association table
    let modlists = mod_item
        .get_associated_modlists(&conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Compute mod counts for each modlist
    let modlists_with_counts: Vec<_> = modlists
        .iter()
        .map(|modlist| {
            let mods_total = modlist.count_mods_total(&conn).unwrap_or(0);
            let mods_available = modlist.count_mods_available(&conn).unwrap_or(0);
            (modlist, mods_total, mods_available)
        })
        .collect();

    // Get mods with the same filename (excluding current mod)
    let mods_same_filename = Mod::get_by_filename_all(&mod_item.filename, mod_item.id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Get mods with the same name (excluding current mod, only if name exists)
    let mods_same_name = if let Some(ref name) = mod_item.name {
        Mod::get_by_name_all(name, mod_item.id, &conn)
            .map_err(actix_web::error::ErrorInternalServerError)?
    } else {
        Vec::new()
    };

    let page = html! {
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title {
                    @if let Some(ref name) = mod_item.name {
                        (name.clone())
                    } @else {
                        (mod_item.filename.clone())
                    }
                    " - Mod Details"
                }
                link rel="stylesheet" href="/res/styles.css";
            }
            body.page-details {
                div.container {
                    div.header {
                        a.back-link href="/" { "← Back to Modlists" }
                        h1 {
                            @if let Some(ref name) = mod_item.name {
                                (name.clone())
                            } @else {
                                (mod_item.filename.clone())
                            }
                        }
                        div.metadata {
                            p { strong { "ID: " } (mod_item.id) }
                            p { strong { "Filename: " } (mod_item.filename.clone()) }
                            @if let Some(ref name) = mod_item.name {
                                p { strong { "Name: " } (name.clone()) }
                            }
                            @if let Some(ref version) = mod_item.version {
                                p { strong { "Version: " } (version.clone()) }
                            }
                            p { strong { "Size: " } (format_size(mod_item.size)) }
                            p { strong { "Hash: " } (format_hash(&mod_item.xxhash64)) }
                            p {
                                strong { "Status: " }
                                @if mod_item.available {
                                    span.status-badge.available { "Available" }
                                } @else {
                                    span.status-badge.unavailable { "Unavailable" }
                                }
                            }
                        }
                    }

                    @if let Some(ref source) = mod_item.source {
                        h2 { "Source" }
                        div.source-section {
                            (render_source(source))
                        }
                    }

                    h2 { "Conflicts - Mods with Same Filename" }
                    @if mods_same_filename.is_empty() {
                        p.empty-state { "No conflicts found." }
                    } @else {
                        table.mod-table.mod-table-with-id {
                            thead {
                                tr {
                                    th { "ID" }
                                    th { "Filename" }
                                    th { "Name" }
                                    th { "Version" }
                                    th { "Size" }
                                    th { "Hash" }
                                    th { "Status" }
                                }
                            }
                            tbody {
                                @for related_mod in &mods_same_filename {
                                    tr class=(if related_mod.available { "" } else { "unavailable-row" }) {
                                        td.id { (related_mod.id) }
                                        td.filename {
                                            a href=(format!("/mod/{}", related_mod.id)) {
                                                (related_mod.filename.clone())
                                            }
                                        }
                                        td.name {
                                            a href=(format!("/mod/{}", related_mod.id)) {
                                                @if let Some(ref name) = related_mod.name {
                                                    (name)
                                                } @else {
                                                    em { "Unknown" }
                                                }
                                            }
                                        }
                                        td.version {
                                            @if let Some(ref version) = related_mod.version {
                                                (version)
                                            } @else {
                                                em { "-" }
                                            }
                                        }
                                        td.size {
                                            (format_size(related_mod.size))
                                        }
                                        td.hash {
                                            code { (format_hash(&related_mod.xxhash64)) }
                                        }
                                        td.status {
                                            @if related_mod.available {
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

                    h2 { "Associated Modlists" }
                    @if modlists_with_counts.is_empty() {
                        p.empty-state { "This mod is not associated with any modlists." }
                    } @else {
                        table.mod-table {
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
                                @for (modlist, mods_total, mods_available) in &modlists_with_counts {
                                    tr {
                                        td.name {
                                            a href=(format!("/modlists/{}", modlist.id)) {
                                                (modlist.name.clone())
                                            }
                                        }
                                        td.version { (modlist.version.clone()) }
                                        td.filename { (modlist.filename.clone()) }
                                        td.size { (format_size(modlist.size)) }
                                        td.hash {
                                            code { (format_hash(&modlist.xxhash64)) }
                                        }
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

                    @if mod_item.name.is_some() {
                        h2 { "Other Versions - Mods with Same Name" }
                        @if mods_same_name.is_empty() {
                            p.empty-state { "No other versions found." }
                        } @else {
                            table.mod-table.mod-table-with-id {
                                thead {
                                    tr {
                                        th { "ID" }
                                        th { "Filename" }
                                        th { "Name" }
                                        th { "Version" }
                                        th { "Size" }
                                        th { "Hash" }
                                        th { "Status" }
                                    }
                                }
                                tbody {
                                    @for related_mod in &mods_same_name {
                                        tr class=(if related_mod.available { "" } else { "unavailable-row" }) {
                                            td.id { (related_mod.id) }
                                            td.filename {
                                                a href=(format!("/mod/{}", related_mod.id)) {
                                                    (related_mod.filename.clone())
                                                }
                                            }
                                            td.name {
                                                a href=(format!("/mod/{}", related_mod.id)) {
                                                    @if let Some(ref name) = related_mod.name {
                                                        (name)
                                                    } @else {
                                                        em { "Unknown" }
                                                    }
                                                }
                                            }
                                            td.version {
                                                @if let Some(ref version) = related_mod.version {
                                                    (version)
                                                } @else {
                                                    em { "-" }
                                                }
                                            }
                                            td.size {
                                                (format_size(related_mod.size))
                                            }
                                            td.hash {
                                                code { (format_hash(&related_mod.xxhash64)) }
                                            }
                                            td.status {
                                                @if related_mod.available {
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
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(page.into_string()))
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
