//! SQLite persistence: the [`Store`] facade, its connection pool, the
//! connect-time PRAGMAs, the embedded schema migrations, and the typed CRUD
//! surface over the domain entities.
//!
//! Each entity's CRUD lives in its own focused submodule ([`individual`],
//! [`family`], [`event`], [`media`], [`name`], [`source`]), each adding an
//! `impl Store` block. This module holds the shared plumbing they build on.

mod event;
mod family;
mod individual;
mod maintenance;
mod media;
mod name;
mod search;
mod source;
mod undo;

pub use media::media_root_for;
pub use search::SearchHit;
pub use undo::{DeleteTarget, Deletion, PartnerSlot};

use std::path::Path;
use std::sync::LazyLock;

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite_migration::{M, Migrations};

use crate::error::Result;

/// Embedded, append-only schema migrations. The schema version is tracked in
/// SQLite's `user_version`; [`Migrations::to_latest`] applies only what is
/// pending, so opening an up-to-date database is a no-op.
static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("../../../../migrations/0001_initial.sql")),
        // 0002: the FTS5 `person_search` index + sync triggers +
        // backfill. Append-only — 0001 is immutable.
        M::up(include_str!("../../../../migrations/0002_search_fts.sql")),
    ])
});

/// PRAGMAs applied to *every* pooled connection via `with_init`.
///
/// `foreign_keys` is per-connection in SQLite — setting it here (not once at
/// open) is what makes `ON DELETE CASCADE` actually fire. `journal_mode = WAL`
/// is a no-op on `:memory:`, which is fine for tests.
const INIT_PRAGMAS: &str = "\
PRAGMA journal_mode = WAL;\
PRAGMA foreign_keys = ON;\
PRAGMA synchronous = NORMAL;\
PRAGMA busy_timeout = 5000;";

type Pool = r2d2::Pool<SqliteConnectionManager>;

/// A pooled connection borrowed from the [`Store`]'s pool. `pub(crate)` so a
/// read-heavy walk ([`crate::query`]) can hold **one** connection for its whole
/// duration and route every per-row read through it (`prepare_cached` statements
/// reused, no per-read pool checkout) — the hot-path lever — without
/// the `r2d2`/`r2d2_sqlite` types leaking past this module.
pub(crate) type PooledConn = PooledConnection<SqliteConnectionManager>;

/// The current UTC time as an ISO-8601 / RFC 3339 string for the `created_at`
/// / `updated_at` columns (e.g. `2026-06-27T18:30:00Z`). Only `individuals`
/// and `families` carry these columns.
///
/// # Panics
/// Panics only if the system clock is set outside jiff's `-9999..=9999` year
/// range — a broken-environment invariant, not a recoverable error
/// (`err-expect-bugs-only`).
pub(crate) fn now_timestamp() -> String {
    jiff::Timestamp::now().to_string()
}

/// A handle to a Kith database.
///
/// `Store` owns an `r2d2` connection pool and is cheap to clone (the pool is
/// reference-counted internally), so it can be shared across threads — the
/// future Tauri layer keeps one in app state.
#[derive(Clone)]
pub struct Store {
    pool: Pool,
}

impl Store {
    /// Opens the database at `path`, creating the file if it does not exist,
    /// and applies all pending migrations.
    ///
    /// The parent directory must already exist.
    ///
    /// # Errors
    /// Returns [`CoreError`](crate::error::CoreError) if the pool cannot be
    /// built or the migrations fail to apply.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::from_manager(SqliteConnectionManager::file(path), 8)
    }

    /// Opens a private in-memory database and applies all migrations.
    ///
    /// The pool is capped at **one** connection on purpose: every `:memory:`
    /// connection is a *distinct* database, so a larger pool would hand out
    /// connections that never saw the migrated schema. The single connection
    /// also keeps the in-memory database alive for the lifetime of the `Store`.
    ///
    /// # Errors
    /// Returns [`CoreError`](crate::error::CoreError) if the pool cannot be
    /// built or the migrations fail to apply.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_manager(SqliteConnectionManager::memory(), 1)
    }

    fn from_manager(manager: SqliteConnectionManager, max_size: u32) -> Result<Self> {
        let manager = manager.with_init(|c| c.execute_batch(INIT_PRAGMAS));
        let pool = r2d2::Pool::builder().max_size(max_size).build(manager)?;
        {
            let mut conn = pool.get()?;
            MIGRATIONS.to_latest(&mut conn)?;
        }
        Ok(Self { pool })
    }

    /// Returns the schema version recorded in the database
    /// (SQLite's `user_version`, equal to the number of applied migrations).
    ///
    /// # Errors
    /// Returns [`CoreError`](crate::error::CoreError) if a connection cannot be
    /// acquired or the query fails.
    pub fn schema_version(&self) -> Result<i64> {
        let conn = self.conn()?;
        Ok(conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?)
    }

    /// Borrows a pooled connection. Internal: callers go through the typed
    /// `Store` API rather than touching raw connections.
    ///
    /// # Errors
    /// Returns [`CoreError`](crate::error::CoreError) if no connection is
    /// available before the pool's timeout.
    pub(crate) fn conn(&self) -> Result<PooledConn> {
        Ok(self.pool.get()?)
    }

    /// Runs `f` inside a single transaction, committing on `Ok` and rolling back
    /// on `Err` (the un-committed [`rusqlite::Transaction`] rolls back on drop).
    ///
    /// The closure receives a `&Connection` — a `&Transaction` deref-coerces — so
    /// the per-entity `*_in` insert helpers (e.g. [`Store::create_individual_in`])
    /// are callable both here and from the auto-commit public methods, keeping one
    /// INSERT implementation behind two entry points. This is the bulk-write seam
    /// the GEDCOM importer uses for atomicity (a parse/validate failure happens
    /// *before* the transaction, so nothing is written).
    ///
    /// # Errors
    /// Propagates `f`'s error (after the rollback) or a pool/commit failure.
    pub(crate) fn transaction<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T>,
    ) -> Result<T> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use crate::model::NewIndividual;

    #[test]
    fn transaction_commits_on_ok() {
        let store = Store::open_in_memory().expect("open store");
        let id = store
            .transaction(|conn| Store::create_individual_in(conn, &NewIndividual::default()))
            .expect("transaction")
            .id;
        assert!(
            store.get_individual(id).is_ok(),
            "committed row is readable"
        );
    }

    #[test]
    fn transaction_rolls_back_on_error() {
        let store = Store::open_in_memory().expect("open store");
        let result: Result<()> = store.transaction(|conn| {
            // A write happens, then the closure fails — the whole tx must roll back.
            Store::create_individual_in(conn, &NewIndividual::default())?;
            Err(CoreError::Validation("boom".to_owned()))
        });
        assert!(matches!(result, Err(CoreError::Validation(_))));
        assert!(
            store.list_individuals().expect("list").is_empty(),
            "a failed transaction leaves zero rows"
        );
    }

    #[test]
    fn open_in_memory_applies_migrations_to_version_2() {
        // Arrange / Act
        let store = Store::open_in_memory().expect("open in-memory store");
        // Assert — two migrations applied (0001 schema + 0002 search FTS).
        assert_eq!(store.schema_version().expect("read version"), 2);
    }

    #[test]
    fn foreign_keys_pragma_is_on_for_pooled_connections() {
        let store = Store::open_in_memory().expect("open store");
        let conn = store.conn().expect("borrow connection");
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("read pragma");
        assert_eq!(fk, 1, "foreign_keys must be ON per connection");
    }

    #[test]
    fn migration_creates_all_expected_tables() {
        let store = Store::open_in_memory().expect("open store");
        let conn = store.conn().expect("borrow connection");
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN
                 ('individuals','names','families','family_children','places',
                  'events','sources','citations','media','media_links');",
                [],
                |row| row.get(0),
            )
            .expect("count tables");
        assert_eq!(count, 10, "all ten schema tables must exist");
    }

    #[test]
    fn events_check_rejects_two_subjects() {
        let store = Store::open_in_memory().expect("open store");
        let conn = store.conn().expect("borrow connection");
        // Need a real individual and family to satisfy the foreign keys.
        conn.execute_batch(
            "INSERT INTO individuals (id, living, created_at, updated_at)
                 VALUES (1, 1, '2026-01-01', '2026-01-01');
             INSERT INTO families (id, created_at, updated_at)
                 VALUES (1, '2026-01-01', '2026-01-01');",
        )
        .expect("seed rows");
        let both = conn.execute(
            "INSERT INTO events (individual_id, family_id, kind)
                 VALUES (1, 1, 'birth');",
            [],
        );
        assert!(both.is_err(), "two subjects must violate the CHECK");
    }

    #[test]
    fn events_check_rejects_no_subject() {
        let store = Store::open_in_memory().expect("open store");
        let conn = store.conn().expect("borrow connection");
        let neither = conn.execute("INSERT INTO events (kind) VALUES ('birth');", []);
        assert!(neither.is_err(), "no subject must violate the CHECK");
    }

    #[test]
    fn open_creates_and_migrates_a_file_database() {
        // tempfile dev-dependency exercises the real file + WAL path.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("kith-test.db");
        let store = Store::open(&path).expect("open file store");
        assert_eq!(store.schema_version().expect("version"), 2);
        assert!(path.exists(), "database file should be created");
        // Reopening is idempotent (no pending migrations).
        let reopened = Store::open(&path).expect("reopen");
        assert_eq!(reopened.schema_version().expect("version"), 2);
    }
}
