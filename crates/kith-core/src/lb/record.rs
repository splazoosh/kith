//! The LB wire record and its field-level conversions: the JSON shape
//! (PascalCase keys), the `DD.MM.YYYY` date parse with the SQL-Server-minimum
//! "unset" sentinel, and the empty-string-to-`None` normalization. Pure value
//! mapping — no database access.

use serde::Deserialize;

use crate::date::{DateModifier, GenealogicalDate, PartialDate};

/// The "no date" sentinel the source application writes for an unset date.
/// `1753-01-01` is SQL Server's `datetime` minimum, used as the column default,
/// so it means *unknown* — never a real 18th-century date. It maps to `None`
/// (no dated event), so an import does not invent a flood of false 1753 births.
const UNKNOWN_DATE: &str = "01.01.1753";

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

/// Parse an LB `DD.MM.YYYY` date, returning `None` for the unknown sentinel, a
/// blank, or anything that does not parse. The reference data is uniformly
/// `DD.MM.YYYY`, so the lenient fallthrough only guards dirty data — and the raw
/// text still survives in the person's notes, so nothing is lost silently.
///
/// A `00` month or day is treated as *unknown*, yielding a partial date
/// (year-only, or year + month), since a day is only meaningful with a month —
/// mirroring [`crate::date`]'s own partial-date rule.
pub(super) fn parse_lb_date(raw: &str) -> Option<GenealogicalDate> {
    let s = raw.trim();
    if s.is_empty() || s == UNKNOWN_DATE {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(year: i32, month: Option<u8>, day: Option<u8>) -> GenealogicalDate {
        GenealogicalDate::Single {
            modifier: DateModifier::Exact,
            date: PartialDate { year, month, day },
        }
    }

    #[test]
    fn unknown_sentinel_and_blank_are_no_date() {
        assert_eq!(parse_lb_date("01.01.1753"), None);
        assert_eq!(parse_lb_date(""), None);
        assert_eq!(parse_lb_date("   "), None);
    }

    #[test]
    fn full_date_parses_as_an_exact_point() {
        assert_eq!(
            parse_lb_date("24.08.1818"),
            Some(exact(1818, Some(8), Some(24)))
        );
        assert_eq!(
            parse_lb_date(" 12.05.1820 "),
            Some(exact(1820, Some(5), Some(12)))
        );
    }

    #[test]
    fn zero_components_degrade_to_a_partial_date() {
        // Unknown month → year only (a day without a month is dropped).
        assert_eq!(parse_lb_date("00.00.1790"), Some(exact(1790, None, None)));
        assert_eq!(parse_lb_date("13.00.1790"), Some(exact(1790, None, None)));
        // Unknown day, known month → year + month.
        assert_eq!(
            parse_lb_date("00.05.1790"),
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
            assert_eq!(parse_lb_date(bad), None, "expected {bad:?} → None");
        }
        // A month above 12 has no valid mapping → unknown month → year only.
        assert_eq!(parse_lb_date("01.13.1850"), Some(exact(1850, None, None)));
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
