//! The `RelativeGraph` walk proof.
//!
//! The hand-shaped fixture trees live in [`common`] (shared with the `layout`
//! suite). Each tree's whole-graph *shape* is pinned with an `insta` RON
//! snapshot; the specific relationship *properties* are asserted in code so an
//! unrelated change churns a snapshot, not a behaviour test.

mod common;

use common::{
    Tree, cousin_marriage, cyclic, isolated, missing_grandparents, multiple_marriage,
    small_balanced,
};
use insta::assert_ron_snapshot;
use kith_core::prelude::*;

// ---------------------------------------------------------------------------
// Snapshots — whole-graph shape regression
// ---------------------------------------------------------------------------

#[test]
fn snapshot_descendants_small_balanced() {
    let (t, root) = small_balanced();
    let graph = descendants(&t.store, root, 2).expect("descendants");
    assert_ron_snapshot!("descendants_small_balanced_g2", &graph);
}

#[test]
fn snapshot_ancestors_small_balanced() {
    let (t, root) = small_balanced();
    let graph = ancestors(&t.store, root, 2).expect("ancestors");
    assert_ron_snapshot!("ancestors_small_balanced_g2", &graph);
}

#[test]
fn snapshot_hourglass_small_balanced() {
    let (t, root) = small_balanced();
    let graph = relatives(&t.store, root, 1, 1).expect("relatives");
    assert_ron_snapshot!("hourglass_small_balanced_up1_down1", &graph);
}

#[test]
fn snapshot_descendants_multiple_marriage() {
    let (t, root) = multiple_marriage();
    let graph = descendants(&t.store, root, 1).expect("descendants");
    assert_ron_snapshot!("descendants_multiple_marriage_g1", &graph);
}

#[test]
fn snapshot_ancestors_missing_grandparents() {
    let (t, root) = missing_grandparents();
    let graph = ancestors(&t.store, root, 2).expect("ancestors");
    assert_ron_snapshot!("ancestors_missing_grandparents_g2", &graph);
}

#[test]
fn snapshot_ancestors_cousin_marriage() {
    let (t, gustav, _) = cousin_marriage();
    let graph = ancestors(&t.store, gustav, 3).expect("ancestors");
    assert_ron_snapshot!("ancestors_cousin_marriage_g3", &graph);
}

// ---------------------------------------------------------------------------
// Behaviour — relationship properties asserted in code
// ---------------------------------------------------------------------------

#[test]
fn generations_count_edges_from_the_root() {
    let (t, root) = small_balanced();

    // gen 0 = just the root, no relations expanded.
    let g0 = ancestors(&t.store, root, 0).expect("ancestors 0");
    assert_eq!(g0.persons.len(), 1);
    assert!(g0.unions.is_empty());
    assert!(g0.edges.is_empty());
    assert!(g0.persons[0].focal);
    assert_eq!(g0.persons[0].generation, 0);

    // gen 1 = root + two parents, but no grandparents yet.
    let g1 = ancestors(&t.store, root, 1).expect("ancestors 1");
    assert_eq!(g1.persons.len(), 3);
    assert_eq!(g1.persons.iter().filter(|p| p.generation == -1).count(), 2);
    assert_eq!(g1.persons.iter().filter(|p| p.generation == -2).count(), 0);

    // gen 2 = root + 2 parents + 4 grandparents.
    let g2 = ancestors(&t.store, root, 2).expect("ancestors 2");
    assert_eq!(g2.persons.len(), 7);
    assert_eq!(g2.persons.iter().filter(|p| p.generation == -2).count(), 4);
}

#[test]
fn descendants_carry_years_and_leave_undated_spouse_blank() {
    let (t, root) = small_balanced();
    let graph = descendants(&t.store, root, 1).expect("descendants");

    // root + spouse + two children.
    assert_eq!(graph.persons.len(), 4);

    let root_node = graph.persons.iter().find(|p| p.focal).expect("focal node");
    assert_eq!(root_node.birth_year, Some(1850));
    assert_eq!(root_node.death_year, Some(1915));

    // The spouse has no dated events → both years are None (never an error).
    let spouse = graph
        .persons
        .iter()
        .find(|p| p.display_name == "Kari Lund")
        .expect("spouse node");
    assert_eq!(spouse.birth_year, None);
    assert_eq!(spouse.death_year, None);
    assert!(!spouse.focal);
}

#[test]
fn twice_married_person_yields_two_unions_with_children_grouped() {
    let (t, root) = multiple_marriage();
    let graph = descendants(&t.store, root, 1).expect("descendants");

    assert_eq!(graph.unions.len(), 2, "one union per marriage");

    // Children grouped under the right union: count Descent edges per union.
    let children_of = |union: NodeRef| {
        graph
            .edges
            .iter()
            .filter(|e| matches!(e, RelEdge::Descent { union: u, .. } if *u == union))
            .count()
    };
    let counts: Vec<usize> = graph.unions.iter().map(|u| children_of(u.node)).collect();
    // First marriage has two children, the second has one (family id order).
    assert_eq!(counts, vec![2, 1]);
}

#[test]
fn ancestors_leave_a_gap_where_grandparents_are_unrecorded() {
    let (t, root) = missing_grandparents();
    let graph = ancestors(&t.store, root, 2).expect("ancestors");

    // root + father + mother + the two paternal grandparents = 5 (no maternal).
    assert_eq!(graph.persons.len(), 5);
    assert_eq!(
        graph.persons.iter().filter(|p| p.generation == -2).count(),
        2,
        "only the paternal grandparents are present"
    );
    // The mother is present even though her parents are not.
    assert!(graph.persons.iter().any(|p| p.display_name == "Greta Holm"));
}

#[test]
fn cousin_marriage_duplicates_the_shared_ancestor() {
    let (t, gustav, old_anders) = cousin_marriage();
    let graph = ancestors(&t.store, gustav, 3).expect("ancestors");

    // The shared great-grandfather is reached down both cousins' lines, so he
    // appears as two distinct appearances of the same row (pedigree collapse).
    let appearances = graph
        .persons
        .iter()
        .filter(|p| p.person == old_anders)
        .count();
    assert_eq!(appearances, 2);

    // …but they are distinct nodes (distinct NodeRefs).
    let refs: Vec<NodeRef> = graph
        .persons
        .iter()
        .filter(|p| p.person == old_anders)
        .map(|p| p.node)
        .collect();
    assert_ne!(refs[0], refs[1]);
}

#[test]
fn a_bad_data_cycle_terminates_in_both_directions() {
    let (t, alpha, beta) = cyclic();

    // The generation budget bounds the walk even though the data is cyclic; the
    // path guard stops each person re-appearing within their own ancestry.
    let up = ancestors(&t.store, alpha, MAX_GENERATIONS).expect("ancestors terminate");
    assert_eq!(up.persons.iter().filter(|p| p.person == alpha).count(), 1);
    assert_eq!(up.persons.iter().filter(|p| p.person == beta).count(), 1);

    let down = descendants(&t.store, alpha, MAX_GENERATIONS).expect("descendants terminate");
    assert_eq!(down.persons.iter().filter(|p| p.person == alpha).count(), 1);
    assert_eq!(down.persons.iter().filter(|p| p.person == beta).count(), 1);
}

#[test]
fn an_isolated_focus_is_a_single_node_with_no_relations() {
    let (t, hermit) = isolated();
    for graph in [
        ancestors(&t.store, hermit, 4).expect("ancestors"),
        descendants(&t.store, hermit, 4).expect("descendants"),
        relatives(&t.store, hermit, 4, 4).expect("relatives"),
    ] {
        assert_eq!(graph.persons.len(), 1);
        assert!(graph.unions.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.persons[0].focal);
    }
}

#[test]
fn hourglass_shares_the_focus_once_across_both_halves() {
    let (t, root) = small_balanced();
    let graph = relatives(&t.store, root, 1, 1).expect("relatives");

    assert_eq!(graph.mode, ChartMode::Hourglass);
    // The focus appears exactly once — not duplicated between the two halves —
    // even though both the ancestor and descendant walks expand from it. (The
    // focus's spouse also sits at rank 0, so a rank-0 count is not the test.)
    assert_eq!(graph.persons.iter().filter(|p| p.focal).count(), 1);
    assert_eq!(graph.persons.iter().filter(|p| p.person == root).count(), 1);
    // Both halves are present: parents above, children below.
    assert!(graph.persons.iter().any(|p| p.generation == -1));
    assert!(graph.persons.iter().any(|p| p.generation == 1));
}

#[test]
fn a_missing_root_is_not_found() {
    let t = Tree::new();
    let err = ancestors(&t.store, PersonId::new(999), 2).expect_err("missing root");
    assert!(matches!(err, CoreError::NotFound { .. }), "got {err:?}");
}

#[test]
fn an_out_of_range_generations_is_a_validation_error() {
    let (t, root) = small_balanced();
    let err = ancestors(&t.store, root, MAX_GENERATIONS + 1).expect_err("over budget");
    assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
    // A missing root is checked *before* the budget (probe, then validate).
    let err = ancestors(&t.store, PersonId::new(999), MAX_GENERATIONS + 1)
        .expect_err("missing + over budget");
    assert!(matches!(err, CoreError::NotFound { .. }), "got {err:?}");
}
