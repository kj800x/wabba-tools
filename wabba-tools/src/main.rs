use crate::download_dir::DownloadDirectory;
use crate::sync_cache::{CACHE_FILENAME, SyncCache, file_fingerprint};
use clap::Parser;
mod cli;
mod download_dir;
mod sync_cache;
use env_logger::Builder;
use reqwest::Client;
use reqwest::header::IF_NONE_MATCH;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::fs::File;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::codec::{BytesCodec, FramedRead};
use wabba_protocol::{hash::Hash, wabbajack::WabbajackMetadata};

#[derive(Debug)]
struct FileComparisonResult {
    missing_files: Vec<String>,
    satisfied_files: Vec<String>,
    extraneous_files: Vec<String>,
}

#[derive(Clone, Copy)]
enum UploadType {
    Modlist,
    Mod,
}

impl UploadType {
    fn from_extension(extension: &str) -> Self {
        match extension {
            "wabbajack" => Self::Modlist,
            _ => Self::Mod,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Modlist => "modlist",
            Self::Mod => "mod",
        }
    }
}

fn upload_type_for(path: &Path) -> UploadType {
    UploadType::from_extension(
        path.extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default(),
    )
}

enum UploadOutcome {
    Uploaded,
    AlreadyPresent,
    Failed(u16, String),
}

/// Probe the server once and return the post-redirect base URL. Reqwest
/// follows GET redirects transparently but cannot replay a streamed POST
/// body, so we resolve any redirect chain (e.g. Traefik's HTTP→HTTPS 308)
/// up front and use the resolved base URL for the rest of the run.
async fn resolve_base_url(client: &Client, server: &str) -> Result<String, reqwest::Error> {
    let server = server.trim_end_matches('/');
    let probe_url = format!("{}/hello", server);
    let response = client.get(&probe_url).send().await?;
    let final_url = response.url().as_str();
    let resolved = final_url
        .trim_end_matches("/hello")
        .trim_end_matches('/')
        .to_string();
    if resolved != server {
        log::info!("Resolved server URL {} -> {}", server, resolved);
    }
    Ok(resolved)
}

/// Ask the server whether it already has a file with the given hash. Returns
/// true when the server reports the hash is already available (304), false
/// when the server needs the upload (200).
async fn server_has_hash(
    client: &Client,
    server: &str,
    upload_type: UploadType,
    hash: &str,
) -> Result<bool, reqwest::Error> {
    let url = format!("{}/check/{}", server, upload_type.as_str());
    let response = client
        .get(&url)
        .header(IF_NONE_MATCH, hash)
        .send()
        .await?;
    Ok(response.status().as_u16() == 304)
}

/// Stream a single file up to the server. The caller is responsible for
/// deciding whether the upload is needed; this function will submit the body
/// regardless.
async fn upload_file(
    client: &Client,
    server: &str,
    file: &Path,
    hash: &str,
) -> Result<UploadOutcome, Box<dyn std::error::Error>> {
    let upload_type = upload_type_for(file);
    let filename = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let url = format!("{}/submit/{}/{}", server, upload_type.as_str(), filename);

    let async_file = File::open(file).await?;
    let stream = FramedRead::new(async_file, BytesCodec::new());
    let body = reqwest::Body::wrap_stream(stream);

    log::info!("POST {}", url);
    let response = client
        .post(&url)
        .header(IF_NONE_MATCH, hash)
        .body(body)
        .send()
        .await?;

    let code = response.status().as_u16();
    match code {
        200 => Ok(UploadOutcome::Uploaded),
        304 => Ok(UploadOutcome::AlreadyPresent),
        _ => {
            let body = response.text().await.unwrap_or_default();
            Ok(UploadOutcome::Failed(code, body))
        }
    }
}

// Compare two lists of files and return:
// - A list of files that are missing
// - A list of files that are satisfied
// - A list of files that are extraneous
fn compare_file_lists(
    required_files: &Vec<String>,
    files_in_download_dir: &Vec<String>,
) -> FileComparisonResult {
    let mut result = FileComparisonResult {
        missing_files: Vec::new(),
        satisfied_files: Vec::new(),
        extraneous_files: Vec::new(),
    };

    for file in files_in_download_dir {
        if !required_files.contains(&file) {
            result.extraneous_files.push(file.clone());
        }
    }

    for file in required_files {
        if files_in_download_dir.contains(&file) {
            result.satisfied_files.push(file.clone());
        } else {
            result.missing_files.push(file.clone());
        }
    }

    result
}

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();

    Builder::from_default_env()
        .filter_level(match cli.debug {
            0 => log::LevelFilter::Info,
            1 => log::LevelFilter::Debug,
            2 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Trace,
        })
        .init();

    match &cli.command {
        cli::Commands::Validate {
            wabbajack_file,
            download_dirs,
        } => {
            let metadata =
                WabbajackMetadata::load(wabbajack_file).expect("Failed to load Wabbajack metadata");

            log::info!("Required archives: {:#?}", metadata.required_archives());

            let files_from_unknown_downloaders = metadata.files_from_unknown_downloaders();
            if !files_from_unknown_downloaders.is_empty() {
                log::warn!(
                    "Found files with unknown downloaders. The results of wabba-tools may be incorrect: {:#?}",
                    files_from_unknown_downloaders
                );
            } else {
                log::info!("No files with unknown downloaders found");
            }

            let required_files = metadata.required_files();
            let download_directory = DownloadDirectory::new(&download_dirs[0])
                .expect("Failed to create download directory");

            let result = compare_file_lists(&required_files, &download_directory.files());

            log::info!("Missing files: {:#?}", result.missing_files);
        }

        cli::Commands::Hash { file } => {
            let hash = Hash::compute(&std::fs::read(file).expect("Failed to read file"));
            log::info!("Hash: {}", hash);
        }

        cli::Commands::Upload { server, file } => {
            log::info!("Computing hash for {}", file.display());
            let hash = Hash::compute(&std::fs::read(file).expect("Failed to read file"));

            let client = Client::new();
            let server = match resolve_base_url(&client, server).await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to reach server: {}", e);
                    return;
                }
            };
            let server = server.as_str();
            match upload_file(&client, server, file, &hash).await {
                Ok(UploadOutcome::Uploaded) => log::info!("Upload successful"),
                Ok(UploadOutcome::AlreadyPresent) => log::info!("File already exists"),
                Ok(UploadOutcome::Failed(code, body)) => {
                    log::error!("Upload failed: {}", code);
                    log::error!("Response body: {}", body);
                }
                Err(e) => log::error!("Upload error: {}", e),
            }
        }

        cli::Commands::Sync {
            server,
            directory,
            no_cache,
            parallel,
        } => {
            let client = Client::new();
            let server = match resolve_base_url(&client, server).await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to reach server: {}", e);
                    return;
                }
            };
            let server = server.as_str();

            let download_directory =
                DownloadDirectory::new(directory).expect("Failed to open directory");

            let files: Vec<PathBuf> = download_directory
                .file_paths()
                .into_iter()
                .filter(|p| {
                    p.file_name().and_then(|n| n.to_str()) != Some(CACHE_FILENAME)
                })
                .collect();
            log::info!(
                "Found {} candidate files in {}",
                files.len(),
                directory.display()
            );

            let parallelism = parallel
                .or_else(|| {
                    std::thread::available_parallelism()
                        .ok()
                        .map(|n| n.get())
                })
                .unwrap_or(1)
                .max(1);
            let use_cache = !no_cache;

            let old_cache = Arc::new(if use_cache {
                SyncCache::load(directory)
            } else {
                SyncCache::default()
            });
            if use_cache {
                log::info!("Loaded {} cached hashes from {}", old_cache.len(), directory.display());
            } else {
                log::info!("Cache disabled (--no-cache); rehashing every file");
            }
            let new_cache = Arc::new(Mutex::new(SyncCache::default()));

            log::info!(
                "Hashing {} files with parallelism={}",
                files.len(),
                parallelism
            );

            let sem = Arc::new(Semaphore::new(parallelism));
            let mut set: JoinSet<(PathBuf, Result<String, String>)> = JoinSet::new();
            let total = files.len();

            // Spawn every task up front so the `for` loop returns immediately
            // and `join_next()` below can start draining (and logging) in
            // parallel with hashing. Each task waits on the semaphore
            // internally, so we only have `parallelism` hashers at a time.
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
                            let metadata = std::fs::metadata(&file)
                                .map_err(|e| format!("stat: {}", e))?;
                            let (size, mtime_nanos) = file_fingerprint(&metadata);

                            if let Some(cached) =
                                old_cache.lookup(&filename, size, mtime_nanos)
                            {
                                log::debug!("Cache hit for {}", filename);
                                new_cache.lock().unwrap().insert(
                                    filename.clone(),
                                    size,
                                    mtime_nanos,
                                    cached.clone(),
                                );
                                return Ok(cached);
                            }

                            let hash = Hash::compute_file(&file)
                                .map_err(|e| format!("hash: {}", e))?;
                            new_cache.lock().unwrap().insert(
                                filename,
                                size,
                                mtime_nanos,
                                hash.clone(),
                            );
                            Ok(hash)
                        })();
                        (file, result)
                    })
                    .await
                    .expect("blocking hash task panicked")
                });
            }

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
                        log::error!("[{}/{}] Failed to hash {}: {}", completed, total, filename, e);
                        failed += 1;
                    }
                }
            }

            // Persist the cache before uploading so an interrupted upload phase
            // doesn't force a rehash next run.
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

            // Sort by filename for deterministic upload order + log output.
            hashed.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));

            let mut uploaded = 0usize;
            let mut skipped = 0usize;

            for (idx, (file, hash)) in hashed.iter().enumerate() {
                let filename = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("<unknown>");
                let upload_type = upload_type_for(file);
                match server_has_hash(&client, server, upload_type, hash).await {
                    Ok(true) => {
                        log::info!(
                            "[{}/{}] Server already has {} — skipping",
                            idx + 1,
                            hashed.len(),
                            filename
                        );
                        skipped += 1;
                        continue;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        log::error!("Hash check failed for {}: {}", filename, e);
                        failed += 1;
                        continue;
                    }
                }

                log::info!("[{}/{}] Uploading {}", idx + 1, hashed.len(), filename);
                match upload_file(&client, server, file, hash).await {
                    Ok(UploadOutcome::Uploaded) => {
                        log::info!("Uploaded {}", filename);
                        uploaded += 1;
                    }
                    Ok(UploadOutcome::AlreadyPresent) => {
                        log::info!("Server reported {} already present", filename);
                        skipped += 1;
                    }
                    Ok(UploadOutcome::Failed(code, body)) => {
                        log::error!("Upload of {} failed: {} — {}", filename, code, body);
                        failed += 1;
                    }
                    Err(e) => {
                        log::error!("Upload error for {}: {}", filename, e);
                        failed += 1;
                    }
                }
            }

            log::info!(
                "Sync complete: {} uploaded, {} already present, {} failed",
                uploaded,
                skipped,
                failed
            );
        }
    }

    // let result = compare_file_lists(&required_files, &files_in_download_dir);

    // let potential_remote_dirs = vec![
    //     "/mnt/users/prensox/WabbajackRepo/downloads",
    //     "/mnt/users/prensox/WabbajackRepo/Wabbajack Backup",
    // ]
    // .into_iter()
    // .map(PathBuf::from)
    // .collect::<Vec<PathBuf>>();

    // // for each file in result.missing_files, check if it exists in potential_remote_dirs
    // for missing_file in &result.missing_files {
    //     let mut found = false;
    //     for dir in &potential_remote_dirs {
    //         let file_path = dir.join(missing_file);
    //         if file_path.exists() {
    //             println!("Found missing file: {} in {}", missing_file, dir.display());
    //             found = true;
    //             break;
    //         }
    //     }
    //     if !found {
    //         println!("File still missing: {}", missing_file);
    //     }
    // }

    // // for each file in result.missing_files, check if it exists in potential_remote_dirs
    // let mut i = 0;
    // let n = result.missing_files.len();
    // for missing_file in &result.missing_files {
    //     i = i + 1;
    //     println!("{}/{}", i + 1, n);
    //     for dir in &potential_remote_dirs {
    //         let file_path = dir.join(missing_file);
    //         let meta_file_path = file_path.with_meta_extension();
    //         if file_path.exists() {
    //             println!("Recovering: {}", missing_file);
    //             let destination = PathBuf::from(download_dir).join(missing_file);
    //             fs::copy(&file_path, &destination).expect("Failed to copy file");
    //             println!("Recovered {} to {}", missing_file, destination.display());

    //             if meta_file_path.exists() {
    //                 let destination_meta = PathBuf::from(download_dir)
    //                     .join(missing_file)
    //                     .with_meta_extension();
    //                 fs::copy(&meta_file_path, &destination_meta).expect("Failed to copy meta file");
    //                 println!(
    //                     "Recovered meta file for {} to {}",
    //                     missing_file,
    //                     destination_meta.display()
    //                 );
    //             } else {
    //                 println!("No meta file found for {}", missing_file);
    //             }

    //             break;
    //         }
    //     }
    // }

    // println!("{:#?}", result);
}

// trait FileExt {
//     fn with_meta_extension(&self) -> PathBuf;
// }

// impl FileExt for PathBuf {
//     fn with_meta_extension(&self) -> PathBuf {
//         let mut meta_extension = self.extension().unwrap_or_default().to_os_string();
//         meta_extension.push(".meta");
//         self.with_extension(meta_extension)
//     }
// }
