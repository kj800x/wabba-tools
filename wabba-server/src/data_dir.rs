use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct DataDir(PathBuf);

impl DataDir {
    pub fn new(path: &PathBuf) -> Result<DataDir, Box<dyn std::error::Error>> {
        let path = PathBuf::from(path);
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        if !path.is_dir() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Path is not a directory",
            )));
        }

        std::fs::create_dir_all(&path.join("Modlists")).unwrap();
        std::fs::create_dir_all(&path.join("Downloads")).unwrap();

        Ok(DataDir(path))
    }

    pub fn get_path(&self) -> &PathBuf {
        &self.0
    }

    pub fn get_db_path(&self) -> PathBuf {
        self.0.join("db.db")
    }

    pub fn get_modlist_dir(&self) -> PathBuf {
        self.0.join("Modlists")
    }

    pub fn get_mod_dir(&self) -> PathBuf {
        self.0.join("Downloads")
    }

    pub fn get_modlist_path(&self, modlist_filename: &str) -> PathBuf {
        self.get_modlist_dir().join(modlist_filename)
    }

    pub fn get_mod_path(&self, mod_filename: &str) -> PathBuf {
        self.get_mod_dir().join(mod_filename)
    }
}
