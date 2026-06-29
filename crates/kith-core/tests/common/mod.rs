//! Shared fixture trees for the `query` walk proof (`walk_graph.rs`) and the
//! `layout` snapshot/invariant suite (`layout_graph.rs`).
//!
//! A small builder seeds an in-memory `Store` with hand-shaped families, each
//! exercising one concern (depth/order, multiple marriage, ancestor gaps,
//! pedigree collapse, a bad-data cycle, an isolated focus). Insertion order is
//! fixed, so the autoincrement row ids — and therefore the snapshots — are stable.
//!
//! Included via `mod common;` in each integration-test binary (the standard
//! `tests/common/` sharing pattern), so the items are `pub` to be reachable from
//! the test crate root. Not every fixture is used by every binary, hence the
//! crate-wide `dead_code` allow.
#![allow(dead_code, reason = "each test binary uses a subset of the fixtures")]

use kith_core::prelude::*;

/// A fixture builder over a fresh in-memory `Store`.
pub struct Tree {
    /// The seeded store the walks and layout read.
    pub store: Store,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            store: Store::open_in_memory().expect("open in-memory store"),
        }
    }

    pub fn person(&self, given: &str, surname: &str, sex: Sex) -> PersonId {
        self.store
            .create_individual(&NewIndividual {
                given_name: Some(given.to_owned()),
                surname: Some(surname.to_owned()),
                sex,
                ..Default::default()
            })
            .expect("create individual")
            .id
    }

    pub fn family(&self, partner1: Option<PersonId>, partner2: Option<PersonId>) -> FamilyId {
        self.store
            .create_family(&NewFamily {
                partner1,
                partner2,
                union_type: UnionType::Marriage,
                ..Default::default()
            })
            .expect("create family")
            .id
    }

    pub fn child(&self, family: FamilyId, child: PersonId, order: i64) {
        self.store
            .add_child(family, child, ChildRelation::Birth, order)
            .expect("add child");
    }

    pub fn birth(&self, person: PersonId, year: i32) {
        self.event(person, EventKind::Birth, year);
    }

    pub fn death(&self, person: PersonId, year: i32) {
        self.event(person, EventKind::Death, year);
    }

    fn event(&self, person: PersonId, kind: EventKind, year: i32) {
        let date = format!("{year}")
            .parse::<GenealogicalDate>()
            .expect("parse year");
        self.store
            .add_event(&NewEvent {
                subject: EventSubject::Individual(person),
                kind,
                date: Some(date),
                place: None,
                notes: None,
            })
            .expect("add event");
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A symmetric three-generation tree with a spouse and two children — the
/// baseline for depth, ordering, and (via the undated spouse) vitals.
pub fn small_balanced() -> (Tree, PersonId) {
    let t = Tree::new();
    // Grandparents (gen -2).
    let pgf = t.person("Per", "Lund", Sex::Male);
    let pgm = t.person("Petra", "Lund", Sex::Female);
    let mgf = t.person("Mads", "Lie", Sex::Male);
    let mgm = t.person("Mona", "Lie", Sex::Female);
    // Parents (gen -1).
    let father = t.person("Anders", "Lund", Sex::Male);
    let mother = t.person("Anna", "Lie", Sex::Female);
    // Focus (gen 0) and spouse.
    let root = t.person("Olav", "Lund", Sex::Male);
    let spouse = t.person("Kari", "Lund", Sex::Female);
    // Children (gen +1).
    let liv = t.person("Liv", "Lund", Sex::Female);
    let nils = t.person("Nils", "Lund", Sex::Male);

    // Vitals: a few dated lives; the spouse stays undated (→ None years).
    t.birth(pgf, 1800);
    t.death(pgf, 1865);
    t.birth(father, 1825);
    t.death(father, 1890);
    t.birth(mother, 1828);
    t.birth(root, 1850);
    t.death(root, 1915);
    t.birth(liv, 1875);

    let paternal = t.family(Some(pgf), Some(pgm));
    t.child(paternal, father, 0);
    let maternal = t.family(Some(mgf), Some(mgm));
    t.child(maternal, mother, 0);
    let parents = t.family(Some(father), Some(mother));
    t.child(parents, root, 0);
    let own = t.family(Some(root), Some(spouse));
    t.child(own, liv, 0);
    t.child(own, nils, 1);

    (t, root)
}

/// A twice-married person: descendants must yield two distinct union nodes with
/// their children grouped under the right one.
pub fn multiple_marriage() -> (Tree, PersonId) {
    let t = Tree::new();
    let root = t.person("Henrik", "Sand", Sex::Male);
    let ingrid = t.person("Ingrid", "Sand", Sex::Female);
    let johanna = t.person("Johanna", "Sand", Sex::Female);
    let arne = t.person("Arne", "Sand", Sex::Male);
    let bente = t.person("Bente", "Sand", Sex::Female);
    let carl = t.person("Carl", "Sand", Sex::Male);

    let first = t.family(Some(root), Some(ingrid));
    t.child(first, arne, 0);
    t.child(first, bente, 1);
    let second = t.family(Some(root), Some(johanna));
    t.child(second, carl, 0);

    (t, root)
}

/// A root whose maternal grandparents are unrecorded — ancestors must leave a
/// gap, never an error.
pub fn missing_grandparents() -> (Tree, PersonId) {
    let t = Tree::new();
    let root = t.person("Sven", "Holm", Sex::Male);
    let father = t.person("Bjorn", "Holm", Sex::Male);
    let mother = t.person("Greta", "Holm", Sex::Female);
    let pgf = t.person("Old", "Holm", Sex::Male);
    let pgm = t.person("Olga", "Holm", Sex::Female);
    // The mother's parents are deliberately absent.

    let parents = t.family(Some(father), Some(mother));
    t.child(parents, root, 0);
    let paternal = t.family(Some(pgf), Some(pgm));
    t.child(paternal, father, 0);

    (t, root)
}

/// Cousin marriage: the focus's parents are cousins, so their shared
/// grandparents are reachable by two distinct branches and must be **duplicated**.
pub fn cousin_marriage() -> (Tree, PersonId, PersonId) {
    let t = Tree::new();
    let old_anders = t.person("Anders", "Stamfar", Sex::Male);
    let old_berta = t.person("Berta", "Stamfar", Sex::Female);
    // Two siblings born to the shared couple.
    let carl = t.person("Carl", "Stamfar", Sex::Male);
    let dora = t.person("Dora", "Stamfar", Sex::Female);
    // Spouses who marry into each sibling's family.
    let wilma = t.person("Wilma", "Vest", Sex::Female);
    let viktor = t.person("Viktor", "Ost", Sex::Male);
    // The cousins (one child of each sibling).
    let erik = t.person("Erik", "Vest", Sex::Male);
    let frida = t.person("Frida", "Ost", Sex::Female);
    // The cousin-marriage child — the focus.
    let gustav = t.person("Gustav", "Vest", Sex::Male);

    let shared = t.family(Some(old_anders), Some(old_berta));
    t.child(shared, carl, 0);
    t.child(shared, dora, 1);
    let carls = t.family(Some(carl), Some(wilma));
    t.child(carls, erik, 0);
    let doras = t.family(Some(dora), Some(viktor));
    t.child(doras, frida, 0);
    let cousins = t.family(Some(erik), Some(frida));
    t.child(cousins, gustav, 0);

    (t, gustav, old_anders)
}

/// Bad data: `alpha` is `beta`'s parent and `beta` is `alpha`'s parent — a true
/// cycle that the path guard + generation budget must terminate.
pub fn cyclic() -> (Tree, PersonId, PersonId) {
    let t = Tree::new();
    let alpha = t.person("Alpha", "Loop", Sex::Male);
    let beta = t.person("Beta", "Loop", Sex::Female);
    let down = t.family(Some(alpha), None);
    t.child(down, beta, 0); // alpha → beta
    let up = t.family(Some(beta), None);
    t.child(up, alpha, 0); // beta → alpha (the cycle)
    (t, alpha, beta)
}

/// A person with no recorded relations at all.
pub fn isolated() -> (Tree, PersonId) {
    let t = Tree::new();
    let hermit = t.person("Hilda", "Alone", Sex::Female);
    (t, hermit)
}

/// Two separate two-generation ancestral lines joined by a marriage in the
/// youngest generation — the canonical Network multi-branch case. The focus is
/// the child of the joining couple, so the whole graph is one connected
/// component reachable from it. (Both lines are the *same* depth, so every edge
/// is between adjacent layers — no routing dummies.)
pub fn two_lineages_joined() -> (Tree, PersonId) {
    let t = Tree::new();
    // Line A: grandparents → father.
    let a_gf = t.person("Arve", "Lind", Sex::Male);
    let a_gm = t.person("Astrid", "Lind", Sex::Female);
    let father = t.person("Anders", "Lind", Sex::Male);
    // Line B: grandparents → mother.
    let b_gf = t.person("Bjorn", "Fjell", Sex::Male);
    let b_gm = t.person("Bodil", "Fjell", Sex::Female);
    let mother = t.person("Berit", "Fjell", Sex::Female);
    // The joining marriage and its child (the focus).
    let focus = t.person("Frida", "Lind", Sex::Female);

    let a_fam = t.family(Some(a_gf), Some(a_gm));
    t.child(a_fam, father, 0);
    let b_fam = t.family(Some(b_gf), Some(b_gm));
    t.child(b_fam, mother, 0);
    let join = t.family(Some(father), Some(mother));
    t.child(join, focus, 0);

    (t, focus)
}

/// Two ancestral lines of **unequal depth** joined by a marriage: the deep line
/// reaches back two generations, the shallow line one. Partner unification pulls
/// the shallow-line parent down to the deep parent's rank, so the edge from the
/// shallow grandparents' union to that parent spans more than one band and is
/// **routed through dummy nodes** — the case that proves `emit_routed` threads
/// interior waypoints and the renderer draws N-anchor polylines.
pub fn unequal_lineages() -> (Tree, PersonId) {
    let t = Tree::new();
    // Deep line: great-grandparents → grandfather → father.
    let d_ggf = t.person("Dag", "Storm", Sex::Male);
    let d_ggm = t.person("Dagny", "Storm", Sex::Female);
    let d_gf = t.person("Daniel", "Storm", Sex::Male);
    let d_gm = t.person("Dora", "Storm", Sex::Female);
    let father = t.person("David", "Storm", Sex::Male);
    // Shallow line: grandparents → mother (one generation only).
    let s_gf = t.person("Sven", "Eik", Sex::Male);
    let s_gm = t.person("Sigrid", "Eik", Sex::Female);
    let mother = t.person("Sofie", "Eik", Sex::Female);
    // The joining marriage and its child (the focus).
    let focus = t.person("Frode", "Storm", Sex::Male);

    let d_top = t.family(Some(d_ggf), Some(d_ggm));
    t.child(d_top, d_gf, 0);
    let d_mid = t.family(Some(d_gf), Some(d_gm));
    t.child(d_mid, father, 0);
    let s_top = t.family(Some(s_gf), Some(s_gm));
    t.child(s_top, mother, 0);
    let join = t.family(Some(father), Some(mother));
    t.child(join, focus, 0);

    (t, focus)
}

/// A wide four-generation pedigree (~20 people) with a cousin marriage — the
/// legible-but-non-trivial Network snapshot. Shared great-grandparents `P`+`M`
/// have three children; two of their grandchildren (cousins) marry; a third
/// branch keeps the graph broad.
pub fn wide_pedigree() -> (Tree, PersonId) {
    let t = Tree::new();
    // Gen 0 — the shared great-grandparents.
    let p = t.person("Peder", "Aas", Sex::Male);
    let m = t.person("Marit", "Aas", Sex::Female);
    // Gen 1 — three children of P+M, each with a married-in spouse.
    let a = t.person("Arne", "Aas", Sex::Male);
    let s_a = t.person("Aud", "Vik", Sex::Female);
    let b = t.person("Brit", "Aas", Sex::Female);
    let s_b = t.person("Bo", "Sol", Sex::Male);
    let c = t.person("Cato", "Aas", Sex::Male);
    let s_c = t.person("Cecilie", "Haug", Sex::Female);
    // Gen 2 — the grandchildren (cousins) plus one married-in spouse.
    let a1 = t.person("Aksel", "Aas", Sex::Male);
    let a2 = t.person("Anja", "Aas", Sex::Female);
    let a3 = t.person("Atle", "Aas", Sex::Male);
    let b1 = t.person("Bente", "Sol", Sex::Female);
    let b2 = t.person("Bjarte", "Sol", Sex::Male);
    let c1 = t.person("Camilla", "Aas", Sex::Female);
    let c2 = t.person("Carl", "Aas", Sex::Male);
    let s_c1 = t.person("Stein", "Moe", Sex::Male);
    // Gen 3 — the cousin-marriage children and a cousin's child.
    let f = t.person("Frida", "Aas", Sex::Female);
    let f2 = t.person("Finn", "Aas", Sex::Male);
    let f3 = t.person("Frode", "Aas", Sex::Male);
    let c1_kid = t.person("Kaja", "Moe", Sex::Female);

    let pm = t.family(Some(p), Some(m));
    t.child(pm, a, 0);
    t.child(pm, b, 1);
    t.child(pm, c, 2);
    let af = t.family(Some(a), Some(s_a));
    t.child(af, a1, 0);
    t.child(af, a2, 1);
    t.child(af, a3, 2);
    let bf = t.family(Some(b), Some(s_b));
    t.child(bf, b1, 0);
    t.child(bf, b2, 1);
    let cf = t.family(Some(c), Some(s_c));
    t.child(cf, c1, 0);
    t.child(cf, c2, 1);
    // The cousin marriage: Atle (A's son) × Bente (B's daughter).
    let cousins = t.family(Some(a3), Some(b1));
    t.child(cousins, f, 0);
    t.child(cousins, f2, 1);
    t.child(cousins, f3, 2);
    // A third-branch grandchild marries in and has a child.
    let c1_fam = t.family(Some(c1), Some(s_c1));
    t.child(c1_fam, c1_kid, 0);

    (t, f)
}
