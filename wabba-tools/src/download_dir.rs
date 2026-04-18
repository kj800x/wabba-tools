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

    pub fn file_paths(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.path)
            .expect("Failed to read download directory")
            .filter_map(|entry| {
                let entry = entry.expect("Failed to read entry");
                let path = entry.path();
                if !path.is_file() {
                    return None;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".meta") {
                    return None;
                }
                Some(path)
            })
            .collect()
    }
}
