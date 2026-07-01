//! The LB wire record and its field-level conversions: the JSON shape
//! (PascalCase keys), the `DD.MM.YYYY` date parse with the unset-date sentinels
//! and future-date guard, and the empty-string-to-`None` normalization. Pure
//! value mapping — no database access.

use serde::Deserialize;

use crate::date::{DateModifier, GenealogicalDate, PartialDate};

/// Literal date strings the source application writes to mean *unset* — each
/// maps to `None` (no dated event) so an import invents no false events:
/// - `01.01.1753` — SQL Server's `datetime` minimum, the column default; means
///   *unknown*, never a real 18th-century date.
/// - `05.01.2021` / `03.01.2021` — the source's fixed export-run stamps
///   (early-January-2021 defaults written for records with no real date). They
///   show up as *both* birth and death on the same person, and as a birth for
///   people who plainly predate them (e.g. one whose notes read "Fødselsdato:
///   05. Januar 1725"), so they are unset markers, not real 2021 dates.
/// - `01.01.2023` — a further stray default the exporter left on records with no
///   real date (a modern stamp on people born in the 1700s).
///
/// A birthplace with no real date still yields a place-only birth event. This
/// list is a backstop for *observed* placeholders; the [future-date
/// guard](parse_lb_date) catches any *new* impossible date generally.
const UNKNOWN_DATES: &[&str] = &["01.01.1753", "05.01.2021", "03.01.2021", "01.01.2023"];

/// With no death on record, a person born within this many years of the import
/// is assumed to still be living (a generous upper bound on a human lifespan).
/// A birth older than this — or no birth at all — imports as deceased. See
/// [`infer_living`].
const MAX_PLAUSIBLE_LIFESPAN: i32 = 110;

/// One person record in an LB JSON array.
///
/// Wire keys are PascalCase (`"FirstName"`); every field but `Id` defaults, so a
/// sparse record still deserializes. Unknown fields (`Children`, `ImageFile`,
/// `Address`, `Phone`, `Email`) are ignored — they have no home in the
/// genealogical model and are empty in the reference data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct LbPerson {
    /// Stable per-file id the parent/spouse pointers reference. `0` is the
    /// reserved "none" pointer value, so it is never a valid record id.
    pub(super) id: i64,
    /// The father's [`id`](Self::id), or `0` for none.
    #[serde(default)]
    pub(super) father_id: i64,
    /// The mother's [`id`](Self::id), or `0` for none.
    #[serde(default)]
    pub(super) mother_id: i64,
    /// A spouse's [`id`](Self::id), or `0` for none.
    #[serde(default)]
    pub(super) spouse_id: i64,
    /// Additional spouses by [`id`](Self::id), if the exporter recorded any.
    #[serde(default)]
    pub(super) spouse_list: Option<Vec<i64>>,
    /// `"M"` / `"F"` (anything else, or blank, reads as unknown sex).
    #[serde(default)]
    pub(super) gender: String,
    /// Given name(s).
    #[serde(default)]
    pub(super) first_name: String,
    /// Surname.
    #[serde(default)]
    pub(super) last_name: String,
    /// Birthplace, free text.
    #[serde(default)]
    pub(super) birth_place: String,
    /// Place of death, free text.
    #[serde(default)]
    pub(super) death_place: String,
    /// Birth date as `DD.MM.YYYY` (or the unknown sentinel / blank).
    #[serde(default)]
    pub(super) birth_date: String,
    /// Death date as `DD.MM.YYYY` (or the unknown sentinel / blank).
    #[serde(default)]
    pub(super) death_date: String,
    /// Free-form notes, preserved verbatim (the raw birth/death prose the
    /// structured fields often omit usually lives here).
    #[serde(default)]
    pub(super) notes: String,
}

impl LbPerson {
    /// The father pointer as an `Option` (`0` → `None`).
    pub(super) fn father(&self) -> Option<i64> {
        (self.father_id != 0).then_some(self.father_id)
    }

    /// The mother pointer as an `Option` (`0` → `None`).
    pub(super) fn mother(&self) -> Option<i64> {
        (self.mother_id != 0).then_some(self.mother_id)
    }

    /// Every spouse pointer (`SpouseId` then any `SpouseList`), zeros removed,
    /// in a stable order.
    pub(super) fn spouses(&self) -> Vec<i64> {
        let mut out = Vec::new();
        if self.spouse_id != 0 {
            out.push(self.spouse_id);
        }
        if let Some(list) = &self.spouse_list {
            out.extend(list.iter().copied().filter(|&s| s != 0));
        }
        out
    }
}

/// Trim `s`, returning `None` for an empty/all-whitespace value.
pub(super) fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Parse an LB `DD.MM.YYYY` date, returning `None` whenever the value carries no
/// real information: a blank, an unset-date sentinel (see [`UNKNOWN_DATES`]),
/// anything that does not parse, or a **future** year (`year > current_year`) —
/// the source stamps some unset records with a fixed export-run date, so a
/// birth/death after the import year is a placeholder, never a real event. The
/// reference data is uniformly `DD.MM.YYYY`, so the lenient fallthrough only
/// guards dirty data — and the raw text still survives in the person's notes, so
/// nothing is lost silently.
///
/// `current_year` (the current calendar year) is a **parameter**, not read from
/// the clock here, so this stays a pure, deterministic, unit-testable function;
/// the caller injects it once per import (via [`crate::db::current_year`]).
///
/// A `00` month or day is treated as *unknown*, yielding a partial date
/// (year-only, or year + month), since a day is only meaningful with a month —
/// mirroring [`crate::date`]'s own partial-date rule.
pub(super) fn parse_lb_date(raw: &str, current_year: i32) -> Option<GenealogicalDate> {
    let s = raw.trim();
    if s.is_empty() || UNKNOWN_DATES.contains(&s) {
        return None;
    }
    let mut parts = s.split('.');
    let day_s = parts.next()?;
    let month_s = parts.next()?;
    let year_s = parts.next()?;
    if parts.next().is_some() {
        return None; // more than three dot-separated components
    }
    let day_n: u8 = day_s.parse().ok()?;
    let month_n: u8 = month_s.parse().ok()?;
    let year: i32 = year_s.parse().ok()?;
    if !(-9999..=9999).contains(&year) {
        return None; // outside the date module's supported range
    }
    if year > current_year {
        return None; // a future date — a placeholder, not a real event
    }
    let month = (1..=12).contains(&month_n).then_some(month_n);
    // A day is only meaningful alongside a month (drop it otherwise).
    let day = match month {
        Some(_) if (1..=31).contains(&day_n) => Some(day_n),
        _ => None,
    };
    Some(GenealogicalDate::Single {
        modifier: DateModifier::Exact,
        date: PartialDate { year, month, day },
    })
}

/// Reconcile a parsed birth/death pair: a birth strictly *after* the death is
/// chronologically impossible. In LB exports this is a stray export-run stamp
/// that landed in the birth field beside a real historical death (e.g. birth
/// `01.01.2022`, death `01.01.1787`) — the modern default is not a real birth,
/// so it is dropped (returned as `None`) and the trustworthy death is kept.
///
/// This generalizes the fixed [`UNKNOWN_DATES`] backstop: it catches a stray
/// modern stamp by its *impossibility relative to the death*, not by a hardcoded
/// value, so a genuine recent birth with no conflicting death (a living child)
/// is left untouched. Comparison is by year — a birth and death in the same year
/// (an infant death) is valid, and the observed stray stamps are centuries off.
/// The death is never altered; when either date is absent the birth is returned
/// unchanged.
pub(super) fn reconcile_birth(
    birth: Option<GenealogicalDate>,
    death: Option<&GenealogicalDate>,
) -> Option<GenealogicalDate> {
    if let (Some(b), Some(d)) = (&birth, death)
        && b.best_estimate().year > d.best_estimate().year
    {
        return None;
    }
    birth
}

/// Infer the `living` (privacy/redaction) flag for an imported LB person from
/// its already-reconciled vital facts. The LB source has **no** explicit
/// alive/deceased field, so this is inferred:
///
/// - any **death evidence** — a real death date *or* a recorded death place ⇒
///   deceased (`false`); the source treats the person as dead.
/// - otherwise, a real **birth** within [`MAX_PLAUSIBLE_LIFESPAN`] years of
///   `current_year` ⇒ living (`true`) — plausibly still alive, so redact by
///   default.
/// - otherwise ⇒ deceased (`false`) — undated, or too old to plausibly be alive;
///   the safe default, since most LB records are long-dead ancestors with no
///   recorded death.
///
/// It takes the *parsed* death date (not the raw string) so a rejected
/// sentinel/future death correctly reads as "no death date", and a separate
/// `has_death_place` flag so a place-only death (a sentinel date beside a real
/// place of death) still reads as deceased. `current_year` is injected (as for
/// [`parse_lb_date`]) to keep this pure and deterministic.
pub(super) fn infer_living(
    birth: Option<&GenealogicalDate>,
    death: Option<&GenealogicalDate>,
    has_death_place: bool,
    current_year: i32,
) -> bool {
    if death.is_some() || has_death_place {
        return false;
    }
    match birth {
        Some(b) => current_year - b.best_estimate().year < MAX_PLAUSIBLE_LIFESPAN,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed "current year" so the date tests stay deterministic (the real
    /// import injects [`crate::db::current_year`]).
    const NOW: i32 = 2026;

    fn exact(year: i32, month: Option<u8>, day: Option<u8>) -> GenealogicalDate {
        GenealogicalDate::Single {
            modifier: DateModifier::Exact,
            date: PartialDate { year, month, day },
        }
    }

    #[test]
    fn unknown_sentinels_and_blank_are_no_date() {
        // The SQL-minimum default plus the observed export-run stamps all mean unset.
        for sentinel in ["01.01.1753", "05.01.2021", "03.01.2021", "01.01.2023"] {
            assert_eq!(parse_lb_date(sentinel, NOW), None, "sentinel {sentinel}");
        }
        assert_eq!(parse_lb_date(" 05.01.2021 ", NOW), None); // trimmed before match
        assert_eq!(parse_lb_date("", NOW), None);
        assert_eq!(parse_lb_date("   ", NOW), None);
    }

    #[test]
    fn future_years_are_dropped_but_the_current_year_is_kept() {
        // Strictly-future years are placeholders, not real events.
        assert_eq!(parse_lb_date("01.01.2027", NOW), None);
        assert_eq!(parse_lb_date("31.12.3000", NOW), None);
        // The current year is a legitimate recent date.
        assert_eq!(
            parse_lb_date("01.01.2026", NOW),
            Some(exact(2026, Some(1), Some(1)))
        );
        // A past modern year that is *not* a listed sentinel still parses — the
        // guard only rejects the future, and `2022` is neither future nor listed.
        assert_eq!(
            parse_lb_date("15.01.2022", NOW),
            Some(exact(2022, Some(1), Some(15)))
        );
    }

    #[test]
    fn full_date_parses_as_an_exact_point() {
        assert_eq!(
            parse_lb_date("24.08.1818", NOW),
            Some(exact(1818, Some(8), Some(24)))
        );
        assert_eq!(
            parse_lb_date(" 12.05.1820 ", NOW),
            Some(exact(1820, Some(5), Some(12)))
        );
    }

    #[test]
    fn zero_components_degrade_to_a_partial_date() {
        // Unknown month → year only (a day without a month is dropped).
        assert_eq!(
            parse_lb_date("00.00.1790", NOW),
            Some(exact(1790, None, None))
        );
        assert_eq!(
            parse_lb_date("13.00.1790", NOW),
            Some(exact(1790, None, None))
        );
        // Unknown day, known month → year + month.
        assert_eq!(
            parse_lb_date("00.05.1790", NOW),
            Some(exact(1790, Some(5), None))
        );
    }

    #[test]
    fn unparseable_input_is_lenient_none() {
        for bad in [
            "garbage",
            "1850",
            "12/05/1820",
            "12.05.1820.1",
            "ab.cd.efgh",
        ] {
            assert_eq!(parse_lb_date(bad, NOW), None, "expected {bad:?} → None");
        }
        // A month above 12 has no valid mapping → unknown month → year only.
        assert_eq!(
            parse_lb_date("01.13.1850", NOW),
            Some(exact(1850, None, None))
        );
    }

    #[test]
    fn reconcile_birth_drops_a_birth_after_the_death() {
        let stamp_birth = exact(2022, Some(1), Some(1));
        let real_death = exact(1787, Some(1), Some(1));
        // Born after died is impossible → the stray birth stamp is dropped.
        assert_eq!(reconcile_birth(Some(stamp_birth), Some(&real_death)), None);

        // A valid chronology is preserved untouched.
        let birth = exact(1750, None, None);
        let death = exact(1800, None, None);
        assert_eq!(reconcile_birth(Some(birth), Some(&death)), Some(birth));

        // Birth and death in the *same* year (an infant death) is valid, not an
        // inversion — even with the death earlier in the year.
        let b = exact(1850, Some(11), Some(2));
        let d = exact(1850, Some(3), Some(5));
        assert_eq!(reconcile_birth(Some(b), Some(&d)), Some(b));

        // A missing side leaves the birth as-is (nothing to compare against).
        assert_eq!(reconcile_birth(Some(birth), None), Some(birth));
        assert_eq!(reconcile_birth(None, Some(&death)), None);
    }

    #[test]
    fn infer_living_follows_death_evidence_then_a_recent_birth() {
        let recent_birth = exact(1945, Some(2), Some(23));
        let ancient_birth = exact(1820, Some(5), Some(12));
        let death = exact(1890, Some(4), Some(3));

        // Any death evidence ⇒ deceased, regardless of birth.
        assert!(!infer_living(Some(&recent_birth), Some(&death), false, NOW));
        // A place-only death (sentinel date, real place) still ⇒ deceased.
        assert!(!infer_living(Some(&recent_birth), None, true, NOW));

        // No death + a recent birth (within the plausible lifespan) ⇒ living.
        assert!(infer_living(Some(&recent_birth), None, false, NOW));
        // No death + an ancient birth ⇒ deceased (too old to be alive).
        assert!(!infer_living(Some(&ancient_birth), None, false, NOW));
        // No death + no birth ⇒ deceased (no evidence either way).
        assert!(!infer_living(None, None, false, NOW));

        // The lifespan boundary: exactly MAX_PLAUSIBLE_LIFESPAN years old is not
        // living; one year younger is.
        let boundary = exact(NOW - 110, None, None);
        let inside = exact(NOW - 109, None, None);
        assert!(!infer_living(Some(&boundary), None, false, NOW));
        assert!(infer_living(Some(&inside), None, false, NOW));
    }

    #[test]
    fn non_empty_trims_and_nulls_blanks() {
        assert_eq!(non_empty(""), None);
        assert_eq!(non_empty("   "), None);
        assert_eq!(non_empty("  Bergen "), Some("Bergen".to_owned()));
    }

    #[test]
    fn spouses_merges_scalar_and_list_dropping_zeros() {
        let person = LbPerson {
            id: 1,
            father_id: 0,
            mother_id: 0,
            spouse_id: 7,
            spouse_list: Some(vec![0, 9, 11]),
            gender: "M".to_owned(),
            first_name: String::new(),
            last_name: String::new(),
            birth_place: String::new(),
            death_place: String::new(),
            birth_date: String::new(),
            death_date: String::new(),
            notes: String::new(),
        };
        assert_eq!(person.spouses(), vec![7, 9, 11]);
    }
}
