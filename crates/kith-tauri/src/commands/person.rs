//! Person commands: list / get / create / update / delete + search.
//!
//! `create` replicates the CLI's optional birth/death convenience: the
//! dates ride as raw strings and are parsed up front so a bad date fails
//! as `validation` before any write. `update` takes the full edited record (the
//! GUI form sends the whole `Individual`); the immutable id rides inside it.

use kith_core::prelude::{
    DeleteTarget, EventKind, EventSubject, Individual, NewEvent, NewIndividual, PersonId, SearchHit,
};
use kith_core::query::PersonView;
use tauri::State;

use crate::commands::date::parse_opt_date;
use crate::error::CommandError;
use crate::state::AppState;

/// Lists every individual (the Library list).
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn person_list_impl(state: &AppState) -> Result<Vec<Individual>, CommandError> {
    state.with_store(|store| store.list_individuals()).await
}

/// Loads a person with their related rows (names, events, family memberships).
///
/// # Errors
/// [`CommandError`] (`not_found` / `database` / `io`).
pub async fn person_get_impl(state: &AppState, id: PersonId) -> Result<PersonView, CommandError> {
    state
        .with_store(move |store| PersonView::load(&store, id))
        .await
}

/// Creates a person, optionally adding birth/death events from raw date strings.
///
/// # Errors
/// [`CommandError`] with `kind: validation` if a date is malformed; otherwise
/// the mapped store error.
pub async fn person_create_impl(
    state: &AppState,
    draft: NewIndividual,
    birth: Option<String>,
    death: Option<String>,
) -> Result<Individual, CommandError> {
    // Parse up front (core logic) so a bad date fails before any write.
    let birth = parse_opt_date(birth.as_deref())?;
    let death = parse_opt_date(death.as_deref())?;
    state
        .with_store(move |store| {
            let person = store.create_individual(&draft)?;
            for (kind, date) in [(EventKind::Birth, birth), (EventKind::Death, death)] {
                if let Some(date) = date {
                    store.add_event(&NewEvent {
                        subject: EventSubject::Individual(person.id),
                        kind,
                        date: Some(date),
                        place: None,
                        notes: None,
                    })?;
                }
            }
            Ok(person)
        })
        .await
}

/// Updates a person from the full edited record (id immutable).
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if the row is gone; otherwise the
/// mapped store error.
pub async fn person_update_impl(
    state: &AppState,
    record: Individual,
) -> Result<Individual, CommandError> {
    state
        .with_store(move |store| {
            store.update_individual(&record)?;
            Ok(record)
        })
        .await
}

/// Deletes a person (cascades names/memberships/individual events; partner refs
/// null), snapshotting the cascade onto the session undo stack first. The UI
/// confirms before calling.
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if the row is gone.
pub async fn person_delete_impl(state: &AppState, id: PersonId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Individual(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// Ranked, multi-field full-text search (names / alternate names / nickname /
/// notes / event places), capped at `limit` hits. All matching/ranking logic
/// lives in [`Store::search`](kith_core::prelude::Store::search) (`db/search.rs`)
/// — this is a thin marshal over it. An empty query lists everyone (bounded).
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn search_impl(
    state: &AppState,
    query: String,
    limit: usize,
) -> Result<Vec<SearchHit>, CommandError> {
    state
        .with_store(move |store| store.search(&query, limit))
        .await
}

/// IPC: list individuals.
///
/// # Errors
/// See [`person_list_impl`].
#[tauri::command]
pub async fn person_list(state: State<'_, AppState>) -> Result<Vec<Individual>, CommandError> {
    person_list_impl(state.inner()).await
}

/// IPC: load a person view by id.
///
/// # Errors
/// See [`person_get_impl`].
#[tauri::command]
pub async fn person_get(state: State<'_, AppState>, id: i64) -> Result<PersonView, CommandError> {
    person_get_impl(state.inner(), PersonId::new(id)).await
}

/// IPC: create a person (+ optional birth/death dates as raw strings).
///
/// # Errors
/// See [`person_create_impl`].
#[tauri::command]
pub async fn person_create(
    state: State<'_, AppState>,
    draft: NewIndividual,
    birth: Option<String>,
    death: Option<String>,
) -> Result<Individual, CommandError> {
    person_create_impl(state.inner(), draft, birth, death).await
}

/// IPC: update a person from the full record.
///
/// # Errors
/// See [`person_update_impl`].
#[tauri::command]
pub async fn person_update(
    state: State<'_, AppState>,
    record: Individual,
) -> Result<Individual, CommandError> {
    person_update_impl(state.inner(), record).await
}

/// IPC: delete a person by id.
///
/// # Errors
/// See [`person_delete_impl`].
#[tauri::command]
pub async fn person_delete(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    person_delete_impl(state.inner(), PersonId::new(id)).await
}

/// IPC: ranked full-text search. The frontend always supplies `limit`; a sane
/// cap is used if it is somehow absent.
///
/// # Errors
/// See [`search_impl`].
#[tauri::command]
pub async fn search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchHit>, CommandError> {
    search_impl(state.inner(), query, limit.unwrap_or(50)).await
}
