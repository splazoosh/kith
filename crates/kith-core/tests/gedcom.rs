//! Targeted public-API tests for `gedcom::{import, export}` — the mapping details
//! the byte-stable round-trip suite (`gedcom_round_trip.rs`) does not assert on
//! their own: content mapping, the `living = false` default, un-redacted export,
//! the merge precondition, the slash-name fallback, and the PEDI relation.

use kith_core::gedcom::{ImportOptions, export, import};
use kith_core::prelude::*;

fn fresh() -> Store {
    Store::open_in_memory().expect("open in-memory store")
}

fn merge_opts(merge: bool) -> ImportOptions {
    let mut o = ImportOptions::default();
    o.merge = merge;
    o
}

/// Import a single GEDCOM string into a fresh store, panicking with context.
fn import_into_fresh(source: &str) -> Store {
    let s = fresh();
    import(&s, source, &merge_opts(false)).expect("import");
    s
}

#[test]
fn maps_names_sex_events_and_dates() {
    let s = import_into_fresh(
        "0 HEAD\n1 CHAR UTF-8\n\
         0 @I1@ INDI\n1 NAME Ada /Lovelace/\n2 GIVN Ada\n2 SURN Lovelace\n1 SEX F\n\
         1 BIRT\n2 DATE 10 DEC 1815\n2 PLAC London, England\n0 TRLR\n",
    );

    let people = s.list_individuals().expect("list");
    assert_eq!(people.len(), 1);
    let ada = &people[0];
    assert_eq!(ada.given_name.as_deref(), Some("Ada"));
    assert_eq!(ada.surname.as_deref(), Some("Lovelace"));
    assert_eq!(ada.sex, Sex::Female);

    let events = s
        .list_events_for(EventSubject::Individual(ada.id))
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Birth);
    assert_eq!(
        events[0].date,
        Some("10 Dec 1815".parse().expect("parse date"))
    );
    let place = s
        .get_place(events[0].place.expect("place id"))
        .expect("place");
    assert_eq!(place.name, "London, England");
}

#[test]
fn imported_individuals_default_to_not_living() {
    // Deterministic: an imported person is `living = false`.
    let s = import_into_fresh("0 @I1@ INDI\n1 NAME A /B/\n1 SEX U\n0 TRLR\n");
    assert!(!s.list_individuals().expect("list")[0].living);
}

#[test]
fn export_is_not_redacted_even_for_living_persons() {
    // GEDCOM is a full-fidelity data move, distinct from the HTML exporter's
    // privacy default. A living person's name must appear in the output.
    let s = fresh();
    s.create_individual(&NewIndividual {
        given_name: Some("Liv".to_owned()),
        surname: Some("Ng".to_owned()),
        sex: Sex::Female,
        living: true,
        ..Default::default()
    })
    .expect("create living person");

    let doc = export(&s).expect("export");
    assert!(
        doc.contains("1 NAME Liv /Ng/"),
        "living person must be exported"
    );
    assert!(doc.starts_with("0 HEAD\n"));
    assert!(doc.ends_with("0 TRLR\n"));
}

#[test]
fn non_merge_into_a_non_empty_store_is_refused_and_merge_appends() {
    let s = import_into_fresh("0 @I1@ INDI\n1 NAME First /Person/\n0 TRLR\n");

    // A non-merge import into a populated store is a clear Validation.
    let refused = import(
        &s,
        "0 @I1@ INDI\n1 NAME Second /Person/\n0 TRLR\n",
        &merge_opts(false),
    );
    assert!(
        matches!(refused, Err(CoreError::Validation(_))),
        "got {refused:?}"
    );
    assert_eq!(
        s.list_individuals().expect("list").len(),
        1,
        "nothing appended"
    );

    // merge = true appends with fresh ids (no dedup).
    import(
        &s,
        "0 @I1@ INDI\n1 NAME Second /Person/\n0 TRLR\n",
        &merge_opts(true),
    )
    .expect("merge import");
    assert_eq!(s.list_individuals().expect("list").len(), 2);
}

#[test]
fn primary_and_alternate_names_are_split_by_type() {
    let s = import_into_fresh(
        "0 @I1@ INDI\n1 NAME Jane /Smith/\n2 GIVN Jane\n2 SURN Smith\n\
         1 NAME Jane /Doe/\n2 TYPE married\n2 GIVN Jane\n2 SURN Doe\n1 SEX F\n0 TRLR\n",
    );
    let person = &s.list_individuals().expect("list")[0];
    // The no-TYPE NAME is the inline primary.
    assert_eq!(person.surname.as_deref(), Some("Smith"));

    let names = s.list_names(person.id).expect("names");
    assert_eq!(names.len(), 1, "the TYPEd NAME becomes an alternate");
    assert_eq!(names[0].kind, NameKind::Married);
    assert_eq!(names[0].surname.as_deref(), Some("Doe"));
}

#[test]
fn name_without_subtags_falls_back_to_the_slash_form() {
    let s = import_into_fresh("0 @I1@ INDI\n1 NAME Bjorn /Holm/\n1 SEX M\n0 TRLR\n");
    let person = &s.list_individuals().expect("list")[0];
    assert_eq!(person.given_name.as_deref(), Some("Bjorn"));
    assert_eq!(person.surname.as_deref(), Some("Holm"));
}

#[test]
fn pedi_maps_to_the_child_relation() {
    let s = import_into_fresh(
        "0 @I1@ INDI\n1 NAME P /H/\n1 FAMS @F1@\n\
         0 @I2@ INDI\n1 NAME Adoptee /H/\n1 FAMC @F1@\n2 PEDI adopted\n\
         0 @F1@ FAM\n1 HUSB @I1@\n1 CHIL @I2@\n0 TRLR\n",
    );
    let families = s.list_families().expect("families");
    assert_eq!(families.len(), 1);
    let children = s.list_children(families[0].id).expect("children");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].relation, ChildRelation::Adopted);
}

#[test]
fn marriage_event_implies_the_marriage_union_type() {
    let s = import_into_fresh(
        "0 @I1@ INDI\n1 NAME A /X/\n1 FAMS @F1@\n\
         0 @I2@ INDI\n1 NAME B /Y/\n1 FAMS @F1@\n\
         0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 MARR\n2 DATE 1900\n0 TRLR\n",
    );
    let family = &s.list_families().expect("families")[0];
    assert_eq!(family.union_type, UnionType::Marriage);
    let events = s
        .list_events_for(EventSubject::Family(family.id))
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Marriage);
}
