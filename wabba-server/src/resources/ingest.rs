use std::path::{Path, PathBuf};

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use wabba_protocol::wabbajack::WabbajackMetadata;

use crate::db::{
    mod_data::{Mod, ModEgg},
    modlist::{Modlist, ModlistEgg},
};

pub fn ingest_mod(
    filename: &str,
    hash: &str,
    path: &Path,
    conn: &PooledConnection<SqliteConnectionManager>,
) -> Result<(), actix_web::Error> {
    // Check if file was in DB but unavailable - if so, mark as available; otherwise create new
    match Mod::get_by_filename_and_hash(&filename, &hash, &conn)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Database error: {}", e)))?
    {
        Some(stored_mod) => {
            log::info!("Mod present in db, marking as available");
            stored_mod.mark_available(&conn).map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?;
        }

        None => {
            log::info!("Mod not found in db, creating new one");
            let size = std::fs::metadata(&path).unwrap().len() as u64;

            let mod_egg = ModEgg {
                filename: filename.to_string(),
                name: None,
                version: None,
                xxhash64: hash.to_string(),
                size: size,
                source: None,
                available: true,
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
        let mod_to_associate = match Mod::get_by_filename_and_hash(
            &archive.filename,
            &archive.hash,
            &conn,
        )
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Database error: {}", e)))?
        {
            Some(existing_mod) => {
                // // Verify filename, size, and hash match
                // if existing_mod.filename != archive.filename {
                //     return Err(actix_web::error::ErrorInternalServerError(format!(
                //         "Hash collision detected: filename {} exists with hash {} but metadata specifies filename {}",
                //         existing_mod.filename, existing_mod.xxhash64, archive.filename
                //     )));
                // }
                if existing_mod.size != archive.size {
                    return Err(actix_web::error::ErrorInternalServerError(format!(
                        "Size mismatch for filename {}: database has {} but metadata specifies {}",
                        archive.filename, existing_mod.size, archive.size
                    )));
                }
                // if existing_mod.xxhash64 != archive.hash {
                //     return Err(actix_web::error::ErrorInternalServerError(format!(
                //         "Hash mismatch for filename {}: database has {} but metadata specifies {}",
                //         existing_mod.filename, existing_mod.xxhash64, archive.hash
                //     )));
                // }

                // Enrich name and version from metadata, keep existing available status
                let updated_mod = Mod {
                    id: existing_mod.id,
                    filename: existing_mod.filename.clone(),
                    name: existing_mod.name.or(archive.name()),
                    version: existing_mod.version.or(archive.version()),
                    xxhash64: existing_mod.xxhash64.clone(),
                    size: existing_mod.size,
                    source: existing_mod.source.or(Some(archive.state.clone())),
                    available: existing_mod.available,
                };

                updated_mod.update(&conn).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
                })?;

                log::info!("Reusing existing mod: {:#?}", updated_mod);
                updated_mod
            }
            None => {
                // Create new mod entry
                let mod_egg = ModEgg {
                    filename: archive.filename.clone(),
                    name: archive.name(),
                    version: archive.version(),
                    xxhash64: archive.hash.clone(),
                    size: archive.size,
                    source: Some(archive.state.clone()),
                    available: false,
                };

                let created_mod = mod_egg.create(&conn).map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
                })?;

                log::info!("Created new mod: {:#?}", created_mod);
                created_mod
            }
        };

        mod_to_associate.associate(&modlist, &conn).map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        })?;
    }

    Ok(())
}
