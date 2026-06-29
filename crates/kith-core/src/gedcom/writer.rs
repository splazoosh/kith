//! The writer: the whole `Store` → a valid, deterministic GEDCOM 5.5.1 document.
//! Composes the existing reads; regenerates dates via
//! [`format_gedcom`](crate::date::GenealogicalDate::format_gedcom). No `now()`; no
//! `HashMap`-into-output; individuals emitted in ascending-id order and xrefs
//! derived from row ids, so an `export → import → export` round trip is
//! byte-stable.

use std::collections::HashMap;
use std::fmt::Write as _;

use super::tags;
use crate::db::Store;
use crate::error::Result;
use crate::model::{
    ChildRelation, Citation, CitationSubject, EventSubject, Family, Individual, Media,
    MediaSubject, Name, Source,
};

/// Serialize the whole database to a GEDCOM 5.5.1 string (`0 HEAD … 0 TRLR`).
///
/// # Errors
/// [`CoreError`](crate::error::CoreError) if a `Store` read fails.
pub(super) fn write_document(store: &Store) -> Result<String> {
    let mut buf = String::with_capacity(8192);
    write_header(&mut buf);

    // Ascending-id order makes export deterministic AND keeps the id↔xref mapping
    // reproducible on re-import (`list_individuals` is surname-ordered, so sort).
    let mut individuals = store.list_individuals()?;
    individuals.sort_by_key(|i| i.id);
    for ind in &individuals {
        write_individual(&mut buf, store, ind)?;
    }
    // `list_families` is already id-ordered.
    for fam in store.list_families()? {
        write_family(&mut buf, store, &fam)?;
    }

    // Top-level multimedia objects, ascending media-id (deterministic). Each
    // `INDI`/`FAM` carries `1 OBJE @M{id}@` pointers to these.
    for media in store.list_all_media()? {
        write_object(&mut buf, &media);
    }

    // Top-level sources, ascending source-id. `INDI`/`FAM`/events
    // carry `SOUR @S{id}@` pointers to these. A source's repository is emitted as
    // a `1 REPO @R{n}@` pointer to a synthesized top-level `REPO` record, one per
    // distinct repository string, numbered by ascending source-id of first use
    // (deterministic).
    let sources = store.list_sources()?;
    let (repo_ids, repo_order) = repository_registry(&sources);
    for source in &sources {
        write_source(&mut buf, source, &repo_ids);
    }
    for (n, name) in repo_order.iter().enumerate() {
        write_repository(&mut buf, n + 1, name);
    }

    buf.push_str("0 TRLR\n");
    Ok(buf)
}

/// Number the distinct non-empty repository strings across `sources` by ascending
/// source-id of first use (the `@R{n}@` assignment). Returns `(name → n)` for the
/// `REPO` pointer lookup and the ordered list of names for the synthesized records.
fn repository_registry(sources: &[Source]) -> (HashMap<&str, usize>, Vec<&str>) {
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for source in sources {
        if let Some(repo) = source.repository.as_deref() {
            if !repo.is_empty() && !ids.contains_key(repo) {
                ids.insert(repo, order.len() + 1);
                order.push(repo);
            }
        }
    }
    (ids, order)
}

/// The `HEAD`: `SOUR Kith`/`VERS`/`NAME`, `GEDC.VERS 5.5.1` + `FORM LINEAGE-LINKED`,
/// `CHAR UTF-8`. **No `DATE`/timestamp** — determinism (exit #2).
fn write_header(buf: &mut String) {
    buf.push_str("0 HEAD\n");
    buf.push_str("1 SOUR Kith\n");
    let _ = writeln!(buf, "2 VERS {}", env!("CARGO_PKG_VERSION"));
    buf.push_str("2 NAME Kith\n");
    buf.push_str("1 GEDC\n");
    buf.push_str("2 VERS 5.5.1\n");
    buf.push_str("2 FORM LINEAGE-LINKED\n");
    buf.push_str("1 CHAR UTF-8\n");
}

fn write_individual(buf: &mut String, store: &Store, ind: &Individual) -> Result<()> {
    let _ = writeln!(buf, "0 @I{}@ INDI", ind.id.get());

    // Primary (inline) name, then the alternate `NAME` rows (each carrying a TYPE).
    write_primary_name(buf, ind);
    for name in store.list_names(ind.id)? {
        write_alternate_name(buf, &name);
    }

    // `Sex::as_str` IS the GEDCOM value (M/F/X/U).
    let _ = writeln!(buf, "1 SEX {}", ind.sex.as_str());

    write_events(buf, store, EventSubject::Individual(ind.id))?;

    // `FAMC` carries the child's `PEDI`; `FAMS` is the partner edge.
    for fam_id in store.families_of_child(ind.id)? {
        let _ = writeln!(buf, "1 FAMC @F{}@", fam_id.get());
        let relation = child_relation(store, fam_id, ind)?;
        if let Some(pedi) = tags::pedi_for_child_relation(relation) {
            let _ = writeln!(buf, "2 PEDI {pedi}");
        }
    }
    for fam in store.families_of_partner(ind.id)? {
        let _ = writeln!(buf, "1 FAMS @F{}@", fam.id.get());
    }

    // Multimedia pointers, ascending media-id (matches the top-level OBJE order).
    for media_id in store.media_ids_for(MediaSubject::Individual(ind.id))? {
        let _ = writeln!(buf, "1 OBJE @M{}@", media_id.get());
    }

    // Record-level citations (`1 SOUR @S{id}@`), ascending citation-id.
    for item in store.citations_for(CitationSubject::Individual(ind.id))? {
        write_citation(buf, &item.citation, 1);
    }

    if let Some(notes) = &ind.notes {
        write_note(buf, 1, notes);
    }
    Ok(())
}

fn write_family(buf: &mut String, store: &Store, fam: &Family) -> Result<()> {
    let _ = writeln!(buf, "0 @F{}@ FAM", fam.id.get());
    if let Some(p1) = fam.partner1 {
        let _ = writeln!(buf, "1 HUSB @I{}@", p1.get());
    }
    if let Some(p2) = fam.partner2 {
        let _ = writeln!(buf, "1 WIFE @I{}@", p2.get());
    }
    for link in store.list_children(fam.id)? {
        let _ = writeln!(buf, "1 CHIL @I{}@", link.child_id.get());
    }
    // Family events (MARR/DIV/…); `union_type` is implied by MARR presence.
    write_events(buf, store, EventSubject::Family(fam.id))?;
    for media_id in store.media_ids_for(MediaSubject::Family(fam.id))? {
        let _ = writeln!(buf, "1 OBJE @M{}@", media_id.get());
    }
    for item in store.citations_for(CitationSubject::Family(fam.id))? {
        write_citation(buf, &item.citation, 1);
    }
    if let Some(notes) = &fam.notes {
        write_note(buf, 1, notes);
    }
    Ok(())
}

/// A top-level multimedia object record (`0 @M{id}@ OBJE`) with its `FILE` and
/// optional `FORM`/`TITL`. `FORM` nests under `FILE` per 5.5.1; emission is
/// deterministic (driven by the ascending-id media list).
fn write_object(buf: &mut String, media: &Media) {
    let _ = writeln!(buf, "0 @M{}@ OBJE", media.id.get());
    let _ = writeln!(buf, "1 FILE {}", media.path);
    if let Some(form) = &media.mime {
        let _ = writeln!(buf, "2 FORM {form}");
    }
    if let Some(title) = &media.caption {
        let _ = writeln!(buf, "1 TITL {title}");
    }
}

/// Emit every event for `subject` (in `list_events_for` order), each with its
/// optional `TYPE`/`DATE`/`PLAC`/`NOTE` and its nested `2 SOUR @S{id}@` citations.
fn write_events(buf: &mut String, store: &Store, subject: EventSubject) -> Result<()> {
    for ev in store.list_events_for(subject)? {
        let (tag, type_value) = tags::tag_for_event_kind(&ev.kind);
        let _ = writeln!(buf, "1 {tag}");
        if let Some(tv) = type_value {
            let _ = writeln!(buf, "2 TYPE {tv}");
        }
        if let Some(date) = ev.date {
            let _ = writeln!(buf, "2 DATE {}", date.format_gedcom());
        }
        if let Some(place_id) = ev.place {
            let _ = writeln!(buf, "2 PLAC {}", store.get_place(place_id)?.name);
        }
        if let Some(notes) = &ev.notes {
            write_note(buf, 2, notes);
        }
        // Event citations nest one level deeper than a record-level one.
        for item in store.citations_for(CitationSubject::Event(ev.id))? {
            write_citation(buf, &item.citation, 2);
        }
    }
    Ok(())
}

/// A top-level source record (`0 @S{id}@ SOUR`) with its `TITL`/`AUTH`/`PUBL`,
/// a `1 REPO @R{n}@` pointer (when its repository is in `repo_ids`), and `NOTE`.
/// Empty/absent fields are omitted so the round trip is byte-stable.
fn write_source(buf: &mut String, source: &Source, repo_ids: &HashMap<&str, usize>) {
    let _ = writeln!(buf, "0 @S{}@ SOUR", source.id.get());
    if !source.title.is_empty() {
        let _ = writeln!(buf, "1 TITL {}", source.title);
    }
    if let Some(author) = &source.author {
        let _ = writeln!(buf, "1 AUTH {author}");
    }
    if let Some(publication) = &source.publication {
        let _ = writeln!(buf, "1 PUBL {publication}");
    }
    if let Some(n) = source.repository.as_deref().and_then(|r| repo_ids.get(r)) {
        let _ = writeln!(buf, "1 REPO @R{n}@");
    }
    if let Some(notes) = &source.notes {
        write_note(buf, 1, notes);
    }
}

/// A synthesized top-level repository record (`0 @R{n}@ REPO` / `1 NAME …`) — one
/// per distinct repository string, so a source's `REPO` pointer is valid 5.5.1.
fn write_repository(buf: &mut String, n: usize, name: &str) {
    let _ = writeln!(buf, "0 @R{n}@ REPO");
    let _ = writeln!(buf, "1 NAME {name}");
}

/// A `SOUR @S{id}@` citation pointer at `level` (1 under `INDI`/`FAM`, 2 under an
/// event), with its `PAGE`/`QUAY`/`DATA`>`TEXT` one and two levels deeper.
fn write_citation(buf: &mut String, citation: &Citation, level: u8) {
    let _ = writeln!(buf, "{level} SOUR @S{}@", citation.source.get());
    if let Some(page) = &citation.page {
        let _ = writeln!(buf, "{} PAGE {page}", level + 1);
    }
    if let Some(confidence) = citation.confidence {
        let _ = writeln!(
            buf,
            "{} QUAY {}",
            level + 1,
            tags::quay_for_confidence(confidence)
        );
    }
    if let Some(detail) = &citation.detail {
        let _ = writeln!(buf, "{} DATA", level + 1);
        write_continued(buf, level + 2, "TEXT", detail);
    }
}

/// The individual's primary name from its inline fields (no `TYPE`), with the
/// `given /surname/` value and `GIVN`/`SURN`/`NPFX`/`NSFX`/`NICK` sub-tags. Skipped
/// entirely when the individual has no name components.
fn write_primary_name(buf: &mut String, ind: &Individual) {
    if ind.given_name.is_none()
        && ind.surname.is_none()
        && ind.name_prefix.is_none()
        && ind.name_suffix.is_none()
        && ind.nickname.is_none()
    {
        return;
    }
    let _ = writeln!(
        buf,
        "1 NAME {}",
        name_value(ind.given_name.as_deref(), ind.surname.as_deref())
    );
    write_name_parts(
        buf,
        ind.given_name.as_deref(),
        ind.surname.as_deref(),
        ind.name_prefix.as_deref(),
        ind.name_suffix.as_deref(),
        ind.nickname.as_deref(),
    );
}

/// An alternate `NAME` row, with its `TYPE` and component sub-tags.
fn write_alternate_name(buf: &mut String, name: &Name) {
    let _ = writeln!(
        buf,
        "1 NAME {}",
        name_value(name.given_name.as_deref(), name.surname.as_deref())
    );
    let _ = writeln!(buf, "2 TYPE {}", tags::type_for_name_kind(name.kind));
    write_name_parts(
        buf,
        name.given_name.as_deref(),
        name.surname.as_deref(),
        name.name_prefix.as_deref(),
        name.name_suffix.as_deref(),
        None,
    );
}

/// Emit the `2 GIVN/SURN/NPFX/NSFX/NICK` sub-tags for the components present.
fn write_name_parts(
    buf: &mut String,
    given: Option<&str>,
    surname: Option<&str>,
    prefix: Option<&str>,
    suffix: Option<&str>,
    nickname: Option<&str>,
) {
    if let Some(g) = given {
        let _ = writeln!(buf, "2 GIVN {g}");
    }
    if let Some(s) = surname {
        let _ = writeln!(buf, "2 SURN {s}");
    }
    if let Some(p) = prefix {
        let _ = writeln!(buf, "2 NPFX {p}");
    }
    if let Some(s) = suffix {
        let _ = writeln!(buf, "2 NSFX {s}");
    }
    if let Some(n) = nickname {
        let _ = writeln!(buf, "2 NICK {n}");
    }
}

/// The GEDCOM `NAME` value: `given /surname/` (the surname always slashed when
/// present; the slashes are omitted when there is no surname).
fn name_value(given: Option<&str>, surname: Option<&str>) -> String {
    let mut s = String::new();
    if let Some(g) = given {
        s.push_str(g);
    }
    if let Some(sn) = surname {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push('/');
        s.push_str(sn);
        s.push('/');
    }
    s
}

/// Emit a `NOTE`, splitting embedded newlines across `CONT` sub-lines (one level
/// deeper). `level` is the `NOTE` line's level (1 inline, 2 under an event).
fn write_note(buf: &mut String, level: u8, text: &str) {
    write_continued(buf, level, "NOTE", text);
}

/// Emit `{level} {tag} {text}`, splitting embedded newlines across `CONT`
/// sub-lines one level deeper — the shared `NOTE`/`TEXT` continuation form.
fn write_continued(buf: &mut String, level: u8, tag: &str, text: &str) {
    let mut segments = text.split('\n');
    let first = segments.next().unwrap_or("");
    let _ = writeln!(buf, "{level} {tag} {first}");
    for segment in segments {
        let _ = writeln!(buf, "{} CONT {segment}", level + 1);
    }
}

/// The child's relation in `family` (for the `FAMC.PEDI`), read from the family's
/// child links. Defaults to `Birth` if the link is missing (never happens for a
/// consistent store).
fn child_relation(
    store: &Store,
    family: crate::model::FamilyId,
    child: &Individual,
) -> Result<ChildRelation> {
    for link in store.list_children(family)? {
        if link.child_id == child.id {
            return Ok(link.relation);
        }
    }
    Ok(ChildRelation::Birth)
}
