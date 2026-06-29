//! `kith event add | show | edit | rm` — events on an individual or a family.
//!
//! Inputs are validated **before** any write: the subject format, date,
//! and kind are parsed at clap time, so a malformed value never
//! reaches a `Store` call. `--place "<name>"` inserts a new place each time (no
//! dedup); `--place-id` references an existing one. The only residual
//! non-atomicity is a `create_place` that succeeds before an `add_event` that
//! fails on a nonexistent subject (a foreign-key error → exit `6`). No new
//! core method is needed — every call is an existing, tested `Store` op.

use anyhow::Context as _;
use kith_core::prelude::{EventId, NewEvent, NewPlace, PlaceId, Store};

use crate::cli::{
    EventAddArgs, EventCommand, EventEditArgs, EventRmArgs, EventShowArgs, GlobalArgs,
};
use crate::output;
use kith_core::query::EventView;

/// Dispatches the `event` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error`.
pub fn run(global: &GlobalArgs, store: &Store, command: &EventCommand) -> anyhow::Result<()> {
    match command {
        EventCommand::Add(args) => add(global, store, args),
        EventCommand::Show(args) => show(global, store, args),
        EventCommand::Edit(args) => edit(global, store, args),
        EventCommand::Rm(args) => rm(global, store, args),
    }
}

fn add(global: &GlobalArgs, store: &Store, args: &EventAddArgs) -> anyhow::Result<()> {
    let place = resolve_place(store, args.place.as_deref(), args.place_id)?;
    let draft = NewEvent {
        subject: args.subject,
        kind: args.kind.clone(),
        date: args.date,
        place,
        notes: args.notes.clone(),
    };
    let event = store.add_event(&draft).context("adding event")?;
    output::report_record(global, "Added", &event, &format!("event {}", event.id));
    Ok(())
}

fn show(global: &GlobalArgs, store: &Store, args: &EventShowArgs) -> anyhow::Result<()> {
    let view = EventView::load(store, EventId::new(args.id))
        .with_context(|| format!("loading event {}", args.id))?;
    output::render_event_view(global, &view);
    Ok(())
}

fn edit(global: &GlobalArgs, store: &Store, args: &EventEditArgs) -> anyhow::Result<()> {
    let id = EventId::new(args.id);
    let mut event = store
        .get_event(id)
        .with_context(|| format!("loading event {id} to edit"))?;

    // Overlay only the flags that were provided; the subject stays immutable.
    if let Some(kind) = &args.kind {
        event.kind = kind.clone();
    }
    if let Some(date) = args.date {
        event.date = Some(date);
    }
    if let Some(place) = resolve_place(store, args.place.as_deref(), args.place_id)? {
        event.place = Some(place);
    }
    if let Some(notes) = &args.notes {
        event.notes = Some(notes.clone());
    }

    store
        .update_event(&event)
        .with_context(|| format!("updating event {id}"))?;
    output::report_record(global, "Updated", &event, &format!("event {id}"));
    Ok(())
}

fn rm(global: &GlobalArgs, store: &Store, args: &EventRmArgs) -> anyhow::Result<()> {
    let id = EventId::new(args.id);
    store
        .delete_event(id)
        .with_context(|| format!("removing event {id}"))?;
    output::report_removed(global, "event", id.get());
    Ok(())
}

/// Resolves the optional place for an `add`/`edit`: an existing `--place-id`
/// wins; otherwise `--place "<name>"` inserts a new place (no dedup);
/// neither given yields `None`. clap forbids passing both (exit `2`).
fn resolve_place(
    store: &Store,
    place: Option<&str>,
    place_id: Option<i64>,
) -> anyhow::Result<Option<PlaceId>> {
    if let Some(id) = place_id {
        return Ok(Some(PlaceId::new(id)));
    }
    match place {
        Some(name) => {
            let draft = NewPlace {
                name: name.to_owned(),
                latitude: None,
                longitude: None,
                parent: None,
            };
            let id = store
                .create_place(&draft)
                .with_context(|| format!("creating place {name:?}"))?;
            Ok(Some(id))
        }
        None => Ok(None),
    }
}
