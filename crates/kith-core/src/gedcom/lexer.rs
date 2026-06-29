//! The GEDCOM line lexer: a `&str` document → [`Line`]s (`level [@xref@] tag
//! [value]`), each carrying its 1-based source line number. Pure, borrowing,
//! line-numbered.

use crate::error::{CoreError, Result};

/// One physical GEDCOM line, borrowed from the source.
#[derive(Debug, Clone, Copy)]
pub(super) struct Line<'a> {
    /// 1-based source line number, for error context.
    pub line_no: u32,
    /// The nesting level (`0` for a record, deeper for sub-records).
    pub level: u8,
    /// The optional `@…@` cross-reference id, with the `@` wrapper stripped.
    pub xref: Option<&'a str>,
    /// The tag (e.g. `INDI`, `NAME`, `BIRT`).
    pub tag: &'a str,
    /// The optional value (everything after the tag's delimiting space).
    pub value: Option<&'a str>,
}

/// Lex `source` into lines. Strips a leading BOM; skips blank lines; counts EVERY
/// physical line (incl. blanks) so `line_no` matches the user's editor.
///
/// # Errors
/// [`CoreError::Validation`] for a line whose level is not a `u8`, that has no
/// tag, or whose `@xref@` pointer is unterminated.
pub(super) fn lex(source: &str) -> Result<Vec<Line<'_>>> {
    let mut out = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line_no = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let text = if i == 0 {
            raw.trim_start_matches('\u{feff}')
        } else {
            raw
        };
        // `str::lines` already strips `\n`; defend against a stray `\r` (CRLF files).
        let text = text.trim_end_matches('\r');
        if text.trim().is_empty() {
            continue;
        }
        out.push(parse_line(text, line_no)?);
    }
    Ok(out)
}

/// Parse one non-blank line into its `level [@xref@] tag [value]` parts, borrowing
/// every slice from `text`.
fn parse_line(text: &str, line_no: u32) -> Result<Line<'_>> {
    // GEDCOM forbids leading whitespace, but real files sometimes indent — be lenient.
    let (level_tok, after_level) = split_first_token(text.trim_start());
    let level: u8 = level_tok.parse().map_err(|_| {
        CoreError::Validation(format!("line {line_no}: invalid level {level_tok:?}"))
    })?;

    let mut rest = after_level.trim_start();
    let xref = if let Some(body) = rest.strip_prefix('@') {
        let end = body.find('@').ok_or_else(|| {
            CoreError::Validation(format!("line {line_no}: unterminated @xref@ pointer"))
        })?;
        let id = &body[..end];
        rest = body[end + 1..].trim_start();
        Some(id)
    } else {
        None
    };

    if rest.is_empty() {
        return Err(CoreError::Validation(format!(
            "line {line_no}: missing tag"
        )));
    }
    let (tag, value) = split_first_token(rest);
    let value = if value.is_empty() { None } else { Some(value) };
    Ok(Line {
        line_no,
        level,
        xref,
        tag,
        value,
    })
}

/// Splits `s` at its first ASCII space into `(token, rest)`, consuming that one
/// space. With no space, `rest` is empty.
fn split_first_token(s: &str) -> (&str, &str) {
    match s.find(' ') {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_ok(src: &str) -> Vec<Line<'_>> {
        lex(src).expect("lex")
    }

    #[test]
    fn lexes_level_xref_tag_and_value() {
        let lines = lex_ok("0 @I1@ INDI\n1 NAME John /Doe/\n1 BIRT\n");
        assert_eq!(lines.len(), 3);

        assert_eq!(lines[0].line_no, 1);
        assert_eq!(lines[0].level, 0);
        assert_eq!(lines[0].xref, Some("I1"));
        assert_eq!(lines[0].tag, "INDI");
        assert_eq!(lines[0].value, None);

        assert_eq!(lines[1].level, 1);
        assert_eq!(lines[1].xref, None);
        assert_eq!(lines[1].tag, "NAME");
        assert_eq!(lines[1].value, Some("John /Doe/"));

        // A tag with no value is `None`, not `Some("")`.
        assert_eq!(lines[2].tag, "BIRT");
        assert_eq!(lines[2].value, None);
    }

    #[test]
    fn strips_bom_and_skips_blank_lines_while_counting_them() {
        // A BOM precedes the first line; a blank line 2 is skipped but counted.
        let lines = lex_ok("\u{feff}0 HEAD\n\n1 CHAR UTF-8\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].tag, "HEAD");
        assert_eq!(lines[1].tag, "CHAR");
        assert_eq!(lines[1].line_no, 3, "line numbers count the blank line");
    }

    #[test]
    fn strips_carriage_returns_from_crlf_files() {
        let lines = lex_ok("0 HEAD\r\n1 CHAR UTF-8\r\n");
        assert_eq!(lines[1].value, Some("UTF-8"));
    }

    #[test]
    fn a_non_numeric_level_is_a_line_cited_validation() {
        let err = lex("X HEAD\n").expect_err("bad level");
        assert!(matches!(err, CoreError::Validation(_)));
        assert!(err.to_string().contains("line 1"), "got {err}");
    }

    #[test]
    fn a_line_with_only_a_level_is_missing_its_tag() {
        let err = lex("0\n").expect_err("missing tag");
        assert!(err.to_string().contains("missing tag"), "got {err}");
    }

    #[test]
    fn an_unterminated_xref_is_rejected() {
        let err = lex("0 @I1 INDI\n").expect_err("unterminated xref");
        assert!(err.to_string().contains("xref"), "got {err}");
    }
}
