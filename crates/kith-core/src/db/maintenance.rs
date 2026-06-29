//! Database-file maintenance: compacted online backup, in-place vacuum, and
//! validated restore. These are the lifecycle operations the CLI `db`
//! subcommands expose; they live in the core because their correctness is
//! non-trivial — WAL snapshotting (`VACUUM INTO`, not a torn file copy),
//! read-only source validation, and clearing the target's stale write-ahead
//! log so a restored file is not silently shadowed.

use std::fs;
use std::io;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::db::Store;
use crate::error::{CoreError, Result};

impl Store {
    /// Writes a compacted, transactionally-consistent snapshot of the database
    /// to `dest` using SQLite's `VACUUM INTO`.
    ///
    /// Unlike a raw file copy, this captures a clean single file regardless of
    /// the live database's write-ahead-log state, and reclaims free pages in the
    /// process. `dest` must **not** already exist (mirroring `VACUUM INTO`); the
    /// caller decides any overwrite policy.
    ///
    /// # Errors
    /// - [`CoreError::Validation`] if `dest` already exists, or its path is not
    ///   valid UTF-8 (required to bind it as a SQL text parameter).
    /// - [`CoreError::Pool`] / [`CoreError::Database`] if a connection cannot be
    ///   acquired or the snapshot fails.
    pub fn backup<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        let dest = dest.as_ref();
        if dest.exists() {
            return Err(CoreError::Validation(format!(
                "backup destination already exists: {}",
                dest.display()
            )));
        }
        let dest_str = dest.to_str().ok_or_else(|| {
            CoreError::Validation(format!(
                "backup path is not valid UTF-8: {}",
                dest.display()
            ))
        })?;
        // `VACUUM INTO ?` binds the filename as a parameter (SQLite >= 3.27).
        self.conn()?.execute("VACUUM INTO ?1", [dest_str])?;
        Ok(())
    }

    /// Rebuilds the database file in place, reclaiming free pages (`VACUUM`).
    ///
    /// # Errors
    /// [`CoreError::Pool`] / [`CoreError::Database`] if a connection cannot be
    /// acquired or the rebuild fails.
    pub fn vacuum(&self) -> Result<()> {
        self.conn()?.execute_batch("VACUUM;")?;
        Ok(())
    }

    /// Replaces the database at `dest` with the contents of the backup at `src`.
    ///
    /// `src` is validated as a genuine, migrated Kith database **before** any
    /// byte at `dest` is touched (read-only, so `src` is never modified), so a
    /// wrong, corrupt, empty, or non-SQLite file can never clobber a real
    /// database. The copy then removes any stale `-wal` / `-shm` companions at
    /// `dest`, which would otherwise shadow the restored file on next open.
    ///
    /// This **unconditionally overwrites** `dest`; the policy of refusing to
    /// overwrite without an explicit opt-in lives in the caller (the CLI
    /// `--force` flag). `dest`'s parent directory must already exist, and no
    /// other process may hold `src` or `dest` open.
    ///
    /// # Errors
    /// - [`CoreError::Validation`] if `src` is missing or is not a valid,
    ///   migrated Kith database.
    /// - [`CoreError::Io`] if copying or sidecar cleanup fails.
    pub fn restore<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dest: Q) -> Result<()> {
        let src = src.as_ref();
        let dest = dest.as_ref();
        validate_kith_db(src)?; // never touches `dest` on failure
        fs::copy(src, dest).map_err(CoreError::Io)?;
        remove_wal_sidecars(dest)?; // stale WAL of the old `dest` must not survive
        Ok(())
    }
}

/// Confirms `path` is an existing, readable, migrated Kith database **without
/// modifying it**: opens read-only (so migrations cannot run), then checks the
/// schema version and a sentinel table. Any failure maps to a single typed
/// [`CoreError::Validation`] — the source is simply "not a valid Kith database".
fn validate_kith_db(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(CoreError::Validation(format!(
            "restore source does not exist: {}",
            path.display()
        )));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| invalid_source(path))?;
    let version: i64 = conn
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .map_err(|_| invalid_source(path))?;
    if version < 1 {
        return Err(invalid_source(path));
    }
    let has_individuals = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'individuals'")
        .and_then(|mut stmt| stmt.exists([]))
        .map_err(|_| invalid_source(path))?;
    if has_individuals {
        Ok(())
    } else {
        Err(invalid_source(path))
    }
}

/// The single "source is not a valid Kith database" error for [`validate_kith_db`].
fn invalid_source(path: &Path) -> CoreError {
    CoreError::Validation(format!(
        "restore source is not a valid Kith database: {}",
        path.display()
    ))
}

/// Removes a database file's `-wal` and `-shm` companions if present. Absent
/// sidecars are not an error. Critical after a restore so a previous database's
/// write-ahead log cannot shadow the freshly-copied main file.
fn remove_wal_sidecars(db: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm"] {
        let mut companion = db.as_os_str().to_owned();
        companion.push(suffix);
        match fs::remove_file(Path::new(&companion)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(CoreError::Io(e)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NewIndividual;

    /// A store with one named individual, plus its temp dir (kept alive by the
    /// caller). Returns `(dir, store)`.
    fn seeded_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = Store::open(dir.path().join("kith.db")).expect("open store");
        let draft = NewIndividual {
            given_name: Some("Ada".into()),
            surname: Some("Lovelace".into()),
            ..NewIndividual::default()
        };
        store.create_individual(&draft).expect("seed individual");
        (dir, store)
    }

    #[test]
    fn backup_produces_a_reopenable_copy_with_the_same_rows() {
        let (dir, store) = seeded_store();
        let backup = dir.path().join("backup.db");
        store.backup(&backup).expect("backup");

        let reopened = Store::open(&backup).expect("reopen backup");
        let people = reopened.list_individuals().expect("list");
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].surname.as_deref(), Some("Lovelace"));
    }

    #[test]
    fn backup_refuses_an_existing_destination() {
        let (dir, store) = seeded_store();
        let dest = dir.path().join("exists.db");
        std::fs::write(&dest, b"already here").expect("touch");
        let err = store.backup(&dest).expect_err("must refuse existing dest");
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn vacuum_succeeds_and_preserves_data() {
        let (_dir, store) = seeded_store();
        store.vacuum().expect("vacuum");
        assert_eq!(store.list_individuals().expect("list").len(), 1);
    }

    #[test]
    fn restore_round_trips_a_backup_onto_a_fresh_target() {
        let (dir, store) = seeded_store();
        let backup = dir.path().join("backup.db");
        store.backup(&backup).expect("backup");

        let target = dir.path().join("restored.db");
        Store::restore(&backup, &target).expect("restore");

        let restored = Store::open(&target).expect("open restored");
        assert_eq!(restored.list_individuals().expect("list").len(), 1);
    }

    #[test]
    fn restore_rejects_a_non_database_source_without_touching_the_target() {
        let dir = tempfile::tempdir().expect("temp dir");
        let garbage = dir.path().join("garbage.bin");
        std::fs::write(&garbage, b"not a sqlite database").expect("write garbage");
        let target = dir.path().join("target.db");

        let err = Store::restore(&garbage, &target).expect_err("must reject garbage");
        assert!(matches!(err, CoreError::Validation(_)));
        assert!(
            !target.exists(),
            "target must be untouched on validation failure"
        );
    }

    #[test]
    fn restore_rejects_a_missing_source() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = Store::restore(dir.path().join("nope.db"), dir.path().join("t.db"))
            .expect_err("must reject missing source");
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn restore_clears_a_stale_target_wal_sidecar() {
        // A backup of an *empty* database…
        let empty_dir = tempfile::tempdir().expect("temp dir");
        let empty = Store::open(empty_dir.path().join("empty.db")).expect("open empty");
        let backup = empty_dir.path().join("empty-backup.db");
        empty.backup(&backup).expect("backup empty");

        // …restored over a populated target that still has a stale `-wal`.
        let (dir, store) = seeded_store();
        let target = dir.path().join("kith.db"); // the seeded DB
        drop(store); // release the pool so the file copy can proceed (Windows)
        std::fs::write(format!("{}-wal", target.display()), b"stale wal").expect("plant wal");

        Store::restore(&backup, &target).expect("restore over stale wal");
        let restored = Store::open(&target).expect("open restored");
        // If the stale WAL had survived, the old rows could reappear; assert empty.
        assert_eq!(restored.list_individuals().expect("list").len(), 0);
    }
}
