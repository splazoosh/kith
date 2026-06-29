//! The `about_info` command: surface the app's identity to the UI.
//!
//! The About/Help modal needs the product name, version, license, repository, and
//! bundle identifier in one shot. The compile-time fields come from Cargo's
//! `CARGO_PKG_*` env vars (populated from `[workspace.package]`), and the display
//! name + identifier come from the runtime [`tauri::Config`]. It is **app-defined**,
//! so Tauri v2 auto-allows it — no capability entry — and **infallible** (it reads
//! constants + config, never a fallible path), so it returns the value directly.

use serde::Serialize;
use tauri::AppHandle;

/// The app's identity, marshalled to the About modal. Plain snake_case fields,
/// matching the other command DTOs' wire convention (cf. `DbInfo.schema_version`).
#[derive(Debug, Clone, Serialize)]
pub struct AboutInfo {
    /// The product name (`tauri.conf.json` `productName`, e.g. `"Kith"`).
    pub name: String,
    /// The semver version — the one source of truth (`[workspace.package] version`).
    pub version: String,
    /// The bundle identifier (`net.splazoosh.kith`).
    pub identifier: String,
    /// The SPDX license id (`"MIT"`).
    pub license: String,
    /// The repository URL, if set (shown as selectable text — no external-open ACL).
    pub repository: String,
    /// The author line (`CARGO_PKG_AUTHORS`).
    pub authors: String,
}

impl AboutInfo {
    /// Builds the info from the runtime-supplied display `name` + `identifier`,
    /// filling the rest from compile-time crate metadata. Split out from the
    /// command so the env-driven fields are unit-testable without an `AppHandle`
    /// (which is runtime-only).
    #[must_use]
    pub fn from_metadata(name: String, identifier: String) -> Self {
        Self {
            name,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            identifier,
            license: env!("CARGO_PKG_LICENSE").to_owned(),
            repository: env!("CARGO_PKG_REPOSITORY").to_owned(),
            authors: env!("CARGO_PKG_AUTHORS").to_owned(),
        }
    }
}

/// IPC: the app's identity for the About/Help modal.
///
/// Infallible — reads the bundle config + compile-time crate metadata, with no
/// database or filesystem access (and so no `# Errors`).
#[tauri::command]
#[must_use]
pub fn about_info(app: AppHandle) -> AboutInfo {
    let config = app.config();
    let name = config
        .product_name
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_NAME").to_owned());
    AboutInfo::from_metadata(name, config.identifier.clone())
}

#[cfg(test)]
mod tests {
    use super::AboutInfo;

    #[test]
    fn about_info_carries_the_release_metadata() {
        // The runtime supplies the display name + identifier; everything else is
        // read from the populated CARGO_PKG_* env vars (the footgun this guards).
        let info = AboutInfo::from_metadata("Kith".to_owned(), "net.splazoosh.kith".to_owned());

        assert_eq!(info.name, "Kith");
        assert_eq!(info.identifier, "net.splazoosh.kith");
        // One source of truth: the workspace version flows here verbatim.
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.license, "MIT");
        assert!(!info.authors.is_empty(), "authors should be populated");
    }
}
