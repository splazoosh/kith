//! Escape user-controlled strings (`display_name`, `lifespan`, `title`) before
//! they enter SVG/HTML text or attribute context. A `<` in a name must not break
//! the document or inject markup.

/// Escapes `&  <  >  "  '` to entities — safe in both element-text and
/// double-quoted-attribute contexts (so one helper covers every interpolation).
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_all_markup_significant_characters() {
        // A hostile-ish name round-trips into a safe, inert string.
        assert_eq!(
            escape(r#"O'Brien <"x"> & Co"#),
            "O&#39;Brien &lt;&quot;x&quot;&gt; &amp; Co",
        );
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        assert_eq!(
            escape("Ada Lovelace 1815\u{2013}1852"),
            "Ada Lovelace 1815\u{2013}1852",
        );
    }
}
