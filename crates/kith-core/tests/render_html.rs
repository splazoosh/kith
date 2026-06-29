//! The `render::html` proof: canvas parity, redaction, and self-contained +
//! deterministic output. `insta` string snapshots over the SAME fixture trees
//! the layout suite pins, plus structural invariants in code.
//!
//! Because the fixtures default `living = true` (schema `living NOT NULL DEFAULT 1`),
//! a *default* export reads mostly `"Living"` — which is exactly what makes
//! redaction obvious in the snapshots; the `include_living` snapshots are the ones
//! showing real names.

mod common;

use common::{missing_grandparents, multiple_marriage, small_balanced, two_lineages_joined};
use insta::assert_snapshot;
use kith_core::prelude::*;
use kith_core::render::html;

/// A model for `mode` over a fixture root — the faithful path the shells take.
fn model_of(store: &Store, root: PersonId, mode: ChartMode) -> LayoutModel {
    compute_layout(store, root, mode, 3).expect("layout")
}

/// Options built default-then-mutate — `HtmlExportOptions` is `#[non_exhaustive]`,
/// so a struct literal is unavailable to this (external) test crate.
fn options(theme: Theme, include_living: bool) -> HtmlExportOptions {
    let mut o = HtmlExportOptions::default();
    o.theme = theme;
    o.include_living = include_living;
    o
}

// ---------------------------------------------------------------------------
// Snapshots — representative charts × themes × redaction (not combinatorial).
// ---------------------------------------------------------------------------

#[test]
fn descendants_small_balanced_light_redacted_matches_snapshot() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &HtmlExportOptions::default(),
    );
    assert_snapshot!("descendants_small_balanced_light_redacted", doc);
}

#[test]
fn descendants_small_balanced_light_include_living_matches_snapshot() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &options(Theme::Light, true),
    );
    assert_snapshot!("descendants_small_balanced_light", doc);
}

#[test]
fn descendants_small_balanced_dark_include_living_matches_snapshot() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &options(Theme::Dark, true),
    );
    assert_snapshot!("descendants_small_balanced_dark", doc);
}

#[test]
fn descendants_multiple_marriage_light_include_living_matches_snapshot() {
    let (t, root) = multiple_marriage();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &options(Theme::Light, true),
    );
    assert_snapshot!("descendants_multiple_marriage_light", doc);
}

#[test]
fn ancestors_missing_grandparents_light_include_living_matches_snapshot() {
    let (t, root) = missing_grandparents();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Ancestors),
        &options(Theme::Light, true),
    );
    assert_snapshot!("ancestors_missing_grandparents_light", doc);
}

#[test]
fn hourglass_small_balanced_light_include_living_matches_snapshot() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Hourglass),
        &options(Theme::Light, true),
    );
    assert_snapshot!("hourglass_small_balanced_light", doc);
}

#[test]
fn network_two_lineages_joined_light_include_living_matches_snapshot() {
    // The fourth mode renders through the SAME exporter: proves
    // canvas/export parity and that the SVG path draws the Network model's
    // links/cards exactly as the tree modes' (N-anchor polylines).
    let (t, root) = two_lineages_joined();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Network),
        &options(Theme::Light, true),
    );
    assert_snapshot!("network_two_lineages_joined_light", doc);
}

// ---------------------------------------------------------------------------
// Portraits — base64-embedded, redacted for the living, gated by the
// `portraits` flag, snapshot-locked against a FIXED fixture image.
// ---------------------------------------------------------------------------

/// A 1×1 PNG fixed fixture: the embedded base64 (and thus the snapshot) is
/// byte-stable, proving the portrait export stays deterministic + self-contained.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// A one-person store with a primary portrait (deceased unless `living`), plus
/// the temp media root (kept alive by the returned `TempDir`) and the model.
fn person_with_portrait(
    living: bool,
) -> (Store, tempfile::TempDir, std::path::PathBuf, LayoutModel) {
    let store = Store::open_in_memory().expect("store");
    let tmp = tempfile::tempdir().expect("tmp");
    let media_root = tmp.path().join("tree.media");
    let src = tmp.path().join("face.png");
    std::fs::write(&src, TINY_PNG).expect("write fixture");
    let person = store
        .create_individual(&NewIndividual {
            given_name: Some("Ada".to_owned()),
            surname: Some("Lovelace".to_owned()),
            living,
            ..Default::default()
        })
        .expect("person");
    store
        .import_media(&media_root, &src, MediaSubject::Individual(person.id), true)
        .expect("import");
    let model = compute_layout(&store, person.id, ChartMode::Descendants, 1).expect("layout");
    (store, tmp, media_root, model)
}

/// Resolve portrait URLs + build options the way the export shells do.
fn portrait_options(
    store: &Store,
    media_root: &std::path::Path,
    model: &LayoutModel,
    include_living: bool,
) -> HtmlExportOptions {
    let ids = kith_core::render::export_portrait_ids(model, include_living);
    let urls = store
        .portrait_data_urls(media_root, &ids)
        .expect("resolve portrait urls");
    let mut o = HtmlExportOptions::default();
    o.include_living = include_living;
    o.portraits = true;
    o.portrait_urls = urls;
    o
}

#[test]
fn portrait_embeds_base64_for_a_deceased_person_when_opted_in() {
    let (store, _tmp, media_root, model) = person_with_portrait(false);
    let doc = html(
        &model,
        &portrait_options(&store, &media_root, &model, false),
    );
    assert!(
        doc.contains("<image href=\"data:image/png;base64,"),
        "the portrait is embedded as a base64 data URL"
    );
    // Still self-contained — the only resource is the inline data: URL.
    for needle in ["http://", "https://", "src=\"http", "href=\"http"] {
        assert!(!doc.contains(needle), "self-contained: found {needle:?}");
    }
    assert_snapshot!("portrait_descendants_deceased", doc);
}

#[test]
fn living_portrait_is_omitted_by_default_even_with_portraits_on() {
    let (store, _tmp, media_root, model) = person_with_portrait(true);
    // `include_living = false`: the person is redacted, so their portrait id is
    // never resolved AND `content.portrait` is cleared — no image either way.
    let doc = html(
        &model,
        &portrait_options(&store, &media_root, &model, false),
    );
    assert!(
        !doc.contains("<image"),
        "a living portrait is redacted by default"
    );
    assert!(doc.contains("Living"), "and the name is redacted");
}

#[test]
fn portraits_flag_off_emits_no_image_even_when_urls_are_present() {
    let (store, _tmp, media_root, model) = person_with_portrait(false);
    // Resolve the URLs but leave the `portraits` gate off.
    let ids = kith_core::render::export_portrait_ids(&model, true);
    let urls = store
        .portrait_data_urls(&media_root, &ids)
        .expect("resolve");
    let mut o = HtmlExportOptions::default();
    o.include_living = true;
    o.portrait_urls = urls; // present, but `portraits` stays false
    let doc = html(&model, &o);
    assert!(!doc.contains("<image"), "the portraits flag gates emission");
}

// ---------------------------------------------------------------------------
// Structural invariants — hold regardless of snapshots.
// ---------------------------------------------------------------------------

#[test]
fn output_is_self_contained() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Hourglass),
        &HtmlExportOptions::default(),
    );
    for needle in [
        "http://",
        "https://",
        "@import",
        "<script src",
        "<link rel",
        "src=\"http",
    ] {
        assert!(!doc.contains(needle), "self-contained: found {needle:?}");
    }
}

#[test]
fn redaction_hides_living_names_by_default_and_include_living_reveals_them() {
    // The fixture root "Olav Lund" is living by default, so the default output
    // reads "Living"; `include_living` restores the real name.
    let (t, root) = small_balanced();
    let redacted = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &HtmlExportOptions::default(),
    );
    assert!(
        redacted.contains("Living"),
        "default output redacts to Living"
    );
    assert!(
        !redacted.contains("Olav"),
        "default output hides the real name"
    );

    let shown = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &options(Theme::Light, true),
    );
    assert!(shown.contains("Olav"), "include_living shows the real name");
}

#[test]
fn both_theme_token_sets_are_present() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Ancestors),
        &HtmlExportOptions::default(),
    );
    assert!(doc.contains("#faf8f4"), "the light paper token ships");
    assert!(doc.contains("#1a1815"), "the dark paper token ships too");
}

#[test]
fn canvas_vocabulary_markers_are_present() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &options(Theme::Light, true),
    );
    for marker in [
        "viewBox",
        "feDropShadow",
        "class=\"card",
        "class=\"link",
        "stroke-dasharray",
        "<title>",
        "preserveAspectRatio=\"xMidYMid meet\"",
    ] {
        assert!(doc.contains(marker), "canvas parity: missing {marker:?}");
    }
}

#[test]
fn output_is_deterministic() {
    let (t, root) = small_balanced();
    let model = model_of(&t.store, root, ChartMode::Descendants);
    let opts = options(Theme::Dark, false);
    assert_eq!(html(&model, &opts), html(&model, &opts));
}

#[test]
fn document_is_a_complete_html_shell() {
    let (t, root) = small_balanced();
    let doc = html(
        &model_of(&t.store, root, ChartMode::Descendants),
        &HtmlExportOptions::default(),
    );
    assert!(doc.starts_with("<!doctype html>"));
    assert!(doc.trim_end().ends_with("</html>"));
    assert!(doc.contains("<svg"));
    // The default opens in the light palette (the chosen theme is explicit).
    assert!(doc.contains("data-theme=\"light\""));
}
