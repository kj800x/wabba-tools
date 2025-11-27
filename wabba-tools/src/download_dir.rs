use std::{fs, path::PathBuf};

pub struct DownloadDirectory {
    path: PathBuf,
}

impl DownloadDirectory {
    pub fn new(path: &PathBuf) -> Result<DownloadDirectory, Box<dyn std::error::Error>> {
        let path = PathBuf::from(path);
        Ok(DownloadDirectory { path })
    }

    pub fn files(&self) -> Vec<String> {
        fs::read_dir(&self.path)
            .expect("Failed to read download directory")
            .map(|x| {
                x.expect("Failed to read entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .filter(|x| !x.ends_with(".meta"))
            .collect::<Vec<String>>()
    }
}
