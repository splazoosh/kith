//! `kith init` — create the database and run migrations (the sole DB creator).

use crate::cli::GlobalArgs;
use crate::{context, output};

/// Resolves the path, creates the file + parent dir, migrates, and reports the
/// path and schema version.
///
/// # Errors
/// Propagates path-resolution, directory-creation, and `Store::open` failures.
pub fn run(global: &GlobalArgs) -> anyhow::Result<()> {
    let path = context::resolve_db_path(global)?;
    let store = context::open_or_create(&path)?;
    let version = store.schema_version()?;
    output::report_init(global, &path, version);
    Ok(())
}
