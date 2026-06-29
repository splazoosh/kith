//! Bounded relationship walks: turn the family graph around a focus into an
//! *unpositioned*, generation-ranked, deterministically-ordered [`RelativeGraph`]
//! — the input the layout engine positions. **No geometry here.**
//!
//! # The algorithm
//!
//! Each walk is one iterative, breadth-first pass over an explicit work queue
//! ([`VecDeque`]) — no recursion, so a pathological chart cannot blow the stack.
//! Every frame carries its rank (a signed generation: `< 0` ancestors, `0` the
//! focus, `> 0` descendants) and the **root→node path** (the set of [`PersonId`]s
//! on the way down to it). Two guards bound the walk:
//!
//! - **The generation budget** is the hard termination guarantee: a frame at
//!   `|generation| >= generations` is never expanded, so even a cyclic family
//!   graph stops. `generations` counts *edges* from the root — `ancestors(root, 0)`
//!   is just the root, `ancestors(root, 2)` is parents + grandparents.
//! - **The per-path guard** ([`HashSet`] membership) skips a person who already
//!   appears on their *own* root→node path, so a person never becomes their own
//!   ancestor. It is deliberately *not* a global visited set: a person reachable
//!   by two distinct branches (cousin marriage → pedigree collapse) **must** still
//!   appear once per branch ([`NodeRef`] makes each appearance a distinct node).
//!
//! [`network`] is the exception: it walks the **whole connected
//! component** as a DAG and uses a *global* visited set, so a person reached by
//! two paths appears exactly **once** (the [`layout::network`](crate::layout)
//! positioner lays the result out as a layered chart, where a shared ancestor is a
//! single node — the reverse of the tree modes' duplication).
//!
//! Output vectors are built in BFS/insertion order; the only [`HashSet`]/
//! [`HashMap`] uses are membership/visited guards, never iterated to emit output.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::db::{PooledConn, Store};
use crate::error::{CoreError, Result};
use crate::model::{Family, FamilyId, Individual, MediaId, PersonId, Sex};

/// The largest `generations` a walk will honour, guarding against absurd input
/// and bounding work and output. Out-of-range input is a
/// [`CoreError::Validation`].
pub const MAX_GENERATIONS: u32 = 64;

/// A graph-local node identity, distinct from a row id: one person may appear
/// several times in a pedigree (cousin marriage → the shared ancestor is
/// duplicated). Assigned in BFS order; edges reference it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeRef(u32);

/// Which relatives a walk gathered (also the `layout` chart mode). The three tree
/// modes, plus `Network` (the whole-component DAG walk).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ChartMode {
    /// Ancestors above the focus.
    Ancestors,
    /// Descendants below the focus.
    Descendants,
    /// Both, around the focus.
    Hourglass,
    /// Full-graph layered layout over the whole connected component ([`network`]).
    Network,
}

/// A person appearance in the graph: a render-lean payload, self-contained so a
/// consumer can draw a card without re-querying. The lifespan *string* is built
/// by the layout layer from these years.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PersonNode {
    /// Graph-local identity (an *appearance*, not the row).
    pub node: NodeRef,
    /// The underlying individual row.
    pub person: PersonId,
    /// Signed generation: `< 0` ancestors, `0` the focus, `> 0` descendants.
    pub generation: i32,
    /// A "Given Surname"-style label (a presentation join of the name parts).
    pub display_name: String,
    /// Best-estimate birth year, if a dated birth event exists.
    pub birth_year: Option<i32>,
    /// Best-estimate death year, if a dated death event exists.
    pub death_year: Option<i32>,
    /// Recorded sex.
    pub sex: Sex,
    /// Privacy flag (drives redaction; not redacted here).
    pub living: bool,
    /// The person's primary (portrait) media, if any — a lean reference, never
    /// the bytes (the renderer resolves it to an image at draw/export time).
    pub primary_portrait: Option<MediaId>,
    /// True only for the focus (`generation == 0`, the walk root).
    pub focal: bool,
}

/// A union (family) appearance joining partners to their children.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UnionNode {
    /// Graph-local identity.
    pub node: NodeRef,
    /// The underlying family row.
    pub family: FamilyId,
    /// The generation of the union's partners (children sit one rank further out).
    pub generation: i32,
    /// The partner appearances present in the graph (0–2; absent partners
    /// omitted), in `partner1`-then-`partner2` order.
    pub partners: Vec<NodeRef>,
}

/// A relationship edge, by node ref. The vocabulary is symmetric across both
/// walk directions: ancestors emit `Partner(parent → union)` + `Descent(union →
/// focus)`; descendants emit `Partner(person → union)` + `Descent(union → child)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelEdge {
    /// A person is a partner of a union.
    Partner {
        /// The partner person appearance.
        person: NodeRef,
        /// The union.
        union: NodeRef,
    },
    /// A child descends from a union.
    Descent {
        /// The union.
        union: NodeRef,
        /// The child person appearance.
        child: NodeRef,
    },
}

/// A bounded slice of the family graph around a focus — generation-ranked,
/// deterministically ordered, the **unpositioned** input to `layout`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RelativeGraph {
    /// The person the walk was rooted at.
    pub focus: PersonId,
    /// Which relatives were gathered.
    pub mode: ChartMode,
    /// Person appearances, in BFS (rank-then-insertion) order.
    pub persons: Vec<PersonNode>,
    /// Union appearances, in the order discovered.
    pub unions: Vec<UnionNode>,
    /// Edges, in the order discovered.
    pub edges: Vec<RelEdge>,
}

/// Walks ancestors up to `generations` ranks above `root` (gen 0 = root).
/// Unrecorded parents leave a gap (no node), never an error.
///
/// # Errors
/// [`CoreError::NotFound`] if `root` does not exist; [`CoreError::Validation`]
/// if `generations > MAX_GENERATIONS`; otherwise any `Store` read failure.
pub fn ancestors(store: &Store, root: PersonId, generations: u32) -> Result<RelativeGraph> {
    let focus = store.get_individual(root)?;
    validate_generations(generations)?;
    let mut builder = GraphBuilder::new(store)?;
    let focus_ref = builder.add_person_with(focus, 0, true)?;
    builder.walk_ancestors(root, focus_ref, generations)?;
    Ok(builder.finish(root, ChartMode::Ancestors))
}

/// Walks descendants up to `generations` ranks below `root`. Multiple unions per
/// person each become a [`UnionNode`] with their own children grouped beneath.
///
/// # Errors
/// As [`ancestors`].
pub fn descendants(store: &Store, root: PersonId, generations: u32) -> Result<RelativeGraph> {
    let focus = store.get_individual(root)?;
    validate_generations(generations)?;
    let mut builder = GraphBuilder::new(store)?;
    let focus_ref = builder.add_person_with(focus, 0, true)?;
    builder.walk_descendants(root, focus_ref, generations)?;
    Ok(builder.finish(root, ChartMode::Descendants))
}

/// The hourglass walk: ancestors (signed-negative ranks) ∪ descendants
/// (signed-positive), sharing the focus at rank 0 (it appears **once**).
///
/// # Errors
/// As [`ancestors`], with both `up` and `down` range-checked.
pub fn relatives(store: &Store, root: PersonId, up: u32, down: u32) -> Result<RelativeGraph> {
    let focus = store.get_individual(root)?;
    validate_generations(up)?;
    validate_generations(down)?;
    let mut builder = GraphBuilder::new(store)?;
    let focus_ref = builder.add_person_with(focus, 0, true)?;
    builder.walk_ancestors(root, focus_ref, up)?;
    builder.walk_descendants(root, focus_ref, down)?;
    Ok(builder.finish(root, ChartMode::Hourglass))
}

/// Walks the **whole connected component** containing `root` as a DAG (Network
/// mode): each person and each family appears **exactly once** (a global
/// visited set, not the tree walks' per-path guard), so a person reached by two
/// paths — a cousin marriage, two lineages joined, a remarriage — is a single
/// node. Unlike the tree walks there is no generation budget: the global visited
/// set is the termination guarantee, so even a malformed cyclic graph stops.
///
/// The BFS follows, at each person, the families it partners
/// ([`Store::families_of_partner`], FAMS) **and** the families it is a child of
/// ([`Store::families_of_child`], FAMC), both in ascending-id order, so the node
/// minting order — and therefore the layout — is deterministic. Each
/// [`PersonNode::generation`] is a best-effort signed BFS rank *hint*; the
/// positioner recomputes authoritative ranks by longest-path layering.
///
/// `mode` is [`ChartMode::Network`]; `root` is the sole focus. An isolated person
/// (no relations) is just the focus, mirroring the tree walks.
///
/// # Errors
/// [`CoreError::NotFound`] if `root` does not exist; otherwise any `Store` read
/// failure. (There is no `generations` argument, so no over-budget validation.)
pub fn network(store: &Store, root: PersonId) -> Result<RelativeGraph> {
    let focus = store.get_individual(root)?;
    let mut builder = GraphBuilder::new(store)?;
    let focus_ref = builder.add_person_with(focus, 0, true)?;
    builder.walk_network(root, focus_ref)?;
    Ok(builder.finish(root, ChartMode::Network))
}

/// Rejects a `generations` budget beyond [`MAX_GENERATIONS`].
fn validate_generations(generations: u32) -> Result<()> {
    if generations > MAX_GENERATIONS {
        return Err(CoreError::Validation(format!(
            "generations {generations} exceeds the maximum of {MAX_GENERATIONS}"
        )));
    }
    Ok(())
}

/// "Given Surname", a single part, or "(unnamed)" — a presentation join of the
/// name parts (precedented by the CLI's person summary), not domain logic.
fn display_name(ind: &Individual) -> String {
    match (ind.given_name.as_deref(), ind.surname.as_deref()) {
        (Some(given), Some(surname)) => format!("{given} {surname}"),
        (Some(part), None) | (None, Some(part)) => part.to_owned(),
        (None, None) => "(unnamed)".to_owned(),
    }
}

/// One unit of pending work: a person reached at `generation`, the [`NodeRef`]
/// already emitted for it, and the root→node `path` that guards against a person
/// becoming their own ancestor.
struct Frame {
    person: PersonId,
    node: NodeRef,
    generation: i32,
    path: HashSet<PersonId>,
}

/// Accumulates the graph in BFS/insertion order and hands out [`NodeRef`]s.
struct GraphBuilder {
    /// One pooled connection held for the whole walk: every per-row read goes
    /// through it via the `Store::*_on` twins (cached prepared statements, no
    /// per-read pool checkout) — the hot-path lever. The data and the
    /// BFS emit order are unchanged, so the `RelativeGraph` → `LayoutModel` output
    /// stays byte-identical (every layout/render snapshot holds).
    ///
    /// Holding the connection means **every** read during a walk must use a
    /// `*_on` twin: a stray `&self`-method read would try to check a *second*
    /// connection out of a single-connection in-memory pool and deadlock until the
    /// pool timeout (the in-memory walk tests are the guard that catches a leak).
    conn: PooledConn,
    next: u32,
    persons: Vec<PersonNode>,
    unions: Vec<UnionNode>,
    edges: Vec<RelEdge>,
}

impl GraphBuilder {
    /// Borrows one connection from `store`'s pool for the walk's whole duration.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired.
    fn new(store: &Store) -> Result<Self> {
        Ok(Self {
            conn: store.conn()?,
            next: 0,
            persons: Vec::new(),
            unions: Vec::new(),
            edges: Vec::new(),
        })
    }

    /// Assigns the next graph-local node identity (BFS order).
    fn alloc(&mut self) -> NodeRef {
        let node = NodeRef(self.next);
        // The graph is generation-bounded, so `u32` cannot realistically be
        // exhausted; saturate rather than risk an overflow panic on absurd data.
        self.next = self.next.saturating_add(1);
        node
    }

    /// Reads a person by id and emits its [`PersonNode`].
    fn add_person(&mut self, person: PersonId, generation: i32, focal: bool) -> Result<NodeRef> {
        let individual = Store::get_individual_on(&self.conn, person)?;
        self.add_person_with(individual, generation, focal)
    }

    /// Emits a [`PersonNode`] from an already-loaded [`Individual`] (avoids a
    /// redundant read for the focus, which is fetched to probe `NotFound`).
    fn add_person_with(
        &mut self,
        individual: Individual,
        generation: i32,
        focal: bool,
    ) -> Result<NodeRef> {
        let (birth_year, death_year) = Store::vital_years_on(&self.conn, individual.id)?;
        let primary_portrait = Store::primary_portrait_on(&self.conn, individual.id)?;
        let node = self.alloc();
        self.persons.push(PersonNode {
            node,
            person: individual.id,
            generation,
            display_name: display_name(&individual),
            birth_year,
            death_year,
            sex: individual.sex,
            living: individual.living,
            primary_portrait,
            focal,
        });
        Ok(node)
    }

    fn push_union(
        &mut self,
        node: NodeRef,
        family: FamilyId,
        generation: i32,
        partners: Vec<NodeRef>,
    ) {
        self.unions.push(UnionNode {
            node,
            family,
            generation,
            partners,
        });
    }

    fn add_edge(&mut self, edge: RelEdge) {
        self.edges.push(edge);
    }

    fn finish(self, focus: PersonId, mode: ChartMode) -> RelativeGraph {
        RelativeGraph {
            focus,
            mode,
            persons: self.persons,
            unions: self.unions,
            edges: self.edges,
        }
    }

    /// BFS **down**: each of a person's unions ([`Store::families_of_partner`],
    /// ascending id) becomes a [`UnionNode`]; the other partner is a leaf at the
    /// same rank; each child ([`Store::list_children`], birth order) descends one
    /// rank and is enqueued unless it is already on the path (cycle guard).
    fn walk_descendants(
        &mut self,
        root: PersonId,
        focus_ref: NodeRef,
        generations: u32,
    ) -> Result<()> {
        let mut queue = VecDeque::new();
        queue.push_back(Frame {
            person: root,
            node: focus_ref,
            generation: 0,
            path: HashSet::from([root]),
        });
        while let Some(frame) = queue.pop_front() {
            // The budget: never expand a frame at the rank limit (the hard
            // termination guarantee, even on cyclic data).
            if frame.generation.unsigned_abs() >= generations {
                continue;
            }
            // Each read fully materializes its owned `Vec` (the `*_on` twin returns
            // owned rows), so the `&self.conn` borrow ends before the `&mut self`
            // calls in the loop body — no borrow conflict.
            for family in Store::families_of_partner_on(&self.conn, frame.person)? {
                let union_ref = self.alloc();
                let mut partners = Vec::new();
                for slot in [family.partner1, family.partner2] {
                    let Some(partner) = slot else { continue };
                    let partner_ref = if partner == frame.person {
                        frame.node
                    } else {
                        self.add_person(partner, frame.generation, false)?
                    };
                    partners.push(partner_ref);
                    self.add_edge(RelEdge::Partner {
                        person: partner_ref,
                        union: union_ref,
                    });
                }
                self.push_union(union_ref, family.id, frame.generation, partners);
                for link in Store::list_children_on(&self.conn, family.id)? {
                    let child = link.child_id;
                    if frame.path.contains(&child) {
                        continue; // a child on its own ancestry path → stop (cycle)
                    }
                    let child_ref = self.add_person(child, frame.generation + 1, false)?;
                    self.add_edge(RelEdge::Descent {
                        union: union_ref,
                        child: child_ref,
                    });
                    let mut path = frame.path.clone();
                    path.insert(child);
                    queue.push_back(Frame {
                        person: child,
                        node: child_ref,
                        generation: frame.generation + 1,
                        path,
                    });
                }
            }
        }
        Ok(())
    }

    /// BFS **up**: each family a person is a child of
    /// ([`Store::families_of_child`], ascending id) becomes a [`UnionNode`] at the
    /// parents' rank with a [`RelEdge::Descent`] to the person; each present
    /// parent (`partner1` then `partner2`) one rank up is enqueued unless it is
    /// already on the path (cycle guard).
    fn walk_ancestors(
        &mut self,
        root: PersonId,
        focus_ref: NodeRef,
        generations: u32,
    ) -> Result<()> {
        let mut queue = VecDeque::new();
        queue.push_back(Frame {
            person: root,
            node: focus_ref,
            generation: 0,
            path: HashSet::from([root]),
        });
        while let Some(frame) = queue.pop_front() {
            if frame.generation.unsigned_abs() >= generations {
                continue;
            }
            for family_id in Store::families_of_child_on(&self.conn, frame.person)? {
                let family = Store::get_family_on(&self.conn, family_id)?;
                let union_ref = self.alloc();
                let mut partners = Vec::new();
                for slot in [family.partner1, family.partner2] {
                    let Some(parent) = slot else { continue };
                    if frame.path.contains(&parent) {
                        continue; // a parent already on the path → stop (cycle)
                    }
                    let parent_ref = self.add_person(parent, frame.generation - 1, false)?;
                    partners.push(parent_ref);
                    self.add_edge(RelEdge::Partner {
                        person: parent_ref,
                        union: union_ref,
                    });
                    let mut path = frame.path.clone();
                    path.insert(parent);
                    queue.push_back(Frame {
                        person: parent,
                        node: parent_ref,
                        generation: frame.generation - 1,
                        path,
                    });
                }
                self.push_union(union_ref, family_id, frame.generation - 1, partners);
                self.add_edge(RelEdge::Descent {
                    union: union_ref,
                    child: frame.node,
                });
            }
        }
        Ok(())
    }

    /// BFS over the **undirected** family graph from `root`, emitting each person
    /// and each family once (a global visited set), for [`network`]. At each
    /// person it follows both FAMS (the person is a partner; children one rank
    /// down) and FAMC (the person is a child; partners one rank up), in
    /// ascending-id order. The `i32` queued with each person is its BFS generation
    /// *hint*.
    fn walk_network(&mut self, root: PersonId, focus_ref: NodeRef) -> Result<()> {
        // `person_node`/`seen_family` are membership/visited guards only — never
        // iterated into output (the BFS-ordered `persons`/`unions`/`edges` vecs
        // are the output), so determinism holds despite the hashing.
        let mut person_node: HashMap<PersonId, NodeRef> = HashMap::new();
        let mut seen_family: HashSet<FamilyId> = HashSet::new();
        let mut queue: VecDeque<(PersonId, i32)> = VecDeque::new();
        person_node.insert(root, focus_ref);
        queue.push_back((root, 0));
        while let Some((person, generation)) = queue.pop_front() {
            // FAMS: families this person partners; its partners share this rank,
            // its children sit one rank down.
            for family in Store::families_of_partner_on(&self.conn, person)? {
                self.emit_network_family(
                    &family,
                    generation,
                    &mut person_node,
                    &mut seen_family,
                    &mut queue,
                )?;
            }
            // FAMC: families this person is a child of; the partners (parents) sit
            // one rank up.
            for family_id in Store::families_of_child_on(&self.conn, person)? {
                let family = Store::get_family_on(&self.conn, family_id)?;
                self.emit_network_family(
                    &family,
                    generation - 1,
                    &mut person_node,
                    &mut seen_family,
                    &mut queue,
                )?;
            }
        }
        Ok(())
    }

    /// Emits one family for the network walk (once — the `seen_family` guard): a
    /// [`UnionNode`] at `partners_generation`, a [`RelEdge::Partner`] for each
    /// present partner, and a [`RelEdge::Descent`] to each child (children one rank
    /// down). Newly-seen persons are minted and enqueued; an already-seen person
    /// reuses its [`NodeRef`] (DAG-once).
    fn emit_network_family(
        &mut self,
        family: &Family,
        partners_generation: i32,
        person_node: &mut HashMap<PersonId, NodeRef>,
        seen_family: &mut HashSet<FamilyId>,
        queue: &mut VecDeque<(PersonId, i32)>,
    ) -> Result<()> {
        if !seen_family.insert(family.id) {
            return Ok(()); // a family is emitted once, even if reached from several members
        }
        let union_ref = self.alloc();
        let mut partners = Vec::new();
        for slot in [family.partner1, family.partner2] {
            let Some(partner) = slot else { continue };
            let partner_ref =
                self.ensure_network_person(partner, partners_generation, person_node, queue)?;
            partners.push(partner_ref);
            self.add_edge(RelEdge::Partner {
                person: partner_ref,
                union: union_ref,
            });
        }
        self.push_union(union_ref, family.id, partners_generation, partners);
        // `list_children_on` returns an owned vec, so the `&self.conn` borrow ends
        // before the `&mut self` calls below.
        for link in Store::list_children_on(&self.conn, family.id)? {
            let child_ref = self.ensure_network_person(
                link.child_id,
                partners_generation + 1,
                person_node,
                queue,
            )?;
            self.add_edge(RelEdge::Descent {
                union: union_ref,
                child: child_ref,
            });
        }
        Ok(())
    }

    /// Returns the [`NodeRef`] for `person`, minting + enqueuing it on first sight
    /// (with the given generation hint) so each person is emitted once.
    fn ensure_network_person(
        &mut self,
        person: PersonId,
        generation: i32,
        person_node: &mut HashMap<PersonId, NodeRef>,
        queue: &mut VecDeque<(PersonId, i32)>,
    ) -> Result<NodeRef> {
        if let Some(&node) = person_node.get(&person) {
            return Ok(node);
        }
        let node = self.add_person(person, generation, false)?;
        person_node.insert(person, node);
        queue.push_back((person, generation));
        Ok(node)
    }
}
