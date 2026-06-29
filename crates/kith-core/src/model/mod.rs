//! The domain model: newtype row IDs, the enums backing the schema's TEXT
//! columns, and the record structs that mirror the rows.
//!
//! Relationship collections (a family's children, a person's events) are not
//! fields here — they are composed by the `query` layer. These types
//! are a thin, `serde`-serializable mirror of storage.

mod draft;
mod enums;
mod ids;
mod record;

pub use draft::{
    NewCitation, NewEvent, NewFamily, NewIndividual, NewMedia, NewName, NewPlace, NewSource,
};
pub use enums::{
    ChildRelation, CitationSubject, Confidence, EventKind, EventSubject, MediaSubject, NameKind,
    Sex, UnionType,
};
pub use ids::{CitationId, EventId, FamilyId, MediaId, NameId, PersonId, PlaceId, SourceId};
pub use record::{
    ChildLink, Citation, CitationItem, Event, Family, Individual, Media, MediaItem, MediaLink,
    Name, Place, Source,
};
