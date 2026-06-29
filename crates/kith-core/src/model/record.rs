//! The domain record structs — a typed mirror of the schema rows.
//!
//! Each struct holds one field per column the entity owns. Relationship
//! collections (a family's children, an individual's events) are loaded on
//! demand by the `query` layer, not embedded here, so these types
//! stay a thin, `serde`-serializable mirror of a row.

use super::enums::{
    ChildRelation, CitationSubject, Confidence, EventKind, EventSubject, MediaSubject, NameKind,
    Sex, UnionType,
};
use super::ids::{CitationId, EventId, FamilyId, MediaId, NameId, PersonId, PlaceId, SourceId};
use crate::date::GenealogicalDate;

/// A person. The **primary** name lives inline (as in the `individuals`
/// table); alternate names are separate [`Name`] rows, loaded on demand.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Individual {
    /// Row id.
    pub id: PersonId,
    /// Primary given name(s).
    pub given_name: Option<String>,
    /// Primary surname.
    pub surname: Option<String>,
    /// Name prefix (e.g. "Dr").
    pub name_prefix: Option<String>,
    /// Name suffix (e.g. "Jr").
    pub name_suffix: Option<String>,
    /// Informal/known-as name.
    pub nickname: Option<String>,
    /// Recorded sex.
    pub sex: Sex,
    /// Privacy flag; drives export redaction.
    pub living: bool,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// An alternate name for an individual (maiden, married, aka, …).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Name {
    /// Row id.
    pub id: NameId,
    /// The individual this name belongs to.
    pub individual_id: PersonId,
    /// What kind of alternate name this is.
    pub kind: NameKind,
    /// Given name(s).
    pub given_name: Option<String>,
    /// Surname.
    pub surname: Option<String>,
    /// Name prefix.
    pub name_prefix: Option<String>,
    /// Name suffix.
    pub name_suffix: Option<String>,
    /// Display order among an individual's alternate names.
    pub sort_order: i64,
}

/// A family / union node joining up to two partners.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Family {
    /// Row id.
    pub id: FamilyId,
    /// First partner, if recorded.
    pub partner1: Option<PersonId>,
    /// Second partner, if recorded.
    pub partner2: Option<PersonId>,
    /// The nature of the union.
    pub union_type: UnionType,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// A child's membership in a family, with relation and birth order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChildLink {
    /// The family the child belongs to.
    pub family_id: FamilyId,
    /// The child.
    pub child_id: PersonId,
    /// How the child is related to the family.
    pub relation: ChildRelation,
    /// Birth order within the family.
    pub sort_order: i64,
}

/// An event belonging to exactly one subject (an individual or a family).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    /// Row id.
    pub id: EventId,
    /// The individual or family this event is about.
    pub subject: EventSubject,
    /// What kind of event this is.
    pub kind: EventKind,
    /// The (fuzzy) date, parsed from `date_original`. `None` if undated.
    pub date: Option<GenealogicalDate>,
    /// Where it happened.
    pub place: Option<PlaceId>,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// A media object — an image file. The bytes live in the media folder beside
/// the database; `path` is **relative** to that folder, keeping the database
/// portable. A `media_links` row attaches it to a subject.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Media {
    /// Row id.
    pub id: MediaId,
    /// Path **relative** to the media folder (e.g. `3.jpg`). For a media row
    /// recorded from a GEDCOM `OBJE`, this is the imported `FILE` value verbatim.
    pub path: String,
    /// Optional caption / title.
    pub caption: Option<String>,
    /// MIME type (e.g. `image/jpeg`), if known.
    pub mime: Option<String>,
}

/// A link attaching a [`Media`] to exactly one subject, optionally marking it
/// the subject's **primary** (portrait) image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaLink {
    /// The media being linked.
    pub media: MediaId,
    /// The individual, family, or event the media belongs to.
    pub subject: MediaSubject,
    /// Whether this is the subject's primary (portrait) image.
    pub is_primary: bool,
}

/// A media item in a subject's gallery: the [`Media`] row plus whether it is the
/// subject's primary image — the shape [`Store::list_media_for`] returns.
///
/// [`Store::list_media_for`]: crate::db::Store::list_media_for
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaItem {
    /// The media row.
    pub media: Media,
    /// Whether this is the subject's primary (portrait) image.
    pub is_primary: bool,
}

/// A source: a piece of evidence (a book, a register, a record set) facts can be
/// cited from. One field per `sources` column; only `title` is `NOT NULL`. The
/// `repository` is a free-text string (no repository table). A
/// [`Citation`] links a source to a fact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Source {
    /// Row id.
    pub id: SourceId,
    /// Title of the source (the only required field).
    pub title: String,
    /// Author / originator, if recorded.
    pub author: Option<String>,
    /// Publication facts (publisher, date, place), if recorded.
    pub publication: Option<String>,
    /// Holding repository, a free-text name (no repository table).
    pub repository: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// A citation linking a [`Source`] to exactly one fact (an individual, a family,
/// or an event). The `source` and the `subject` are orthogonal: `source` is the
/// evidence, `subject` is the fact it supports. The "exactly one subject"
/// discipline lives in [`CitationSubject`] (the `citations` table has no `CHECK`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Citation {
    /// Row id.
    pub id: CitationId,
    /// The source being cited.
    pub source: SourceId,
    /// The fact this citation supports.
    pub subject: CitationSubject,
    /// Where in the source (page / entry), if recorded.
    pub page: Option<String>,
    /// A transcription / extra detail, if recorded.
    pub detail: Option<String>,
    /// Confidence in the evidence (`None` = unspecified).
    pub confidence: Option<Confidence>,
}

/// A citation with its [`Source`] resolved alongside — the shape
/// [`Store::citations_for`] returns, so a provenance row renders the source
/// title without a second fetch (no N+1).
///
/// [`Store::citations_for`]: crate::db::Store::citations_for
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CitationItem {
    /// The citation row.
    pub citation: Citation,
    /// The cited source, resolved.
    pub source: Source,
}

/// A place: a free-form location string with optional coordinates and an
/// optional enclosing place.
///
/// Drops `Eq` from its derives (and so does [`NewPlace`](super::NewPlace))
/// because it holds `f64` coordinates, which are `PartialEq` but not `Eq`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Place {
    /// Row id.
    pub id: PlaceId,
    /// Free-form full place string.
    pub name: String,
    /// Latitude, if known.
    pub latitude: Option<f64>,
    /// Longitude, if known.
    pub longitude: Option<f64>,
    /// Enclosing place, if any.
    pub parent: Option<PlaceId>,
}
