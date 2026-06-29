//! Input-shaping request DTOs for the date-bearing commands.
//!
//! `GenealogicalDate` is a structured enum the frontend must **not** build or
//! parse (no date math in the frontend, `CLAUDE.md`). So event commands accept a
//! thin request carrying the date as a raw `String`; the command parses it via
//! the core date subsystem (`GenealogicalDate::from_str`) before constructing
//! the core draft, always preserving the raw string. Every
//! *date-free* draft (`NewFamily`, `NewName`) and **all** record outputs
//! (de)serialize the core types directly — these DTOs exist only where a raw
//! date must cross the boundary.

use kith_core::prelude::{EventKind, EventSubject};
use serde::Deserialize;

/// The payload for `event_add`: a [`NewEvent`](kith_core::prelude::NewEvent)
/// whose date rides as a raw string and whose place is given by id **or** new
/// name (existing `place_id` wins; else `place_name` inserts one; else none).
#[derive(Debug, Clone, Deserialize)]
pub struct NewEventRequest {
    /// The individual or family the event belongs to.
    pub subject: EventSubject,
    /// The kind of event (open enum; unknown codes preserved verbatim).
    pub kind: EventKind,
    /// The raw date string, parsed by the command; `None` if undated.
    pub date: Option<String>,
    /// An existing place id (wins over `place_name`).
    pub place_id: Option<i64>,
    /// A new place name to insert when no `place_id` is given (no dedup).
    pub place_name: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// The payload for `event_update`: the immutable subject is resolved from the
/// stored event, so only the editable fields ride here (date again as a raw
/// string). A resolved place overlays the existing one; `kind`/`date`/`notes`
/// replace.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEventRequest {
    /// The event to update.
    pub id: i64,
    /// The new kind.
    pub kind: EventKind,
    /// The new raw date string; `None` clears the date.
    pub date: Option<String>,
    /// An existing place id to set (wins over `place_name`).
    pub place_id: Option<i64>,
    /// A new place name to insert and set when no `place_id` is given.
    pub place_name: Option<String>,
    /// Free-form notes (replaces the stored value).
    pub notes: Option<String>,
}
