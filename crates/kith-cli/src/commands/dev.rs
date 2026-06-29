//! `kith dev seed` — seed a synthetic database for manual GUI profiling.
//!
//! Dev-only (built with `--features dev`): the noun does not exist in a release
//! build, and `kith_core::synth` is compiled out without the feature. All
//! generation logic lives in [`kith_core::synth::seed_synthetic`]; this is a thin
//! noun over it that opens (creating if needed) the `--db` target, seeds it, and
//! prints the stable focal id to root a chart on.

use anyhow::Context as _;
use kith_core::prelude::Store;
use kith_core::synth::seed_synthetic;

use crate::cli::{DevCommand, DevSeedArgs, GlobalArgs};
use crate::output;

/// Dispatches the `dev` subcommand against an open store.
///
/// # Errors
/// Propagates any seeding failure as an `anyhow::Error`.
pub fn run(global: &GlobalArgs, store: &Store, command: &DevCommand) -> anyhow::Result<()> {
    match command {
        DevCommand::Seed(args) => seed(global, store, args),
    }
}

/// Seeds about `args.individuals` synthetic people and reports the focal id.
fn seed(global: &GlobalArgs, store: &Store, args: &DevSeedArgs) -> anyhow::Result<()> {
    let focal = seed_synthetic(store, args.individuals, args.seed)
        .with_context(|| format!("seeding {} synthetic individuals", args.individuals))?;
    output::report_dev_seed(global, focal, args.individuals);
    Ok(())
}
