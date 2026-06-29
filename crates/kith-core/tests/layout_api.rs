//! Public-surface tests for the `layout` vocabulary: the [`compute_layout`] error
//! contract (the missing-root / over-budget rejections), the now-live tree- and
//! Network-mode dispatch, the prelude re-exports, and an external
//! consumer deserialising a `LayoutModel`.
//!
//! The model structs are `#[non_exhaustive]`, so an out-of-crate caller cannot
//! build one with a struct literal — it arrives only via serde. That is exactly
//! the frontend's path, so deserialising a hand-written JSON document is the
//! faithful test here; the in-crate round-trip (struct-literal → json/ron → back)
//! lives in the `layout` module's own `#[cfg(test)]` block.

use kith_core::prelude::*;

/// A fresh in-memory store seeded with a single individual, returning its id.
fn store_with_one_person() -> (Store, PersonId) {
    let store = Store::open_in_memory().expect("open in-memory store");
    let person = store
        .create_individual(&NewIndividual {
            given_name: Some("Ada".to_owned()),
            surname: Some("Lovelace".to_owned()),
            sex: Sex::Female,
            ..Default::default()
        })
        .expect("create individual")
        .id;
    (store, person)
}

#[test]
fn missing_root_is_not_found() {
    // Arrange — an empty store has no individual 404.
    let store = Store::open_in_memory().expect("open in-memory store");

    // Act
    let err = compute_layout(&store, PersonId::new(404), ChartMode::Descendants, 3)
        .expect_err("a missing root is an error");

    // Assert
    assert!(matches!(err, CoreError::NotFound { id: 404, .. }));
}

#[test]
fn network_mode_positions_a_lone_focus() {
    // Arrange — a lone person is its own (single-node) connected component.
    let (store, root) = store_with_one_person();

    // Act — `generations` is ignored by Network (the whole component renders).
    let model = compute_layout(&store, root, ChartMode::Network, 0)
        .expect("Network positions a lone focus");

    // Assert
    assert_eq!(model.mode, ChartMode::Network);
    assert_eq!(model.nodes.len(), 1, "a lone focus is a single node");
    assert!(model.nodes[0].focal);
    assert_eq!(model.nodes[0].kind, NodeKind::Person);
    assert!(model.links.is_empty());
    assert_eq!(model.bounds.width, model.nodes[0].width);
    assert_eq!(model.bounds.height, model.nodes[0].height);
}

#[test]
fn tree_modes_position_a_lone_focus() {
    // Arrange — a single individual with no relations.
    let (store, root) = store_with_one_person();

    // Act / Assert — each tree mode now returns a real model: one focal person
    // card, no links, and a tight one-card bounds.
    for mode in [
        ChartMode::Ancestors,
        ChartMode::Descendants,
        ChartMode::Hourglass,
    ] {
        let model = compute_layout(&store, root, mode, 2)
            .unwrap_or_else(|e| panic!("tree mode {mode:?} now positions: {e:?}"));
        assert_eq!(model.mode, mode);
        assert_eq!(model.nodes.len(), 1, "a lone focus is a single node");
        assert!(model.nodes[0].focal);
        assert_eq!(model.nodes[0].kind, NodeKind::Person);
        assert!(model.links.is_empty());
        assert_eq!(model.bounds.width, model.nodes[0].width);
        assert_eq!(model.bounds.height, model.nodes[0].height);
    }
}

#[test]
fn generations_over_the_maximum_is_rejected() {
    // Arrange
    let (store, root) = store_with_one_person();

    // Act
    let err = compute_layout(&store, root, ChartMode::Descendants, MAX_GENERATIONS + 1)
        .expect_err("over-budget generations is a validation error");

    // Assert
    assert!(matches!(err, CoreError::Validation(_)));
}

#[test]
fn an_external_consumer_can_deserialise_a_layout_model() {
    // The frontend's view of a model: variant-name enums, transparent ids, an
    // externally-tagged `entity`, and a `null` content on the union node. Proves
    // the `#[non_exhaustive]` model structs deserialise from outside the crate.
    let json = r#"{
        "mode": "Descendants",
        "nodes": [
            {
                "id": 0, "kind": "Person", "entity": {"Person": 1},
                "x": -110.0, "y": 0.0, "width": 220.0, "height": 72.0,
                "content": {
                    "display_name": "Ada Lovelace", "lifespan": "1815–1852",
                    "sex": "Female", "living": false
                },
                "focal": true
            },
            {
                "id": 1, "kind": "Union", "entity": {"Union": 1},
                "x": -8.0, "y": 136.0, "width": 16.0, "height": 16.0,
                "content": null, "focal": false
            }
        ],
        "links": [
            {
                "from": 0, "to": 1, "kind": "Partner",
                "anchors": [{"x": 0.0, "y": 72.0}, {"x": 0.0, "y": 136.0}]
            }
        ],
        "bounds": {"x": -110.0, "y": 0.0, "width": 220.0, "height": 152.0}
    }"#;

    let model: LayoutModel = serde_json::from_str(json).expect("deserialise model");

    assert_eq!(model.mode, ChartMode::Descendants);
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.nodes[0].entity, NodeEntity::Person(PersonId::new(1)));
    assert!(model.nodes[0].focal);
    let content = model.nodes[0]
        .content
        .as_ref()
        .expect("person carries content");
    assert_eq!(content.display_name, "Ada Lovelace");
    assert_eq!(content.lifespan.as_deref(), Some("1815\u{2013}1852"));
    assert_eq!(content.sex, Sex::Female);
    assert_eq!(model.nodes[1].kind, NodeKind::Union);
    assert!(model.nodes[1].content.is_none());
    assert_eq!(model.links[0].kind, LinkKind::Partner);
    assert_eq!(model.links[0].anchors.len(), 2);
    assert_eq!(model.bounds.width, 220.0);
}
