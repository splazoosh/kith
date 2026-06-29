//! Genealogical (fuzzy) dates: parse and format GEDCOM-style strings, and
//! derive the denormalized sort key and best-estimate components the schema's
//! `events` table stores alongside the verbatim `date_original`.
//!
//! This is the single implementation of genealogical dates shared by GEDCOM,
//! the GUI date field, the CLI, and the exporter, so it is written to be
//! exactly right and is exercised by a `proptest` round-trip suite.
//!
//! # Examples
//!
//! ```
//! use kith_core::date::{DateModifier, GenealogicalDate};
//!
//! let d: GenealogicalDate = "ABT 1850".parse()?;
//! assert_eq!(d.modifier(), DateModifier::About);
//! assert_eq!(d.best_estimate().year, 1850);
//! assert_eq!(d.format_short(), "c. 1850");
//! assert!(d.sort_key().is_some());
//! # Ok::<(), kith_core::error::CoreError>(())
//! ```

use std::fmt;
use std::str::FromStr;

use jiff::civil::Date;

use crate::error::CoreError;

/// How certain/precise a date is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DateModifier {
    /// An observed, exact (if partial) date.
    Exact,
    /// `about` / `circa` (`ABT`).
    About,
    /// `before` (`BEF`) — an open upper bound.
    Before,
    /// `after` (`AFT`) — an open lower bound.
    After,
    /// `between` (`BET … AND …`) — the modifier a [`GenealogicalDate::Range`] reports.
    Between,
    /// `estimated` (`EST`).
    Estimated,
    /// `calculated` (`CAL`).
    Calculated,
}

impl DateModifier {
    /// The TEXT code stored in the `events.date_modifier` column.
    ///
    /// Write-only in the persistence layer for now: an [`Event`](crate::model::Event)'s
    /// date is reconstructed by re-parsing `date_original`, so nothing reads this
    /// code back until the query layer sorts on the denormalized columns.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::About => "about",
            Self::Before => "before",
            Self::After => "after",
            Self::Between => "between",
            Self::Estimated => "estimated",
            Self::Calculated => "calculated",
        }
    }
}

/// A possibly-partial calendar date: a year, with optional month and day.
///
/// A day is only meaningful alongside a month; parsing rejects a day without
/// one, and formatting drops a day when the month is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PartialDate {
    /// Calendar year (proleptic Gregorian; negative = BCE).
    pub year: i32,
    /// Month `1..=12`, if known.
    pub month: Option<u8>,
    /// Day `1..=31`, if known.
    pub day: Option<u8>,
}

/// A genealogical date: a single (possibly modified, possibly partial) point,
/// or a closed range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GenealogicalDate {
    /// A single point with a certainty modifier.
    Single {
        /// Certainty/precision.
        modifier: DateModifier,
        /// The point.
        date: PartialDate,
    },
    /// A closed `from..=to` range (`BET … AND …`).
    Range {
        /// Lower bound.
        from: PartialDate,
        /// Upper bound.
        to: PartialDate,
    },
}

/// Reference epoch for [`GenealogicalDate::sort_key`]: 1970-01-01. The stored
/// `date_sort` is the signed count of days from this epoch.
const SORT_EPOCH: Date = Date::constant(1970, 1, 1);

impl GenealogicalDate {
    /// The best-estimate point used for the `date_year/month/day` columns and
    /// the sort key. For a [`GenealogicalDate::Range`] this is the lower bound
    /// (`from`); for a `Single` it is the stated point regardless of modifier.
    #[must_use]
    pub const fn best_estimate(&self) -> PartialDate {
        match *self {
            Self::Single { date, .. } => date,
            Self::Range { from, .. } => from,
        }
    }

    /// The value stored in the `date_modifier` column (`Range` ⇒ `Between`).
    #[must_use]
    pub const fn modifier(&self) -> DateModifier {
        match *self {
            Self::Single { modifier, .. } => modifier,
            Self::Range { .. } => DateModifier::Between,
        }
    }

    /// A sortable integer key: the proleptic day number of the best estimate
    /// relative to [`SORT_EPOCH`] (missing month/day default to January / the
    /// 1st). Returns `None` only when the year falls outside jiff's supported
    /// `-9999..=9999` range — genealogical data never does, but the core does
    /// not panic on bad input.
    #[must_use]
    pub fn sort_key(&self) -> Option<i64> {
        let p = self.best_estimate();
        let year = i16::try_from(p.year).ok()?; // narrowing i32 -> i16 (num-cast-try-from)
        let month = i8::try_from(p.month.unwrap_or(1)).ok()?;
        let day = i8::try_from(p.day.unwrap_or(1)).ok()?;
        // Defensive: a bad day-in-month (e.g. "31 Feb" from a dirty import)
        // falls back to the 1st of the month, then to Jan 1.
        let date = Date::new(year, month, day)
            .or_else(|_| Date::new(year, month, 1))
            .or_else(|_| Date::new(year, 1, 1))
            .ok()?;
        // A `Date - Date` difference defaults its largest unit to days, so the
        // whole span is days; `get_days()` is therefore the total day count.
        let span = date.since(SORT_EPOCH).ok()?;
        Some(i64::from(span.get_days()))
    }

    /// The compact display form: `c. 1887`, `b. 1900`, `a. 1850`,
    /// `est. 1860`, `cal. 1860`, `1850–1860`, or just `12 Mar 1887`.
    #[must_use]
    pub fn format_short(&self) -> String {
        match self {
            Self::Single { modifier, date } => {
                let payload = payload_string(*date);
                match modifier {
                    DateModifier::Exact | DateModifier::Between => payload,
                    DateModifier::About => format!("c. {payload}"),
                    DateModifier::Before => format!("b. {payload}"),
                    DateModifier::After => format!("a. {payload}"),
                    DateModifier::Estimated => format!("est. {payload}"),
                    DateModifier::Calculated => format!("cal. {payload}"),
                }
            }
            Self::Range { from, to } => {
                format!("{}–{}", payload_string(*from), payload_string(*to))
            }
        }
    }

    /// Format as a GEDCOM 5.5.1 date value — the `ABT`/`BEF`/`AFT`/`EST`/`CAL`
    /// prefixes (or `BET … AND …` for a [`Range`](Self::Range)) and **uppercase**
    /// three-letter months (`12 MAR 1887`). The exact inverse of
    /// [`FromStr`](std::str::FromStr): `parse(d.format_gedcom()) == d` for every
    /// date (a `proptest` round-trip).
    ///
    /// Distinct from [`Display`](Self::fmt), which is the lowercase human
    /// long-form (`about 1887`); this is the on-the-wire GEDCOM token form
    /// (`ABT 1887`). It is the single GEDCOM date *output*, used by the
    /// [`gedcom`](crate::gedcom) writer.
    ///
    /// # Examples
    /// ```
    /// # use kith_core::date::GenealogicalDate;
    /// let d: GenealogicalDate = "12 Mar 1887".parse()?;
    /// assert_eq!(d.format_gedcom(), "12 MAR 1887");
    /// let r: GenealogicalDate = "BET 1850 AND 1860".parse()?;
    /// assert_eq!(r.format_gedcom(), "BET 1850 AND 1860");
    /// # Ok::<(), kith_core::error::CoreError>(())
    /// ```
    #[must_use]
    pub fn format_gedcom(&self) -> String {
        match self {
            Self::Single { modifier, date } => {
                let prefix = match modifier {
                    DateModifier::Exact => "",
                    DateModifier::About => "ABT ",
                    DateModifier::Before => "BEF ",
                    DateModifier::After => "AFT ",
                    DateModifier::Estimated => "EST ",
                    DateModifier::Calculated => "CAL ",
                    // A `Single` never carries `Between` (only a `Range` reports it);
                    // kept total for completeness, emitting a bare payload.
                    DateModifier::Between => "",
                };
                format!("{prefix}{}", payload_gedcom(*date))
            }
            Self::Range { from, to } => {
                format!("BET {} AND {}", payload_gedcom(*from), payload_gedcom(*to))
            }
        }
    }
}

impl fmt::Display for GenealogicalDate {
    /// The long form: `about 1887`, `before 1900`, `after 1850`,
    /// `estimated 1860`, `calculated 1860`, `between 1850 and 1860`,
    /// or `12 Mar 1887` for an exact date.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single { modifier, date } => {
                let payload = payload_string(*date);
                match modifier {
                    DateModifier::Exact => f.write_str(&payload),
                    DateModifier::About => write!(f, "about {payload}"),
                    DateModifier::Before => write!(f, "before {payload}"),
                    DateModifier::After => write!(f, "after {payload}"),
                    DateModifier::Estimated => write!(f, "estimated {payload}"),
                    DateModifier::Calculated => write!(f, "calculated {payload}"),
                    // Unreachable via parsing; kept total for completeness.
                    DateModifier::Between => write!(f, "between {payload}"),
                }
            }
            Self::Range { from, to } => {
                write!(
                    f,
                    "between {} and {}",
                    payload_string(*from),
                    payload_string(*to)
                )
            }
        }
    }
}

impl FromStr for GenealogicalDate {
    type Err = CoreError;

    /// Parses a GEDCOM-style date. Keywords and month abbreviations are
    /// case-insensitive; both the abbreviated forms (`ABT`, `BEF`, `AFT`,
    /// `EST`, `CAL`, `BET … AND …`) and the long forms emitted by [`Display`]
    /// (`about`, `before`, `between … and …`, …) are accepted, so any
    /// formatted date re-parses to itself.
    ///
    /// # Errors
    /// Returns [`CoreError::Validation`] for empty input, an unknown month, an
    /// out-of-range component, a malformed range, or any unrecognized token.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<&str> = s.split_whitespace().collect();
        let Some(&head) = tokens.first() else {
            return Err(CoreError::Validation("empty date string".to_owned()));
        };
        let upper = head.to_ascii_uppercase();

        if upper == "BET" || upper == "BETWEEN" {
            let Some(and_pos) = tokens.iter().position(|t| t.eq_ignore_ascii_case("AND")) else {
                return Err(CoreError::Validation(format!(
                    "range date {s:?} is missing its AND separator"
                )));
            };
            let from = parse_payload(&tokens[1..and_pos])?;
            let to = parse_payload(&tokens[and_pos + 1..])?;
            return Ok(Self::Range { from, to });
        }

        let (modifier, payload) = match upper.as_str() {
            "ABT" | "ABOUT" => (DateModifier::About, &tokens[1..]),
            "BEF" | "BEFORE" => (DateModifier::Before, &tokens[1..]),
            "AFT" | "AFTER" => (DateModifier::After, &tokens[1..]),
            "EST" | "ESTIMATED" => (DateModifier::Estimated, &tokens[1..]),
            "CAL" | "CALCULATED" => (DateModifier::Calculated, &tokens[1..]),
            _ => (DateModifier::Exact, &tokens[..]),
        };
        Ok(Self::Single {
            modifier,
            date: parse_payload(payload)?,
        })
    }
}

/// Parses a `[day] [month] year` payload from its whitespace-split tokens.
fn parse_payload(tokens: &[&str]) -> Result<PartialDate, CoreError> {
    if tokens.is_empty() {
        return Err(CoreError::Validation("date payload is empty".to_owned()));
    }

    let mut month: Option<u8> = None;
    let mut numerics: Vec<i32> = Vec::with_capacity(2);
    for &tok in tokens {
        if let Ok(n) = tok.parse::<i32>() {
            numerics.push(n);
        } else if tok.chars().all(|c| c.is_ascii_alphabetic()) {
            let Some(m) = month_from_abbrev(&tok.to_ascii_uppercase()) else {
                return Err(CoreError::Validation(format!("unknown month {tok:?}")));
            };
            if month.is_some() {
                return Err(CoreError::Validation(format!(
                    "more than one month in date payload {tokens:?}"
                )));
            }
            month = Some(m);
        } else {
            return Err(CoreError::Validation(format!(
                "unrecognized date token {tok:?}"
            )));
        }
    }

    let (day, year) = match numerics.as_slice() {
        [year] => (None, *year),
        [day, year] => {
            if month.is_none() {
                return Err(CoreError::Validation("a day requires a month".to_owned()));
            }
            (Some(*day), *year)
        }
        [] => return Err(CoreError::Validation("date payload has no year".to_owned())),
        _ => {
            return Err(CoreError::Validation(format!(
                "too many numbers in date payload {tokens:?}"
            )));
        }
    };

    if !(-9999..=9999).contains(&year) {
        return Err(CoreError::Validation(format!(
            "year {year} is out of range -9999..=9999"
        )));
    }
    let day = match day {
        Some(d) if (1..=31).contains(&d) => Some(
            u8::try_from(d).map_err(|_| CoreError::Validation(format!("day {d} out of range")))?,
        ),
        Some(d) => {
            return Err(CoreError::Validation(format!(
                "day {d} is out of range 1..=31"
            )));
        }
        None => None,
    };

    Ok(PartialDate { year, month, day })
}

/// Renders a payload as `[day ]Mon year`, dropping absent components (and any
/// day when the month is absent).
fn payload_string(p: PartialDate) -> String {
    match (p.month.and_then(month_abbrev), p.day) {
        (Some(mon), Some(day)) => format!("{day} {mon} {}", p.year),
        (Some(mon), None) => format!("{mon} {}", p.year),
        _ => p.year.to_string(),
    }
}

/// Renders a payload as `[day ]MON year` with an **uppercase** month — the
/// GEDCOM payload form. Mirrors [`payload_string`] (so it inherits the same
/// partial-date and year/BCE handling); only the month case differs.
fn payload_gedcom(p: PartialDate) -> String {
    match (p.month.and_then(month_abbrev), p.day) {
        (Some(mon), Some(day)) => format!("{day} {} {}", mon.to_ascii_uppercase(), p.year),
        (Some(mon), None) => format!("{} {}", mon.to_ascii_uppercase(), p.year),
        _ => p.year.to_string(),
    }
}

/// Maps an uppercased 3-letter English abbreviation to a month number `1..=12`.
fn month_from_abbrev(upper: &str) -> Option<u8> {
    let m = match upper {
        "JAN" => 1,
        "FEB" => 2,
        "MAR" => 3,
        "APR" => 4,
        "MAY" => 5,
        "JUN" => 6,
        "JUL" => 7,
        "AUG" => 8,
        "SEP" => 9,
        "OCT" => 10,
        "NOV" => 11,
        "DEC" => 12,
        _ => return None,
    };
    Some(m)
}

/// Maps a month number to its capitalized 3-letter abbreviation.
fn month_abbrev(m: u8) -> Option<&'static str> {
    let name = match m {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => return None,
    };
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Shorthand for building a [`PartialDate`] in tests.
    fn pd(year: i32, month: Option<u8>, day: Option<u8>) -> PartialDate {
        PartialDate { year, month, day }
    }

    fn single(modifier: DateModifier, date: PartialDate) -> GenealogicalDate {
        GenealogicalDate::Single { modifier, date }
    }

    #[test]
    fn parses_the_example_table() {
        use DateModifier::{About, After, Before, Calculated, Estimated, Exact};
        let cases = [
            ("1887", single(Exact, pd(1887, None, None))),
            ("Mar 1887", single(Exact, pd(1887, Some(3), None))),
            ("12 Mar 1887", single(Exact, pd(1887, Some(3), Some(12)))),
            ("ABT 1887", single(About, pd(1887, None, None))),
            ("BEF 1900", single(Before, pd(1900, None, None))),
            ("AFT 1850", single(After, pd(1850, None, None))),
            ("EST 1860", single(Estimated, pd(1860, None, None))),
            ("CAL 1860", single(Calculated, pd(1860, None, None))),
            (
                "BET 1850 AND 1860",
                GenealogicalDate::Range {
                    from: pd(1850, None, None),
                    to: pd(1860, None, None),
                },
            ),
        ];
        for (input, expected) in cases {
            let parsed: GenealogicalDate = input.parse().expect("parse example");
            assert_eq!(parsed, expected, "parsing {input:?}");
        }
    }

    #[test]
    fn parsing_is_case_insensitive_for_keywords_and_months() {
        assert_eq!(
            "abt 1850".parse::<GenealogicalDate>().expect("parse"),
            single(DateModifier::About, pd(1850, None, None))
        );
        assert_eq!(
            "12 mar 1887".parse::<GenealogicalDate>().expect("parse"),
            single(DateModifier::Exact, pd(1887, Some(3), Some(12)))
        );
    }

    #[test]
    fn long_form_keywords_re_parse() {
        // The long forms that Display emits must parse back identically.
        for input in ["about 1887", "before 1900", "after 1850", "estimated 1860"] {
            let d: GenealogicalDate = input.parse().expect("parse long form");
            assert_eq!(d.to_string(), input);
        }
        let r: GenealogicalDate = "between 1850 and 1860".parse().expect("parse range");
        assert_eq!(
            r,
            GenealogicalDate::Range {
                from: pd(1850, None, None),
                to: pd(1860, None, None),
            }
        );
    }

    #[test]
    fn formats_long_and_short_forms() {
        use DateModifier::{About, After, Before, Calculated, Estimated, Exact};
        let exact_full = single(Exact, pd(1887, Some(3), Some(12)));
        assert_eq!(exact_full.to_string(), "12 Mar 1887");
        assert_eq!(exact_full.format_short(), "12 Mar 1887");

        assert_eq!(
            single(About, pd(1887, None, None)).to_string(),
            "about 1887"
        );
        assert_eq!(
            single(About, pd(1887, None, None)).format_short(),
            "c. 1887"
        );
        assert_eq!(
            single(Before, pd(1900, None, None)).to_string(),
            "before 1900"
        );
        assert_eq!(
            single(Before, pd(1900, None, None)).format_short(),
            "b. 1900"
        );
        assert_eq!(
            single(After, pd(1850, None, None)).to_string(),
            "after 1850"
        );
        assert_eq!(
            single(After, pd(1850, None, None)).format_short(),
            "a. 1850"
        );
        assert_eq!(
            single(Estimated, pd(1860, None, None)).to_string(),
            "estimated 1860"
        );
        assert_eq!(
            single(Estimated, pd(1860, None, None)).format_short(),
            "est. 1860"
        );
        assert_eq!(
            single(Calculated, pd(1860, None, None)).to_string(),
            "calculated 1860"
        );
        assert_eq!(
            single(Calculated, pd(1860, None, None)).format_short(),
            "cal. 1860"
        );

        let range = GenealogicalDate::Range {
            from: pd(1850, None, None),
            to: pd(1860, None, None),
        };
        assert_eq!(range.to_string(), "between 1850 and 1860");
        assert_eq!(range.format_short(), "1850–1860");
    }

    #[test]
    fn date_modifier_codes_match_the_schema_comment() {
        // Guards against drift from the `events.date_modifier` schema comment.
        use DateModifier::{About, After, Before, Between, Calculated, Estimated, Exact};
        assert_eq!(Exact.as_str(), "exact");
        assert_eq!(About.as_str(), "about");
        assert_eq!(Before.as_str(), "before");
        assert_eq!(After.as_str(), "after");
        assert_eq!(Between.as_str(), "between");
        assert_eq!(Estimated.as_str(), "estimated");
        assert_eq!(Calculated.as_str(), "calculated");
    }

    #[test]
    fn abt_1850_parses_with_a_populated_sort_key() {
        let d: GenealogicalDate = "ABT 1850".parse().expect("parse");
        assert_eq!(d, single(DateModifier::About, pd(1850, None, None)));
        assert!(d.sort_key().is_some(), "ABT 1850 must have a sort key");
    }

    #[test]
    fn sort_key_orders_dates_chronologically() {
        let key = |s: &str| {
            s.parse::<GenealogicalDate>()
                .expect("parse")
                .sort_key()
                .expect("sort key")
        };
        assert!(key("1849") < key("Mar 1850"));
        assert!(key("Mar 1850") < key("12 Mar 1850"));
        assert!(key("12 Mar 1850") < key("1851"));

        // A range sorts by its lower bound.
        assert_eq!(key("BET 1850 AND 1860"), key("1850"));

        // Defensive: an impossible day falls back instead of panicking.
        let feb31: GenealogicalDate = "31 Feb 1850".parse().expect("parse");
        assert!(feb31.sort_key().is_some());
    }

    #[test]
    fn rejects_unparseable_input() {
        for bad in [
            "",
            "   ",
            "not a date",
            "Foo 1887",
            "BET 1850",
            "1 2 3 1887",
        ] {
            assert!(
                bad.parse::<GenealogicalDate>().is_err(),
                "expected {bad:?} to be rejected"
            );
        }
        // A day without a month is ambiguous and rejected.
        assert!("12 1887".parse::<GenealogicalDate>().is_err());
    }

    fn partial_date_strategy() -> impl Strategy<Value = PartialDate> {
        (-4000i32..=4000).prop_flat_map(|year| {
            prop_oneof![
                Just((None::<u8>, None::<u8>)),
                (1u8..=12).prop_map(|m| (Some(m), None)),
                (1u8..=12, 1u8..=28).prop_map(|(m, d)| (Some(m), Some(d))),
            ]
            .prop_map(move |(month, day)| PartialDate { year, month, day })
        })
    }

    fn modifier_strategy() -> impl Strategy<Value = DateModifier> {
        // `Between` is excluded: it is only produced by a `Range`, never a
        // `Single`, so it has no parseable single-date long form.
        prop_oneof![
            Just(DateModifier::Exact),
            Just(DateModifier::About),
            Just(DateModifier::Before),
            Just(DateModifier::After),
            Just(DateModifier::Estimated),
            Just(DateModifier::Calculated),
        ]
    }

    fn genealogical_date_strategy() -> impl Strategy<Value = GenealogicalDate> {
        prop_oneof![
            (modifier_strategy(), partial_date_strategy())
                .prop_map(|(modifier, date)| GenealogicalDate::Single { modifier, date }),
            (partial_date_strategy(), partial_date_strategy())
                .prop_map(|(from, to)| GenealogicalDate::Range { from, to }),
        ]
    }

    #[test]
    fn format_gedcom_canonical_spellings() {
        // The GEDCOM token form: uppercase prefixes + uppercase months + `BET … AND …`.
        let cases = [
            ("12 MAR 1887", "12 Mar 1887"),
            ("MAR 1887", "Mar 1887"),
            ("ABT 1850", "ABT 1850"),
            ("BEF 1900", "BEF 1900"),
            ("AFT 1850", "AFT 1850"),
            ("EST 1860", "EST 1860"),
            ("CAL 1860", "CAL 1860"),
            ("BET 1850 AND 1860", "BET 1850 AND 1860"),
        ];
        for (gedcom, parseable) in cases {
            let d: GenealogicalDate = parseable.parse().expect("parse");
            assert_eq!(d.format_gedcom(), gedcom, "format_gedcom of {parseable:?}");
        }
    }

    proptest! {
        #[test]
        fn parse_of_long_form_round_trips(d in genealogical_date_strategy()) {
            let long = d.to_string();
            let reparsed: GenealogicalDate = long.parse().expect("re-parse long form");
            prop_assert_eq!(reparsed, d);
            // Formatting is idempotent.
            prop_assert_eq!(reparsed.to_string(), long);
        }

        /// The GEDCOM output is the exact inverse of the parser.
        #[test]
        fn gedcom_format_round_trips(d in genealogical_date_strategy()) {
            let gedcom = d.format_gedcom();
            let reparsed: GenealogicalDate = gedcom.parse().expect("re-parse gedcom form");
            prop_assert_eq!(reparsed, d);
        }

        #[test]
        fn every_parsed_date_has_a_sort_key(d in genealogical_date_strategy()) {
            // Years stay within jiff's range, so a key is always derivable.
            prop_assert!(d.sort_key().is_some());
        }
    }
}
