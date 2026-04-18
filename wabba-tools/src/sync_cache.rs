use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const CACHE_FILENAME: &str = ".wabba-sync-cache.json";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SyncCache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub size: u64,
    pub mtime_nanos: i128,
    pub hash: String,
}

pub fn cache_path(dir: &Path) -> PathBuf {
    dir.join(CACHE_FILENAME)
}

/// Read the (size, mtime) pair used as the cache key for a file. mtime is
/// expressed as signed nanoseconds since UNIX_EPOCH — signed because dates
/// prior to 1970 round-trip as negatives.
pub fn file_fingerprint(metadata: &fs::Metadata) -> (u64, i128) {
    let size = metadata.len();
    let mtime_nanos = match metadata.modified() {
        Ok(t) => match t.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_nanos() as i128,
            Err(e) => -(e.duration().as_nanos() as i128),
        },
        Err(_) => 0,
    };
    (size, mtime_nanos)
}

impl SyncCache {
    pub fn load(dir: &Path) -> Self {
        let path = cache_path(dir);
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                log::warn!(
                    "Cache file at {} is unreadable ({}), ignoring",
                    path.display(),
                    e
                );
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                log::warn!(
                    "Failed to read cache file at {}: {}, ignoring",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// Serialize to a temp file and rename over the real cache path. The
    /// rename is atomic within a single filesystem, so an interrupted write
    /// leaves either the previous file intact or the new one — never a
    /// half-written JSON that would fail to parse on the next run.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let path = cache_path(dir);
        let tmp_name = format!(
            "{}.tmp",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| CACHE_FILENAME.to_string())
        );
        let tmp_path = path.with_file_name(tmp_name);
        let json = serde_json::to_string(self).expect("SyncCache serializes");
        fs::write(&tmp_path, json)?;
        fs::rename(&tmp_path, &path)
    }

    pub fn lookup(&self, filename: &str, size: u64, mtime_nanos: i128) -> Option<String> {
        let entry = self.entries.get(filename)?;
        if entry.size == size && entry.mtime_nanos == mtime_nanos {
            Some(entry.hash.clone())
        } else {
            None
        }
    }

    pub fn insert(&mut self, filename: String, size: u64, mtime_nanos: i128, hash: String) {
        self.entries.insert(
            filename,
            CacheEntry {
                size,
                mtime_nanos,
                hash,
            },
        );
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
