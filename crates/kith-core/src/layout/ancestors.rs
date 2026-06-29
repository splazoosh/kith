//! The ancestors mode adapter: focus at the bottom, ancestors above.
//!
//! # Arena shape
//!
//! The tidy arena is rooted at the focus: a person's tidy-children are the
//! union(s) it is a *child* of (its parents' marriage), and a union's
//! tidy-children are its parent persons (`partner1` then `partner2`). Unlike the
//! descendants adapter there is **no spouse reservation** — the two parents are
//! ordinary tidy siblings under their union, so this half is a naturally clean
//! tree. Unrecorded parents are simply a union with fewer (or zero) children — a
//! gap, never a placeholder.
//!
//! # Orientation
//!
//! The core lays out downward; a single post-pass **reflects y**
//! (`y' = extent.height − (y + height)`) so the focus lands at the bottom band
//! and ancestors stack upward. x is untouched. A vertical reflection preserves
//! non-overlap, so the core's guarantee survives it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::query::{NodeRef, RelativeGraph};

use super::LayoutModel;
use super::build::{Adjacency, Placement, adjacency, emit, focus_ref, tidy_opts};
use super::metrics::{CARD_HEIGHT, CARD_WIDTH, UNION_H, UNION_W};
use super::tidy::{TidyNode, TidyTree, layout};

/// What a tidy arena node stands for (no spouse reservation in this mode).
enum Slot {
    /// A person card (the focus, a parent, a grandparent, …).
    Person(NodeRef),
    /// A parents' union joiner.
    Union(NodeRef),
}

/// Positions an ancestors [`RelativeGraph`] into a [`LayoutModel`].
pub(crate) fn position(graph: &RelativeGraph) -> LayoutModel {
    emit(graph, &placements(graph))
}

/// Computes a top-left box per graph node, y-reflected so the focus is at the
/// bottom.
///
/// Shared with [`hourglass`](super::hourglass), which runs this on the full
/// `relatives` graph: it naturally covers only the ancestor side, since the focus
/// is a *child* of only its (upward) parents' union.
pub(super) fn placements(graph: &RelativeGraph) -> BTreeMap<NodeRef, Placement> {
    let adj = adjacency(graph);
    let focus = focus_ref(graph);

    let mut nodes: Vec<TidyNode> = Vec::with_capacity(graph.persons.len() + graph.unions.len());
    let mut slots: Vec<Slot> = Vec::with_capacity(nodes.capacity());
    let mut visited: BTreeSet<NodeRef> = BTreeSet::new();

    // Root: the focus person.
    nodes.push(person_node(None));
    slots.push(Slot::Person(focus));
    visited.insert(focus);

    let mut queue: VecDeque<(NodeRef, usize)> = VecDeque::from([(focus, 0usize)]);
    while let Some((person, person_idx)) = queue.pop_front() {
        for &union in parent_unions(&adj, person) {
            let union_idx = nodes.len();
            nodes.push(TidyNode {
                parent: Some(person_idx),
                children: Vec::new(),
                width: UNION_W,
                height: UNION_H,
            });
            slots.push(Slot::Union(union));
            nodes[person_idx].children.push(union_idx);

            for &parent in parents_of(&adj, union) {
                if !visited.insert(parent) {
                    continue;
                }
                let parent_idx = nodes.len();
                nodes.push(person_node(Some(union_idx)));
                slots.push(Slot::Person(parent));
                nodes[union_idx].children.push(parent_idx);
                queue.push_back((parent, parent_idx));
            }
        }
    }

    let tree = TidyTree { nodes, root: 0 };
    let out = layout(&tree, &tidy_opts());
    let extent_height = out.extent.height;

    let mut placements: BTreeMap<NodeRef, Placement> = BTreeMap::new();
    for (idx, slot) in slots.iter().enumerate() {
        let pos = out.positions[idx];
        let height = tree.nodes[idx].height;
        let reflected_top = extent_height - (pos.y + height);
        let (node, width) = match *slot {
            Slot::Person(node) => (node, CARD_WIDTH),
            Slot::Union(node) => (node, UNION_W),
        };
        placements.insert(
            node,
            Placement {
                x: pos.x - width / 2.0,
                y: reflected_top,
                width,
                height,
            },
        );
    }
    placements
}

/// A blank person-sized tidy node.
fn person_node(parent: Option<usize>) -> TidyNode {
    TidyNode {
        parent,
        children: Vec::new(),
        width: CARD_WIDTH,
        height: CARD_HEIGHT,
    }
}

/// The unions `person` is a child of (upward), in edge order; empty if none.
fn parent_unions(adj: &Adjacency, person: NodeRef) -> &[NodeRef] {
    adj.child_unions.get(&person).map_or(&[][..], Vec::as_slice)
}

/// The parent persons of `union`, in `partner1`-then-`partner2` order; empty if
/// none.
fn parents_of(adj: &Adjacency, union: NodeRef) -> &[NodeRef] {
    adj.union_partners
        .get(&union)
        .map_or(&[][..], Vec::as_slice)
}
