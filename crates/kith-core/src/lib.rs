//! `kith-core` — the UI-agnostic domain core for Kith.
//!
//! All domain logic lives here: the data model, genealogical dates, SQLite
//! persistence, queries, layout, rendering, and GEDCOM interchange. The CLI
//! (`kith-cli`) and the desktop app (`kith-tauri`) are thin layers over this
//! crate; dependencies point *toward* the core, never out of it.

pub mod date;
pub mod db;
pub mod error;
pub mod gedcom;
pub mod layout;
pub mod model;
pub mod prelude;
pub mod query;
pub mod render;

mod util;

/// A deterministic synthetic-database generator for benchmarks and the
/// large-graph invariant test. Behind the `dev` feature and `#[doc(hidden)]` —
/// a dev/test utility, **not** a supported API; out of the release build
/// entirely.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod synth;
