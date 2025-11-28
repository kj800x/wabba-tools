use actix_web::HttpRequest;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

use crate::db::mod_data::Mod;
use crate::db::modlist::Modlist;

#[derive(Debug)]
pub enum UploadValidationResult {
    NotModified,
    AcceptUpload,
    RejectNeedsBootstrap(String),
    RejectUserError(String),
}

pub trait ArchiveType: Clone {
    fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error>;
    fn hash(&self) -> &str;
    fn available(&self) -> bool;
}

impl ArchiveType for Mod {
    fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        Mod::get_by_filename(filename, conn)
    }

    fn hash(&self) -> &str {
        &self.xxhash64
    }

    fn available(&self) -> bool {
        self.available
    }
}

impl ArchiveType for Modlist {
    fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        Modlist::get_by_filename(filename, conn)
    }

    fn hash(&self) -> &str {
        &self.xxhash64
    }

    fn available(&self) -> bool {
        self.available
    }
}

pub fn validate_upload_request<A: ArchiveType>(
    req: &HttpRequest,
    filename: &str,
    file_path: &Path,
    conn: &PooledConnection<SqliteConnectionManager>,
) -> Result<UploadValidationResult, rusqlite::Error> {
    let headers = req.headers();

    // Require If-None-Match header
    let if_none_match = headers.get("If-None-Match").and_then(|x| x.to_str().ok());
    let if_none_match = match if_none_match {
        Some(hash) => hash,
        None => {
            return Ok(UploadValidationResult::RejectUserError(
                "If-None-Match header is required".to_string(),
            ));
        }
    };

    // Check if filename exists in DB - uniqueness is only by filename
    if let Some(stored_by_filename) = A::get_by_filename(filename, conn)? {
        // Filename exists in database - check hash and availability
        if stored_by_filename.hash() != if_none_match {
            // Filename exists but with different hash - this is an error
            return Ok(UploadValidationResult::RejectUserError(format!(
                "Filename already exists in database with different hash: user provided {}, but database has {}",
                if_none_match,
                stored_by_filename.hash()
            )));
        }

        // Hash matches - check availability
        if stored_by_filename.available() {
            // Filename exists with matching hash and is available - not modified
            return Ok(UploadValidationResult::NotModified);
        } else {
            // Filename exists with matching hash but is unavailable - accept upload
            return Ok(UploadValidationResult::AcceptUpload);
        }
    }

    // Filename doesn't exist in DB - check if file exists on disk
    if file_path.exists() {
        return Ok(UploadValidationResult::RejectNeedsBootstrap(
            "File already exists on disk (but not db)".to_string(),
        ));
    }

    // Filename is not in DB and file doesn't exist on disk - accept upload
    Ok(UploadValidationResult::AcceptUpload)
}
