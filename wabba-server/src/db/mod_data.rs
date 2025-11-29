use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use wabba_protocol::archive_state::ArchiveState;

use crate::db::modlist::Modlist;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mod {
    pub id: u64,
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub xxhash64: String,
    pub source: Option<ArchiveState>,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEgg {
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub xxhash64: String,
    pub source: Option<ArchiveState>,
    pub available: bool,
}

impl Mod {
    pub fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Mod {
            id: row.get(0)?,
            filename: row.get(1)?,
            name: row.get::<_, Option<String>>(2)?,
            version: row.get::<_, Option<String>>(3)?,
            size: row.get(4)?,
            xxhash64: row.get(5)?,
            source: row
                .get::<_, Option<String>>(6)?
                .and_then(|x| serde_json::from_str(&x).ok()),
            available: row.get(7)?,
        })
    }

    pub fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, source, available FROM \"mod\" WHERE filename = ?1")?
        .query_row(params![filename], |row| {
          Ok(Mod::from_row(row))
        })
        .optional()?
        .transpose()?;

        Ok(archive)
    }

    pub fn get_by_filename_and_hash(
        filename: &str,
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, source, available FROM \"mod\" WHERE filename = ?1 AND xxhash64 = ?2")?
        .query_row(params![filename, hash], |row| {
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
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, source, available FROM \"mod\" WHERE id = ?1")?
            .query_row(params![id], |row| {
                Ok(Mod::from_row(row))
            })
            .optional()?
            .transpose()?;

        Ok(archive)
    }

    pub fn get_all(
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT id, filename, name, version, size, xxhash64, source, available FROM \"mod\" ORDER BY filename")?;
        let mods = stmt
            .query_map([], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    pub fn get_unavailable(
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT id, filename, name, version, size, xxhash64, source, available FROM \"mod\" WHERE available = FALSE ORDER BY filename")?;
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
            "SELECT \"mod\".id, \"mod\".filename, \"mod\".name, \"mod\".version, \"mod\".size, \"mod\".xxhash64, \"mod\".source, \"mod\".available
             FROM \"mod\"
             INNER JOIN mod_association ON \"mod\".id = mod_association.mod_id
             WHERE mod_association.modlist_id = ?1
             ORDER BY \"mod\".filename"
        )?;
        let mods = stmt
            .query_map(params![modlist_id], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR REPLACE INTO \"mod\" (id, filename, name, version, size, xxhash64, source, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?
        .execute(params![self.id, self.filename, self.name, self.version, self.size, self.xxhash64, self.source.clone().map(|x| serde_json::to_string(&x).unwrap()), self.available])?;

        Ok(())
    }

    pub fn mark_available(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("UPDATE \"mod\" SET available = TRUE WHERE id = ?1")?
            .execute(params![self.id])?;

        Ok(())
    }

    pub fn associate(
        &self,
        modlist: &Modlist,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR IGNORE INTO mod_association (modlist_id, mod_id) VALUES (?1, ?2)")?
            .execute(params![modlist.id, self.id])?;

        Ok(())
    }

    pub fn get_associated_modlists(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Modlist>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT modlist.id, modlist.filename, modlist.name, modlist.version, modlist.size, modlist.xxhash64, modlist.available
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

    pub fn get_by_filename_all(
        filename: &str,
        exclude_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, filename, name, version, size, xxhash64, source, available
             FROM \"mod\"
             WHERE filename = ?1 AND id != ?2
             ORDER BY id",
        )?;
        let mods = stmt
            .query_map(params![filename, exclude_id], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    pub fn get_by_name_all(
        name: &str,
        exclude_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT id, filename, name, version, size, xxhash64, source, available
             FROM \"mod\"
             WHERE name = ?1 AND id != ?2
             ORDER BY id",
        )?;
        let mods = stmt
            .query_map(params![name, exclude_id], |row| Ok(Mod::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }
}

impl ModEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Mod, rusqlite::Error> {
        conn.prepare("INSERT INTO \"mod\" (filename, name, version, size, xxhash64, source, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
          .execute(params![self.filename, self.name, self.version, self.size, self.xxhash64, self.source.clone().map(|x| serde_json::to_string(&x).unwrap()), self.available])?;

        Ok(Mod {
            id: conn.last_insert_rowid() as u64,
            filename: self.filename.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            size: self.size,
            xxhash64: self.xxhash64.clone(),
            source: self.source.clone(),
            available: self.available,
        })
    }
}
