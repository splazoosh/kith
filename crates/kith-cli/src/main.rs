//! `kith` — the command-line interface over `kith-core`.
//!
//! This binary is a thin shell: it delegates to [`kith_cli::run`], which owns
//! argument parsing, command dispatch, rendering, and the exit-code mapping.

use std::process::ExitCode;

fn main() -> ExitCode {
    kith_cli::run()
}
