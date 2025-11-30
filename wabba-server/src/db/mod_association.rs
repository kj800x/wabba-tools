use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use wabba_protocol::archive_state::ArchiveState;

use crate::db::mod_data::Mod;
use crate::db::modlist::Modlist;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModAssociation {
    pub modlist_id: u64,
    pub mod_id: u64,
    pub source: ArchiveState,
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModAssociationEgg {
    pub modlist_id: u64,
    pub mod_id: u64,
    pub source: ArchiveState,
    pub filename: String,
    pub name: Option<String>,
    pub version: Option<String>,
}

impl ModAssociation {
    pub fn from_row(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        let source_str: String = row.get(2)?;
        let source: ArchiveState = serde_json::from_str(&source_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse ArchiveState: {}", e),
                )),
            )
        })?;

        Ok(ModAssociation {
            modlist_id: row.get(0)?,
            mod_id: row.get(1)?,
            source,
            filename: row.get(3)?,
            name: row.get::<_, Option<String>>(4)?,
            version: row.get::<_, Option<String>>(5)?,
        })
    }

    pub fn get_by_modlist_and_mod(
        modlist_id: u64,
        mod_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Self>, rusqlite::Error> {
        let association = conn
            .prepare(
                "SELECT modlist_id, mod_id, source, filename, name, version
                 FROM mod_association
                 WHERE modlist_id = ?1 AND mod_id = ?2",
            )?
            .query_row(params![modlist_id, mod_id], |row| {
                Ok(ModAssociation::from_row(row))
            })
            .optional()?
            .transpose()?;

        Ok(association)
    }

    pub fn get_by_modlist_id(
        modlist_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT modlist_id, mod_id, source, filename, name, version
             FROM mod_association
             WHERE modlist_id = ?1
             ORDER BY filename",
        )?;
        let associations = stmt
            .query_map(params![modlist_id], |row| {
                Ok(ModAssociation::from_row(row)?)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(associations)
    }

    pub fn get_by_mod_id(
        mod_id: u64,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT modlist_id, mod_id, source, filename, name, version
             FROM mod_association
             WHERE mod_id = ?1
             ORDER BY modlist_id",
        )?;
        let associations = stmt
            .query_map(params![mod_id], |row| Ok(ModAssociation::from_row(row)?))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(associations)
    }

    pub fn get_mod_with_association(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Mod>, rusqlite::Error> {
        Mod::get_by_id(self.mod_id, conn)
    }

    pub fn get_modlist_with_association(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<Option<Modlist>, rusqlite::Error> {
        Modlist::get_by_id(self.modlist_id, conn)
    }

    pub fn update(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare(
            "INSERT OR REPLACE INTO mod_association (modlist_id, mod_id, source, filename, name, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )?
        .execute(params![
            self.modlist_id,
            self.mod_id,
            serde_json::to_string(&self.source).unwrap(),
            self.filename,
            self.name,
            self.version
        ])?;

        Ok(())
    }

    pub fn delete(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<(), rusqlite::Error> {
        conn.prepare("DELETE FROM mod_association WHERE modlist_id = ?1 AND mod_id = ?2")?
            .execute(params![self.modlist_id, self.mod_id])?;

        Ok(())
    }
}

impl ModAssociationEgg {
    pub fn create(
        &self,
        conn: &PooledConnection<SqliteConnectionManager>,
    ) -> Result<ModAssociation, rusqlite::Error> {
        conn.prepare(
            "INSERT INTO mod_association (modlist_id, mod_id, source, filename, name, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?
        .execute(params![
            self.modlist_id,
            self.mod_id,
            serde_json::to_string(&self.source).unwrap(),
            self.filename,
            self.name,
            self.version
        ])?;

        Ok(ModAssociation {
            modlist_id: self.modlist_id,
            mod_id: self.mod_id,
            source: self.source.clone(),
            filename: self.filename.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        })
    }
}
