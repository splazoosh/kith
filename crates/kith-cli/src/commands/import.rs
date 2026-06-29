//! `kith import gedcom` — read a GEDCOM 5.5.1 file into the database.
//!
//! A thin noun over [`kith_core::gedcom::import`]. **No GEDCOM logic lives here** —
//! parsing, validation, the two-pass xref mapping, and the one-transaction atomicity
//! all live in `kith_core::gedcom`. This handler only decodes the file to
//! a `&str` (the UTF-8 boundary), resolves the target store (fresh vs `--merge`),
//! calls `import`, and reports the [`ImportSummary`](kith_core::prelude::ImportSummary).

use std::path::Path;

use anyhow::Context as _;
use kith_core::prelude::{CoreError, ImportOptions};

use crate::cli::{GlobalArgs, ImportCommand, ImportGedcomArgs};
use crate::{context, output};

/// Dispatches the `import` subcommand. Like `db`, it owns its store resolution
/// (the default path may **create** the target — the one noun besides `init`/`db
/// restore` that does), so it takes the resolved path, not an open store.
///
/// # Errors
/// Propagates the file read (`Io` → 5), the UTF-8 decode (`Validation` → 4), the
/// store open/create, and the import (`Validation` → 4 on a malformed file or a
/// non-merge import into a non-empty DB; rolls back, writing nothing).
pub fn run(global: &GlobalArgs, db_path: &Path, command: &ImportCommand) -> anyhow::Result<()> {
    match command {
        ImportCommand::Gedcom(args) => gedcom(global, db_path, args),
    }
}

/// Reads and decodes a GEDCOM file, resolves the target store, and imports it.
fn gedcom(global: &GlobalArgs, db_path: &Path, args: &ImportGedcomArgs) -> anyhow::Result<()> {
    // 1. Read bytes (missing/unreadable → Io → 5), then decode UTF-8. A
    //    non-UTF-8 file is a *validation* failure the user can fix (→ 4), not an Io
    //    one — `read_to_string` would conflate the two.
    let bytes = std::fs::read(&args.file)
        .map_err(CoreError::Io)
        .with_context(|| format!("reading {}", args.file.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| {
        CoreError::Validation(format!(
            "{} is not valid UTF-8 — ANSEL/UTF-16 GEDCOM is unsupported; re-export as UTF-8",
            args.file.display()
        ))
    })?;

    // 2. Resolve the store: --merge appends to the existing DB; the default
    //    creates + migrates a fresh target (the engine refuses a non-empty one).
    let store = if args.merge {
        context::open_existing(db_path)?
    } else {
        context::open_or_create(db_path)?
    };

    // 3. One core call — parse + validate + write in one transaction (atomic).
    //    `ImportOptions` is `#[non_exhaustive]`: build from `default()`, then set fields.
    let mut options = ImportOptions::default();
    options.merge = args.merge;
    let summary = kith_core::gedcom::import(&store, &text, &options)
        .with_context(|| format!("importing {}", args.file.display()))?;

    output::report_import(global, &summary);
    Ok(())
}
