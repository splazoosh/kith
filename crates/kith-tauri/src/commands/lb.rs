//! `import_lb` — the desktop "LB" JSON import surface.
//!
//! A single thin command over [`kith_core::lb::import`]. **No LB logic lives
//! here** — the JSON parse, the family synthesis from parent/spouse pointers, the
//! `01.01.1753` unknown-date sentinel, the one-transaction atomicity, and the
//! id/pointer validation all live in `kith_core::lb`. This command only marshals
//! paths, does the file IO off the UI thread, and returns the [`LbImport`]. The
//! dialog (frontend) yields the path *strings*; this command does the IO, so no
//! `fs` ACL is needed.
//!
//! **Import is a *new tree*** — exactly like the GUI's GEDCOM import (`gedcom.rs`):
//! it creates a fresh database at the chosen path, imports into it (non-merge), and
//! makes it the open one. A bad file never swaps the open tree for a broken one: the
//! new store is opened, imported, and only *then* attached, so a malformed file
//! leaves the previously-open database untouched. Additive `merge` stays a
//! core-only capability — there is no LB merge surface in the GUI.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use kith_core::lb;
use kith_core::prelude::{CoreError, ImportOptions, ImportSummary, Store};

use crate::commands::db::{self, DbInfo};
use crate::error::CommandError;
use crate::state::AppState;

/// The outcome of a fresh-tree LB import: the now-open database + the summary.
///
/// The same shape as [`GedcomImport`](crate::commands::gedcom::GedcomImport) (a new
/// [`DbInfo`] + the shared [`ImportSummary`]); LB carries no alternate names / media
/// / sources / citations, so those summary counts stay zero.
#[derive(Debug, Clone, Serialize)]
pub struct LbImport {
    /// The freshly created database, now the open one.
    pub db: DbInfo,
    /// Counts from the import (individuals / families / events / places).
    pub summary: ImportSummary,
}

/// Reads an "LB" JSON file into a **new** database at `db_path` and makes it the
/// open one, returning the [`LbImport`] (the new [`DbInfo`] + the [`ImportSummary`]).
///
/// All the heavy IO/DB work runs off the UI thread: it reads the file bytes, decodes
/// UTF-8 (a non-UTF-8 file → [`CoreError::Validation`], not `Io`, raised *before* any
/// database is created), creates + migrates a fresh [`Store`] at `db_path`, and imports
/// with `merge = false` (a brand-new tree). The JSON parse, validation, and the
/// one-transaction write are inherited from the engine. The new store is **attached
/// only on success**, so a malformed file leaves the previously-open database untouched
/// and opens nothing.
///
/// # Errors
/// - [`CommandError`] `Io` if the file can't be read, or the new database can't be
///   created (bad path, permissions).
/// - `Validation` if the file is not UTF-8, is malformed JSON, or carries a
///   zero/duplicate record id or a dangling parent/spouse pointer (from the engine).
/// - `Database` if a write fails (the transaction rolls back — nothing is committed).
pub async fn import_lb_impl(
    state: &AppState,
    config_dir: &Path,
    file_path: String,
    db_path: PathBuf,
) -> Result<LbImport, CommandError> {
    let target = db_path.clone();
    // Read + decode + create + import, all off the UI thread; none of it touches
    // `AppState`, so a failure leaves whatever is open exactly as it was.
    let (store, schema_version, summary) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(Store, i64, ImportSummary), CommandError> {
            // Read BYTES then decode (not read_to_string) so a non-UTF-8 file is a
            // Validation the user can fix, not an Io — and before any DB is created,
            // so a bad encoding leaves no stray file.
            let bytes = fs::read(&file_path).map_err(CoreError::Io)?;
            let text = String::from_utf8(bytes).map_err(|_| {
                CoreError::Validation(
                    "the file is not valid UTF-8 — re-export the LB JSON as UTF-8".to_owned(),
                )
            })?;
            // Create + migrate the fresh target, then import into it. `merge = false`
            // (the default) is the new-tree path: the engine refuses a non-empty store,
            // and a freshly created one is empty.
            db::ensure_parent_dir(&target)?;
            let store = Store::open(&target)?;
            let options = ImportOptions::default();
            let summary = lb::import(&store, &text, &options)?;
            let schema_version = store.schema_version()?;
            Ok((store, schema_version, summary))
        },
    )
    .await
    .map_err(|e| CommandError::unexpected(format!("background task failed: {e}")))??;

    // Success — adopt the new store as the open database (quick: a lock + a config write).
    db::attach(state, config_dir, store, db_path.clone())?;
    Ok(LbImport {
        db: DbInfo {
            path: db_path,
            schema_version,
        },
        summary,
    })
}

/// `import_lb` IPC command — see [`import_lb_impl`].
///
/// # Errors
/// See [`import_lb_impl`].
#[tauri::command]
pub async fn import_lb(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
    db_path: PathBuf,
) -> Result<LbImport, CommandError> {
    let dir = db::config_dir(&app)?;
    let result = import_lb_impl(state.inner(), &dir, file_path, db_path).await?;
    // The fresh tree is now the open one; keep the media-scope invariant uniform with
    // the other DB-open paths (LB imports no media, so this is a harmless no-op today).
    db::allow_media_scope(&app, &result.db.path);
    Ok(result)
}
