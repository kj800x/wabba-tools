use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::db::modlist::Modlist;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mod {
    pub id: u64,
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub xxhash64: String,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModEgg {
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub xxhash64: String,
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
            available: row.get(6)?,
        })
    }

    pub fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM \"mod\" WHERE filename = ?1")?
        .query_row(params![filename], |row| {
          Ok(Mod::from_row(row))
        })
        .optional()?
        .transpose()?;

        Ok(archive)
    }

    pub fn get_by_hash(
        hash: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM \"mod\" WHERE xxhash64 = ?1")?
      .query_row(params![hash], |row| {
        Ok(Mod::from_row(row))
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
        let mut all_mods = Vec::new();
        for hash in hashes {
            if let Some(mod_item) = Self::get_by_hash(hash, conn)? {
                all_mods.push(mod_item);
            }
        }

        Ok(all_mods)
    }

    pub fn get_by_modlist_id(
        modlist_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT \"mod\".id, \"mod\".filename, \"mod\".name, \"mod\".version, \"mod\".size, \"mod\".xxhash64, \"mod\".available
             FROM \"mod\"
             INNER JOIN mod_association ON \"mod\".id = mod_association.mod_id
             WHERE mod_association.modlist_id = ?1
             ORDER BY \"mod\".filename"
        )?;
        let mods = stmt
            .query_map(params![modlist_id], |row| {
                Ok(Mod::from_row(row)?)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR REPLACE INTO \"mod\" (id, filename, name, version, size, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
        .execute(params![self.id, self.filename, self.name, self.version, self.size, self.xxhash64, self.available])?;

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
}

impl ModEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Mod, rusqlite::Error> {
        conn.prepare("INSERT INTO \"mod\" (filename, name, version, size, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?
          .execute(params![self.filename, self.name, self.version, self.size, self.xxhash64, self.available])?;

        Ok(Mod {
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

