//! Undoable deletes: snapshot-then-delete and explicit-id restore.
//!
//! The eight destructive actions in the editor (deleting a person, family, event,
//! alternate name, child link, source, citation, or media) are made reversible by
//! a pair of transactional methods on [`Store`]:
//!
//! - [`Store::delete_undoable`] reads the affected row **and its entire cascade
//!   set** into a serializable [`Deletion`], then runs the same single `DELETE`
//!   the per-entity `delete_*` methods run — all in **one** transaction.
//! - [`Store::restore_deletion`] re-inserts every captured row **with its original
//!   id** in **one** transaction, re-setting the `families.partner{1,2}_id`
//!   pointers a person-delete had `SET NULL`.
//!
//! Because every table is `INTEGER PRIMARY KEY` (a rowid alias, **not**
//! `AUTOINCREMENT`), a freed id is reusable; an explicit-id restore therefore
//! brings a row — and every foreign key that referenced it — back exactly. The
//! one risk is **id reuse**: delete the highest-id row, create a new row (which
//! takes that id), then restore → the explicit-id `INSERT` hits a `PRIMARY KEY`
//! conflict. `restore_deletion` runs in a single transaction, so the conflict rolls
//! back cleanly (no partial restore) and surfaces as a typed [`CoreError`].
//!
//! ## The capture set runs as deep as the cascade
//!
//! A person-delete cascades further than the obvious one level: it removes the
//! person's *individual events*, and each of those events in turn cascades its
//! **own** citations and `media_links`. So [`Deletion::Individual`] (and
//! [`Deletion::Family`]) capture each event together with its citations and media
//! links ([`CapturedEvent`]) — otherwise undo would silently lose a sourced birth.
//! Places are referenced `ON DELETE SET NULL` and no delete in scope removes a
//! place, so only the `place_id` is captured, never the place row. Media files on
//! disk are left in place by [`Store::delete_media`], so a media undo restores
//! rows only — the bytes are still there.
//!
//! All reads happen on the **transaction's** connection (the in-memory pool holds
//! a single connection, so a `&self` reader called mid-transaction would deadlock
//! on it). The row mappers are local to this module so a future schema column is a
//! compile error here, not a silent omission.

use rusqlite::{Connection, OptionalExtension, params};

use crate::date::GenealogicalDate;
use crate::error::{CoreError, Result};
use crate::model::{
    ChildLink, Citation, CitationId, CitationSubject, Confidence, Event, EventId, EventKind,
    EventSubject, Family, FamilyId, Individual, Media, MediaId, MediaLink, MediaSubject, Name,
    NameId, PersonId, PlaceId, Sex, Source, SourceId, UnionType,
};

use super::Store;
use super::event::derive_date_columns;

/// Which destructive action to perform undoably — one variant per delete path the
/// editor exposes. Exhaustively matched, so adding a ninth delete is a compile
/// error until it is handled here and in the restore dispatch (`pat-exhaustive-enum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DeleteTarget {
    /// Delete an individual (cascades names / memberships / individual events /
    /// citations / media links; `SET NULL`s partner pointers).
    Individual(PersonId),
    /// Delete a family (cascades memberships / family events / citations / media
    /// links; partners untouched).
    Family(FamilyId),
    /// Delete an event (cascades its citations / media links).
    Event(EventId),
    /// Remove an alternate name.
    Name(NameId),
    /// Remove a child's membership in a family.
    Child {
        /// The family the child belongs to.
        family: FamilyId,
        /// The child being removed.
        child: PersonId,
    },
    /// Delete a source (cascades its citations).
    Source(SourceId),
    /// Delete a citation.
    Citation(CitationId),
    /// Delete a media object (cascades its links).
    Media(MediaId),
}

/// Which partner slot of a [`Family`] a restored individual occupied, so
/// [`Store::restore_deletion`] can re-set the pointer a person-delete had `SET NULL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PartnerSlot {
    /// `families.partner1_id`.
    One,
    /// `families.partner2_id`.
    Two,
}

/// An event captured with the rows that cascade with it — its citations and media
/// links — so a person/family/event delete restores fully.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CapturedEvent {
    /// The event row.
    pub event: Event,
    /// Citations whose subject is this event.
    pub citations: Vec<Citation>,
    /// Media links whose subject is this event.
    pub media_links: Vec<MediaLink>,
}

/// The captured rows for a deleted individual (the [`Individual`] plus its full
/// cascade set and the partner-slot pointers to restore).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndividualDeletion {
    /// The individual row.
    pub individual: Individual,
    /// The `created_at` timestamp (not on [`Individual`]; captured for a byte-identical restore).
    pub created_at: String,
    /// The `updated_at` timestamp.
    pub updated_at: String,
    /// The person's alternate names.
    pub names: Vec<Name>,
    /// The person's `family_children`-as-child memberships.
    pub child_links: Vec<ChildLink>,
    /// The person's individual events (each with its own cascade set).
    pub events: Vec<CapturedEvent>,
    /// Citations whose subject is this person directly.
    pub citations: Vec<Citation>,
    /// Media links whose subject is this person directly.
    pub media_links: Vec<MediaLink>,
    /// The `(family, slot)` partner pointers the delete had `SET NULL`.
    pub partner_slots: Vec<(FamilyId, PartnerSlot)>,
}

/// The captured rows for a deleted family.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FamilyDeletion {
    /// The family row.
    pub family: Family,
    /// The `created_at` timestamp.
    pub created_at: String,
    /// The `updated_at` timestamp.
    pub updated_at: String,
    /// The family's child memberships.
    pub child_links: Vec<ChildLink>,
    /// The family's events (each with its own cascade set).
    pub events: Vec<CapturedEvent>,
    /// Citations whose subject is this family directly.
    pub citations: Vec<Citation>,
    /// Media links whose subject is this family directly.
    pub media_links: Vec<MediaLink>,
}

/// The captured rows for a deleted source (the source plus its cascaded citations).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceDeletion {
    /// The source row.
    pub source: Source,
    /// Citations that cited this source.
    pub citations: Vec<Citation>,
}

/// The captured rows for a deleted media object (the media plus its cascaded links).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaDeletion {
    /// The media row.
    pub media: Media,
    /// Links that attached this media to a subject.
    pub links: Vec<MediaLink>,
}

/// A captured deletion: every row a [`Store::delete_undoable`] removed, enough for
/// [`Store::restore_deletion`] to bring it (and its relationships) back with original ids.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Deletion {
    /// An individual and its cascade set.
    Individual(IndividualDeletion),
    /// A family and its cascade set.
    Family(FamilyDeletion),
    /// An event and its cascade set.
    Event(CapturedEvent),
    /// An alternate name.
    Name(Name),
    /// A child membership.
    Child(ChildLink),
    /// A source and its cascaded citations.
    Source(SourceDeletion),
    /// A citation.
    Citation(Citation),
    /// A media object and its cascaded links.
    Media(MediaDeletion),
}

impl Store {
    /// Snapshots `target`'s row(s) **and their cascade set**, then deletes the
    /// target — both in one transaction — returning the [`Deletion`] that
    /// [`Store::restore`] can replay.
    ///
    /// The reads run on the transaction's connection (so a single-connection
    /// in-memory pool never deadlocks), then the same single `DELETE` the
    /// per-entity `delete_*` method runs fires the foreign-key cascade.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if `target` does not exist (mirroring the
    /// per-entity deletes), [`CoreError::Validation`] if a captured row is corrupt
    /// (a malformed stored date, or a citation/media link with not-exactly-one
    /// subject), or [`CoreError::Database`] on a SQL failure.
    pub fn delete_undoable(&self, target: DeleteTarget) -> Result<Deletion> {
        self.transaction(|conn| match target {
            DeleteTarget::Individual(id) => capture_individual(conn, id).map(Deletion::Individual),
            DeleteTarget::Family(id) => capture_family(conn, id).map(Deletion::Family),
            DeleteTarget::Event(id) => {
                let captured = capture_event_reads(conn, read_event(conn, id)?)?;
                delete_one(conn, "events", id.get(), EventId::ENTITY)?;
                Ok(Deletion::Event(captured))
            }
            DeleteTarget::Name(id) => {
                let name = read_name(conn, id)?;
                delete_one(conn, "names", id.get(), NameId::ENTITY)?;
                Ok(Deletion::Name(name))
            }
            DeleteTarget::Child { family, child } => {
                let link = read_child_link(conn, family, child)?;
                let n = conn.execute(
                    "DELETE FROM family_children WHERE family_id=?1 AND child_id=?2",
                    params![family, child],
                )?;
                if n == 0 {
                    return Err(not_found(FamilyId::ENTITY, family.get()));
                }
                Ok(Deletion::Child(link))
            }
            DeleteTarget::Source(id) => {
                let source = read_source(conn, id)?;
                let citations = read_citations(conn, "source_id", id.get())?;
                delete_one(conn, "sources", id.get(), SourceId::ENTITY)?;
                Ok(Deletion::Source(SourceDeletion { source, citations }))
            }
            DeleteTarget::Citation(id) => {
                let citation = read_citation(conn, id)?;
                delete_one(conn, "citations", id.get(), CitationId::ENTITY)?;
                Ok(Deletion::Citation(citation))
            }
            DeleteTarget::Media(id) => {
                let media = read_media(conn, id)?;
                let links = read_media_links_of_media(conn, id)?;
                delete_one(conn, "media", id.get(), MediaId::ENTITY)?;
                Ok(Deletion::Media(MediaDeletion { media, links }))
            }
        })
    }

    /// Re-inserts every row a [`Deletion`] captured **with its original id**, in
    /// one transaction, re-setting any `families.partner{1,2}_id` pointers a
    /// person-delete had `SET NULL`.
    ///
    /// Named `restore_deletion` (not `restore`) because [`Store::restore`] already
    /// names the database-file restore in `maintenance` — a different concern.
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a `PRIMARY KEY` conflict (the id was
    /// reused since the delete — the transaction rolls back, leaving the database
    /// unchanged) or any other SQL failure.
    pub fn restore_deletion(&self, deletion: &Deletion) -> Result<()> {
        self.transaction(|conn| match deletion {
            Deletion::Individual(d) => restore_individual(conn, d),
            Deletion::Family(d) => restore_family(conn, d),
            Deletion::Event(e) => restore_event(conn, e),
            Deletion::Name(n) => insert_name(conn, n),
            Deletion::Child(c) => insert_child(conn, c),
            Deletion::Source(d) => {
                insert_source(conn, &d.source)?;
                for c in &d.citations {
                    insert_citation(conn, c)?;
                }
                Ok(())
            }
            Deletion::Citation(c) => insert_citation(conn, c),
            Deletion::Media(d) => {
                insert_media(conn, &d.media)?;
                for link in &d.links {
                    insert_media_link(conn, link)?;
                }
                Ok(())
            }
        })
    }
}

// — capture (read + delete the target) ————————————————————————————————————————

/// Captures an individual's full cascade set, then deletes the individual.
fn capture_individual(conn: &Connection, id: PersonId) -> Result<IndividualDeletion> {
    let (individual, created_at, updated_at) = read_individual(conn, id)?;
    let names = read_names_of(conn, id)?;
    let child_links = read_child_links(conn, "child_id", id.get())?;
    let events = read_events(conn, "individual_id", id.get())?
        .into_iter()
        .map(|e| capture_event_reads(conn, e))
        .collect::<Result<Vec<_>>>()?;
    let citations = read_citations(conn, "individual_id", id.get())?;
    let media_links = read_media_links(conn, "individual_id", id.get())?;
    let partner_slots = read_partner_slots(conn, id)?;
    delete_one(conn, "individuals", id.get(), PersonId::ENTITY)?;
    Ok(IndividualDeletion {
        individual,
        created_at,
        updated_at,
        names,
        child_links,
        events,
        citations,
        media_links,
        partner_slots,
    })
}

/// Captures a family's full cascade set, then deletes the family.
fn capture_family(conn: &Connection, id: FamilyId) -> Result<FamilyDeletion> {
    let (family, created_at, updated_at) = read_family(conn, id)?;
    let child_links = read_child_links(conn, "family_id", id.get())?;
    let events = read_events(conn, "family_id", id.get())?
        .into_iter()
        .map(|e| capture_event_reads(conn, e))
        .collect::<Result<Vec<_>>>()?;
    let citations = read_citations(conn, "family_id", id.get())?;
    let media_links = read_media_links(conn, "family_id", id.get())?;
    delete_one(conn, "families", id.get(), FamilyId::ENTITY)?;
    Ok(FamilyDeletion {
        family,
        created_at,
        updated_at,
        child_links,
        events,
        citations,
        media_links,
    })
}

/// Reads an event's cascade set (citations + media links) **without** deleting it.
/// Used both for [`DeleteTarget::Event`] (the caller then deletes the event) and
/// per-event inside an individual/family snapshot (the parent `DELETE` removes it).
fn capture_event_reads(conn: &Connection, event: Event) -> Result<CapturedEvent> {
    let id = event.id.get();
    let citations = read_citations(conn, "event_id", id)?;
    let media_links = read_media_links(conn, "event_id", id)?;
    Ok(CapturedEvent {
        event,
        citations,
        media_links,
    })
}

/// Deletes one row of `table` by id, returning [`CoreError::NotFound`] (labelled
/// `entity`) when the row is absent — the per-entity `delete_*` behaviour.
fn delete_one(conn: &Connection, table: &str, id: i64, entity: &'static str) -> Result<()> {
    // `table` is always a module-internal literal, never user input.
    let n = conn.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id])?;
    if n == 0 {
        return Err(not_found(entity, id));
    }
    Ok(())
}

fn not_found(entity: &'static str, id: i64) -> CoreError {
    CoreError::NotFound { entity, id }
}

// — restore (explicit-id inserts) ——————————————————————————————————————————————

fn restore_individual(conn: &Connection, d: &IndividualDeletion) -> Result<()> {
    insert_individual(conn, &d.individual, &d.created_at, &d.updated_at)?;
    // Re-set the partner pointers the delete `SET NULL`ed. The family rows survive a
    // person-delete; if a family was itself deleted since, the UPDATE simply matches
    // nothing (that marriage can't be restored without its family).
    for (family, slot) in &d.partner_slots {
        let col = match slot {
            PartnerSlot::One => "partner1_id",
            PartnerSlot::Two => "partner2_id",
        };
        conn.execute(
            &format!("UPDATE families SET {col}=?1 WHERE id=?2"),
            params![d.individual.id, family],
        )?;
    }
    for name in &d.names {
        insert_name(conn, name)?;
    }
    for link in &d.child_links {
        insert_child(conn, link)?;
    }
    for ev in &d.events {
        restore_event(conn, ev)?;
    }
    for c in &d.citations {
        insert_citation(conn, c)?;
    }
    for link in &d.media_links {
        insert_media_link(conn, link)?;
    }
    Ok(())
}

fn restore_family(conn: &Connection, d: &FamilyDeletion) -> Result<()> {
    insert_family(conn, &d.family, &d.created_at, &d.updated_at)?;
    for link in &d.child_links {
        insert_child(conn, link)?;
    }
    for ev in &d.events {
        restore_event(conn, ev)?;
    }
    for c in &d.citations {
        insert_citation(conn, c)?;
    }
    for link in &d.media_links {
        insert_media_link(conn, link)?;
    }
    Ok(())
}

fn restore_event(conn: &Connection, captured: &CapturedEvent) -> Result<()> {
    insert_event(conn, &captured.event)?;
    for c in &captured.citations {
        insert_citation(conn, c)?;
    }
    for link in &captured.media_links {
        insert_media_link(conn, link)?;
    }
    Ok(())
}

fn insert_individual(
    conn: &Connection,
    ind: &Individual,
    created_at: &str,
    updated_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO individuals
            (id, sex, given_name, surname, name_prefix, name_suffix, nickname,
             living, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            ind.id,
            ind.sex,
            ind.given_name,
            ind.surname,
            ind.name_prefix,
            ind.name_suffix,
            ind.nickname,
            ind.living,
            ind.notes,
            created_at,
            updated_at,
        ],
    )?;
    Ok(())
}

fn insert_family(
    conn: &Connection,
    fam: &Family,
    created_at: &str,
    updated_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO families
            (id, partner1_id, partner2_id, union_type, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            fam.id,
            fam.partner1,
            fam.partner2,
            fam.union_type,
            fam.notes,
            created_at,
            updated_at,
        ],
    )?;
    Ok(())
}

fn insert_name(conn: &Connection, name: &Name) -> Result<()> {
    conn.execute(
        "INSERT INTO names
            (id, individual_id, kind, given_name, surname, name_prefix, name_suffix, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            name.id,
            name.individual_id,
            name.kind,
            name.given_name,
            name.surname,
            name.name_prefix,
            name.name_suffix,
            name.sort_order,
        ],
    )?;
    Ok(())
}

fn insert_child(conn: &Connection, link: &ChildLink) -> Result<()> {
    conn.execute(
        "INSERT INTO family_children (family_id, child_id, relation, sort_order)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            link.family_id,
            link.child_id,
            link.relation,
            link.sort_order
        ],
    )?;
    Ok(())
}

fn insert_event(conn: &Connection, event: &Event) -> Result<()> {
    let (date_original, date_modifier, date_sort, date_year, date_month, date_day) =
        derive_date_columns(event.date);
    conn.execute(
        "INSERT INTO events
            (id, individual_id, family_id, kind, date_original, date_modifier,
             date_sort, date_year, date_month, date_day, place_id, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event.id,
            event.subject.individual_id(),
            event.subject.family_id(),
            event.kind,
            date_original,
            date_modifier,
            date_sort,
            date_year,
            date_month,
            date_day,
            event.place,
            event.notes,
        ],
    )?;
    Ok(())
}

fn insert_source(conn: &Connection, source: &Source) -> Result<()> {
    conn.execute(
        "INSERT INTO sources (id, title, author, publication, repository, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source.id,
            source.title,
            source.author,
            source.publication,
            source.repository,
            source.notes,
        ],
    )?;
    Ok(())
}

fn insert_citation(conn: &Connection, c: &Citation) -> Result<()> {
    conn.execute(
        "INSERT INTO citations
            (id, source_id, individual_id, family_id, event_id, page, detail, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            c.id,
            c.source,
            c.subject.individual_id(),
            c.subject.family_id(),
            c.subject.event_id(),
            c.page,
            c.detail,
            c.confidence,
        ],
    )?;
    Ok(())
}

fn insert_media(conn: &Connection, media: &Media) -> Result<()> {
    conn.execute(
        "INSERT INTO media (id, path, caption, mime) VALUES (?1, ?2, ?3, ?4)",
        params![media.id, media.path, media.caption, media.mime],
    )?;
    Ok(())
}

fn insert_media_link(conn: &Connection, link: &MediaLink) -> Result<()> {
    conn.execute(
        "INSERT INTO media_links (media_id, individual_id, family_id, event_id, is_primary)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            link.media,
            link.subject.individual_id(),
            link.subject.family_id(),
            link.subject.event_id(),
            link.is_primary,
        ],
    )?;
    Ok(())
}

// — reads (all on the transaction's connection) ————————————————————————————————

fn read_individual(conn: &Connection, id: PersonId) -> Result<(Individual, String, String)> {
    conn.query_row(
        "SELECT id, given_name, surname, name_prefix, name_suffix, nickname, sex,
                living, notes, created_at, updated_at
         FROM individuals WHERE id = ?1",
        [id],
        |row| {
            let individual = Individual {
                id: row.get("id")?,
                given_name: row.get("given_name")?,
                surname: row.get("surname")?,
                name_prefix: row.get("name_prefix")?,
                name_suffix: row.get("name_suffix")?,
                nickname: row.get("nickname")?,
                sex: row.get::<_, Option<Sex>>("sex")?.unwrap_or(Sex::Unknown),
                living: row.get("living")?,
                notes: row.get("notes")?,
            };
            Ok((individual, row.get("created_at")?, row.get("updated_at")?))
        },
    )
    .optional()?
    .ok_or_else(|| not_found(PersonId::ENTITY, id.get()))
}

fn read_family(conn: &Connection, id: FamilyId) -> Result<(Family, String, String)> {
    conn.query_row(
        "SELECT id, partner1_id, partner2_id, union_type, notes, created_at, updated_at
         FROM families WHERE id = ?1",
        [id],
        |row| {
            let family = Family {
                id: row.get("id")?,
                partner1: row.get("partner1_id")?,
                partner2: row.get("partner2_id")?,
                union_type: row
                    .get::<_, Option<UnionType>>("union_type")?
                    .unwrap_or(UnionType::Unknown),
                notes: row.get("notes")?,
            };
            Ok((family, row.get("created_at")?, row.get("updated_at")?))
        },
    )
    .optional()?
    .ok_or_else(|| not_found(FamilyId::ENTITY, id.get()))
}

fn read_name(conn: &Connection, id: NameId) -> Result<Name> {
    conn.query_row(
        "SELECT id, individual_id, kind, given_name, surname, name_prefix, name_suffix, sort_order
         FROM names WHERE id = ?1",
        [id],
        row_to_name,
    )
    .optional()?
    .ok_or_else(|| not_found(NameId::ENTITY, id.get()))
}

fn read_names_of(conn: &Connection, individual: PersonId) -> Result<Vec<Name>> {
    let mut stmt = conn.prepare(
        "SELECT id, individual_id, kind, given_name, surname, name_prefix, name_suffix, sort_order
         FROM names WHERE individual_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([individual], row_to_name)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn row_to_name(row: &rusqlite::Row<'_>) -> rusqlite::Result<Name> {
    Ok(Name {
        id: row.get("id")?,
        individual_id: row.get("individual_id")?,
        kind: row.get("kind")?,
        given_name: row.get("given_name")?,
        surname: row.get("surname")?,
        name_prefix: row.get("name_prefix")?,
        name_suffix: row.get("name_suffix")?,
        sort_order: row.get("sort_order")?,
    })
}

fn read_child_link(conn: &Connection, family: FamilyId, child: PersonId) -> Result<ChildLink> {
    conn.query_row(
        "SELECT family_id, child_id, relation, sort_order FROM family_children
         WHERE family_id = ?1 AND child_id = ?2",
        params![family, child],
        row_to_child_link,
    )
    .optional()?
    .ok_or_else(|| not_found(FamilyId::ENTITY, family.get()))
}

fn read_child_links(conn: &Connection, col: &str, id: i64) -> Result<Vec<ChildLink>> {
    // `col` is a module-internal literal ("child_id" | "family_id"), never user input.
    let mut stmt = conn.prepare(&format!(
        "SELECT family_id, child_id, relation, sort_order FROM family_children
         WHERE {col} = ?1 ORDER BY family_id, child_id"
    ))?;
    let rows = stmt
        .query_map([id], row_to_child_link)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn row_to_child_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChildLink> {
    Ok(ChildLink {
        family_id: row.get("family_id")?,
        child_id: row.get("child_id")?,
        relation: row.get("relation")?,
        sort_order: row.get("sort_order")?,
    })
}

fn read_event(conn: &Connection, id: EventId) -> Result<Event> {
    let row = conn
        .query_row(
            "SELECT id, individual_id, family_id, kind, date_original, place_id, notes
             FROM events WHERE id = ?1",
            [id],
            event_columns,
        )
        .optional()?;
    match row {
        Some(r) => row_to_event(r),
        None => Err(not_found(EventId::ENTITY, id.get())),
    }
}

fn read_events(conn: &Connection, col: &str, id: i64) -> Result<Vec<Event>> {
    // `col` is a module-internal literal ("individual_id" | "family_id").
    let mut stmt = conn.prepare(&format!(
        "SELECT id, individual_id, family_id, kind, date_original, place_id, notes
         FROM events WHERE {col} = ?1 ORDER BY id"
    ))?;
    let rows = stmt
        .query_map([id], event_columns)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(row_to_event).collect()
}

/// The raw `events` column tuple, before subject/date reconstruction.
type EventRow = (
    EventId,
    Option<PersonId>,
    Option<FamilyId>,
    EventKind,
    Option<String>,
    Option<PlaceId>,
    Option<String>,
);

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

fn row_to_event((id, ind, fam, kind, date_original, place, notes): EventRow) -> Result<Event> {
    let subject = match (ind, fam) {
        (Some(i), None) => EventSubject::Individual(i),
        (None, Some(f)) => EventSubject::Family(f),
        _ => {
            return Err(CoreError::Validation(format!(
                "event {id} has an invalid subject (the schema CHECK should forbid this)"
            )));
        }
    };
    let date = date_original
        .as_deref()
        .map(str::parse::<GenealogicalDate>)
        .transpose()?;
    Ok(Event {
        id,
        subject,
        kind,
        date,
        place,
        notes,
    })
}

fn read_source(conn: &Connection, id: SourceId) -> Result<Source> {
    conn.query_row(
        "SELECT id, title, author, publication, repository, notes FROM sources WHERE id = ?1",
        [id],
        |row| {
            Ok(Source {
                id: row.get("id")?,
                title: row.get("title")?,
                author: row.get("author")?,
                publication: row.get("publication")?,
                repository: row.get("repository")?,
                notes: row.get("notes")?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| not_found(SourceId::ENTITY, id.get()))
}

fn read_citation(conn: &Connection, id: CitationId) -> Result<Citation> {
    let row = conn
        .query_row(
            "SELECT id, source_id, event_id, individual_id, family_id, page, detail, confidence
             FROM citations WHERE id = ?1",
            [id],
            citation_columns,
        )
        .optional()?;
    match row {
        Some(r) => row_to_citation(r),
        None => Err(not_found(CitationId::ENTITY, id.get())),
    }
}

fn read_citations(conn: &Connection, col: &str, id: i64) -> Result<Vec<Citation>> {
    // `col` is a module-internal literal (an FK column name), never user input.
    let mut stmt = conn.prepare(&format!(
        "SELECT id, source_id, event_id, individual_id, family_id, page, detail, confidence
         FROM citations WHERE {col} = ?1 ORDER BY id"
    ))?;
    let rows = stmt
        .query_map([id], citation_columns)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(row_to_citation).collect()
}

/// The raw `citations` column tuple, before subject reconstruction.
type CitationRow = (
    CitationId,
    SourceId,
    Option<EventId>,
    Option<PersonId>,
    Option<FamilyId>,
    Option<String>,
    Option<String>,
    Option<Confidence>,
);

fn citation_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<CitationRow> {
    Ok((
        row.get("id")?,
        row.get("source_id")?,
        row.get("event_id")?,
        row.get("individual_id")?,
        row.get("family_id")?,
        row.get("page")?,
        row.get("detail")?,
        row.get("confidence")?,
    ))
}

fn row_to_citation(
    (id, source, event, individual, family, page, detail, confidence): CitationRow,
) -> Result<Citation> {
    let subject = match (individual, family, event) {
        (Some(i), None, None) => CitationSubject::Individual(i),
        (None, Some(f), None) => CitationSubject::Family(f),
        (None, None, Some(e)) => CitationSubject::Event(e),
        _ => {
            return Err(CoreError::Validation(format!(
                "citation {id} has an invalid subject (exactly one of \
                 individual/family/event must be set)"
            )));
        }
    };
    Ok(Citation {
        id,
        source,
        subject,
        page,
        detail,
        confidence,
    })
}

fn read_media(conn: &Connection, id: MediaId) -> Result<Media> {
    conn.query_row(
        "SELECT id, path, caption, mime FROM media WHERE id = ?1",
        [id],
        |row| {
            Ok(Media {
                id: row.get("id")?,
                path: row.get("path")?,
                caption: row.get("caption")?,
                mime: row.get("mime")?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| not_found(MediaId::ENTITY, id.get()))
}

fn read_media_links(conn: &Connection, col: &str, id: i64) -> Result<Vec<MediaLink>> {
    // `col` is a module-internal literal (an FK column name), never user input.
    let mut stmt = conn.prepare(&format!(
        "SELECT media_id, individual_id, family_id, event_id, is_primary FROM media_links
         WHERE {col} = ?1 ORDER BY media_id"
    ))?;
    collect_media_links(&mut stmt, id)
}

fn read_media_links_of_media(conn: &Connection, media: MediaId) -> Result<Vec<MediaLink>> {
    let mut stmt = conn.prepare(
        "SELECT media_id, individual_id, family_id, event_id, is_primary FROM media_links
         WHERE media_id = ?1 ORDER BY individual_id, family_id, event_id",
    )?;
    collect_media_links(&mut stmt, media.get())
}

/// Runs a prepared `media_links` statement bound to one id and reconstructs the
/// [`MediaLink`]s (rebuilding each subject from the three nullable FKs).
fn collect_media_links(stmt: &mut rusqlite::Statement<'_>, id: i64) -> Result<Vec<MediaLink>> {
    let raw = stmt
        .query_map([id], |row| {
            Ok((
                row.get::<_, MediaId>("media_id")?,
                row.get::<_, Option<PersonId>>("individual_id")?,
                row.get::<_, Option<FamilyId>>("family_id")?,
                row.get::<_, Option<EventId>>("event_id")?,
                row.get::<_, bool>("is_primary")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raw.into_iter()
        .map(|(media, individual, family, event, is_primary)| {
            let subject = match (individual, family, event) {
                (Some(i), None, None) => MediaSubject::Individual(i),
                (None, Some(f), None) => MediaSubject::Family(f),
                (None, None, Some(e)) => MediaSubject::Event(e),
                _ => {
                    return Err(CoreError::Validation(format!(
                        "media link for media {media} has an invalid subject \
                         (exactly one of individual/family/event must be set)"
                    )));
                }
            };
            Ok(MediaLink {
                media,
                subject,
                is_primary,
            })
        })
        .collect()
}

fn read_partner_slots(conn: &Connection, person: PersonId) -> Result<Vec<(FamilyId, PartnerSlot)>> {
    let mut stmt = conn.prepare(
        "SELECT id, partner1_id, partner2_id FROM families
         WHERE partner1_id = ?1 OR partner2_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([person], |row| {
            Ok((
                row.get::<_, FamilyId>("id")?,
                row.get::<_, Option<PersonId>>("partner1_id")?,
                row.get::<_, Option<PersonId>>("partner2_id")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut slots = Vec::with_capacity(rows.len());
    for (family, p1, p2) in rows {
        if p1 == Some(person) {
            slots.push((family, PartnerSlot::One));
        }
        if p2 == Some(person) {
            slots.push((family, PartnerSlot::Two));
        }
    }
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChildRelation, NewCitation, NewFamily, NewIndividual, NewName, NewSource};
    use crate::model::{NameKind, NewEvent, UnionType};

    /// Every row of every table, formatted deterministically (sorted) — including
    /// the `created_at`/`updated_at` columns the model types omit, so a restore
    /// that regenerated a timestamp would show here. Uses the in-crate `conn()`.
    fn dump(store: &Store) -> Vec<String> {
        let conn = store.conn().expect("conn");
        let tables = [
            "individuals",
            "names",
            "families",
            "family_children",
            "places",
            "events",
            "sources",
            "citations",
            "media",
            "media_links",
        ];
        let mut out = Vec::new();
        for table in tables {
            let mut stmt = conn
                .prepare(&format!("SELECT * FROM {table}"))
                .expect("prepare");
            let ncols = stmt.column_count();
            let mut rows = stmt
                .query_map([], |row| {
                    let mut cells = Vec::with_capacity(ncols);
                    for i in 0..ncols {
                        cells.push(format!("{:?}", row.get::<_, rusqlite::types::Value>(i)?));
                    }
                    Ok(cells.join("|"))
                })
                .expect("query")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("collect");
            rows.sort();
            out.push(format!("{table}=[{}]", rows.join(", ")));
        }
        out
    }

    #[test]
    fn individual_delete_then_restore_is_byte_identical() {
        // A person who is a partner in two families and a child in a third, with a
        // name, an event (citation + a media link), a direct citation, and a direct
        // media link. The raw dump (timestamps included) must be unchanged after a
        // delete-then-restore.
        let store = Store::open_in_memory().expect("store");
        let p = store
            .create_individual(&NewIndividual {
                given_name: Some("Ada".to_owned()),
                surname: Some("Lovelace".to_owned()),
                notes: Some("note".to_owned()),
                ..NewIndividual::default()
            })
            .expect("person")
            .id;
        let spouse_a = store
            .create_individual(&NewIndividual::default())
            .expect("a")
            .id;
        let spouse_b = store
            .create_individual(&NewIndividual::default())
            .expect("b")
            .id;
        let parent = store
            .create_individual(&NewIndividual::default())
            .expect("parent")
            .id;
        store
            .add_name(&NewName {
                individual_id: p,
                kind: NameKind::Married,
                given_name: Some("Ada".to_owned()),
                surname: Some("Byron".to_owned()),
                name_prefix: None,
                name_suffix: None,
                sort_order: 0,
            })
            .expect("name");
        store
            .create_family(&NewFamily {
                partner1: Some(p),
                partner2: Some(spouse_a),
                union_type: UnionType::Marriage,
                ..NewFamily::default()
            })
            .expect("f1");
        store
            .create_family(&NewFamily {
                partner1: Some(spouse_b),
                partner2: Some(p),
                ..NewFamily::default()
            })
            .expect("f2");
        let birth_family = store
            .create_family(&NewFamily {
                partner1: Some(parent),
                ..NewFamily::default()
            })
            .expect("f3");
        store
            .add_child(birth_family.id, p, ChildRelation::Birth, 0)
            .expect("child");
        let event = store
            .add_event(&NewEvent {
                subject: EventSubject::Individual(p),
                kind: EventKind::Birth,
                date: Some("ABT 1815".parse().expect("date")),
                place: None,
                notes: None,
            })
            .expect("event");
        let src = store
            .create_source(&NewSource {
                title: "Register".to_owned(),
                ..NewSource::default()
            })
            .expect("source");
        store
            .add_citation(&NewCitation {
                source: src.id,
                subject: CitationSubject::Event(event.id),
                page: Some("p.1".to_owned()),
                detail: None,
                confidence: Some(Confidence::Primary),
            })
            .expect("event citation");
        store
            .add_citation(&NewCitation {
                source: src.id,
                subject: CitationSubject::Individual(p),
                page: None,
                detail: None,
                confidence: None,
            })
            .expect("person citation");
        // Media rows + links via the in-crate seams (no file IO needed for the dump).
        store
            .transaction(|conn| {
                let m1 = Store::create_media_in(
                    conn,
                    &crate::model::NewMedia {
                        path: "1.jpg".to_owned(),
                        caption: None,
                        mime: Some("image/jpeg".to_owned()),
                    },
                )?;
                Store::link_media_in(conn, m1.id, MediaSubject::Event(event.id), true)?;
                let m2 = Store::create_media_in(
                    conn,
                    &crate::model::NewMedia {
                        path: "2.jpg".to_owned(),
                        caption: None,
                        mime: Some("image/jpeg".to_owned()),
                    },
                )?;
                Store::link_media_in(conn, m2.id, MediaSubject::Individual(p), true)?;
                Ok(())
            })
            .expect("media");

        let before = dump(&store);
        let deletion = store
            .delete_undoable(DeleteTarget::Individual(p))
            .expect("delete");
        store.restore_deletion(&deletion).expect("restore");
        let after = dump(&store);
        assert_eq!(before, after, "delete → restore must be byte-identical");
    }
}
