//! `kith` CLI library: the command tree, DB-context resolution, output
//! rendering, and exit-code mapping over [`kith_core`]. The `kith` binary is a
//! thin shell over [`run`].
//!
//! `kith-core` owns all logic; this crate only parses arguments, calls the
//! [`Store`](kith_core::prelude::Store), and renders results. No layout, query,
//! or domain logic lives here.

mod cli;
mod commands;
mod context;
mod exit;
mod output;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

/// Parses arguments and runs the requested command, returning the process
/// exit code. clap owns usage errors (it exits with code `2` before this
/// returns); every other outcome is mapped here via [`exit::code_for`].
#[must_use]
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            output::render_error(&err, &cli.global);
            exit::code_for(&err)
        }
    }
}

/// Routes a parsed [`Cli`] to its command handler. `init` is the sole database
/// creator; every other command opens an existing database first.
fn dispatch(cli: &Cli) -> anyhow::Result<()> {
    let global = &cli.global;
    match &cli.command {
        Command::Init => commands::init::run(global),
        Command::Person { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::person::run(global, &store, command)
        }
        Command::Name { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::name::run(global, &store, command)
        }
        Command::Family { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::family::run(global, &store, command)
        }
        Command::Event { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::event::run(global, &store, command)
        }
        Command::Query { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::query::run(global, &store, command)
        }
        Command::Db { command } => {
            let path = context::resolve_db_path(global)?;
            commands::db::run(global, &path, command)
        }
        Command::Export { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::export::run(global, &store, command)
        }
        Command::Import { command } => {
            // Like `db`, the handler decides the store: the default path may *create*
            // the target, so we resolve the path but do not open it here.
            let path = context::resolve_db_path(global)?;
            commands::import::run(global, &path, command)
        }
        Command::Media { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::media::run(global, &store, command)
        }
        Command::Source { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::source::run_source(global, &store, command)
        }
        Command::Citation { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::source::run_citation(global, &store, command)
        }
        Command::Search(args) => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_existing(&path)?;
            commands::search::run(global, &store, args)
        }
        // `dev seed` may *create* its target (like `init`), so it opens-or-creates
        // rather than requiring an existing DB. Dev-only — absent without `--features dev`.
        #[cfg(feature = "dev")]
        Command::Dev { command } => {
            let path = context::resolve_db_path(global)?;
            let store = context::open_or_create(&path)?;
            commands::dev::run(global, &store, command)
        }
    }
}
