#![allow(unused)]

use serde::Deserialize;
use std::{fs, path::PathBuf};
use zip::ZipArchive;

use crate::archive_state::ArchiveState;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Archive {
    pub hash: String,
    pub meta: String,
    #[serde(rename = "Name")]
    pub filename: String,
    pub size: u64,
    pub state: ArchiveState,
}

impl Archive {
    pub fn name(&self) -> Option<String> {
        self.state.name()
    }

    pub fn version(&self) -> Option<String> {
        self.state.version()
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WabbajackMetadata {
    pub archives: Vec<Archive>,
    pub author: String,
    pub description: String,
    pub directives: Vec<serde_json::Value>,
    pub version: String,
    pub game_type: String,
    pub image: String,
    pub name: String,
    pub readme: String,
    pub wabbajack_version: String,
    pub website: String,
    #[serde(rename = "IsNSFW")]
    pub is_nsfw: bool,
}

impl WabbajackMetadata {
    pub fn load(path: &PathBuf) -> Result<WabbajackMetadata, Box<dyn std::error::Error>> {
        let mut zip = ZipArchive::new(fs::File::open(path)?)?;
        let mut file = zip.by_name("modlist")?;
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut file, &mut contents)?;

        let raw_value: serde_json::Value = serde_json::from_str(&contents)?;
        let formatted_value = serde_json::to_string_pretty(&raw_value)?;

        log::debug!("Wabbajack metadata: {}", formatted_value);

        let metadata: WabbajackMetadata = serde_json::from_str(&formatted_value)?;
        Ok(metadata)
    }

    pub fn files_from_unknown_downloaders(&self) -> Vec<String> {
        self.archives
            .iter()
            .filter(|x| matches!(x.state, ArchiveState::UnknownDownloader))
            .map(|x| x.filename.clone())
            .collect::<Vec<String>>()
    }

    pub fn required_archives(&self) -> Vec<&Archive> {
        self.archives
            .iter()
            .filter(|x| x.state.requires_download())
            .collect()
    }

    pub fn required_files(&self) -> Vec<String> {
        self.required_archives()
            .iter()
            .map(|x| x.filename.clone())
            .collect()
    }
}

// fn print_with_line_numbers(text: &str) {
//     let lines = text.lines();
//     for (i, line) in lines.enumerate() {
//         println!("{:4}: {}", i + 1, line);
//     }
// }
