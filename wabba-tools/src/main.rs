use crate::download_dir::DownloadDirectory;
use crate::hashing::hash_files_with_cache;
use crate::sync_cache::CACHE_FILENAME;
use clap::Parser;
mod cli;
mod download_dir;
mod hashing;
mod sync_cache;
use env_logger::Builder;
use reqwest::Client;
use reqwest::header::IF_NONE_MATCH;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs::File;
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
    let response = client.get(&url).header(IF_NONE_MATCH, hash).send().await?;
    Ok(response.status().as_u16() == 304)
}

/// Server's answer to `GET /resolve` for a single `--keep` hash. `mod_hashes`
/// lists the xxhash64 of every mod required by a modlist; it is empty for a mod.
#[derive(serde::Deserialize)]
struct KeepResolution {
    kind: String,
    hash: String,
    mod_hashes: Vec<String>,
}

/// Resolve a `--keep` hash to a mod or modlist on the server. Returns `None`
/// when the server does not know the hash (404).
async fn resolve_keep_hash(
    client: &Client,
    server: &str,
    hash: &str,
) -> Result<Option<KeepResolution>, reqwest::Error> {
    let url = format!("{}/resolve", server);
    let response = client.get(&url).header(IF_NONE_MATCH, hash).send().await?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    let resolution = response
        .error_for_status()?
        .json::<KeepResolution>()
        .await?;
    Ok(Some(resolution))
}

/// Append `.meta` to a path, yielding the sidecar metadata file Wabbajack
/// stores next to each download (e.g. `foo.7z` -> `foo.7z.meta`).
fn meta_path_for(path: &Path) -> PathBuf {
    let mut os = path.to_path_buf().into_os_string();
    os.push(".meta");
    PathBuf::from(os)
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
        if !required_files.contains(file) {
            result.extraneous_files.push(file.clone());
        }
    }

    for file in required_files {
        if files_in_download_dir.contains(file) {
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
                .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some(CACHE_FILENAME))
                .collect();
            log::info!(
                "Found {} candidate files in {}",
                files.len(),
                directory.display()
            );

            let parallelism = (*parallel).max(1);
            let use_cache = !no_cache;

            let hashing::HashResults {
                mut hashed,
                mut failed,
            } = hash_files_with_cache(directory, files, use_cache, parallelism).await;

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

        cli::Commands::Prune {
            server,
            directory,
            keep,
            dry_run,
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

            // Resolve every --keep hash into the set of hashes we must retain:
            // the named hash itself, plus (for a modlist) every mod it requires.
            // Abort if any --keep hash is unknown to the server, so we never
            // prune against an incomplete keep set.
            let mut keep_set: HashSet<String> = HashSet::new();
            let mut missing: Vec<String> = Vec::new();
            for keep_hash in keep {
                match resolve_keep_hash(&client, server, keep_hash).await {
                    Ok(Some(resolution)) => {
                        keep_set.insert(resolution.hash.clone());
                        let mod_count = resolution.mod_hashes.len();
                        keep_set.extend(resolution.mod_hashes);
                        log::info!(
                            "Resolved --keep {} as {} (keeping {} required mods)",
                            keep_hash,
                            resolution.kind,
                            mod_count
                        );
                    }
                    Ok(None) => missing.push(keep_hash.clone()),
                    Err(e) => {
                        log::error!("Failed to resolve --keep {}: {}", keep_hash, e);
                        return;
                    }
                }
            }
            if !missing.is_empty() {
                log::error!(
                    "Aborting: these --keep hashes were not found on the server: {:?}",
                    missing
                );
                return;
            }
            log::info!("Keep set contains {} hashes", keep_set.len());

            let download_directory =
                DownloadDirectory::new(directory).expect("Failed to open directory");

            // `file_paths()` already drops `.meta` sidecars and subdirectories;
            // also skip the sync cache file so we never consider it for pruning.
            let files: Vec<PathBuf> = download_directory
                .file_paths()
                .into_iter()
                .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some(CACHE_FILENAME))
                .collect();
            log::info!(
                "Found {} candidate files in {}",
                files.len(),
                directory.display()
            );

            let parallelism = (*parallel).max(1);
            let use_cache = !no_cache;

            let hashing::HashResults {
                mut hashed,
                mut failed,
            } = hash_files_with_cache(directory, files, use_cache, parallelism).await;

            // Deterministic order for stable log output.
            hashed.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));

            if *dry_run {
                log::info!("DRY RUN — no files will be deleted (pass --dry-run false to delete)");
            }

            let mut deleted = 0usize;
            let mut would_delete = 0usize;
            let mut kept = 0usize;
            let mut not_archived = 0usize;

            for (file, hash) in &hashed {
                let filename = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("<unknown>");

                // Reachable from a --keep argument: always retained, and we can
                // skip the server round-trip entirely.
                if keep_set.contains(hash) {
                    log::debug!("Keeping {} (reachable from --keep)", filename);
                    kept += 1;
                    continue;
                }

                // Not kept — only safe to delete if the server already has it
                // archived. Otherwise leave it untouched.
                let upload_type = upload_type_for(file);
                match server_has_hash(&client, server, upload_type, hash).await {
                    Ok(true) => {}
                    Ok(false) => {
                        log::info!("Keeping {} (not archived on server)", filename);
                        not_archived += 1;
                        continue;
                    }
                    Err(e) => {
                        log::error!(
                            "Archive check failed for {} — leaving in place: {}",
                            filename,
                            e
                        );
                        failed += 1;
                        continue;
                    }
                }

                // Server has it and it is not kept: prune the file and its
                // `.meta` sidecar (if present).
                let meta = meta_path_for(file);
                let has_meta = meta.exists();
                if *dry_run {
                    if has_meta {
                        log::info!("WOULD DELETE {} (and its .meta)", filename);
                    } else {
                        log::info!("WOULD DELETE {}", filename);
                    }
                    would_delete += 1;
                    continue;
                }

                match std::fs::remove_file(file) {
                    Ok(()) => {
                        log::info!("DELETED {}", filename);
                        deleted += 1;
                    }
                    Err(e) => {
                        log::error!("Failed to delete {}: {}", filename, e);
                        failed += 1;
                        continue;
                    }
                }
                if has_meta {
                    match std::fs::remove_file(&meta) {
                        Ok(()) => log::info!("DELETED {}.meta", filename),
                        Err(e) => log::warn!("Failed to delete meta for {}: {}", filename, e),
                    }
                }
            }

            if *dry_run {
                log::info!(
                    "Prune dry run complete: {} would delete, {} kept, {} not archived, {} errors",
                    would_delete,
                    kept,
                    not_archived,
                    failed
                );
            } else {
                log::info!(
                    "Prune complete: {} deleted, {} kept, {} not archived, {} errors",
                    deleted,
                    kept,
                    not_archived,
                    failed
                );
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_path_appends_meta_to_full_filename() {
        // The sidecar keeps the archive's full name, including its extension:
        // foo.7z -> foo.7z.meta (not foo.meta).
        assert_eq!(
            meta_path_for(Path::new("/downloads/foo.7z")),
            PathBuf::from("/downloads/foo.7z.meta")
        );
        assert_eq!(
            meta_path_for(Path::new("/downloads/no-ext")),
            PathBuf::from("/downloads/no-ext.meta")
        );
        assert_eq!(
            meta_path_for(Path::new("/downloads/a.tar.gz")),
            PathBuf::from("/downloads/a.tar.gz.meta")
        );
    }
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
