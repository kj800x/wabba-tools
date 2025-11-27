use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModArchive {
    pub id: u64,
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub xxhash64: String,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModArchiveEgg {
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub xxhash64: String,
    pub available: bool,
}

impl ModArchive {
    pub fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(ModArchive {
            id: row.get(0)?,
            filename: row.get(1)?,
            name: row.get(2).optional()?,
            version: row.get(3).optional()?,
            xxhash64: row.get(4)?,
            available: row.get(5)?,
        })
    }

    pub fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, xxhash64, available FROM mod_archive WHERE filename = ?1")?
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
        let archive = conn.prepare("SELECT id, filename, name, version, xxhash64, available FROM mod_archive WHERE xxhash64 = ?1")?
      .query_row(params![hash], |row| {
        Ok(ModArchive::from_row(row))
      })
      .optional()?

          .transpose()?;

        Ok(archive)
    }

    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR REPLACE INTO mod_archive (id, filename, name, version, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?
        .execute(params![self.id, self.filename, self.name, self.version, self.xxhash64, self.available])?;

        Ok(())
    }
}

impl ModArchiveEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<ModArchive, rusqlite::Error> {
        conn.prepare("INSERT INTO mod_archive (filename, name, version, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5)")?
          .execute(params![self.filename, self.name, self.version, self.xxhash64, self.available])?;

        Ok(ModArchive {
            id: conn.last_insert_rowid() as u64,
            filename: self.filename.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            xxhash64: self.xxhash64.clone(),
            available: self.available,
        })
    }
}
