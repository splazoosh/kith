//! "LB" interchange: import a flat JSON array of person records — parent and
//! spouse relationships expressed as id pointers — into the Kith model.
//!
//! # The contract (the whole module obeys it)
//! - **All LB knowledge lives here.** Date *parsing* reuses [`crate::date`]'s
//!   types; nothing else understands the LB shape.
//! - **Families are synthesized.** LB has no family records: each distinct
//!   `(FatherId, MotherId)` pair becomes one family with its children linked, and
//!   each spouse pointer a couple family (deduped against the parent families).
//!   The redundant `Children` array (a back-reference to these same links) is
//!   ignored — parentage is taken from each child's `FatherId`/`MotherId`.
//! - **The unknown-date sentinel is dropped.** `01.01.1753` (SQL Server's
//!   `datetime` minimum — the source's "unset" default) maps to *no date*, so an
//!   import does not invent a flood of false 1753 births. A birthplace with no
//!   real date still yields a place-only birth event.
//! - **Atomicity** — parse + validate fully, then write in ONE transaction; a
//!   malformed file writes nothing.
//! - **Clear failure, never a panic** — malformed JSON, a zero/duplicate id, or a
//!   dangling pointer is a [`CoreError::Validation`]; no `unwrap()` on a parse path.
//!
//! It shares [`ImportOptions`](crate::gedcom::ImportOptions) and
//! [`ImportSummary`](crate::gedcom::ImportSummary) with [`crate::gedcom`] — the
//! common import vocabulary. The `names`/`media`/`sources`/`citations`/
//! `skipped_tags` fields stay zero/empty: LB carries none of those.

use crate::db::Store;
use crate::error::{CoreError, Result};
use crate::gedcom::{ImportOptions, ImportSummary};

mod map;
mod record;

use record::LbPerson;

/// Parse `source` (a UTF-8 LB document — a top-level JSON array of person
/// records), validate it fully, then write it to `store` in one transaction per
/// `options`.
///
/// Relationships are reconstructed into Kith's family model: children sharing a
/// `(FatherId, MotherId)` pair join one family; spouse pointers form couple
/// families (deduped). The `01.01.1753` unknown-date sentinel becomes no date.
///
/// # Errors
/// [`CoreError::Validation`] for malformed JSON, a zero/duplicate record id, a
/// dangling parent/spouse pointer, or a non-merge import into a non-empty store;
/// another [`CoreError`] if a write fails (the transaction rolls back — nothing
/// is written).
pub fn import(store: &Store, source: &str, options: &ImportOptions) -> Result<ImportSummary> {
    let people: Vec<LbPerson> = serde_json::from_str(source)
        .map_err(|e| CoreError::Validation(format!("invalid LB JSON: {e}")))?;
    map::validate(&people)?;
    map::apply(store, &people, options)
}
