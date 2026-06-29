//! `parse_date` and the shared raw-date helper.
//!
//! The genealogical date subsystem is core logic reached through a command,
//! never reimplemented in TypeScript. `parse_date` validates/previews a raw
//! string as the user types; [`parse_opt_date`] is the same parse reused by the
//! date-bearing record commands so a bad date fails as `validation` *before* any
//! write (`api-parse-dont-validate`).

use kith_core::prelude::{DateModifier, GenealogicalDate};
use serde::Serialize;

use crate::error::CommandError;

/// The preview a date field shows: the compact and long forms plus the parsed
/// certainty modifier. The raw input is preserved by the caller (it is the
/// `date_original` the core stores).
#[derive(Debug, Clone, Serialize)]
pub struct DatePreview {
    /// The compact form, e.g. `c. 1850`, `1850–1860`, `12 Mar 1887`.
    pub short: String,
    /// The long form, e.g. `about 1850`, `between 1850 and 1860`.
    pub long: String,
    /// The parsed certainty/precision modifier.
    pub modifier: DateModifier,
}

/// Parses and previews a genealogical date string without touching the database.
///
/// # Errors
/// [`CommandError`] with `kind: validation` if the input is not a recognized
/// genealogical date.
#[tauri::command]
pub async fn parse_date(input: String) -> Result<DatePreview, CommandError> {
    let date: GenealogicalDate = input.parse()?; // CoreError::Validation → kind: validation
    Ok(DatePreview {
        short: date.format_short(),
        long: date.to_string(),
        modifier: date.modifier(),
    })
}

/// Parses an optional raw date string into the typed core date, mapping a parse
/// failure to a `validation` [`CommandError`]. `None` (and only `None`) yields
/// no date — a present-but-blank string is a validation error, as in the CLI.
///
/// # Errors
/// [`CommandError`] with `kind: validation` if `Some(s)` is not a recognized date.
pub(crate) fn parse_opt_date(
    input: Option<&str>,
) -> Result<Option<GenealogicalDate>, CommandError> {
    match input {
        Some(s) => Ok(Some(s.parse::<GenealogicalDate>()?)),
        None => Ok(None),
    }
}
