//! Correctness-at-scale net (`#[cfg(feature = "dev")]`).
//!
//! Seeds several-thousand individuals via [`kith_core::synth`], then exercises the
//! full read pipeline — all four layout modes, ranked search, a detail-view load,
//! and a GEDCOM export→import round-trip — asserting the geometric, determinism,
//! and count invariants still hold **at scale**. This catches a scale-only bug (an
//! integer overflow, a quadratic blow-up, a determinism break) the small fixtures
//! miss; it is deliberately **not** a timing assertion (timings live in the
//! `criterion` benches, which CI does not wall-clock-assert, to avoid flakiness).
//!
//! Runs under CI's `--all-features`; out of the default `cargo test` (it needs the
//! `dev`-gated `synth`).
#![cfg(feature = "dev")]

use kith_core::prelude::*;
use kith_core::synth::seed_synthetic;

/// Several-thousand individuals — solidly the "responsive on a real-sized tree"
/// scale the performance targets aim for, while keeping the debug-build
/// invariant checks quick.
const INDIVIDUALS: u32 = 3_000;
/// The fixed seed shared with the benches, so the stress shape is reproducible.
const SEED: u64 = 0x5EED_C0DE;
/// Geometry compares are exact grid sums; assert with a tolerance.
const EPSILON: f64 = 1e-6;
/// The tree walks' generation budget (Network ignores it — whole component).
const GENERATIONS: u32 = 6;

/// One seeded in-memory store and its mid-tree focal — a single connected
/// component, so Network walks the whole graph.
fn seeded() -> (Store, PersonId) {
    let store = Store::open_in_memory().expect("open in-memory store");
    let focal = seed_synthetic(&store, INDIVIDUALS, SEED).expect("seed synthetic db");
    (store, focal)
}

/// True when two top-left `(x, y, w, h)` boxes overlap by more than `EPSILON` on
/// both axes (mere touching is not an overlap) — mirrors the `layout_graph` net.
fn boxes_overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    let x_overlap = (ax + aw - bx).min(bx + bw - ax);
    let y_overlap = (ay + ah - by).min(by + bh - ay);
    x_overlap > EPSILON && y_overlap > EPSILON
}

/// The three geometric invariants the small-fixture suite asserts, re-checked at
/// scale: exactly one focus, no two person boxes overlap, `bounds` is the exact
/// tight union of every node box.
fn assert_invariants(model: &LayoutModel, label: &str) {
    let focal = model.nodes.iter().filter(|n| n.focal).count();
    assert_eq!(focal, 1, "{label}: expected exactly one focal node");

    let boxes: Vec<(f64, f64, f64, f64)> = model
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Person)
        .map(|n| (n.x, n.y, n.width, n.height))
        .collect();
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            assert!(
                !boxes_overlap(boxes[i], boxes[j]),
                "{label}: person boxes {i} and {j} overlap at scale",
            );
        }
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for n in &model.nodes {
        min_x = min_x.min(n.x);
        max_x = max_x.max(n.x + n.width);
        min_y = min_y.min(n.y);
        max_y = max_y.max(n.y + n.height);
    }
    let close = |a: f64, b: f64| (a - b).abs() < EPSILON;
    assert!(close(model.bounds.x, min_x), "{label}: bounds.x not tight");
    assert!(close(model.bounds.y, min_y), "{label}: bounds.y not tight");
    assert!(
        close(model.bounds.x + model.bounds.width, max_x),
        "{label}: bounds right edge not tight",
    );
    assert!(
        close(model.bounds.y + model.bounds.height, max_y),
        "{label}: bounds bottom edge not tight",
    );
}

#[test]
fn all_four_layout_modes_hold_their_invariants_and_are_deterministic_at_scale() {
    let (store, focal) = seeded();
    let modes = [
        ChartMode::Ancestors,
        ChartMode::Descendants,
        ChartMode::Hourglass,
        ChartMode::Network,
    ];
    for mode in modes {
        let label = format!("{mode:?}@{INDIVIDUALS}");
        let model = compute_layout(&store, focal, mode, GENERATIONS)
            .unwrap_or_else(|e| panic!("{label}: layout failed: {e:?}"));
        assert_invariants(&model, &label);
        // Determinism: the same DB lays out byte-identically — the guard that no
        // hashing non-determinism crept into the tuned per-row reads.
        let again = compute_layout(&store, focal, mode, GENERATIONS)
            .unwrap_or_else(|e| panic!("{label}: second layout failed: {e:?}"));
        assert_eq!(
            model, again,
            "{label}: layout is not deterministic at scale"
        );
    }
}

#[test]
fn search_returns_ranked_hits_at_scale() {
    let (store, _) = seeded();
    // A common bloodline/married-in surname must match many people, ranked.
    let hits = store.search("Lund", 50).expect("search a surname");
    assert!(
        !hits.is_empty(),
        "a common surname should match at scale, ranked"
    );
    // The empty query lists everyone, bounded by the limit (the "matches all" path).
    let bounded = store.search("", 25).expect("search empty");
    assert_eq!(bounded.len(), 25, "an empty query lists up to the limit");
}

#[test]
fn a_detail_view_loads_at_scale() {
    let (store, focal) = seeded();
    let view = PersonView::load(&store, focal).expect("load the focal's person view");
    assert_eq!(view.individual.id, focal);
    // The mid-tree focal has parents and dated vitals — real related rows.
    assert!(
        !view.child_in.is_empty(),
        "a mid-tree focal is a child of some family"
    );
    assert!(!view.events.is_empty(), "the focal has dated events");
}

#[test]
fn gedcom_round_trips_at_scale_preserving_counts() {
    let (store, _) = seeded();
    let doc = kith_core::gedcom::export(&store).expect("export the whole tree");

    let fresh = Store::open_in_memory().expect("fresh in-memory store");
    let options = ImportOptions::default();
    kith_core::gedcom::import(&fresh, &doc, &options).expect("re-import the export");

    // A whole-tree round-trip preserves the individual and family counts (synth
    // people are not living, so the un-redacted export carries them all).
    assert_eq!(
        store.list_individuals().expect("original people").len(),
        fresh.list_individuals().expect("re-imported people").len(),
        "individual count survives a GEDCOM round-trip at scale",
    );
    assert_eq!(
        store.list_families().expect("original families").len(),
        fresh.list_families().expect("re-imported families").len(),
        "family count survives a GEDCOM round-trip at scale",
    );
}
