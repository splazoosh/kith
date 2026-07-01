//! The GEDCOM round-trip proof: stable round trip, atomic + line-cited failure,
//! and honest supported-field scope. A small `.ged` corpus, a writer snapshot,
//! and a malformed-file suite.
//!
//! The primary round-trip invariant is **byte-stability**: `export → import →
//! export` is byte-identical, because id assignment is order-stable on a fresh
//! import (the writer emits individuals in ascending-id order). That is the
//! strongest, cheapest form of "structural equivalence over the supported fields".

use kith_core::gedcom::{ImportOptions, ImportSummary, export, import};
use kith_core::prelude::*;

const NUCLEAR: &str = include_str!("fixtures/nuclear.ged");
const REMARRIAGE: &str = include_str!("fixtures/remarriage.ged");
const FUZZY_DATES: &str = include_str!("fixtures/fuzzy_dates.ged");
const ADOPTED_CHILD: &str = include_str!("fixtures/adopted_child.ged");
const ALT_NAMES: &str = include_str!("fixtures/alt_names.ged");
const MULTILINE_NOTE: &str = include_str!("fixtures/multiline_note.ged");
const UNSUPPORTED_TAGS: &str = include_str!("fixtures/unsupported_tags.ged");
const MEDIA: &str = include_str!("fixtures/media.ged");
const SOURCES: &str = include_str!("fixtures/sources.ged");

const CORPUS: &[(&str, &str)] = &[
    ("nuclear", NUCLEAR),
    ("remarriage", REMARRIAGE),
    ("fuzzy_dates", FUZZY_DATES),
    ("adopted_child", ADOPTED_CHILD),
    ("alt_names", ALT_NAMES),
    ("multiline_note", MULTILINE_NOTE),
    ("unsupported_tags", UNSUPPORTED_TAGS),
    ("media", MEDIA),
    ("sources", SOURCES),
];

fn fresh() -> Store {
    Store::open_in_memory().expect("open in-memory store")
}

/// `ImportOptions` is `#[non_exhaustive]`, so build it default-then-mutate.
fn merge_opts(merge: bool) -> ImportOptions {
    let mut o = ImportOptions::default();
    o.merge = merge;
    o
}

/// Export the store, normalizing the HEAD's crate-version line so a version
/// bump doesn't churn the snapshots. The writer emits the crate version as
/// `2 VERS {CARGO_PKG_VERSION}` under `1 SOUR Kith`; only that line is rewritten
/// to `[VERSION]`. The GEDCOM spec version (`2 VERS 5.5.1` under `1 GEDC`) is
/// left intact, so the snapshot still pins it. Byte-stability across a round
/// trip is proven separately and is version-agnostic already.
fn export_snapshot(s: &Store) -> String {
    export(s).expect("export").replace(
        &format!("1 SOUR Kith\n2 VERS {}\n", env!("CARGO_PKG_VERSION")),
        "1 SOUR Kith\n2 VERS [VERSION]\n",
    )
}

#[test]
fn import_export_import_is_byte_stable() {
    // Arrange — import the corpus into a fresh store and export it once.
    let s1 = fresh();
    let summary = import(&s1, NUCLEAR, &merge_opts(false)).expect("import");
    let g1 = export(&s1).expect("export");

    // Act — round-trip the exported document through a second fresh store.
    let s2 = fresh();
    import(&s2, &g1, &merge_opts(false)).expect("re-import");
    let g2 = export(&s2).expect("re-export");

    // Assert — id assignment is order-stable, so the re-export reproduces g1.
    assert_eq!(g1, g2, "export → import → export must be byte-identical");
    assert_eq!(summary.individuals, 4);
    assert_eq!(summary.families, 1);
    assert_eq!(
        summary.places, 2,
        "Bergen is deduped across a birth and a marriage"
    );
}

#[test]
fn every_corpus_fixture_round_trips_byte_stably() {
    for (name, source) in CORPUS {
        let s1 = fresh();
        import(&s1, source, &merge_opts(false)).unwrap_or_else(|e| panic!("import {name}: {e}"));
        let g1 = export(&s1).expect("export");

        let s2 = fresh();
        import(&s2, &g1, &merge_opts(false)).unwrap_or_else(|e| panic!("re-import {name}: {e}"));
        let g2 = export(&s2).expect("re-export");

        assert_eq!(
            g1, g2,
            "fixture {name} is not byte-stable across a round trip"
        );
    }
}

#[test]
fn export_of_nuclear_matches_snapshot() {
    let s = fresh();
    import(&s, NUCLEAR, &merge_opts(false)).expect("import");
    insta::assert_snapshot!("nuclear_export", export_snapshot(&s));
}

#[test]
fn unsupported_tags_are_skipped_and_counted() {
    let s = fresh();
    let summary = import(&s, UNSUPPORTED_TAGS, &merge_opts(false)).expect("import");

    // The supported INDI is imported.
    assert_eq!(summary.individuals, 1);
    assert_eq!(
        s.list_individuals().expect("list").len(),
        1,
        "the supported record still imported"
    );
    // OBJE and SOUR/REPO are mapped, not skipped: the @S1@
    // SOUR is a source (its @R1@ REPO resolves to its repository), the @O1@ OBJE a
    // media row — none are counted as skipped any more.
    assert_eq!(summary.skipped_tags.get("SOUR"), None);
    assert_eq!(summary.skipped_tags.get("REPO"), None);
    assert_eq!(summary.skipped_tags.get("OBJE"), None);
    assert_eq!(summary.sources, 1, "the @S1@ SOUR is mapped, not skipped");
    assert_eq!(summary.media, 1, "the @O1@ OBJE is mapped, not skipped");
    assert_eq!(
        s.list_sources().expect("sources")[0].repository.as_deref(),
        Some("Some Archive"),
        "REPO resolves into the source's repository"
    );
    // A genuinely-unsupported record (SUBM) is still counted, never dropped.
    assert_eq!(summary.skipped_tags.get("SUBM"), Some(&1));
}

#[test]
fn sources_and_citations_map_to_their_subjects_and_round_trip() {
    let s = fresh();
    let summary = import(&s, SOURCES, &merge_opts(false)).expect("import");

    // Two top-level SOUR → two sources; three SOUR pointers → three citations
    // (one on a birth event, one on the individual, one on the family).
    assert_eq!(summary.sources, 2);
    assert_eq!(summary.citations, 3);

    // The event citation carries page / QUAY→confidence / DATA.TEXT→detail.
    let ada = s.list_individuals().expect("list")[0].id;
    let birth = s
        .list_events_for(EventSubject::Individual(ada))
        .expect("events")[0]
        .id;
    let event_cites = s
        .citations_for(CitationSubject::Event(birth))
        .expect("event citations");
    assert_eq!(event_cites.len(), 1);
    assert_eq!(event_cites[0].citation.page.as_deref(), Some("p. 42"));
    assert_eq!(
        event_cites[0].citation.confidence,
        Some(Confidence::Primary),
        "QUAY 3 → Primary"
    );
    assert_eq!(
        event_cites[0].citation.detail.as_deref(),
        Some("Born in London")
    );
    assert_eq!(
        event_cites[0].source.repository.as_deref(),
        Some("London Metropolitan Archives"),
        "the source's REPO pointer resolves to its repository"
    );

    // Byte-stable round trip (a focused signal beyond the corpus sweep).
    let g1 = export(&s).expect("export");
    let s2 = fresh();
    import(&s2, &g1, &merge_opts(false)).expect("re-import");
    assert_eq!(
        g1,
        export(&s2).expect("re-export"),
        "sources/citations round trip drifts"
    );
}

#[test]
fn export_of_sources_matches_snapshot() {
    let s = fresh();
    import(&s, SOURCES, &merge_opts(false)).expect("import");
    insta::assert_snapshot!("sources_export", export_snapshot(&s));
}

#[test]
fn a_dangling_source_pointer_is_a_line_cited_validation() {
    // A `1 SOUR @S9@` with no matching top-level SOUR is a Validation, not a panic
    // or a half-write (import stays atomic).
    const DANGLING_SOUR: &str = "0 HEAD\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME A /B/\n1 SOUR @S9@\n";
    let s = fresh();
    let err = import(&s, DANGLING_SOUR, &merge_opts(false)).expect_err("dangling SOUR must error");
    assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
    assert!(err.to_string().contains("line"), "message cites a line");
    assert!(
        s.list_individuals().expect("list").is_empty(),
        "a dangling SOUR writes nothing"
    );
}

#[test]
fn media_objects_map_link_to_their_subject_and_round_trip() {
    let s = fresh();
    let summary = import(&s, MEDIA, &merge_opts(false)).expect("import");

    // Two top-level OBJE → two media rows; one links the person, one the family.
    assert_eq!(summary.media, 2);
    let person = s.list_individuals().expect("list")[0].id;
    let items = s
        .list_media_for(MediaSubject::Individual(person))
        .expect("media");
    assert_eq!(items.len(), 1);
    assert!(
        items[0].is_primary,
        "the first OBJE link is the subject's primary"
    );
    assert_eq!(items[0].media.path, "portrait.jpg"); // FILE recorded verbatim, no copy
    assert_eq!(items[0].media.mime.as_deref(), Some("jpeg"));
    assert_eq!(items[0].media.caption.as_deref(), Some("Ada portrait"));

    // Byte-stable round trip (a focused signal beyond the corpus sweep).
    let g1 = export(&s).expect("export");
    let s2 = fresh();
    import(&s2, &g1, &merge_opts(false)).expect("re-import");
    assert_eq!(
        g1,
        export(&s2).expect("re-export"),
        "media round trip drifts"
    );
}

#[test]
fn malformed_files_are_line_cited_validation_not_panic() {
    // Each is a distinct structural failure; none may panic or half-write.
    const TRUNCATED: &str = "0 HEAD\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME J /D/\n1 FAMS @F9@\n";
    const BAD_LEVEL: &str = "0 HEAD\nX CHAR UTF-8\n";
    const LEVEL_JUMP: &str = "0 HEAD\n2 CHAR UTF-8\n";
    const DANGLING_XREF: &str = "0 HEAD\n1 CHAR UTF-8\n0 @F1@ FAM\n1 CHIL @I9@\n";
    const ANSEL_HEADER: &str = "0 HEAD\n1 CHAR ANSEL\n0 @I1@ INDI\n1 NAME A /B/\n";

    for (src, needle) in [
        (TRUNCATED, "line"),
        (BAD_LEVEL, "line"),
        (LEVEL_JUMP, "line"),
        (DANGLING_XREF, "line"),
        (ANSEL_HEADER, "ANSEL"),
    ] {
        let s = fresh();
        let err = import(&s, src, &merge_opts(false)).expect_err("malformed must error");
        assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
        assert!(
            err.to_string().contains(needle),
            "message {err:?} should mention {needle:?}"
        );
        assert!(
            s.list_individuals().expect("list").is_empty(),
            "a malformed import must write nothing"
        );
    }
}

#[test]
fn import_summary_default_is_empty() {
    // Guards the `#[non_exhaustive]` + `Default` contract the shells rely on.
    let summary = ImportSummary::default();
    assert_eq!(summary.individuals, 0);
    assert!(summary.skipped_tags.is_empty());
}
