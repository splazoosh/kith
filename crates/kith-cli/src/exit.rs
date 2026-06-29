//! The stable CLI exit-code mapping. clap owns usage errors
//! (exit `2`); this module maps every `anyhow::Error` `run` produces.

use std::process::ExitCode;

use kith_core::prelude::CoreError;

/// Maps the final error to a process exit code by downcasting to [`CoreError`].
///
/// | Code | Meaning                         | Source                                   |
/// |------|---------------------------------|------------------------------------------|
/// | `1`  | unexpected / uncategorized      | any `anyhow` error with no `CoreError`   |
/// | `3`  | not found                       | [`CoreError::NotFound`]                   |
/// | `4`  | validation                      | [`CoreError::Validation`]                 |
/// | `5`  | I/O (incl. "database missing")  | [`CoreError::Io`]                         |
/// | `6`  | database / migration / pool     | `Database` / `Migration` / `Pool`        |
///
/// (`0` success and `2` clap usage errors are produced elsewhere.) Codes `3`
/// and `4` are exercised by the `person show/rm`, `family`, and `event`
/// commands; they are defined because the mapping is the contract.
///
/// `anyhow::Error::downcast_ref` walks the `.context()` chain, so a
/// `CoreError` wrapped with `.with_context(...)` is still recognized.
#[must_use]
pub fn code_for(err: &anyhow::Error) -> ExitCode {
    let code: u8 = match err.downcast_ref::<CoreError>() {
        Some(CoreError::NotFound { .. }) => 3,
        Some(CoreError::Validation(_)) => 4,
        Some(CoreError::Io(_)) => 5,
        Some(CoreError::Database(_) | CoreError::Pool(_) | CoreError::Migration(_)) => 6,
        // `None`, plus any future `#[non_exhaustive]` variant.
        _ => 1,
    };
    ExitCode::from(code)
}
