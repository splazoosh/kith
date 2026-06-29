//! `kith-tauri` — the Tauri 2 desktop backend, a thin shell over `kith-core`.
//!
//! The backend works headlessly:
//! it manages [`AppState`](state::AppState), exposes the full
//! `#[tauri::command]` surface (`commands`) over the synchronous `Store` through
//! the one [`with_store`](state::AppState::with_store) sync/async seam, defines
//! the serializable [`CommandError`](error::CommandError) contract, and reopens
//! the last database at startup. `kith-core` owns all logic; this crate only
//! wires the shell and marshals IPC calls — no layout, query, or domain logic
//! lives here.
//!
//! This is an **application** crate, not a published library, so it allows
//! `missing_docs`: the workspace warns on it and CI runs clippy with
//! `-D warnings`, which would otherwise fail on Tauri-generated command
//! registrations. `unsafe_code = "deny"` (workspace) stays — no `unsafe` here.
#![allow(missing_docs)]

pub mod commands;
pub mod config;
pub mod error;
pub mod state;

use tauri::Manager;

/// Builds and runs the Kith desktop application.
///
/// # Panics
/// Panics if the Tauri runtime fails to initialize — an unrecoverable startup
/// invariant (a malformed `tauri.conf.json`, a missing embedded icon, or a
/// webview that cannot be created), not a user-facing, recoverable error.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .setup(|app| {
            // Best-effort reopen of the last database. A moved/deleted file
            // (or unreadable config) leaves no database open — the UI prompts
            // open/create; never fatal.
            if let Ok(dir) = app.path().app_config_dir() {
                let state = app.state::<state::AppState>();
                commands::db::reopen_last(&state, &dir);
                // If a database reopened, allow its media folder in the asset
                // scope so portraits stream on the canvas.
                let reopened = state
                    .last_path
                    .lock()
                    .expect("AppState.last_path mutex poisoned")
                    .clone();
                if let Some(path) = reopened {
                    commands::db::allow_media_scope(app.handle(), &path);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::about::about_info,
            commands::db::db_create,
            commands::db::db_open,
            commands::db::db_close,
            commands::db::db_current,
            commands::person::person_list,
            commands::person::person_get,
            commands::person::person_create,
            commands::person::person_update,
            commands::person::person_delete,
            commands::person::search,
            commands::family::family_list,
            commands::family::family_get,
            commands::family::family_create,
            commands::family::family_update,
            commands::family::family_delete,
            commands::family::family_add_partner,
            commands::family::family_add_child,
            commands::family::family_remove_child,
            commands::event::event_add,
            commands::event::event_get,
            commands::event::event_update,
            commands::event::event_delete,
            commands::name::name_add,
            commands::name::name_list,
            commands::name::name_remove,
            commands::date::parse_date,
            commands::layout::compute_layout,
            commands::export::export_html,
            commands::gedcom::export_gedcom,
            commands::gedcom::import_gedcom,
            commands::media::media_import,
            commands::media::media_for,
            commands::media::media_paths,
            commands::media::media_set_primary,
            commands::media::media_delete,
            commands::source::source_create,
            commands::source::source_list,
            commands::source::source_get,
            commands::source::source_update,
            commands::source::source_delete,
            commands::source::citation_add,
            commands::source::citations_for,
            commands::source::citation_delete,
            commands::undo::undo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Kith Tauri application");
}
