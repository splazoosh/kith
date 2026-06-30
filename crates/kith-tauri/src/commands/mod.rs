//! The IPC command surface (`proj-mod-by-feature`).
//!
//! Each entity gets a module of thin `#[tauri::command]` wrappers paired with
//! plain-async `_impl` logic fns that take `&AppState` (and, for DB-lifecycle
//! commands, an explicit `&Path` config dir). `tauri::State`/`AppHandle` are
//! runtime-only and cannot be built in a unit test, so the headless suite
//! drives the `_impl` fns directly with no webview. Every record command
//! routes through [`AppState::with_store`](crate::state::AppState::with_store);
//! the pure `parse_date` does not touch the database.

pub mod about;
pub mod date;
pub mod db;
pub mod dto;
pub mod event;
pub mod export;
pub mod family;
pub mod gedcom;
pub mod layout;
pub mod lb;
pub mod media;
pub mod name;
pub mod person;
pub mod source;
pub mod undo;
