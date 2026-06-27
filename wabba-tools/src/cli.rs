use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short='v', long="verbose", action = clap::ArgAction::Count)]
    pub debug: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Validates that the required files are available
    Validate {
        /// Path to the Wabbajack file
        #[arg(value_name = "WABBJACK_FILE")]
        wabbajack_file: PathBuf,

        /// Path to the download directory
        #[arg(value_name = "DOWNLOAD_DIRS")]
        download_dirs: Vec<PathBuf>,
    },

    /// Hash a file using xxhash64
    Hash {
        /// Path to the file to hash
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Upload a modlist file or mod file to the server
    Upload {
        /// Base URL of the server to upload to
        #[arg(value_name = "SERVER")]
        server: String,

        /// Path to the modlist file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Sync a local directory with the server, uploading any files the server
    /// does not already have. Only the top-level files of the directory are
    /// considered; subdirectories and `.meta` files are ignored. Files are
    /// never downloaded from the server.
    Sync {
        /// Base URL of the server to upload to
        #[arg(value_name = "SERVER")]
        server: String,

        /// Path to the directory to sync
        #[arg(value_name = "DIRECTORY")]
        directory: PathBuf,

        /// Skip the local hash cache and rehash every file.
        #[arg(long = "no-cache")]
        no_cache: bool,

        /// Number of files to hash in parallel. Defaults to 1 because the
        /// download directory is typically on a spinning HDD, where parallel
        /// reads thrash the disk head and slow throughput. Raise for SSD
        /// (~4–8) or NVMe (~8–16) sources.
        #[arg(long = "parallel", short = 'p', value_name = "N", default_value_t = 1)]
        parallel: usize,
    },

    /// Prune archived files from a local downloads directory that are not
    /// reachable from any `--keep` hash. A file is deleted only when the server
    /// already has it archived AND it is not kept; files the server has not
    /// archived are left untouched. `.meta` sidecar files are removed alongside
    /// the archive they belong to. Defaults to a dry run — you must pass
    /// `--dry-run false` to actually delete.
    Prune {
        /// Base URL of the server to consult
        #[arg(value_name = "SERVER")]
        server: String,

        /// Path to the local downloads directory to prune
        #[arg(value_name = "DIRECTORY")]
        directory: PathBuf,

        /// xxhash64 of a mod or modlist file to keep. When a modlist is named,
        /// every mod it requires is kept too. Repeatable; at least one required.
        #[arg(long = "keep", value_name = "XXHASH64", required = true)]
        keep: Vec<String>,

        /// When true (the default), report what would be deleted without
        /// touching anything. Pass `--dry-run false` to perform real deletes.
        #[arg(
            long = "dry-run",
            action = clap::ArgAction::Set,
            default_value_t = true,
            value_name = "BOOL"
        )]
        dry_run: bool,

        /// Skip the local hash cache and rehash every file.
        #[arg(long = "no-cache")]
        no_cache: bool,

        /// Number of files to hash in parallel (defaults to 1, HDD-friendly;
        /// raise for SSD/NVMe sources).
        #[arg(long = "parallel", short = 'p', value_name = "N", default_value_t = 1)]
        parallel: usize,
    },

    /// Fetch every mod required by a modlist from the server into a local
    /// downloads directory. Mods already present (matching filename and
    /// xxhash64) are skipped; only mods the server has archived can be
    /// fetched. Each download is verified against its expected hash.
    FetchMods {
        /// Base URL of the server to fetch from
        #[arg(value_name = "SERVER")]
        server: String,

        /// Path to the local downloads directory to populate
        #[arg(value_name = "DIRECTORY")]
        directory: PathBuf,

        /// xxhash64 of the modlist whose required mods should be fetched
        #[arg(value_name = "MODLIST_XXHASH64")]
        modlist: String,

        /// Skip the local hash cache and rehash every candidate file.
        #[arg(long = "no-cache")]
        no_cache: bool,

        /// Number of already-present files to hash in parallel (defaults to 1,
        /// HDD-friendly; raise for SSD/NVMe sources).
        #[arg(long = "parallel", short = 'p', value_name = "N", default_value_t = 1)]
        parallel: usize,
    },
}
