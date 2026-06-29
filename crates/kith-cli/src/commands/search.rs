//! `kith search <query> [--limit N]` — ranked, multi-field people search.
//!
//! All matching/ranking/sanitization lives in `kith_core::db::Store::search`;
//! this is a thin noun over it. A no-match is success with an empty table
//! (exit 0) — search is a read, not a lookup, so "nothing found" is not an error.

use anyhow::Context as _;
use kith_core::prelude::Store;

use crate::cli::{GlobalArgs, SearchArgs};
use crate::output;

/// Runs `search` against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error`.
pub fn run(global: &GlobalArgs, store: &Store, args: &SearchArgs) -> anyhow::Result<()> {
    let hits = store
        .search(&args.query, args.limit)
        .with_context(|| format!("searching for {:?}", args.query))?;
    output::render_search_hits(global, &hits);
    Ok(())
}
