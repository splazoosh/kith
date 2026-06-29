//! `kith db backup|restore|vacuum` — thin wrappers over the `kith-core::db`
//! maintenance ops. `backup`/`vacuum` require an existing database;
//! `restore` creates or replaces one, so — unlike every other command — it
//! resolves the path but does **not** open the target first. The `--force`
//! overwrite policy lives here; the WAL-safe mechanism lives in the core.

use std::path::Path;

use anyhow::Context as _;
use kith_core::prelude::{CoreError, Store};

use crate::cli::{DbBackupArgs, DbCommand, DbRestoreArgs, GlobalArgs};
use crate::{context, output};

/// Dispatches the `db` subcommand. Owns whether to open the store:
/// `backup`/`vacuum` open the existing database, but `restore` must **not**
/// open the target it is about to replace, so this resolves the path and lets
/// each handler decide. The asymmetry is deliberate and lives only here.
///
/// # Errors
/// Propagates path resolution, store open, and core-maintenance failures.
pub fn run(global: &GlobalArgs, db_path: &Path, command: &DbCommand) -> anyhow::Result<()> {
    match command {
        DbCommand::Backup(args) => backup(global, db_path, args),
        DbCommand::Vacuum => vacuum(global, db_path),
        DbCommand::Restore(args) => restore(global, db_path, args),
    }
}

fn backup(global: &GlobalArgs, db_path: &Path, args: &DbBackupArgs) -> anyhow::Result<()> {
    let store = context::open_existing(db_path)?;
    guard_overwrite(&args.file, args.force, "backup destination")?;
    if args.file.exists() {
        // force == true (guard passed); clear so core `backup` (which refuses an
        // existing dest) can write the fresh snapshot.
        std::fs::remove_file(&args.file)
            .with_context(|| format!("removing existing backup {}", args.file.display()))?;
    }
    store
        .backup(&args.file)
        .with_context(|| format!("backing up to {}", args.file.display()))?;
    output::report_db_action(global, "backup", &args.file);
    Ok(())
}

fn vacuum(global: &GlobalArgs, db_path: &Path) -> anyhow::Result<()> {
    let store = context::open_existing(db_path)?;
    store.vacuum().context("vacuuming database")?;
    output::report_db_action(global, "vacuum", db_path);
    Ok(())
}

fn restore(global: &GlobalArgs, db_path: &Path, args: &DbRestoreArgs) -> anyhow::Result<()> {
    guard_overwrite(db_path, args.force, "target database")?;
    context::ensure_parent_dir(db_path)?; // restore may create the target afresh
    Store::restore(&args.file, db_path)
        .with_context(|| format!("restoring {} to {}", args.file.display(), db_path.display()))?;
    output::report_db_action(global, "restore", db_path);
    Ok(())
}

/// Refuses to clobber an existing `path` unless `force`. Surfaced as
/// `CoreError::Validation` so the existing exit mapping returns code `4`. Shared
/// with `export html` — one overwrite policy, one behaviour.
pub(crate) fn guard_overwrite(path: &Path, force: bool, what: &str) -> anyhow::Result<()> {
    if path.exists() && !force {
        return Err(CoreError::Validation(format!(
            "{what} already exists: {} (use --force to overwrite)",
            path.display()
        ))
        .into());
    }
    Ok(())
}
