//! The undo command: pop the last captured deletion and restore it.
//!
//! All restore logic lives in `kith_core` ([`Store::restore_deletion`]); this
//! command only pops the session stack ([`AppState`]), runs the restore off the UI
//! thread via [`AppState::with_store`], and marshals an [`UndoOutcome`] back so the
//! frontend can refresh the right view (`kind`), name what came back (`label`), and
//! keep its undo affordance in sync (`remaining`). On a restore failure — the
//! reused-id conflict — the popped entry is **dropped** (not re-pushed) and the
//! typed error surfaces.

use kith_core::prelude::{Deletion, Individual};
use serde::Serialize;
use tauri::State;

use crate::error::CommandError;
use crate::state::AppState;

/// The result of a successful undo: which view family to refresh, a human label
/// for the "Restored …" confirmation, and the remaining stack depth.
#[derive(Debug, Clone, Serialize)]
pub struct UndoOutcome {
    /// The restored entity family (`"person"`, `"family"`, `"event"`, `"name"`,
    /// `"child"`, `"source"`, `"citation"`, `"media"`) — the frontend refreshes by it.
    pub kind: String,
    /// A human label for the restored record (e.g. `"Ada Lovelace"`).
    pub label: String,
    /// The number of deletions still on the undo stack after this one.
    pub remaining: usize,
}

/// Pops the most recent deletion and restores it, returning the [`UndoOutcome`] —
/// or `None` if the undo stack is empty.
///
/// # Errors
/// [`CommandError`] `io` if no database is open, or the mapped store error if the
/// restore fails (notably a `database` error when the original id was reused since
/// the delete — the popped entry is dropped, not re-pushed).
pub async fn undo_impl(state: &AppState) -> Result<Option<UndoOutcome>, CommandError> {
    let Some(deletion) = state.pop_undo() else {
        return Ok(None);
    };
    // Derive the label + kind before moving the snapshot into the restore closure.
    let kind = deletion_kind(&deletion).to_owned();
    let label = deletion_label(&deletion);
    state
        .with_store(move |store| store.restore_deletion(&deletion))
        .await?;
    Ok(Some(UndoOutcome {
        kind,
        label,
        remaining: state.undo_depth(),
    }))
}

/// IPC: undo the last destructive action.
///
/// # Errors
/// See [`undo_impl`].
#[tauri::command]
pub async fn undo(state: State<'_, AppState>) -> Result<Option<UndoOutcome>, CommandError> {
    undo_impl(state.inner()).await
}

/// The entity family of a deletion — the refresh key the frontend keys off.
fn deletion_kind(deletion: &Deletion) -> &'static str {
    match deletion {
        Deletion::Individual(_) => "person",
        Deletion::Family(_) => "family",
        Deletion::Event(_) => "event",
        Deletion::Name(_) => "name",
        Deletion::Child(_) => "child",
        Deletion::Source(_) => "source",
        Deletion::Citation(_) => "citation",
        Deletion::Media(_) => "media",
    }
}

/// A human label for the restored record (presentation marshalling, not domain
/// logic — the restore itself is entirely in the core).
fn deletion_label(deletion: &Deletion) -> String {
    match deletion {
        Deletion::Individual(d) => person_label(&d.individual),
        Deletion::Family(_) => "the family".to_owned(),
        Deletion::Event(d) => format!("the {} event", d.event.kind),
        Deletion::Name(_) => "the name".to_owned(),
        Deletion::Child(_) => "the child link".to_owned(),
        Deletion::Source(d) => d.source.title.clone(),
        Deletion::Citation(_) => "the citation".to_owned(),
        Deletion::Media(d) => d
            .media
            .caption
            .clone()
            .unwrap_or_else(|| "the photo".to_owned()),
    }
}

/// `"Given Surname"`, or the best available part, or a generic fallback.
fn person_label(individual: &Individual) -> String {
    match (&individual.given_name, &individual.surname) {
        (Some(given), Some(surname)) => format!("{given} {surname}"),
        (Some(given), None) => given.clone(),
        (None, Some(surname)) => surname.clone(),
        (None, None) => "this person".to_owned(),
    }
}
