//! Composite read *views*: a core record bundled with the related rows a
//! detail panel (the CLI `show` commands, the Tauri detail pane) presents
//! together. They are `Serialize` + `Deserialize` so every surface emits and
//! round-trips them.
//!
//! A view is composition of existing reads for display, not new domain logic.
//! A person's family memberships come from the indexed
//! [`Store::families_of_partner`](crate::db::Store::families_of_partner) /
//! [`Store::families_of_child`](crate::db::Store::families_of_child) reads
//! rather than a "scan every family" N+1, behind the same `load` signatures
//! and with the same membership ordering.

use serde::{Deserialize, Serialize};

use crate::db::Store;
use crate::error::Result;
use crate::model::{
    ChildLink, Citation, CitationItem, CitationSubject, Event, EventId, EventSubject, Family,
    FamilyId, Individual, Name, PersonId, Place, Source, SourceId,
};

/// A person with the related rows shown by a detail view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonView {
    /// The individual record (round-trips into [`Individual`]).
    pub individual: Individual,
    /// Alternate names, in display order.
    pub names: Vec<Name>,
    /// Events for this individual, chronological (undated last).
    pub events: Vec<Event>,
    /// Ids of families in which this person is a partner.
    pub partner_in: Vec<FamilyId>,
    /// Ids of families in which this person is a child.
    pub child_in: Vec<FamilyId>,
}

impl PersonView {
    /// Loads a person and their related rows.
    ///
    /// # Errors
    /// [`CoreError::NotFound`](crate::error::CoreError::NotFound) if no
    /// individual has `id`; otherwise any `Store` read failure.
    pub fn load(store: &Store, id: PersonId) -> Result<Self> {
        let individual = store.get_individual(id)?;
        let names = store.list_names(id)?;
        let events = store.list_events_for(EventSubject::Individual(id))?;

        // Relationship membership via the indexed FAMS/FAMC reads.
        // Both return ascending-id order — the same order the old
        // family-scan produced, so `--json` is byte-identical.
        let partner_in = store
            .families_of_partner(id)?
            .into_iter()
            .map(|f| f.id)
            .collect();
        let child_in = store.families_of_child(id)?;

        Ok(Self {
            individual,
            names,
            events,
            partner_in,
            child_in,
        })
    }
}

/// A family with partners and children resolved to full records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyView {
    /// The family record (round-trips into [`Family`]).
    pub family: Family,
    /// The first partner, resolved (if any).
    pub partner1: Option<Individual>,
    /// The second partner, resolved (if any).
    pub partner2: Option<Individual>,
    /// Children in birth order, each with its membership link and record.
    pub children: Vec<ChildView>,
    /// Family events (marriage, divorce, …), chronological.
    pub events: Vec<Event>,
}

/// A child membership with the child's record resolved alongside the link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildView {
    /// The membership link (`family_id`, `child_id`, `relation`, `sort_order`),
    /// flattened into this object.
    #[serde(flatten)]
    pub link: ChildLink,
    /// The child's individual record.
    pub individual: Individual,
}

impl FamilyView {
    /// Loads a family with its partners and children resolved.
    ///
    /// # Errors
    /// [`CoreError::NotFound`](crate::error::CoreError::NotFound) if no
    /// family has `id` (or a referenced partner/child vanished mid-read);
    /// otherwise any `Store` read failure.
    pub fn load(store: &Store, id: FamilyId) -> Result<Self> {
        let family = store.get_family(id)?;
        let partner1 = family
            .partner1
            .map(|p| store.get_individual(p))
            .transpose()?;
        let partner2 = family
            .partner2
            .map(|p| store.get_individual(p))
            .transpose()?;
        let children = store
            .list_children(id)?
            .into_iter()
            .map(|link| {
                store
                    .get_individual(link.child_id)
                    .map(|individual| ChildView { link, individual })
            })
            .collect::<Result<Vec<_>>>()?;
        let events = store.list_events_for(EventSubject::Family(id))?;
        Ok(Self {
            family,
            partner1,
            partner2,
            children,
            events,
        })
    }
}

/// An event with its place resolved and its citations (each source resolved
/// alongside) for display. The event is the primary fact target for
/// provenance, so citation editing in the GUI lives here (events-only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventView {
    /// The event record (round-trips into [`Event`]).
    pub event: Event,
    /// The resolved place, if the event has one.
    pub place: Option<Place>,
    /// Citations attached to this event, each with its source resolved (no N+1),
    /// in ascending citation-id order.
    pub citations: Vec<CitationItem>,
}

impl EventView {
    /// Loads an event and resolves its place and citations.
    ///
    /// # Errors
    /// [`CoreError::NotFound`](crate::error::CoreError::NotFound) if no
    /// event has `id` (or its place vanished mid-read); otherwise any `Store`
    /// read failure.
    pub fn load(store: &Store, id: EventId) -> Result<Self> {
        let event = store.get_event(id)?;
        let place = event.place.map(|p| store.get_place(p)).transpose()?;
        let citations = store.citations_for(CitationSubject::Event(id))?;
        Ok(Self {
            event,
            place,
            citations,
        })
    }
}

/// A source with the facts it supports — backs the Sources management surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceView {
    /// The source record (round-trips into [`Source`]).
    pub source: Source,
    /// Every citation that cites this source, ascending citation-id order.
    pub citations: Vec<Citation>,
}

impl SourceView {
    /// Loads a source and the citations that cite it.
    ///
    /// # Errors
    /// [`CoreError::NotFound`](crate::error::CoreError::NotFound) if no
    /// source has `id`; otherwise any `Store` read failure.
    pub fn load(store: &Store, id: SourceId) -> Result<Self> {
        let source = store.get_source(id)?;
        let citations = store.list_citations_for_source(id)?;
        Ok(Self { source, citations })
    }
}
