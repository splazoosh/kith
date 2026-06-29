//! Living-person redaction — the export's headline privacy promise.
//! A render concern, NOT a model/query change: the live canvas shows the owner
//! everything; only the *export* redacts, and only here, so both shells
//! (CLI, GUI) share one path and neither can forget it.

use std::borrow::Cow;

use crate::layout::{LayoutModel, NodeKind};

/// Borrow the model when `include_living`; else return an owned clone with every
/// living person node redacted to `"Living"` / no lifespan / no portrait, leaving
/// the box, sex edge, focal flag, unions, and links untouched.
///
/// Clearing `portrait` here means a living person's media id never
/// reaches the export's portrait resolver — the export resolves URLs over *this*
/// redacted model — so no image is embedded. `include_living` is the only opt-out.
pub(crate) fn redact_living(model: &LayoutModel, include_living: bool) -> Cow<'_, LayoutModel> {
    if include_living {
        return Cow::Borrowed(model);
    }
    let mut redacted = model.clone();
    for node in &mut redacted.nodes {
        if node.kind == NodeKind::Person
            && let Some(content) = node.content.as_mut()
            && content.living
        {
            content.display_name = "Living".to_owned();
            content.lifespan = None;
            content.portrait = None;
        }
    }
    Cow::Owned(redacted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ChartMode, LayoutNode, NodeContent, NodeEntity, NodeId, Rect};
    use crate::model::{FamilyId, MediaId, PersonId, Sex};

    /// A hand-built model (in-crate, so the `#[non_exhaustive]` structs are
    /// constructible): a living person, a dead person, and an inert union.
    fn sample(name: &str) -> LayoutModel {
        LayoutModel {
            mode: ChartMode::Descendants,
            nodes: vec![
                LayoutNode {
                    id: NodeId::new(0),
                    kind: NodeKind::Person,
                    entity: NodeEntity::Person(PersonId::new(1)),
                    x: 0.0,
                    y: 0.0,
                    width: 220.0,
                    height: 72.0,
                    content: Some(NodeContent {
                        display_name: name.to_owned(),
                        lifespan: Some("b. 1990".to_owned()),
                        sex: Sex::Female,
                        living: true,
                        portrait: Some(MediaId::new(7)),
                    }),
                    focal: true,
                },
                LayoutNode {
                    id: NodeId::new(1),
                    kind: NodeKind::Person,
                    entity: NodeEntity::Person(PersonId::new(2)),
                    x: 0.0,
                    y: 200.0,
                    width: 220.0,
                    height: 72.0,
                    content: Some(NodeContent {
                        display_name: "Dead Ancestor".to_owned(),
                        lifespan: Some("1800\u{2013}1880".to_owned()),
                        sex: Sex::Male,
                        living: false,
                        portrait: None,
                    }),
                    focal: false,
                },
                LayoutNode {
                    id: NodeId::new(2),
                    kind: NodeKind::Union,
                    entity: NodeEntity::Union(FamilyId::new(1)),
                    x: 100.0,
                    y: 150.0,
                    width: 16.0,
                    height: 16.0,
                    content: None,
                    focal: false,
                },
            ],
            links: vec![],
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 220.0,
                height: 272.0,
            },
        }
    }

    #[test]
    fn redacts_a_living_person_keeping_box_sex_focal_and_entity() {
        // Arrange
        let model = sample("Private Person");

        // Act
        let redacted = redact_living(&model, false);

        // Assert — name/lifespan gone; structure intact.
        let node = &redacted.nodes[0];
        let content = node.content.as_ref().expect("person keeps content");
        assert_eq!(content.display_name, "Living");
        assert_eq!(content.lifespan, None);
        assert_eq!(content.portrait, None); // the living portrait is dropped
        assert_eq!(content.sex, Sex::Female); // the sex accent is kept
        assert!(node.focal);
        assert_eq!(node.width, 220.0);
        assert_eq!(node.entity, NodeEntity::Person(PersonId::new(1)));
    }

    #[test]
    fn leaves_a_non_living_person_unchanged() {
        // Arrange / Act
        let model = sample("Private Person");
        let redacted = redact_living(&model, false);

        // Assert
        let content = redacted.nodes[1].content.as_ref().expect("content");
        assert_eq!(content.display_name, "Dead Ancestor");
        assert_eq!(content.lifespan.as_deref(), Some("1800\u{2013}1880"));
    }

    #[test]
    fn include_living_borrows_the_model_untouched() {
        // Arrange
        let model = sample("Private Person");

        // Act
        let borrowed = redact_living(&model, true);

        // Assert — no clone, every field intact.
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let content = borrowed.nodes[0].content.as_ref().expect("content");
        assert_eq!(content.display_name, "Private Person");
        assert_eq!(content.lifespan.as_deref(), Some("b. 1990"));
        assert_eq!(content.portrait, Some(MediaId::new(7))); // kept when opted in
    }

    #[test]
    fn a_union_without_content_is_untouched_and_does_not_panic() {
        // Arrange / Act
        let model = sample("Private Person");
        let redacted = redact_living(&model, false);

        // Assert
        assert_eq!(redacted.nodes[2].kind, NodeKind::Union);
        assert_eq!(redacted.nodes[2].content, None);
    }
}
