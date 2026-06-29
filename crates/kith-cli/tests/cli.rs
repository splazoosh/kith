//! Black-box tests for the spine: init → add → list(--json), plus the
//! missing-DB and bad-value exit paths.

use std::path::Path;

use assert_cmd::Command;
use kith_core::prelude::{
    ChartMode, CitationItem, Confidence, Event, EventKind, Family, Individual, Name, PersonId,
    RelativeGraph, SearchHit, Source,
};
use kith_core::query::SourceView;
use predicates::prelude::*;

/// A `kith` invocation already pointed at the isolated test database.
fn kith(db: &Path) -> Command {
    let mut cmd = Command::cargo_bin("kith").expect("kith binary builds");
    cmd.arg("--db").arg(db);
    cmd
}

#[test]
fn init_then_add_then_list_json_round_trips() {
    // Arrange
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");

    // Act 1 — init creates and migrates.
    kith(&db)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema v2"));

    // Act 2 — add one person.
    kith(&db)
        .args([
            "person",
            "add",
            "--given",
            "Ada",
            "--surname",
            "Lovelace",
            "--sex",
            "F",
        ])
        .assert()
        .success();

    // Act 3 — list as JSON.
    let output = kith(&db)
        .args(["person", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Assert — the JSON deserializes back into the core type.
    let people: Vec<Individual> =
        serde_json::from_slice(&output).expect("person list --json parses as Vec<Individual>");
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].surname.as_deref(), Some("Lovelace"));
    assert_eq!(people[0].given_name.as_deref(), Some("Ada"));
}

#[test]
fn non_init_command_against_missing_db_exits_io() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("absent.db");
    kith(&missing)
        .args(["person", "list"])
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("run `kith init`"));
}

#[test]
fn bad_sex_code_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    // A malformed value is a clap usage error → exit 2.
    kith(&db)
        .args(["person", "add", "--given", "X", "--sex", "Q"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn db_backup_then_restore_round_trips_a_person() {
    // Arrange — a DB with one person.
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success();

    // Act — back up, then restore onto a *fresh* database path.
    let backup = dir.path().join("snapshot.db");
    kith(&db)
        .args(["db", "backup"])
        .arg(&backup)
        .assert()
        .success();

    let restored_db = dir.path().join("restored.db");
    kith(&restored_db)
        .args(["db", "restore"])
        .arg(&backup)
        .assert()
        .success();

    // Assert — the restored database lists the person.
    kith(&restored_db)
        .args(["person", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Lovelace"));
}

#[test]
fn db_backup_refuses_existing_destination_without_force() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();

    let backup = dir.path().join("snapshot.db");
    kith(&db)
        .args(["db", "backup"])
        .arg(&backup)
        .assert()
        .success();

    // Second backup to the same path → validation error (exit 4)…
    kith(&db)
        .args(["db", "backup"])
        .arg(&backup)
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("--force"));

    // …unless --force is given.
    kith(&db)
        .args(["db", "backup"])
        .arg(&backup)
        .arg("--force")
        .assert()
        .success();
}

#[test]
fn db_restore_rejects_a_non_database_source() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    let garbage = dir.path().join("garbage.bin");
    std::fs::write(&garbage, b"not a database").expect("write garbage");

    kith(&db)
        .args(["db", "restore"])
        .arg(&garbage)
        .assert()
        .failure()
        .code(4);
    assert!(
        !db.exists(),
        "a rejected restore must not create the target"
    );
}

#[test]
fn db_vacuum_succeeds() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db).args(["db", "vacuum"]).assert().success();
}

#[test]
fn person_show_json_exposes_the_individual() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success();

    let out = kith(&db)
        .args(["person", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let v: serde_json::Value = serde_json::from_slice(&out).expect("show --json is valid JSON");
    // The embedded record round-trips into the core type.
    let individual: Individual =
        serde_json::from_value(v["individual"].clone()).expect("individual deserializes");
    assert_eq!(individual.surname.as_deref(), Some("Lovelace"));
    assert!(
        v["events"].as_array().is_some(),
        "events array present (currently empty)"
    );
}

#[test]
fn person_edit_changes_only_named_fields() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args([
            "person",
            "add",
            "--given",
            "Ada",
            "--surname",
            "Lovelace",
            "--sex",
            "F",
        ])
        .assert()
        .success();

    kith(&db)
        .args(["person", "edit", "1", "--surname", "King"])
        .assert()
        .success();

    let out = kith(&db)
        .args(["person", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["individual"]["surname"], "King", "surname changed");
    assert_eq!(v["individual"]["given_name"], "Ada", "given name untouched");
    // `Sex` serializes via serde's variant name (`"Female"`), not the TEXT code
    // `"F"` used by its `Display`/`FromStr`; the round-trip is symmetric.
    assert_eq!(v["individual"]["sex"], "Female", "sex untouched");
}

#[test]
fn person_rm_of_missing_id_exits_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    // First time exit 3 is exercised end-to-end in the CLI suite.
    kith(&db)
        .args(["person", "rm", "999"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn name_add_list_rm_round_trips() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Jane"])
        .assert()
        .success();

    kith(&db)
        .args(["name", "add", "1", "--kind", "married", "--surname", "Doe"])
        .assert()
        .success();

    let out = kith(&db)
        .args(["name", "list", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let names: Vec<Name> = serde_json::from_slice(&out).expect("name list --json is Vec<Name>");
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].surname.as_deref(), Some("Doe"));

    kith(&db).args(["name", "rm", "1"]).assert().success();
    let out = kith(&db)
        .args(["name", "list", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let names: Vec<Name> = serde_json::from_slice(&out).unwrap();
    assert!(names.is_empty(), "name removed");
}

#[test]
fn name_add_bad_kind_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Jane"])
        .assert()
        .success();
    kith(&db)
        .args(["name", "add", "1", "--kind", "bogus"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn family_new_add_child_then_show_lists_partners_and_child() {
    // The family/child portion of the exit-criteria scripted session.
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success(); // id 1
    kith(&db)
        .args(["person", "add", "--given", "Charles"])
        .assert()
        .success(); // id 2
    kith(&db)
        .args(["person", "add", "--given", "Byron"])
        .assert()
        .success(); // id 3 (child)

    kith(&db)
        .args([
            "family",
            "new",
            "--partner",
            "1",
            "--partner",
            "2",
            "--type",
            "marriage",
        ])
        .assert()
        .success();
    kith(&db)
        .args(["family", "add-child", "1", "3"])
        .assert()
        .success();

    let out = kith(&db)
        .args(["family", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let family: Family = serde_json::from_value(v["family"].clone()).expect("family deserializes");
    assert_eq!(family.union_type, kith_core::prelude::UnionType::Marriage);
    assert_eq!(v["partner1"]["given_name"], "Ada");
    assert_eq!(v["partner2"]["given_name"], "Charles");
    let kids = v["children"].as_array().expect("children array");
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0]["individual"]["given_name"], "Byron");
}

#[test]
fn family_new_rejects_more_than_two_partners() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    for _ in 0..3 {
        kith(&db)
            .args(["person", "add", "--given", "P"])
            .assert()
            .success();
    }
    kith(&db)
        .args([
            "family",
            "new",
            "--partner",
            "1",
            "--partner",
            "2",
            "--partner",
            "3",
        ])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn family_add_partner_to_a_full_family_exits_validation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    for _ in 0..3 {
        kith(&db)
            .args(["person", "add", "--given", "P"])
            .assert()
            .success();
    }
    kith(&db)
        .args(["family", "new", "--partner", "1", "--partner", "2"])
        .assert()
        .success();
    kith(&db)
        .args(["family", "add-partner", "1", "3"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn scripted_session_with_events_round_trips_as_json() {
    // init → people → family → child → events (individual + family) → show/list.
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");

    kith(&db).arg("init").assert().success();
    // Ada gets a birth event via the --birth convenience → event id 1.
    kith(&db)
        .args(["person", "add", "--given", "Ada", "--birth", "ABT 1815"])
        .assert()
        .success(); // person 1
    kith(&db)
        .args(["person", "add", "--given", "Charles"])
        .assert()
        .success(); // person 2
    kith(&db)
        .args(["person", "add", "--given", "Byron"])
        .assert()
        .success(); // person 3

    kith(&db)
        .args([
            "family",
            "new",
            "--partner",
            "1",
            "--partner",
            "2",
            "--type",
            "marriage",
        ])
        .assert()
        .success(); // family 1
    kith(&db)
        .args(["family", "add-child", "1", "3"])
        .assert()
        .success();

    // An individual event and a family event (with a place) → event ids 2 and 3.
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "person:3",
            "--kind",
            "birth",
            "--date",
            "1816",
        ])
        .assert()
        .success();
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "family:1",
            "--kind",
            "marriage",
            "--date",
            "ABT 1835",
            "--place",
            "London",
        ])
        .assert()
        .success();

    // person show exposes Ada's birth event (created by --birth), round-tripped.
    let out = kith(&db)
        .args(["person", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let events = v["events"].as_array().expect("events array");
    assert_eq!(events.len(), 1, "Ada has her birth event");
    let ada_birth: Event = serde_json::from_value(events[0].clone()).expect("event round-trips");
    assert_eq!(ada_birth.kind, EventKind::Birth);
    assert_eq!(ada_birth.date, Some("ABT 1815".parse().unwrap()));

    // family show exposes the marriage event in its events array.
    let out = kith(&db)
        .args(["family", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let fam_events = v["events"].as_array().expect("family events array");
    assert_eq!(fam_events.len(), 1);
    let marriage: Event = serde_json::from_value(fam_events[0].clone()).expect("event round-trips");
    assert_eq!(marriage.kind, EventKind::Marriage);

    // event show round-trips the Event and resolves its place.
    let out = kith(&db)
        .args(["event", "show", "3", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let ev: Event = serde_json::from_value(v["event"].clone()).expect("event round-trips");
    assert_eq!(ev.kind, EventKind::Marriage);
    assert_eq!(v["place"]["name"], "London");

    // person list is still a clean Vec<Individual>.
    let out = kith(&db)
        .args(["person", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let people: Vec<Individual> = serde_json::from_slice(&out).expect("Vec<Individual>");
    assert_eq!(people.len(), 3);
}

#[test]
fn event_edit_changes_only_the_date() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "person:1",
            "--kind",
            "birth",
            "--date",
            "1815",
        ])
        .assert()
        .success(); // event 1

    kith(&db)
        .args(["event", "edit", "1", "--date", "ABT 1816"])
        .assert()
        .success();

    let out = kith(&db)
        .args(["event", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let ev: Event = serde_json::from_value(v["event"].clone()).unwrap();
    assert_eq!(ev.kind, EventKind::Birth, "kind untouched");
    assert_eq!(ev.date, Some("ABT 1816".parse().unwrap()), "date changed");
}

#[test]
fn event_rm_then_show_exits_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    kith(&db)
        .args(["event", "add", "--subject", "person:1", "--kind", "birth"])
        .assert()
        .success();
    kith(&db).args(["event", "rm", "1"]).assert().success();
    kith(&db)
        .args(["event", "show", "1"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn event_add_bad_subject_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["event", "add", "--subject", "bogus:1", "--kind", "birth"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn event_add_bad_date_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    // "Bogus" is an unknown month → GenealogicalDate::from_str → Validation,
    // surfaced by the value-parser as a clap usage error.
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "person:1",
            "--kind",
            "birth",
            "--date",
            "13 Bogus 1850",
        ])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn event_add_conflicting_place_flags_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "person:1",
            "--kind",
            "residence",
            "--place",
            "Oslo",
            "--place-id",
            "1",
        ])
        .assert()
        .failure()
        .code(2); // clap conflicts_with
}

#[test]
fn event_add_to_nonexistent_subject_is_a_database_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    // Well-formed subject, nonexistent individual → FK violation at add_event.
    kith(&db)
        .args(["event", "add", "--subject", "person:999", "--kind", "birth"])
        .assert()
        .failure()
        .code(6);
}

#[test]
fn event_add_unknown_kind_is_accepted_verbatim() {
    // EventKind is open: an unknown code is preserved, not rejected.
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "person:1",
            "--kind",
            "emigration",
        ])
        .assert()
        .success();
    let out = kith(&db)
        .args(["event", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let ev: Event = serde_json::from_value(v["event"].clone()).unwrap();
    assert_eq!(ev.kind, EventKind::Other("emigration".to_owned()));
}

/// Seeds a three-generation line: Anna (1) → Bob (2) → Cara (3), each parent in
/// a single-partner family with the next as their child. Returns the path.
fn seed_three_generations(db: &Path) {
    kith(db).arg("init").assert().success();
    kith(db)
        .args(["person", "add", "--given", "Anna", "--birth", "1850"])
        .assert()
        .success(); // person 1
    kith(db)
        .args(["person", "add", "--given", "Bob"])
        .assert()
        .success(); // person 2
    kith(db)
        .args(["person", "add", "--given", "Cara"])
        .assert()
        .success(); // person 3
    kith(db)
        .args(["family", "new", "--partner", "1"])
        .assert()
        .success(); // family 1
    kith(db)
        .args(["family", "add-child", "1", "2"])
        .assert()
        .success();
    kith(db)
        .args(["family", "new", "--partner", "2"])
        .assert()
        .success(); // family 2
    kith(db)
        .args(["family", "add-child", "2", "3"])
        .assert()
        .success();
}

#[test]
fn query_ancestors_json_round_trips_into_the_relative_graph() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    seed_three_generations(&db);

    let out = kith(&db)
        .args(["query", "ancestors", "3", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // The emitted graph deserializes back into the core type.
    let graph: RelativeGraph =
        serde_json::from_slice(&out).expect("query --json parses as RelativeGraph");
    assert_eq!(graph.focus, PersonId::new(3));
    assert_eq!(graph.mode, ChartMode::Ancestors);
    // Cara's line up to the default depth: herself + Bob + Anna.
    assert!(graph.persons.iter().any(|p| p.person == PersonId::new(1)));
    assert!(graph.persons.iter().any(|p| p.person == PersonId::new(2)));
    assert_eq!(graph.persons.iter().filter(|p| p.focal).count(), 1);
}

#[test]
fn query_descendants_human_tree_matches_the_fixture_shape() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    seed_three_generations(&db);

    // The human tree walks the graph from the focus in its deterministic order.
    kith(&db)
        .args(["query", "descendants", "1"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Anna")
                .and(predicate::str::contains("Bob"))
                .and(predicate::str::contains("Cara"))
                // Anna's lifespan is rendered from the walk's vitals.
                .and(predicate::str::contains("1850")),
        );
}

#[test]
fn query_missing_root_exits_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["query", "ancestors", "999"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn query_out_of_range_generations_exits_validation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    // Range-checked by the *core* walk (MAX_GENERATIONS = 64), so it is a
    // Validation error → exit 4, not a clap usage error.
    kith(&db)
        .args(["query", "ancestors", "1", "--generations", "999"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn help_lists_the_query_noun() {
    Command::cargo_bin("kith")
        .expect("kith binary builds")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("query"));
}

// ── `kith export html` over the frozen renderer ──────────────────

#[test]
fn export_html_writes_a_self_contained_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args([
            "person",
            "add",
            "--given",
            "Ada",
            "--surname",
            "Lovelace",
            "--sex",
            "F",
            "--living",
            "false", // dead → name shows unredacted
        ])
        .assert()
        .success();

    let out = dir.path().join("tree.html");
    kith(&db)
        .args(["export", "html"])
        .arg(&out)
        .args(["--root", "1", "--mode", "descendants", "--theme", "dark"])
        .assert()
        .success();

    let html = std::fs::read_to_string(&out).expect("export written");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<svg"));
    assert!(html.contains("Lovelace")); // a dead person is not redacted
    for needle in ["http://", "https://", "<script src", "@import"] {
        assert!(!html.contains(needle), "self-contained: found {needle:?}");
    }
}

#[test]
fn export_redacts_living_by_default_and_include_living_opts_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db) // living defaults true (schema NOT NULL DEFAULT 1)
        .args(["person", "add", "--given", "Grace", "--surname", "Hopper"])
        .assert()
        .success();
    let out = dir.path().join("t.html");

    kith(&db)
        .args(["export", "html"])
        .arg(&out)
        .args(["--root", "1", "--mode", "ancestors"])
        .assert()
        .success();
    let redacted = std::fs::read_to_string(&out).unwrap();
    assert!(redacted.contains("Living"));
    assert!(!redacted.contains("Hopper"));

    kith(&db)
        .args(["export", "html"])
        .arg(&out)
        .args([
            "--root",
            "1",
            "--mode",
            "ancestors",
            "--include-living",
            "--force",
        ])
        .assert()
        .success();
    assert!(std::fs::read_to_string(&out).unwrap().contains("Hopper"));
}

#[test]
fn export_missing_root_exits_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["export", "html"])
        .arg(dir.path().join("x.html"))
        .args(["--root", "999", "--mode", "descendants"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn export_bad_mode_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    // `network` is reserved and `bogus` is unknown; both are rejected by
    // the value-parser as clap usage errors → exit 2, before any DB work.
    for mode in ["network", "bogus"] {
        kith(&db)
            .args(["export", "html"])
            .arg(dir.path().join("x.html"))
            .args(["--root", "1", "--mode", mode])
            .assert()
            .failure()
            .code(2);
    }
}

#[test]
fn export_refuses_overwrite_without_force() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "X"])
        .assert()
        .success();
    let out = dir.path().join("dup.html");
    std::fs::write(&out, "existing").unwrap();
    kith(&db)
        .args(["export", "html"])
        .arg(&out)
        .args(["--root", "1", "--mode", "descendants"])
        .assert()
        .failure()
        .code(4); // Validation (guard_overwrite)
    kith(&db)
        .args(["export", "html"])
        .arg(&out)
        .args(["--root", "1", "--mode", "descendants", "--force"])
        .assert()
        .success();
}

#[test]
fn export_over_budget_generations_exits_validation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "X"])
        .assert()
        .success();
    // 9999 > MAX_GENERATIONS (64) → core Validation → exit 4 (not a clap error).
    kith(&db)
        .args(["export", "html"])
        .arg(dir.path().join("x.html"))
        .args([
            "--root",
            "1",
            "--mode",
            "descendants",
            "--generations",
            "9999",
        ])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn export_write_to_a_missing_parent_dir_exits_io() {
    // The write's io::Error is carried as CoreError::Io, so a failed
    // write maps to exit 5 (not the uncategorized 1). The destination's parent
    // does not exist, so the overwrite guard passes (the path is absent) and
    // `std::fs::write` is what fails — deterministically on Windows.
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "X"])
        .assert()
        .success();
    let missing_parent = dir.path().join("no/such/dir/x.html"); // Windows accepts `/`
    kith(&db)
        .args(["export", "html"])
        .arg(&missing_parent)
        .args(["--root", "1", "--mode", "descendants"])
        .assert()
        .failure()
        .code(5);
}

// ── `export gedcom` / `import gedcom` ──────────────────────────────

#[test]
fn export_gedcom_writes_a_valid_document() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args([
            "person",
            "add",
            "--given",
            "Ada",
            "--surname",
            "Lovelace",
            "--sex",
            "F",
        ])
        .assert()
        .success();

    let out = dir.path().join("tree.ged");
    kith(&db)
        .args(["export", "gedcom"])
        .arg(&out)
        .assert()
        .success();

    let ged = std::fs::read_to_string(&out).expect("export written");
    assert!(ged.starts_with("0 HEAD"), "starts with HEAD: {ged:?}");
    assert!(ged.trim_end().ends_with("0 TRLR"), "ends with TRLR");
    assert!(ged.contains(" INDI"), "has an INDI record");
    assert!(ged.contains("Lovelace"), "carries the surname");
}

#[test]
fn export_gedcom_refuses_overwrite_without_force() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "X"])
        .assert()
        .success();

    let out = dir.path().join("dup.ged");
    std::fs::write(&out, "existing").expect("seed dest");
    // An existing destination without --force → Validation (exit 4).
    kith(&db)
        .args(["export", "gedcom"])
        .arg(&out)
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("--force"));
    // …unless --force is given.
    kith(&db)
        .args(["export", "gedcom"])
        .arg(&out)
        .arg("--force")
        .assert()
        .success();
}

#[test]
fn import_gedcom_round_trips_from_the_cli() {
    let dir = tempfile::tempdir().expect("temp dir");
    let src = dir.path().join("src.db");
    kith(&src).arg("init").assert().success();
    kith(&src)
        .args([
            "person",
            "add",
            "--given",
            "Ada",
            "--surname",
            "Lovelace",
            "--sex",
            "F",
            "--living",
            "false",
        ])
        .assert()
        .success();

    let ged = dir.path().join("out.ged");
    kith(&src)
        .args(["export", "gedcom"])
        .arg(&ged)
        .assert()
        .success();

    // Import into a FRESH db (created + migrated by the default path).
    let fresh = dir.path().join("fresh.db");
    let out = kith(&fresh)
        .args(["import", "gedcom"])
        .arg(&ged)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let summary: kith_core::prelude::ImportSummary =
        serde_json::from_slice(&out).expect("import --json parses as ImportSummary");
    assert_eq!(summary.individuals, 1);

    // The imported tree re-exports to a structurally-equal GEDCOM.
    let ged2 = dir.path().join("out2.ged");
    kith(&fresh)
        .args(["export", "gedcom"])
        .arg(&ged2)
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(&ged2)
            .expect("re-export written")
            .contains("Lovelace")
    );
}

#[test]
fn import_into_populated_db_without_merge_exits_validation() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Seed"])
        .assert()
        .success();

    let ged = dir.path().join("x.ged");
    kith(&db)
        .args(["export", "gedcom"])
        .arg(&ged)
        .assert()
        .success();

    // Re-importing into the same (now populated) DB without --merge is refused.
    // `-v` surfaces the cause chain so we can assert the *specific* validation
    // (the engine's emptiness guard), not merely an exit-4.
    kith(&db)
        .args(["import", "gedcom"])
        .arg(&ged)
        .arg("-v")
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("not empty"));
    // --merge appends (additive, no dedup).
    kith(&db)
        .args(["import", "gedcom"])
        .arg(&ged)
        .arg("--merge")
        .assert()
        .success();
}

#[test]
fn import_malformed_gedcom_exits_validation_not_panic() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("fresh.db");
    let bad = dir.path().join("bad.ged");
    std::fs::write(&bad, "0 HEAD\nNOTALEVEL garbage\n").expect("seed bad file");

    // A non-numeric level trips the lexer → line-cited Validation (exit 4), no panic.
    kith(&db)
        .args(["import", "gedcom"])
        .arg(&bad)
        .assert()
        .failure()
        .code(4);

    // Atomicity: the fresh target holds nothing after the failed import.
    kith(&db)
        .args(["person", "list", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[]"));
}

#[test]
fn import_missing_file_exits_io() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("fresh.db");
    kith(&db)
        .args(["import", "gedcom"])
        .arg(dir.path().join("nope.ged"))
        .assert()
        .failure()
        .code(5); // CoreError::Io (the read fails before any store work)
}

/// A 1×1 PNG; the media CRUD path only copies the bytes (mime is keyed off the
/// extension), so any content works.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[test]
fn media_add_list_set_primary_and_rm_round_trip() {
    use kith_core::prelude::MediaItem;

    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada", "--surname", "Lovelace"])
        .assert()
        .success();

    // A source image beside the temp dir; `media add` copies it into `kith.media/`.
    let img = dir.path().join("face.png");
    std::fs::write(&img, TINY_PNG).expect("write image");
    kith(&db)
        .args(["media", "add", "person:1"])
        .arg(&img)
        .arg("--primary")
        .assert()
        .success();

    // The media folder is a sibling of the DB; the file was copied in.
    let media_dir = dir.path().join("kith.media");
    assert!(media_dir.join("1.png").exists(), "the image was copied in");

    // List as JSON → one primary item.
    let output = kith(&db)
        .args(["media", "list", "person:1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let items: Vec<MediaItem> = serde_json::from_slice(&output).expect("media list --json");
    assert_eq!(items.len(), 1);
    assert!(items[0].is_primary);
    assert_eq!(items[0].media.mime.as_deref(), Some("image/png"));

    // set-primary is idempotent; rm removes it.
    kith(&db)
        .args(["media", "set-primary", "1", "person:1"])
        .assert()
        .success();
    kith(&db).args(["media", "rm", "1"]).assert().success();
    kith(&db)
        .args(["media", "list", "person:1", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[]"));
}

#[test]
fn media_add_unsupported_type_is_a_validation_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success();
    let txt = dir.path().join("notes.txt");
    std::fs::write(&txt, b"not an image").expect("write txt");
    kith(&db)
        .args(["media", "add", "person:1"])
        .arg(&txt)
        .assert()
        .failure()
        .code(4); // CoreError::Validation → exit 4
}

#[test]
fn source_and_citation_noun_round_trips_as_json() {
    // init → person → event → source → citation → show/list → rm (cascade).
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args(["person", "add", "--given", "Ada"])
        .assert()
        .success(); // person 1
    kith(&db)
        .args(["event", "add", "--subject", "person:1", "--kind", "birth"])
        .assert()
        .success(); // event 1

    // A source, then a citation on the birth event.
    let out = kith(&db)
        .args([
            "source",
            "add",
            "--title",
            "Parish Register",
            "--repository",
            "Archives",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let source: Source = serde_json::from_slice(&out).expect("source add --json → Source");
    assert_eq!(source.title, "Parish Register");

    kith(&db)
        .args([
            "citation",
            "add",
            "--source",
            "1",
            "--subject",
            "event:1",
            "--page",
            "p. 4",
            "--confidence",
            "primary",
        ])
        .assert()
        .success();

    // source show resolves the supported facts.
    let out = kith(&db)
        .args(["source", "show", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let view: SourceView = serde_json::from_slice(&out).expect("source show --json → SourceView");
    assert_eq!(view.citations.len(), 1, "the source supports one fact");

    // citation list returns the item with its source + confidence.
    let out = kith(&db)
        .args(["citation", "list", "event:1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let items: Vec<CitationItem> =
        serde_json::from_slice(&out).expect("citation list --json → Vec<CitationItem>");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].citation.confidence, Some(Confidence::Primary));
    assert_eq!(items[0].source.title, "Parish Register");

    // Deleting the source cascades its citation.
    kith(&db).args(["source", "rm", "1"]).assert().success();
    let out = kith(&db)
        .args(["citation", "list", "event:1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let items: Vec<CitationItem> = serde_json::from_slice(&out).expect("list after cascade");
    assert!(items.is_empty(), "deleting a source cascades its citations");
}

#[test]
fn citation_add_bad_confidence_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db)
        .args([
            "citation",
            "add",
            "--source",
            "1",
            "--subject",
            "event:1",
            "--confidence",
            "bogus",
        ])
        .assert()
        .code(2); // clap usage error on the bad confidence value
}

#[test]
fn source_show_missing_exits_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    kith(&db).args(["source", "show", "999"]).assert().code(3);
}

#[test]
fn search_ranks_a_name_above_a_place_and_json_round_trips() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = dir.path().join("kith.db");
    kith(&db).arg("init").assert().success();
    // Person 1 matches "Bergen" by name; person 2 only by a birthplace.
    kith(&db)
        .args(["person", "add", "--given", "Bergen", "--surname", "Hansen"])
        .assert()
        .success();
    kith(&db)
        .args(["person", "add", "--given", "Ola", "--surname", "Nordmann"])
        .assert()
        .success();
    kith(&db)
        .args([
            "event",
            "add",
            "--subject",
            "person:2",
            "--kind",
            "birth",
            "--place",
            "Bergen, Norway",
        ])
        .assert()
        .success();

    // Human table surfaces both people.
    kith(&db)
        .args(["search", "Bergen"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bergen Hansen"))
        .stdout(predicate::str::contains("Ola Nordmann"));

    // --json round-trips into Vec<SearchHit>, ranked name-first.
    let out = kith(&db)
        .args(["search", "Bergen", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let hits: Vec<SearchHit> =
        serde_json::from_slice(&out).expect("search --json parses as Vec<SearchHit>");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].individual.surname.as_deref(), Some("Hansen"));

    // A no-match is success (exit 0) with an empty result — search is a read.
    let out = kith(&db)
        .args(["search", "Zzz", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let none: Vec<SearchHit> = serde_json::from_slice(&out).expect("empty Vec<SearchHit>");
    assert!(none.is_empty(), "no match is an empty list, exit 0");
}
