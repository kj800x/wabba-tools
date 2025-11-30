use std::path::{Path, PathBuf};

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use wabba_protocol::wabbajack::WabbajackMetadata;

use crate::db::{
    mod_association::{ModAssociation, ModAssociationEgg},
    mod_data::{Mod, ModEgg},
    modlist::{Modlist, ModlistEgg},
};

pub fn ingest_mod(
    filename: &str,
    hash: &str,
    path: &Path,
    conn: &PooledConnection<SqliteConnectionManager>,
) -> Result<(), actix_web::Error> {
    let size = std::fs::metadata(&path).unwrap().len() as u64;

    // Check if file was in DB but unavailable - if so, mark as available; otherwise create new
    match Mod::get_by_size_and_hash(size, hash, &conn)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Database error: {}", e)))?
    {
        Some(stored_mod) => {
            log::info!("Mod present in db, setting disk filename");
            stored_mod.set_disk_filename(filename, &conn).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?;
        }

        None => {
            log::info!("Mod not found in db, creating new one");
            let mod_egg = ModEgg {
                disk_filename: Some(filename.to_string()),
                xxhash64: hash.to_string(),
                size: size,
            };

            mod_egg.create(&conn).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?;
        }
    }

    Ok(())
}

pub fn ingest_modlist(
    filename: &str,
    hash: &str,
    path: &PathBuf,
    conn: &PooledConnection<SqliteConnectionManager>,
) -> Result<(), actix_web::Error> {
    let size = std::fs::metadata(path).unwrap().len() as u64;
    let metadata = WabbajackMetadata::load(path).expect("Failed to load Wabbajack metadata");

    // Check if modlist already exists - update if needed, otherwise create new
    let modlist = match Modlist::get_by_filename(&filename, &conn)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Database error: {}", e)))?
    {
        Some(existing) => {
            // Modlist exists - update it to ensure metadata is current
            log::info!("Updating existing modlist entry");
            let updated = Modlist {
                id: existing.id,
                filename: filename.to_string(),
                name: metadata.name.clone(),
                version: metadata.version.clone(),
                xxhash64: hash.to_string(),
                size: size,
                available: true,
                muted: existing.muted,
            };
            updated.update(&conn).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?;
            updated
        }
        None => {
            // Create new entry
            log::info!("Creating new modlist entry");
            let modlist_egg = ModlistEgg {
                filename: filename.to_string(),
                name: metadata.name.clone(),
                version: metadata.version.clone(),
                xxhash64: hash.to_string(),
                size: size,
                available: true,
            };

            modlist_egg.create(&conn).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?
        }
    };

    log::info!("modlist: {:#?}", modlist);

    // Associate required mods
    for archive in metadata.required_archives() {
        // Find or create the Mod entry (unique file identified by size + hash)
        let mod_to_associate = match Mod::get_by_size_and_hash(archive.size, &archive.hash, &conn)
            .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        })? {
            Some(existing_mod) => existing_mod,
            None => {
                // Create new mod entry
                let mod_egg = ModEgg {
                    disk_filename: None,
                    xxhash64: archive.hash.clone(),
                    size: archive.size,
                };

                let created_mod = mod_egg.create(&conn).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
                })?;

                log::info!("Created new mod: {:#?}", created_mod);
                created_mod
            }
        };

        // Create or update the ModAssociation with modlist-specific metadata
        // Check if association already exists
        match ModAssociation::get_by_modlist_and_mod(modlist.id, mod_to_associate.id, &conn)
            .map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })? {
            Some(mut existing_assoc) => {
                // Update existing association with latest metadata
                existing_assoc.source = archive.state.clone();
                existing_assoc.filename = archive.filename.clone();
                existing_assoc.name = archive.name();
                existing_assoc.version = archive.version();
                existing_assoc.update(&conn).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
                })?;
                log::info!("Updated mod association: {:#?}", existing_assoc);
            }
            None => {
                let association_egg = ModAssociationEgg {
                    modlist_id: modlist.id,
                    mod_id: mod_to_associate.id,
                    source: archive.state.clone(),
                    filename: archive.filename.clone(),
                    name: archive.name(),
                    version: archive.version(),
                };

                // Create new association
                association_egg.create(&conn).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
                })?;
                log::info!("Created new mod association");
            }
        }
    }

    Ok(())
}
