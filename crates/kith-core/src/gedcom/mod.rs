//! GEDCOM 5.5.1 interoperability: a hand-rolled, dependency-free reader and writer
//! mapping the Kith model to and from the genealogy lingua franca.
//!
//! # The contract (the whole module obeys it)
//! - **All GEDCOM knowledge lives here** (date *formatting* in [`crate::date`]). No
//!   tag is parsed or emitted outside this module.
//! - **Determinism** — [`export`] for a fixed database is byte-identical every run:
//!   no `now()` (the `HEAD` carries no timestamp), no `HashMap` iterated into output
//!   (ascending-id `Vec`s drive emission), xrefs derived from row ids.
//! - **Round-trip over the parsed representation** — the model has no `date_original`;
//!   [`export`] regenerates a canonical GEDCOM date via
//!   [`GenealogicalDate::format_gedcom`](crate::date::GenealogicalDate::format_gedcom).
//! - **Atomicity** — [`import`] parses and validates fully, then writes in ONE
//!   transaction; a malformed file writes nothing.
//! - **Clear failure, never a panic** — malformed input is a
//!   [`CoreError::Validation`](crate::error::CoreError::Validation) carrying the
//!   offending line number; no `unwrap()`/`expect()` on a parse path.
//! - **Supported-field scope is explicit** — unsupported records are skipped
//!   and counted in [`ImportSummary::skipped_tags`], never silently dropped.

use serde::{Deserialize, Serialize};

use crate::db::Store;
use crate::error::Result;

mod lexer;
mod map;
mod tags;
mod tree;
mod writer;

/// Options for [`import`]. `#[non_exhaustive]` so later fields (an encoding option,
/// a reconciling merge strategy) stay additive; [`Default`] is a fresh (non-merge)
/// import. Build it with [`ImportOptions::default`] and set the fields you need.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportOptions {
    /// `true` appends into an existing database (fresh ids; no dedup); `false`
    /// (the default) requires an empty store (the shell creates the fresh DB).
    pub merge: bool,
}

/// What [`import`] wrote, for the caller's summary. `#[non_exhaustive]`; serde for
/// the CLI `--json` and the IPC wire. `skipped_tags` is a deterministic
/// (sorted) tag → count map of the records the importer defers, so a future "we now support
/// SOUR" change visibly moves a fixture from skipped to mapped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportSummary {
    /// Individuals (`INDI`) created.
    pub individuals: usize,
    /// Families (`FAM`) created.
    pub families: usize,
    /// Events created (individual + family).
    pub events: usize,
    /// Alternate names created.
    pub names: usize,
    /// Distinct places created (deduplicated by name).
    pub places: usize,
    /// Media objects (`OBJE`) created — top-level records plus inline references
    /// under `INDI`/`FAM`.
    pub media: usize,
    /// Sources (`SOUR`) created from top-level records.
    pub sources: usize,
    /// Citations created from `SOUR` pointers under `INDI`/`FAM`/events.
    pub citations: usize,
    /// Unsupported top-level records, by tag → count (e.g. `SUBM`).
    pub skipped_tags: std::collections::BTreeMap<String, usize>,
}

/// Serialize the whole database to a complete, valid, **deterministic** GEDCOM
/// 5.5.1 document (`0 HEAD … 0 TRLR`).
///
/// Living persons are **not** redacted: GEDCOM is a full-fidelity data move, and
/// round-trip losslessness requires it (the HTML exporter's privacy default is a
/// separate concern).
///
/// # Errors
/// [`CoreError`](crate::error::CoreError) if a `Store` read fails.
pub fn export(store: &Store) -> Result<String> {
    writer::write_document(store)
}

/// Parse `source` (an already-decoded UTF-8 GEDCOM 5.5.1 document), validate it
/// fully, then write it to `store` in one transaction per `options`.
///
/// # Errors
/// [`CoreError::Validation`](crate::error::CoreError::Validation) (with the
/// offending line number) for a malformed file, a declared `CHAR ANSEL`, a dangling
/// `@…@` xref, or a non-merge import into a non-empty store; another
/// [`CoreError`](crate::error::CoreError) if a write fails (the transaction rolls
/// back — nothing is written).
pub fn import(store: &Store, source: &str, options: &ImportOptions) -> Result<ImportSummary> {
    let lines = lexer::lex(source)?;
    let records = tree::build(&lines)?;
    map::check_encoding(&records)?; // reject a declared ANSEL/non-UTF-8
    map::validate(&records)?; // structural + xref-presence, no writes
    map::apply(store, &records, options)
}
