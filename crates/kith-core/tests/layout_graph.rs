//! The `LayoutModel` proof.
//!
//! Two nets over the shared fixture trees ([`common`]):
//!
//! - **Snapshots** pin the whole positioned model's *shape* per fixture × mode
//!   (`insta` RON), so a positioning regression flips a reviewable diff.
//! - **Invariants asserted in code** — no two `Person` boxes overlap, `bounds`
//!   is the exact tight union, the focus is unique — hold for *every* fixture ×
//!   mode, so a geometry regression flips an assertion, not just a snapshot.
//!
//! The two-layer split means a walk regression flips a `walk_graph.rs`
//! snapshot while a positioning regression flips one here.

mod common;

use common::{
    Tree, cousin_marriage, cyclic, isolated, missing_grandparents, multiple_marriage,
    small_balanced, two_lineages_joined, unequal_lineages, wide_pedigree,
};
use insta::assert_ron_snapshot;
use kith_core::prelude::*;

/// Geometry compares are exact grid sums, but assert with a tolerance per
/// `num-float-compare`.
const EPSILON: f64 = 1e-6;

// ---------------------------------------------------------------------------
// Invariants asserted in code
// ---------------------------------------------------------------------------

/// A node box as top-left `(x, y, width, height)` — the `LayoutNode` convention.
type BoxRect = (f64, f64, f64, f64);

/// True when two top-left boxes overlap by more than `EPSILON` on both axes
/// (mere touching is not an overlap).
fn boxes_overlap(a: BoxRect, b: BoxRect) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    let x_overlap = (ax + aw - bx).min(bx + bw - ax);
    let y_overlap = (ay + ah - by).min(by + bh - ay);
    x_overlap > EPSILON && y_overlap > EPSILON
}

/// No two `Person` node boxes overlap.
fn assert_no_person_overlap(model: &LayoutModel, label: &str) {
    let boxes: Vec<BoxRect> = model
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Person)
        .map(|n| (n.x, n.y, n.width, n.height))
        .collect();
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            assert!(
                !boxes_overlap(boxes[i], boxes[j]),
                "{label}: person boxes {i} {:?} and {j} {:?} overlap",
                boxes[i],
                boxes[j],
            );
        }
    }
}

/// `bounds` is the exact tight union of *all* node boxes (persons, unions, and
/// the post-pass spouse cards alike).
fn assert_bounds_are_tight(model: &LayoutModel, label: &str) {
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

/// Exactly one node is the focus.
fn assert_single_focus(model: &LayoutModel, label: &str) {
    let focal = model.nodes.iter().filter(|n| n.focal).count();
    assert_eq!(focal, 1, "{label}: expected exactly one focal node");
}

/// All three geometric invariants for one model.
fn assert_invariants(model: &LayoutModel, label: &str) {
    assert_single_focus(model, label);
    assert_no_person_overlap(model, label);
    assert_bounds_are_tight(model, label);
}

/// Every fixture, in every tree mode, satisfies the geometric invariants.
#[test]
fn invariants_hold_for_every_fixture_and_mode() {
    let single_root: Vec<(&str, (Tree, PersonId))> = vec![
        ("small_balanced", small_balanced()),
        ("multiple_marriage", multiple_marriage()),
        ("missing_grandparents", missing_grandparents()),
        ("isolated", isolated()),
    ];
    let modes = [
        ChartMode::Descendants,
        ChartMode::Ancestors,
        ChartMode::Hourglass,
    ];
    for (name, (tree, root)) in &single_root {
        for mode in modes {
            let model = compute_layout(&tree.store, *root, mode, 3)
                .unwrap_or_else(|e| panic!("{name}/{mode:?}: {e:?}"));
            assert_invariants(&model, &format!("{name}/{mode:?}"));
        }
    }

    // The pedigree-collapse and cyclic fixtures, exercised at depth.
    let (cousins, gustav, _) = cousin_marriage();
    for mode in modes {
        let model = compute_layout(&cousins.store, gustav, mode, 3)
            .unwrap_or_else(|e| panic!("cousin_marriage/{mode:?}: {e:?}"));
        assert_invariants(&model, &format!("cousin_marriage/{mode:?}"));
    }
    let (loop_tree, alpha, _) = cyclic();
    for mode in modes {
        let model = compute_layout(&loop_tree.store, alpha, mode, MAX_GENERATIONS)
            .unwrap_or_else(|e| panic!("cyclic/{mode:?}: {e:?}"));
        assert_invariants(&model, &format!("cyclic/{mode:?}"));
    }
}

// ---------------------------------------------------------------------------
// Network mode — invariants, determinism, edge readability
// ---------------------------------------------------------------------------

/// `generations` is ignored by Network; a clear sentinel makes that explicit.
const NETWORK_GENERATIONS_IGNORED: u32 = 0;

/// Every `Descent` link runs strictly downward (the union sits above its child),
/// the simple form of "no edge passes through an unrelated card".
fn assert_descent_links_go_downward(model: &LayoutModel, label: &str) {
    for link in model.links.iter().filter(|l| l.kind == LinkKind::Descent) {
        let (first, last) = (
            link.anchors.first().expect("a link has anchors"),
            link.anchors.last().expect("a link has anchors"),
        );
        assert!(
            last.y > first.y + EPSILON,
            "{label}: a Descent link does not run downward ({first:?} -> {last:?})",
        );
    }
}

/// Network satisfies the geometric invariants, is deterministic, and routes
/// readably for every multi-branch fixture.
#[test]
fn network_invariants_hold_for_every_fixture() {
    let cases: Vec<(&str, (Tree, PersonId))> = vec![
        ("two_lineages_joined", two_lineages_joined()),
        ("unequal_lineages", unequal_lineages()),
        ("wide_pedigree", wide_pedigree()),
        ("isolated", isolated()),
        ("small_balanced", small_balanced()),
    ];
    for (name, (tree, root)) in &cases {
        let model = compute_layout(
            &tree.store,
            *root,
            ChartMode::Network,
            NETWORK_GENERATIONS_IGNORED,
        )
        .unwrap_or_else(|e| panic!("{name}/Network: {e:?}"));
        let label = format!("{name}/Network");
        assert_eq!(model.mode, ChartMode::Network);
        assert_invariants(&model, &label);
        assert_descent_links_go_downward(&model, &label);

        // Determinism: the same DB lays out byte-identically.
        let again = compute_layout(
            &tree.store,
            *root,
            ChartMode::Network,
            NETWORK_GENERATIONS_IGNORED,
        )
        .expect("second network layout");
        assert_eq!(model, again, "{label}: Network layout is not deterministic");
    }

    // The pedigree-collapse and bad-data cycle fixtures also lay out (the global
    // visited set bounds the cycle — no infinite loop, a valid model).
    let (cousins, gustav, _) = cousin_marriage();
    let model = compute_layout(
        &cousins.store,
        gustav,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("cousin_marriage/Network");
    assert_invariants(&model, "cousin_marriage/Network");
    assert_descent_links_go_downward(&model, "cousin_marriage/Network");

    let (loop_tree, alpha, _) = cyclic();
    let model = compute_layout(
        &loop_tree.store,
        alpha,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("cyclic/Network terminates");
    assert_invariants(&model, "cyclic/Network");
}

/// A cousin marriage is a single connected component: the shared ancestor appears
/// **once** in Network (vs. duplicated in the tree modes — DAG-once).
#[test]
fn network_collapses_a_shared_ancestor_to_one_node() {
    let (t, gustav, old_anders) = cousin_marriage();
    let model = compute_layout(
        &t.store,
        gustav,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("network");
    let appearances = model
        .nodes
        .iter()
        .filter(|n| n.entity == NodeEntity::Person(old_anders))
        .count();
    assert_eq!(
        appearances, 1,
        "the shared ancestor is a single Network node"
    );
}

/// A generation-skewed marriage forces a `Descent` edge across more than one band,
/// which the positioner routes through dummy nodes — so the emitted link carries
/// **interior** waypoints (more than the two endpoint anchors), and the model is
/// still overlap-free (exercising dummy-node edge routing end to end).
#[test]
fn network_routes_long_edges_with_interior_anchors() {
    let (t, focus) = unequal_lineages();
    let model = compute_layout(
        &t.store,
        focus,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("network");
    let routed = model
        .links
        .iter()
        .filter(|l| l.kind == LinkKind::Descent)
        .any(|l| l.anchors.len() > 2);
    assert!(
        routed,
        "the unequal-depth join must route at least one Descent edge through dummies",
    );
    assert_no_person_overlap(&model, "unequal_lineages/Network");
}

/// The invariants hold at scale: a synthetic few-thousand-person pedigree lays out
/// in Network mode with no person overlap, tight bounds, one focus, and downward
/// descent. Gated on the `dev` feature (the synthetic generator); runs under CI's
/// `--all-features`.
#[cfg(feature = "dev")]
#[test]
fn network_invariants_hold_at_scale() {
    let store = Store::open_in_memory().expect("store");
    let focal =
        kith_core::synth::seed_synthetic(&store, 2000, 0xC0FFEE).expect("seed synthetic db");
    let model = compute_layout(
        &store,
        focal,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("network at scale");
    assert_invariants(&model, "synth-2000/Network");
    assert_descent_links_go_downward(&model, "synth-2000/Network");
    assert!(
        model.nodes.len() > 1000,
        "the synthetic component should be large (got {} nodes)",
        model.nodes.len(),
    );
}

/// Structural spot-checks the snapshots alone would not make legible.
#[test]
fn structure_spot_checks() {
    // multiple_marriage descendants → two union nodes.
    let (t, root) = multiple_marriage();
    let model = compute_layout(&t.store, root, ChartMode::Descendants, 1).expect("descendants");
    let unions = model
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Union)
        .count();
    assert_eq!(unions, 2, "two marriages → two union joiners");

    // hourglass has the focus once, with persons both above and below it.
    let (t, root) = small_balanced();
    let model = compute_layout(&t.store, root, ChartMode::Hourglass, 1).expect("hourglass");
    let focus = model.nodes.iter().find(|n| n.focal).expect("a focus node");
    let focus_mid = focus.y + focus.height / 2.0;
    let persons = || model.nodes.iter().filter(|n| n.kind == NodeKind::Person);
    assert!(
        persons().any(|n| n.y + n.height / 2.0 < focus_mid - EPSILON),
        "ancestors sit above the focus",
    );
    assert!(
        persons().any(|n| n.y + n.height / 2.0 > focus_mid + EPSILON),
        "descendants sit below the focus",
    );
}

// ---------------------------------------------------------------------------
// Snapshots — whole positioned model, per fixture × mode
// ---------------------------------------------------------------------------

#[test]
fn snapshot_descendants_small_balanced() {
    let (t, root) = small_balanced();
    let model = compute_layout(&t.store, root, ChartMode::Descendants, 2).expect("descendants");
    assert_ron_snapshot!("descendants_small_balanced_g2", &model);
}

#[test]
fn snapshot_descendants_multiple_marriage() {
    let (t, root) = multiple_marriage();
    let model = compute_layout(&t.store, root, ChartMode::Descendants, 1).expect("descendants");
    assert_ron_snapshot!("descendants_multiple_marriage_g1", &model);
}

#[test]
fn snapshot_descendants_isolated() {
    let (t, root) = isolated();
    let model = compute_layout(&t.store, root, ChartMode::Descendants, 2).expect("descendants");
    assert_ron_snapshot!("descendants_isolated_g2", &model);
}

#[test]
fn snapshot_ancestors_small_balanced() {
    let (t, root) = small_balanced();
    let model = compute_layout(&t.store, root, ChartMode::Ancestors, 2).expect("ancestors");
    assert_ron_snapshot!("ancestors_small_balanced_g2", &model);
}

#[test]
fn snapshot_ancestors_missing_grandparents() {
    let (t, root) = missing_grandparents();
    let model = compute_layout(&t.store, root, ChartMode::Ancestors, 2).expect("ancestors");
    assert_ron_snapshot!("ancestors_missing_grandparents_g2", &model);
}

#[test]
fn snapshot_ancestors_cousin_marriage() {
    let (t, gustav, _) = cousin_marriage();
    let model = compute_layout(&t.store, gustav, ChartMode::Ancestors, 3).expect("ancestors");
    assert_ron_snapshot!("ancestors_cousin_marriage_g3", &model);
}

#[test]
fn snapshot_hourglass_small_balanced() {
    let (t, root) = small_balanced();
    let model = compute_layout(&t.store, root, ChartMode::Hourglass, 1).expect("hourglass");
    assert_ron_snapshot!("hourglass_small_balanced_g1", &model);
}

#[test]
fn snapshot_network_two_lineages_joined() {
    let (t, root) = two_lineages_joined();
    let model = compute_layout(
        &t.store,
        root,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("network");
    assert_ron_snapshot!("network_two_lineages_joined", &model);
}

#[test]
fn snapshot_network_wide_pedigree() {
    let (t, root) = wide_pedigree();
    let model = compute_layout(
        &t.store,
        root,
        ChartMode::Network,
        NETWORK_GENERATIONS_IGNORED,
    )
    .expect("network");
    assert_ron_snapshot!("network_wide_pedigree", &model);
}
