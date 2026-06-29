//! The crate-wide error type and `Result` alias.

use thiserror::Error;

/// The error returned by every fallible `kith-core` operation.
///
/// Third-party failures (`rusqlite`, `r2d2`, `rusqlite_migration`, I/O) are
/// wrapped transparently so their source chain is preserved; domain failures
/// (`NotFound`, `Validation`) carry their own messages.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// A SQLite operation failed.
    #[error(transparent)]
    Database(#[from] rusqlite::Error),

    /// Acquiring a connection from the pool failed (e.g. timed out).
    #[error(transparent)]
    Pool(#[from] r2d2::Error),

    /// Applying schema migrations failed.
    #[error(transparent)]
    Migration(#[from] rusqlite_migration::Error),

    /// An I/O operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A requested entity does not exist.
    #[error("{entity} with id {id} not found")]
    NotFound {
        /// The entity kind, e.g. `"individual"` or `"family"`.
        entity: &'static str,
        /// The numeric id that was not found.
        id: i64,
    },

    /// A value failed domain validation.
    #[error("{0}")]
    Validation(String),
}

/// A specialized [`Result`](std::result::Result) for `kith-core`, fixing the
/// error type to [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;
