//! Positioned chart model: the `LayoutModel` a chart mode produces and the
//! canvas and HTML export render. The geometry is computed by the pure
//! tidy-tree core ([`tidy`]); this module owns the *vocabulary* and the public
//! [`compute_layout`] entry. Coordinates are `f64` logical units (a render layer
//! scales them); `(0, 0)` is the top-left of the chart, y increasing downward.
//!
//! # The four modes
//!
//! The vocabulary (the `LayoutModel` family) and the pure positioner ([`tidy`])
//! back three **tree mode adapters** that turn a
//! [`RelativeGraph`](crate::query::RelativeGraph) into a positioned model. The
//! fourth mode, [`ChartMode::Network`], is a hand-rolled layered (Sugiyama)
//! positioner ([`network`]) over the whole connected family graph (a DAG, not a
//! tree). All four modes return a real `Ok(LayoutModel)` in the same vocabulary,
//! so the canvas and the HTML exporter render Network unchanged.

use serde::{Deserialize, Serialize};

use crate::db::Store;
use crate::error::{CoreError, Result};
use crate::model::{FamilyId, MediaId, PersonId, Sex};

mod ancestors;
mod build;
mod descendants;
mod hourglass;
pub(crate) mod metrics;
mod network;
pub(crate) mod tidy;

/// Re-exported so a consumer writes `kith_core::layout::ChartMode`; the single
/// definition lives in [`query`](crate::query) (it ships before `layout` and
/// tags the [`RelativeGraph`](crate::query::RelativeGraph)), so this is the
/// *same* type, never a rival.
pub use crate::query::ChartMode;

/// A layout-local node identity, distinct from the walk's
/// [`NodeRef`](crate::query::NodeRef) and from any row id: a person duplicated in
/// a pedigree (collapse) is two nodes referencing one [`PersonId`]. Minted by the
/// mode adapters in traversal order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(u32);

impl NodeId {
    /// Wraps a raw layout-local index.
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the underlying index.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Whether a node draws a person card or a small union joiner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// A person card.
    Person,
    /// A union (family) joiner between partners and their children.
    Union,
}

/// The real row a layout node stands for (the back-reference the canvas and
/// export use to re-root or open a detail panel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeEntity {
    /// An individual row.
    Person(PersonId),
    /// A family row.
    Union(FamilyId),
}

/// An axis-aligned rectangle in logical units (the chart `bounds`, or a node box).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// A point in logical units (a link routing anchor).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

/// Render-ready card data carried on a person node so the model is self-contained
/// over IPC and in a single-file export — no per-node re-query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeContent {
    /// "Given Surname"-style label (from the walk's `display_name`).
    pub display_name: String,
    /// Lifespan string (`1887–1956`, `b. 1990`); `None` when no year is known.
    pub lifespan: Option<String>,
    /// Recorded sex (a quiet accent cue for the renderer).
    pub sex: Sex,
    /// Privacy flag (drives export redaction; not redacted here).
    pub living: bool,
    /// The person's primary (portrait) media, if any — a lean reference, never
    /// the bytes. The canvas resolves it to an asset URL; the HTML exporter
    /// resolves it to a base64 `data:` URL. Cleared by redaction for the living.
    pub portrait: Option<MediaId>,
}

/// A positioned node: a person card or a union joiner, with its box and (for
/// persons) its render-ready content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LayoutNode {
    /// Layout-local identity (links reference it).
    pub id: NodeId,
    /// Person card or union joiner.
    pub kind: NodeKind,
    /// The real row this node stands for.
    pub entity: NodeEntity,
    /// Left edge of the node box.
    pub x: f64,
    /// Top edge of the node box.
    pub y: f64,
    /// Box width.
    pub width: f64,
    /// Box height.
    pub height: f64,
    /// Render-ready card data — `Some` for persons, `None` for unions.
    pub content: Option<NodeContent>,
    /// True only for the focus (the chart root).
    pub focal: bool,
}

/// Whether a link joins a person to a union ([`LinkKind::Partner`]) or a union to
/// a child ([`LinkKind::Descent`]) — mirrors [`RelEdge`](crate::query::RelEdge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkKind {
    /// Union → child.
    Descent,
    /// Person ↔ union.
    Partner,
}

/// A routed link between two nodes. `anchors` are the polyline waypoints (attach
/// points + the inter-generation bus); the actual curve/elbow is a render
/// concern — the model carries anchors only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LayoutLink {
    /// Source node.
    pub from: NodeId,
    /// Target node.
    pub to: NodeId,
    /// Partner or descent.
    pub kind: LinkKind,
    /// Routing waypoints, in order.
    pub anchors: Vec<Point>,
}

/// A fully positioned chart: nodes, routed links, and the tight `bounds` (the
/// exact union of all node boxes, for fit-to-screen).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LayoutModel {
    /// Which chart mode produced this model.
    pub mode: ChartMode,
    /// Positioned nodes, in a stable traversal order.
    pub nodes: Vec<LayoutNode>,
    /// Routed links, in a stable order.
    pub links: Vec<LayoutLink>,
    /// The tight union of all node boxes.
    pub bounds: Rect,
}

/// Formats a lifespan label from best-estimate years. Pure integer
/// formatting — no date math, so it stays in core, not the frontend.
///
/// The arms, by which years are known:
/// - both → `"1887–1956"`;
/// - birth only & living → `"b. 1990"`;
/// - birth only & not living → `"1887–"`;
/// - death only → `"–1956"`;
/// - neither → `None`.
///
/// The dash is an en-dash (`U+2013`), matching the typographic intent.
///
/// Called by the mode adapters when building each person node's [`NodeContent`].
pub(crate) fn lifespan(birth: Option<i32>, death: Option<i32>, living: bool) -> Option<String> {
    match (birth, death) {
        (Some(b), Some(d)) => Some(format!("{b}\u{2013}{d}")),
        (Some(b), None) if living => Some(format!("b. {b}")),
        (Some(b), None) => Some(format!("{b}\u{2013}")),
        (None, Some(d)) => Some(format!("\u{2013}{d}")),
        (None, None) => None,
    }
}

/// Computes a positioned [`LayoutModel`] for `root` in the given `mode`, walking
/// up to `generations` ranks (counted as edges from the root, as in the
/// [`query`](crate::query) walks).
///
/// Each tree mode runs its [`query`](crate::query) walk for the mode (so a walk
/// regression and a positioning regression fail *different* snapshots), then a
/// pure adapter maps the resulting [`RelativeGraph`](crate::query::RelativeGraph)
/// to the tidy arena and emits the model. For [`ChartMode::Hourglass`] the single
/// `generations` budget feeds both the up and down walks.
///
/// [`ChartMode::Network`] **ignores** `generations`: it walks the whole connected
/// component containing `root` ([`query::network`](crate::query::network)) and
/// lays it out with the layered [`network`] positioner. (The over-budget check
/// below is still applied uniformly, but Network never consumes the value.)
///
/// # Errors
/// - [`CoreError::NotFound`] if `root` does not exist.
/// - [`CoreError::Validation`] if `generations` exceeds
///   [`MAX_GENERATIONS`](crate::query::MAX_GENERATIONS).
pub fn compute_layout(
    store: &Store,
    root: PersonId,
    mode: ChartMode,
    generations: u32,
) -> Result<LayoutModel> {
    // Probe first, so a missing root is NotFound regardless of mode (mirrors the walks).
    store.get_individual(root)?;
    if generations > crate::query::MAX_GENERATIONS {
        return Err(CoreError::Validation(format!(
            "generations {generations} exceeds the maximum of {}",
            crate::query::MAX_GENERATIONS
        )));
    }
    // `ChartMode` is defined in this crate, so the match is exhaustive over its
    // variants without a wildcard (a `_` arm would be unreachable here).
    match mode {
        ChartMode::Network => Ok(network::position(&crate::query::network(store, root)?)),
        ChartMode::Descendants => Ok(descendants::position(&crate::query::descendants(
            store,
            root,
            generations,
        )?)),
        ChartMode::Ancestors => Ok(ancestors::position(&crate::query::ancestors(
            store,
            root,
            generations,
        )?)),
        ChartMode::Hourglass => Ok(hourglass::position(&crate::query::relatives(
            store,
            root,
            generations,
            generations,
        )?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FamilyId, PersonId, Sex};

    /// A hand-built two-node model (a person, a union, and a partner link) used to
    /// prove the vocabulary serialises losslessly. Built in-crate because the
    /// model structs are `#[non_exhaustive]` (no struct literal from outside).
    fn sample_model() -> LayoutModel {
        LayoutModel {
            mode: ChartMode::Descendants,
            nodes: vec![
                LayoutNode {
                    id: NodeId::new(0),
                    kind: NodeKind::Person,
                    entity: NodeEntity::Person(PersonId::new(1)),
                    x: -110.0,
                    y: 0.0,
                    width: 220.0,
                    height: 72.0,
                    content: Some(NodeContent {
                        display_name: "Ada Lovelace".to_owned(),
                        lifespan: lifespan(Some(1815), Some(1852), false),
                        sex: Sex::Female,
                        living: false,
                        portrait: None,
                    }),
                    focal: true,
                },
                LayoutNode {
                    id: NodeId::new(1),
                    kind: NodeKind::Union,
                    entity: NodeEntity::Union(FamilyId::new(1)),
                    x: -8.0,
                    y: 136.0,
                    width: 16.0,
                    height: 16.0,
                    content: None,
                    focal: false,
                },
            ],
            links: vec![LayoutLink {
                from: NodeId::new(0),
                to: NodeId::new(1),
                kind: LinkKind::Partner,
                anchors: vec![Point { x: 0.0, y: 72.0 }, Point { x: 0.0, y: 136.0 }],
            }],
            bounds: Rect {
                x: -110.0,
                y: 0.0,
                width: 220.0,
                height: 152.0,
            },
        }
    }

    #[test]
    fn layout_model_round_trips_byte_stably_through_json() {
        // Arrange
        let model = sample_model();

        // Act
        let json = serde_json::to_string(&model).expect("serialise to json");
        let back: LayoutModel = serde_json::from_str(&json).expect("deserialise from json");

        // Assert — value-stable and byte-stable.
        assert_eq!(model, back);
        assert_eq!(
            json,
            serde_json::to_string(&back).expect("re-serialise"),
            "serialisation is byte-stable across a round-trip",
        );
    }

    #[test]
    fn layout_model_round_trips_through_ron() {
        // Arrange
        let model = sample_model();

        // Act
        let ron = ron::to_string(&model).expect("serialise to ron");
        let back: LayoutModel = ron::from_str(&ron).expect("deserialise from ron");

        // Assert
        assert_eq!(model, back);
    }

    #[test]
    fn chart_mode_serialises_by_variant_name() {
        // The frontend deserialiser sees `"Descendants"`, matching the
        // `Sex`/`ChartMode`/`RelEdge` casing convention.
        let json = serde_json::to_string(&ChartMode::Descendants).expect("serialise mode");
        assert_eq!(json, "\"Descendants\"");
    }

    #[test]
    fn lifespan_both_years_uses_an_en_dash_range() {
        // Arrange / Act
        let label = lifespan(Some(1887), Some(1956), false);

        // Assert
        assert_eq!(label.as_deref(), Some("1887\u{2013}1956"));
    }

    #[test]
    fn lifespan_birth_only_living_reads_as_born_in() {
        assert_eq!(lifespan(Some(1990), None, true).as_deref(), Some("b. 1990"));
    }

    #[test]
    fn lifespan_birth_only_not_living_is_open_ended() {
        assert_eq!(
            lifespan(Some(1887), None, false).as_deref(),
            Some("1887\u{2013}")
        );
    }

    #[test]
    fn lifespan_death_only_is_a_leading_dash() {
        assert_eq!(
            lifespan(None, Some(1956), false).as_deref(),
            Some("\u{2013}1956")
        );
    }

    #[test]
    fn lifespan_no_years_is_none() {
        assert_eq!(lifespan(None, None, false), None);
        // `living` never invents a string when no birth year is known.
        assert_eq!(lifespan(None, None, true), None);
    }
}
