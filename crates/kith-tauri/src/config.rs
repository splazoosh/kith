//! The tiny app-config file that records which database to reopen at startup.
//!
//! SQLite persists the *data*; this persists *which file*. It lives at
//! `<app_config_dir>/kith.json`, where `app_config_dir` comes from the Tauri
//! path API (`app.path().app_config_dir()`, keyed off the `net.splazoosh.kith`
//! identifier) — not the `directories` crate the CLI uses. A missing or corrupt
//! file is non-fatal: the app simply starts with no database open.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The persisted configuration. Additive by design — later keys (window
/// geometry, theme) extend it without breaking older files.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// The last database opened/created, reopened best-effort at startup.
    pub last_db_path: Option<PathBuf>,
}

/// The config file path inside `config_dir`.
fn config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("kith.json")
}

/// Loads the config from `config_dir`, returning the default if the file is
/// missing or unreadable (a corrupt config is not fatal — start fresh).
#[must_use]
pub fn load(config_dir: &Path) -> AppConfig {
    std::fs::read(config_path(config_dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes `cfg` to `config_dir`, creating the directory if needed.
///
/// # Errors
/// Returns [`std::io::Error`] if the directory cannot be created or the file
/// cannot be written/serialized.
pub fn save(config_dir: &Path, cfg: &AppConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let bytes = serde_json::to_vec_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(config_path(config_dir), bytes)
}
