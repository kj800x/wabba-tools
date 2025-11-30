use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::modlist::Modlist;

#[derive(Debug)]
pub enum ToggleLostForeverError {
    ModHasDiskFilename,
    DatabaseError(rusqlite::Error),
}

impl std::fmt::Display for ToggleLostForeverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToggleLostForeverError::ModHasDiskFilename => {
                write!(
                    f,
                    "Cannot mark mod as lost forever when disk_filename is set"
                )
            }
            ToggleLostForeverError::DatabaseError(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl std::error::Error for ToggleLostForeverError {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mod {
    pub id: u64,
    pub disk_filename: Option<String>,
    pub size: u64,
    pub xxhash64: String,
    pub lost_forever: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEgg {
    pub disk_filename: Option<String>,
    pub size: u64,
    pub xxhash64: String,
}

impl Mod {
    pub fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Mod {
            id: row.get(0)?,
            disk_filename: row.get(1)?,
            size: row.get(2)?,
            xxhash64: row.get(3)?,
            lost_forever: row.get(4)?,
        })
    }

    pub fn is_available(&self) -> bool {
        self.disk_filename.is_some()
    }

    #[allow(dead_code)]
    pub fn get_by_disk_filename(
        disk_filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn
            .prepare(
                "SELECT id, disk_filename, size, xxhash64, lost_forever FROM \"mod\" WHERE disk_filename = ?1",
            )?
            .query_row(params![disk_filename], |row| Ok(Mod::from_row(row)))
            .optional()?
            .transpose()?;

        Ok(archive)
    }

    pub fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn
            .prepare("SELECT id, disk_filename, size, xxhash64, lost_forever FROM \"mod\" WHERE xxhash64 = ?1")?
            .query_row(params![hash], |row| Ok(Mod::from_row(row)))
            .optional()?
            .transpose()?;

        Ok(archive)
    }

    pub fn get_by_size_and_hash(
        size: u64,
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, disk_filename, size, xxhash64, lost_forever FROM \"mod\" WHERE size = ?1 AND xxhash64 = ?2")?
        .query_row(params![size, hash], |row| {
            Ok(Mod::from_row(row))
        })
        .optional()?
        .transpose()?;

        Ok(archive)
    }

    pub fn get_by_id(
        id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn
            .prepare(
                "SELECT id, disk_filename, size, xxhash64, lost_forever FROM \"mod\" WHERE id = ?1",
            )?
            .query_row(params![id], |row| Ok(Mod::from_row(row)))
            .optional()?
            .transpose()?;

        Ok(archive)
    }

    pub fn get_all(
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, disk_filename, size, xxhash64, lost_forever FROM \"mod\" ORDER BY disk_filename",
        )?;
        let mods = stmt
            .query_map([], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    pub fn get_unavailable(
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, disk_filename, size, xxhash64, lost_forever FROM \"mod\" WHERE disk_filename IS NULL",
        )?;
        let mods = stmt
            .query_map([], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    pub fn get_by_modlist_id(
        modlist_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT \"mod\".id, \"mod\".disk_filename, \"mod\".size, \"mod\".xxhash64, \"mod\".lost_forever
             FROM \"mod\"
             INNER JOIN mod_association ON \"mod\".id = mod_association.mod_id
             WHERE mod_association.modlist_id = ?1
             ORDER BY \"mod\".disk_filename",
        )?;
        let mods = stmt
            .query_map(params![modlist_id], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    #[allow(dead_code)]
    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR REPLACE INTO \"mod\" (id, disk_filename, size, xxhash64, lost_forever) VALUES (?1, ?2, ?3, ?4, ?5)")?
        .execute(params![self.id, self.disk_filename, self.size, self.xxhash64, self.lost_forever])?;

        Ok(())
    }

    pub fn set_disk_filename(
        &self,
        disk_filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("UPDATE \"mod\" SET disk_filename = ?1, lost_forever = FALSE WHERE id = ?2")?
            .execute(params![disk_filename, self.id])?;

        Ok(())
    }

    pub fn toggle_lost_forever(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), ToggleLostForeverError> {
        // Can't mark as lost_forever if disk_filename is set
        if self.disk_filename.is_some() {
            return Err(ToggleLostForeverError::ModHasDiskFilename);
        }

        let new_value = !self.lost_forever;
        conn.prepare("UPDATE \"mod\" SET lost_forever = ?1 WHERE id = ?2")
            .map_err(|e| ToggleLostForeverError::DatabaseError(e))?
            .execute(params![new_value, self.id])
            .map_err(|e| ToggleLostForeverError::DatabaseError(e))?;

        Ok(())
    }

    pub fn get_associated_modlists(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Modlist>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT modlist.id, modlist.filename, modlist.name, modlist.version, modlist.size, modlist.xxhash64, modlist.available, modlist.muted
             FROM modlist
             INNER JOIN mod_association ON modlist.id = mod_association.modlist_id
             WHERE mod_association.mod_id = ?1
             ORDER BY modlist.name"
        )?;
        let modlists = stmt
            .query_map(params![self.id], |row| Ok(Modlist::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(modlists)
    }

    pub fn count_modlists(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<u64, rusqlite::Error> {
        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM mod_association WHERE mod_id = ?1")?
            .query_row(params![self.id], |row| row.get(0))?;

        Ok(count as u64)
    }

    pub fn get_by_disk_filename_all(
        disk_filename: &str,
        exclude_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, disk_filename, size, xxhash64, lost_forever
             FROM \"mod\"
             WHERE disk_filename = ?1 AND id != ?2
             ORDER BY id",
        )?;
        let mods = stmt
            .query_map(params![disk_filename, exclude_id], |row| {
                Ok(Mod::from_row(row)?)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }
}

impl ModEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Mod, rusqlite::Error> {
        conn.prepare("INSERT INTO \"mod\" (disk_filename, size, xxhash64) VALUES (?1, ?2, ?3)")?
            .execute(params![self.disk_filename, self.size, self.xxhash64])?;

        Ok(Mod {
            id: conn.last_insert_rowid() as u64,
            disk_filename: self.disk_filename.clone(),
            size: self.size,
            xxhash64: self.xxhash64.clone(),
            lost_forever: false,
        })
    }
}
