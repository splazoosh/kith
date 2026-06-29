//! CRUD over the `events` table (and minimal `places` for `events.place_id`).
//!
//! The date seam is the crux of this module: on write, all six `date_*`
//! columns are derived from the parsed [`GenealogicalDate`]; on read,
//! [`Event::date`](crate::model::Event) is reconstructed by *re-parsing*
//! `date_original`. This round-trips faithfully because the date module's
//! `proptest` suite proves `parse(format(d)) == d`, so the denormalized
//! `date_modifier`/`date_sort`/`date_month`/`date_day` columns are write-only
//! here. [`Store::vital_years`] is the first reader of the denormalized layer:
//! it reads `date_year`/`date_sort` for the query walks' lifespan years.

use rusqlite::{OptionalExtension, params};

use crate::date::GenealogicalDate;
use crate::error::{CoreError, Result};
use crate::model::{
    Event, EventId, EventKind, EventSubject, FamilyId, NewEvent, NewPlace, PersonId, Place, PlaceId,
};

use super::Store;

const COLUMNS: &str = "id, individual_id, family_id, kind, date_original, place_id, notes";

/// The raw column shape selected for an event, before domain reconstruction.
type EventRow = (
    EventId,
    Option<PersonId>,
    Option<FamilyId>,
    EventKind,
    Option<String>, // date_original
    Option<PlaceId>,
    Option<String>, // notes
);

/// The six denormalized `events.date_*` columns derived from a date:
/// `(date_original, date_modifier, date_sort, date_year, date_month, date_day)`.
///
/// `pub(super)` so the undo/restore path ([`super::undo`]) can re-derive the same
/// columns when re-inserting a captured event with its original id.
pub(super) type DateColumns = (
    Option<String>,
    Option<&'static str>,
    Option<i64>,
    Option<i32>,
    Option<u8>,
    Option<u8>,
);

/// Derives the denormalized `date_*` columns from a (possibly absent) date.
/// `date_original` is the date's canonical [`Display`](std::fmt::Display) form.
///
/// `pub(super)` so [`super::undo`] re-derives them identically on restore.
pub(super) fn derive_date_columns(date: Option<GenealogicalDate>) -> DateColumns {
    match date {
        Some(d) => {
            let be = d.best_estimate();
            (
                Some(d.to_string()),
                Some(d.modifier().as_str()),
                d.sort_key(),
                Some(be.year),
                be.month,
                be.day,
            )
        }
        None => (None, None, None, None, None, None),
    }
}

/// Selects the [`EventRow`] columns from a `&Row`.
fn event_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRow> {
    Ok((
        row.get("id")?,
        row.get("individual_id")?,
        row.get("family_id")?,
        row.get("kind")?,
        row.get("date_original")?,
        row.get("place_id")?,
        row.get("notes")?,
    ))
}

/// Reconstructs an [`Event`] from its raw row: rebuilds the XOR subject and
/// re-parses `date_original` into the typed date.
fn row_to_event((id, ind, fam, kind, date_original, place, notes): EventRow) -> Result<Event> {
    let subject = match (ind, fam) {
        (Some(i), None) => EventSubject::Individual(i),
        (None, Some(f)) => EventSubject::Family(f),
        // The schema CHECK forbids both/neither; treat the impossible row as
        // corrupt data rather than panicking inside the core.
        _ => {
            return Err(CoreError::Validation(format!(
                "event {id} has an invalid subject (the schema CHECK should forbid this)"
            )));
        }
    };
    let date = date_original
        .as_deref()
        .map(str::parse::<GenealogicalDate>)
        .transpose()?; // CoreError::Validation on a malformed stored string
    Ok(Event {
        id,
        subject,
        kind,
        date,
        place,
        notes,
    })
}

impl Store {
    /// Adds an event against exactly one subject, deriving the denormalized
    /// `date_*` columns from `draft.date`.
    ///
    /// `date_original` stores the date's canonical [`Display`](std::fmt::Display)
    /// form; the typed date is reconstructed from it on read (a faithful
    /// round-trip, since `parse(format(d)) == d`).
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert
    /// fails (e.g. a foreign-key violation on a non-existent subject/place).
    pub fn add_event(&self, draft: &NewEvent) -> Result<Event> {
        let conn = self.conn()?;
        Self::add_event_in(&conn, draft)
    }

    /// The `add_event` INSERT body, callable on any connection (a pooled
    /// connection or a transaction) — the transactional twin behind the public
    /// auto-commit method. Derives the denormalized `date_*` columns exactly as
    /// [`add_event`](Self::add_event) does.
    ///
    /// # Errors
    /// Returns [`CoreError`] if the insert fails (e.g. a foreign-key violation on
    /// a non-existent subject/place).
    pub(crate) fn add_event_in(conn: &rusqlite::Connection, draft: &NewEvent) -> Result<Event> {
        let (date_original, date_modifier, date_sort, date_year, date_month, date_day) =
            derive_date_columns(draft.date);
        conn.execute(
            "INSERT INTO events
                (individual_id, family_id, kind, date_original, date_modifier,
                 date_sort, date_year, date_month, date_day, place_id, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                draft.subject.individual_id(),
                draft.subject.family_id(),
                draft.kind,
                date_original,
                date_modifier,
                date_sort,
                date_year,
                date_month,
                date_day,
                draft.place,
                draft.notes,
            ],
        )?;
        Ok(Event {
            id: EventId::new(conn.last_insert_rowid()),
            subject: draft.subject,
            kind: draft.kind.clone(),
            date: draft.date,
            place: draft.place,
            notes: draft.notes.clone(),
        })
    }

    /// Fetches an event by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such event exists, [`CoreError::Validation`]
    /// if a stored `date_original` is malformed, or another [`CoreError`] on failure.
    pub fn get_event(&self, id: EventId) -> Result<Event> {
        let conn = self.conn()?;
        let row = conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM events WHERE id = ?1"),
                [id],
                event_columns,
            )
            .optional()?;
        match row {
            Some(r) => row_to_event(r),
            None => Err(CoreError::NotFound {
                entity: EventId::ENTITY,
                id: id.get(),
            }),
        }
    }

    /// Lists every event for a subject, undated rows last, then chronological.
    ///
    /// # Errors
    /// Returns [`CoreError::Validation`] if a stored `date_original` is
    /// malformed, or another [`CoreError`] on a connection/query failure.
    pub fn list_events_for(&self, subject: EventSubject) -> Result<Vec<Event>> {
        let conn = self.conn()?;
        let (where_col, id) = match subject {
            EventSubject::Individual(p) => ("individual_id", p.get()),
            EventSubject::Family(f) => ("family_id", f.get()),
        };
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM events
             WHERE {where_col} = ?1
             ORDER BY date_sort IS NULL, date_sort, id"
        ))?;
        let rows = stmt
            .query_map([id], event_columns)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().map(row_to_event).collect()
    }

    /// Updates an event's kind, derived date columns, place, and notes. The
    /// subject is immutable once set; changing it is a delete + [`add_event`](Self::add_event).
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `ev.id`, or another
    /// [`CoreError`] on failure.
    pub fn update_event(&self, ev: &Event) -> Result<()> {
        let (date_original, date_modifier, date_sort, date_year, date_month, date_day) =
            derive_date_columns(ev.date);
        let conn = self.conn()?;
        let n = conn.execute(
            "UPDATE events SET
                kind=?1, date_original=?2, date_modifier=?3, date_sort=?4,
                date_year=?5, date_month=?6, date_day=?7, place_id=?8, notes=?9
             WHERE id=?10",
            params![
                ev.kind,
                date_original,
                date_modifier,
                date_sort,
                date_year,
                date_month,
                date_day,
                ev.place,
                ev.notes,
                ev.id,
            ],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: EventId::ENTITY,
                id: ev.id.get(),
            });
        }
        Ok(())
    }

    /// Deletes an event.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `id`, or another
    /// [`CoreError`] on failure.
    pub fn delete_event(&self, id: EventId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM events WHERE id=?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: EventId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }

    /// Inserts a place, returning its assigned id.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert fails.
    pub fn create_place(&self, draft: &NewPlace) -> Result<PlaceId> {
        let conn = self.conn()?;
        Self::create_place_in(&conn, draft)
    }

    /// The `create_place` INSERT body, callable on any connection (a pooled
    /// connection or a transaction) — the transactional twin behind the public
    /// auto-commit method.
    ///
    /// # Errors
    /// Returns [`CoreError`] if the insert fails.
    pub(crate) fn create_place_in(
        conn: &rusqlite::Connection,
        draft: &NewPlace,
    ) -> Result<PlaceId> {
        conn.execute(
            "INSERT INTO places (name, latitude, longitude, parent_id) VALUES (?1, ?2, ?3, ?4)",
            params![draft.name, draft.latitude, draft.longitude, draft.parent],
        )?;
        Ok(PlaceId::new(conn.last_insert_rowid()))
    }

    /// Fetches a place by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such place exists, or another
    /// [`CoreError`] on a connection/query failure.
    pub fn get_place(&self, id: PlaceId) -> Result<Place> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, name, latitude, longitude, parent_id FROM places WHERE id = ?1",
            [id],
            |row| {
                Ok(Place {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    latitude: row.get("latitude")?,
                    longitude: row.get("longitude")?,
                    parent: row.get("parent_id")?,
                })
            },
        )
        .optional()?
        .ok_or(CoreError::NotFound {
            entity: PlaceId::ENTITY,
            id: id.get(),
        })
    }

    /// The best-estimate **birth** and **death** years for `person`, read from
    /// the denormalized `events.date_year` (the first reader of that column —
    /// see the module header). For each kind the earliest *dated* event wins
    /// (`ORDER BY date_sort, id`); undated events (`date_year IS NULL`) and a
    /// person with no birth/death event yield `None`. One query, no re-parse.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn vital_years(&self, person: PersonId) -> Result<(Option<i32>, Option<i32>)> {
        let conn = self.conn()?;
        Self::vital_years_on(&conn, person)
    }

    /// The [`vital_years`](Self::vital_years) read on a caller-supplied
    /// connection, using a **cached** prepared statement — the read twin a
    /// relationship walk routes its per-person lookups through. Same
    /// SQL and ordering → identical result.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a query failure.
    pub(crate) fn vital_years_on(
        conn: &rusqlite::Connection,
        person: PersonId,
    ) -> Result<(Option<i32>, Option<i32>)> {
        let mut stmt = conn.prepare_cached(
            "SELECT kind, date_year FROM events
             WHERE individual_id = ?1
               AND kind IN ('birth','death')
               AND date_year IS NOT NULL
             ORDER BY date_sort, id",
        )?;
        let rows = stmt.query_map([person], |row| {
            Ok((
                row.get::<_, String>("kind")?,
                row.get::<_, i32>("date_year")?,
            ))
        })?;
        let mut birth = None;
        let mut death = None;
        for row in rows {
            let (kind, year) = row?;
            match kind.as_str() {
                "birth" if birth.is_none() => birth = Some(year),
                "death" if death.is_none() => death = Some(year),
                _ => {}
            }
        }
        Ok((birth, death))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NewIndividual;

    fn subject(store: &Store) -> EventSubject {
        let id = store
            .create_individual(&NewIndividual::default())
            .expect("create individual")
            .id;
        EventSubject::Individual(id)
    }

    #[test]
    fn add_get_update_delete_round_trip_preserves_the_date() {
        let store = Store::open_in_memory().expect("open store");
        let subj = subject(&store);
        let date: GenealogicalDate = "12 Mar 1850".parse().expect("parse date");

        let added = store
            .add_event(&NewEvent {
                subject: subj,
                kind: EventKind::Birth,
                date: Some(date),
                place: None,
                notes: None,
            })
            .expect("add");
        assert_eq!(store.get_event(added.id).expect("get").date, Some(date));

        let new_date: GenealogicalDate = "ABT 1851".parse().expect("parse date");
        let updated = Event {
            date: Some(new_date),
            ..added
        };
        store.update_event(&updated).expect("update");
        assert_eq!(
            store.get_event(added.id).expect("re-get").date,
            Some(new_date)
        );

        store.delete_event(added.id).expect("delete");
        assert!(matches!(
            store.get_event(added.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn undated_event_has_a_null_date() {
        let store = Store::open_in_memory().expect("open store");
        let subj = subject(&store);
        let added = store
            .add_event(&NewEvent {
                subject: subj,
                kind: EventKind::Residence,
                date: None,
                place: None,
                notes: None,
            })
            .expect("add");
        assert_eq!(store.get_event(added.id).expect("get").date, None);
    }

    #[test]
    fn events_list_dated_before_undated() {
        let store = Store::open_in_memory().expect("open store");
        let subj = subject(&store);
        let undated = |store: &Store| {
            store
                .add_event(&NewEvent {
                    subject: subj,
                    kind: EventKind::Residence,
                    date: None,
                    place: None,
                    notes: None,
                })
                .expect("add undated");
        };
        let dated = |store: &Store, s: &str| {
            store
                .add_event(&NewEvent {
                    subject: subj,
                    kind: EventKind::Residence,
                    date: Some(s.parse().expect("parse")),
                    place: None,
                    notes: None,
                })
                .expect("add dated");
        };
        undated(&store);
        dated(&store, "1900");
        dated(&store, "1850");

        let dates: Vec<_> = store
            .list_events_for(subj)
            .expect("list")
            .into_iter()
            .map(|e| e.date)
            .collect();
        assert_eq!(dates[0], Some("1850".parse().unwrap()));
        assert_eq!(dates[1], Some("1900".parse().unwrap()));
        assert_eq!(dates[2], None, "undated rows sort last");
    }

    #[test]
    fn vital_years_picks_earliest_birth_and_death() {
        // Arrange — a person with two recorded births (keep the earliest), one
        // death, and an undated residence that must not leak into the result.
        let store = Store::open_in_memory().expect("open store");
        let subj = subject(&store);
        let add = |date: Option<&str>, kind: EventKind| {
            store
                .add_event(&NewEvent {
                    subject: subj,
                    kind,
                    date: date.map(|s| s.parse().expect("parse")),
                    place: None,
                    notes: None,
                })
                .expect("add event");
        };
        add(Some("1850"), EventKind::Birth);
        add(Some("ABT 1851"), EventKind::Birth); // a later, fuzzier birth — ignored
        add(Some("12 Mar 1910"), EventKind::Death);
        add(None, EventKind::Residence);

        // Act / Assert
        let years = store
            .vital_years(subj.individual_id().expect("person subject"))
            .expect("vital years");
        assert_eq!(years, (Some(1850), Some(1910)));
    }

    #[test]
    fn vital_years_is_none_when_undated_or_absent() {
        let store = Store::open_in_memory().expect("open store");
        // A person with no events at all.
        let bare = store
            .create_individual(&NewIndividual::default())
            .expect("create");
        assert_eq!(
            store.vital_years(bare.id).expect("vital years"),
            (None, None)
        );

        // A person whose only birth event is undated → still (None, None).
        let subj = subject(&store);
        store
            .add_event(&NewEvent {
                subject: subj,
                kind: EventKind::Birth,
                date: None,
                place: None,
                notes: None,
            })
            .expect("add undated birth");
        let years = store
            .vital_years(subj.individual_id().expect("person subject"))
            .expect("vital years");
        assert_eq!(years, (None, None));
    }

    #[test]
    fn place_round_trips_with_coordinates() {
        let store = Store::open_in_memory().expect("open store");
        let id = store
            .create_place(&NewPlace {
                name: "Oslo, Norway".to_owned(),
                latitude: Some(59.9139),
                longitude: Some(10.7522),
                parent: None,
            })
            .expect("create place");
        let place = store.get_place(id).expect("get place");
        assert_eq!(place.name, "Oslo, Norway");
        assert_eq!(place.latitude, Some(59.9139));
        assert!(matches!(
            store.get_place(PlaceId::new(999)),
            Err(CoreError::NotFound { .. })
        ));
    }
}
