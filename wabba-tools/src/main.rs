use crate::download_dir::DownloadDirectory;
use clap::Parser;
mod cli;
mod download_dir;
use env_logger::Builder;
use reqwest::Client;
use reqwest::header::IF_NONE_MATCH;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use wabba_protocol::{hash::Hash, wabbajack::WabbajackMetadata};

#[derive(Debug)]
struct FileComparisonResult {
    missing_files: Vec<String>,
    satisfied_files: Vec<String>,
    extraneous_files: Vec<String>,
}

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
            let upload_type = UploadType::from_extension(
                file.extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default(),
            );

            log::info!("Computing hash for {}", file.display());
            let hash = Hash::compute(&std::fs::read(file).expect("Failed to read file"));

            let url = format!(
                "{}/submit/{}/{}",
                server,
                upload_type.as_str(),
                file.file_name().unwrap().to_str().unwrap_or_default()
            );

            // Open the file asynchronously
            let file = File::open(file).await.unwrap();
            let stream = FramedRead::new(file, BytesCodec::new());
            let body = reqwest::Body::wrap_stream(stream);

            log::info!("POST {}", url);
            let client = Client::new();
            let response = client
                .post(&url)
                .header(IF_NONE_MATCH, hash.to_string())
                .body(body)
                .send()
                .await
                .unwrap();
            // .error_for_status()
            // .unwrap();

            let response_code = response.status().as_u16();
            match response_code {
                200 => log::info!("Upload successful"),
                304 => log::info!("File already exists"),
                _ => {
                    log::error!("Upload failed: {}", response_code);
                    log::error!("Response: {:#?}", response);
                    log::error!("Response body: {}", response.text().await.unwrap());
                }
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
