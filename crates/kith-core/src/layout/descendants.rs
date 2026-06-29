//! The descendants mode adapter: focus at the top, descendants below.
//!
//! # Arena shape
//!
//! The tidy arena is a forest rooted at the focus where **unions are intermediate
//! nodes**: a person's tidy-children are the unions it partners (walk order), and
//! a union's tidy-children are that union's child persons (birth order). This is
//! what makes the core centre each union over *its own* children — the
//! multiple-marriage requirement.
//!
//! # The non-focal partner (spouse)
//!
//! A spouse is **not** its own tidy node. Instead the union's tidy node is
//! widened to `UNION_W + SIBLING_GAP + CARD_WIDTH` to reserve a slot, and a
//! deterministic post-pass drops the spouse card into that slot. Because the
//! reserved width is narrower than the union's children span and the tidy core
//! keeps union boxes apart, the spouse card lives strictly inside its subtree's
//! contour — so the core's non-overlap guarantee extends to it for free.
//!
//! > **Note.** Placing the spouse at the *person's* y-band overlaps the two
//! > cards, because the primary card is tidy-centred over the inflated union
//! > box. The spouse is therefore placed at the *union's* band (beside the
//! > joiner), which keeps the no-overlap invariant — an aesthetic-vs-geometry
//! > trade.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::query::{NodeRef, RelativeGraph};

use super::LayoutModel;
use super::build::{Adjacency, Placement, adjacency, emit, focus_ref, tidy_opts};
use super::metrics::{CARD_HEIGHT, CARD_WIDTH, SIBLING_GAP, UNION_H, UNION_W};
use super::tidy::{TidyNode, TidyTree, layout};

/// What a tidy arena node stands for.
enum Slot {
    /// A primary person card (the focus or a descendant).
    Person(NodeRef),
    /// A union joiner, optionally reserving a slot for the non-focal partner.
    Union {
        /// The union's graph node.
        node: NodeRef,
        /// The spouse to drop into the reserved slot, if the union has one.
        spouse: Option<NodeRef>,
    },
}

/// Positions a descendants [`RelativeGraph`] into a [`LayoutModel`].
pub(crate) fn position(graph: &RelativeGraph) -> LayoutModel {
    emit(graph, &placements(graph))
}

/// Computes a top-left box per graph node (persons primary + spouses + unions).
///
/// Shared with [`hourglass`](super::hourglass), which runs this on the full
/// `relatives` graph: it naturally covers only the descendant side, since the
/// focus partners only its own (downward) unions.
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
        for &union in unions_of(&adj, person) {
            // The other partner (if any) becomes the reserved spouse.
            let spouse = adj
                .union_partners
                .get(&union)
                .and_then(|partners| partners.iter().copied().find(|&p| p != person));
            let (width, height) = if spouse.is_some() {
                (UNION_W + SIBLING_GAP + CARD_WIDTH, CARD_HEIGHT)
            } else {
                (UNION_W, UNION_H)
            };
            let union_idx = nodes.len();
            nodes.push(TidyNode {
                parent: Some(person_idx),
                children: Vec::new(),
                width,
                height,
            });
            slots.push(Slot::Union {
                node: union,
                spouse,
            });
            nodes[person_idx].children.push(union_idx);

            for &child in children_of(&adj, union) {
                if !visited.insert(child) {
                    continue; // a node is placed once (the walk already deduped)
                }
                let child_idx = nodes.len();
                nodes.push(person_node(Some(union_idx)));
                slots.push(Slot::Person(child));
                nodes[union_idx].children.push(child_idx);
                queue.push_back((child, child_idx));
            }
        }
    }

    let tree = TidyTree { nodes, root: 0 };
    let out = layout(&tree, &tidy_opts());

    let mut placements: BTreeMap<NodeRef, Placement> = BTreeMap::new();
    for (idx, slot) in slots.iter().enumerate() {
        let pos = out.positions[idx];
        match *slot {
            Slot::Person(node) => {
                placements.insert(
                    node,
                    Placement {
                        x: pos.x - CARD_WIDTH / 2.0,
                        y: pos.y,
                        width: CARD_WIDTH,
                        height: CARD_HEIGHT,
                    },
                );
            }
            Slot::Union { node, spouse } => {
                let box_left = pos.x - tree.nodes[idx].width / 2.0;
                placements.insert(
                    node,
                    Placement {
                        x: box_left,
                        y: pos.y,
                        width: UNION_W,
                        height: UNION_H,
                    },
                );
                if let Some(spouse) = spouse {
                    placements.insert(
                        spouse,
                        Placement {
                            x: box_left + UNION_W + SIBLING_GAP,
                            y: pos.y,
                            width: CARD_WIDTH,
                            height: CARD_HEIGHT,
                        },
                    );
                }
            }
        }
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

/// The unions `person` partners (downward), in edge order; empty if none.
fn unions_of(adj: &Adjacency, person: NodeRef) -> &[NodeRef] {
    adj.person_unions
        .get(&person)
        .map_or(&[][..], Vec::as_slice)
}

/// The child persons of `union`, in edge order; empty if none.
fn children_of(adj: &Adjacency, union: NodeRef) -> &[NodeRef] {
    adj.union_children
        .get(&union)
        .map_or(&[][..], Vec::as_slice)
}
