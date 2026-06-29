//! Integration tests for the undo primitive
//! ([`Store::delete_undoable`] / [`Store::restore_deletion`]).
//!
//! Each of the eight delete targets round-trips through the public API: a
//! delete-then-restore brings the row(s) back with their **original ids** and
//! their full cascade set. The headline case is an individual who is a partner in
//! two families, a child in a third, and carries names, an event (with its own
//! citation + media link), a direct citation, and a direct media link — every
//! branch of [`Deletion::Individual`], including the `SET NULL` partner pointers.
//! The id-reuse conflict (delete the max id, create a new row that takes it, then
//! restore) must surface a typed error and leave the database unchanged.

use std::path::Path;

use kith_core::prelude::*;

/// A 1×1 PNG; the media CRUD only copies the bytes, so any content works.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn fresh() -> Store {
    Store::open_in_memory().expect("open in-memory store")
}

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

fn source(store: &Store, title: &str) -> SourceId {
    store
        .create_source(&NewSource {
            title: title.to_owned(),
            ..NewSource::default()
        })
        .expect("create source")
        .id
}

/// Writes the tiny PNG into `dir` as `name` and imports it for `subject`.
fn import_image(store: &Store, media_root: &Path, dir: &Path, name: &str, subject: MediaSubject) {
    let src = dir.join(name);
    std::fs::write(&src, TINY_PNG).expect("write source image");
    store
        .import_media(media_root, &src, subject, true)
        .expect("import media");
}

#[test]
fn individual_round_trips_with_its_entire_cascade_and_partner_pointers() {
    // Arrange — a rich person: a name, an event carrying a citation + a media
    // link, a direct citation, a direct media link, a child membership, and two
    // partner families (one in each slot).
    let store = fresh();
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");

    let p = person(&store, "Ada", "Lovelace");
    let spouse_a = person(&store, "Spouse", "A");
    let spouse_b = person(&store, "Spouse", "B");
    let parent = person(&store, "Parent", "P");

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
        .expect("add name");

    let f1 = store
        .create_family(&NewFamily {
            partner1: Some(p),
            partner2: Some(spouse_a),
            union_type: UnionType::Marriage,
            ..NewFamily::default()
        })
        .expect("family 1");
    let f2 = store
        .create_family(&NewFamily {
            partner1: Some(spouse_b),
            partner2: Some(p), // p is the *second* partner here
            union_type: UnionType::Partnership,
            ..NewFamily::default()
        })
        .expect("family 2");
    let birth_family = store
        .create_family(&NewFamily {
            partner1: Some(parent),
            ..NewFamily::default()
        })
        .expect("birth family");
    store
        .add_child(birth_family.id, p, ChildRelation::Birth, 0)
        .expect("add child");

    let event = store
        .add_event(&NewEvent {
            subject: EventSubject::Individual(p),
            kind: EventKind::Birth,
            date: Some("ABT 1815".parse().expect("date")),
            place: None,
            notes: Some("recorded".to_owned()),
        })
        .expect("add event");
    let src = source(&store, "Parish Register");
    store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Event(event.id),
            page: Some("p. 7".to_owned()),
            detail: None,
            confidence: Some(Confidence::Primary),
        })
        .expect("event citation");
    store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Individual(p),
            page: None,
            detail: Some("a note".to_owned()),
            confidence: None,
        })
        .expect("person citation");
    import_image(
        &store,
        &media_root,
        tmp.path(),
        "face.png",
        MediaSubject::Event(event.id),
    );
    import_image(
        &store,
        &media_root,
        tmp.path(),
        "portrait.png",
        MediaSubject::Individual(p),
    );

    // Snapshot the world via the public API before the delete.
    let individual_before = store.get_individual(p).expect("get");
    let names_before = store.list_names(p).expect("names");
    let partner_fams_before = store.families_of_partner(p).expect("partner fams");
    let child_fams_before = store.families_of_child(p).expect("child fams");
    let events_before = store
        .list_events_for(EventSubject::Individual(p))
        .expect("events");
    let person_cites_before = store
        .citations_for(CitationSubject::Individual(p))
        .expect("person cites");
    let event_cites_before = store
        .citations_for(CitationSubject::Event(event.id))
        .expect("event cites");
    let person_media_before = store
        .list_media_for(MediaSubject::Individual(p))
        .expect("person media");
    let event_media_before = store
        .list_media_for(MediaSubject::Event(event.id))
        .expect("event media");
    let f1_before = store.get_family(f1.id).expect("f1");
    let f2_before = store.get_family(f2.id).expect("f2");

    // Act — delete (undoable), then restore.
    let deletion = store
        .delete_undoable(DeleteTarget::Individual(p))
        .expect("delete undoable");

    // The delete really happened: the person is gone and the partner pointers nulled.
    assert!(matches!(
        store.get_individual(p),
        Err(CoreError::NotFound { .. })
    ));
    assert_eq!(store.get_family(f1.id).expect("f1 after").partner1, None);
    assert_eq!(store.get_family(f2.id).expect("f2 after").partner2, None);

    store.restore_deletion(&deletion).expect("restore");

    // Assert — every read matches its before-snapshot (same ids, rows, relationships).
    assert_eq!(
        store.get_individual(p).expect("get after"),
        individual_before
    );
    assert_eq!(store.list_names(p).expect("names after"), names_before);
    assert_eq!(
        store.families_of_partner(p).expect("partner fams after"),
        partner_fams_before
    );
    assert_eq!(
        store.families_of_child(p).expect("child fams after"),
        child_fams_before
    );
    assert_eq!(
        store
            .list_events_for(EventSubject::Individual(p))
            .expect("events after"),
        events_before
    );
    assert_eq!(
        store
            .citations_for(CitationSubject::Individual(p))
            .expect("person cites after"),
        person_cites_before
    );
    assert_eq!(
        store
            .citations_for(CitationSubject::Event(event.id))
            .expect("event cites after"),
        event_cites_before
    );
    assert_eq!(
        store
            .list_media_for(MediaSubject::Individual(p))
            .expect("person media after"),
        person_media_before
    );
    assert_eq!(
        store
            .list_media_for(MediaSubject::Event(event.id))
            .expect("event media after"),
        event_media_before
    );
    // The marriage links came back (the SET NULL pointers restored).
    assert_eq!(
        store.get_family(f1.id).expect("f1 after restore"),
        f1_before
    );
    assert_eq!(
        store.get_family(f2.id).expect("f2 after restore"),
        f2_before
    );
    assert_eq!(store.get_family(f1.id).expect("f1").partner1, Some(p));
    assert_eq!(store.get_family(f2.id).expect("f2").partner2, Some(p));
    // The untouched relatives are unchanged.
    for relative in [spouse_a, spouse_b, parent] {
        assert!(store.get_individual(relative).is_ok());
    }
}

#[test]
fn family_round_trips_with_children_events_and_citations() {
    let store = fresh();
    let p1 = person(&store, "A", "One");
    let p2 = person(&store, "B", "Two");
    let kid = person(&store, "Kid", "Three");
    let fam = store
        .create_family(&NewFamily {
            partner1: Some(p1),
            partner2: Some(p2),
            union_type: UnionType::Marriage,
            ..NewFamily::default()
        })
        .expect("family");
    store
        .add_child(fam.id, kid, ChildRelation::Birth, 0)
        .expect("child");
    let marriage = store
        .add_event(&NewEvent {
            subject: EventSubject::Family(fam.id),
            kind: EventKind::Marriage,
            date: Some("1850".parse().expect("date")),
            place: None,
            notes: None,
        })
        .expect("marriage event");
    let src = source(&store, "Marriage Register");
    store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Event(marriage.id),
            page: None,
            detail: None,
            confidence: None,
        })
        .expect("event citation");
    store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Family(fam.id),
            page: None,
            detail: None,
            confidence: None,
        })
        .expect("family citation");

    let fam_before = store.get_family(fam.id).expect("fam");
    let children_before = store.list_children(fam.id).expect("children");
    let events_before = store
        .list_events_for(EventSubject::Family(fam.id))
        .expect("events");
    let cites_before = store
        .citations_for(CitationSubject::Family(fam.id))
        .expect("cites");

    let deletion = store
        .delete_undoable(DeleteTarget::Family(fam.id))
        .expect("delete");
    assert!(matches!(
        store.get_family(fam.id),
        Err(CoreError::NotFound { .. })
    ));
    // The partner individuals survive a family delete.
    assert!(store.get_individual(p1).is_ok() && store.get_individual(p2).is_ok());

    store.restore_deletion(&deletion).expect("restore");
    assert_eq!(store.get_family(fam.id).expect("fam after"), fam_before);
    assert_eq!(
        store.list_children(fam.id).expect("children after"),
        children_before
    );
    assert_eq!(
        store
            .list_events_for(EventSubject::Family(fam.id))
            .expect("events after"),
        events_before
    );
    assert_eq!(
        store
            .citations_for(CitationSubject::Family(fam.id))
            .expect("cites after"),
        cites_before
    );
}

#[test]
fn event_round_trips_with_its_citation() {
    let store = fresh();
    let p = person(&store, "Ada", "L");
    let event = store
        .add_event(&NewEvent {
            subject: EventSubject::Individual(p),
            kind: EventKind::Birth,
            date: Some("12 Mar 1815".parse().expect("date")),
            place: None,
            notes: None,
        })
        .expect("event");
    let src = source(&store, "Register");
    store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Event(event.id),
            page: Some("p. 1".to_owned()),
            detail: None,
            confidence: Some(Confidence::Secondary),
        })
        .expect("citation");

    let event_before = store.get_event(event.id).expect("event");
    let cites_before = store
        .citations_for(CitationSubject::Event(event.id))
        .expect("cites");

    let deletion = store
        .delete_undoable(DeleteTarget::Event(event.id))
        .expect("delete");
    assert!(matches!(
        store.get_event(event.id),
        Err(CoreError::NotFound { .. })
    ));

    store.restore_deletion(&deletion).expect("restore");
    assert_eq!(
        store.get_event(event.id).expect("event after"),
        event_before
    );
    assert_eq!(
        store
            .citations_for(CitationSubject::Event(event.id))
            .expect("cites after"),
        cites_before
    );
}

#[test]
fn name_child_citation_round_trip() {
    let store = fresh();
    let p = person(&store, "Ada", "L");
    let name = store
        .add_name(&NewName {
            individual_id: p,
            kind: NameKind::Aka,
            given_name: Some("A.".to_owned()),
            surname: None,
            name_prefix: None,
            name_suffix: None,
            sort_order: 2,
        })
        .expect("name");
    // Name.
    let names_before = store.list_names(p).expect("names");
    let d = store
        .delete_undoable(DeleteTarget::Name(name.id))
        .expect("delete name");
    assert!(store.list_names(p).expect("names").is_empty());
    store.restore_deletion(&d).expect("restore name");
    assert_eq!(store.list_names(p).expect("names after"), names_before);

    // Child link.
    let parent = person(&store, "Par", "Ent");
    let fam = store
        .create_family(&NewFamily {
            partner1: Some(parent),
            ..NewFamily::default()
        })
        .expect("family");
    store
        .add_child(fam.id, p, ChildRelation::Adopted, 3)
        .expect("child");
    let children_before = store.list_children(fam.id).expect("children");
    let d = store
        .delete_undoable(DeleteTarget::Child {
            family: fam.id,
            child: p,
        })
        .expect("delete child");
    assert!(store.list_children(fam.id).expect("children").is_empty());
    store.restore_deletion(&d).expect("restore child");
    assert_eq!(
        store.list_children(fam.id).expect("children after"),
        children_before
    );

    // Citation.
    let src = source(&store, "Reg");
    let cite = store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Individual(p),
            page: Some("p. 9".to_owned()),
            detail: None,
            confidence: None,
        })
        .expect("citation");
    let cites_before = store
        .citations_for(CitationSubject::Individual(p))
        .expect("cites");
    let d = store
        .delete_undoable(DeleteTarget::Citation(cite.id))
        .expect("delete citation");
    assert!(
        store
            .citations_for(CitationSubject::Individual(p))
            .expect("cites")
            .is_empty()
    );
    store.restore_deletion(&d).expect("restore citation");
    assert_eq!(
        store
            .citations_for(CitationSubject::Individual(p))
            .expect("cites after"),
        cites_before
    );
}

#[test]
fn source_round_trips_with_its_citations() {
    let store = fresh();
    let p = person(&store, "Ada", "L");
    let src = source(&store, "Bergen Parish Register");
    store
        .add_citation(&NewCitation {
            source: src,
            subject: CitationSubject::Individual(p),
            page: None,
            detail: None,
            confidence: None,
        })
        .expect("citation");

    let source_before = store.get_source(src).expect("source");
    let cites_before = store.list_citations_for_source(src).expect("source cites");

    let deletion = store
        .delete_undoable(DeleteTarget::Source(src))
        .expect("delete");
    assert!(matches!(
        store.get_source(src),
        Err(CoreError::NotFound { .. })
    ));
    // The cascade dropped its citation.
    assert!(
        store
            .citations_for(CitationSubject::Individual(p))
            .expect("cites")
            .is_empty()
    );

    store.restore_deletion(&deletion).expect("restore");
    assert_eq!(store.get_source(src).expect("source after"), source_before);
    assert_eq!(
        store
            .list_citations_for_source(src)
            .expect("source cites after"),
        cites_before
    );
}

#[test]
fn media_round_trips_with_its_links() {
    let store = fresh();
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let p = person(&store, "Ada", "L");
    let subject = MediaSubject::Individual(p);
    let src = tmp.path().join("face.png");
    std::fs::write(&src, TINY_PNG).expect("write image");
    let media = store
        .import_media(&media_root, &src, subject, true)
        .expect("import");

    let media_before = store.list_media_for(subject).expect("media");
    let portrait_before = store.primary_portrait(p).expect("portrait");
    assert_eq!(portrait_before, Some(media.id));

    let deletion = store
        .delete_undoable(DeleteTarget::Media(media.id))
        .expect("delete");
    assert!(matches!(
        store.get_media(media.id),
        Err(CoreError::NotFound { .. })
    ));
    assert!(store.list_media_for(subject).expect("media").is_empty());

    store.restore_deletion(&deletion).expect("restore");
    assert_eq!(
        store.list_media_for(subject).expect("media after"),
        media_before
    );
    assert_eq!(
        store.primary_portrait(p).expect("portrait after"),
        portrait_before
    );
}

#[test]
fn restore_after_id_reuse_conflicts_and_leaves_the_database_unchanged() {
    // Delete the highest-id person, create a new one (SQLite reuses the freed max
    // rowid), then restore → the explicit-id INSERT hits a PRIMARY KEY conflict,
    // the transaction rolls back, and the database is untouched.
    let store = fresh();
    let _a = person(&store, "A", "X");
    let b = person(&store, "B", "Y"); // the max id

    let deletion = store
        .delete_undoable(DeleteTarget::Individual(b))
        .expect("delete b");
    let c = store
        .create_individual(&NewIndividual {
            given_name: Some("C".to_owned()),
            ..NewIndividual::default()
        })
        .expect("create c");
    assert_eq!(c.id, b, "SQLite reuses the freed max rowid");

    let before = store.list_individuals().expect("before");
    let err = store
        .restore_deletion(&deletion)
        .expect_err("the reused id must conflict");
    assert!(matches!(err, CoreError::Database(_)), "got {err:?}");
    assert_eq!(
        store.list_individuals().expect("after"),
        before,
        "a conflicting restore rolls back, leaving the database unchanged"
    );
}

#[test]
fn restoring_the_same_snapshot_twice_conflicts() {
    let store = fresh();
    let p = person(&store, "Ada", "L");
    let deletion = store
        .delete_undoable(DeleteTarget::Individual(p))
        .expect("delete");
    store.restore_deletion(&deletion).expect("first restore");
    let err = store
        .restore_deletion(&deletion)
        .expect_err("a second restore conflicts");
    assert!(matches!(err, CoreError::Database(_)), "got {err:?}");
}

#[test]
fn delete_undoable_of_a_missing_target_is_not_found() {
    let store = fresh();
    for target in [
        DeleteTarget::Individual(PersonId::new(404)),
        DeleteTarget::Family(FamilyId::new(404)),
        DeleteTarget::Event(EventId::new(404)),
        DeleteTarget::Name(NameId::new(404)),
        DeleteTarget::Source(SourceId::new(404)),
        DeleteTarget::Citation(CitationId::new(404)),
        DeleteTarget::Media(MediaId::new(404)),
        DeleteTarget::Child {
            family: FamilyId::new(404),
            child: PersonId::new(404),
        },
    ] {
        let err = store
            .delete_undoable(target)
            .expect_err("missing target is not found");
        assert!(
            matches!(err, CoreError::NotFound { .. }),
            "got {err:?} for {target:?}"
        );
    }
}
