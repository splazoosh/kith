//! The serializable IPC error contract. [`CoreError`] and `anyhow` are not
//! `Serialize`; every Tauri command returns `Result<T, CommandError>` so the
//! frontend can narrow on [`kind`](CommandError::kind) and show an actionable
//! message instead of crashing.
//!
//! The buckets mirror the CLI's shipped `output.rs::error_kind` table (the
//! exit-code contract) — the only deliberate divergence is the fallback name
//! (`unexpected` here vs. the CLI's `error`). The
//! single [`From<CoreError>`] impl is the only mapping point (`err-from-impl`).

use kith_core::prelude::CoreError;
use serde::Serialize;

/// The category of an IPC failure. Serializes as a `snake_case` string
/// (`"not_found"`, `"validation"`, …) the frontend matches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// A requested record does not exist.
    NotFound,
    /// A value failed domain validation (incl. the full-slot partner guard and
    /// malformed dates).
    Validation,
    /// An I/O failure, plus the synthesized "no database open" / "file missing".
    Io,
    /// A SQLite / pool / migration failure.
    Database,
    /// Anything not matched above (a future `CoreError` variant or a background
    /// task panic) — never a user error.
    Unexpected,
}

/// A serializable error returned to the frontend across the IPC boundary.
///
/// Serializes as `{ "kind": "not_found", "message": "…" }`.
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    /// The error category the frontend narrows on.
    pub kind: ErrorKind,
    /// A human-readable, lowercase-leading message (no trailing punctuation).
    pub message: String,
}

impl CommandError {
    /// No database is open — surfaced as `io` so the UI prompts open/create.
    #[must_use]
    pub fn no_database() -> Self {
        Self {
            kind: ErrorKind::Io,
            message: "no database is open; create or open one first".to_owned(),
        }
    }

    /// An I/O failure (a missing file, a config-dir write, …).
    pub fn io(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Io,
            message: message.into(),
        }
    }

    /// A `spawn_blocking` join failure (the worker panicked) — never a user error.
    pub fn unexpected(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Unexpected,
            message: message.into(),
        }
    }
}

impl From<CoreError> for CommandError {
    fn from(err: CoreError) -> Self {
        let kind = match &err {
            CoreError::NotFound { .. } => ErrorKind::NotFound,
            CoreError::Validation(_) => ErrorKind::Validation,
            CoreError::Io(_) => ErrorKind::Io,
            CoreError::Database(_) | CoreError::Pool(_) | CoreError::Migration(_) => {
                ErrorKind::Database
            }
            // `CoreError` is `#[non_exhaustive]`; map any future variant safely.
            _ => ErrorKind::Unexpected,
        };
        Self {
            kind,
            message: err.to_string(),
        }
    }
}
