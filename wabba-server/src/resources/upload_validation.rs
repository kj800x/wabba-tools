use actix_web::HttpRequest;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;

use crate::db::mod_data::Mod;
use crate::db::modlist::Modlist;

#[derive(Debug)]
pub enum UploadValidationResult {
    NotModified,
    AcceptUpload,
    RejectUserError(String),
}

pub trait ArchiveType: Clone {
    fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error>;
    fn is_available(&self) -> bool;
}

impl ArchiveType for Mod {
    fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        Mod::get_by_hash(hash, conn)
    }

    fn is_available(&self) -> bool {
        self.is_available()
    }
}

impl ArchiveType for Modlist {
    fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        Modlist::get_by_hash(hash, conn)
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

pub fn validate_upload_request<A: ArchiveType>(
    req: &HttpRequest,
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

    // Check if hash already exists in DB
    if let Some(stored_by_hash) = A::get_by_hash(if_none_match, conn)? {
        // Hash exists in database - check availability
        if stored_by_hash.is_available() {
            // Hash exists and is available - not modified
            return Ok(UploadValidationResult::NotModified);
        } else {
            // Hash exists but is unavailable - accept upload to make it available
            return Ok(UploadValidationResult::AcceptUpload);
        }
    }

    // Hash doesn't exist in DB - accept upload
    Ok(UploadValidationResult::AcceptUpload)
}
