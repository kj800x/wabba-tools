#![allow(unused)]
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "$type")]
pub enum ArchiveState {
    #[serde(rename = "NexusDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    NexusDownloader {
        author: Option<String>,
        description: String,
        #[serde(rename = "FileID")]
        file_id: u64,
        game_name: String,
        #[serde(rename = "ImageURL")]
        image_url: Option<String>,
        #[serde(rename = "IsNSFW")]
        is_nsfw: bool,
        #[serde(rename = "ModID")]
        mod_id: u64,
        name: String,
        version: String,
    },

    #[serde(rename = "HttpDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    HttpDownloader {
        url: String,
        headers: serde_json::Value,
    },

    #[serde(rename = "GameFileSourceDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    GameFileSourceDownloader {
        game: String,
        game_file: String,
        game_version: String,
        hash: String,
    },

    #[serde(rename = "WabbajackCDNDownloader+State, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    WabbajackCDNDownloader { url: String },

    #[serde(rename = "ManualDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    ManualDownloader { prompt: String, url: String },

    #[serde(rename = "MegaDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    MegaDownloader { url: String },

    #[serde(rename = "GoogleDriveDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    GoogleDriveDownloader { id: String },

    #[serde(rename = "MediaFireDownloader+State, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    MediaFireDownloader { url: String },

    #[serde(rename = "LoversLabOAuthDownloader, Wabbajack.Lib")]
    #[serde(rename_all = "PascalCase")]
    LoversLabOAuthDownloader {
        author: Option<String>,
        description: Option<String>,
        #[serde(rename = "IPS4File")]
        ips4_file: Option<String>,
        #[serde(rename = "IPS4Mod")]
        ips4_mod: u64,
        #[serde(rename = "IPS4Url")]
        ips4_url: String,
        #[serde(rename = "ImageURL")]
        image_url: Option<String>,
        is_attachment: bool,
        #[serde(rename = "IsNSFW")]
        is_nsfw: bool,
        name: Option<String>,
        primary_key_string: String,
        #[serde(rename = "URL")]
        url: String,
        version: Option<String>,
    },

    #[serde(other)]
    UnknownDownloader,
}

impl ArchiveState {
    pub fn requires_download(&self) -> bool {
        match self {
            ArchiveState::NexusDownloader { .. }
            | ArchiveState::HttpDownloader { .. }
            | ArchiveState::WabbajackCDNDownloader { .. }
            | ArchiveState::ManualDownloader { .. }
            | ArchiveState::MegaDownloader { .. }
            | ArchiveState::GoogleDriveDownloader { .. }
            | ArchiveState::MediaFireDownloader { .. }
            | ArchiveState::LoversLabOAuthDownloader { .. }
            | ArchiveState::UnknownDownloader => true,

            ArchiveState::GameFileSourceDownloader { .. } => false,
        }
    }

    /// Short human-readable label for the download source type. Matches the
    /// labels rendered on the mod details page.
    pub fn source_label(&self) -> &'static str {
        match self {
            ArchiveState::NexusDownloader { .. } => "Nexus Mods",
            ArchiveState::HttpDownloader { .. } => "HTTP Download",
            ArchiveState::WabbajackCDNDownloader { .. } => "Wabbajack CDN",
            ArchiveState::ManualDownloader { .. } => "Manual Download",
            ArchiveState::MegaDownloader { .. } => "MEGA",
            ArchiveState::GoogleDriveDownloader { .. } => "Google Drive",
            ArchiveState::MediaFireDownloader { .. } => "MediaFire",
            ArchiveState::LoversLabOAuthDownloader { .. } => "LoversLab",
            ArchiveState::GameFileSourceDownloader { .. } => "Game File",
            ArchiveState::UnknownDownloader => "Unknown Source",
        }
    }

    /// Terse label for the source chip shown in dense list/table views.
    /// Keep these short; the full [`Self::source_label`] is used on detail pages.
    pub fn source_chip_label(&self) -> &'static str {
        match self {
            ArchiveState::NexusDownloader { .. } => "Nexus",
            ArchiveState::HttpDownloader { .. } => "HTTP",
            ArchiveState::WabbajackCDNDownloader { .. } => "CDN",
            ArchiveState::ManualDownloader { .. } => "Manual",
            ArchiveState::MegaDownloader { .. } => "MEGA",
            ArchiveState::GoogleDriveDownloader { .. } => "GDrive",
            ArchiveState::MediaFireDownloader { .. } => "MediaFire",
            ArchiveState::LoversLabOAuthDownloader { .. } => "LoversLab",
            ArchiveState::GameFileSourceDownloader { .. } => "Game",
            ArchiveState::UnknownDownloader => "Unknown",
        }
    }

    /// External URL where this archive can be found / downloaded from upstream,
    /// if one applies. Used for the "Open Source" action on the mod details page.
    pub fn source_url(&self) -> Option<String> {
        match self {
            ArchiveState::NexusDownloader {
                game_name, mod_id, ..
            } => Some(format!(
                "https://www.nexusmods.com/{}/mods/{}",
                game_name.to_lowercase().replace(' ', ""),
                mod_id
            )),
            ArchiveState::HttpDownloader { url, .. } => Some(url.clone()),
            ArchiveState::WabbajackCDNDownloader { url } => Some(url.clone()),
            ArchiveState::ManualDownloader { url, .. } => Some(url.clone()),
            ArchiveState::MegaDownloader { url } => Some(url.clone()),
            ArchiveState::MediaFireDownloader { url } => Some(url.clone()),
            ArchiveState::LoversLabOAuthDownloader { url, .. } => Some(url.clone()),
            ArchiveState::GoogleDriveDownloader { id } => {
                Some(format!("https://drive.google.com/file/d/{}/view", id))
            }
            ArchiveState::GameFileSourceDownloader { .. } | ArchiveState::UnknownDownloader => None,
        }
    }

    /// CSS class suffix used to colour the source chip (e.g. `nexus` -> `.source-chip.nexus`).
    pub fn source_chip_class(&self) -> &'static str {
        match self {
            ArchiveState::NexusDownloader { .. } => "nexus",
            ArchiveState::HttpDownloader { .. } => "http",
            ArchiveState::WabbajackCDNDownloader { .. } => "cdn",
            ArchiveState::ManualDownloader { .. } => "manual",
            ArchiveState::MegaDownloader { .. } => "mega",
            ArchiveState::GoogleDriveDownloader { .. } => "gdrive",
            ArchiveState::MediaFireDownloader { .. } => "mediafire",
            ArchiveState::LoversLabOAuthDownloader { .. } => "loverslab",
            ArchiveState::GameFileSourceDownloader { .. } => "game",
            ArchiveState::UnknownDownloader => "unknown",
        }
    }

    pub fn name(&self) -> Option<String> {
        match self {
            ArchiveState::NexusDownloader { name, .. } => Some(name.clone()),
            ArchiveState::LoversLabOAuthDownloader { name, .. } => name.clone(),
            ArchiveState::HttpDownloader { .. }
            | ArchiveState::GameFileSourceDownloader { .. }
            | ArchiveState::WabbajackCDNDownloader { .. }
            | ArchiveState::ManualDownloader { .. }
            | ArchiveState::MegaDownloader { .. }
            | ArchiveState::GoogleDriveDownloader { .. }
            | ArchiveState::MediaFireDownloader { .. }
            | ArchiveState::UnknownDownloader => None,
        }
    }

    pub fn version(&self) -> Option<String> {
        match self {
            ArchiveState::NexusDownloader { version, .. } => Some(version.clone()),
            ArchiveState::LoversLabOAuthDownloader { version, .. } => version.clone(),
            ArchiveState::HttpDownloader { .. }
            | ArchiveState::GameFileSourceDownloader { .. }
            | ArchiveState::WabbajackCDNDownloader { .. }
            | ArchiveState::ManualDownloader { .. }
            | ArchiveState::MegaDownloader { .. }
            | ArchiveState::GoogleDriveDownloader { .. }
            | ArchiveState::MediaFireDownloader { .. }
            | ArchiveState::UnknownDownloader => None,
        }
    }
}
