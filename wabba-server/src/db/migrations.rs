use crate::prelude::*;
use indoc::indoc;

pub fn migrate(mut conn: PooledConnection<SqliteConnectionManager>) -> Result<()> {
    let migrations: Migrations = Migrations::new(vec![
        M::up(indoc! { r#"
          CREATE TABLE wabbajack_archive (
              id INTEGER PRIMARY KEY NOT NULL,
              filename TEXT NOT NULL,
              name TEXT NOT NULL,
              version TEXT NOT NULL,
              xxhash64 TEXT NOT NULL,
              available BOOLEAN NOT NULL DEFAULT FALSE
          );

          CREATE TABLE mod_archive (
              id INTEGER PRIMARY KEY NOT NULL,
              filename TEXT NOT NULL,
              name TEXT,
              version TEXT,
              xxhash64 TEXT NOT NULL,
              available BOOLEAN NOT NULL DEFAULT FALSE
          );

          CREATE TABLE mod_association (
              archive_id INTEGER NOT NULL,
              mod_id INTEGER NOT NULL,
              PRIMARY KEY(archive_id, mod_id),
              FOREIGN KEY(archive_id) REFERENCES wabbajack_archive(id),
              FOREIGN KEY(mod_id) REFERENCES mod_archive(id)
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
