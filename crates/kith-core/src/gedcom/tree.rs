//! Fold the flat [`Line`]s into a record tree by level, applying CONC (concatenate,
//! no separator) and CONT (newline) to the record whose value they continue.

use super::lexer::Line;
use crate::error::{CoreError, Result};

/// A GEDCOM record and its sub-records. Values are OWNED — CONC/CONT concatenate
/// across physical lines. `line_no` is retained for error context in later passes.
#[derive(Debug, Default, Clone)]
pub(super) struct GedcomRecord {
    /// 1-based source line number of the record's own line.
    pub line_no: u32,
    /// The optional `@…@` cross-reference id (`@` wrapper stripped).
    pub xref: Option<String>,
    /// The tag (e.g. `INDI`, `NAME`).
    pub tag: String,
    /// The value, with any CONC/CONT continuations already folded in.
    pub value: Option<String>,
    /// Sub-records (CONC/CONT are folded into `value`, never kept as children).
    pub children: Vec<GedcomRecord>,
}

impl GedcomRecord {
    /// The first child with `tag`, if any.
    pub(super) fn child(&self, tag: &str) -> Option<&GedcomRecord> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// Every child with `tag` (handles repeated `NAME`/`FAMS`/`CHIL`).
    pub(super) fn children_with<'a>(
        &'a self,
        tag: &'a str,
    ) -> impl Iterator<Item = &'a GedcomRecord> + 'a {
        self.children.iter().filter(move |c| c.tag == tag)
    }

    /// `value` as `&str`.
    pub(super) fn value_str(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

/// Build the level-0 records. CONC/CONT are folded into the preceding record's
/// value and never surface as records.
///
/// # Errors
/// [`CoreError::Validation`] (with the line number) on a non-zero top-level line,
/// a level jump greater than `+1`, or a CONC/CONT with no preceding record.
pub(super) fn build(lines: &[Line<'_>]) -> Result<Vec<GedcomRecord>> {
    let mut roots = Vec::new();
    let mut pos = 0;
    while pos < lines.len() {
        let line = &lines[pos];
        if line.level != 0 {
            return Err(CoreError::Validation(format!(
                "line {}: expected a level-0 record, found level {}",
                line.line_no, line.level
            )));
        }
        if line.tag == "CONC" || line.tag == "CONT" {
            return Err(CoreError::Validation(format!(
                "line {}: {} with no preceding record",
                line.line_no, line.tag
            )));
        }
        let (record, next) = build_record(lines, pos)?;
        roots.push(record);
        pos = next;
    }
    Ok(roots)
}

/// Build the record at `lines[pos]` and its descendants/continuations. Returns the
/// record and the index of the next unconsumed line.
fn build_record(lines: &[Line<'_>], pos: usize) -> Result<(GedcomRecord, usize)> {
    let line = &lines[pos];
    let level = line.level;
    let mut record = GedcomRecord {
        line_no: line.line_no,
        xref: line.xref.map(str::to_owned),
        tag: line.tag.to_owned(),
        value: line.value.map(str::to_owned),
        children: Vec::new(),
    };

    let mut i = pos + 1;
    while i < lines.len() {
        let next = &lines[i];
        if next.level <= level {
            break;
        }
        if next.level > level + 1 {
            return Err(CoreError::Validation(format!(
                "line {}: level jumped from {} to {} (skipping {})",
                next.line_no,
                level,
                next.level,
                level + 1
            )));
        }
        // `next.level == level + 1`: an immediate child OR a CONC/CONT continuation
        // of THIS record's value.
        if next.tag == "CONC" || next.tag == "CONT" {
            apply_continuation(&mut record, next);
            i += 1;
        } else {
            let (child, after) = build_record(lines, i)?;
            record.children.push(child);
            i = after;
        }
    }
    Ok((record, i))
}

/// Fold a CONC (no separator) or CONT (a newline) line into `record.value`.
fn apply_continuation(record: &mut GedcomRecord, line: &Line<'_>) {
    let buf = record.value.get_or_insert_with(String::new);
    if line.tag == "CONT" {
        buf.push('\n');
    }
    buf.push_str(line.value.unwrap_or(""));
}

#[cfg(test)]
mod tests {
    use super::super::lexer::lex;
    use super::*;

    fn build_ok(src: &str) -> Vec<GedcomRecord> {
        build(&lex(src).expect("lex")).expect("build")
    }

    #[test]
    fn folds_a_record_with_nested_children() {
        let roots = build_ok("0 @I1@ INDI\n1 NAME John /Doe/\n2 GIVN John\n1 SEX M\n");
        assert_eq!(roots.len(), 1);
        let indi = &roots[0];
        assert_eq!(indi.tag, "INDI");
        assert_eq!(indi.xref.as_deref(), Some("I1"));
        let name = indi.child("NAME").expect("NAME child");
        assert_eq!(name.value_str(), Some("John /Doe/"));
        assert_eq!(name.child("GIVN").and_then(|g| g.value_str()), Some("John"));
        assert_eq!(indi.child("SEX").and_then(|s| s.value_str()), Some("M"));
    }

    #[test]
    fn conc_concatenates_and_cont_adds_a_newline() {
        let roots = build_ok("0 @I1@ INDI\n1 NOTE Line one\n2 CONC  continued\n2 CONT Line two\n");
        let note = roots[0].child("NOTE").expect("NOTE");
        assert_eq!(note.value_str(), Some("Line one continued\nLine two"));
        assert!(note.children.is_empty(), "CONC/CONT never become children");
    }

    #[test]
    fn repeated_tags_are_all_kept() {
        let roots = build_ok("0 @I1@ INDI\n1 NAME A /One/\n1 NAME B /Two/\n");
        let names: Vec<_> = roots[0].children_with("NAME").collect();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].value_str(), Some("A /One/"));
        assert_eq!(names[1].value_str(), Some("B /Two/"));
    }

    #[test]
    fn a_level_jump_is_a_line_cited_validation() {
        let err = build(&lex("0 HEAD\n2 CHAR UTF-8\n").expect("lex")).expect_err("jump");
        assert!(matches!(err, CoreError::Validation(_)));
        assert!(err.to_string().contains("line 2"), "got {err}");
    }

    #[test]
    fn a_top_level_continuation_has_no_preceding_record() {
        let err = build(&lex("0 CONT orphan\n").expect("lex")).expect_err("orphan cont");
        assert!(err.to_string().contains("no preceding record"), "got {err}");
    }
}
