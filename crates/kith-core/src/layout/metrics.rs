//! Uniform layout metrics in logical units. The canvas and HTML export mirror
//! these as CSS design tokens so the live and exported charts match. Values are
//! a starting palette, tunable without an API change — consumers read computed
//! positions, never these.
//!
//! These constants are the palette the mode adapters feed the tidy core (as
//! [`TidyOptions`](super::tidy::TidyOptions)) and use to size each card.

/// Person card width.
pub(crate) const CARD_WIDTH: f64 = 220.0;
/// Person card height.
pub(crate) const CARD_HEIGHT: f64 = 72.0;
/// Gap between siblings (children of one union).
pub(crate) const SIBLING_GAP: f64 = 24.0;
/// Gap between adjacent nodes in different subtrees.
pub(crate) const SUBTREE_GAP: f64 = 48.0;
/// Vertical gap between generation bands.
pub(crate) const RANK_GAP: f64 = 64.0;
/// Union joiner width.
pub(crate) const UNION_W: f64 = 16.0;
/// Union joiner height.
pub(crate) const UNION_H: f64 = 16.0;
/// Portrait avatar diameter — a circular portrait drawn **inside** the existing
/// card box (portraits do not change layout geometry). The card
/// size above is unchanged; the avatar overlays its left edge.
pub(crate) const PORTRAIT_D: f64 = 48.0;
/// Inset of the portrait avatar from the card's left and the gap before the name.
pub(crate) const PORTRAIT_INSET: f64 = 12.0;
