//! Curated re-exports for downstream crates: `use kith_core::prelude::*;`.

pub use crate::date::{DateModifier, GenealogicalDate, PartialDate};
pub use crate::db::{DeleteTarget, Deletion, PartnerSlot, SearchHit, Store, media_root_for};
pub use crate::error::{CoreError, Result};
// `gedcom::export`/`import` stay qualified (a bare `export`/`import` in the glob is
// too generic); only the option/summary types are re-exported here.
pub use crate::gedcom::{ImportOptions, ImportSummary};
// `ChartMode` is re-exported via the `query` line below, not here — `layout`
// re-exports the same `query` type, so importing it from both would clash.
pub use crate::layout::{
    LayoutLink, LayoutModel, LayoutNode, LinkKind, NodeContent, NodeEntity, NodeId, NodeKind,
    Point, Rect, compute_layout,
};
pub use crate::model::{
    ChildLink, ChildRelation, Citation, CitationId, CitationItem, CitationSubject, Confidence,
    Event, EventId, EventKind, EventSubject, Family, FamilyId, Individual, Media, MediaId,
    MediaItem, MediaLink, MediaSubject, Name, NameId, NameKind, NewCitation, NewEvent, NewFamily,
    NewIndividual, NewMedia, NewName, NewPlace, NewSource, PersonId, Place, PlaceId, Sex, Source,
    SourceId, UnionType,
};
// `render::html` stays qualified (a bare `html` in the glob is too generic).
pub use crate::query::{
    ChartMode, ChildView, EventView, FamilyView, MAX_GENERATIONS, NodeRef, PersonNode, PersonView,
    RelEdge, RelativeGraph, SourceView, UnionNode, ancestors, descendants, network, relatives,
};
pub use crate::render::{HtmlExportOptions, Theme};
