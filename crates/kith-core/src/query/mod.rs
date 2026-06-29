//! Read compositions over the [`Store`](crate::db::Store).
//!
//! Two layers, both UI-agnostic:
//!
//! - [`views`] — composite *detail* views ([`PersonView`], [`FamilyView`],
//!   [`EventView`]): a record bundled with its related rows for a `show` panel.
//! - [`walk`] — the bounded relationship walks ([`ancestors`], [`descendants`],
//!   [`relatives`]) that turn the family graph around a focus into an
//!   unpositioned, generation-ranked [`RelativeGraph`], the input the layout
//!   engine positions.

mod views;
mod walk;

pub use views::{ChildView, EventView, FamilyView, PersonView, SourceView};
pub use walk::{
    ChartMode, MAX_GENERATIONS, NodeRef, PersonNode, RelEdge, RelativeGraph, UnionNode, ancestors,
    descendants, network, relatives,
};
