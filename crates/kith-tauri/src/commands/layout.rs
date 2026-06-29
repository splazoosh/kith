//! The `compute_layout` command: the one IPC entry that turns a
//! `(root, mode, generations)` request into the positioned `LayoutModel` the
//! canvas renders — for all four modes (Network included).
//! Thin by construction — [`compute_layout`](kith_core::layout::compute_layout)
//! already probes the root (`NotFound`) and range-checks `generations`
//! (`Validation`); this wrapper only marshals the id and routes through
//! [`with_store`](crate::state::AppState::with_store), so a large layout
//! never runs on the UI thread.

use kith_core::prelude::{ChartMode, LayoutModel, PersonId};
use tauri::State;

use crate::error::CommandError;
use crate::state::AppState;

/// Computes a positioned [`LayoutModel`] for `root` in `mode`, walking up to
/// `generations` ranks (edges from the root). Pure forwarding — no validation or
/// geometry lives here; [`compute_layout`](kith_core::layout::compute_layout)
/// owns both.
///
/// # Errors
/// [`CommandError`] — `not_found` if `root` is absent; `validation` if
/// `generations` exceeds [`MAX_GENERATIONS`](kith_core::prelude::MAX_GENERATIONS);
/// otherwise the mapped store error. (Network ignores `generations` — it lays out
/// the whole connected component.)
pub async fn compute_layout_impl(
    state: &AppState,
    root: PersonId,
    mode: ChartMode,
    generations: u32,
) -> Result<LayoutModel, CommandError> {
    state
        // Call the core fn by full path: a `use` of the prelude re-export would
        // clash with this module's own `compute_layout` command fn.
        .with_store(move |store| kith_core::layout::compute_layout(&store, root, mode, generations))
        .await
}

/// IPC: compute a chart layout for `root` in `mode` to `generations` depth.
///
/// # Errors
/// See [`compute_layout_impl`].
#[tauri::command]
pub async fn compute_layout(
    state: State<'_, AppState>,
    root: i64,
    mode: ChartMode,
    generations: u32,
) -> Result<LayoutModel, CommandError> {
    compute_layout_impl(state.inner(), PersonId::new(root), mode, generations).await
}
