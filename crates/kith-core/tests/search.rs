//! Public-API tests for the FTS5-backed [`Store::search`]:
//! multi-field match (names / alternate names / nickname / notes / event
//! places), `bm25` ranking with deterministic ties, query sanitization against
//! FTS metacharacters, diacritic folding, trigger-sync on name/place edits, the
//! bulk-import-then-search path, and no-resurrection on delete.

use kith_core::gedcom::{ImportOptions, import};
use kith_core::prelude::*;

fn fresh() -> Store {
    Store::open_in_memory().expect("open in-memory store")
}

/// Create a person with a given/surname, returning the id.
fn person(store: &Store, given: &str, surname: &str) -> PersonId {
    store
        .create_individual(&NewIndividual {
            given_name: Some(given.to_owned()),
            surname: Some(surname.to_owned()),
            ..NewIndividual::default()
        })
        .expect("create individual")
        .id
}

/// Add a birth event at a freshly-created place, returning nothing.
fn birth_at(store: &Store, who: PersonId, place_name: &str) {
    let place = store
        .create_place(&NewPlace {
            name: place_name.to_owned(),
            latitude: None,
            longitude: None,
            parent: None,
        })
        .expect("create place");
    store
        .add_event(&NewEvent {
            subject: EventSubject::Individual(who),
            kind: EventKind::Birth,
            date: None,
            place: Some(place),
            notes: None,
        })
        .expect("add birth event");
}

/// The ids returned by a search, in result order.
fn ids(hits: &[SearchHit]) -> Vec<i64> {
    hits.iter().map(|h| h.individual.id.get()).collect()
}

#[test]
fn finds_a_person_by_name_maiden_name_nickname_note_and_birthplace() {
    let store = fresh();
    let ada = store
        .create_individual(&NewIndividual {
            given_name: Some("Ada".to_owned()),
            surname: Some("Lovelace".to_owned()),
            nickname: Some("Countess".to_owned()),
            notes: Some("a pioneer of computing".to_owned()),
            ..NewIndividual::default()
        })
        .expect("create Ada")
        .id;
    store
        .add_name(&NewName {
            individual_id: ada,
            kind: NameKind::Birth,
            given_name: Some("Ada".to_owned()),
            surname: Some("Byron".to_owned()), // her maiden name
            name_prefix: None,
            name_suffix: None,
            sort_order: 0,
        })
        .expect("add maiden name");
    birth_at(&store, ada, "London, England");

    for needle in ["Lovelace", "Byron", "Countess", "computing", "London"] {
        let hits = store.search(needle, 50).expect("search");
        assert_eq!(ids(&hits), vec![ada.get()], "{needle:?} should surface Ada");
    }
}

#[test]
fn ranks_a_name_hit_above_a_place_hit() {
    let store = fresh();
    // A matches "Bergen" in the (highly-weighted) names column…
    let by_name = person(&store, "Bergen", "Hansen");
    // …B only via a birthplace (the low-weighted places column).
    let by_place = person(&store, "Ola", "Nordmann");
    birth_at(&store, by_place, "Bergen, Norway");

    let hits = store.search("Bergen", 50).expect("search");
    assert_eq!(
        ids(&hits),
        vec![by_name.get(), by_place.get()],
        "a name hit outranks a place hit (bm25 column weights)"
    );
}

#[test]
fn ties_break_by_surname_then_given_then_id() {
    let store = fresh();
    // Two identical "Ann Smith" rows (a true bm25 tie → id breaks it) plus a
    // "Bob Smith" (given-name breaks it). All match the surname token "Smith".
    let ann1 = person(&store, "Ann", "Smith");
    let ann2 = person(&store, "Ann", "Smith");
    let bob = person(&store, "Bob", "Smith");

    let hits = store.search("Smith", 50).expect("search");
    assert_eq!(
        ids(&hits),
        vec![ann1.get(), ann2.get(), bob.get()],
        "ties order by surname, given_name, id — and are reproducible"
    );
    // Determinism: a second identical query yields the identical order.
    assert_eq!(
        ids(&store.search("Smith", 50).expect("search again")),
        ids(&hits)
    );
}

#[test]
fn prefix_matches_as_you_type() {
    let store = fresh();
    let ada = person(&store, "Ada", "Lovelace");
    // A partial term finds the full name (the sanitizer appends `*`).
    assert_eq!(
        ids(&store.search("Lov", 50).expect("search")),
        vec![ada.get()]
    );
    assert_eq!(
        ids(&store.search("Ad Lov", 50).expect("search")),
        vec![ada.get()]
    );
}

#[test]
fn folds_decomposable_diacritics_so_ascii_spellings_match() {
    let store = fresh();
    // `unicode61 remove_diacritics 2` folds accents that have a canonical
    // decomposition (base letter + combining mark): ü→u, é→e, å→a.
    let muller = person(&store, "Hans", "Müller");
    let desiree = person(&store, "Désirée", "Clary");
    let hakon = person(&store, "Håkon", "Sverresson");
    assert_eq!(
        ids(&store.search("Muller", 50).expect("search")),
        vec![muller.get()]
    );
    assert_eq!(
        ids(&store.search("Desiree", 50).expect("search")),
        vec![desiree.get()]
    );
    assert_eq!(
        ids(&store.search("Hakon", 50).expect("search")),
        vec![hakon.get()]
    );
}

#[test]
fn stroke_letters_without_a_decomposition_are_not_folded() {
    // `unicode61 remove_diacritics 2` cannot
    // fold letters Unicode classifies as distinct rather than "base + diacritic"
    // — `ø`, `ð`, `ł`, `ß`. So "Bjorn" does NOT match "Bjørn"; the accented
    // spelling does. A custom tokenizer (the `rusqlite/fts5` feature) would be
    // needed to bridge these — deliberately out of scope for v1. This test
    // pins the known behaviour so a future tokenizer change is a conscious one.
    let store = fresh();
    let bjorn = person(&store, "Bjørn", "Dahl");
    assert!(
        store.search("Bjorn", 50).expect("search").is_empty(),
        "ø is not folded to o by unicode61"
    );
    assert_eq!(
        ids(&store.search("Bjørn", 50).expect("search")),
        vec![bjorn.get()],
        "the accented spelling matches"
    );
}

#[test]
fn a_query_of_fts_metacharacters_does_not_error() {
    let store = fresh();
    person(&store, "Ada", "Lovelace");
    // A lone quote / star / colon / boolean keyword must never reach FTS raw.
    for q in ["\"", "*", ":", "AND", "OR", "* ? \"", "Lovelace\"*", "a:b"] {
        assert!(store.search(q, 50).is_ok(), "{q:?} must not error");
    }
}

#[test]
fn an_empty_query_lists_everyone_bounded_by_limit() {
    let store = fresh();
    person(&store, "Zara", "Adams");
    person(&store, "Jane", "Doe");
    person(&store, "John", "Doe");

    // Empty query → name-ordered (surname, given) full list, capped by `limit`.
    let two = store.search("   ", 2).expect("search");
    assert_eq!(two.len(), 2);
    assert_eq!(
        two.iter()
            .map(|h| h.individual.surname.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["Adams".to_owned(), "Doe".to_owned()],
        "the empty-query slice is name-ordered"
    );
    assert!(
        two.iter().all(|h| h.context.is_none()),
        "no why-matched for the list path"
    );
}

#[test]
fn editing_a_name_updates_the_next_search() {
    let store = fresh();
    let p = person(&store, "Jane", "Doe");
    assert!(store.search("Smithson", 50).expect("search").is_empty());

    // Attaching an alternate name reindexes the person (the names trigger).
    store
        .add_name(&NewName {
            individual_id: p,
            kind: NameKind::Married,
            given_name: None,
            surname: Some("Smithson".to_owned()),
            name_prefix: None,
            name_suffix: None,
            sort_order: 0,
        })
        .expect("add name");
    assert_eq!(
        ids(&store.search("Smithson", 50).expect("search")),
        vec![p.get()]
    );
}

// The place-rename trigger fan-out is exercised as a unit test in
// `db/search.rs` (it needs a raw `UPDATE places` — there is no public
// rename-place API yet, places being created only via events).

#[test]
fn a_bulk_imported_person_is_immediately_searchable() {
    // The GEDCOM importer writes through unchanged `*_in` helpers; the sync
    // triggers keep the index current, so an imported person is found at once —
    // by name AND by birthplace — with zero importer changes.
    let store = fresh();
    import(
        &store,
        "0 HEAD\n1 CHAR UTF-8\n\
         0 @I1@ INDI\n1 NAME Ada /Lovelace/\n1 BIRT\n2 PLAC London, England\n0 TRLR\n",
        &ImportOptions::default(),
    )
    .expect("import");

    let by_name = store.search("Lovelace", 50).expect("search by name");
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].individual.surname.as_deref(), Some("Lovelace"));
    assert_eq!(
        store.search("England", 50).expect("search by place").len(),
        1,
        "the imported birthplace is indexed"
    );
}

#[test]
fn deleting_a_person_removes_them_from_results_without_resurrection() {
    let store = fresh();
    let p = person(&store, "Ada", "Lovelace");
    store
        .add_name(&NewName {
            individual_id: p,
            kind: NameKind::Birth,
            given_name: None,
            surname: Some("Byron".to_owned()),
            name_prefix: None,
            name_suffix: None,
            sort_order: 0,
        })
        .expect("name");
    birth_at(&store, p, "London");
    assert_eq!(store.search("Lovelace", 50).expect("search").len(), 1);

    // Deleting cascades names + events; the index row must not be resurrected by
    // the child-delete triggers (the `FROM individuals` join is the guard).
    store.delete_individual(p).expect("delete");
    for needle in ["Lovelace", "Byron", "London"] {
        assert!(
            store.search(needle, 50).expect("search").is_empty(),
            "{needle:?} must not surface a deleted person"
        );
    }
}

#[test]
fn the_context_snippet_reflects_the_match() {
    let store = fresh();
    person(&store, "Ada", "Lovelace");
    // A matched search carries a why-matched snippet drawn from the indexed text.
    let hits = store.search("Lovelace", 50).expect("search");
    assert_eq!(hits.len(), 1);
    let ctx = hits[0].context.as_deref().unwrap_or_default();
    assert!(
        ctx.contains("Lovelace"),
        "context {ctx:?} should show the match"
    );
}
