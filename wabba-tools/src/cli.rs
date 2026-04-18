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
    },
}
