//! Event commands: add / get / update / delete.
//!
//! `add`/`update` take a request DTO carrying the date as a raw string,
//! parsed up front so a bad date fails as `validation` before any write. Place
//! resolution mirrors the CLI's `resolve_place` (existing `place_id` wins; else
//! `place_name` inserts a new place — no dedup; else none). The event
//! subject is immutable: `update` resolves it from the stored row.

use kith_core::prelude::{DeleteTarget, Event, EventId, NewEvent, NewPlace, PlaceId, Store};
use kith_core::query::EventView;
use tauri::State;

use crate::commands::date::parse_opt_date;
use crate::commands::dto::{NewEventRequest, UpdateEventRequest};
use crate::error::CommandError;
use crate::state::AppState;

/// Resolves the optional place for an add/update: an existing `place_id` wins;
/// else `place_name` inserts a new place; else `None`.
fn resolve_place(
    store: &Store,
    place_id: Option<i64>,
    place_name: Option<&str>,
) -> kith_core::prelude::Result<Option<PlaceId>> {
    if let Some(id) = place_id {
        return Ok(Some(PlaceId::new(id)));
    }
    match place_name {
        Some(name) => {
            let id = store.create_place(&NewPlace {
                name: name.to_owned(),
                latitude: None,
                longitude: None,
                parent: None,
            })?;
            Ok(Some(id))
        }
        None => Ok(None),
    }
}

/// Adds an event against its subject.
///
/// # Errors
/// [`CommandError`] with `kind: validation` if the date is malformed; otherwise
/// the mapped store error (e.g. an FK violation on a non-existent subject).
pub async fn event_add_impl(state: &AppState, req: NewEventRequest) -> Result<Event, CommandError> {
    let date = parse_opt_date(req.date.as_deref())?;
    let NewEventRequest {
        subject,
        kind,
        place_id,
        place_name,
        notes,
        ..
    } = req;
    state
        .with_store(move |store| {
            let place = resolve_place(&store, place_id, place_name.as_deref())?;
            store.add_event(&NewEvent {
                subject,
                kind,
                date,
                place,
                notes,
            })
        })
        .await
}

/// Loads an event with its place resolved.
///
/// # Errors
/// [`CommandError`] (`not_found` / `database` / `io`).
pub async fn event_get_impl(state: &AppState, id: EventId) -> Result<EventView, CommandError> {
    state
        .with_store(move |store| EventView::load(&store, id))
        .await
}

/// Updates an event's editable fields (subject immutable, resolved from the
/// stored row). A resolved place overlays the existing one; kind/date/notes
/// replace.
///
/// # Errors
/// [`CommandError`] with `kind: validation` if the date is malformed; `not_found`
/// if the event is gone.
pub async fn event_update_impl(
    state: &AppState,
    req: UpdateEventRequest,
) -> Result<Event, CommandError> {
    let date = parse_opt_date(req.date.as_deref())?;
    let UpdateEventRequest {
        id,
        kind,
        place_id,
        place_name,
        notes,
        ..
    } = req;
    let id = EventId::new(id);
    state
        .with_store(move |store| {
            let mut event = store.get_event(id)?; // preserves the immutable subject
            event.kind = kind;
            event.date = date;
            if let Some(place) = resolve_place(&store, place_id, place_name.as_deref())? {
                event.place = Some(place);
            }
            event.notes = notes;
            store.update_event(&event)?;
            Ok(event)
        })
        .await
}

/// Deletes an event (cascades its citations + media links), snapshotting the
/// cascade onto the session undo stack first.
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if the row is gone.
pub async fn event_delete_impl(state: &AppState, id: EventId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Event(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// IPC: add an event.
///
/// # Errors
/// See [`event_add_impl`].
#[tauri::command]
pub async fn event_add(
    state: State<'_, AppState>,
    request: NewEventRequest,
) -> Result<Event, CommandError> {
    event_add_impl(state.inner(), request).await
}

/// IPC: load an event view by id.
///
/// # Errors
/// See [`event_get_impl`].
#[tauri::command]
pub async fn event_get(state: State<'_, AppState>, id: i64) -> Result<EventView, CommandError> {
    event_get_impl(state.inner(), EventId::new(id)).await
}

/// IPC: update an event.
///
/// # Errors
/// See [`event_update_impl`].
#[tauri::command]
pub async fn event_update(
    state: State<'_, AppState>,
    request: UpdateEventRequest,
) -> Result<Event, CommandError> {
    event_update_impl(state.inner(), request).await
}

/// IPC: delete an event by id.
///
/// # Errors
/// See [`event_delete_impl`].
#[tauri::command]
pub async fn event_delete(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    event_delete_impl(state.inner(), EventId::new(id)).await
}
