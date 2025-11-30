use actix_web::{HttpResponse, Responder, get, post, web};
use maud::html;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::db::mod_association::ModAssociation;
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

fn nexus_game_url_slug(game_name: &str) -> String {
    game_name.to_lowercase().replace(" ", "")
}

fn render_source(source: &ArchiveState, mod_id: u64) -> maud::Markup {
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
                @let game_slug = nexus_game_url_slug(game_name);
                div.source-info {
                    div.source-header {
                        span.source-type { "Nexus Mods" }
                        @if *is_nsfw {
                            span.nsfw-badge { "NSFW" }
                        }
                    }
                    @if let Some(img_url) = image_url {
                        div.source-image {
                            a href=(format!("https://www.nexusmods.com/{}/mods/{}", game_slug, mod_id)) target="_blank" {
                                img src=(img_url) alt="Mod image" {}
                            }
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
                            a href=(format!("https://www.nexusmods.com/{}/mods/{}", game_slug, mod_id)) target="_blank" {
                                (name)
                            }
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
                            a href=(format!("https://www.nexusmods.com/{}/mods/{}", game_slug, mod_id)) target="_blank" {
                                code { (mod_id) }
                            }
                        }
                        div.source-field {
                            strong { "File ID: " }
                            a href=(format!("https://www.nexusmods.com/{}/mods/{}?tab=files&file_id={}", game_slug, mod_id, file_id)) target="_blank" {
                                code { (file_id) }
                            }
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
                    @if image_url.is_some() {
                        div.source-image {
                            a href=(url) target="_blank" {
                                img src=(format!("/mod-image/{}", mod_id)) alt="Mod image" {}
                            }
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

    // Get all associations for this mod
    let associations = ModAssociation::get_by_mod_id(mod_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Get modlists via association table
    let modlists = mod_item
        .get_associated_modlists(&conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Create a map from modlist_id to ModAssociation for quick lookup
    use std::collections::HashMap;
    let assoc_map: HashMap<u64, &ModAssociation> = associations
        .iter()
        .map(|assoc| (assoc.modlist_id, assoc))
        .collect();

    // Create tuples with modlists and their associations
    let modlists_with_assocs: Vec<_> = modlists
        .iter()
        .map(|modlist| {
            let assoc = assoc_map.get(&modlist.id).cloned();
            (modlist, assoc)
        })
        .collect();

    // Get primary association (first one) for display purposes
    let primary_assoc = associations.first();

    // Get mods with the same disk filename (excluding current mod)
    let mods_same_filename = if let Some(ref disk_filename) = mod_item.disk_filename {
        Mod::get_by_disk_filename_all(disk_filename, mod_item.id, &conn)
            .map_err(actix_web::error::ErrorInternalServerError)?
    } else {
        Vec::new()
    };

    // Get associations for mods with same filename
    let mut mods_same_filename_with_assocs = Vec::new();
    for related_mod in &mods_same_filename {
        let related_assocs = ModAssociation::get_by_mod_id(related_mod.id, &conn)
            .map_err(actix_web::error::ErrorInternalServerError)?;
        let related_first_assoc = related_assocs.first().cloned();
        mods_same_filename_with_assocs.push((related_mod, related_first_assoc));
    }

    // Get mods with the same name from associations (excluding current mod)
    let mods_same_name = if let Some(assoc) = primary_assoc {
        if let Some(ref name) = assoc.name {
            // Find all mods that have associations with the same name
            let all_mods =
                Mod::get_all(&conn).map_err(actix_web::error::ErrorInternalServerError)?;
            let mut same_name_mods = Vec::new();
            for other_mod in all_mods {
                if other_mod.id == mod_id {
                    continue;
                }
                let other_assocs = ModAssociation::get_by_mod_id(other_mod.id, &conn)
                    .map_err(actix_web::error::ErrorInternalServerError)?;
                if other_assocs.iter().any(|a| a.name.as_ref() == Some(name)) {
                    let other_first_assoc = other_assocs.first().cloned();
                    same_name_mods.push((other_mod, other_first_assoc));
                }
            }
            same_name_mods
        } else {
            Vec::new()
        }
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
                    @match primary_assoc {
                        Some(assoc) => {
                            @match &assoc.name {
                                Some(name) => {
                                    (name.clone())
                                }
                                None => {
                                    @match &mod_item.disk_filename {
                                        Some(disk_filename) => {
                                            (disk_filename.clone())
                                        }
                                        None => {
                                            (assoc.filename.clone())
                                        }
                                    }
                                }
                            }
                        }
                        None => {
                            @match &mod_item.disk_filename {
                                Some(disk_filename) => {
                                    (disk_filename.clone())
                                }
                                None => {
                                    "Unknown Mod"
                                }
                            }
                        }
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
                            @match primary_assoc {
                                Some(assoc) => {
                                    @match &assoc.name {
                                        Some(name) => {
                                            (name.clone())
                                        }
                                        None => {
                                            @match &mod_item.disk_filename {
                                                Some(disk_filename) => {
                                                    (disk_filename.clone())
                                                }
                                                None => {
                                                    (assoc.filename.clone())
                                                }
                                            }
                                        }
                                    }
                                }
                                None => {
                                    @match &mod_item.disk_filename {
                                        Some(disk_filename) => {
                                            (disk_filename.clone())
                                        }
                                        None => {
                                            "Unknown Mod"
                                        }
                                    }
                                }
                            }
                        }
                        div.metadata {
                            p { strong { "ID: " } (mod_item.id) }
                            p {
                                strong { "Disk Filename: " }
                                @match &mod_item.disk_filename {
                                    Some(disk_filename) => {
                                        (disk_filename.clone())
                                    }
                                    None => {
                                        em { "Not available on disk" }
                                    }
                                }
                            }
                            @if let Some(assoc) = primary_assoc {
                                p { strong { "Modlist Filename: " } (assoc.filename.clone()) }
                        @if let Some(name) = &assoc.name {
                            p { strong { "Name: " } (name.clone()) }
                        }
                        @if let Some(version) = &assoc.version {
                            p { strong { "Version: " } (version.clone()) }
                        }
                            }
                            p { strong { "Size: " } (format_size(mod_item.size)) }
                            p { strong { "Hash: " } span.hash { code { (format_hash(&mod_item.xxhash64)) } } }
                            p {
                                strong { "Status: " }
                                @if mod_item.is_available() {
                                    span.status-badge.available { "Available" }
                                } @else {
                                    span.status-badge.unavailable { "Unavailable" }
                                }
                            }
                            @if !mod_item.is_available() {
                                p {
                                    strong { "Lost Forever: " }
                                    @if mod_item.lost_forever {
                                        span.status-badge.missing { "Yes" }
                                    } @else {
                                        span { "No" }
                                    }
                                    form method="post" action=(format!("/mod/{}/toggle-lost-forever", mod_item.id)) style="display: inline-block; margin-left: 1rem;" {
                                        button type="submit" style="padding: 0.4rem 0.8rem; border-radius: 4px; border: none; cursor: pointer; background-color: #3498db; color: white; font-weight: 500;" {
                                            @if mod_item.lost_forever {
                                                "Mark as Recoverable"
                                            } @else {
                                                "Mark as Lost Forever"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    @if let Some(assoc) = primary_assoc {
                        h2 { "Source" }
                        div.source-section {
                            (render_source(&assoc.source, mod_id))
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
                                @for (related_mod, related_first_assoc) in &mods_same_filename_with_assocs {
                                    tr class=(if related_mod.is_available() { "" } else { "unavailable-row" }) {
                                        td.id { (related_mod.id) }
                                        td.filename {
                                            a href=(format!("/mod/{}", related_mod.id)) {
                                                @match &related_mod.disk_filename {
                                                    Some(disk_filename) => {
                                                        (disk_filename.clone())
                                                    }
                                                    None => {
                                                        @match related_first_assoc {
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
                                            a href=(format!("/mod/{}", related_mod.id)) {
                                                @match related_first_assoc {
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
                                            @match related_first_assoc {
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
                                        td.size {
                                            (format_size(related_mod.size))
                                        }
                                        td.hash {
                                            code { (format_hash(&related_mod.xxhash64)) }
                                        }
                                        td.status {
                                            @if related_mod.is_available() {
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
                    @if modlists_with_assocs.is_empty() {
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
                                @for (modlist, assoc) in &modlists_with_assocs {
                                    tr {
                                        td.name {
                                            a href=(format!("/modlists/{}", modlist.id)) {
                                                (modlist.name.clone())
                                            }
                                        }
                                        td.version { (modlist.version.clone()) }
                                        td.filename {
                                            @match assoc {
                                                Some(assoc) => {
                                                    (assoc.filename.clone())
                                                }
                                                None => {
                                                    @match &mod_item.disk_filename {
                                                        Some(disk_filename) => {
                                                            (disk_filename.clone())
                                                        }
                                                        None => {
                                                            em { "Unknown" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        td.size { (format_size(mod_item.size)) }
                                        td.hash {
                                            span.hash {
                                                code { (format_hash(&mod_item.xxhash64)) }
                                            }
                                        }
                                        td.status {
                                            @if mod_item.is_available() {
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

                    @if primary_assoc.is_some_and(|a| a.name.is_some()) {
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
                                @for (related_mod, related_first_assoc) in &mods_same_name {
                                        tr class=(if related_mod.is_available() { "" } else { "unavailable-row" }) {
                                            td.id { (related_mod.id) }
                                            td.filename {
                                                a href=(format!("/mod/{}", related_mod.id)) {
                                                    @match &related_mod.disk_filename {
                                                        Some(disk_filename) => {
                                                            (disk_filename.clone())
                                                        }
                                                        None => {
                                                            @match related_first_assoc {
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
                                                a href=(format!("/mod/{}", related_mod.id)) {
                                                    @match related_first_assoc {
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
                                                @match related_first_assoc {
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
                                            td.size {
                                                (format_size(related_mod.size))
                                            }
                                            td.hash {
                                                code { (format_hash(&related_mod.xxhash64)) }
                                            }
                                            td.status {
                                                @if related_mod.is_available() {
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

#[get("/mod-image/{id}")]
pub async fn mod_image(
    id: web::Path<u64>,
    pool: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<impl Responder, actix_web::Error> {
    let conn = pool
        .get()
        .map_err(actix_web::error::ErrorInternalServerError)?;
    let mod_id = id.into_inner();

    // Get mod associations to find the image URL
    let associations = ModAssociation::get_by_mod_id(mod_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Find LoversLab association with image_url
    let image_url = associations
        .iter()
        .find_map(|assoc| {
            if let ArchiveState::LoversLabOAuthDownloader { image_url, .. } = &assoc.source {
                image_url.as_ref()
            } else {
                None
            }
        })
        .ok_or_else(|| actix_web::error::ErrorNotFound("Mod image not found"))?;

    // Fetch the image from the upstream URL
    let client = reqwest::Client::new();
    let response = client.get(image_url).send().await.map_err(|e| {
        log::error!("Failed to fetch mod image from {}: {}", image_url, e);
        actix_web::error::ErrorInternalServerError("Failed to fetch mod image")
    })?;

    if !response.status().is_success() {
        return Err(actix_web::error::ErrorNotFound("Mod image not found"));
    }

    // Determine content type from response or default to image/jpeg
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());

    // Get the image bytes
    let image_bytes = response.bytes().await.map_err(|e| {
        log::error!("Failed to read mod image bytes: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to read mod image")
    })?;

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(image_bytes))
}

#[post("/mod/{id}/toggle-lost-forever")]
pub async fn toggle_lost_forever(
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

    mod_item.toggle_lost_forever(&conn).map_err(|e| match e {
        crate::db::mod_data::ToggleLostForeverError::ModHasDiskFilename => {
            actix_web::error::ErrorBadRequest(
                "Cannot mark mod as lost forever when disk_filename is set",
            )
        }
        crate::db::mod_data::ToggleLostForeverError::DatabaseError(e) => {
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        }
    })?;

    // Redirect back to the mod details page
    Ok(HttpResponse::SeeOther()
        .append_header(("Location", format!("/mod/{}", mod_id)))
        .finish())
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

    // Get associations for all mods in this modlist
    let associations = ModAssociation::get_by_modlist_id(archive_id, &conn)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Create a map from mod_id to association for quick lookup
    use std::collections::HashMap;
    let assoc_map: HashMap<u64, &ModAssociation> = associations
        .iter()
        .map(|assoc| (assoc.mod_id, assoc))
        .collect();

    // Separate unavailable mods for the missing mods table
    let unavailable_mods: Vec<_> = mods.iter().filter(|m| !m.is_available()).cloned().collect();
    let show_missing_table = !unavailable_mods.is_empty() && unavailable_mods.len() < 25;

    // Create tuples with mods and their associations for rendering
    let unavailable_mods_with_assocs: Vec<_> = unavailable_mods
        .iter()
        .map(|mod_item| {
            let assoc = assoc_map.get(&mod_item.id).cloned();
            (mod_item, assoc)
        })
        .collect();

    let mods_with_assocs: Vec<_> = mods
        .iter()
        .map(|mod_item| {
            let assoc = assoc_map.get(&mod_item.id).cloned();
            (mod_item, assoc)
        })
        .collect();

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
                            p { strong { "Hash: " } span.hash { code { (format_hash(&modlist.xxhash64)) } } }
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
                                @for (mod_item, assoc) in &unavailable_mods_with_assocs {
                                    tr {
                                        td.filename {
                                            a href=(format!("/mod/{}", mod_item.id)) {
                                                @match assoc {
                                                    Some(assoc) => {
                                                        (assoc.filename.clone())
                                                    }
                                                    None => {
                                                        @match &mod_item.disk_filename {
                                                            Some(disk_filename) => {
                                                                (disk_filename.clone())
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
                                                @match assoc {
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
                                            @match assoc {
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
                                @for (mod_item, assoc) in &mods_with_assocs {
                                    tr {
                                        td.filename {
                                            a href=(format!("/mod/{}", mod_item.id)) {
                                                @match assoc {
                                                    Some(assoc) => {
                                                        (assoc.filename.clone())
                                                    }
                                                    None => {
                                                        @match &mod_item.disk_filename {
                                                            Some(disk_filename) => {
                                                                (disk_filename.clone())
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
                                                @match assoc {
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
                                            @match assoc {
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
                                        td.size {
                                            (format_size(mod_item.size))
                                        }
                                        td.hash {
                                            code { (format_hash(&mod_item.xxhash64)) }
                                        }
                                        td.status {
                                            @if mod_item.is_available() {
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
