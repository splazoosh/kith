//! Source & citation commands: create/list/get/update/delete sources
//! and add/list/delete citations.
//!
//! Pure-DB wrappers over the `kith_core::db` source/citation surface, each routed
//! through [`AppState::with_store`] so they run off the UI thread. Unlike the
//! media commands, sources carry no bytes — there is **no dialog, no path string,
//! and no `fs`/asset ACL** (`capabilities/default.json` is unchanged). No
//! source/citation logic lives here: the CRUD, the cascade, and the "exactly one
//! subject" discipline are all in `kith_core`. Deleting a source cascades its
//! citations (the schema's `ON DELETE CASCADE`); the *frontend* surfaces that in
//! its confirm — the command just executes.

use kith_core::prelude::{
    CitationId, CitationItem, CitationSubject, DeleteTarget, NewCitation, NewSource, Source,
    SourceId, SourceView,
};
use tauri::State;

use crate::error::CommandError;
use crate::state::AppState;

/// Creates a source, returning the persisted record.
///
/// # Errors
/// [`CommandError`] if no database is open or the insert fails.
pub async fn source_create_impl(
    state: &AppState,
    source: NewSource,
) -> Result<Source, CommandError> {
    state
        .with_store(move |store| store.create_source(&source))
        .await
}

/// Lists every source, ascending id.
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn source_list_impl(state: &AppState) -> Result<Vec<Source>, CommandError> {
    state.with_store(|store| store.list_sources()).await
}

/// Loads a source with the facts it supports.
///
/// # Errors
/// [`CommandError`] `not_found` if no source has `id`, else the mapped error.
pub async fn source_get_impl(state: &AppState, id: SourceId) -> Result<SourceView, CommandError> {
    state
        .with_store(move |store| SourceView::load(&store, id))
        .await
}

/// Updates a source's fields, returning the updated record.
///
/// # Errors
/// [`CommandError`] `not_found` if no source has `id`, else the mapped error.
pub async fn source_update_impl(
    state: &AppState,
    id: SourceId,
    source: NewSource,
) -> Result<Source, CommandError> {
    state
        .with_store(move |store| store.update_source(id, &source))
        .await
}

/// Deletes a source; its citations cascade. Snapshots the cascade onto the session
/// undo stack first.
///
/// # Errors
/// [`CommandError`] `not_found` if no source has `id`, else the mapped error.
pub async fn source_delete_impl(state: &AppState, id: SourceId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Source(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// Adds a citation against a fact, returning the item with its source resolved.
///
/// # Errors
/// [`CommandError`] if no database is open or the insert fails (e.g. an FK
/// violation on a non-existent source/subject).
pub async fn citation_add_impl(
    state: &AppState,
    citation: NewCitation,
) -> Result<CitationItem, CommandError> {
    state
        .with_store(move |store| {
            let added = store.add_citation(&citation)?;
            let source = store.get_source(added.source)?;
            Ok(CitationItem {
                citation: added,
                source,
            })
        })
        .await
}

/// Lists a subject's citations, each with its source resolved.
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn citations_for_impl(
    state: &AppState,
    subject: CitationSubject,
) -> Result<Vec<CitationItem>, CommandError> {
    state
        .with_store(move |store| store.citations_for(subject))
        .await
}

/// Deletes a citation, snapshotting it onto the session undo stack first.
///
/// # Errors
/// [`CommandError`] `not_found` if no citation has `id`, else the mapped error.
pub async fn citation_delete_impl(state: &AppState, id: CitationId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Citation(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// `source_create` IPC command — see [`source_create_impl`].
///
/// # Errors
/// See [`source_create_impl`].
#[tauri::command]
pub async fn source_create(
    state: State<'_, AppState>,
    source: NewSource,
) -> Result<Source, CommandError> {
    source_create_impl(state.inner(), source).await
}

/// `source_list` IPC command — see [`source_list_impl`].
///
/// # Errors
/// See [`source_list_impl`].
#[tauri::command]
pub async fn source_list(state: State<'_, AppState>) -> Result<Vec<Source>, CommandError> {
    source_list_impl(state.inner()).await
}

/// `source_get` IPC command — see [`source_get_impl`].
///
/// # Errors
/// See [`source_get_impl`].
#[tauri::command]
pub async fn source_get(
    state: State<'_, AppState>,
    id: SourceId,
) -> Result<SourceView, CommandError> {
    source_get_impl(state.inner(), id).await
}

/// `source_update` IPC command — see [`source_update_impl`].
///
/// # Errors
/// See [`source_update_impl`].
#[tauri::command]
pub async fn source_update(
    state: State<'_, AppState>,
    id: SourceId,
    source: NewSource,
) -> Result<Source, CommandError> {
    source_update_impl(state.inner(), id, source).await
}

/// `source_delete` IPC command — see [`source_delete_impl`].
///
/// # Errors
/// See [`source_delete_impl`].
#[tauri::command]
pub async fn source_delete(state: State<'_, AppState>, id: SourceId) -> Result<(), CommandError> {
    source_delete_impl(state.inner(), id).await
}

/// `citation_add` IPC command — see [`citation_add_impl`].
///
/// # Errors
/// See [`citation_add_impl`].
#[tauri::command]
pub async fn citation_add(
    state: State<'_, AppState>,
    citation: NewCitation,
) -> Result<CitationItem, CommandError> {
    citation_add_impl(state.inner(), citation).await
}

/// `citations_for` IPC command — see [`citations_for_impl`].
///
/// # Errors
/// See [`citations_for_impl`].
#[tauri::command]
pub async fn citations_for(
    state: State<'_, AppState>,
    subject: CitationSubject,
) -> Result<Vec<CitationItem>, CommandError> {
    citations_for_impl(state.inner(), subject).await
}

/// `citation_delete` IPC command — see [`citation_delete_impl`].
///
/// # Errors
/// See [`citation_delete_impl`].
#[tauri::command]
pub async fn citation_delete(
    state: State<'_, AppState>,
    id: CitationId,
) -> Result<(), CommandError> {
    citation_delete_impl(state.inner(), id).await
}
