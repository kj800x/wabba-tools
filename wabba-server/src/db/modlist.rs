use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Modlist {
    pub id: u64,
    pub filename: String,
    pub name: String,
    pub version: String,
    pub size: u64,
    pub xxhash64: String,
    pub available: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModlistEgg {
    pub filename: String,
    pub name: String,
    pub version: String,
    pub size: u64,
    pub xxhash64: String,
    pub available: bool,
}

impl Modlist {
    pub fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Modlist {
            id: row.get(0)?,
            filename: row.get(1)?,
            name: row.get(2)?,
            version: row.get(3)?,
            size: row.get(4)?,
            xxhash64: row.get(5)?,
            available: row.get(6)?,
        })
    }

    pub fn get_by_filename(
        filename: &str,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM modlist WHERE filename = ?1")?
        .query_row(params![filename], |row| {
          Ok(Modlist::from_row(row))
        })
        .optional()?
        .transpose()?;

        Ok(archive)
    }

    pub fn get_by_id(
        id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let archive = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM modlist WHERE id = ?1")?
            .query_row(params![id], |row| {
                Ok(Modlist::from_row(row))
            })
            .optional()?
            .transpose()?;

        Ok(archive)
    }

    pub fn get_all(
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT id, filename, name, version, size, xxhash64, available FROM modlist ORDER BY name, version DESC")?;
        let archives = stmt
            .query_map([], |row| Ok(Modlist::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(archives)
    }

    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("INSERT OR REPLACE INTO modlist (id, filename, name, version, size, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?
        .execute(params![self.id, self.filename, self.name, self.version, self.size, self.xxhash64, self.available])?;

        Ok(())
    }

    pub fn count_mods_total(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<u64, rusqlite::Error> {
        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM mod_association WHERE modlist_id = ?1")?
            .query_row(params![self.id], |row| row.get(0))?;

        Ok(count as u64)
    }

    pub fn count_mods_available(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<u64, rusqlite::Error> {
        let count: i64 = conn
            .prepare(
                "SELECT COUNT(*) FROM mod_association
             INNER JOIN \"mod\" ON mod_association.mod_id = \"mod\".id
             WHERE mod_association.modlist_id = ?1 AND \"mod\".available = TRUE",
            )?
            .query_row(params![self.id], |row| row.get(0))?;

        Ok(count as u64)
    }
}

impl ModlistEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Modlist, rusqlite::Error> {
        conn.prepare("INSERT INTO modlist (filename, name, version, size, xxhash64, available) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?
          .execute(params![self.filename, self.name, self.version, self.size, self.xxhash64, self.available])?;

        Ok(Modlist {
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
