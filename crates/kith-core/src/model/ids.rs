//! Strongly-typed row identifiers.
//!
//! Each ID is a `Copy` newtype over the SQLite `i64` primary key. Distinct
//! types stop a [`PersonId`] from being passed where a [`FamilyId`] is meant
//! (`type-newtype-ids`, `api-newtype-safety`). All eight share one generated
//! impl set: `serde` transparency, `Display`, and `rusqlite` `ToSql`/`FromSql`.

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

/// Generates a newtype ID over `i64` with the full impl set.
macro_rules! newtype_id {
    ($(#[$meta:meta])* $name:ident, $entity:literal) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            serde::Serialize, serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(i64);

        impl $name {
            /// The entity kind this ID names, for error messages.
            pub const ENTITY: &'static str = $entity;

            /// Wraps a raw row id.
            #[must_use]
            pub const fn new(id: i64) -> Self {
                Self(id)
            }

            /// Returns the underlying row id.
            #[must_use]
            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.0))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                <i64 as FromSql>::column_result(value).map(Self)
            }
        }
    };
}

newtype_id!(/// Identifies a row in `individuals`.
    PersonId, "individual");
newtype_id!(/// Identifies a row in `families`.
    FamilyId, "family");
newtype_id!(/// Identifies a row in `events`.
    EventId, "event");
newtype_id!(/// Identifies a row in `places`.
    PlaceId, "place");
newtype_id!(/// Identifies a row in `names`.
    NameId, "name");
newtype_id!(/// Identifies a row in `sources`.
    SourceId, "source");
newtype_id!(/// Identifies a row in `citations`.
    CitationId, "citation");
newtype_id!(/// Identifies a row in `media`.
    MediaId, "media");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_id_round_trips_through_sqlite_i64() {
        // Arrange
        let conn = rusqlite::Connection::open_in_memory().expect("open in-memory");
        conn.execute_batch("CREATE TABLE t (x INTEGER);")
            .expect("create table");
        let id = PersonId::new(42);

        // Act
        conn.execute("INSERT INTO t (x) VALUES (?1);", [id])
            .expect("insert id");
        let back: PersonId = conn
            .query_row("SELECT x FROM t;", [], |row| row.get(0))
            .expect("read id");

        // Assert
        assert_eq!(back, id);
        assert_eq!(back.get(), 42);
    }

    #[test]
    fn newtype_id_serializes_as_a_bare_number() {
        // Arrange / Act
        let json = serde_json::to_string(&PersonId::new(5)).expect("serialize");

        // Assert
        assert_eq!(json, "5", "#[serde(transparent)] yields a bare number");
        let back: PersonId = serde_json::from_str("5").expect("deserialize");
        assert_eq!(back, PersonId::new(5));
    }

    #[test]
    fn entity_label_is_exposed_for_error_messages() {
        assert_eq!(PersonId::ENTITY, "individual");
        assert_eq!(FamilyId::ENTITY, "family");
    }
}
