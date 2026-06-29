//! Name commands: add / list / remove (an individual's alternate names).
//!
//! Date-free, so they (de)serialize the core types directly — `add` takes a
//! [`NewName`] (which carries its own `individual_id` and `sort_order`).

use kith_core::prelude::{DeleteTarget, Name, NameId, NewName, PersonId};
use tauri::State;

use crate::error::CommandError;
use crate::state::AppState;

/// Attaches a new alternate name.
///
/// # Errors
/// [`CommandError`] if the insert fails (e.g. an FK violation on a non-existent
/// individual).
pub async fn name_add_impl(state: &AppState, draft: NewName) -> Result<Name, CommandError> {
    state.with_store(move |store| store.add_name(&draft)).await
}

/// Lists an individual's alternate names.
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn name_list_impl(
    state: &AppState,
    individual: PersonId,
) -> Result<Vec<Name>, CommandError> {
    state
        .with_store(move |store| store.list_names(individual))
        .await
}

/// Removes an alternate name by id, snapshotting it onto the session undo stack first.
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if the row is gone.
pub async fn name_remove_impl(state: &AppState, id: NameId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Name(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// IPC: add an alternate name.
///
/// # Errors
/// See [`name_add_impl`].
#[tauri::command]
pub async fn name_add(state: State<'_, AppState>, draft: NewName) -> Result<Name, CommandError> {
    name_add_impl(state.inner(), draft).await
}

/// IPC: list an individual's alternate names.
///
/// # Errors
/// See [`name_list_impl`].
#[tauri::command]
pub async fn name_list(
    state: State<'_, AppState>,
    individual_id: i64,
) -> Result<Vec<Name>, CommandError> {
    name_list_impl(state.inner(), PersonId::new(individual_id)).await
}

/// IPC: remove an alternate name by id.
///
/// # Errors
/// See [`name_remove_impl`].
#[tauri::command]
pub async fn name_remove(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    name_remove_impl(state.inner(), NameId::new(id)).await
}
