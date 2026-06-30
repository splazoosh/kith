//! Headless command tests.
//!
//! `tauri::State`/`AppHandle` are runtime-only, so these drive the `_impl` logic
//! fns directly over an in-memory `Store` — no webview. They prove the full CRUD
//! surface, the error contract (`not_found` / `validation` / `io`), and DB
//! restart persistence over a tempdir.

use kith_core::prelude::{
    ChartMode, ChildRelation, CitationSubject, Confidence, EventKind, EventSubject, Individual,
    NameKind, NewCitation, NewFamily, NewIndividual, NewName, NewSource, NodeEntity, PersonId, Sex,
    Store, Theme,
};
use kith_tauri_lib::commands::date::parse_date;
use kith_tauri_lib::commands::dto::NewEventRequest;
use kith_tauri_lib::commands::{
    db, event, export, family, gedcom, layout, lb, name, person, source, undo,
};
use kith_tauri_lib::error::ErrorKind;
use kith_tauri_lib::state::AppState;

/// An `AppState` over a fresh in-memory database.
fn memory_state() -> AppState {
    let state = AppState::default();
    *state.db.lock().expect("db mutex") = Some(Store::open_in_memory().expect("memory db"));
    state
}

/// A `NewIndividual` with just a given name.
fn given(name: &str) -> NewIndividual {
    NewIndividual {
        given_name: Some(name.to_owned()),
        ..Default::default()
    }
}

/// A `NewIndividual` with given + surname and an explicit `living` flag (for the
/// export-redaction cases, which turn on whether a person reads "Living").
fn named(given: &str, surname: &str, living: bool) -> NewIndividual {
    NewIndividual {
        given_name: Some(given.to_owned()),
        surname: Some(surname.to_owned()),
        living,
        ..Default::default()
    }
}

/// A tiny PNG for the media-import tests (the import only copies the bytes; the
/// mime is keyed off the `.png` extension, so any content works).
const PNG_BYTES: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[tokio::test]
async fn person_round_trips_through_the_command_layer() {
    // Arrange
    let state = memory_state();
    let draft = NewIndividual {
        given_name: Some("Jane".to_owned()),
        surname: Some("Doe".to_owned()),
        sex: Sex::Female,
        ..Default::default()
    };

    // Act — create → get → update → delete.
    let created = person::person_create_impl(&state, draft, None, None)
        .await
        .expect("create");
    let view = person::person_get_impl(&state, created.id)
        .await
        .expect("get");
    assert_eq!(view.individual.given_name.as_deref(), Some("Jane"));
    assert_eq!(view.individual.sex, Sex::Female);

    let edited = Individual {
        surname: Some("Smith".to_owned()),
        ..view.individual
    };
    let updated = person::person_update_impl(&state, edited)
        .await
        .expect("update");
    assert_eq!(updated.surname.as_deref(), Some("Smith"));

    person::person_delete_impl(&state, created.id)
        .await
        .expect("delete");

    // Assert — a get after delete is a typed not-found, not a crash.
    let err = person::person_get_impl(&state, created.id)
        .await
        .expect_err("missing after delete");
    assert_eq!(err.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn person_create_with_a_bad_date_is_validation() {
    let state = memory_state();
    let err =
        person::person_create_impl(&state, given("Ada"), Some("13 Bogus 1850".to_owned()), None)
            .await
            .expect_err("bad birth date");
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[tokio::test]
async fn person_get_of_a_missing_id_is_not_found() {
    let state = memory_state();
    let err = person::person_get_impl(&state, PersonId::new(404))
        .await
        .expect_err("no such person");
    assert_eq!(err.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn search_returns_ranked_hits_and_logic_lives_in_core() {
    // The search command is a thin marshal over `Store::search`: seed two people,
    // a name hit must outrank a place-only hit, and the limit caps the result.
    let state = memory_state();
    let by_name = person::person_create_impl(&state, named("Bergen", "Hansen", false), None, None)
        .await
        .expect("by name");
    let by_place = person::person_create_impl(&state, named("Ola", "Nordmann", false), None, None)
        .await
        .expect("by place");
    event::event_add_impl(
        &state,
        NewEventRequest {
            subject: EventSubject::Individual(by_place.id),
            kind: EventKind::Birth,
            date: None,
            place_id: None,
            place_name: Some("Bergen, Norway".to_owned()),
            notes: None,
        },
    )
    .await
    .expect("birth at Bergen");

    let hits = person::search_impl(&state, "Bergen".to_owned(), 50)
        .await
        .expect("search");
    let ids: Vec<_> = hits.iter().map(|h| h.individual.id).collect();
    assert_eq!(ids, vec![by_name.id, by_place.id], "a name hit ranks first");

    let capped = person::search_impl(&state, "Bergen".to_owned(), 1)
        .await
        .expect("search capped");
    assert_eq!(capped.len(), 1, "the limit caps the result set");
}

#[tokio::test]
async fn a_command_without_an_open_database_is_io() {
    let state = AppState::default(); // no db attached
    let err = person::person_list_impl(&state)
        .await
        .expect_err("no database open");
    assert_eq!(err.kind, ErrorKind::Io);
}

#[tokio::test]
async fn family_build_out_and_third_partner_is_validation() {
    // Arrange
    let state = memory_state();
    let p1 = person::person_create_impl(&state, given("A"), None, None)
        .await
        .expect("p1");
    let p2 = person::person_create_impl(&state, given("B"), None, None)
        .await
        .expect("p2");
    let p3 = person::person_create_impl(&state, given("C"), None, None)
        .await
        .expect("p3");

    // Act — create, fill both partner slots, add a child.
    let fam = family::family_create_impl(&state, NewFamily::default())
        .await
        .expect("create family");
    family::family_add_partner_impl(&state, fam.id, p1.id)
        .await
        .expect("partner1");
    let fam = family::family_add_partner_impl(&state, fam.id, p2.id)
        .await
        .expect("partner2");
    assert_eq!(fam.partner1, Some(p1.id));
    assert_eq!(fam.partner2, Some(p2.id));

    let link = family::family_add_child_impl(&state, fam.id, p3.id, ChildRelation::Birth, None)
        .await
        .expect("add child");
    assert_eq!(link.sort_order, 0, "order defaults to an append");

    // Assert — a third partner is a typed validation error.
    let err = family::family_add_partner_impl(&state, fam.id, p3.id)
        .await
        .expect_err("third partner");
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[tokio::test]
async fn event_add_and_name_add_remove_round_trip() {
    let state = memory_state();
    let person = person::person_create_impl(&state, given("Ada"), None, None)
        .await
        .expect("person");

    // event_add with a raw date string + a new place.
    let ev = event::event_add_impl(
        &state,
        NewEventRequest {
            subject: EventSubject::Individual(person.id),
            kind: EventKind::Birth,
            date: Some("ABT 1815".to_owned()),
            place_id: None,
            place_name: Some("London".to_owned()),
            notes: None,
        },
    )
    .await
    .expect("add event");
    assert_eq!(ev.kind, EventKind::Birth);
    assert_eq!(ev.date, Some("ABT 1815".parse().expect("date")));

    // The event surfaces in the person view (and place resolves via event_get).
    let view = person::person_get_impl(&state, person.id)
        .await
        .expect("view");
    assert_eq!(view.events.len(), 1);
    let resolved = event::event_get_impl(&state, ev.id)
        .await
        .expect("event view");
    assert_eq!(resolved.place.expect("place").name, "London");

    // name_add → list → remove.
    let alt = name::name_add_impl(
        &state,
        NewName {
            individual_id: person.id,
            kind: NameKind::Married,
            given_name: None,
            surname: Some("Byron".to_owned()),
            name_prefix: None,
            name_suffix: None,
            sort_order: 0,
        },
    )
    .await
    .expect("add name");
    assert_eq!(
        name::name_list_impl(&state, person.id)
            .await
            .expect("list")
            .len(),
        1
    );
    name::name_remove_impl(&state, alt.id)
        .await
        .expect("remove");
    assert!(
        name::name_list_impl(&state, person.id)
            .await
            .expect("list")
            .is_empty()
    );
}

#[tokio::test]
async fn parse_date_previews_and_rejects() {
    let preview = parse_date("ABT 1850".to_owned()).await.expect("parse");
    assert_eq!(preview.short, "c. 1850");
    assert_eq!(preview.long, "about 1850");

    let err = parse_date("not a date".to_owned())
        .await
        .expect_err("reject garbage");
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[tokio::test]
async fn open_missing_database_is_io() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("nope").join("missing.db");
    let state = AppState::default();
    let err = db::db_open_impl(&state, dir.path(), missing).expect_err("missing file");
    assert_eq!(err.kind, ErrorKind::Io, "a missing file is never created");
}

#[tokio::test]
async fn db_create_persists_and_reopens_across_a_fresh_state() {
    // Arrange — create a real database in a tempdir and add a person.
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path();
    let db_path = config_dir.join("family.db");

    let state = AppState::default();
    let info = db::db_create_impl(&state, config_dir, db_path.clone()).expect("create db");
    assert_eq!(info.schema_version, 2);
    assert_eq!(info.path, db_path);
    person::person_create_impl(&state, given("Ada"), None, None)
        .await
        .expect("seed person");

    // Act — a brand-new state runs the startup reopen against the same config dir.
    let reopened = AppState::default();
    db::reopen_last(&reopened, config_dir);

    // Assert — it reattached to the same file, and the data is there.
    let current = db::db_current_impl(&reopened)
        .expect("current")
        .expect("a database is open after reopen");
    assert_eq!(current.path, db_path);
    let people = person::person_list_impl(&reopened).await.expect("list");
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].given_name.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn db_close_clears_the_open_database() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path();
    let db_path = config_dir.join("kith.db");

    let state = AppState::default();
    db::db_create_impl(&state, config_dir, db_path).expect("create");
    assert!(db::db_current_impl(&state).expect("current").is_some());

    db::db_close_impl(&state, config_dir).expect("close");
    assert!(db::db_current_impl(&state).expect("current").is_none());

    // A fresh reopen now finds the persisted-empty config and opens nothing.
    let reopened = AppState::default();
    db::reopen_last(&reopened, config_dir);
    assert!(db::db_current_impl(&reopened).expect("current").is_none());
}

#[tokio::test]
async fn compute_layout_returns_a_model_rooted_at_the_focus() {
    // Arrange — a lone person is a valid one-node chart.
    let state = memory_state();
    let ada = person::person_create_impl(&state, given("Ada"), None, None)
        .await
        .expect("create");

    // Act
    let model = layout::compute_layout_impl(&state, ada.id, ChartMode::Descendants, 4)
        .await
        .expect("layout");

    // Assert — the mode round-trips and the focal person is present.
    assert_eq!(model.mode, ChartMode::Descendants);
    assert!(
        model
            .nodes
            .iter()
            .any(|n| { n.focal && matches!(n.entity, NodeEntity::Person(p) if p == ada.id) })
    );
}

#[tokio::test]
async fn compute_layout_of_a_missing_root_is_not_found() {
    let state = memory_state();
    let err = layout::compute_layout_impl(&state, PersonId::new(404), ChartMode::Ancestors, 2)
        .await
        .expect_err("no such root");
    assert_eq!(err.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn compute_layout_past_the_cap_is_validation() {
    let state = memory_state();
    let p = person::person_create_impl(&state, given("Cap"), None, None)
        .await
        .expect("p");
    let err = layout::compute_layout_impl(&state, p.id, ChartMode::Ancestors, 9_999)
        .await
        .expect_err("over the cap");
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[tokio::test]
async fn compute_layout_network_mode_positions_the_component() {
    // In Network mode, a lone person is its own single-node
    // connected component (the command forwards to the core, which lays it out).
    let state = memory_state();
    let p = person::person_create_impl(&state, given("Net"), None, None)
        .await
        .expect("p");
    let model = layout::compute_layout_impl(&state, p.id, ChartMode::Network, 2)
        .await
        .expect("network layout");
    assert_eq!(model.mode, ChartMode::Network);
    assert_eq!(model.nodes.len(), 1);
    assert!(model.nodes[0].focal);
}

#[tokio::test]
async fn export_html_writes_a_self_contained_file() {
    // Arrange — a dead person, so the name shows unredacted.
    let state = memory_state();
    let ada = person::person_create_impl(&state, named("Ada", "Lovelace", false), None, None)
        .await
        .expect("person");
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("tree.html");

    // Act
    export::export_html_impl(
        &state,
        ada.id,
        ChartMode::Descendants,
        4,
        Theme::Dark,
        false,
        false,
        out.display().to_string(),
    )
    .await
    .expect("export");

    // Assert — a complete, self-contained document carrying the (unredacted) name.
    let html = std::fs::read_to_string(&out).expect("written");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<svg"));
    assert!(html.contains("Lovelace")); // a dead person is not redacted
    for needle in ["http://", "https://", "<script src", "@import"] {
        assert!(!html.contains(needle), "self-contained: found {needle:?}");
    }
}

#[tokio::test]
async fn export_html_redacts_living_by_default_and_include_living_opts_out() {
    // Arrange — a living person (schema default), redacted unless opted out.
    let state = memory_state();
    let grace = person::person_create_impl(&state, named("Grace", "Hopper", true), None, None)
        .await
        .expect("person");
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("t.html");

    // Default — the name is replaced with "Living".
    export::export_html_impl(
        &state,
        grace.id,
        ChartMode::Ancestors,
        4,
        Theme::Light,
        false,
        false,
        out.display().to_string(),
    )
    .await
    .expect("default");
    let redacted = std::fs::read_to_string(&out).expect("written");
    assert!(redacted.contains("Living"));
    assert!(!redacted.contains("Hopper"));

    // include_living — the real name is written.
    export::export_html_impl(
        &state,
        grace.id,
        ChartMode::Ancestors,
        4,
        Theme::Light,
        true,
        false,
        out.display().to_string(),
    )
    .await
    .expect("include-living");
    assert!(
        std::fs::read_to_string(&out)
            .expect("written")
            .contains("Hopper")
    );
}

#[tokio::test]
async fn export_html_missing_root_is_not_found() {
    let state = memory_state();
    let dir = tempfile::tempdir().expect("temp dir");
    let err = export::export_html_impl(
        &state,
        PersonId::new(999),
        ChartMode::Descendants,
        4,
        Theme::Light,
        false,
        false,
        dir.path().join("x.html").display().to_string(),
    )
    .await
    .expect_err("missing root");
    assert_eq!(err.kind, ErrorKind::NotFound);
}

#[tokio::test]
async fn export_html_over_budget_generations_is_validation() {
    let state = memory_state();
    let p = person::person_create_impl(&state, named("Cap", "Stone", false), None, None)
        .await
        .expect("p");
    let dir = tempfile::tempdir().expect("temp dir");
    let err = export::export_html_impl(
        &state,
        p.id,
        ChartMode::Ancestors,
        9_999,
        Theme::Light,
        false,
        false,
        dir.path().join("x.html").display().to_string(),
    )
    .await
    .expect_err("over budget");
    assert_eq!(err.kind, ErrorKind::Validation);
}

#[tokio::test]
async fn export_html_write_to_a_missing_parent_dir_is_io() {
    // Pins the footgun: the write's io::Error MUST be carried as CoreError::Io so
    // the failure maps to ErrorKind::Io (not Unexpected).
    let state = memory_state();
    let p = person::person_create_impl(&state, named("Io", "Test", false), None, None)
        .await
        .expect("p");
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("no-such-subdir").join("x.html"); // parent does not exist
    let err = export::export_html_impl(
        &state,
        p.id,
        ChartMode::Descendants,
        4,
        Theme::Light,
        false,
        false,
        out.display().to_string(),
    )
    .await
    .expect_err("missing parent dir");
    assert_eq!(err.kind, ErrorKind::Io);
}

// — GEDCOM interop: the two thin commands over the frozen engine. —

#[tokio::test]
async fn export_gedcom_writes_a_valid_document() {
    // Arrange — a dead person, so the (never-redacted) name shows in the export.
    let state = memory_state();
    person::person_create_impl(&state, named("Ada", "Lovelace", false), None, None)
        .await
        .expect("person");
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("tree.ged");

    // Act
    gedcom::export_gedcom_impl(&state, out.display().to_string())
        .await
        .expect("export");

    // Assert — a complete, valid 5.5.1 document carrying the unredacted name.
    let ged = std::fs::read_to_string(&out).expect("written");
    assert!(ged.starts_with("0 HEAD"));
    assert!(ged.trim_end().ends_with("0 TRLR"));
    assert!(ged.contains(" INDI"));
    assert!(ged.contains("Lovelace")); // GEDCOM export is never redacted
}

#[tokio::test]
async fn import_gedcom_creates_a_fresh_tree_and_opens_it() {
    // Export a seeded DB to a file, then import it as a NEW tree (the GUI's
    // fresh-import model) into a state with NO database open.
    let src = memory_state();
    person::person_create_impl(&src, named("Ada", "Lovelace", false), None, None)
        .await
        .expect("seed");
    let dir = tempfile::tempdir().expect("temp dir");
    let ged = dir.path().join("out.ged");
    gedcom::export_gedcom_impl(&src, ged.display().to_string())
        .await
        .expect("export");

    let state = AppState::default(); // no DB open — import must still work
    let new_db = dir.path().join("imported.db");
    let result = gedcom::import_gedcom_impl(
        &state,
        dir.path(),
        ged.display().to_string(),
        new_db.clone(),
    )
    .await
    .expect("import");
    assert_eq!(result.summary.individuals, 1);
    assert_eq!(result.db.path, new_db);

    // The freshly imported database is now the open one, and holds the person.
    let current = db::db_current_impl(&state)
        .expect("current")
        .expect("a database is open after import");
    assert_eq!(current.path, new_db);
    let people = person::person_list_impl(&state).await.expect("list");
    assert_eq!(people.len(), 1);
}

#[tokio::test]
async fn import_gedcom_malformed_is_validation_and_opens_nothing() {
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let bad = dir.path().join("bad.ged");
    // A non-numeric level trips the lexer (an *unsupported tag* would be skipped-and-
    // counted, not an error) — the engine cites the offending line.
    std::fs::write(&bad, "0 HEAD\nNOTALEVEL garbage\n").expect("write fixture");

    let err = gedcom::import_gedcom_impl(
        &state,
        dir.path(),
        bad.display().to_string(),
        dir.path().join("bad.db"),
    )
    .await
    .expect_err("malformed");
    assert_eq!(err.kind, ErrorKind::Validation); // line-cited, from the engine
    // The fresh store is attached only on success: a malformed file opens nothing.
    assert!(db::db_current_impl(&state).expect("current").is_none());
}

#[tokio::test]
async fn import_gedcom_non_utf8_is_validation_and_creates_no_database() {
    // A non-UTF-8 file is an *encoding* problem the user fixes by re-exporting — a
    // Validation, not the Io that `read_to_string` would have produced. The
    // decode runs BEFORE the database is created, so no stray file is left behind.
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let f = dir.path().join("ansel.ged");
    std::fs::write(&f, [0xff, 0xfe, 0x00, 0x41]).expect("write fixture"); // not valid UTF-8
    let new_db = dir.path().join("x.db");
    let err =
        gedcom::import_gedcom_impl(&state, dir.path(), f.display().to_string(), new_db.clone())
            .await
            .expect_err("non-utf8");
    assert_eq!(err.kind, ErrorKind::Validation);
    assert!(!new_db.exists(), "no database created on a decode failure");
    assert!(db::db_current_impl(&state).expect("current").is_none());
}

#[tokio::test]
async fn import_gedcom_missing_file_is_io() {
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let err = gedcom::import_gedcom_impl(
        &state,
        dir.path(),
        dir.path().join("nope.ged").display().to_string(),
        dir.path().join("x.db"),
    )
    .await
    .expect_err("missing");
    assert_eq!(err.kind, ErrorKind::Io);
}

// — "LB" JSON import: the thin new-tree command over kith_core::lb. Mirrors the
//   GEDCOM import surface (the engine's mapping is the core crate's lb.rs suite). —

/// A minimal LB document: one person with a real birth date (`DD.MM.YYYY`).
const LB_DOC: &str =
    r#"[{"Id":1,"Gender":"M","FirstName":"Ada","LastName":"Lovelace","BirthDate":"10.12.1815"}]"#;

#[tokio::test]
async fn import_lb_creates_a_fresh_tree_and_opens_it() {
    // No database open — the new-tree import must still work and become the open one.
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let json = dir.path().join("people.json");
    std::fs::write(&json, LB_DOC).expect("write fixture");
    let new_db = dir.path().join("imported.db");

    let result = lb::import_lb_impl(
        &state,
        dir.path(),
        json.display().to_string(),
        new_db.clone(),
    )
    .await
    .expect("import");
    assert_eq!(result.summary.individuals, 1);
    assert_eq!(result.summary.events, 1); // the birth
    assert_eq!(result.db.path, new_db);

    // The freshly imported database is now the open one and holds the person.
    let current = db::db_current_impl(&state)
        .expect("current")
        .expect("a database is open after import");
    assert_eq!(current.path, new_db);
    assert_eq!(
        person::person_list_impl(&state).await.expect("list").len(),
        1
    );
}

#[tokio::test]
async fn import_lb_malformed_is_validation_and_opens_nothing() {
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, "{ not json").expect("write fixture");

    let err = lb::import_lb_impl(
        &state,
        dir.path(),
        bad.display().to_string(),
        dir.path().join("bad.db"),
    )
    .await
    .expect_err("malformed");
    assert_eq!(err.kind, ErrorKind::Validation); // from the engine
    // Attached only on success: a malformed file opens nothing.
    assert!(db::db_current_impl(&state).expect("current").is_none());
}

#[tokio::test]
async fn import_lb_non_utf8_is_validation_and_creates_no_database() {
    // A non-UTF-8 file is a Validation the user fixes by re-exporting, not the Io
    // `read_to_string` would give — decoded BEFORE the database is created.
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let f = dir.path().join("latin1.json");
    std::fs::write(&f, [0xff, 0xfe, 0x00, 0x41]).expect("write fixture"); // not valid UTF-8
    let new_db = dir.path().join("x.db");

    let err = lb::import_lb_impl(&state, dir.path(), f.display().to_string(), new_db.clone())
        .await
        .expect_err("non-utf8");
    assert_eq!(err.kind, ErrorKind::Validation);
    assert!(!new_db.exists(), "no database created on a decode failure");
    assert!(db::db_current_impl(&state).expect("current").is_none());
}

#[tokio::test]
async fn import_lb_missing_file_is_io() {
    let state = AppState::default();
    let dir = tempfile::tempdir().expect("temp dir");
    let err = lb::import_lb_impl(
        &state,
        dir.path(),
        dir.path().join("nope.json").display().to_string(),
        dir.path().join("x.db"),
    )
    .await
    .expect_err("missing");
    assert_eq!(err.kind, ErrorKind::Io);
}

#[tokio::test]
async fn media_import_list_set_primary_paths_and_delete_round_trip() {
    use kith_core::prelude::MediaSubject;
    use kith_tauri_lib::commands::media;

    // A file-backed DB so `last_path` (→ the media folder) is set.
    let dir = tempfile::tempdir().expect("temp dir");
    let state = AppState::default();
    db::db_create_impl(&state, dir.path(), dir.path().join("tree.db")).expect("create db");

    let p = person::person_create_impl(&state, named("Ada", "Lovelace", false), None, None)
        .await
        .expect("person");
    let subject = MediaSubject::Individual(p.id);
    let img = dir.path().join("face.png");
    std::fs::write(&img, PNG_BYTES).expect("write image");

    // Import → it is the subject's primary; the gallery lists it.
    let item = media::media_import_impl(&state, subject, img.display().to_string(), true)
        .await
        .expect("import");
    assert!(item.is_primary);
    assert_eq!(
        media::media_for_impl(&state, subject)
            .await
            .expect("list")
            .len(),
        1
    );

    // `media_paths` resolves the id to an absolute path under the media folder.
    let paths = media::media_paths_impl(&state, vec![item.media.id])
        .await
        .expect("paths");
    assert!(
        paths[&item.media.id].ends_with(&item.media.path),
        "abs path ends with the relative media path"
    );

    // set-primary is a no-op here (already primary); delete clears the gallery.
    media::media_set_primary_impl(&state, item.media.id, subject)
        .await
        .expect("set primary");
    media::media_delete_impl(&state, item.media.id)
        .await
        .expect("delete");
    assert!(
        media::media_for_impl(&state, subject)
            .await
            .expect("list")
            .is_empty()
    );
}

#[tokio::test]
async fn media_import_without_an_open_database_is_io() {
    use kith_core::prelude::MediaSubject;
    use kith_tauri_lib::commands::media;

    let state = AppState::default(); // no DB / no path → nowhere to put files
    let err = media::media_import_impl(
        &state,
        MediaSubject::Individual(PersonId::new(1)),
        "face.png".to_owned(),
        true,
    )
    .await
    .expect_err("no database");
    assert_eq!(err.kind, ErrorKind::Io);
}

#[tokio::test]
async fn source_and_citation_round_trip_through_the_command_layer() {
    // Arrange — a person + event to cite.
    let state = memory_state();
    let person = person::person_create_impl(&state, given("Ada"), None, None)
        .await
        .expect("person");
    let event = event::event_add_impl(
        &state,
        NewEventRequest {
            subject: EventSubject::Individual(person.id),
            kind: EventKind::Birth,
            date: None,
            place_id: None,
            place_name: None,
            notes: None,
        },
    )
    .await
    .expect("event");

    // Act — create a source, attach a citation to the event, read it back.
    let src = source::source_create_impl(
        &state,
        NewSource {
            title: "Parish Register".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("create source");
    assert_eq!(
        source::source_list_impl(&state).await.expect("list").len(),
        1
    );

    let item = source::citation_add_impl(
        &state,
        NewCitation {
            source: src.id,
            subject: CitationSubject::Event(event.id),
            page: Some("p. 7".to_owned()),
            detail: None,
            confidence: Some(Confidence::Primary),
        },
    )
    .await
    .expect("add citation");
    assert_eq!(item.source.id, src.id, "the source is resolved alongside");
    assert_eq!(item.citation.confidence, Some(Confidence::Primary));

    let for_event = source::citations_for_impl(&state, CitationSubject::Event(event.id))
        .await
        .expect("citations_for");
    assert_eq!(for_event.len(), 1);

    // The SourceView lists the supported fact.
    let view = source::source_get_impl(&state, src.id).await.expect("get");
    assert_eq!(view.citations.len(), 1);

    // Deleting the source cascades its citation; the GUI just surfaces the warning.
    source::source_delete_impl(&state, src.id)
        .await
        .expect("delete source");
    assert!(
        source::citations_for_impl(&state, CitationSubject::Event(event.id))
            .await
            .expect("after")
            .is_empty(),
        "deleting a source cascades its citations"
    );
}

#[tokio::test]
async fn source_get_missing_is_not_found() {
    use kith_core::prelude::SourceId;
    let state = memory_state();
    let err = source::source_get_impl(&state, SourceId::new(999))
        .await
        .expect_err("missing source");
    assert_eq!(err.kind, ErrorKind::NotFound);
}

// — Undo: the session stack + the undo command over the core primitive. —

#[tokio::test]
async fn delete_then_undo_restores_the_record_and_reports_the_outcome() {
    let state = memory_state();
    let p = person::person_create_impl(&state, named("Ada", "Lovelace", false), None, None)
        .await
        .expect("create");

    // Delete pushes one undo entry; the row is gone.
    person::person_delete_impl(&state, p.id)
        .await
        .expect("delete");
    assert_eq!(state.undo_depth(), 1, "the delete pushed one entry");
    let gone = person::person_get_impl(&state, p.id)
        .await
        .expect_err("deleted");
    assert_eq!(gone.kind, ErrorKind::NotFound);

    // Undo pops + restores and reports the outcome.
    let outcome = undo::undo_impl(&state)
        .await
        .expect("undo")
        .expect("an outcome (the stack was non-empty)");
    assert_eq!(outcome.kind, "person");
    assert_eq!(outcome.label, "Ada Lovelace");
    assert_eq!(outcome.remaining, 0);
    let restored = person::person_get_impl(&state, p.id)
        .await
        .expect("restored");
    assert_eq!(restored.individual.surname.as_deref(), Some("Lovelace"));
}

#[tokio::test]
async fn undo_on_an_empty_stack_is_none() {
    let state = memory_state();
    assert!(
        undo::undo_impl(&state).await.expect("undo").is_none(),
        "undo with nothing to restore is a no-op None"
    );
}

#[tokio::test]
async fn multi_level_undo_walks_the_stack_lifo() {
    let state = memory_state();
    let p = person::person_create_impl(&state, named("Ada", "Lovelace", false), None, None)
        .await
        .expect("person");
    let alt = name::name_add_impl(
        &state,
        NewName {
            individual_id: p.id,
            kind: NameKind::Aka,
            given_name: Some("A.".to_owned()),
            surname: None,
            name_prefix: None,
            name_suffix: None,
            sort_order: 0,
        },
    )
    .await
    .expect("add name");

    // Remove the name, then delete the person — two entries, newest last.
    name::name_remove_impl(&state, alt.id)
        .await
        .expect("remove name");
    person::person_delete_impl(&state, p.id)
        .await
        .expect("delete person");
    assert_eq!(state.undo_depth(), 2);

    // LIFO: the person (last deleted) restores first, then the name (FK now satisfied).
    let first = undo::undo_impl(&state).await.expect("undo").expect("some");
    assert_eq!(first.kind, "person");
    assert_eq!(first.remaining, 1);
    let second = undo::undo_impl(&state).await.expect("undo").expect("some");
    assert_eq!(second.kind, "name");
    assert_eq!(second.remaining, 0);
    assert_eq!(
        name::name_list_impl(&state, p.id)
            .await
            .expect("names")
            .len(),
        1,
        "both the person and their name are back"
    );
}

#[tokio::test]
async fn db_close_clears_the_undo_stack() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path();
    let state = AppState::default();
    db::db_create_impl(&state, config_dir, config_dir.join("kith.db")).expect("create");

    let p = person::person_create_impl(&state, given("Ada"), None, None)
        .await
        .expect("person");
    person::person_delete_impl(&state, p.id)
        .await
        .expect("delete");
    assert_eq!(state.undo_depth(), 1);

    db::db_close_impl(&state, config_dir).expect("close");
    assert_eq!(
        state.undo_depth(),
        0,
        "closing the database clears the stack"
    );
}

#[tokio::test]
async fn undo_after_id_reuse_is_a_typed_error_and_drops_the_dead_entry() {
    // Delete the highest-id person, create a new one (SQLite reuses the freed id),
    // then undo → the explicit-id restore conflicts. The error is typed `database`
    // and the dead entry is dropped, never corrupting anything.
    let state = memory_state();
    person::person_create_impl(&state, given("A"), None, None)
        .await
        .expect("a");
    let b = person::person_create_impl(&state, given("B"), None, None)
        .await
        .expect("b");
    person::person_delete_impl(&state, b.id)
        .await
        .expect("delete b");
    let c = person::person_create_impl(&state, given("C"), None, None)
        .await
        .expect("c");
    assert_eq!(c.id, b.id, "the freed max id is reused");

    let err = undo::undo_impl(&state)
        .await
        .expect_err("the reused id conflicts");
    assert_eq!(err.kind, ErrorKind::Database);
    assert_eq!(
        state.undo_depth(),
        0,
        "the dead entry is dropped, not re-pushed"
    );
    // The reusing person is untouched — the conflicting restore rolled back.
    assert!(person::person_get_impl(&state, c.id).await.is_ok());
}
