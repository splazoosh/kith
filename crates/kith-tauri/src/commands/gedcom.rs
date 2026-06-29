//! `export_gedcom` / `import_gedcom` — the desktop GEDCOM 5.5.1 interop surface.
//!
//! Two thin commands over [`kith_core::gedcom::export`] / [`kith_core::gedcom::import`].
//! **No GEDCOM logic lives here** — parsing, serialization, the two-pass xref mapping,
//! the one-transaction atomicity, the redaction-exemption, and the supported-field
//! skip-and-count all live in `kith_core::gedcom`. These commands only
//! marshal paths, do the file IO off the UI thread, and return `()` / the
//! [`GedcomImport`]. The dialogs (frontend) yield the path *strings*; these commands
//! do the IO, so no `fs` ACL is needed.
//!
//! **Import is a *new tree*.** Unlike
//! the CLI's `--merge`, the GUI never appends into the open database: `import_gedcom`
//! **creates a fresh database** at the chosen path, imports into it (non-merge), and
//! makes it the open one — the conventional "Import GEDCOM → new tree" desktop flow,
//! reachable whether or not a database is already open. A bad file never swaps the
//! open tree for a broken one: the new store is opened, imported, and only *then*
//! attached, so a malformed file leaves the previously-open database untouched.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use kith_core::gedcom;
use kith_core::prelude::{CoreError, ImportOptions, ImportSummary, Store};

use crate::commands::db::{self, DbInfo};
use crate::error::CommandError;
use crate::state::AppState;

/// Serializes the whole open database to GEDCOM 5.5.1 and writes it to `out_path`.
///
/// Runs off the UI thread via [`AppState::with_store`]. The export is whole-tree and
/// **un-redacted** (a full-fidelity data move): there are no chart args
/// and no `include_living` opt-out. The write's `io::Error` is carried as
/// [`CoreError::Io`] so the [`CommandError`] surfaces with `kind == Io` (the GUI
/// analogue of the CLI's exit-5 mapping).
///
/// # Errors
/// - [`CommandError`] `Io` if the write fails (bad path, permissions, missing parent),
///   or if no database is open.
/// - `Database` if a `Store` read fails.
pub async fn export_gedcom_impl(state: &AppState, out_path: String) -> Result<(), CommandError> {
    state
        .with_store(move |store| {
            // Whole-DB, deterministic, un-redacted. One core call — no tag logic here.
            let doc = gedcom::export(&store)?;
            // The write's io::Error MUST be carried as CoreError::Io or `From<CoreError>`
            // maps it to `Unexpected`, not `Io`.
            fs::write(&out_path, doc).map_err(CoreError::Io)?;
            Ok(())
        })
        .await
}

/// `export_gedcom` IPC command — see [`export_gedcom_impl`].
///
/// # Errors
/// See [`export_gedcom_impl`].
#[tauri::command]
pub async fn export_gedcom(
    state: State<'_, AppState>,
    out_path: String,
) -> Result<(), CommandError> {
    export_gedcom_impl(state.inner(), out_path).await
}

/// The outcome of a fresh-tree GEDCOM import: the now-open database + the summary.
#[derive(Debug, Clone, Serialize)]
pub struct GedcomImport {
    /// The freshly created database, now the open one.
    pub db: DbInfo,
    /// Counts + the skipped-tag map from the import.
    pub summary: ImportSummary,
}

/// Reads a GEDCOM 5.5.1 file into a **new** database at `db_path` and makes it the
/// open one, returning the [`GedcomImport`] (the new [`DbInfo`] + the
/// [`ImportSummary`]).
///
/// All the heavy IO/DB work runs off the UI thread: it reads the file bytes, decodes
/// UTF-8 (a non-UTF-8 file → [`CoreError::Validation`], not `Io`, raised *before* any
/// database is created), creates + migrates a fresh [`Store`] at `db_path`, and imports
/// with `merge = false` (a brand-new tree). Parsing, validation, and the one-transaction
/// write are inherited from the engine. The new store is **attached only on success**, so
/// a malformed file leaves the previously-open database untouched and opens nothing.
///
/// # Errors
/// - [`CommandError`] `Io` if the file can't be read, or the new database can't be
///   created (bad path, permissions).
/// - `Validation` if the file is not UTF-8, is malformed, declares `CHAR ANSEL`, or
///   carries a dangling xref (line-numbered, from the engine).
/// - `Database` if a write fails (the transaction rolls back — nothing is committed).
pub async fn import_gedcom_impl(
    state: &AppState,
    config_dir: &Path,
    file_path: String,
    db_path: PathBuf,
) -> Result<GedcomImport, CommandError> {
    let target = db_path.clone();
    // Read + decode + create + import, all off the UI thread; none of it touches
    // `AppState`, so a failure leaves whatever is open exactly as it was.
    let (store, schema_version, summary) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(Store, i64, ImportSummary), CommandError> {
            // Read BYTES then decode (not read_to_string) so a non-UTF-8 file is a
            // Validation the user can fix, not an Io — and before any DB
            // is created, so a bad encoding leaves no stray file.
            let bytes = fs::read(&file_path).map_err(CoreError::Io)?;
            let text = String::from_utf8(bytes).map_err(|_| {
                CoreError::Validation(
                    "the file is not valid UTF-8 — ANSEL/UTF-16 GEDCOM is unsupported; \
                     re-export it as UTF-8"
                        .to_owned(),
                )
            })?;
            // Create + migrate the fresh target, then import into it. `merge = false`
            // (the default) is the new-tree path: the engine refuses a non-empty store,
            // and a freshly created one is empty — no de-duplication needed.
            db::ensure_parent_dir(&target)?;
            let store = Store::open(&target)?;
            let options = ImportOptions::default();
            let summary = gedcom::import(&store, &text, &options)?;
            let schema_version = store.schema_version()?;
            Ok((store, schema_version, summary))
        },
    )
    .await
    .map_err(|e| CommandError::unexpected(format!("background task failed: {e}")))??;

    // Success — adopt the new store as the open database (quick: a lock + a config write).
    db::attach(state, config_dir, store, db_path.clone())?;
    Ok(GedcomImport {
        db: DbInfo {
            path: db_path,
            schema_version,
        },
        summary,
    })
}

/// `import_gedcom` IPC command — see [`import_gedcom_impl`].
///
/// # Errors
/// See [`import_gedcom_impl`].
#[tauri::command]
pub async fn import_gedcom(
    app: AppHandle,
    state: State<'_, AppState>,
    file_path: String,
    db_path: PathBuf,
) -> Result<GedcomImport, CommandError> {
    let dir = db::config_dir(&app)?;
    let result = import_gedcom_impl(state.inner(), &dir, file_path, db_path).await?;
    // The fresh tree is now the open one; allow its media folder in the asset scope
    // so any imported `OBJE` portraits stream to the canvas.
    db::allow_media_scope(&app, &result.db.path);
    Ok(result)
}
