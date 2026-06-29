//! CRUD over the `sources` table and its `citations` join — the evidence layer.
//!
//! One submodule holds **both** sources and citations, exactly as [`media`] holds
//! `media` + `media_links`: they are one concern. [`Store::delete_source`] relies
//! on the schema's `ON DELETE CASCADE` to drop a source's citations — there is no
//! hand-rolled cascade.
//!
//! A citation links one `source_id` (mandatory) to exactly one fact. The
//! `citations` table has **no `CHECK`** (unlike `events`), so the "exactly one
//! subject" discipline lives in [`CitationSubject`]: a [`NewCitation`] cannot
//! express zero or two subjects, and the insert writes exactly the one matching
//! fact-FK column (the [`Store::add_event_in`] precedent). [`row_to_citation`]
//! guards the read side — a corrupt row with no/two FKs is a [`CoreError::Validation`],
//! never a panic.
//!
//! [`media`]: super::media
//! [`Store::add_event_in`]: Store::add_event_in

use rusqlite::{OptionalExtension, params};

use crate::error::{CoreError, Result};
use crate::model::{
    Citation, CitationId, CitationItem, CitationSubject, Confidence, EventId, FamilyId,
    NewCitation, NewSource, PersonId, Source, SourceId,
};

use super::Store;

const SOURCE_COLUMNS: &str = "id, title, author, publication, repository, notes";
const CITATION_COLUMNS: &str =
    "id, source_id, event_id, individual_id, family_id, page, detail, confidence";

/// The raw column shape selected for a citation, before subject reconstruction.
type CitationRow = (
    CitationId,
    SourceId,
    Option<EventId>,
    Option<PersonId>,
    Option<FamilyId>,
    Option<String>,     // page
    Option<String>,     // detail
    Option<Confidence>, // confidence (NULL → None)
);

/// The `citations` fact-FK column and raw id for a subject — the SQL-side
/// projection of [`CitationSubject`]'s "exactly one subject" discipline.
fn subject_column(subject: CitationSubject) -> (&'static str, i64) {
    match subject {
        CitationSubject::Individual(p) => ("individual_id", p.get()),
        CitationSubject::Family(f) => ("family_id", f.get()),
        CitationSubject::Event(e) => ("event_id", e.get()),
    }
}

/// Reconstructs a [`Source`] from a row selected as [`SOURCE_COLUMNS`].
fn source_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        id: row.get("id")?,
        title: row.get("title")?,
        author: row.get("author")?,
        publication: row.get("publication")?,
        repository: row.get("repository")?,
        notes: row.get("notes")?,
    })
}

/// Selects the [`CitationRow`] columns from a `&Row`.
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

/// Reconstructs a [`Citation`] from its raw row, rebuilding the subject from the
/// three nullable fact FKs.
fn row_to_citation(
    (id, source, event, individual, family, page, detail, confidence): CitationRow,
) -> Result<Citation> {
    let subject = match (individual, family, event) {
        (Some(i), None, None) => CitationSubject::Individual(i),
        (None, Some(f), None) => CitationSubject::Family(f),
        (None, None, Some(e)) => CitationSubject::Event(e),
        // The "exactly one subject" discipline lives in `CitationSubject`, but the
        // table has no `CHECK`; treat a corrupt row as data error, not a panic.
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

impl Store {
    /// Creates a source, returning the persisted record.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert fails.
    pub fn create_source(&self, draft: &NewSource) -> Result<Source> {
        let conn = self.conn()?;
        Self::create_source_in(&conn, draft)
    }

    /// The `create_source` INSERT body, callable on any connection (a pooled
    /// connection or a transaction) — the transactional twin the GEDCOM importer
    /// uses to write `SOUR` records atomically.
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub(crate) fn create_source_in(
        conn: &rusqlite::Connection,
        draft: &NewSource,
    ) -> Result<Source> {
        conn.execute(
            "INSERT INTO sources (title, author, publication, repository, notes)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                draft.title,
                draft.author,
                draft.publication,
                draft.repository,
                draft.notes,
            ],
        )?;
        Ok(Source {
            id: SourceId::new(conn.last_insert_rowid()),
            title: draft.title.clone(),
            author: draft.author.clone(),
            publication: draft.publication.clone(),
            repository: draft.repository.clone(),
            notes: draft.notes.clone(),
        })
    }

    /// Fetches a source by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such source exists, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn get_source(&self, id: SourceId) -> Result<Source> {
        let conn = self.conn()?;
        conn.query_row(
            &format!("SELECT {SOURCE_COLUMNS} FROM sources WHERE id = ?1"),
            [id],
            source_columns,
        )
        .optional()?
        .ok_or(CoreError::NotFound {
            entity: SourceId::ENTITY,
            id: id.get(),
        })
    }

    /// Lists every source, ascending id (the Sources surface + the GEDCOM writer's
    /// deterministic emission order).
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub fn list_sources(&self) -> Result<Vec<Source>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare(&format!("SELECT {SOURCE_COLUMNS} FROM sources ORDER BY id"))?;
        let rows = stmt
            .query_map([], source_columns)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Updates a source's fields to `draft`, returning the updated record.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no source has `id`, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn update_source(&self, id: SourceId, draft: &NewSource) -> Result<Source> {
        let conn = self.conn()?;
        let n = conn.execute(
            "UPDATE sources SET title=?1, author=?2, publication=?3, repository=?4, notes=?5
             WHERE id=?6",
            params![
                draft.title,
                draft.author,
                draft.publication,
                draft.repository,
                draft.notes,
                id,
            ],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: SourceId::ENTITY,
                id: id.get(),
            });
        }
        Ok(Source {
            id,
            title: draft.title.clone(),
            author: draft.author.clone(),
            publication: draft.publication.clone(),
            repository: draft.repository.clone(),
            notes: draft.notes.clone(),
        })
    }

    /// Deletes a source; its citations cascade (the table's `ON DELETE CASCADE`).
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such source exists, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn delete_source(&self, id: SourceId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM sources WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: SourceId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }

    /// Adds a citation against exactly one fact, returning the persisted record.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert
    /// fails (e.g. a foreign-key violation on a non-existent source/subject).
    pub fn add_citation(&self, draft: &NewCitation) -> Result<Citation> {
        let conn = self.conn()?;
        Self::add_citation_in(&conn, draft)
    }

    /// The `add_citation` INSERT body, callable on any connection — the
    /// transactional twin the GEDCOM importer uses to write `SOUR` pointers.
    /// Writes exactly the one fact-FK column the subject names.
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure (e.g. a foreign-key
    /// violation on a non-existent source/subject).
    pub(crate) fn add_citation_in(
        conn: &rusqlite::Connection,
        draft: &NewCitation,
    ) -> Result<Citation> {
        conn.execute(
            "INSERT INTO citations
                (source_id, individual_id, family_id, event_id, page, detail, confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                draft.source,
                draft.subject.individual_id(),
                draft.subject.family_id(),
                draft.subject.event_id(),
                draft.page,
                draft.detail,
                draft.confidence,
            ],
        )?;
        Ok(Citation {
            id: CitationId::new(conn.last_insert_rowid()),
            source: draft.source,
            subject: draft.subject,
            page: draft.page.clone(),
            detail: draft.detail.clone(),
            confidence: draft.confidence,
        })
    }

    /// Lists the citations attached to `subject` as [`CitationItem`]s — each with
    /// its [`Source`] resolved in the same read (one JOIN, no N+1) — ascending
    /// citation id (the detail-pane provenance order + the GEDCOM writer's order).
    ///
    /// # Errors
    /// Returns [`CoreError::Validation`] if a stored citation row is corrupt
    /// (no/two fact FKs), or [`CoreError::Database`] on a SQL failure.
    pub fn citations_for(&self, subject: CitationSubject) -> Result<Vec<CitationItem>> {
        let conn = self.conn()?;
        let (col, id) = subject_column(subject);
        let mut stmt = conn.prepare(&format!(
            "SELECT c.id, c.source_id, c.event_id, c.individual_id, c.family_id,
                    c.page, c.detail, c.confidence,
                    s.title AS s_title, s.author AS s_author, s.publication AS s_publication,
                    s.repository AS s_repository, s.notes AS s_notes
             FROM citations c JOIN sources s ON s.id = c.source_id
             WHERE c.{col} = ?1
             ORDER BY c.id"
        ))?;
        let raw = stmt
            .query_map([id], |row| {
                let citation = citation_columns(row)?;
                let source = Source {
                    id: citation.1, // c.source_id
                    title: row.get("s_title")?,
                    author: row.get("s_author")?,
                    publication: row.get("s_publication")?,
                    repository: row.get("s_repository")?,
                    notes: row.get("s_notes")?,
                };
                Ok((citation, source))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        raw.into_iter()
            .map(|(c, source)| {
                Ok(CitationItem {
                    citation: row_to_citation(c)?,
                    source,
                })
            })
            .collect()
    }

    /// Lists every citation that cites `source`, ascending id (the Sources
    /// surface's "facts this source supports" list).
    ///
    /// # Errors
    /// Returns [`CoreError::Validation`] if a stored citation row is corrupt, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn list_citations_for_source(&self, source: SourceId) -> Result<Vec<Citation>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {CITATION_COLUMNS} FROM citations WHERE source_id = ?1 ORDER BY id"
        ))?;
        let raw = stmt
            .query_map([source], citation_columns)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        raw.into_iter().map(row_to_citation).collect()
    }

    /// Deletes a citation.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no citation has `id`, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn delete_citation(&self, id: CitationId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM citations WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: CitationId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventSubject, NewEvent, NewIndividual};

    /// A corrupt citation row (two fact FKs) must read back as a typed
    /// [`CoreError::Validation`], never a panic. Only reachable via raw SQL — the
    /// public API + [`CitationSubject`] make a two-subject row impossible.
    #[test]
    fn a_two_subject_citation_row_is_a_validation_not_a_panic() {
        let store = Store::open_in_memory().expect("open store");
        let person = store
            .create_individual(&NewIndividual::default())
            .expect("person");
        let source = store
            .create_source(&NewSource {
                title: "Register".to_owned(),
                ..NewSource::default()
            })
            .expect("source");
        let family = store
            .create_family(&crate::model::NewFamily::default())
            .expect("family");
        // Hand-write a row with BOTH individual_id and family_id set.
        {
            let conn = store.conn().expect("conn");
            conn.execute(
                "INSERT INTO citations (source_id, individual_id, family_id) VALUES (?1, ?2, ?3)",
                params![source.id, person.id, family.id],
            )
            .expect("insert corrupt row");
        }
        let err = store
            .citations_for(CitationSubject::Individual(person.id))
            .expect_err("a two-subject row must be rejected");
        assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
    }

    /// A subject FK is `event_id`; deleting the event cascades the citation. (A
    /// focused sanity check of the FK wiring beyond the integration suite.)
    #[test]
    fn deleting_an_event_cascades_its_citations() {
        let store = Store::open_in_memory().expect("open store");
        let person = store
            .create_individual(&NewIndividual::default())
            .expect("person");
        let event = store
            .add_event(&NewEvent {
                subject: EventSubject::Individual(person.id),
                kind: crate::model::EventKind::Birth,
                date: None,
                place: None,
                notes: None,
            })
            .expect("event");
        let source = store
            .create_source(&NewSource {
                title: "Parish".to_owned(),
                ..NewSource::default()
            })
            .expect("source");
        store
            .add_citation(&NewCitation {
                source: source.id,
                subject: CitationSubject::Event(event.id),
                page: None,
                detail: None,
                confidence: None,
            })
            .expect("citation");
        assert_eq!(
            store
                .citations_for(CitationSubject::Event(event.id))
                .expect("list")
                .len(),
            1
        );
        store.delete_event(event.id).expect("delete event");
        assert!(
            store
                .citations_for(CitationSubject::Event(event.id))
                .expect("list")
                .is_empty(),
            "the event's citation cascades with it"
        );
    }
}
