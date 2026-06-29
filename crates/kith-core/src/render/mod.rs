//! Static HTML export: turn a positioned [`LayoutModel`](crate::layout::LayoutModel)
//! into a single, self-contained `.html` document that renders the *same* chart
//! the canvas draws — inline `<svg>`, inline `<style>` (the design tokens
//! for both themes + print), and a tiny dependency-free pan/zoom + theme-toggle
//! script — with living-person redaction on by default.
//!
//! # The contract (the whole module obeys it)
//! - **No geometry is recomputed.** [`html`] reads `node.x/y/width/height`,
//!   `link.anchors`, and `bounds`; it computes only the `viewBox` and the stroke
//!   shape between the model's own waypoints (mirroring the frontend, on the Rust
//!   side). It never positions a node or routes a link.
//! - **Parity with the canvas is mechanical** — every SVG element + token has a
//!   counterpart in `TreeNode.svelte`/`TreeCanvas.svelte`/`tokens.css`.
//! - **Determinism** — same model + options ⇒ byte-identical HTML (no `now()`,
//!   no random, no `HashMap` iterated into output).
//! - **Privacy by default** — living persons are redacted unless `include_living`.
//! - **Self-contained** — no external `href`/`src`/`@import`/web-font/`<script src>`.

use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::layout::LayoutModel;
use crate::model::MediaId;

mod escape;
mod html;
mod redact;
mod svg;
mod theme;

pub use html::html;

/// The portrait media ids an export of `model` will actually embed: each person
/// node's `portrait`, minus living persons unless `include_living`.
///
/// The export caller pairs this with [`Store::portrait_data_urls`] to resolve
/// **only** the bytes that will be drawn — so a living person's image is never
/// read (the redaction gate, applied before any file IO), and `render::html`
/// still receives a finished `portrait_urls` map and stays pure.
///
/// [`Store::portrait_data_urls`]: crate::db::Store::portrait_data_urls
#[must_use]
pub fn export_portrait_ids(model: &LayoutModel, include_living: bool) -> Vec<MediaId> {
    model
        .nodes
        .iter()
        .filter_map(|n| n.content.as_ref())
        .filter(|c| include_living || !c.living)
        .filter_map(|c| c.portrait)
        .collect()
}

/// Which palette the exported document opens in (the toggle can flip it).
///
/// Serialises by variant name (`"Light"`/`"Dark"`), matching the `Sex`/`ChartMode`
/// wire convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Theme {
    /// The warm-neutral light palette (the default).
    #[default]
    Light,
    /// The dark palette.
    Dark,
}

impl FromStr for Theme {
    type Err = CoreError;

    /// Parses `"light"`/`"dark"` (case-insensitive) for the CLI `--theme`.
    ///
    /// # Errors
    /// [`CoreError::Validation`] for any other string.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            other => Err(CoreError::Validation(format!(
                "unknown theme {other:?}; expected `light` or `dark`"
            ))),
        }
    }
}

/// Options for [`html`].
///
/// `#[non_exhaustive]` so later fields stay additive; [`Default`] is light,
/// redact the living, no portraits, and a derived title. Construct it with
/// [`HtmlExportOptions::default`] and set the public fields you need.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HtmlExportOptions {
    /// The palette the document opens in.
    pub theme: Theme,
    /// When `true`, do **not** redact living persons (the opt-out; default `false`).
    pub include_living: bool,
    /// An explicit document `<title>`; `None` derives one from the focal node + mode.
    pub title: Option<String>,
    /// When `true`, embed a portrait `<image>` inside a person's card for any
    /// node whose `portrait` id resolves in [`portrait_urls`](Self::portrait_urls).
    /// Default `false` (no portraits).
    pub portraits: bool,
    /// Portrait `MediaId` → a self-contained base64 `data:` URL, resolved by the
    /// **caller** (over the redacted model) so [`html`] stays a pure
    /// `(model, options) -> String` and the export carries no external reference.
    /// A [`BTreeMap`] so embedding order is deterministic.
    pub portrait_urls: BTreeMap<MediaId, String>,
}
