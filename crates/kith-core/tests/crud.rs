//! Integration tests: create a small family through the public `Store` CRUD API
//! and read it back; prove cascade-delete behavior.

use kith_core::prelude::*;

/// Helper: a named individual with default sex/living.
fn person(store: &Store, given: &str, surname: &str, sex: Sex) -> Individual {
    store
        .create_individual(&NewIndividual {
            given_name: Some(given.to_owned()),
            surname: Some(surname.to_owned()),
            sex,
            ..Default::default()
        })
        .expect("create individual")
}

#[test]
fn builds_and_reads_back_two_people_a_family_a_child_and_a_birth_event() {
    // Arrange
    let store = Store::open_in_memory().expect("open store");

    // Act — two individuals, a family joining them, a child, a birth event.
    let jane = person(&store, "Jane", "Doe", Sex::Female);
    let john = person(&store, "John", "Doe", Sex::Male);
    let family = store
        .create_family(&NewFamily {
            partner1: Some(jane.id),
            partner2: Some(john.id),
            union_type: UnionType::Marriage,
            ..Default::default()
        })
        .expect("create family");
    let sam = person(&store, "Sam", "Doe", Sex::Unknown);
    store
        .add_child(family.id, sam.id, ChildRelation::Birth, 0)
        .expect("add child");
    let date: GenealogicalDate = "ABT 1850".parse().expect("parse date");
    let birth = store
        .add_event(&NewEvent {
            subject: EventSubject::Individual(sam.id),
            kind: EventKind::Birth,
            date: Some(date),
            place: None,
            notes: None,
        })
        .expect("add birth event");

    // Assert — read everything back.
    assert_eq!(
        store.get_individual(jane.id).unwrap().given_name.as_deref(),
        Some("Jane")
    );
    let read_family = store.get_family(family.id).unwrap();
    assert_eq!(read_family.partner1, Some(jane.id));
    assert_eq!(read_family.partner2, Some(john.id));
    assert_eq!(read_family.union_type, UnionType::Marriage);

    let children = store.list_children(family.id).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].child_id, sam.id);
    assert_eq!(children[0].relation, ChildRelation::Birth);

    let events = store
        .list_events_for(EventSubject::Individual(sam.id))
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Birth);
    // The date survives GenealogicalDate -> date_original -> GenealogicalDate.
    assert_eq!(events[0].date, Some(date));
    assert_eq!(store.get_event(birth.id).unwrap().date, Some(date));
}

#[test]
fn deleting_a_family_cascades_children_and_family_events_but_not_partners() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("kith.db")).unwrap();

    let jane = person(&store, "Jane", "Doe", Sex::Female);
    let john = person(&store, "John", "Doe", Sex::Male);
    let family = store
        .create_family(&NewFamily {
            partner1: Some(jane.id),
            partner2: Some(john.id),
            union_type: UnionType::Marriage,
            ..Default::default()
        })
        .unwrap();
    let sam = person(&store, "Sam", "Doe", Sex::Unknown);
    store
        .add_child(family.id, sam.id, ChildRelation::Birth, 0)
        .unwrap();
    store
        .add_event(&NewEvent {
            subject: EventSubject::Family(family.id),
            kind: EventKind::Marriage,
            date: Some("1849".parse().unwrap()),
            place: None,
            notes: None,
        })
        .unwrap();

    store.delete_family(family.id).unwrap();

    assert!(matches!(
        store.get_family(family.id),
        Err(CoreError::NotFound { .. })
    ));
    assert!(store.list_children(family.id).unwrap().is_empty());
    assert!(
        store
            .list_events_for(EventSubject::Family(family.id))
            .unwrap()
            .is_empty()
    );
    // Partners and child survive the family's deletion.
    assert!(store.get_individual(jane.id).is_ok());
    assert!(store.get_individual(john.id).is_ok());
    assert!(store.get_individual(sam.id).is_ok());
}

#[test]
fn deleting_a_partner_nulls_the_family_reference() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("kith.db")).unwrap();

    let jane = person(&store, "Jane", "Doe", Sex::Female);
    let john = person(&store, "John", "Doe", Sex::Male);
    let family = store
        .create_family(&NewFamily {
            partner1: Some(jane.id),
            partner2: Some(john.id),
            ..Default::default()
        })
        .unwrap();

    store.delete_individual(jane.id).unwrap(); // ON DELETE SET NULL

    let read = store.get_family(family.id).unwrap();
    assert_eq!(read.partner1, None);
    assert_eq!(read.partner2, Some(john.id));
}

#[test]
fn deleting_a_child_individual_removes_the_membership() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("kith.db")).unwrap();

    let family = store.create_family(&NewFamily::default()).unwrap();
    let sam = person(&store, "Sam", "Doe", Sex::Unknown);
    store
        .add_child(family.id, sam.id, ChildRelation::Birth, 0)
        .unwrap();

    store.delete_individual(sam.id).unwrap(); // ON DELETE CASCADE on family_children

    assert!(store.list_children(family.id).unwrap().is_empty());
    assert!(store.get_family(family.id).is_ok());
}

#[test]
fn getting_a_missing_individual_is_not_found_with_the_right_label() {
    let store = Store::open_in_memory().unwrap();
    let err = store.get_individual(PersonId::new(999)).unwrap_err();
    assert!(matches!(
        err,
        CoreError::NotFound {
            entity: "individual",
            id: 999
        }
    ));
}
