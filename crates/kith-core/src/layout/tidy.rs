//! A pure, deterministic tidy-tree positioner — the linear-time Buchheim–Walker
//! improvement of Reingold–Tilford / Walker (Buchheim, Jünger & Leipert,
//! *"Improving Walker's Algorithm to Run in Linear Time"*, GD 2002).
//!
//! The core is **orientation-neutral**: it lays a tree out *downward* (y grows
//! with depth) with non-overlapping subtrees, each parent centred over its
//! children. The mode adapters build the arena from a
//! [`RelativeGraph`](crate::query::RelativeGraph) and flip/offset the result for
//! ancestors and hourglass — the core never grows a mode parameter. It takes
//! **no [`Store`](crate::db::Store)**: a plain arena in, positions out, so the
//! no-overlap guarantee is unit- and property-tested without a database.
//!
//! # Algorithm (per the paper)
//! 1. **first walk** (post-order): a leaf takes a preliminary x from its left
//!    sibling; an internal node is centred over its children after `apportion`
//!    threads the left/right subtree **contours** and shifts overlapping subtrees
//!    (the non-overlap guarantee), accumulating per-node `modifier`s.
//! 2. **second walk** (pre-order): each node's absolute x = `prelim` + Σ ancestor
//!    modifiers.
//!
//! Separation between two adjacent contour nodes is **size-aware**:
//! `half_width(left) + gap + half_width(right)`, with `gap = sibling_gap` for
//! same-parent neighbours else `subtree_gap` — so wide person cards and
//! tiny union joiners never collide. y is **per level**: each depth's band height
//! is the tallest node at that depth, bands stacked top-down with `rank_gap`.
//!
//! The Buchheim scratch (`prelim`, `modifier`, `thread`, `ancestor`, `change`,
//! `shift`, sibling `number`) lives in parallel arrays indexed by arena node, so
//! the input [`TidyTree`] stays read-only.
//!
//! The mode adapters ([`descendants`](super::descendants),
//! [`ancestors`](super::ancestors), [`hourglass`](super::hourglass)) build the
//! arena from a [`RelativeGraph`](crate::query::RelativeGraph) and call
//! [`layout`]; the in-module tests retire the geometry in isolation.

use std::collections::VecDeque;

use super::{Point, Rect};

/// One node of the abstract sized tree (persons-vs-unions agnostic).
pub(crate) struct TidyNode {
    /// Parent arena index (`None` for the root).
    pub parent: Option<usize>,
    /// Child arena indices, in final left-to-right order.
    pub children: Vec<usize>,
    /// Node box width (logical units).
    pub width: f64,
    /// Node box height (logical units).
    pub height: f64,
}

/// The arena to position.
pub(crate) struct TidyTree {
    /// All nodes; `root` indexes the single root.
    pub nodes: Vec<TidyNode>,
    /// The root arena index.
    pub root: usize,
}

/// Spacing policy.
pub(crate) struct TidyOptions {
    /// Horizontal gap between siblings (same parent).
    pub sibling_gap: f64,
    /// Horizontal gap between adjacent nodes in different subtrees.
    pub subtree_gap: f64,
    /// Vertical gap between generation bands.
    pub rank_gap: f64,
}

/// The positioned result.
pub(crate) struct TidyLayout {
    /// `positions[i]` is node `i`'s **centre x, band-top y**: its box spans
    /// `(x - width/2 ..= x + width/2)` horizontally and `(y ..= y + height)`
    /// vertically. Centre-x makes "parent centred over children" a direct `x`
    /// comparison and lets an adapter draw a card from `(x - w/2, y)`.
    pub positions: Vec<Point>,
    /// The tight union of all node boxes.
    pub extent: Rect,
}

/// Positions `tree` with the given spacing. Deterministic: identical arena +
/// options ⇒ identical output. Recursion is bounded by tree height (the adapters
/// keep it ≤ 2·`MAX_GENERATIONS`), so the stack is safe.
pub(crate) fn layout(tree: &TidyTree, opts: &TidyOptions) -> TidyLayout {
    let n = tree.nodes.len();
    let mut state = Tidy::new(tree, opts);

    // x: the two Buchheim walks.
    state.first_walk(tree.root);
    let depth = state.depths();
    let band_top = state.band_tops(&depth);
    state.second_walk(tree.root, 0.0, 0, &depth, &band_top);

    // extent: the tight union of every node box (centre-x, band-top-y boxes).
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (node, pos) in tree.nodes.iter().zip(&state.positions) {
        let half = node.width / 2.0;
        min_x = min_x.min(pos.x - half);
        max_x = max_x.max(pos.x + half);
        min_y = min_y.min(pos.y);
        max_y = max_y.max(pos.y + node.height);
    }
    // `n >= 1` (the root always exists), so the sentinels are always replaced.
    debug_assert!(n >= 1, "a TidyTree always has at least its root");
    let extent = Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };

    TidyLayout {
        positions: state.positions,
        extent,
    }
}

/// The Buchheim scratch state: parallel arrays over the arena plus the read-only
/// `tree`/`opts`. Kept off [`TidyNode`] so the input arena stays clean.
struct Tidy<'a> {
    tree: &'a TidyTree,
    opts: &'a TidyOptions,
    /// Preliminary x, relative to the subtree (resolved in the second walk).
    prelim: Vec<f64>,
    /// Accumulated horizontal shift applied to a node's whole subtree.
    modifier: Vec<f64>,
    /// The contour thread: the next node along a subtree's left/right outline.
    thread: Vec<Option<usize>>,
    /// The greatest distinct ancestor used by `apportion` (defaults to self).
    ancestor: Vec<usize>,
    /// Deferred shift bookkeeping (smoothing pushes across many subtrees).
    change: Vec<f64>,
    /// Deferred shift bookkeeping (the cumulative push).
    shift: Vec<f64>,
    /// 1-based index of a node within its parent's `children`.
    number: Vec<usize>,
    /// Resolved centre-x / band-top-y per node (filled by the second walk).
    positions: Vec<Point>,
}

impl<'a> Tidy<'a> {
    fn new(tree: &'a TidyTree, opts: &'a TidyOptions) -> Self {
        let n = tree.nodes.len();
        let mut number = vec![1usize; n];
        for node in &tree.nodes {
            for (i, &child) in node.children.iter().enumerate() {
                number[child] = i + 1;
            }
        }
        Self {
            tree,
            opts,
            prelim: vec![0.0; n],
            modifier: vec![0.0; n],
            thread: vec![None; n],
            ancestor: (0..n).collect(),
            change: vec![0.0; n],
            shift: vec![0.0; n],
            number,
            positions: vec![Point { x: 0.0, y: 0.0 }; n],
        }
    }

    fn children(&self, v: usize) -> &[usize] {
        &self.tree.nodes[v].children
    }

    /// The node immediately left of `v` among its siblings, if any.
    fn left_sibling(&self, v: usize) -> Option<usize> {
        let parent = self.tree.nodes[v].parent?;
        let siblings = &self.tree.nodes[parent].children;
        let idx = siblings.iter().position(|&c| c == v)?;
        idx.checked_sub(1).map(|prev| siblings[prev])
    }

    /// The leftmost sibling of `v` (its parent's first child); `v` itself when it
    /// has no parent. Only called for nodes that have a left sibling, so the
    /// parent (and thus a first child) always exists in practice.
    fn leftmost_sibling(&self, v: usize) -> usize {
        match self.tree.nodes[v].parent {
            Some(parent) => self.tree.nodes[parent].children[0],
            None => v,
        }
    }

    /// Required centre-to-centre distance between two adjacent contour nodes:
    /// half-widths plus the gap, which is `sibling_gap` for same-parent
    /// neighbours and `subtree_gap` otherwise.
    fn separation(&self, left: usize, right: usize) -> f64 {
        let same_parent = self.tree.nodes[left].parent == self.tree.nodes[right].parent;
        let gap = if same_parent {
            self.opts.sibling_gap
        } else {
            self.opts.subtree_gap
        };
        self.tree.nodes[left].width / 2.0 + gap + self.tree.nodes[right].width / 2.0
    }

    /// Down a left contour: a node's leftmost child, else its thread.
    fn next_left(&self, v: usize) -> Option<usize> {
        match self.children(v).first() {
            Some(&first) => Some(first),
            None => self.thread[v],
        }
    }

    /// Down a right contour: a node's rightmost child, else its thread.
    fn next_right(&self, v: usize) -> Option<usize> {
        match self.children(v).last() {
            Some(&last) => Some(last),
            None => self.thread[v],
        }
    }

    /// First walk (post-order): preliminary x and modifiers, with `apportion`
    /// guaranteeing non-overlapping subtrees.
    fn first_walk(&mut self, v: usize) {
        if self.children(v).is_empty() {
            self.prelim[v] = match self.left_sibling(v) {
                Some(w) => self.prelim[w] + self.separation(w, v),
                None => 0.0,
            };
            return;
        }
        let children = self.children(v).to_vec();
        let mut default_ancestor = children[0];
        for &w in &children {
            self.first_walk(w);
            default_ancestor = self.apportion(w, default_ancestor);
        }
        self.execute_shifts(v);
        let leftmost = children[0];
        let rightmost = children[children.len() - 1];
        let midpoint = (self.prelim[leftmost] + self.prelim[rightmost]) / 2.0;
        match self.left_sibling(v) {
            Some(w) => {
                self.prelim[v] = self.prelim[w] + self.separation(w, v);
                self.modifier[v] = self.prelim[v] - midpoint;
            }
            None => self.prelim[v] = midpoint,
        }
    }

    /// Threads the contours of `v`'s subtree against its left siblings and shifts
    /// `v` right by however much they would otherwise overlap.
    fn apportion(&mut self, v: usize, mut default_ancestor: usize) -> usize {
        let Some(w) = self.left_sibling(v) else {
            return default_ancestor;
        };
        // "inside"/"outside" contours of the right (v) and left (siblings) trees.
        let mut vip = v; // inside right
        let mut vop = v; // outside right
        let mut vim = w; // inside left
        let mut vom = self.leftmost_sibling(v); // outside left
        let mut sip = self.modifier[vip];
        let mut sop = self.modifier[vop];
        let mut sim = self.modifier[vim];
        let mut som = self.modifier[vom];

        while let (Some(next_vim), Some(next_vip)) = (self.next_right(vim), self.next_left(vip)) {
            vim = next_vim;
            vip = next_vip;
            // The outer contours stay in lockstep with the inner ones.
            if let Some(next_vom) = self.next_left(vom) {
                vom = next_vom;
            }
            if let Some(next_vop) = self.next_right(vop) {
                vop = next_vop;
            }
            self.ancestor[vop] = v;
            let shift =
                (self.prelim[vim] + sim) - (self.prelim[vip] + sip) + self.separation(vim, vip);
            if shift > 0.0 {
                let wm = self.ancestor_or_default(vim, v, default_ancestor);
                self.move_subtree(wm, v, shift);
                sip += shift;
                sop += shift;
            }
            sim += self.modifier[vim];
            sip += self.modifier[vip];
            som += self.modifier[vom];
            sop += self.modifier[vop];
        }

        // Thread the shorter contour onto the taller one so later siblings see it.
        if self.next_right(vim).is_some() && self.next_right(vop).is_none() {
            self.thread[vop] = self.next_right(vim);
            self.modifier[vop] += sim - sop;
        }
        if self.next_left(vip).is_some() && self.next_left(vom).is_none() {
            self.thread[vom] = self.next_left(vip);
            self.modifier[vom] += sip - som;
            default_ancestor = v;
        }
        default_ancestor
    }

    /// The left contour's ancestor if it is a sibling of `v`, else the running
    /// default — the node whose subtree a shift must move.
    fn ancestor_or_default(&self, vim: usize, v: usize, default_ancestor: usize) -> usize {
        let candidate = self.ancestor[vim];
        if self.tree.nodes[candidate].parent == self.tree.nodes[v].parent {
            candidate
        } else {
            default_ancestor
        }
    }

    /// Shifts the subtree rooted at `wp` right by `shift`, smoothing the push
    /// across the `wm..wp` sibling span via the `change`/`shift` ledgers.
    fn move_subtree(&mut self, wm: usize, wp: usize, shift: f64) {
        // `wp` is to the right of `wm`, so their 1-based numbers differ by ≥ 1;
        // `.max(1)` is a belt-and-braces guard against a zero divide.
        let subtrees = self.number[wp].saturating_sub(self.number[wm]).max(1) as f64;
        self.change[wp] -= shift / subtrees;
        self.shift[wp] += shift;
        self.change[wm] += shift / subtrees;
        self.prelim[wp] += shift;
        self.modifier[wp] += shift;
    }

    /// Applies the deferred `shift`/`change` ledgers to `v`'s children, right to
    /// left, so the smoothed pushes land as even spacing.
    fn execute_shifts(&mut self, v: usize) {
        let children = self.children(v).to_vec();
        let mut shift = 0.0;
        let mut change = 0.0;
        for &w in children.iter().rev() {
            self.prelim[w] += shift;
            self.modifier[w] += shift;
            change += self.change[w];
            shift += self.shift[w] + change;
        }
    }

    /// Depth of every node (root = 0), by a BFS down the children.
    fn depths(&self) -> Vec<usize> {
        let mut depth = vec![0usize; self.tree.nodes.len()];
        let mut queue = VecDeque::from([self.tree.root]);
        while let Some(v) = queue.pop_front() {
            for &child in self.children(v) {
                depth[child] = depth[v] + 1;
                queue.push_back(child);
            }
        }
        depth
    }

    /// The top y of each depth band: a band's height is the tallest node at that
    /// depth, bands stacked from `0` with `rank_gap` between them.
    fn band_tops(&self, depth: &[usize]) -> Vec<f64> {
        let max_depth = depth.iter().copied().max().unwrap_or(0);
        let mut band_height = vec![0.0f64; max_depth + 1];
        for (node, &d) in self.tree.nodes.iter().zip(depth) {
            band_height[d] = band_height[d].max(node.height);
        }
        let mut band_top = vec![0.0f64; max_depth + 1];
        for d in 1..=max_depth {
            band_top[d] = band_top[d - 1] + band_height[d - 1] + self.opts.rank_gap;
        }
        band_top
    }

    /// Second walk (pre-order): resolve absolute x = `prelim` + Σ ancestor
    /// modifiers, and set y from the node's depth band.
    fn second_walk(&mut self, v: usize, m: f64, d: usize, depth: &[usize], band_top: &[f64]) {
        self.positions[v] = Point {
            x: self.prelim[v] + m,
            y: band_top[d],
        };
        let children = self.children(v).to_vec();
        let child_m = m + self.modifier[v];
        for &w in &children {
            self.second_walk(w, child_m, depth[w], depth, band_top);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::metrics::{CARD_HEIGHT, CARD_WIDTH, RANK_GAP, SIBLING_GAP, SUBTREE_GAP};
    use super::{TidyNode, TidyOptions, TidyTree, layout};
    use proptest::prelude::*;

    const EPSILON: f64 = 1e-9;

    /// The uniform spacing the adapters will use.
    fn uniform_opts() -> TidyOptions {
        TidyOptions {
            sibling_gap: SIBLING_GAP,
            subtree_gap: SUBTREE_GAP,
            rank_gap: RANK_GAP,
        }
    }

    fn leaf(parent: usize, width: f64) -> TidyNode {
        TidyNode {
            parent: Some(parent),
            children: Vec::new(),
            width,
            height: CARD_HEIGHT,
        }
    }

    fn assert_close(actual: f64, expected: f64, what: &str) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{what}: expected {expected}, got {actual}",
        );
    }

    /// A 3-level uniform tree — root → 2 children → each 2 leaves — positions to
    /// the coordinates a by-hand Reingold–Tilford gives. Indices:
    /// `0`=root, `1`=A, `2`=B, `3`=a1, `4`=a2, `5`=b1, `6`=b2.
    ///
    /// Note: because `SUBTREE_GAP > SIBLING_GAP`, the two *cousin* leaves (a2, b1)
    /// sit a wider gap apart than same-parent siblings — so the four leaves are
    /// **not** evenly spaced by `CARD_WIDTH + SIBLING_GAP`; the size-aware
    /// separation widens cousin gaps. The
    /// centring invariants still hold exactly.
    #[test]
    fn three_level_uniform_tree_matches_hand_computed_coordinates() {
        // Arrange
        let nodes = vec![
            TidyNode {
                parent: None,
                children: vec![1, 2],
                width: CARD_WIDTH,
                height: CARD_HEIGHT,
            },
            TidyNode {
                parent: Some(0),
                children: vec![3, 4],
                width: CARD_WIDTH,
                height: CARD_HEIGHT,
            },
            TidyNode {
                parent: Some(0),
                children: vec![5, 6],
                width: CARD_WIDTH,
                height: CARD_HEIGHT,
            },
            leaf(1, CARD_WIDTH),
            leaf(1, CARD_WIDTH),
            leaf(2, CARD_WIDTH),
            leaf(2, CARD_WIDTH),
        ];
        let tree = TidyTree { nodes, root: 0 };

        // Act
        let out = layout(&tree, &uniform_opts());
        let p = &out.positions;

        // Assert — leaf x's: same-parent gap = W + sibling, cousin gap = W + subtree.
        let sibling_step = CARD_WIDTH + SIBLING_GAP;
        let cousin_step = CARD_WIDTH + SUBTREE_GAP;
        assert_close(p[3].x, 0.0, "a1.x");
        assert_close(p[4].x, sibling_step, "a2.x");
        assert_close(p[5].x, sibling_step + cousin_step, "b1.x");
        assert_close(p[6].x, 2.0 * sibling_step + cousin_step, "b2.x");

        // Each parent centred over its two leaves; the root over the parents.
        assert_close(p[1].x, (p[3].x + p[4].x) / 2.0, "A centred over a1,a2");
        assert_close(p[2].x, (p[5].x + p[6].x) / 2.0, "B centred over b1,b2");
        assert_close(p[0].x, (p[1].x + p[2].x) / 2.0, "root centred over A,B");

        // y bands at 0, H+rank, 2·(H+rank).
        let rank_step = CARD_HEIGHT + RANK_GAP;
        assert_close(p[0].y, 0.0, "root.y");
        assert_close(p[1].y, rank_step, "A.y");
        assert_close(p[2].y, rank_step, "B.y");
        for pos in &p[3..=6] {
            assert_close(pos.y, 2.0 * rank_step, "leaf.y");
        }
    }

    /// Two adjacent siblings of very different widths keep at least the
    /// size-aware separation (`half(wide) + sibling_gap + half(narrow)`).
    #[test]
    fn adjacent_siblings_respect_size_aware_separation() {
        // Arrange — a root with a wide and a narrow leaf child.
        let wide = 300.0;
        let narrow = 40.0;
        let nodes = vec![
            TidyNode {
                parent: None,
                children: vec![1, 2],
                width: CARD_WIDTH,
                height: CARD_HEIGHT,
            },
            leaf(0, wide),
            leaf(0, narrow),
        ];
        let tree = TidyTree { nodes, root: 0 };

        // Act
        let out = layout(&tree, &uniform_opts());

        // Assert
        let gap = out.positions[2].x - out.positions[1].x;
        let required = wide / 2.0 + SIBLING_GAP + narrow / 2.0;
        assert_close(gap, required, "wide→narrow centre distance");
        assert!(gap + EPSILON >= required, "separation must not shrink");
    }

    /// A single node lays out at the origin with its own box as the extent.
    #[test]
    fn singleton_tree_sits_at_the_origin() {
        let nodes = vec![TidyNode {
            parent: None,
            children: Vec::new(),
            width: CARD_WIDTH,
            height: CARD_HEIGHT,
        }];
        let tree = TidyTree { nodes, root: 0 };

        let out = layout(&tree, &uniform_opts());

        assert_close(out.positions[0].x, 0.0, "lone node x");
        assert_close(out.positions[0].y, 0.0, "lone node y");
        assert_close(out.extent.width, CARD_WIDTH, "extent width");
        assert_close(out.extent.height, CARD_HEIGHT, "extent height");
    }

    // --- property-based net: no overlap, parent centring, determinism --------

    /// A node box as `(centre_x, width, top_y, height)`.
    type NodeBox = (f64, f64, f64, f64);

    /// True when two centre-x / band-top-y boxes overlap by more than `EPSILON`
    /// in both axes (mere touching is not an overlap).
    fn boxes_overlap(a: NodeBox, b: NodeBox) -> bool {
        let (ax, aw, ay, ah) = a;
        let (bx, bw, by, bh) = b;
        let (al, ar, at, ab) = (ax - aw / 2.0, ax + aw / 2.0, ay, ay + ah);
        let (bl, br, bt, bb) = (bx - bw / 2.0, bx + bw / 2.0, by, by + bh);
        let x_overlap = (ar - bl).min(br - al);
        let y_overlap = (ab - bt).min(bb - at);
        x_overlap > EPSILON && y_overlap > EPSILON
    }

    /// Builds an arena from `raw_parents` (node `i>0`'s parent is
    /// `raw_parents[i] % i`, guaranteeing a parent index `< i` ⇒ a real tree) and
    /// per-node `sizes`.
    fn build_tree(raw_parents: &[usize], sizes: &[(f64, f64)]) -> TidyTree {
        let n = sizes.len();
        let mut nodes: Vec<TidyNode> = sizes
            .iter()
            .map(|&(width, height)| TidyNode {
                parent: None,
                children: Vec::new(),
                width,
                height,
            })
            .collect();
        for child in 1..n {
            let parent = raw_parents[child] % child;
            nodes[child].parent = Some(parent);
            nodes[parent].children.push(child);
        }
        TidyTree { nodes, root: 0 }
    }

    fn arena_strategy() -> impl Strategy<Value = (Vec<usize>, Vec<(f64, f64)>)> {
        (1usize..=24).prop_flat_map(|n| {
            let parents = proptest::collection::vec(0usize..10_000, n);
            let sizes = proptest::collection::vec((10.0f64..200.0, 10.0f64..100.0), n);
            (parents, sizes)
        })
    }

    proptest! {
        /// No two node boxes overlap, and the layout is deterministic.
        #[test]
        fn random_trees_never_overlap_and_are_deterministic(
            (raw_parents, sizes) in arena_strategy()
        ) {
            let tree = build_tree(&raw_parents, &sizes);
            let opts = uniform_opts();

            let out = layout(&tree, &opts);

            // No pair of boxes overlaps.
            for i in 0..tree.nodes.len() {
                for j in (i + 1)..tree.nodes.len() {
                    let a = &tree.nodes[i];
                    let b = &tree.nodes[j];
                    let pa = out.positions[i];
                    let pb = out.positions[j];
                    prop_assert!(
                        !boxes_overlap(
                            (pa.x, a.width, pa.y, a.height),
                            (pb.x, b.width, pb.y, b.height),
                        ),
                        "nodes {i} and {j} overlap",
                    );
                }
            }

            // Determinism: the same arena lays out identically.
            let again = layout(&tree, &opts);
            for (first, second) in out.positions.iter().zip(&again.positions) {
                prop_assert_eq!(first.x, second.x);
                prop_assert_eq!(first.y, second.y);
            }
        }

        /// Every parent's centre x lies within its children's centre span.
        #[test]
        fn parents_are_centred_over_their_children(
            (raw_parents, sizes) in arena_strategy()
        ) {
            let tree = build_tree(&raw_parents, &sizes);

            let out = layout(&tree, &uniform_opts());

            for (v, node) in tree.nodes.iter().enumerate() {
                if node.children.is_empty() {
                    continue;
                }
                let xs = node.children.iter().map(|&c| out.positions[c].x);
                let min = xs.clone().fold(f64::INFINITY, f64::min);
                let max = xs.fold(f64::NEG_INFINITY, f64::max);
                let parent_x = out.positions[v].x;
                prop_assert!(
                    parent_x >= min - EPSILON && parent_x <= max + EPSILON,
                    "parent {v} at {parent_x} outside child span [{min}, {max}]",
                );
            }
        }
    }
}
