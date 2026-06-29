//! The reader/mapper: a record tree → the model, in ONE transaction. Two
//! passes: pass 1 creates all INDI then all FAM (building `@I…@→PersonId` /
//! `@F…@→FamilyId`); pass 2 wires children (+PEDI). Events, alternate names, and
//! places (deduped by name) are written alongside their owner. Unsupported
//! top-level records are skipped and counted; a dangling xref is a line-cited
//! [`CoreError::Validation`] caught *before* the transaction, so a malformed file
//! writes nothing.

use std::collections::{HashMap, HashSet};

use super::tags;
use super::tree::GedcomRecord;
use super::{ImportOptions, ImportSummary};
use crate::date::GenealogicalDate;
use crate::db::Store;
use crate::error::{CoreError, Result};
use crate::model::{
    ChildRelation, CitationSubject, EventSubject, FamilyId, MediaId, MediaSubject, NewCitation,
    NewEvent, NewFamily, NewIndividual, NewMedia, NewName, NewPlace, NewSource, PersonId, PlaceId,
    Sex, SourceId, UnionType,
};

/// The sub-record tags under an `INDI` that map to events.
const INDI_EVENT_TAGS: &[&str] = &["BIRT", "DEAT", "BAPM", "BURI", "RESI", "OCCU", "EVEN"];
/// The sub-record tags under a `FAM` that map to events.
const FAM_EVENT_TAGS: &[&str] = &["MARR", "DIV", "EVEN"];

/// Reject a declared `CHAR ANSEL` (or any non-UTF-8 declaration). The input
/// is already a decoded `&str`; this guards against a header that *claims* an
/// encoding Kith would have mis-decoded.
///
/// # Errors
/// [`CoreError::Validation`] if `HEAD.CHAR` is not UTF-8/ASCII.
pub(super) fn check_encoding(records: &[GedcomRecord]) -> Result<()> {
    let Some(head) = records.iter().find(|r| r.tag == "HEAD") else {
        return Ok(());
    };
    let Some(char_rec) = head.child("CHAR") else {
        return Ok(());
    };
    if let Some(v) = char_rec.value_str() {
        let up = v.to_ascii_uppercase();
        if up != "UTF-8" && up != "UTF8" && up != "ASCII" {
            return Err(CoreError::Validation(format!(
                "line {}: unsupported character encoding {v:?}; only UTF-8 is supported \
                 (re-export as UTF-8)",
                char_rec.line_no
            )));
        }
    }
    Ok(())
}

/// Validate structure WITHOUT writing: every `HUSB`/`WIFE`/`CHIL`/`FAMC`/`FAMS`
/// points at a present `@I@`/`@F@`. Runs before the transaction so a malformed file
/// does zero DB work.
///
/// # Errors
/// [`CoreError::Validation`] (with the offending line number) for a dangling xref.
pub(super) fn validate(records: &[GedcomRecord]) -> Result<()> {
    let mut indi_xrefs: HashSet<&str> = HashSet::new();
    let mut fam_xrefs: HashSet<&str> = HashSet::new();
    let mut media_xrefs: HashSet<&str> = HashSet::new();
    let mut source_xrefs: HashSet<&str> = HashSet::new();
    let mut repo_xrefs: HashSet<&str> = HashSet::new();
    for rec in records {
        match rec.tag.as_str() {
            "INDI" => {
                if let Some(x) = rec.xref.as_deref() {
                    indi_xrefs.insert(x);
                }
            }
            "FAM" => {
                if let Some(x) = rec.xref.as_deref() {
                    fam_xrefs.insert(x);
                }
            }
            "OBJE" => {
                if let Some(x) = rec.xref.as_deref() {
                    media_xrefs.insert(x);
                }
            }
            "SOUR" => {
                if let Some(x) = rec.xref.as_deref() {
                    source_xrefs.insert(x);
                }
            }
            "REPO" => {
                if let Some(x) = rec.xref.as_deref() {
                    repo_xrefs.insert(x);
                }
            }
            _ => {}
        }
    }
    for rec in records {
        match rec.tag.as_str() {
            "INDI" => {
                for famc in rec.children_with("FAMC") {
                    check_ref(famc, &fam_xrefs, "FAMC", "family")?;
                }
                for fams in rec.children_with("FAMS") {
                    check_ref(fams, &fam_xrefs, "FAMS", "family")?;
                }
                check_media_refs(rec, &media_xrefs)?;
                check_source_refs(rec, &source_xrefs)?;
            }
            "FAM" => {
                for tag in ["HUSB", "WIFE", "CHIL"] {
                    for r in rec.children_with(tag) {
                        check_ref(r, &indi_xrefs, tag, "individual")?;
                    }
                }
                check_media_refs(rec, &media_xrefs)?;
                check_source_refs(rec, &source_xrefs)?;
            }
            // A top-level source's `REPO @R@` pointer must resolve to a `REPO` record.
            "SOUR" => {
                if let Some(repo) = rec.child("REPO") {
                    if repo.value_str().is_some() {
                        check_ref(repo, &repo_xrefs, "REPO", "repository")?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Recursively verify every `SOUR @S@` **pointer** (a `SOUR` sub-record with a
/// value) anywhere under `rec` — directly under `INDI`/`FAM` (`1 SOUR @S@`) or
/// nested under an event (`2 SOUR @S@`) — resolves to a defined top-level source.
/// An inline `SOUR` (no value) is not a pointer and is left to the apply pass.
fn check_source_refs(rec: &GedcomRecord, defined: &HashSet<&str>) -> Result<()> {
    for child in &rec.children {
        if child.tag == "SOUR" && child.value_str().is_some() {
            check_ref(child, defined, "SOUR", "source")?;
        }
        check_source_refs(child, defined)?;
    }
    Ok(())
}

/// Validate the `OBJE` sub-records of an `INDI`/`FAM`: a **pointer** form
/// (`1 OBJE @M@`, with a value) must resolve to a top-level `OBJE`; an **inline**
/// form (`1 OBJE` with sub-`FILE`, no value) needs no target.
fn check_media_refs(rec: &GedcomRecord, defined: &HashSet<&str>) -> Result<()> {
    for obje in rec.children_with("OBJE") {
        if obje.value_str().is_some() {
            check_ref(obje, defined, "OBJE", "object")?;
        }
    }
    Ok(())
}

/// Write the validated records to `store` in one transaction.
///
/// # Errors
/// [`CoreError::Validation`] for a non-merge import into a non-empty store;
/// another [`CoreError`] if a write fails (the transaction rolls back).
pub(super) fn apply(
    store: &Store,
    records: &[GedcomRecord],
    options: &ImportOptions,
) -> Result<ImportSummary> {
    if !options.merge && !store.list_individuals()?.is_empty() {
        return Err(CoreError::Validation(
            "database is not empty; pass merge to append".to_owned(),
        ));
    }

    store.transaction(|conn| {
        let mut persons: HashMap<&str, PersonId> = HashMap::new();
        let mut families: HashMap<&str, FamilyId> = HashMap::new();
        let mut places: HashMap<String, PlaceId> = HashMap::new();
        let mut summary = ImportSummary::default();

        // Index INDI records by xref for the pass-2 PEDI lookup.
        let indi_by_xref: HashMap<&str, &GedcomRecord> = records
            .iter()
            .filter(|r| r.tag == "INDI")
            .filter_map(|r| r.xref.as_deref().map(|x| (x, r)))
            .collect();

        // SOURCES — top-level `SOUR` (+ `REPO`) records become `sources` rows,
        // built FIRST (before individuals/families/events) so every later pass can
        // cite them by `@S…@`. `@R…@ → name` resolves a source's
        // repository (a free-text column — no repository table).
        let repo_names: HashMap<&str, &str> = records
            .iter()
            .filter(|r| r.tag == "REPO")
            .filter_map(|r| {
                let xref = r.xref.as_deref()?;
                let name = r.child("NAME").and_then(value_of)?;
                Some((xref, name))
            })
            .collect();
        let mut source_xref: HashMap<&str, SourceId> = HashMap::new();
        for rec in records.iter().filter(|r| r.tag == "SOUR") {
            let source = Store::create_source_in(conn, &source_draft(rec, &repo_names))?;
            summary.sources += 1;
            if let Some(xref) = rec.xref.as_deref() {
                source_xref.insert(xref, source.id);
            }
        }

        // PASS 1a — individuals, their alternate names, and their events.
        for rec in records.iter().filter(|r| r.tag == "INDI") {
            let xref = rec.xref.as_deref().ok_or_else(|| {
                CoreError::Validation(format!(
                    "line {}: INDI record without an @xref@",
                    rec.line_no
                ))
            })?;
            let (primary, alternates) = split_names(rec);
            let ind = Store::create_individual_in(conn, &individual_draft(rec, primary))?;
            persons.insert(xref, ind.id);
            summary.individuals += 1;

            for (order, name_rec) in alternates.iter().enumerate() {
                let parts = name_parts(name_rec);
                let kind = tags::name_kind_for_type(name_rec.child("TYPE").and_then(value_of));
                Store::add_name_in(
                    conn,
                    &NewName {
                        individual_id: ind.id,
                        kind,
                        given_name: parts.given,
                        surname: parts.surname,
                        name_prefix: parts.prefix,
                        name_suffix: parts.suffix,
                        sort_order: i64::try_from(order).unwrap_or(i64::MAX),
                    },
                )?;
                summary.names += 1;
            }

            add_events(
                conn,
                rec,
                EventSubject::Individual(ind.id),
                INDI_EVENT_TAGS,
                &mut places,
                &source_xref,
                &mut summary,
            )?;

            // Record-level `1 SOUR @S@` pointers → citations on the individual.
            add_citations(
                conn,
                rec,
                CitationSubject::Individual(ind.id),
                &source_xref,
                &mut summary,
            )?;
        }

        // PASS 1b — families (partners resolved from `persons`) and their events.
        for rec in records.iter().filter(|r| r.tag == "FAM") {
            let xref = rec.xref.as_deref().ok_or_else(|| {
                CoreError::Validation(format!(
                    "line {}: FAM record without an @xref@",
                    rec.line_no
                ))
            })?;
            let fam = Store::create_family_in(conn, &family_draft(rec, &persons)?)?;
            families.insert(xref, fam.id);
            summary.families += 1;

            add_events(
                conn,
                rec,
                EventSubject::Family(fam.id),
                FAM_EVENT_TAGS,
                &mut places,
                &source_xref,
                &mut summary,
            )?;

            // Record-level `1 SOUR @S@` pointers → citations on the family.
            add_citations(
                conn,
                rec,
                CitationSubject::Family(fam.id),
                &source_xref,
                &mut summary,
            )?;
        }

        // PASS 1c — media. Top-level `OBJE` records become `media` rows (building
        // `@M…@ → MediaId`); each INDI/FAM's `OBJE` sub-records become `media_links`
        // (a pointer resolves via the map; an inline `OBJE` synthesises its own
        // row). The first link per subject is marked primary — order-stable, since
        // `_PRIM` is not standard 5.5.1. `FILE` paths are recorded
        // verbatim; no external file is copied (import stays DB-only/atomic).
        let mut media_xref: HashMap<&str, MediaId> = HashMap::new();
        for rec in records.iter().filter(|r| r.tag == "OBJE") {
            let media = Store::create_media_in(conn, &media_draft(rec))?;
            summary.media += 1;
            if let Some(xref) = rec.xref.as_deref() {
                media_xref.insert(xref, media.id);
            }
        }
        let mut primaried: HashSet<MediaSubject> = HashSet::new();
        for rec in records.iter().filter(|r| r.tag == "INDI") {
            if let Some(&pid) = rec.xref.as_deref().and_then(|x| persons.get(x)) {
                link_media(
                    conn,
                    rec,
                    MediaSubject::Individual(pid),
                    &media_xref,
                    &mut primaried,
                    &mut summary,
                )?;
            }
        }
        for rec in records.iter().filter(|r| r.tag == "FAM") {
            if let Some(&fid) = rec.xref.as_deref().and_then(|x| families.get(x)) {
                link_media(
                    conn,
                    rec,
                    MediaSubject::Family(fid),
                    &media_xref,
                    &mut primaried,
                    &mut summary,
                )?;
            }
        }

        // PASS 2 — children (relation from the child INDI's FAMC.PEDI, carried on
        // the individual side).
        for rec in records.iter().filter(|r| r.tag == "FAM") {
            let Some(fam_xref) = rec.xref.as_deref() else {
                continue;
            };
            let Some(&fam_id) = families.get(fam_xref) else {
                continue;
            };
            for (order, chil) in rec.children_with("CHIL").enumerate() {
                let ptr = chil.value_str().and_then(strip_pointer).ok_or_else(|| {
                    CoreError::Validation(format!(
                        "line {}: CHIL has no @xref@ pointer",
                        chil.line_no
                    ))
                })?;
                let child_id = *persons.get(ptr).ok_or_else(|| {
                    CoreError::Validation(format!(
                        "line {}: CHIL points to undefined individual @{ptr}@",
                        chil.line_no
                    ))
                })?;
                let relation = indi_by_xref.get(ptr).map_or(ChildRelation::Birth, |indi| {
                    relation_of_child(indi, fam_xref)
                });
                Store::add_child_in(
                    conn,
                    fam_id,
                    child_id,
                    relation,
                    i64::try_from(order).unwrap_or(i64::MAX),
                )?;
            }
        }

        // Skip-and-count the still-unsupported top-level records. `OBJE`
        // and `SOUR`/`REPO` are mapped above, so they
        // drop out of `skipped_tags`; the genuinely-unsupported records that remain
        // (e.g. `SUBM`) are still counted, never silently dropped.
        for rec in records {
            match rec.tag.as_str() {
                "HEAD" | "TRLR" | "INDI" | "FAM" | "OBJE" | "SOUR" | "REPO" => {}
                other => {
                    *summary.skipped_tags.entry(other.to_owned()).or_insert(0) += 1;
                }
            }
        }

        Ok(summary)
    })
}

/// A `record.value_str()` accessor usable as a function value in `and_then`.
fn value_of(rec: &GedcomRecord) -> Option<&str> {
    rec.value_str()
}

/// Build a [`NewMedia`] from an `OBJE` record (top-level or inline): `FILE` →
/// `path` (recorded verbatim — no external copy), `FORM` → `mime`, `TITL` →
/// `caption`. `FORM`/`TITL` are accepted under either `OBJE` (5.5) or `FILE`
/// (5.5.1).
fn media_draft(rec: &GedcomRecord) -> NewMedia {
    let file = rec.child("FILE");
    let under_file = |tag: &str| file.and_then(|f| f.child(tag)).and_then(value_of);
    NewMedia {
        path: file.and_then(value_of).unwrap_or_default().to_owned(),
        mime: rec
            .child("FORM")
            .and_then(value_of)
            .or_else(|| under_file("FORM"))
            .map(str::to_owned),
        caption: rec
            .child("TITL")
            .and_then(value_of)
            .or_else(|| under_file("TITL"))
            .map(str::to_owned),
    }
}

/// Link every `OBJE` sub-record of `rec` to `subject`. A pointer (`1 OBJE @M@`)
/// resolves via `media_xref`; an inline `OBJE` synthesises its own `media` row.
/// The first link per subject is marked primary (order-stable).
fn link_media(
    conn: &rusqlite::Connection,
    rec: &GedcomRecord,
    subject: MediaSubject,
    media_xref: &HashMap<&str, MediaId>,
    primaried: &mut HashSet<MediaSubject>,
    summary: &mut ImportSummary,
) -> Result<()> {
    for obje in rec.children_with("OBJE") {
        let media_id = match obje.value_str().and_then(strip_pointer) {
            Some(ptr) => *media_xref.get(ptr).ok_or_else(|| {
                CoreError::Validation(format!(
                    "line {}: OBJE points to undefined object @{ptr}@",
                    obje.line_no
                ))
            })?,
            None => {
                let media = Store::create_media_in(conn, &media_draft(obje))?;
                summary.media += 1;
                media.id
            }
        };
        // `insert` returns true the first time a subject is seen → its first link
        // is its primary; later links are non-primary.
        let is_primary = primaried.insert(subject);
        Store::link_media_in(conn, media_id, subject, is_primary)?;
    }
    Ok(())
}

/// Strip the `@…@` wrapper from a pointer value (`@F1@` → `F1`).
fn strip_pointer(v: &str) -> Option<&str> {
    v.strip_prefix('@')?.strip_suffix('@')
}

/// Verify one pointer sub-record (`FAMC`/`FAMS`/`HUSB`/`WIFE`/`CHIL`) resolves.
fn check_ref(rec: &GedcomRecord, defined: &HashSet<&str>, tag: &str, kind: &str) -> Result<()> {
    let v = rec.value_str().ok_or_else(|| {
        CoreError::Validation(format!("line {}: {tag} has no @xref@ pointer", rec.line_no))
    })?;
    let id = strip_pointer(v).ok_or_else(|| {
        CoreError::Validation(format!(
            "line {}: {tag} value {v:?} is not an @xref@ pointer",
            rec.line_no
        ))
    })?;
    if !defined.contains(id) {
        return Err(CoreError::Validation(format!(
            "line {}: {tag} points to {kind} {v} which is not defined",
            rec.line_no
        )));
    }
    Ok(())
}

/// The name components of a `NAME` record. `#[derive(Default)]` is the no-name case.
#[derive(Default)]
struct NameParts {
    given: Option<String>,
    surname: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    nickname: Option<String>,
}

/// Read a `NAME` record's components, preferring the `GIVN`/`SURN`/… sub-tags and
/// falling back to the `given /surname/` slash form for files that omit them.
fn name_parts(rec: &GedcomRecord) -> NameParts {
    let sub = |tag: &str| rec.child(tag).and_then(value_of).map(str::to_owned);
    NameParts {
        given: sub("GIVN").or_else(|| slash_given(rec.value_str())),
        surname: sub("SURN").or_else(|| slash_surname(rec.value_str())),
        prefix: sub("NPFX"),
        suffix: sub("NSFX"),
        nickname: sub("NICK"),
    }
}

/// The given-name part of a `given /surname/` value (the text before the first `/`).
fn slash_given(value: Option<&str>) -> Option<String> {
    let before = value?.split('/').next().unwrap_or("").trim();
    (!before.is_empty()).then(|| before.to_owned())
}

/// The surname part of a `given /surname/` value (the text between the slashes).
fn slash_surname(value: Option<&str>) -> Option<String> {
    let surname = value?.split('/').nth(1)?.trim();
    (!surname.is_empty()).then(|| surname.to_owned())
}

/// Split an INDI's `NAME` records into the primary (the first without a `TYPE`,
/// else the first) and the alternates.
fn split_names(rec: &GedcomRecord) -> (Option<&GedcomRecord>, Vec<&GedcomRecord>) {
    let names: Vec<&GedcomRecord> = rec.children_with("NAME").collect();
    if names.is_empty() {
        return (None, Vec::new());
    }
    let primary_idx = names
        .iter()
        .position(|n| n.child("TYPE").is_none())
        .unwrap_or(0);
    let alternates = names
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != primary_idx)
        .map(|(_, n)| *n)
        .collect();
    (Some(names[primary_idx]), alternates)
}

/// Build a [`NewIndividual`] from an INDI record and its primary `NAME`.
fn individual_draft(rec: &GedcomRecord, primary: Option<&GedcomRecord>) -> NewIndividual {
    let parts = primary.map(name_parts).unwrap_or_default();
    let sex = rec
        .child("SEX")
        .and_then(value_of)
        .and_then(|v| v.parse::<Sex>().ok())
        .unwrap_or(Sex::Unknown);
    NewIndividual {
        given_name: parts.given,
        surname: parts.surname,
        name_prefix: parts.prefix,
        name_suffix: parts.suffix,
        nickname: parts.nickname,
        sex,
        living: false, // deterministic; imported people default to deceased
        notes: rec.child("NOTE").and_then(value_of).map(str::to_owned),
    }
}

/// Build a [`NewFamily`] from a FAM record, resolving `HUSB`/`WIFE` via `persons`.
fn family_draft(rec: &GedcomRecord, persons: &HashMap<&str, PersonId>) -> Result<NewFamily> {
    let resolve = |tag: &str| -> Result<Option<PersonId>> {
        match rec.child(tag).and_then(value_of) {
            Some(v) => {
                let ptr = strip_pointer(v).ok_or_else(|| {
                    CoreError::Validation(format!(
                        "line {}: {tag} value {v:?} is not an @xref@ pointer",
                        rec.line_no
                    ))
                })?;
                let id = persons.get(ptr).copied().ok_or_else(|| {
                    CoreError::Validation(format!(
                        "line {}: {tag} points to undefined individual {v}",
                        rec.line_no
                    ))
                })?;
                Ok(Some(id))
            }
            None => Ok(None),
        }
    };
    Ok(NewFamily {
        partner1: resolve("HUSB")?,
        partner2: resolve("WIFE")?,
        // `union_type` is implied by MARR presence/absence; Partnership has no
        // standard tag and reads back as Unknown.
        union_type: if rec.children_with("MARR").next().is_some() {
            UnionType::Marriage
        } else {
            UnionType::Unknown
        },
        notes: rec.child("NOTE").and_then(value_of).map(str::to_owned),
    })
}

/// The child's relation in `family_xref`, read from the child INDI's matching
/// `FAMC.PEDI`. Defaults to `Birth`.
fn relation_of_child(indi: &GedcomRecord, family_xref: &str) -> ChildRelation {
    for famc in indi.children_with("FAMC") {
        if famc.value_str().and_then(strip_pointer) == Some(family_xref) {
            return tags::child_relation_for_pedi(famc.child("PEDI").and_then(value_of));
        }
    }
    ChildRelation::Birth
}

/// Map the event sub-records of `rec` (those whose tag is in `event_tags`) to
/// `add_event_in`, resolving dates via the parse side and places by dedup. Each
/// event's nested `2 SOUR @S@` pointers become citations on that event
/// — the returned [`EventId`] is captured to attach them.
fn add_events(
    conn: &rusqlite::Connection,
    rec: &GedcomRecord,
    subject: EventSubject,
    event_tags: &[&str],
    places: &mut HashMap<String, PlaceId>,
    source_xref: &HashMap<&str, SourceId>,
    summary: &mut ImportSummary,
) -> Result<()> {
    for child in &rec.children {
        if !event_tags.contains(&child.tag.as_str()) {
            continue;
        }
        let kind = tags::event_kind_for_tag(&child.tag, child.child("TYPE").and_then(value_of));
        let date = match child.child("DATE").and_then(value_of) {
            Some(v) => Some(v.parse::<GenealogicalDate>().map_err(|e| {
                let line = child.child("DATE").map_or(child.line_no, |d| d.line_no);
                CoreError::Validation(format!("line {line}: {e}"))
            })?),
            None => None,
        };
        let place = match child.child("PLAC").and_then(value_of) {
            Some(name) => Some(place_id_for(conn, places, name, summary)?),
            None => None,
        };
        let event = Store::add_event_in(
            conn,
            &NewEvent {
                subject,
                kind,
                date,
                place,
                notes: child.child("NOTE").and_then(value_of).map(str::to_owned),
            },
        )?;
        summary.events += 1;

        // Event-nested `2 SOUR @S@` pointers → citations on this event.
        add_citations(
            conn,
            child,
            CitationSubject::Event(event.id),
            source_xref,
            summary,
        )?;
    }
    Ok(())
}

/// Build a [`NewSource`] from a top-level `SOUR` record: `TITL`→title (the only
/// `NOT NULL` column, defaulting to empty), `AUTH`→author, `PUBL`→publication,
/// `NOTE`→notes, and the resolved repository ([`repository_of`]).
fn source_draft(rec: &GedcomRecord, repo_names: &HashMap<&str, &str>) -> NewSource {
    NewSource {
        title: rec
            .child("TITL")
            .and_then(value_of)
            .unwrap_or_default()
            .to_owned(),
        author: rec.child("AUTH").and_then(value_of).map(str::to_owned),
        publication: rec.child("PUBL").and_then(value_of).map(str::to_owned),
        repository: repository_of(rec, repo_names),
        notes: rec.child("NOTE").and_then(value_of).map(str::to_owned),
    }
}

/// Resolve a source's repository name: a `1 REPO @R@` pointer via `repo_names`,
/// else a tolerated inline `1 REPO <name>` value, else a `1 REPO` with a
/// `2 NAME` sub-record, else `None`. A pointer that doesn't resolve (caught by
/// [`validate`]) collapses to `None` rather than recording the raw `@R@`.
fn repository_of(rec: &GedcomRecord, repo_names: &HashMap<&str, &str>) -> Option<String> {
    let repo = rec.child("REPO")?;
    if let Some(ptr) = repo.value_str().and_then(strip_pointer) {
        return repo_names.get(ptr).map(|n| (*n).to_owned());
    }
    repo.value_str()
        .map(str::to_owned)
        .or_else(|| repo.child("NAME").and_then(value_of).map(str::to_owned))
}

/// Attach every `SOUR @S@` **pointer** sub-record of `rec` to `subject` as a
/// citation (`PAGE`→page, `QUAY`→confidence, `DATA`>`TEXT`/`NOTE`→detail). An
/// inline `SOUR` (no value) is not a pointer and is skipped — Kith emits only
/// pointers, so this never loses round-tripped data.
fn add_citations(
    conn: &rusqlite::Connection,
    rec: &GedcomRecord,
    subject: CitationSubject,
    source_xref: &HashMap<&str, SourceId>,
    summary: &mut ImportSummary,
) -> Result<()> {
    for sour in rec.children_with("SOUR") {
        let Some(ptr) = sour.value_str().and_then(strip_pointer) else {
            continue; // inline SOUR — not a pointer
        };
        let source = *source_xref.get(ptr).ok_or_else(|| {
            CoreError::Validation(format!(
                "line {}: SOUR points to undefined source @{ptr}@",
                sour.line_no
            ))
        })?;
        Store::add_citation_in(
            conn,
            &NewCitation {
                source,
                subject,
                page: sour.child("PAGE").and_then(value_of).map(str::to_owned),
                detail: citation_detail(sour),
                confidence: tags::confidence_for_quay(sour.child("QUAY").and_then(value_of)),
            },
        )?;
        summary.citations += 1;
    }
    Ok(())
}

/// A citation's transcription/detail: `DATA`>`TEXT`, else a sibling `NOTE`.
fn citation_detail(sour: &GedcomRecord) -> Option<String> {
    sour.child("DATA")
        .and_then(|d| d.child("TEXT"))
        .and_then(value_of)
        .or_else(|| sour.child("NOTE").and_then(value_of))
        .map(str::to_owned)
}

/// Resolve a place name to an id, creating (and counting) it once and reusing it
/// thereafter — dedup by name, since there is no `find_place_by_name`.
fn place_id_for(
    conn: &rusqlite::Connection,
    places: &mut HashMap<String, PlaceId>,
    name: &str,
    summary: &mut ImportSummary,
) -> Result<PlaceId> {
    if let Some(id) = places.get(name) {
        return Ok(*id);
    }
    let id = Store::create_place_in(
        conn,
        &NewPlace {
            name: name.to_owned(),
            latitude: None,
            longitude: None,
            parent: None,
        },
    )?;
    places.insert(name.to_owned(), id);
    summary.places += 1;
    Ok(id)
}
