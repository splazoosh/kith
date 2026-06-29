//! A deterministic synthetic-database generator for the performance harness.
//!
//! [`seed_synthetic`] builds a realistic multi-generation pedigree — couples,
//! children in birth order, birth/death/marriage events and a few places — from a
//! fixed PRNG seed, so the same `(individuals, seed)` always produces the same
//! *structure* and therefore the same [`LayoutModel`](crate::layout::LayoutModel)
//! and walk output. (The `created_at`/`updated_at` timestamps the insert helpers
//! stamp are wall-clock, but **no** read path — the walks, the layout, the render
//! — consumes them, so every snapshot- and determinism-bearing output is stable.)
//!
//! Randomness is a self-contained `splitmix64` over the seed — never `rand`, never
//! the wall clock — so a build is reproducible bit-for-bit in its decisions.
//!
//! This module is **dev-only** (`#[cfg(feature = "dev")]`, `#[doc(hidden)]`): the
//! `layout` bench and the `--all-features` large-graph test use it; it is not part
//! of the supported surface and is compiled out of release builds.

use rusqlite::Connection;

use crate::db::Store;
use crate::error::{CoreError, Result};
use crate::model::{
    ChildRelation, EventKind, EventSubject, NewEvent, NewFamily, NewIndividual, NewPlace, PersonId,
    PlaceId, Sex, UnionType,
};

/// A handful of given names cycled through deterministically (the data is for
/// scale, so a small varied pool suffices).
const GIVEN_NAMES: [&str; 12] = [
    "Olav", "Kari", "Anders", "Anna", "Per", "Petra", "Mads", "Mona", "Liv", "Nils", "Erik",
    "Frida",
];

/// A few surnames; the bloodline carries one, married-in partners another.
const SURNAMES: [&str; 8] = ["Lund", "Lie", "Sand", "Holm", "Vest", "Ost", "Dahl", "Vik"];

/// A few reusable places, attached to birth events round-robin for realism.
const PLACE_NAMES: [&str; 4] = [
    "Bergen, Norway",
    "Oslo, Norway",
    "Trondheim, Norway",
    "Stavanger, Norway",
];

/// The first generation's approximate birth year; each generation advances by
/// [`GEN_SPAN`] so dated events read sensibly.
const BASE_YEAR: i32 = 1700;
/// Years between successive generations.
const GEN_SPAN: i32 = 28;

/// A tiny, fast, fully-deterministic PRNG (`splitmix64`). Seeded once; no global
/// state, no wall clock — the source of every "random" decision the generator
/// makes, so a `(individuals, seed)` pair reproduces an identical structure.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n` (`n > 0`); a uniform-enough modulo for synthetic data.
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

/// Seeds `store` with a synthetic pedigree of about `individuals` people and
/// returns a stable focal id well inside the tree (it has both ancestors and
/// descendants, so every chart mode has real work to do).
///
/// The whole forest is a single connected component reachable from the focal, so
/// [`crate::query::network`] walks the full graph — the scale the harness targets.
///
/// Determinism: the same `(individuals, seed)` builds the same structure and the
/// same row ids (inserts happen in a fixed order inside one transaction), so the
/// resulting [`LayoutModel`](crate::layout::LayoutModel) is byte-identical run to
/// run. Inserts are batched in one [`Store::transaction`] for speed.
///
/// # Errors
/// Propagates any [`CoreError`] from the underlying inserts (e.g. a failed write),
/// after the transaction rolls back.
pub fn seed_synthetic(store: &Store, individuals: u32, seed: u64) -> Result<PersonId> {
    let target = individuals.max(1);
    store.transaction(|conn| {
        let mut rng = SplitMix64::new(seed);

        let places: Vec<PlaceId> = PLACE_NAMES
            .iter()
            .map(|name| {
                Store::create_place_in(
                    conn,
                    &NewPlace {
                        name: (*name).to_owned(),
                        latitude: None,
                        longitude: None,
                        parent: None,
                    },
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let mut created: u32 = 0;
        // `gen_firsts[g]` is the first bloodline person born into generation `g`
        // (the founder for `g == 0`); the focal is taken from the middle so it has
        // ancestors above and descendants below.
        let mut gen_firsts: Vec<PersonId> = Vec::new();

        // Generation 0: the founder of the bloodline.
        let founder = mint_person(conn, &mut rng)?;
        created += 1;
        add_vitals(conn, &mut rng, founder, BASE_YEAR, &places)?;
        gen_firsts.push(founder);

        let mut bloodline = vec![founder];
        let mut generation: i32 = 0;
        while created < target && !bloodline.is_empty() {
            let birth_year = BASE_YEAR + (generation + 1) * GEN_SPAN;
            let mut next: Vec<PersonId> = Vec::new();
            for &person in &bloodline {
                if created >= target {
                    break;
                }
                // Each bloodline person marries in a fresh partner …
                let spouse = mint_person(conn, &mut rng)?;
                created += 1;
                add_vitals(conn, &mut rng, spouse, birth_year - GEN_SPAN, &places)?;
                let family = Store::create_family_in(
                    conn,
                    &NewFamily {
                        partner1: Some(person),
                        partner2: Some(spouse),
                        union_type: UnionType::Marriage,
                        notes: None,
                    },
                )?
                .id;
                Store::add_event_in(
                    conn,
                    &NewEvent {
                        subject: EventSubject::Family(family),
                        kind: EventKind::Marriage,
                        date: year_date(birth_year - 4)?,
                        place: Some(places[(created as usize) % places.len()]),
                        notes: None,
                    },
                )?;

                // … and they have one to three children, the next generation.
                let children = 1 + rng.below(3) as i64;
                for order in 0..children {
                    if created >= target {
                        break;
                    }
                    let child = mint_person(conn, &mut rng)?;
                    created += 1;
                    add_vitals(conn, &mut rng, child, birth_year, &places)?;
                    Store::add_child_in(conn, family, child, ChildRelation::Birth, order)?;
                    next.push(child);
                }
            }
            generation += 1;
            if let Some(&first) = next.first() {
                gen_firsts.push(first);
            }
            bloodline = next;
        }

        Ok(gen_firsts[gen_firsts.len() / 2])
    })
}

/// Inserts one person with a deterministic name and sex; not living (a long-dead
/// ancestor), so exports never redact synthetic data.
fn mint_person(conn: &Connection, rng: &mut SplitMix64) -> Result<PersonId> {
    let given = GIVEN_NAMES[(rng.below(GIVEN_NAMES.len() as u64)) as usize];
    let surname = SURNAMES[(rng.below(SURNAMES.len() as u64)) as usize];
    let sex = match rng.below(2) {
        0 => Sex::Male,
        _ => Sex::Female,
    };
    Ok(Store::create_individual_in(
        conn,
        &NewIndividual {
            given_name: Some(given.to_owned()),
            surname: Some(surname.to_owned()),
            sex,
            living: false,
            ..Default::default()
        },
    )?
    .id)
}

/// Adds a dated birth and a dated death event, so the walks' `vital_years` reads
/// do real work (the query benches measure the per-person event lookup too).
fn add_vitals(
    conn: &Connection,
    rng: &mut SplitMix64,
    person: PersonId,
    birth_year: i32,
    places: &[PlaceId],
) -> Result<()> {
    Store::add_event_in(
        conn,
        &NewEvent {
            subject: EventSubject::Individual(person),
            kind: EventKind::Birth,
            date: year_date(birth_year)?,
            place: Some(places[(person.get() as usize) % places.len()]),
            notes: None,
        },
    )?;
    let death_year = birth_year + 60 + rng.below(25) as i32;
    Store::add_event_in(
        conn,
        &NewEvent {
            subject: EventSubject::Individual(person),
            kind: EventKind::Death,
            date: year_date(death_year)?,
            place: None,
            notes: None,
        },
    )?;
    Ok(())
}

/// Parses a plain integer year into a [`GenealogicalDate`](crate::date::GenealogicalDate).
/// An integer year always parses; the `?` keeps `seed_synthetic` panic-free regardless.
fn year_date(year: i32) -> Result<Option<crate::date::GenealogicalDate>> {
    let date = year
        .to_string()
        .parse()
        .map_err(|e| CoreError::Validation(format!("synthetic year {year}: {e:?}")))?;
    Ok(Some(date))
}
