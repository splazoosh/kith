//! `kith query ancestors | descendants` — exercise the relationship walks.
//!
//! A thin noun over [`kith_core::query::ancestors`] / [`descendants`]: it surfaces
//! the unpositioned `RelativeGraph` as `--json` (round-trippable) or an indented
//! human tree. **No layout/positions are surfaced** — those have no consumer
//! yet.

use anyhow::Context as _;
use kith_core::prelude::{PersonId, Store, ancestors, descendants};

use crate::cli::{GlobalArgs, QueryCommand};
use crate::output;

/// Dispatches the `query` subcommand against an open store.
///
/// # Errors
/// Propagates any walk failure as an `anyhow::Error`: a missing id is
/// `NotFound` → exit 3; an out-of-range `--generations` is `Validation` → 4.
pub fn run(global: &GlobalArgs, store: &Store, command: &QueryCommand) -> anyhow::Result<()> {
    let graph = match command {
        QueryCommand::Ancestors(args) => ancestors(store, PersonId::new(args.id), args.generations)
            .with_context(|| format!("walking ancestors of {}", args.id))?,
        QueryCommand::Descendants(args) => {
            descendants(store, PersonId::new(args.id), args.generations)
                .with_context(|| format!("walking descendants of {}", args.id))?
        }
    };
    output::render_relative_graph(global, &graph);
    Ok(())
}
