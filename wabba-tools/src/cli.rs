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
}
