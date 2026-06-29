//! The hourglass mode adapter: ancestors above, descendants below, around
//! one shared focus.
//!
//! # The stitch
//!
//! The `relatives` walk produces a single graph with both halves and the focus
//! shared at generation 0. The two half-placement passes are run on that whole
//! graph: each naturally covers only its side, because the focus *partners* only
//! its downward unions (descendants) and is a *child* of only its upward union
//! (ancestors). The descendant half puts the focus at `y ≈ 0` growing positive;
//! the ancestor half is translated so its focus coincides with the descendant
//! focus, which pushes its ancestors to **negative** y (above). The focus is
//! emitted **once** (from the descendant half); `bounds` (recomputed in
//! [`emit`](super::build::emit)) may have a negative `y`.

use std::collections::BTreeMap;

use crate::query::{NodeRef, RelativeGraph};

use super::LayoutModel;
use super::build::{Placement, emit, focus_ref};
use super::{ancestors, descendants};

/// Positions a relatives [`RelativeGraph`] into a stitched hourglass
/// [`LayoutModel`].
pub(crate) fn position(graph: &RelativeGraph) -> LayoutModel {
    let focus = focus_ref(graph);
    let descendant = descendants::placements(graph);
    let mut ancestor = ancestors::placements(graph);

    // Align the ancestor half onto the descendant focus, then drop the ancestor
    // half's focus so the shared person is emitted once (from the descendant
    // half). If either half is missing the focus (it never is), skip the shift.
    if let (Some(&down_focus), Some(&up_focus)) = (descendant.get(&focus), ancestor.get(&focus)) {
        let dx = down_focus.x - up_focus.x;
        let dy = down_focus.y - up_focus.y;
        for place in ancestor.values_mut() {
            place.x += dx;
            place.y += dy;
        }
    }

    let mut merged: BTreeMap<NodeRef, Placement> = descendant;
    for (node, place) in ancestor {
        if node != focus {
            merged.insert(node, place);
        }
    }
    emit(graph, &merged)
}
