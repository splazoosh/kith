//! The LB reader/mapper: a parsed person array → the model, in ONE transaction.
//!
//! Validate first (no writes): reject a `0` or duplicate record id and a
//! dangling parent/spouse pointer. Then apply in three passes inside a single
//! transaction:
//!
//! 1. **individuals** + their birth/death events (a place-only event is created
//!    when a place is present but the date is the unknown sentinel),
//! 2. **families** synthesized from each distinct `(father, mother)` pair, with
//!    children linked in input order, and
//! 3. **couple families** from spouse pointers, deduped against the parent
//!    families (and each other — spouse links are symmetric).
//!
//! A dangling pointer is caught *before* the transaction, so a malformed file
//! writes nothing.

use std::collections::{HashMap, HashSet};

use crate::db::Store;
use crate::error::{CoreError, Result};
use crate::gedcom::{ImportOptions, ImportSummary};
use crate::model::{
    ChildRelation, EventKind, EventSubject, FamilyId, NewEvent, NewFamily, NewIndividual, NewPlace,
    PersonId, PlaceId, Sex, UnionType,
};

use super::record::{LbPerson, non_empty, parse_lb_date};

/// Validate the parsed records without writing: each id must be non-zero and
/// unique, and every parent/spouse pointer must resolve to a present id. Runs
/// before the transaction so a bad file does zero DB work.
///
/// # Errors
/// [`CoreError::Validation`] for a zero/duplicate id or a dangling pointer.
pub(super) fn validate(people: &[LbPerson]) -> Result<()> {
    let mut ids: HashSet<i64> = HashSet::with_capacity(people.len());
    for p in people {
        if p.id == 0 {
            return Err(CoreError::Validation(
                "LB record has id 0, the reserved \"none\" pointer value".to_owned(),
            ));
        }
        if !ids.insert(p.id) {
            return Err(CoreError::Validation(format!(
                "duplicate LB record id {}",
                p.id
            )));
        }
    }
    for p in people {
        check_ref(&ids, "father", p.father_id, p.id)?;
        check_ref(&ids, "mother", p.mother_id, p.id)?;
        check_ref(&ids, "spouse", p.spouse_id, p.id)?;
        if let Some(list) = &p.spouse_list {
            for &s in list {
                check_ref(&ids, "spouse", s, p.id)?;
            }
        }
    }
    Ok(())
}

/// A non-zero `pointer` on record `owner` must resolve to a present id.
fn check_ref(ids: &HashSet<i64>, kind: &str, pointer: i64, owner: i64) -> Result<()> {
    if pointer != 0 && !ids.contains(&pointer) {
        return Err(CoreError::Validation(format!(
            "LB record {owner} references a {kind} id {pointer} that is not present"
        )));
    }
    Ok(())
}

/// Write the validated records to `store` in one transaction.
///
/// # Errors
/// [`CoreError::Validation`] for a non-merge import into a non-empty store;
/// another [`CoreError`] if a write fails (the transaction rolls back — nothing
/// is written).
pub(super) fn apply(
    store: &Store,
    people: &[LbPerson],
    options: &ImportOptions,
) -> Result<ImportSummary> {
    if !options.merge && !store.list_individuals()?.is_empty() {
        return Err(CoreError::Validation(
            "database is not empty; pass merge to append".to_owned(),
        ));
    }

    store.transaction(|conn| {
        let mut summary = ImportSummary::default();
        let mut id_map: HashMap<i64, PersonId> = HashMap::with_capacity(people.len());
        let mut places: HashMap<String, PlaceId> = HashMap::new();

        // PASS 1 — individuals + their birth/death events.
        for p in people {
            let ind = Store::create_individual_in(conn, &individual_draft(p))?;
            id_map.insert(p.id, ind.id);
            summary.individuals += 1;

            add_vital_event(
                conn,
                ind.id,
                EventKind::Birth,
                &p.birth_date,
                &p.birth_place,
                &mut places,
                &mut summary,
            )?;
            add_vital_event(
                conn,
                ind.id,
                EventKind::Death,
                &p.death_date,
                &p.death_place,
                &mut places,
                &mut summary,
            )?;
        }

        // PASS 2 — one family per distinct (father, mother) pair; children
        // linked in input order. A couple index (keyed by the unordered partner
        // pair) lets PASS 3 dedup spouse links against these.
        let mut parent_family: HashMap<(Option<i64>, Option<i64>), FamilyId> = HashMap::new();
        let mut couple_index: HashMap<(PersonId, PersonId), FamilyId> = HashMap::new();
        let mut child_order: HashMap<FamilyId, i64> = HashMap::new();

        for p in people {
            let (father, mother) = (p.father(), p.mother());
            if father.is_none() && mother.is_none() {
                continue; // a root: no parents to form a family from
            }
            let key = (father, mother);
            let fam_id = match parent_family.get(&key) {
                Some(&id) => id,
                None => {
                    let partner1 = father.map(|f| lookup(&id_map, f)).transpose()?;
                    let partner2 = mother.map(|m| lookup(&id_map, m)).transpose()?;
                    let fam = Store::create_family_in(
                        conn,
                        &NewFamily {
                            partner1,
                            partner2,
                            union_type: UnionType::Unknown,
                            notes: None,
                        },
                    )?;
                    summary.families += 1;
                    parent_family.insert(key, fam.id);
                    index_couple(&mut couple_index, partner1, partner2, fam.id);
                    fam.id
                }
            };
            let order = child_order.entry(fam_id).or_insert(0);
            Store::add_child_in(
                conn,
                fam_id,
                lookup(&id_map, p.id)?,
                ChildRelation::Birth,
                *order,
            )?;
            *order += 1;
        }

        // PASS 3 — couple families from spouse pointers, deduped against the
        // parent families and each other (A↔B and B↔A collapse to one family).
        for p in people {
            let a = lookup(&id_map, p.id)?;
            for spouse_lb in p.spouses() {
                let b = lookup(&id_map, spouse_lb)?;
                let key = couple_key(a, b);
                if couple_index.contains_key(&key) {
                    continue;
                }
                let fam = Store::create_family_in(
                    conn,
                    &NewFamily {
                        partner1: Some(key.0),
                        partner2: Some(key.1),
                        union_type: UnionType::Unknown,
                        notes: None,
                    },
                )?;
                summary.families += 1;
                couple_index.insert(key, fam.id);
            }
        }

        Ok(summary)
    })
}

/// Resolve an LB id to its created [`PersonId`]. Absence is unreachable after
/// [`validate`] (every referenced id is present, and PASS 1 created them all),
/// so this is a defensive `Validation` rather than an `unwrap`/index panic.
fn lookup(id_map: &HashMap<i64, PersonId>, lb_id: i64) -> Result<PersonId> {
    id_map.get(&lb_id).copied().ok_or_else(|| {
        CoreError::Validation(format!(
            "internal: LB id {lb_id} was not created (unreachable after validation)"
        ))
    })
}

/// Build a [`NewIndividual`] from an LB record. `living` defaults to `false`
/// (imported people default to deceased — deterministic, matching the GEDCOM
/// importer; the privacy flag is the user's to flip afterward).
fn individual_draft(p: &LbPerson) -> NewIndividual {
    NewIndividual {
        given_name: non_empty(&p.first_name),
        surname: non_empty(&p.last_name),
        name_prefix: None,
        name_suffix: None,
        nickname: None,
        sex: p.gender.parse::<Sex>().unwrap_or(Sex::Unknown),
        living: false,
        notes: non_empty(&p.notes),
    }
}

/// Add a birth/death event for `person`, parsing the LB date and resolving the
/// place (deduped). An event is recorded only when *something* is known — a date
/// or a place; a bare sentinel/blank with no place adds no information and no row.
fn add_vital_event(
    conn: &rusqlite::Connection,
    person: PersonId,
    kind: EventKind,
    date_str: &str,
    place_str: &str,
    places: &mut HashMap<String, PlaceId>,
    summary: &mut ImportSummary,
) -> Result<()> {
    let date = parse_lb_date(date_str);
    let place = match non_empty(place_str) {
        Some(name) => Some(place_id_for(conn, places, &name, summary)?),
        None => None,
    };
    if date.is_none() && place.is_none() {
        return Ok(());
    }
    Store::add_event_in(
        conn,
        &NewEvent {
            subject: EventSubject::Individual(person),
            kind,
            date,
            place,
            notes: None,
        },
    )?;
    summary.events += 1;
    Ok(())
}

/// Resolve a place name to an id, creating (and counting) it once and reusing it
/// thereafter — dedup by name, matching the GEDCOM importer.
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

/// The canonical unordered key for a couple — the two partners ordered by id —
/// so A↔B and B↔A index the same family.
fn couple_key(a: PersonId, b: PersonId) -> (PersonId, PersonId) {
    if a.get() <= b.get() { (a, b) } else { (b, a) }
}

/// Record a two-partner family in the couple index (no-op if a partner is
/// absent, e.g. a single-parent family, which a spouse link can never collide
/// with). A pre-existing entry wins, so a parent family is never displaced.
fn index_couple(
    index: &mut HashMap<(PersonId, PersonId), FamilyId>,
    partner1: Option<PersonId>,
    partner2: Option<PersonId>,
    fam: FamilyId,
) {
    if let (Some(a), Some(b)) = (partner1, partner2) {
        index.entry(couple_key(a, b)).or_insert(fam);
    }
}
