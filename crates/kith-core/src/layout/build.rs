//! Shared translation helpers for the three mode adapters
//! ([`descendants`](super::descendants), [`ancestors`](super::ancestors),
//! [`hourglass`](super::hourglass)).
//!
//! Each adapter does the genealogy-specific arena shaping; everything that is the
//! *same* across the three lives here: deriving the edge adjacency once,
//! minting [`NodeId`]s in a single deterministic pass, building the
//! [`LayoutModel`] from a `NodeRef → `[`Placement`] map, and the link routing
//! anchors. None of this touches a [`Store`](crate::db::Store) — it is a pure
//! `&RelativeGraph` (+ computed positions) → `LayoutModel` translation.

use std::collections::BTreeMap;

use crate::query::{NodeRef, RelEdge, RelativeGraph};

use super::metrics::{RANK_GAP, SIBLING_GAP, SUBTREE_GAP};
use super::tidy::TidyOptions;
use super::{
    LayoutLink, LayoutModel, LayoutNode, LinkKind, NodeContent, NodeEntity, NodeId, NodeKind,
    Point, Rect, lifespan,
};

/// A positioned, **top-left-anchored** box for one graph node (a person card or a
/// union joiner). The adapters compute these from the tidy core's centre-x /
/// band-top-y positions; [`emit`] turns them into [`LayoutNode`]s.
#[derive(Debug, Clone, Copy)]
pub(super) struct Placement {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Box width.
    pub width: f64,
    /// Box height.
    pub height: f64,
}

/// The uniform spacing every adapter feeds the tidy core.
pub(super) fn tidy_opts() -> TidyOptions {
    TidyOptions {
        sibling_gap: SIBLING_GAP,
        subtree_gap: SUBTREE_GAP,
        rank_gap: RANK_GAP,
    }
}

/// Adjacency derived once from the graph's edges, each list in the graph's
/// (deterministic) edge order. The adapters read these instead of re-scanning
/// `edges` per node.
pub(super) struct Adjacency {
    /// person → the unions it partners (a marriage / FAMS), in edge order.
    pub person_unions: BTreeMap<NodeRef, Vec<NodeRef>>,
    /// union → its partner persons, in edge order (`partner1` then `partner2`).
    pub union_partners: BTreeMap<NodeRef, Vec<NodeRef>>,
    /// union → its child persons, in edge order (birth order).
    pub union_children: BTreeMap<NodeRef, Vec<NodeRef>>,
    /// person → the unions it is a child of (FAMC), in edge order.
    pub child_unions: BTreeMap<NodeRef, Vec<NodeRef>>,
}

/// Derives the [`Adjacency`] from a graph's edges.
pub(super) fn adjacency(graph: &RelativeGraph) -> Adjacency {
    let mut adj = Adjacency {
        person_unions: BTreeMap::new(),
        union_partners: BTreeMap::new(),
        union_children: BTreeMap::new(),
        child_unions: BTreeMap::new(),
    };
    for edge in &graph.edges {
        match *edge {
            RelEdge::Partner { person, union } => {
                adj.person_unions.entry(person).or_default().push(union);
                adj.union_partners.entry(union).or_default().push(person);
            }
            RelEdge::Descent { union, child } => {
                adj.union_children.entry(union).or_default().push(child);
                adj.child_unions.entry(child).or_default().push(union);
            }
        }
    }
    adj
}

/// The focus person's graph node (`focal == true`). The walk guarantees exactly
/// one focal person, so the absence of one is a builder bug, not a runtime path.
pub(super) fn focus_ref(graph: &RelativeGraph) -> NodeRef {
    graph
        .persons
        .iter()
        .find(|p| p.focal)
        .map(|p| p.node)
        .expect("a RelativeGraph always has exactly one focal person")
}

/// The centre point of a box.
fn center(p: &Placement) -> Point {
    Point {
        x: p.x + p.width / 2.0,
        y: p.y + p.height / 2.0,
    }
}

/// The midpoint of the box side facing `toward` — the attach point a link uses so
/// it leaves the card's edge nearest its neighbour. Ties (a node directly
/// above/below) resolve to a vertical attach.
fn anchor_on(b: &Placement, toward: Point) -> Point {
    let c = center(b);
    let dx = toward.x - c.x;
    let dy = toward.y - c.y;
    if dy.abs() >= dx.abs() {
        // Vertical attach: bottom edge if the target is below, else the top.
        Point {
            x: c.x,
            y: if dy >= 0.0 { b.y + b.height } else { b.y },
        }
    } else {
        // Horizontal attach: right edge if the target is to the right, else left.
        Point {
            x: if dx >= 0.0 { b.x + b.width } else { b.x },
            y: c.y,
        }
    }
}

/// Builds the finished [`LayoutModel`] with **straight** 2-anchor links — the
/// three tree adapters' routing. A thin delegate to [`emit_routed`] with no
/// interior waypoints; the empty-routes path is byte-identical to a plain
/// straight-link `emit`, so the tree-mode snapshots must not move.
pub(super) fn emit(
    graph: &RelativeGraph,
    placements: &BTreeMap<NodeRef, Placement>,
) -> LayoutModel {
    emit_routed(graph, placements, &BTreeMap::new())
}

/// Builds the finished [`LayoutModel`] from the graph, a placement per node, and a
/// per-edge interior-waypoint polyline (`routes`) — the Network positioner's
/// routed links. An edge absent from `routes` (or with an empty polyline) gets the
/// straight 2-anchor form, so [`emit`] (empty map) reproduces the tree modes.
///
/// Determinism: [`NodeId`]s are minted in one pass — persons in graph
/// order, then unions — and nodes are emitted in that id order; links follow the
/// graph's edge order. `placements`/`routes` are lookups only, never iterated to
/// produce output. `bounds` is recomputed as the tight union of **every** emitted
/// node box (dummies are routing-only and never become nodes).
pub(super) fn emit_routed(
    graph: &RelativeGraph,
    placements: &BTreeMap<NodeRef, Placement>,
    routes: &BTreeMap<(NodeRef, NodeRef), Vec<Point>>,
) -> LayoutModel {
    let mut ids: BTreeMap<NodeRef, NodeId> = BTreeMap::new();
    let mut next: u32 = 0;
    for person in &graph.persons {
        ids.insert(person.node, NodeId::new(next));
        next = next.saturating_add(1);
    }
    for union in &graph.unions {
        ids.insert(union.node, NodeId::new(next));
        next = next.saturating_add(1);
    }

    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(graph.persons.len() + graph.unions.len());
    for person in &graph.persons {
        let Some(place) = placements.get(&person.node) else {
            continue;
        };
        nodes.push(LayoutNode {
            id: ids[&person.node],
            kind: NodeKind::Person,
            entity: NodeEntity::Person(person.person),
            x: place.x,
            y: place.y,
            width: place.width,
            height: place.height,
            content: Some(NodeContent {
                display_name: person.display_name.clone(),
                lifespan: lifespan(person.birth_year, person.death_year, person.living),
                sex: person.sex,
                living: person.living,
                portrait: person.primary_portrait,
            }),
            focal: person.focal,
        });
    }
    for union in &graph.unions {
        let Some(place) = placements.get(&union.node) else {
            continue;
        };
        nodes.push(LayoutNode {
            id: ids[&union.node],
            kind: NodeKind::Union,
            entity: NodeEntity::Union(union.family),
            x: place.x,
            y: place.y,
            width: place.width,
            height: place.height,
            content: None,
            focal: false,
        });
    }

    let empty: Vec<Point> = Vec::new();
    let mut links: Vec<LayoutLink> = Vec::with_capacity(graph.edges.len());
    for edge in &graph.edges {
        match *edge {
            RelEdge::Partner { person, union } => {
                if let (Some(pp), Some(up)) = (placements.get(&person), placements.get(&union)) {
                    // A Partner link terminates at the joiner's centre; interior
                    // waypoints (if any) thread between the card and that centre.
                    let union_center = center(up);
                    let interior = routes.get(&(person, union)).unwrap_or(&empty);
                    let first = interior.first().copied().unwrap_or(union_center);
                    let mut anchors = Vec::with_capacity(interior.len() + 2);
                    anchors.push(anchor_on(pp, first));
                    anchors.extend_from_slice(interior);
                    anchors.push(union_center);
                    links.push(LayoutLink {
                        from: ids[&person],
                        to: ids[&union],
                        kind: LinkKind::Partner,
                        anchors,
                    });
                }
            }
            RelEdge::Descent { union, child } => {
                if let (Some(up), Some(cp)) = (placements.get(&union), placements.get(&child)) {
                    let interior = routes.get(&(union, child)).unwrap_or(&empty);
                    // Each endpoint attaches toward the nearest waypoint (or the
                    // other box's centre when the link is straight).
                    let from_target = interior.first().copied().unwrap_or_else(|| center(cp));
                    let to_target = interior.last().copied().unwrap_or_else(|| center(up));
                    let mut anchors = Vec::with_capacity(interior.len() + 2);
                    anchors.push(anchor_on(up, from_target));
                    anchors.extend_from_slice(interior);
                    anchors.push(anchor_on(cp, to_target));
                    links.push(LayoutLink {
                        from: ids[&union],
                        to: ids[&child],
                        kind: LinkKind::Descent,
                        anchors,
                    });
                }
            }
        }
    }

    let bounds = bounds_of(&nodes);
    LayoutModel {
        mode: graph.mode,
        nodes,
        links,
        bounds,
    }
}

/// The tight union of every node box. A model always has at least the focus, so
/// the sentinels are always replaced; the empty fallback is defensive only.
fn bounds_of(nodes: &[LayoutNode]) -> Rect {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for node in nodes {
        min_x = min_x.min(node.x);
        max_x = max_x.max(node.x + node.width);
        min_y = min_y.min(node.y);
        max_y = max_y.max(node.y + node.height);
    }
    if !min_x.is_finite() {
        return Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}
