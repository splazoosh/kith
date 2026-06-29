//! Database lifecycle: `db_create` / `db_open` / `db_close` / `db_current`, plus
//! the startup reopen.
//!
//! Mirrors the CLI's `init`-only-creates rule: `db_create` is the **only**
//! creator (it ensures the parent dir); `db_open` errors `io` if the file is
//! missing — never silently spawning a database from a typo'd path. Both persist
//! the opened path to the app-config file so [`reopen_last`] can reattach on the
//! next launch. The logic fns are synchronous: `Store::open` on a local file is
//! quick and takes no lock until [`attach`], so the seam's "no lock across await"
//! property is preserved without `spawn_blocking` here.

use std::path::{Path, PathBuf};

use kith_core::prelude::{Store, media_root_for};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::config::{self, AppConfig};
use crate::error::CommandError;
use crate::state::AppState;

/// What's open: the database path and its schema version.
#[derive(Debug, Clone, Serialize)]
pub struct DbInfo {
    /// The path of the open database.
    pub path: PathBuf,
    /// The schema version recorded in the file (`PRAGMA user_version`).
    pub schema_version: i64,
}

/// Ensures `path`'s parent directory exists (a bare filename is a no-op).
pub(crate) fn ensure_parent_dir(path: &Path) -> Result<(), CommandError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| CommandError::io(e.to_string()))?;
        }
    }
    Ok(())
}

/// Sets the open store + path into state and persists the app-config file. Adopting
/// a (different) database invalidates every undo snapshot, so the stack is cleared
/// — this covers create / open / GEDCOM-import.
pub(crate) fn attach(
    state: &AppState,
    config_dir: &Path,
    store: Store,
    path: PathBuf,
) -> Result<(), CommandError> {
    *state.db.lock().expect("AppState.db mutex poisoned") = Some(store);
    *state
        .last_path
        .lock()
        .expect("AppState.last_path mutex poisoned") = Some(path.clone());
    state.clear_undo();
    config::save(
        config_dir,
        &AppConfig {
            last_db_path: Some(path),
        },
    )
    .map_err(|e| CommandError::io(e.to_string()))
}

/// Creates (and migrates) a new database at `path`, the **only** path that
/// creates a file.
///
/// # Errors
/// [`CommandError`] (`io`/`database`) if the parent dir or the store cannot be
/// created, or the config cannot be written.
pub fn db_create_impl(
    state: &AppState,
    config_dir: &Path,
    path: PathBuf,
) -> Result<DbInfo, CommandError> {
    ensure_parent_dir(&path)?;
    let store = Store::open(&path)?; // creates + migrates
    let schema_version = store.schema_version()?;
    attach(state, config_dir, store, path.clone())?;
    Ok(DbInfo {
        path,
        schema_version,
    })
}

/// Opens an **existing** database at `path`.
///
/// # Errors
/// [`CommandError`] with `kind: io` if the file is missing (never silently
/// create); otherwise the mapped store/config error.
pub fn db_open_impl(
    state: &AppState,
    config_dir: &Path,
    path: PathBuf,
) -> Result<DbInfo, CommandError> {
    if !path.exists() {
        return Err(CommandError::io(format!(
            "no database at {}",
            path.display()
        )));
    }
    let store = Store::open(&path)?;
    let schema_version = store.schema_version()?;
    attach(state, config_dir, store, path.clone())?;
    Ok(DbInfo {
        path,
        schema_version,
    })
}

/// Closes the open database: clears state and persists an empty config so the
/// next launch starts with nothing open.
///
/// # Errors
/// [`CommandError`] with `kind: io` if the config cannot be written.
pub fn db_close_impl(state: &AppState, config_dir: &Path) -> Result<(), CommandError> {
    *state.db.lock().expect("AppState.db mutex poisoned") = None;
    *state
        .last_path
        .lock()
        .expect("AppState.last_path mutex poisoned") = None;
    state.clear_undo();
    config::save(config_dir, &AppConfig::default()).map_err(|e| CommandError::io(e.to_string()))
}

/// Reports what's open (the "what's open" header), or `None` if no database is open.
///
/// # Errors
/// [`CommandError`] if reading the schema version fails.
pub fn db_current_impl(state: &AppState) -> Result<Option<DbInfo>, CommandError> {
    let path = state
        .last_path
        .lock()
        .expect("AppState.last_path mutex poisoned")
        .clone();
    let store = state.db.lock().expect("AppState.db mutex poisoned").clone();
    match (path, store) {
        (Some(path), Some(store)) => Ok(Some(DbInfo {
            path,
            schema_version: store.schema_version()?,
        })),
        _ => Ok(None),
    }
}

/// Best-effort reopen of the last database at startup. A moved/deleted file
/// (or unreadable config) leaves no database open — the UI prompts open/create.
/// Factored out of `run()`'s `setup` so the headless suite can exercise it.
pub fn reopen_last(state: &AppState, config_dir: &Path) {
    let Some(path) = config::load(config_dir).last_db_path else {
        return;
    };
    if path.exists() {
        if let Ok(store) = Store::open(&path) {
            *state.db.lock().expect("AppState.db mutex poisoned") = Some(store);
            *state
                .last_path
                .lock()
                .expect("AppState.last_path mutex poisoned") = Some(path);
        }
    }
}

/// Resolves the app-config dir from the handle (keyed off the bundle identifier).
pub(crate) fn config_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app.path()
        .app_config_dir()
        .map_err(|e| CommandError::io(e.to_string()))
}

/// Extends the asset-protocol scope to the media folder beside `db_path`, so the
/// WebView can stream a person's portrait via `convertFileSrc` (the
/// asset-protocol display path). Called whenever a database becomes the open
/// one (create/open/import/reopen). Best-effort: a scope error (an unglobbable
/// path) is non-fatal — portraits simply won't stream; every other feature works.
pub(crate) fn allow_media_scope(app: &AppHandle, db_path: &Path) {
    let media_root = media_root_for(db_path);
    let _ = app
        .asset_protocol_scope()
        .allow_directory(&media_root, false);
}

/// Creates a new database and makes it the open one.
///
/// # Errors
/// See [`db_create_impl`].
#[tauri::command]
pub async fn db_create(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<DbInfo, CommandError> {
    let dir = config_dir(&app)?;
    let info = db_create_impl(state.inner(), &dir, path)?;
    allow_media_scope(&app, &info.path);
    Ok(info)
}

/// Opens an existing database and makes it the open one.
///
/// # Errors
/// See [`db_open_impl`].
#[tauri::command]
pub async fn db_open(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<DbInfo, CommandError> {
    let dir = config_dir(&app)?;
    let info = db_open_impl(state.inner(), &dir, path)?;
    allow_media_scope(&app, &info.path);
    Ok(info)
}

/// Closes the open database.
///
/// # Errors
/// See [`db_close_impl`].
#[tauri::command]
pub async fn db_close(app: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError> {
    let dir = config_dir(&app)?;
    db_close_impl(state.inner(), &dir)
}

/// Reports the open database, or `null` if none.
///
/// # Errors
/// See [`db_current_impl`].
#[tauri::command]
pub async fn db_current(state: State<'_, AppState>) -> Result<Option<DbInfo>, CommandError> {
    db_current_impl(state.inner())
}
