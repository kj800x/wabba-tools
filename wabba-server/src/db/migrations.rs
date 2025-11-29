use crate::prelude::*;
use indoc::indoc;

pub fn migrate(mut conn: PooledConnection<SqliteConnectionManager>) -> Result<()> {
    let migrations: Migrations = Migrations::new(vec![
        M::up(indoc! { r#"
          CREATE TABLE modlist (
              id INTEGER PRIMARY KEY NOT NULL,
              filename TEXT NOT NULL,
              name TEXT NOT NULL,
              version TEXT NOT NULL,
              size INTEGER NOT NULL,
              xxhash64 TEXT NOT NULL,
              available BOOLEAN NOT NULL DEFAULT FALSE,
              created_at TIMESTAMP NOT NULL DEFAULT (unixepoch())
          );
          CREATE INDEX modlist_filename_idx ON modlist(filename);
          CREATE INDEX modlist_xxhash64_idx ON modlist(xxhash64);

          CREATE TABLE "mod" (
              id INTEGER PRIMARY KEY NOT NULL,
              filename TEXT NOT NULL,
              name TEXT,
              version TEXT,
              source TEXT,
              size INTEGER NOT NULL,
              xxhash64 TEXT NOT NULL,
              available BOOLEAN NOT NULL DEFAULT FALSE,
              created_at TIMESTAMP NOT NULL DEFAULT (unixepoch())
          );
          CREATE INDEX mod_filename_idx ON "mod"(filename);
          CREATE INDEX mod_xxhash64_idx ON "mod"(xxhash64);

          CREATE TABLE mod_association (
              modlist_id INTEGER NOT NULL,
              mod_id INTEGER NOT NULL,
              PRIMARY KEY(modlist_id, mod_id),
              FOREIGN KEY(modlist_id) REFERENCES modlist(id),
              FOREIGN KEY(mod_id) REFERENCES "mod"(id),
              UNIQUE(modlist_id, mod_id)
          );
      "#}),
        // M::up( indoc! { r#"
        //     SQL GOES HERE
        // "#}),
    ]);

    conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))
        .unwrap();

    migrations
        .to_latest(&mut conn)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    Ok(())
}
