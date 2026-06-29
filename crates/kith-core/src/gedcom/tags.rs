//! The GEDCOM-tag ↔ domain mappings shared by the writer and the mapper, so the
//! two halves cannot drift.
//!
//! NOTE: do **not** reuse [`EventKind::from`] for tags — it maps the lowercase
//! `as_str` *codes* (`"birth"`), NOT GEDCOM tags (`"BIRT"`). The GEDCOM tag set is
//! GEDCOM's own and is owned HERE.

use crate::model::{ChildRelation, Confidence, EventKind, NameKind};

/// A GEDCOM event tag (+ optional `TYPE` value) → [`EventKind`]. An unknown tag
/// (or a generic `EVEN`) preserves its source in [`EventKind::Other`].
pub(super) fn event_kind_for_tag(tag: &str, type_value: Option<&str>) -> EventKind {
    match tag {
        "BIRT" => EventKind::Birth,
        "DEAT" => EventKind::Death,
        "MARR" => EventKind::Marriage,
        "DIV" => EventKind::Divorce,
        "BAPM" => EventKind::Baptism,
        "BURI" => EventKind::Burial,
        "RESI" => EventKind::Residence,
        "OCCU" => EventKind::Occupation,
        "EVEN" => EventKind::Other(type_value.unwrap_or("EVEN").to_owned()),
        other => EventKind::Other(other.to_owned()),
    }
}

/// [`EventKind`] → (GEDCOM tag, optional `TYPE` value) — the writer's inverse of
/// [`event_kind_for_tag`]. An [`EventKind::Other`] round-trips as `EVEN` + a `TYPE`
/// carrying its code.
pub(super) fn tag_for_event_kind(kind: &EventKind) -> (&'static str, Option<&str>) {
    match kind {
        EventKind::Birth => ("BIRT", None),
        EventKind::Death => ("DEAT", None),
        EventKind::Marriage => ("MARR", None),
        EventKind::Divorce => ("DIV", None),
        EventKind::Baptism => ("BAPM", None),
        EventKind::Burial => ("BURI", None),
        EventKind::Residence => ("RESI", None),
        EventKind::Occupation => ("OCCU", None),
        EventKind::Other(s) => ("EVEN", Some(s.as_str())),
    }
}

/// A child's `PEDI` value → [`ChildRelation`] (an absent/unknown value → `Birth`).
pub(super) fn child_relation_for_pedi(pedi: Option<&str>) -> ChildRelation {
    match pedi {
        Some("adopted") => ChildRelation::Adopted,
        Some("foster") => ChildRelation::Foster,
        _ => ChildRelation::Birth,
    }
}

/// [`ChildRelation`] → the `PEDI` value to emit, or `None` to omit it. `Birth` is
/// the GEDCOM default (omitted); `Step` has no standard 5.5.1 `PEDI` value, so it
/// is also omitted — a documented lossy edge (`Step` reads back as `Birth`).
pub(super) fn pedi_for_child_relation(rel: ChildRelation) -> Option<&'static str> {
    match rel {
        ChildRelation::Birth | ChildRelation::Step => None,
        ChildRelation::Adopted => Some("adopted"),
        ChildRelation::Foster => Some("foster"),
    }
}

/// A `NAME.TYPE` value → [`NameKind`] (an absent/unknown `TYPE` → `Aka`).
pub(super) fn name_kind_for_type(type_value: Option<&str>) -> NameKind {
    match type_value {
        Some("birth") => NameKind::Birth,
        Some("married") => NameKind::Married,
        Some("religious") => NameKind::Religious,
        _ => NameKind::Aka,
    }
}

/// [`NameKind`] → the `NAME.TYPE` value the writer emits for an alternate name.
pub(super) fn type_for_name_kind(kind: NameKind) -> &'static str {
    match kind {
        NameKind::Birth => "birth",
        NameKind::Married => "married",
        NameKind::Aka => "aka",
        NameKind::Religious => "religious",
    }
}

/// A citation `QUAY` value → [`Confidence`]. `3`→Primary,
/// `2`→Secondary, `0`/`1`→Questionable; an absent or unknown value → `None`
/// (unspecified).
pub(super) fn confidence_for_quay(quay: Option<&str>) -> Option<Confidence> {
    match quay {
        Some("3") => Some(Confidence::Primary),
        Some("2") => Some(Confidence::Secondary),
        Some("0" | "1") => Some(Confidence::Questionable),
        _ => None,
    }
}

/// [`Confidence`] → the `QUAY` value the writer emits. Primary→`3`, Secondary→`2`,
/// Questionable→`1`. The GEDCOM `0` (unreliable) code folds into `Questionable` on
/// import — a documented lossy edge: a `0` reads back as `Questionable` and
/// re-exports as `1` (stable after the first import).
pub(super) fn quay_for_confidence(c: Confidence) -> &'static str {
    match c {
        Confidence::Primary => "3",
        Confidence::Secondary => "2",
        Confidence::Questionable => "1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_tag_table_round_trips_every_known_kind() {
        let kinds = [
            EventKind::Birth,
            EventKind::Death,
            EventKind::Marriage,
            EventKind::Divorce,
            EventKind::Baptism,
            EventKind::Burial,
            EventKind::Residence,
            EventKind::Occupation,
            EventKind::Other("graduation".to_owned()),
        ];
        for kind in kinds {
            let (tag, type_value) = tag_for_event_kind(&kind);
            assert_eq!(
                event_kind_for_tag(tag, type_value),
                kind,
                "round-trip {kind:?}"
            );
        }
        // The tag set is GEDCOM's, NOT the DB codes — guard against reusing EventKind::from.
        assert_eq!(tag_for_event_kind(&EventKind::Birth).0, "BIRT");
        assert_ne!(EventKind::Birth.as_str(), "BIRT");
    }

    #[test]
    fn pedi_table_round_trips_supported_relations() {
        for rel in [
            ChildRelation::Birth,
            ChildRelation::Adopted,
            ChildRelation::Foster,
        ] {
            assert_eq!(child_relation_for_pedi(pedi_for_child_relation(rel)), rel);
        }
        // Step has no standard PEDI value and reads back as Birth (documented loss).
        assert_eq!(pedi_for_child_relation(ChildRelation::Step), None);
        assert_eq!(
            child_relation_for_pedi(pedi_for_child_relation(ChildRelation::Step)),
            ChildRelation::Birth
        );
    }

    #[test]
    fn quay_table_round_trips_every_confidence_and_folds_zero() {
        for c in [
            Confidence::Primary,
            Confidence::Secondary,
            Confidence::Questionable,
        ] {
            assert_eq!(confidence_for_quay(Some(quay_for_confidence(c))), Some(c));
        }
        // `0` (unreliable) folds into Questionable; an absent/unknown value is None.
        assert_eq!(
            confidence_for_quay(Some("0")),
            Some(Confidence::Questionable)
        );
        assert_eq!(confidence_for_quay(None), None);
        assert_eq!(confidence_for_quay(Some("9")), None);
    }

    #[test]
    fn name_type_table_round_trips_every_kind() {
        for kind in [
            NameKind::Birth,
            NameKind::Married,
            NameKind::Religious,
            NameKind::Aka,
        ] {
            assert_eq!(name_kind_for_type(Some(type_for_name_kind(kind))), kind);
        }
        // An absent TYPE defaults to Aka.
        assert_eq!(name_kind_for_type(None), NameKind::Aka);
    }
}
