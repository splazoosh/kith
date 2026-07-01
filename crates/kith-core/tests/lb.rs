//! Integration coverage for the LB JSON importer: family synthesis from parent
//! pointers, the unset-date sentinels (`01.01.1753`, `05.01.2021`) and the
//! future-date guard, the born-after-death birth drop, the inferred living flag
//! (recent birth vs. death-date/death-place evidence), place dedup + place-only
//! events, symmetric spouse-pair dedup, merge/non-merge semantics, the
//! atomic-failure paths, and a real-world whole-file import.

use std::path::Path;

use kith_core::lb;
use kith_core::prelude::{
    CoreError, EventKind, EventSubject, Family, ImportOptions, ImportSummary, Individual, PersonId,
    Sex, Store,
};

const SAMPLE: &str = include_str!("fixtures/lb_sample.json");

/// Import the hand-built sample into a fresh in-memory store.
fn import_sample() -> (Store, ImportSummary) {
    let store = Store::open_in_memory().expect("open store");
    let summary = lb::import(&store, SAMPLE, &ImportOptions::default()).expect("import LB sample");
    (store, summary)
}

/// Find the single individual with this given + surname (panics if not unique).
fn person(store: &Store, given: &str, surname: &str) -> Individual {
    let mut hits: Vec<Individual> = store
        .list_individuals()
        .expect("list individuals")
        .into_iter()
        .filter(|i| i.given_name.as_deref() == Some(given) && i.surname.as_deref() == Some(surname))
        .collect();
    assert_eq!(hits.len(), 1, "expected exactly one {given} {surname}");
    hits.remove(0)
}

/// The set of partner ids actually filled on a family.
fn partners(fam: &Family) -> Vec<PersonId> {
    [fam.partner1, fam.partner2].into_iter().flatten().collect()
}

#[test]
fn summary_counts_cover_individuals_families_events_and_places() {
    let (_store, summary) = import_sample();
    assert_eq!(summary.individuals, 8);
    // (1,2) + (1,·) + couple(6,7) — three synthesized families.
    assert_eq!(summary.families, 3);
    // Hans: birth + death; Anne: birth; Olea: place-only birth.
    assert_eq!(summary.events, 4);
    // Bergen (shared across three events) + Oslo, deduped.
    assert_eq!(summary.places, 2);
    // LB has no alternate names / media / sources / citations.
    assert_eq!(summary.names, 0);
    assert_eq!(summary.media, 0);
    assert_eq!(summary.sources, 0);
    assert_eq!(summary.citations, 0);
    assert!(summary.skipped_tags.is_empty());
}

#[test]
fn fields_map_onto_the_individual() {
    let (store, _) = import_sample();
    let hans = person(&store, "Hans", "Olsen");
    assert_eq!(hans.sex, Sex::Male);
    assert_eq!(hans.notes.as_deref(), Some("Patriark."));
    // Hans has a real death date (1890) → deceased.
    assert!(!hans.living);
    assert_eq!(person(&store, "Marta", "Nilsdatter").sex, Sex::Female);
}

#[test]
fn parent_pairs_become_families_with_their_children() {
    let (store, _) = import_sample();
    let hans = person(&store, "Hans", "Olsen");
    let marta = person(&store, "Marta", "Nilsdatter");
    let anne = person(&store, "Anne", "Hansdatter");
    let ole = person(&store, "Ole", "Hansen");
    let lars = person(&store, "Lars", "Hansen");

    // Hans is a partner in two families: (Hans, Marta) and (Hans, —).
    let hans_families = store.families_of_partner(hans.id).expect("families");
    assert_eq!(hans_families.len(), 2);

    // The two-partner family carries Anne and Ole in input order.
    let two_parent = hans_families
        .iter()
        .find(|f| f.partner2.is_some())
        .expect("a family with both partners");
    let mut tp = partners(two_parent);
    tp.sort();
    let mut expected = vec![hans.id, marta.id];
    expected.sort();
    assert_eq!(tp, expected);
    let kids: Vec<PersonId> = store
        .list_children(two_parent.id)
        .expect("children")
        .into_iter()
        .map(|c| c.child_id)
        .collect();
    assert_eq!(kids, vec![anne.id, ole.id]);

    // The single-parent family (father only) carries Lars.
    let one_parent = hans_families
        .iter()
        .find(|f| f.partner2.is_none())
        .expect("a single-parent family");
    assert_eq!(one_parent.partner1, Some(hans.id));
    let kids: Vec<PersonId> = store
        .list_children(one_parent.id)
        .expect("children")
        .into_iter()
        .map(|c| c.child_id)
        .collect();
    assert_eq!(kids, vec![lars.id]);
}

#[test]
fn the_unknown_date_sentinel_creates_no_event() {
    let (store, _) = import_sample();
    // Marta has sentinel birth + death and no places → zero events.
    let marta = person(&store, "Marta", "Nilsdatter");
    assert!(
        store
            .list_events_for(EventSubject::Individual(marta.id))
            .expect("events")
            .is_empty()
    );
    // Hans has real dates → a dated birth and death.
    let hans = person(&store, "Hans", "Olsen");
    let hans_events = store
        .list_events_for(EventSubject::Individual(hans.id))
        .expect("events");
    assert_eq!(hans_events.len(), 2);
    assert!(
        hans_events
            .iter()
            .all(|e| e.date.is_some() && e.place.is_some())
    );
}

#[test]
fn the_export_default_stamp_and_future_dates_create_no_event() {
    // Neither the fixed `05.01.2021` export-run stamp (a birth) nor a far-future
    // date (a death) is a real event, and with no place there is nothing to
    // anchor a place-only event to → the person imports with zero events.
    let store = Store::open_in_memory().expect("open store");
    let json = r#"[
      {"Id":1,"Gender":"M","FirstName":"Ex","LastName":"Port",
       "BirthDate":"05.01.2021","DeathDate":"01.01.3000"}
    ]"#;
    let summary = lb::import(&store, json, &ImportOptions::default()).expect("import");
    assert_eq!(summary.individuals, 1);
    assert_eq!(summary.events, 0, "sentinel + future dates yield no events");
    let person = &store.list_individuals().expect("list")[0];
    assert!(
        store
            .list_events_for(EventSubject::Individual(person.id))
            .expect("events")
            .is_empty()
    );
}

#[test]
fn a_recent_birth_without_a_death_is_inferred_living() {
    // No death evidence and a birth well within a human lifespan → living (so the
    // person is redacted from exports), unlike the undated ancestors that default
    // to deceased. 2020 stays inside the 110-year window for the app's lifetime.
    let store = Store::open_in_memory().expect("open store");
    let json = r#"[
      {"Id":1,"Gender":"F","FirstName":"Nyleg","LastName":"Levande",
       "BirthDate":"01.01.2020","DeathDate":""}
    ]"#;
    lb::import(&store, json, &ImportOptions::default()).expect("import");
    let person = &store.list_individuals().expect("list")[0];
    assert!(
        person.living,
        "a recent birth with no death evidence should read as living"
    );
}

#[test]
fn a_death_place_without_a_death_date_still_reads_deceased() {
    // A recent birth but a recorded place of death (with only a sentinel death
    // date) → the death place is death evidence, so the person is deceased —
    // never wrongly flipped to living by the recent birth.
    let store = Store::open_in_memory().expect("open store");
    let json = r#"[
      {"Id":1,"Gender":"M","FirstName":"Dod","LastName":"Stad",
       "BirthDate":"01.01.2020","DeathDate":"01.01.1753","DeathPlace":"Bergen"}
    ]"#;
    lb::import(&store, json, &ImportOptions::default()).expect("import");
    let person = &store.list_individuals().expect("list")[0];
    assert!(
        !person.living,
        "a recorded death place is death evidence → deceased"
    );
    // The place-only death event is still created.
    let events = store
        .list_events_for(EventSubject::Individual(person.id))
        .expect("events");
    assert!(
        events
            .iter()
            .any(|e| e.kind == EventKind::Death && e.place.is_some())
    );
}

#[test]
fn a_birth_after_the_death_is_dropped_keeping_the_death() {
    // A stray modern export stamp in the birth field (2022) beside a real
    // historical death (1787): the impossible birth is discarded, the death is
    // kept, and the person reads as deceased.
    let store = Store::open_in_memory().expect("open store");
    let json = r#"[
      {"Id":1,"Gender":"M","FirstName":"Herman","LastName":"Skomo",
       "BirthPlace":"","BirthDate":"01.01.2022","DeathDate":"01.01.1787"}
    ]"#;
    lb::import(&store, json, &ImportOptions::default()).expect("import");
    let person = &store.list_individuals().expect("list")[0];
    assert!(!person.living, "a person with a real death is deceased");

    // The birth stamp carried no place, so dropping its date leaves no birth
    // event — only the death survives, with its real date.
    let events = store
        .list_events_for(EventSubject::Individual(person.id))
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Death);
    assert!(events[0].date.is_some());
}

#[test]
fn a_place_without_a_date_still_yields_a_birth_event() {
    let (store, _) = import_sample();
    // Olea: sentinel birth date but a known birthplace → an undated birth event.
    let olea = person(&store, "Olea", "Alfsdatter");
    let events = store
        .list_events_for(EventSubject::Individual(olea.id))
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Birth);
    assert!(events[0].date.is_none());
    assert!(events[0].place.is_some());
}

#[test]
fn symmetric_spouse_pointers_collapse_to_one_family() {
    let (store, _) = import_sample();
    let per = person(&store, "Per", "Andersen");
    let kari = person(&store, "Kari", "Persdatter");

    let per_families = store.families_of_partner(per.id).expect("families");
    assert_eq!(
        per_families.len(),
        1,
        "Per↔Kari must form exactly one family"
    );
    let couple = &per_families[0];
    let mut got = partners(couple);
    got.sort();
    let mut expected = vec![per.id, kari.id];
    expected.sort();
    assert_eq!(got, expected);
    // A couple with no shared children has no child rows.
    assert!(store.list_children(couple.id).expect("children").is_empty());
    // Kari resolves to the same family (the back-pointer did not create a second).
    assert_eq!(
        store.families_of_partner(kari.id).expect("families")[0].id,
        couple.id
    );
}

#[test]
fn non_merge_into_a_nonempty_store_is_rejected_and_changes_nothing() {
    let (store, _) = import_sample();
    let err = lb::import(&store, SAMPLE, &ImportOptions::default()).expect_err("must reject");
    assert!(matches!(err, CoreError::Validation(_)));
    assert_eq!(store.list_individuals().expect("list").len(), 8);
}

#[test]
fn merge_appends_into_a_populated_store() {
    let (store, _) = import_sample();
    let mut opts = ImportOptions::default();
    opts.merge = true;
    let second = lb::import(&store, SAMPLE, &opts).expect("merge import");
    assert_eq!(second.individuals, 8);
    assert_eq!(store.list_individuals().expect("list").len(), 16);
    assert_eq!(store.list_families().expect("list").len(), 6);
}

#[test]
fn malformed_json_is_a_validation_error_and_writes_nothing() {
    let store = Store::open_in_memory().expect("open store");
    let err = lb::import(&store, "{ not json", &ImportOptions::default()).expect_err("must fail");
    assert!(matches!(err, CoreError::Validation(_)));
    assert!(store.list_individuals().expect("list").is_empty());
}

#[test]
fn a_dangling_parent_pointer_is_rejected_before_any_write() {
    let json = r#"[{"Id":1,"FatherId":99,"Gender":"M","FirstName":"X","LastName":"Y"}]"#;
    let store = Store::open_in_memory().expect("open store");
    let err = lb::import(&store, json, &ImportOptions::default()).expect_err("must fail");
    assert!(matches!(err, CoreError::Validation(_)));
    assert!(store.list_individuals().expect("list").is_empty());
}

#[test]
fn duplicate_and_zero_ids_are_rejected() {
    let store = Store::open_in_memory().expect("open store");
    let dup = lb::import(&store, r#"[{"Id":1},{"Id":1}]"#, &ImportOptions::default())
        .expect_err("duplicate id");
    assert!(matches!(dup, CoreError::Validation(_)));

    let zero = lb::import(&store, r#"[{"Id":0}]"#, &ImportOptions::default()).expect_err("zero id");
    assert!(matches!(zero, CoreError::Validation(_)));
}

#[test]
fn empty_array_imports_cleanly_to_nothing() {
    let store = Store::open_in_memory().expect("open store");
    let summary = lb::import(&store, "[]", &ImportOptions::default()).expect("empty import");
    assert_eq!(summary, ImportSummary::default());
}

#[test]
fn the_reference_file_imports_deterministically() {
    // The maintainer's real LB export lives in docs/. Resolve it relative to the
    // crate manifest so the test is CWD-independent.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/Persons5.json");
    // The file is gitignored (private data), so it is absent in CI and fresh
    // checkouts. Run the full determinism check where it is present; skip cleanly
    // where it is not, rather than failing on a fixture that cannot be shipped.
    let Ok(source) = std::fs::read_to_string(&path) else {
        eprintln!("skipping: {} not present", path.display());
        return;
    };

    let store = Store::open_in_memory().expect("open store");
    let summary = lb::import(&store, &source, &ImportOptions::default()).expect("import reference");
    assert_eq!(summary.individuals, 268);
    assert_eq!(summary.families, 127); // distinct (father, mother) parent pairs
    assert!(summary.places > 0);

    // Determinism: a second import into a fresh store yields identical counts.
    let store2 = Store::open_in_memory().expect("open store2");
    let summary2 = lb::import(&store2, &source, &ImportOptions::default()).expect("re-import");
    assert_eq!(summary, summary2);
}
