use crate::sync_cache::{SyncCache, file_fingerprint};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use wabba_protocol::hash::Hash;

pub struct HashResults {
    pub hashed: Vec<(PathBuf, String)>,
    pub failed: usize,
}

/// Hash every file in `files`, reusing the on-disk sync cache to skip files
/// whose (size, mtime) fingerprint is unchanged. This is the hashing phase
/// shared by the `sync` and `prune` commands: load the cache, hash with
/// bounded parallelism, flush periodically, and save at the end. Newly hashed
/// files are recorded back into `.wabba-sync-cache.json` (when caching is on)
/// so later runs can skip them.
pub async fn hash_files_with_cache(
    directory: &Path,
    files: Vec<PathBuf>,
    use_cache: bool,
    parallelism: usize,
) -> HashResults {
    let parallelism = parallelism.max(1);
    let total = files.len();

    let old_cache = Arc::new(if use_cache {
        SyncCache::load(directory)
    } else {
        SyncCache::default()
    });
    if use_cache {
        log::info!(
            "Loaded {} cached hashes from {}",
            old_cache.len(),
            directory.display()
        );
    } else {
        log::info!("Cache disabled (--no-cache); rehashing every file");
    }
    let new_cache = Arc::new(Mutex::new(SyncCache::default()));

    log::info!("Hashing {} files with parallelism={}", total, parallelism);

    let sem = Arc::new(Semaphore::new(parallelism));
    let mut set: JoinSet<(PathBuf, Result<String, String>)> = JoinSet::new();

    // Spawn every task up front so the `for` loop returns immediately and
    // `join_next()` below can start draining (and logging) in parallel with
    // hashing. Each task waits on the semaphore internally, so we only have
    // `parallelism` hashers at a time.
    for file in files.into_iter() {
        let sem = Arc::clone(&sem);
        let old_cache = Arc::clone(&old_cache);
        let new_cache = Arc::clone(&new_cache);
        set.spawn(async move {
            let permit = sem.acquire_owned().await.expect("semaphore not closed");
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let filename = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                let result = (|| -> Result<String, String> {
                    let metadata = std::fs::metadata(&file).map_err(|e| format!("stat: {}", e))?;
                    let (size, mtime_nanos) = file_fingerprint(&metadata);

                    if let Some(cached) = old_cache.lookup(&filename, size, mtime_nanos) {
                        log::debug!("Cache hit for {}", filename);
                        new_cache.lock().unwrap().insert(
                            filename.clone(),
                            size,
                            mtime_nanos,
                            cached.clone(),
                        );
                        return Ok(cached);
                    }

                    let hash = Hash::compute_file(&file).map_err(|e| format!("hash: {}", e))?;
                    new_cache
                        .lock()
                        .unwrap()
                        .insert(filename, size, mtime_nanos, hash.clone());
                    Ok(hash)
                })();
                (file, result)
            })
            .await
            .expect("blocking hash task panicked")
        });
    }

    // Flush the cache every N completed hashes so ctrl-c during the hash phase
    // loses at most N-1 entries of work. The atomic save() keeps the on-disk
    // file always consistent.
    const CACHE_FLUSH_INTERVAL: usize = 50;

    let mut hashed: Vec<(PathBuf, String)> = Vec::with_capacity(total);
    let mut failed = 0usize;
    let mut completed = 0usize;
    while let Some(joined) = set.join_next().await {
        let (file, result) = joined.expect("hash task panicked");
        completed += 1;
        let filename = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>")
            .to_string();
        match result {
            Ok(hash) => {
                log::info!("[{}/{}] Hashed {}", completed, total, filename);
                hashed.push((file, hash));
            }
            Err(e) => {
                log::error!(
                    "[{}/{}] Failed to hash {}: {}",
                    completed,
                    total,
                    filename,
                    e
                );
                failed += 1;
            }
        }

        if use_cache && completed.is_multiple_of(CACHE_FLUSH_INTERVAL) {
            let snapshot = new_cache.lock().unwrap().clone();
            if let Err(e) = snapshot.save(directory) {
                log::warn!("Cache flush failed at {} entries: {}", completed, e);
            } else {
                log::debug!("Flushed cache ({}/{} files hashed)", completed, total);
            }
        }
    }

    // Final save — covers the last partial batch and any error paths that
    // skipped the interval flush.
    if use_cache {
        let cache = Arc::try_unwrap(new_cache)
            .expect("cache Arc should be unique now")
            .into_inner()
            .expect("mutex not poisoned");
        if let Err(e) = cache.save(directory) {
            log::warn!("Failed to save hash cache: {}", e);
        } else {
            log::info!("Saved {} hashes to {}", cache.len(), directory.display());
        }
    }

    HashResults { hashed, failed }
}
