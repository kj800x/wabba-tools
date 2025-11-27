use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::wabbajack_archive::WabbajackArchive;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModArchive {
    pub id: u64,
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub xxhash64: String,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModArchiveEgg {
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub xxhash64: String,
    pub available: bool,
}

impl ModArchive {
    pub fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(ModArchive {
            id: row.get(0)?,
            filename: row.get(1)?,
            name: row.get::<_, Option<String>>(2)?,
            version: row.get::<_, Option<String>>(3)?,
            size: row.get(4)?,
            xxhash64: row.get(5)?,
            available: row.get(6)?,
        })
    }

    pub fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM mod_archive WHERE filename = ?1")?
        .query_row(params![filename], |row| {
          Ok(ModArchive::from_row(row))
        })
        .optional()?
        .transpose()?;

        Ok(archive)
    }

    pub fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM mod_archive WHERE xxhash64 = ?1")?
      .query_row(params![hash], |row| {
        Ok(ModArchive::from_row(row))
      })
      .optional()?

          .transpose()?;

        Ok(archive)
    }

    pub fn get_by_hashes(
        hashes: &[String],
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }

        // For dynamic IN clauses, we'll query each hash individually and collect results
        // This is less efficient but more reliable than trying to use dynamic params
        let mut all_archives = Vec::new();
        for hash in hashes {
            if let Some(archive) = Self::get_by_hash(hash, conn)? {
                all_archives.push(archive);
            }
        }

        Ok(all_archives)
    }

    pub fn get_by_wabbajack_archive_id(
        wabbajack_archive_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT mod_archive.id, mod_archive.filename, mod_archive.name, mod_archive.version, mod_archive.size, mod_archive.xxhash64, mod_archive.available
             FROM mod_archive
             INNER JOIN mod_association ON mod_archive.id = mod_association.mod_id
             WHERE mod_association.archive_id = ?1
             ORDER BY mod_archive.filename"
        )?;
        let archives = stmt
            .query_map(params![wabbajack_archive_id], |row| {
                Ok(ModArchive::from_row(row)?)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(archives)
    }

    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR REPLACE INTO mod_archive (id, filename, name, version, size, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
        .execute(params![self.id, self.filename, self.name, self.version, self.size, self.xxhash64, self.available])?;

        Ok(())
    }

    pub fn associate(
        &self,
        wabbajack_archive: &WabbajackArchive,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR IGNORE INTO mod_association (archive_id, mod_id) VALUES (?1, ?2)")?
            .execute(params![wabbajack_archive.id, self.id])?;

        Ok(())
    }
}

impl ModArchiveEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<ModArchive, rusqlite::Error> {
        conn.prepare("INSERT INTO mod_archive (filename, name, version, size, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?
          .execute(params![self.filename, self.name, self.version, self.size, self.xxhash64, self.available])?;

        Ok(ModArchive {
            id: conn.last_insert_rowid() as u64,
            filename: self.filename.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            size: self.size,
            xxhash64: self.xxhash64.clone(),
            available: self.available,
        })
    }
}
