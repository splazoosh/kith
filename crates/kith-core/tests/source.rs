//! `Store` source/citation CRUD: create/read/update/delete a source,
//! attach a citation to each of an event/person/family and read it back with its
//! source resolved, the `ON DELETE CASCADE` from a source to its citations, and
//! the confidence serialize-vs-store split.

use kith_core::prelude::*;

fn fresh() -> Store {
    Store::open_in_memory().expect("open in-memory store")
}

/// A person, a family, and an event — the three citation subjects.
fn subjects(store: &Store) -> (PersonId, FamilyId, EventId) {
    let person = store
        .create_individual(&NewIndividual::default())
        .expect("person")
        .id;
    let family = store
        .create_family(&NewFamily::default())
        .expect("family")
        .id;
    let event = store
        .add_event(&NewEvent {
            subject: EventSubject::Individual(person),
            kind: EventKind::Birth,
            date: None,
            place: None,
            notes: None,
        })
        .expect("event")
        .id;
    (person, family, event)
}

#[test]
fn source_create_read_update_delete_round_trip() {
    // Arrange / Act — create.
    let store = fresh();
    let created = store
        .create_source(&NewSource {
            title: "Bergen Parish Register".to_owned(),
            author: Some("Den norske kirke".to_owned()),
            publication: None,
            repository: Some("Statsarkivet i Bergen".to_owned()),
            notes: None,
        })
        .expect("create source");

    // Assert — read back.
    let got = store.get_source(created.id).expect("get source");
    assert_eq!(got, created);
    assert_eq!(store.list_sources().expect("list").len(), 1);

    // Act / Assert — update.
    let updated = store
        .update_source(
            created.id,
            &NewSource {
                title: "Bergen Parish Register (1850–1900)".to_owned(),
                ..NewSource::default()
            },
        )
        .expect("update source");
    assert_eq!(updated.title, "Bergen Parish Register (1850–1900)");
    assert_eq!(updated.repository, None, "update replaces all fields");
    assert_eq!(store.get_source(created.id).expect("re-get"), updated);

    // Act / Assert — delete.
    store.delete_source(created.id).expect("delete source");
    assert!(matches!(
        store.get_source(created.id),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn update_or_delete_of_a_missing_source_is_not_found() {
    let store = fresh();
    let missing = SourceId::new(999);
    assert!(matches!(
        store.update_source(missing, &NewSource::default()),
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        store.delete_source(missing),
        Err(CoreError::NotFound { .. })
    ));
}

#[test]
fn a_citation_attaches_to_each_subject_and_resolves_its_source() {
    let store = fresh();
    let (person, family, event) = subjects(&store);
    let source = store
        .create_source(&NewSource {
            title: "Census 1865".to_owned(),
            ..NewSource::default()
        })
        .expect("source");

    for subject in [
        CitationSubject::Individual(person),
        CitationSubject::Family(family),
        CitationSubject::Event(event),
    ] {
        let added = store
            .add_citation(&NewCitation {
                source: source.id,
                subject,
                page: Some("p. 12".to_owned()),
                detail: None,
                confidence: Some(Confidence::Primary),
            })
            .expect("add citation");
        assert_eq!(added.subject, subject);

        let items = store.citations_for(subject).expect("citations_for");
        assert_eq!(items.len(), 1, "exactly the one citation for {subject:?}");
        assert_eq!(items[0].citation.subject, subject);
        assert_eq!(items[0].citation.page.as_deref(), Some("p. 12"));
        // The source is resolved in the same read (no N+1).
        assert_eq!(items[0].source, source);
    }

    // The source supports all three facts.
    assert_eq!(
        store
            .list_citations_for_source(source.id)
            .expect("for source")
            .len(),
        3
    );
}

#[test]
fn deleting_a_source_cascades_its_citations() {
    let store = fresh();
    let (_, _, event) = subjects(&store);
    let source = store
        .create_source(&NewSource {
            title: "Probate".to_owned(),
            ..NewSource::default()
        })
        .expect("source");
    store
        .add_citation(&NewCitation {
            source: source.id,
            subject: CitationSubject::Event(event),
            page: None,
            detail: None,
            confidence: None,
        })
        .expect("citation");
    assert_eq!(
        store
            .citations_for(CitationSubject::Event(event))
            .expect("before")
            .len(),
        1
    );

    store.delete_source(source.id).expect("delete source");

    assert!(
        store
            .citations_for(CitationSubject::Event(event))
            .expect("after")
            .is_empty(),
        "deleting a source cascades (ON DELETE CASCADE) to its citations"
    );
}

#[test]
fn confidence_serializes_by_variant_name_and_stores_the_text_code() {
    let store = fresh();
    let (_, _, event) = subjects(&store);
    let source = store
        .create_source(&NewSource {
            title: "Letter".to_owned(),
            ..NewSource::default()
        })
        .expect("source");
    let citation = store
        .add_citation(&NewCitation {
            source: source.id,
            subject: CitationSubject::Event(event),
            page: None,
            detail: None,
            confidence: Some(Confidence::Primary),
        })
        .expect("citation");

    // serde uses the variant name (the wire/CLI form); the TEXT-code storage
    // (`"primary"`) is proven by the `model::enums` unit suite.
    let json = serde_json::to_string(&citation).expect("serialize");
    assert!(
        json.contains("\"confidence\":\"Primary\""),
        "confidence serializes by variant name, got {json}"
    );

    // It round-trips back through SQLite as the same typed value.
    let stored = store
        .citations_for(CitationSubject::Event(event))
        .expect("read");
    assert_eq!(stored[0].citation.confidence, Some(Confidence::Primary));
}
