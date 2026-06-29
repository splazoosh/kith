//! Domain enums backing the schema's TEXT columns.
//!
//! Closed enums map 1:1 to a fixed set of TEXT codes and parse-don't-validate
//! at the boundary: an unknown code becomes [`CoreError::Validation`], never a
//! silent default. [`EventKind`] is open — its [`EventKind::Other`] arm
//! preserves any imported code verbatim. [`EventSubject`] is a model-only join
//! with no column of its own.

use std::str::FromStr;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use crate::error::CoreError;

use super::ids::{EventId, FamilyId, PersonId};

// `CitationSubject` (below) names the same three fact subjects as `MediaSubject`.

/// Generates a closed string-backed enum with its TEXT mapping.
macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $variant:ident => $code:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash,
            serde::Serialize, serde::Deserialize,
        )]
        pub enum $name {
            $( #[doc = concat!("Stored as `", $code, "`.")] $variant ),+
        }

        impl $name {
            /// The TEXT code stored in SQLite.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $code ),+ }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = CoreError;
            /// Parses the SQLite TEXT code into this enum.
            ///
            /// # Errors
            /// Returns [`CoreError::Validation`] if `s` is not a known code.
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $code => Ok(Self::$variant), )+
                    other => Err(CoreError::Validation(
                        format!(concat!("unknown ", stringify!($name), " code {:?}"), other),
                    )),
                }
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.as_str()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                let s = value.as_str()?;
                s.parse().map_err(|e| FromSqlError::Other(Box::new(e)))
            }
        }
    };
}

text_enum!(
    /// Biological/recorded sex. `Other` and `Unknown` are first-class, not gaps.
    Sex { Male => "M", Female => "F", Other => "X", Unknown => "U" }
);

text_enum!(
    /// How a child is related to the family they appear in.
    ChildRelation { Birth => "birth", Adopted => "adopted", Step => "step", Foster => "foster" }
);

text_enum!(
    /// The nature of a family/union.
    UnionType { Marriage => "marriage", Partnership => "partnership", Unknown => "unknown" }
);

text_enum!(
    /// The kind of an alternate name row.
    NameKind { Birth => "birth", Married => "married", Aka => "aka", Religious => "religious" }
);

text_enum!(
    /// A citation's confidence in the evidence it records.
    ///
    /// Closed, like [`Sex`]/[`NameKind`]: stored as the schema's TEXT codes
    /// (`primary`/`secondary`/`questionable`) and serialized by **variant name**
    /// (`"Primary"`) over the wire. The `citations.confidence` column is nullable,
    /// so a citation carries `Option<Confidence>` (NULL = unspecified). GEDCOM
    /// `QUAY` maps onto these in [`crate::gedcom`] (the lossy `0`→`Questionable`
    /// fold is documented there).
    Confidence { Primary => "primary", Secondary => "secondary", Questionable => "questionable" }
);

/// The kind of an event. Known kinds map to fixed codes; anything else is
/// preserved verbatim in [`EventKind::Other`] so no imported data is lost.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    /// Birth.
    Birth,
    /// Death.
    Death,
    /// Marriage (a family event).
    Marriage,
    /// Divorce (a family event).
    Divorce,
    /// Baptism / christening.
    Baptism,
    /// Burial.
    Burial,
    /// Residence.
    Residence,
    /// Occupation.
    Occupation,
    /// Any other kind, stored as-is.
    Other(String),
}

impl EventKind {
    /// The TEXT code stored in SQLite. For [`EventKind::Other`] this borrows
    /// the inner string, so the call allocates nothing.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Birth => "birth",
            Self::Death => "death",
            Self::Marriage => "marriage",
            Self::Divorce => "divorce",
            Self::Baptism => "baptism",
            Self::Burial => "burial",
            Self::Residence => "residence",
            Self::Occupation => "occupation",
            Self::Other(s) => s,
        }
    }
}

impl From<&str> for EventKind {
    fn from(s: &str) -> Self {
        match s {
            "birth" => Self::Birth,
            "death" => Self::Death,
            "marriage" => Self::Marriage,
            "divorce" => Self::Divorce,
            "baptism" => Self::Baptism,
            "burial" => Self::Burial,
            "residence" => Self::Residence,
            "occupation" => Self::Occupation,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl ::core::fmt::Display for EventKind {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ToSql for EventKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for EventKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Ok(Self::from(value.as_str()?))
    }
}

/// The single subject of an [`Event`](super::record::Event): an individual
/// **or** a family, mirroring the schema's `CHECK ((individual_id IS NULL) <> (family_id IS NULL))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EventSubject {
    /// The event belongs to an individual.
    Individual(PersonId),
    /// The event belongs to a family.
    Family(FamilyId),
}

impl EventSubject {
    /// The `individual_id` column value (`Some` only for the individual case).
    #[must_use]
    pub const fn individual_id(self) -> Option<PersonId> {
        match self {
            Self::Individual(id) => Some(id),
            Self::Family(_) => None,
        }
    }

    /// The `family_id` column value (`Some` only for the family case).
    #[must_use]
    pub const fn family_id(self) -> Option<FamilyId> {
        match self {
            Self::Family(id) => Some(id),
            Self::Individual(_) => None,
        }
    }
}

/// The single subject a piece of [`Media`](super::record::Media) is linked to:
/// an individual, a family, **or** an event, mirroring the `media_links` table's
/// three nullable foreign keys (exactly one is set on any row, the same
/// "exactly one subject" discipline [`EventSubject`] enforces for events).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MediaSubject {
    /// The media is linked to an individual.
    Individual(PersonId),
    /// The media is linked to a family.
    Family(FamilyId),
    /// The media is linked to an event.
    Event(EventId),
}

impl MediaSubject {
    /// The `individual_id` column value (`Some` only for the individual case).
    #[must_use]
    pub const fn individual_id(self) -> Option<PersonId> {
        match self {
            Self::Individual(id) => Some(id),
            Self::Family(_) | Self::Event(_) => None,
        }
    }

    /// The `family_id` column value (`Some` only for the family case).
    #[must_use]
    pub const fn family_id(self) -> Option<FamilyId> {
        match self {
            Self::Family(id) => Some(id),
            Self::Individual(_) | Self::Event(_) => None,
        }
    }

    /// The `event_id` column value (`Some` only for the event case).
    #[must_use]
    pub const fn event_id(self) -> Option<EventId> {
        match self {
            Self::Event(id) => Some(id),
            Self::Individual(_) | Self::Family(_) => None,
        }
    }
}

/// The single fact a [`Citation`](super::record::Citation) attaches a source to:
/// an individual, a family, **or** an event, mirroring the `citations` table's
/// three nullable foreign keys. Unlike `events`, the table carries **no `CHECK`**,
/// so the "exactly one subject" discipline lives here — a [`CitationSubject`]
/// cannot express zero or two subjects, so a citation row always writes exactly
/// one fact FK (the `source_id` is a *separate*, mandatory field — a citation
/// links one source to one fact). Mirrors [`MediaSubject`] exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CitationSubject {
    /// The citation supports a fact about an individual.
    Individual(PersonId),
    /// The citation supports a fact about a family.
    Family(FamilyId),
    /// The citation supports an event (the primary fact target).
    Event(EventId),
}

impl CitationSubject {
    /// The `individual_id` column value (`Some` only for the individual case).
    #[must_use]
    pub const fn individual_id(self) -> Option<PersonId> {
        match self {
            Self::Individual(id) => Some(id),
            Self::Family(_) | Self::Event(_) => None,
        }
    }

    /// The `family_id` column value (`Some` only for the family case).
    #[must_use]
    pub const fn family_id(self) -> Option<FamilyId> {
        match self {
            Self::Family(id) => Some(id),
            Self::Individual(_) | Self::Event(_) => None,
        }
    }

    /// The `event_id` column value (`Some` only for the event case).
    #[must_use]
    pub const fn event_id(self) -> Option<EventId> {
        match self {
            Self::Event(id) => Some(id),
            Self::Individual(_) | Self::Family(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trips every variant of a closed enum through `Display`/`FromStr`
    /// and through a bare in-memory SQLite column.
    macro_rules! assert_closed_enum_round_trips {
        ($ty:ty, [ $( $variant:expr ),+ $(,)? ]) => {{
            let conn = rusqlite::Connection::open_in_memory().expect("open in-memory");
            conn.execute_batch("CREATE TABLE t (x TEXT);")
                .expect("create table");
            $(
                let v: $ty = $variant;
                // Display <-> FromStr
                let parsed: $ty = v.as_str().parse().expect("parse own code");
                assert_eq!(parsed, v);
                assert_eq!(v.to_string(), v.as_str());
                // ToSql -> FromSql through SQLite
                conn.execute("DELETE FROM t;", []).expect("clear");
                conn.execute("INSERT INTO t (x) VALUES (?1);", [v])
                    .expect("insert");
                let back: $ty = conn
                    .query_row("SELECT x FROM t;", [], |row| row.get(0))
                    .expect("read back");
                assert_eq!(back, v);
            )+
        }};
    }

    #[test]
    fn closed_enums_round_trip_through_text_and_sqlite() {
        assert_closed_enum_round_trips!(Sex, [Sex::Male, Sex::Female, Sex::Other, Sex::Unknown]);
        assert_closed_enum_round_trips!(
            ChildRelation,
            [
                ChildRelation::Birth,
                ChildRelation::Adopted,
                ChildRelation::Step,
                ChildRelation::Foster,
            ]
        );
        assert_closed_enum_round_trips!(
            UnionType,
            [
                UnionType::Marriage,
                UnionType::Partnership,
                UnionType::Unknown
            ]
        );
        assert_closed_enum_round_trips!(
            NameKind,
            [
                NameKind::Birth,
                NameKind::Married,
                NameKind::Aka,
                NameKind::Religious
            ]
        );
        assert_closed_enum_round_trips!(
            Confidence,
            [
                Confidence::Primary,
                Confidence::Secondary,
                Confidence::Questionable
            ]
        );
    }

    #[test]
    fn confidence_serializes_by_variant_name_but_stores_the_text_code() {
        // The [enum-JSON-uses-variant-names] split: serde uses the variant name,
        // SQLite stores the lowercase TEXT code.
        let json = serde_json::to_string(&Confidence::Primary).expect("serialize");
        assert_eq!(json, "\"Primary\"");
        assert_eq!(Confidence::Primary.as_str(), "primary");
        let back: Confidence = serde_json::from_str("\"Questionable\"").expect("deserialize");
        assert_eq!(back, Confidence::Questionable);
    }

    #[test]
    fn citation_subject_exposes_the_matching_column_only() {
        let individual = CitationSubject::Individual(PersonId::new(7));
        assert_eq!(individual.individual_id(), Some(PersonId::new(7)));
        assert_eq!(individual.family_id(), None);
        assert_eq!(individual.event_id(), None);

        let family = CitationSubject::Family(FamilyId::new(3));
        assert_eq!(family.family_id(), Some(FamilyId::new(3)));
        assert_eq!(family.individual_id(), None);
        assert_eq!(family.event_id(), None);

        let event = CitationSubject::Event(EventId::new(9));
        assert_eq!(event.event_id(), Some(EventId::new(9)));
        assert_eq!(event.individual_id(), None);
        assert_eq!(event.family_id(), None);
    }

    #[test]
    fn unknown_code_maps_to_validation_error() {
        let err = "Q".parse::<Sex>().expect_err("Q is not a Sex code");
        assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
        let err = "spouse"
            .parse::<ChildRelation>()
            .expect_err("not a relation");
        assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn event_kind_known_codes_map_both_ways() {
        for (code, kind) in [
            ("birth", EventKind::Birth),
            ("death", EventKind::Death),
            ("marriage", EventKind::Marriage),
            ("divorce", EventKind::Divorce),
            ("baptism", EventKind::Baptism),
            ("burial", EventKind::Burial),
            ("residence", EventKind::Residence),
            ("occupation", EventKind::Occupation),
        ] {
            assert_eq!(EventKind::from(code), kind);
            assert_eq!(kind.as_str(), code);
        }
    }

    #[test]
    fn event_kind_other_round_trips_verbatim() {
        // Arrange
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory");
        conn.execute_batch("CREATE TABLE t (x TEXT);")
            .expect("create table");
        let kind = EventKind::Other("residence-ish".to_owned());

        // Act / Assert — through From<&str>/as_str
        assert_eq!(EventKind::from("residence-ish"), kind);
        assert_eq!(kind.as_str(), "residence-ish");

        // Act / Assert — through SQLite
        conn.execute("INSERT INTO t (x) VALUES (?1);", [&kind])
            .expect("insert");
        let back: EventKind = conn
            .query_row("SELECT x FROM t;", [], |row| row.get(0))
            .expect("read back");
        assert_eq!(back, kind);
    }

    #[test]
    fn event_subject_exposes_the_matching_column_only() {
        let individual = EventSubject::Individual(PersonId::new(7));
        assert_eq!(individual.individual_id(), Some(PersonId::new(7)));
        assert_eq!(individual.family_id(), None);

        let family = EventSubject::Family(FamilyId::new(3));
        assert_eq!(family.family_id(), Some(FamilyId::new(3)));
        assert_eq!(family.individual_id(), None);
    }

    #[test]
    fn media_subject_exposes_the_matching_column_only() {
        let individual = MediaSubject::Individual(PersonId::new(7));
        assert_eq!(individual.individual_id(), Some(PersonId::new(7)));
        assert_eq!(individual.family_id(), None);
        assert_eq!(individual.event_id(), None);

        let family = MediaSubject::Family(FamilyId::new(3));
        assert_eq!(family.family_id(), Some(FamilyId::new(3)));
        assert_eq!(family.individual_id(), None);
        assert_eq!(family.event_id(), None);

        let event = MediaSubject::Event(EventId::new(9));
        assert_eq!(event.event_id(), Some(EventId::new(9)));
        assert_eq!(event.individual_id(), None);
        assert_eq!(event.family_id(), None);
    }
}
