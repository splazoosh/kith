//! `export_html` — render a chart to a single self-contained `.html` file from the GUI.
//!
//! A thin command over [`compute_layout`](kith_core::layout::compute_layout) +
//! [`kith_core::render::html`] + a guarded `std::fs::write`. **No rendering / redaction /
//! layout logic lives here** — redaction is applied inside `render::html`, so
//! `include_living` is the only opt-out and the CLI and this GUI surface share
//! one path. The save dialog (frontend) yields the `out_path` *string*; this command does
//! the IO, so no `fs` ACL is needed.

use std::fs;

use tauri::State;

use kith_core::layout::compute_layout;
use kith_core::prelude::{ChartMode, CoreError, HtmlExportOptions, PersonId, Theme};
use kith_core::render;

use crate::commands::media::media_root;
use crate::error::CommandError;
use crate::state::AppState;

/// Renders the chart rooted at `root` and writes the self-contained HTML to `out_path`.
///
/// Runs off the UI thread via [`AppState::with_store`]: it computes the layout, renders
/// the HTML (redaction applied inside `render::html` from `include_living`), and writes
/// the file. The `io::Error` is carried as [`CoreError::Io`] so the [`CommandError`]
/// surfaces with `kind == Io` (mirroring the CLI's exit-5 mapping).
///
/// # Errors
/// - [`CommandError`] `NotFound` if `root` names no person (the `compute_layout` probe).
/// - `Validation` if `generations` is over budget (or, defensively, `Network` — never sent).
/// - `Io` if the write fails (bad path, permissions, missing parent directory).
// The export's options are independent flat parameters (mirrored from the CLI's
// `export html` flags and the GUI dialog); a wrapper struct would only obscure them.
#[allow(clippy::too_many_arguments)]
pub async fn export_html_impl(
    state: &AppState,
    root: PersonId,
    mode: ChartMode,
    generations: u32,
    theme: Theme,
    include_living: bool,
    portraits: bool,
    out_path: String,
) -> Result<(), CommandError> {
    // Resolve the media folder up front (only when portraits are requested); the
    // closure base64-embeds over it so the export stays self-contained.
    let media_root = if portraits {
        Some(media_root(state)?)
    } else {
        None
    };
    state
        .with_store(move |store| {
            let model = compute_layout(&store, root, mode, generations)?;
            // `HtmlExportOptions` is `#[non_exhaustive]` — build from `default()` and set
            // the public fields (no struct literal outside `kith-core`; the CLI does the same).
            let mut options = HtmlExportOptions::default();
            options.theme = theme;
            options.include_living = include_living;
            // `options.title` stays `None` → the renderer derives a title from focus + mode.
            if let Some(media_root) = &media_root {
                // Resolve only the ids that survive redaction (a living person's
                // bytes are never read), then base64-embed; `render::html`
                // stays pure and the file carries no external reference.
                let ids = render::export_portrait_ids(&model, include_living);
                options.portrait_urls = store.portrait_data_urls(media_root, &ids)?;
                options.portraits = true;
            }
            let doc = render::html(&model, &options);
            // The write's `io::Error` MUST be carried as `CoreError::Io` or `with_store`'s
            // `From<CoreError>` maps it to `Unexpected`, not `Io`.
            fs::write(&out_path, doc).map_err(CoreError::Io)?;
            Ok(())
        })
        .await
}

/// `export_html` IPC command — see [`export_html_impl`].
///
/// # Errors
/// See [`export_html_impl`].
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn export_html(
    state: State<'_, AppState>,
    root: i64,
    mode: ChartMode,
    generations: u32,
    theme: Theme,
    include_living: bool,
    portraits: bool,
    out_path: String,
) -> Result<(), CommandError> {
    export_html_impl(
        state.inner(),
        PersonId::new(root),
        mode,
        generations,
        theme,
        include_living,
        portraits,
        out_path,
    )
    .await
}
