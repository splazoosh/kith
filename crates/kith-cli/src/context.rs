//! Database-context resolution: turn `--db` (or the default per-user path) into
//! an open [`Store`]. `init` creates; everything else requires an existing file
//! (never silently spawn a database from a mistyped `--db`).

use std::io;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use directories::ProjectDirs;
use kith_core::prelude::{CoreError, Store};

use crate::cli::GlobalArgs;

/// Resolves the database path: explicit `--db`, else the per-user data dir.
///
/// The qualifier/organization/application are a placeholder pending the
/// final bundle id.
pub fn resolve_db_path(global: &GlobalArgs) -> anyhow::Result<PathBuf> {
    if let Some(path) = &global.db {
        return Ok(path.clone());
    }
    let dirs = ProjectDirs::from("net", "Splazoosh", "Kith")
        .context("could not determine the per-user data directory")?;
    Ok(dirs.data_dir().join("kith.db"))
}

/// Opens an **existing** database for a non-`init` command. Errors (as
/// [`CoreError::Io`] → exit `5`) if the file is absent, rather than letting
/// `Store::open` create an empty one.
pub fn open_existing(path: &Path) -> anyhow::Result<Store> {
    if !path.exists() {
        let msg = format!("no database at {}; run `kith init`", path.display());
        return Err(CoreError::Io(io::Error::new(io::ErrorKind::NotFound, msg)).into());
    }
    Store::open(path).with_context(|| format!("opening database at {}", path.display()))
}

/// Ensures `path`'s parent directory exists (creating it), so a database file
/// can be created there. A bare filename (no parent) is a no-op.
///
/// # Errors
/// [`anyhow::Error`] if the directory cannot be created.
pub fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating data directory {}", parent.display()))?;
        }
    }
    Ok(())
}

/// Opens (creating if needed) the database for `init`, ensuring the parent
/// directory exists first (`Store::open` requires it).
pub fn open_or_create(path: &Path) -> anyhow::Result<Store> {
    ensure_parent_dir(path)?;
    Store::open(path).with_context(|| format!("creating database at {}", path.display()))
}
