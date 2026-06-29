//! Media commands: import / list / set-primary / delete, plus the asset-protocol
//! path resolution for the live canvas.
//!
//! Thin wrappers over the `kith_core::db` media surface. The open/save dialogs
//! (frontend) yield the chosen image's path *string*; [`media_import`] copies the
//! bytes in Rust, so **no `fs` ACL is added**. The live canvas streams a portrait
//! over Tauri's **asset protocol**: [`media_paths`] returns each
//! media file's absolute on-disk path and the frontend feeds it to
//! `convertFileSrc`. The HTML *export* takes a different path — base64 `data:`
//! URLs (`export.rs`) — so it stays self-contained. **No media logic lives here**:
//! the copy, the single-primary invariant, and the mime check are all in
//! `kith_core::db`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kith_core::prelude::{DeleteTarget, MediaId, MediaItem, MediaSubject, media_root_for};
use tauri::{AppHandle, State};

use crate::commands::db;
use crate::error::CommandError;
use crate::state::AppState;

/// The media folder beside the open database (`<db-stem>.media`), or
/// [`CommandError::no_database`] when nothing is open (an unsaved DB has nowhere
/// to put files). Shared with the export command's portrait resolver.
pub(crate) fn media_root(state: &AppState) -> Result<PathBuf, CommandError> {
    let path = state
        .last_path
        .lock()
        .expect("AppState.last_path mutex poisoned")
        .clone()
        .ok_or_else(CommandError::no_database)?;
    Ok(media_root_for(&path))
}

/// Imports an image for `subject`, copying the bytes into the media folder and
/// (optionally) marking it the subject's primary. Returns the created item.
///
/// # Errors
/// [`CommandError`] `validation` (no DB / unsupported type), `io` (unreadable
/// source / copy failure), or `database`.
pub async fn media_import_impl(
    state: &AppState,
    subject: MediaSubject,
    file_path: String,
    is_primary: bool,
) -> Result<MediaItem, CommandError> {
    let root = media_root(state)?;
    state
        .with_store(move |store| {
            let media = store.import_media(&root, Path::new(&file_path), subject, is_primary)?;
            Ok(MediaItem { media, is_primary })
        })
        .await
}

/// Lists a subject's media (primary first), for the detail-view gallery.
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn media_for_impl(
    state: &AppState,
    subject: MediaSubject,
) -> Result<Vec<MediaItem>, CommandError> {
    state
        .with_store(move |store| store.list_media_for(subject))
        .await
}

/// Resolves media ids to absolute file paths for `convertFileSrc` (the canvas
/// portrait stream). Batched — one call per chart, not one per node.
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn media_paths_impl(
    state: &AppState,
    ids: Vec<MediaId>,
) -> Result<BTreeMap<MediaId, String>, CommandError> {
    let root = media_root(state)?;
    state
        .with_store(move |store| store.media_paths(&root, &ids))
        .await
}

/// Marks a media item the subject's primary (portrait).
///
/// # Errors
/// [`CommandError`] `not_found` if it is not linked to the subject, else the
/// mapped store error.
pub async fn media_set_primary_impl(
    state: &AppState,
    media: MediaId,
    subject: MediaSubject,
) -> Result<(), CommandError> {
    state
        .with_store(move |store| store.set_primary(media, subject))
        .await
}

/// Deletes a media row (its links cascade). The on-disk file is left in place, so
/// an undo restores the rows and the bytes are still there. Snapshots the cascade
/// onto the session undo stack first.
///
/// # Errors
/// [`CommandError`] `not_found` if no such row, else the mapped store error.
pub async fn media_delete_impl(state: &AppState, id: MediaId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Media(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// `media_import` IPC command — see [`media_import_impl`].
///
/// # Errors
/// See [`media_import_impl`].
#[tauri::command]
pub async fn media_import(
    app: AppHandle,
    state: State<'_, AppState>,
    subject: MediaSubject,
    file_path: String,
    is_primary: bool,
) -> Result<MediaItem, CommandError> {
    let item = media_import_impl(state.inner(), subject, file_path, is_primary).await?;
    // The media folder now certainly exists — (re-)allow it in the asset scope so
    // the new portrait streams immediately (it may not have existed at DB-open).
    if let Some(path) = state
        .last_path
        .lock()
        .expect("AppState.last_path mutex poisoned")
        .clone()
    {
        db::allow_media_scope(&app, &path);
    }
    Ok(item)
}

/// `media_for` IPC command — see [`media_for_impl`].
///
/// # Errors
/// See [`media_for_impl`].
#[tauri::command]
pub async fn media_for(
    state: State<'_, AppState>,
    subject: MediaSubject,
) -> Result<Vec<MediaItem>, CommandError> {
    media_for_impl(state.inner(), subject).await
}

/// `media_paths` IPC command — see [`media_paths_impl`].
///
/// # Errors
/// See [`media_paths_impl`].
#[tauri::command]
pub async fn media_paths(
    state: State<'_, AppState>,
    ids: Vec<MediaId>,
) -> Result<BTreeMap<MediaId, String>, CommandError> {
    media_paths_impl(state.inner(), ids).await
}

/// `media_set_primary` IPC command — see [`media_set_primary_impl`].
///
/// # Errors
/// See [`media_set_primary_impl`].
#[tauri::command]
pub async fn media_set_primary(
    state: State<'_, AppState>,
    media: MediaId,
    subject: MediaSubject,
) -> Result<(), CommandError> {
    media_set_primary_impl(state.inner(), media, subject).await
}

/// `media_delete` IPC command — see [`media_delete_impl`].
///
/// # Errors
/// See [`media_delete_impl`].
#[tauri::command]
pub async fn media_delete(state: State<'_, AppState>, id: MediaId) -> Result<(), CommandError> {
    media_delete_impl(state.inner(), id).await
}
