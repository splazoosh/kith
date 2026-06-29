//! Unsaved input records ("drafts") for the create side of the `Store` CRUD API.
//!
//! A draft mirrors a [record](super::record) without its server-assigned `id`
//! and without db-managed timestamps. `Store::create_*` consumes a draft by
//! reference and returns the persisted record. `Default` is implemented where
//! the schema has column defaults, so only differing fields need to be set.

use crate::date::GenealogicalDate;

use super::enums::{
    CitationSubject, Confidence, EventKind, EventSubject, NameKind, Sex, UnionType,
};
use super::ids::{PersonId, PlaceId, SourceId};

/// A new individual to insert. `sex` defaults to [`Sex::Unknown`] and `living`
/// to `true`, matching `living INTEGER NOT NULL DEFAULT 1`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewIndividual {
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

impl Default for NewIndividual {
    fn default() -> Self {
        Self {
            given_name: None,
            surname: None,
            name_prefix: None,
            name_suffix: None,
            nickname: None,
            sex: Sex::Unknown,
            living: true, // schema: living NOT NULL DEFAULT 1
            notes: None,
        }
    }
}

/// A new family to insert. `union_type` defaults to [`UnionType::Unknown`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewFamily {
    /// First partner, if recorded.
    pub partner1: Option<PersonId>,
    /// Second partner, if recorded.
    pub partner2: Option<PersonId>,
    /// The nature of the union.
    pub union_type: UnionType,
    /// Free-form notes.
    pub notes: Option<String>,
}

impl Default for NewFamily {
    fn default() -> Self {
        Self {
            partner1: None,
            partner2: None,
            union_type: UnionType::Unknown,
            notes: None,
        }
    }
}

/// A new alternate name to attach to an individual.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewName {
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

/// A new event to insert against exactly one subject. The persistence layer
/// derives every `events.date_*` column from `date`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewEvent {
    /// The individual or family this event is about.
    pub subject: EventSubject,
    /// What kind of event this is.
    pub kind: EventKind,
    /// The (fuzzy) date; `None` if undated.
    pub date: Option<GenealogicalDate>,
    /// Where it happened.
    pub place: Option<PlaceId>,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// A new media row to insert, recording a path verbatim with **no** file copy.
///
/// The GEDCOM importer uses this to record an `OBJE` `FILE` path as-is;
/// interactive imports go through [`Store::import_media`], which copies the
/// bytes into the media folder first and then derives the relative `path`.
///
/// [`Store::import_media`]: crate::db::Store::import_media
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewMedia {
    /// Path **relative** to the media folder, or — for a GEDCOM import — the
    /// `FILE` value recorded verbatim.
    pub path: String,
    /// Optional caption / title.
    pub caption: Option<String>,
    /// MIME type, if known.
    pub mime: Option<String>,
}

/// A new source to insert. `Default` (an empty `title`) is implemented because
/// only `title` is `NOT NULL`, so only the differing fields need to be set.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewSource {
    /// Title of the source (the only required field).
    pub title: String,
    /// Author / originator, if recorded.
    pub author: Option<String>,
    /// Publication facts, if recorded.
    pub publication: Option<String>,
    /// Holding repository, a free-text name (no repository table).
    pub repository: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// A new citation to insert against exactly one fact. The `source` and `subject`
/// are both mandatory (a citation links one source to one fact), so this draft —
/// unlike [`NewSource`] — has no `Default`: a citation cannot exist without them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewCitation {
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

/// A new place to insert (minimal: a free-form name plus optional coordinates
/// and parent place). Richer place handling is a future enhancement.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewPlace {
    /// Free-form full place string (`places.name` is NOT NULL).
    pub name: String,
    /// Latitude, if known.
    pub latitude: Option<f64>,
    /// Longitude, if known.
    pub longitude: Option<f64>,
    /// Enclosing place, if any.
    pub parent: Option<PlaceId>,
}
