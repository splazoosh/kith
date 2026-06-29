//! The Network mode positioner: a hand-rolled Sugiyama-style layered
//! layout over the **whole connected family graph** (a DAG, not a tree), emitting
//! the same [`LayoutModel`](super::LayoutModel) the tree adapters do so the
//! canvas and the HTML exporter render it unchanged.
//!
//! # Why not the tidy core
//!
//! The tidy core ([`tidy`](super::tidy)) is strictly tree-shaped (one root,
//! parent→children). A genealogy DAG — shared ancestors, cousin marriages, two
//! lineages joined by a marriage, remarriage — cannot be a tidy arena, so Network
//! is its own positioner. It reuses only [`metrics`](super::metrics) (card/gap
//! sizes) and [`build`](super::build) (id minting, node emission, bounds, link
//! anchors) — never the tidy geometry.
//!
//! # The pipeline (a small Sugiyama)
//!
//! 1. **Rank** persons by longest-path layering with **partner unification**: a
//!    union's partners share a rank (the deeper one), and each child sits one rank
//!    below (persons carry the integer generations; a malformed cycle is
//!    bounded by an iteration cap, not an infinite loop). A union sits in the gap
//!    *between* its partners' band and its children's band.
//! 2. **Layer** on a doubled scale so unions land between person bands: a person at
//!    rank `r` → layer `2r`; a union whose partners are rank `r` → layer `2r + 1`.
//!    Every edge runs downward between two layers; an edge spanning more than one
//!    layer (a cousin-collapse / remarriage skew) is split with **dummy nodes**,
//!    one per crossed layer, carrying the routing polyline.
//! 3. **Order** each layer by a few median-heuristic sweeps to reduce crossings,
//!    seeded by the walk's BFS discovery order, tie-broken by node identity.
//! 4. **X-assign** by a priority-ish method: an initial size-aware packing, then a
//!    few sweeps nudging each node toward the mean x of its neighbours, never
//!    violating the per-layer minimum separation — which is what keeps person
//!    boxes from overlapping.
//! 5. **Emit** via [`build::emit_routed`](super::build::emit_routed): straight
//!    2-anchor links for adjacent-layer edges (byte-identical to the tree modes'
//!    routing) and dummy-chain waypoints for the long ones.
//!
//! # Determinism
//!
//! Every structure read *into output* is a [`BTreeMap`]/[`Vec`]; the only ordering
//! tie-breaks are by node identity (a stable `LRef`); there is no `now()` and no
//! `HashMap` iterated to emit. Same graph ⇒ byte-identical [`LayoutModel`], so
//! `insta` locks Network like the tree modes.

use std::collections::BTreeMap;

use crate::query::{NodeRef, RelEdge, RelativeGraph};

use super::build::{Placement, adjacency, emit_routed};
use super::metrics::{
    CARD_HEIGHT, CARD_WIDTH, RANK_GAP, SIBLING_GAP, SUBTREE_GAP, UNION_H, UNION_W,
};
use super::{LayoutModel, Point};

/// Median-heuristic ordering sweeps (down, up, …). A small fixed count: enough to
/// settle the legible fixtures, cheap at scale.
const ORDER_SWEEPS: usize = 4;
/// X-coordinate refinement sweeps (alternating down/up, alternating bias).
const X_SWEEPS: usize = 8;

/// A node in the layered graph: a person card, a union joiner, or a routing-only
/// dummy on a long edge. `Ord` (Person < Union < Dummy, then by inner id) is the
/// deterministic tie-break for ordering and x-assignment. Dummies are
/// **never** persons/unions and never become [`LayoutNode`](super::LayoutNode)s —
/// they contribute interior route points only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LRef {
    Person(NodeRef),
    Union(NodeRef),
    Dummy(u32),
}

/// Positions a Network [`RelativeGraph`] into a [`LayoutModel`].
pub(crate) fn position(graph: &RelativeGraph) -> LayoutModel {
    let mut net = Net::build(graph);
    net.reduce_crossings();
    net.assign_x();
    let (placements, routes) = net.finish(graph);
    emit_routed(graph, &placements, &routes)
}

/// The mutable layered-graph state (mirrors `tidy::Tidy`): per-node layer, the
/// per-layer left-to-right order, up/down adjacency, and x; plus the dummy chains
/// that become routed polylines.
struct Net {
    /// Layer (band index on the doubled scale) of every layered node.
    layer: BTreeMap<LRef, i32>,
    /// `order[l]` is layer `l`'s nodes, left to right (the crossing-reduction works on this).
    order: Vec<Vec<LRef>>,
    /// Neighbours one layer **up** (smaller layer index).
    up: BTreeMap<LRef, Vec<LRef>>,
    /// Neighbours one layer **down** (larger layer index).
    down: BTreeMap<LRef, Vec<LRef>>,
    /// Resolved centre x per node.
    x: BTreeMap<LRef, f64>,
    /// Person rank per person (the integer generation; layer `= 2·rank`).
    prank: BTreeMap<NodeRef, i32>,
    /// Union layer per union node (`= 2·partner_rank + 1`).
    union_layer: BTreeMap<NodeRef, i32>,
    /// Dummy nodes in creation order (for a deterministic order seed).
    dummies: Vec<LRef>,
    /// The dummy chain inserted for each long edge, keyed by the edge's
    /// `(from, to)` node refs — the interior waypoints [`build::emit_routed`] threads.
    chains: BTreeMap<(NodeRef, NodeRef), Vec<LRef>>,
    /// Next dummy id (minted in edge order — deterministic).
    next_dummy: u32,
}

impl Net {
    /// Ranks, layers, dummy-split edges, and seeds the per-layer order.
    fn build(graph: &RelativeGraph) -> Self {
        let adj = adjacency(graph);

        // --- rank persons (longest path + partner unification) ------------------
        let mut prank: BTreeMap<NodeRef, i32> =
            graph.persons.iter().map(|p| (p.node, 0i32)).collect();
        // Ranks only increase; a DAG settles quickly. The cap bounds a malformed
        // cycle (the `cyclic` fixture) instead of looping forever.
        let cap = graph.persons.len() + 4;
        for _ in 0..cap {
            let mut changed = false;
            for union in graph.unions.iter().map(|u| u.node) {
                let partners = adj.union_partners.get(&union);
                let pr = partners
                    .map(|ps| ps.iter().map(|p| rank_of(&prank, *p)).max().unwrap_or(0))
                    .unwrap_or(0);
                if let Some(ps) = partners {
                    for &p in ps {
                        if rank_of(&prank, p) < pr {
                            prank.insert(p, pr);
                            changed = true;
                        }
                    }
                }
                if let Some(cs) = adj.union_children.get(&union) {
                    for &c in cs {
                        if rank_of(&prank, c) < pr + 1 {
                            prank.insert(c, pr + 1);
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // --- union layers (in the gap below their partners) ---------------------
        let mut union_layer: BTreeMap<NodeRef, i32> = BTreeMap::new();
        for union in graph.unions.iter().map(|u| u.node) {
            let pr = adj
                .union_partners
                .get(&union)
                .and_then(|ps| ps.iter().map(|p| rank_of(&prank, *p)).max())
                .or_else(|| {
                    // Partnerless union: sit just above its shallowest child.
                    adj.union_children
                        .get(&union)
                        .and_then(|cs| cs.iter().map(|c| rank_of(&prank, *c)).min())
                        .map(|min_child| (min_child - 1).max(0))
                })
                .unwrap_or(0);
            union_layer.insert(union, 2 * pr + 1);
        }

        let mut net = Self {
            layer: BTreeMap::new(),
            order: Vec::new(),
            up: BTreeMap::new(),
            down: BTreeMap::new(),
            x: BTreeMap::new(),
            prank,
            union_layer,
            dummies: Vec::new(),
            chains: BTreeMap::new(),
            next_dummy: 0,
        };

        // --- assign layers to every real node -----------------------------------
        for person in &graph.persons {
            let l = 2 * rank_of(&net.prank, person.node);
            net.layer.insert(LRef::Person(person.node), l);
        }
        for union in &graph.unions {
            let l = net.union_layer[&union.node];
            net.layer.insert(LRef::Union(union.node), l);
        }

        // --- split edges into adjacent-layer links, inserting dummies -----------
        for edge in &graph.edges {
            match *edge {
                RelEdge::Partner { person, union } => {
                    let a = LRef::Person(person);
                    let b = LRef::Union(union);
                    net.add_edge(a, b, (person, union));
                }
                RelEdge::Descent { union, child } => {
                    let a = LRef::Union(union);
                    let b = LRef::Person(child);
                    net.add_edge(a, b, (union, child));
                }
            }
        }

        net.seed_order(graph);
        net
    }

    /// Links `a`→`b` (with `a` on the smaller layer), splitting a multi-layer span
    /// with dummies and recording the chain under `key` for routing.
    fn add_edge(&mut self, a: LRef, b: LRef, key: (NodeRef, NodeRef)) {
        let (la, lb) = (self.layer[&a], self.layer[&b]);
        // Endpoints always run downward by construction (children/unions below
        // their parents); guard the rare equal/inverted case defensively.
        let (top, bottom, lt, lb) = if la <= lb {
            (a, b, la, lb)
        } else {
            (b, a, lb, la)
        };
        if lb - lt <= 1 {
            self.link(top, bottom);
            return;
        }
        let mut prev = top;
        let mut chain = Vec::new();
        for l in (lt + 1)..lb {
            let d = LRef::Dummy(self.next_dummy);
            self.next_dummy += 1;
            self.layer.insert(d, l);
            self.dummies.push(d);
            self.link(prev, d);
            chain.push(d);
            prev = d;
        }
        self.link(prev, bottom);
        self.chains.insert(key, chain);
    }

    /// Records adjacency for an upper→lower pair (one layer apart).
    fn link(&mut self, upper: LRef, lower: LRef) {
        self.down.entry(upper).or_default().push(lower);
        self.up.entry(lower).or_default().push(upper);
    }

    /// Seeds `order` from the walk's BFS discovery order: persons, then unions, then
    /// dummies — a deterministic starting point the median sweeps refine.
    fn seed_order(&mut self, graph: &RelativeGraph) {
        let max_layer = self.layer.values().copied().max().unwrap_or(0);
        self.order = vec![Vec::new(); (max_layer + 1) as usize];
        for person in &graph.persons {
            let n = LRef::Person(person.node);
            self.order[self.layer[&n] as usize].push(n);
        }
        for union in &graph.unions {
            let n = LRef::Union(union.node);
            self.order[self.layer[&n] as usize].push(n);
        }
        for &d in &self.dummies {
            self.order[self.layer[&d] as usize].push(d);
        }
    }

    /// A few median-heuristic sweeps (down then up, alternating) to reduce edge
    /// crossings. A node with no neighbour in the reference layer keeps its place.
    fn reduce_crossings(&mut self) {
        for sweep in 0..ORDER_SWEEPS {
            let down = sweep % 2 == 0;
            let layers: Vec<usize> = if down {
                (1..self.order.len()).collect()
            } else {
                (0..self.order.len().saturating_sub(1)).rev().collect()
            };
            for l in layers {
                self.reorder_layer(l, down);
            }
        }
    }

    /// Reorders one layer by the median index of each node's neighbours in the
    /// adjacent (reference) layer; stable, tie-broken by `LRef`.
    fn reorder_layer(&mut self, l: usize, down: bool) {
        let ref_layer = if down {
            l.checked_sub(1)
        } else {
            l.checked_add(1)
        };
        let Some(ref_layer) = ref_layer else { return };
        let Some(ref_nodes) = self.order.get(ref_layer) else {
            return;
        };
        let pos: BTreeMap<LRef, usize> =
            ref_nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let neighbours = if down { &self.up } else { &self.down };
        let mut keyed: Vec<(f64, LRef)> = self.order[l]
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let med = neighbours
                    .get(&v)
                    .filter(|ns| !ns.is_empty())
                    .map(|ns| {
                        let mut idx: Vec<usize> =
                            ns.iter().filter_map(|n| pos.get(n).copied()).collect();
                        idx.sort_unstable();
                        median(&idx).unwrap_or(i as f64)
                    })
                    .unwrap_or(i as f64);
                (med, v)
            })
            .collect();
        keyed.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        self.order[l] = keyed.into_iter().map(|(_, v)| v).collect();
    }

    /// Assigns x: a size-aware initial packing, then alternating sweeps pulling each
    /// node toward the mean x of its neighbours while preserving min separation.
    fn assign_x(&mut self) {
        self.pack_initial();
        for sweep in 0..X_SWEEPS {
            let down = sweep % 2 == 0;
            let ltr = sweep % 2 == 0;
            let layers: Vec<usize> = if down {
                (1..self.order.len()).collect()
            } else {
                (0..self.order.len().saturating_sub(1)).rev().collect()
            };
            for l in layers {
                let desired = self.desired_x(l, down);
                self.place_layer(l, &desired, ltr);
            }
        }
    }

    /// Initial left-to-right packing per layer (starts at 0); guarantees no
    /// within-layer overlap, which (with disjoint y-bands) guarantees no
    /// person-box overlap.
    fn pack_initial(&mut self) {
        for l in 0..self.order.len() {
            let nodes = self.order[l].clone();
            let mut cursor = 0.0;
            let mut prev: Option<LRef> = None;
            for &v in &nodes {
                if let Some(p) = prev {
                    cursor += self.separation(p, v);
                }
                self.x.insert(v, cursor);
                prev = Some(v);
            }
        }
    }

    /// Each node's target x = the mean x of its neighbours in the reference layer
    /// (the adjacent layer toward the sweep direction); no neighbour → keep current.
    fn desired_x(&self, l: usize, down: bool) -> BTreeMap<LRef, f64> {
        let neighbours = if down { &self.up } else { &self.down };
        let mut desired = BTreeMap::new();
        for &v in &self.order[l] {
            let target = neighbours
                .get(&v)
                .filter(|ns| !ns.is_empty())
                .map(|ns| {
                    let sum: f64 = ns.iter().map(|n| self.x_of(*n)).sum();
                    sum / ns.len() as f64
                })
                .unwrap_or_else(|| self.x_of(v));
            desired.insert(v, target);
        }
        desired
    }

    /// Places a layer's nodes toward `desired`, honouring the fixed left-to-right
    /// order and the minimum separation. `ltr` packs from the left (lower bounds),
    /// otherwise from the right (upper bounds); alternating balances the bias. Either
    /// direction keeps the layer non-overlapping.
    fn place_layer(&mut self, l: usize, desired: &BTreeMap<LRef, f64>, ltr: bool) {
        let nodes = self.order[l].clone();
        if nodes.is_empty() {
            return;
        }
        if ltr {
            let mut prev: Option<LRef> = None;
            let mut prev_x = 0.0;
            for &v in &nodes {
                let want = desired.get(&v).copied().unwrap_or_else(|| self.x_of(v));
                let nx = match prev {
                    Some(p) => want.max(prev_x + self.separation(p, v)),
                    None => want,
                };
                self.x.insert(v, nx);
                prev = Some(v);
                prev_x = nx;
            }
        } else {
            let mut next: Option<LRef> = None;
            let mut next_x = 0.0;
            for &v in nodes.iter().rev() {
                let want = desired.get(&v).copied().unwrap_or_else(|| self.x_of(v));
                let nx = match next {
                    Some(n) => want.min(next_x - self.separation(v, n)),
                    None => want,
                };
                self.x.insert(v, nx);
                next = Some(v);
                next_x = nx;
            }
        }
    }

    /// Builds the placement per real node and the routed-polyline per long edge.
    fn finish(&self, graph: &RelativeGraph) -> (BTreeMap<NodeRef, Placement>, RouteMap) {
        let mut placements: BTreeMap<NodeRef, Placement> = BTreeMap::new();
        for person in &graph.persons {
            let n = LRef::Person(person.node);
            let cx = self.x_of(n);
            let y = layer_top_y(self.layer[&n]);
            placements.insert(
                person.node,
                Placement {
                    x: cx - CARD_WIDTH / 2.0,
                    y,
                    width: CARD_WIDTH,
                    height: CARD_HEIGHT,
                },
            );
        }
        for union in &graph.unions {
            let n = LRef::Union(union.node);
            let cx = self.x_of(n);
            let cy = layer_center_y(self.layer[&n]);
            placements.insert(
                union.node,
                Placement {
                    x: cx - UNION_W / 2.0,
                    y: cy - UNION_H / 2.0,
                    width: UNION_W,
                    height: UNION_H,
                },
            );
        }

        let mut routes: RouteMap = BTreeMap::new();
        for (key, chain) in &self.chains {
            let points: Vec<Point> = chain
                .iter()
                .map(|&d| Point {
                    x: self.x_of(d),
                    y: layer_center_y(self.layer[&d]),
                })
                .collect();
            routes.insert(*key, points);
        }
        (placements, routes)
    }

    /// Required centre-to-centre distance between two adjacent same-layer nodes:
    /// half-widths plus a gap (tighter around routing dummies, so long edges stay
    /// straight; the wider `SUBTREE_GAP` between two cards).
    fn separation(&self, a: LRef, b: LRef) -> f64 {
        let gap = if matches!(a, LRef::Dummy(_)) || matches!(b, LRef::Dummy(_)) {
            SIBLING_GAP
        } else {
            SUBTREE_GAP
        };
        half_width(a) + gap + half_width(b)
    }

    /// Current x of a node (`0.0` if unset — every node is packed before reads).
    fn x_of(&self, n: LRef) -> f64 {
        self.x.get(&n).copied().unwrap_or(0.0)
    }
}

/// The dummy-chain map type alias (`(from, to)` edge → interior waypoints).
type RouteMap = BTreeMap<(NodeRef, NodeRef), Vec<Point>>;

/// A person's current rank (defaulting to 0 for an unseen ref — defensive).
fn rank_of(prank: &BTreeMap<NodeRef, i32>, p: NodeRef) -> i32 {
    prank.get(&p).copied().unwrap_or(0)
}

/// Half the node box width by kind (dummies are zero-width routing points).
fn half_width(n: LRef) -> f64 {
    match n {
        LRef::Person(_) => CARD_WIDTH / 2.0,
        LRef::Union(_) => UNION_W / 2.0,
        LRef::Dummy(_) => 0.0,
    }
}

/// The vertical step between successive person bands.
fn band_step() -> f64 {
    CARD_HEIGHT + RANK_GAP
}

/// The top y of a person band (`layer` even → person rank `layer / 2`).
fn layer_top_y(layer: i32) -> f64 {
    f64::from(layer / 2) * band_step()
}

/// The centre y of any layer: a person band's mid for an even layer, the gap's mid
/// (where unions and dummies live) for an odd one.
fn layer_center_y(layer: i32) -> f64 {
    if layer % 2 == 0 {
        layer_top_y(layer) + CARD_HEIGHT / 2.0
    } else {
        let r = (layer - 1) / 2;
        f64::from(r) * band_step() + CARD_HEIGHT + RANK_GAP / 2.0
    }
}

/// The median of a sorted, non-empty index slice (the average of the two central
/// elements for an even count). `None` for an empty slice.
fn median(sorted: &[usize]) -> Option<f64> {
    let n = sorted.len();
    if n == 0 {
        return None;
    }
    let mid = n / 2;
    if n % 2 == 1 {
        Some(sorted[mid] as f64)
    } else {
        Some((sorted[mid - 1] as f64 + sorted[mid] as f64) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Store;
    use crate::model::{ChildRelation, NewFamily, NewIndividual, PersonId, Sex, UnionType};
    use proptest::prelude::*;

    const EPSILON: f64 = 1e-9;

    /// A node box as top-left `(x, y, width, height)`.
    type BoxRect = (f64, f64, f64, f64);

    fn boxes_overlap(a: BoxRect, b: BoxRect) -> bool {
        let (ax, ay, aw, ah) = a;
        let (bx, by, bw, bh) = b;
        let x = (ax + aw - bx).min(bx + bw - ax);
        let y = (ay + ah - by).min(by + bh - ay);
        x > EPSILON && y > EPSILON
    }

    /// Builds a small random family graph in a fresh in-memory store and returns it
    /// with a focal. Families link persons by index; children always have a larger
    /// index than their parents, so the data is a sensible (acyclic) pedigree.
    fn build_store(n: usize, families: &[(usize, usize, Vec<usize>)]) -> (Store, PersonId) {
        let store = Store::open_in_memory().expect("store");
        let people: Vec<PersonId> = (0..n)
            .map(|i| {
                let sex = if i % 2 == 0 { Sex::Male } else { Sex::Female };
                store
                    .create_individual(&NewIndividual {
                        given_name: Some(format!("P{i}")),
                        sex,
                        living: false,
                        ..Default::default()
                    })
                    .expect("person")
                    .id
            })
            .collect();
        for (p1, p2, kids) in families {
            let family = store
                .create_family(&NewFamily {
                    partner1: Some(people[*p1]),
                    partner2: Some(people[*p2]),
                    union_type: UnionType::Marriage,
                    notes: None,
                })
                .expect("family")
                .id;
            let mut order = 0i64;
            let mut seen = std::collections::BTreeSet::new();
            for &k in kids {
                if !seen.insert(k) {
                    continue; // a child is added to a family once
                }
                store
                    .add_child(family, people[k], ChildRelation::Birth, order)
                    .expect("child");
                order += 1;
            }
        }
        (store, people[0])
    }

    /// A random acyclic family graph spec: `n` persons and a handful of families
    /// whose children always sit below their (lower-indexed) parents.
    fn graph_spec() -> impl Strategy<Value = (usize, Vec<(usize, usize, Vec<usize>)>)> {
        (3usize..=10).prop_flat_map(|n| {
            let family = (0..n, 0..n, proptest::collection::vec(0..n, 0..=3));
            let families = proptest::collection::vec(family, 1..=6);
            (Just(n), families)
        })
    }

    proptest! {
        /// Over random pedigrees: no two person boxes overlap, and the positioner is
        /// deterministic (two runs are byte-identical).
        #[test]
        fn random_pedigrees_never_overlap_and_are_deterministic(
            (n, raw) in graph_spec()
        ) {
            // Normalise indices so partners < children (acyclic, legible).
            let families: Vec<(usize, usize, Vec<usize>)> = raw
                .into_iter()
                .map(|(a, b, kids)| {
                    let p1 = a % n;
                    let p2 = b % n;
                    let base = p1.max(p2);
                    let kids = kids
                        .into_iter()
                        .map(|k| base + 1 + (k % n))
                        .filter(|&k| k < n)
                        .collect();
                    (p1, p2, kids)
                })
                .collect();

            let (store, focal) = build_store(n, &families);
            let graph = crate::query::network(&store, focal).expect("network walk");

            let model = position(&graph);
            let again = position(&graph);
            prop_assert_eq!(&model, &again, "the positioner is deterministic");

            let persons: Vec<BoxRect> = model
                .nodes
                .iter()
                .filter(|nd| nd.kind == super::super::NodeKind::Person)
                .map(|nd| (nd.x, nd.y, nd.width, nd.height))
                .collect();
            for i in 0..persons.len() {
                for j in (i + 1)..persons.len() {
                    prop_assert!(
                        !boxes_overlap(persons[i], persons[j]),
                        "person boxes {} and {} overlap", i, j
                    );
                }
            }
        }
    }
}
