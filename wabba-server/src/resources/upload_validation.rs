use actix_web::HttpRequest;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

use crate::db::mod_archive::ModArchive;
use crate::db::wabbajack_archive::WabbajackArchive;

#[derive(Debug)]
pub enum UploadValidationResult {
    NotModified,
    AcceptUpload,
    RejectCorruptedState(String),
    RejectNeedsBootstrap(String),
    RejectUserError(String),
}

pub trait ArchiveType: Clone {
    fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error>;
    fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error>;
    fn filename(&self) -> &str;
    fn hash(&self) -> &str;
    fn available(&self) -> bool;
}

impl ArchiveType for ModArchive {
    fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        ModArchive::get_by_hash(hash, conn)
    }

    fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        ModArchive::get_by_filename(filename, conn)
    }

    fn filename(&self) -> &str {
        &self.filename
    }

    fn hash(&self) -> &str {
        &self.xxhash64
    }

    fn available(&self) -> bool {
        self.available
    }
}

impl ArchiveType for WabbajackArchive {
    fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        WabbajackArchive::get_by_hash(hash, conn)
    }

    fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        WabbajackArchive::get_by_filename(filename, conn)
    }

    fn filename(&self) -> &str {
        &self.filename
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

    // Check if filename exists in DB - if it does, its hash must match If-None-Match
    if let Some(stored_by_filename) = A::get_by_filename(filename, conn)? {
        if stored_by_filename.hash() != if_none_match {
            return Ok(UploadValidationResult::RejectUserError(format!(
                "Filename already exists in database with different hash: user provided {}, but database has {}",
                if_none_match,
                stored_by_filename.hash()
            )));
        }
        // Hash matches - continue with normal validation logic below
    }

    // Check if file exists in DB by hash (only if available)
    if let Some(stored_archive) = A::get_by_hash(if_none_match, conn)? {
        if stored_archive.available() {
            // Hash matches, file is in DB, and available
            if stored_archive.filename() == filename {
                return Ok(UploadValidationResult::NotModified);
            } else {
                return Ok(UploadValidationResult::RejectCorruptedState(
                    "Content hash already stored in db under a different filename".to_string(),
                ));
            }
        } else {
            // File is in DB but not available - check if it's on disk
            if file_path.exists() {
                return Ok(UploadValidationResult::RejectNeedsBootstrap(
                    "File exists in db as unavailable and also exists on disk".to_string(),
                ));
            }

            if stored_archive.filename() != filename {
                return Ok(UploadValidationResult::RejectCorruptedState(
                    "Content hash already stored in db under a different filename".to_string(),
                ));
            }

            // File is in DB but unavailable and not on disk - accept upload
            return Ok(UploadValidationResult::AcceptUpload);
        }
    }

    // File is not in DB - check if it exists on disk
    if file_path.exists() {
        return Ok(UploadValidationResult::RejectNeedsBootstrap(
            "File already exists on disk (but not db)".to_string(),
        ));
    }

    // File is not in DB and not on disk - accept upload
    Ok(UploadValidationResult::AcceptUpload)
}
