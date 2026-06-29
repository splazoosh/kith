//! Ranked, multi-field full-text search over individuals.
//!
//! [`Store::search`] queries the `person_search` FTS5 index (migration 0002),
//! which covers an individual's core names, alternate names, nickname, notes,
//! and the place names of their own events. Results are ordered by relevance
//! (`bm25`, with the `names` column weighted highest) and broken by a
//! deterministic `surname, given_name, id` tie so the output is stable for
//! tests, the CLI, and snapshots.
//!
//! The user's raw query is never injected as a `MATCH` expression: it is
//! sanitized by [`to_match_expr`] into a quoted, prefix-matched, term-ANDed
//! expression, so a stray `"`/`*`/`AND`/`:` is harmless rather than an FTS
//! syntax error. An empty (or all-punctuation) query short-circuits to a
//! bounded, name-ordered slice of the full individual list — the historical
//! "empty query matches all" contract.

use rusqlite::params;

use crate::error::Result;
use crate::model::Individual;

use super::Store;
use super::individual::row_to_individual;

/// The individual columns selected (prefixed for the FTS join), in the order
/// [`row_to_individual`] reads them by name.
const INDIVIDUAL_COLUMNS: &str = "i.id, i.given_name, i.surname, i.name_prefix, \
     i.name_suffix, i.nickname, i.sex, i.living, i.notes";

/// A ranked search result: the matched [`Individual`] plus an optional short
/// "why-matched" snippet (the best-matching indexed text — e.g. a maiden name
/// or a birthplace), shown as a subtitle in the GUI palette and the CLI table.
///
/// The underlying `bm25` score stays internal (it only drives ordering), so it
/// is not a wire field and cannot drift a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    /// The person who matched.
    pub individual: Individual,
    /// A short excerpt of the text that matched, if any (`None` for the
    /// empty-query "list everything" path).
    pub context: Option<String>,
}

impl Store {
    /// Searches individuals by `query` across names, alternate names, nickname,
    /// notes, and event places, returning up to `limit` [`SearchHit`]s ranked
    /// best-match-first (`bm25`) with deterministic `surname, given_name, id`
    /// ties. An empty or all-punctuation `query` returns a bounded, name-ordered
    /// slice of every individual (the "matches all" contract).
    ///
    /// The query is sanitized into a safe FTS5 `MATCH` expression (each term
    /// quoted and prefix-matched), so metacharacters never cause an error.
    ///
    /// # Errors
    /// Returns [`CoreError`](crate::error::CoreError) if a connection cannot be
    /// acquired or the query fails.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let conn = self.conn()?;
        // `limit` is a small bound; clamp rather than risk a usize→i64 overflow.
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);

        let Some(match_expr) = to_match_expr(query) else {
            // Empty / all-punctuation query → the full list, name-ordered, no context.
            let mut stmt = conn.prepare(&format!(
                "SELECT {INDIVIDUAL_COLUMNS} FROM individuals i
                 ORDER BY i.surname, i.given_name, i.id
                 LIMIT ?1"
            ))?;
            let hits = stmt
                .query_map([limit], |row| {
                    Ok(SearchHit {
                        individual: row_to_individual(row)?,
                        context: None,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            return Ok(hits);
        };

        let mut stmt = conn.prepare(&format!(
            "SELECT {INDIVIDUAL_COLUMNS},
                    snippet(person_search, -1, '', '', '…', 8) AS context
             FROM person_search ps JOIN individuals i ON i.id = ps.rowid
             WHERE person_search MATCH ?1
             ORDER BY bm25(person_search, 10.0, 2.0, 1.0), i.surname, i.given_name, i.id
             LIMIT ?2"
        ))?;
        let hits = stmt
            .query_map(params![match_expr, limit], |row| {
                Ok(SearchHit {
                    individual: row_to_individual(row)?,
                    context: clean_snippet(row.get::<_, Option<String>>("context")?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(hits)
    }
}

/// Turns a raw user query into a safe FTS5 `MATCH` expression: each
/// whitespace-separated term that contains at least one alphanumeric character
/// is double-quoted (embedded quotes doubled — FTS5 string escaping) and given a
/// `*` suffix for as-you-type prefix matching; terms are implicitly ANDed.
/// Punctuation-only terms (which would tokenize to nothing) are dropped. Returns
/// `None` when no usable term remains — the caller then lists everything.
fn to_match_expr(query: &str) -> Option<String> {
    let mut expr = String::with_capacity(query.len() + 8);
    for term in query.split_whitespace() {
        // A term with no alphanumeric char yields no FTS token; skip it so the
        // expression can never be an empty/zero-token phrase (a syntax error).
        if !term.chars().any(char::is_alphanumeric) {
            continue;
        }
        if !expr.is_empty() {
            expr.push(' ');
        }
        expr.push('"');
        for ch in term.chars() {
            if ch == '"' {
                expr.push('"'); // double embedded quotes
            }
            expr.push(ch);
        }
        expr.push_str("\"*"); // prefix-match the term's last token
    }
    (!expr.is_empty()).then_some(expr)
}

/// Normalizes an FTS `snippet()` result into a context string: `None` for an
/// absent or whitespace-only snippet, otherwise the trimmed text.
fn clean_snippet(snippet: Option<String>) -> Option<String> {
    snippet
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventKind, EventSubject, NewEvent, NewIndividual, NewPlace};

    /// Renaming a place must reindex every person with an event there. This lives
    /// here (not the integration suite) because it needs a raw `UPDATE places` —
    /// there is no public rename-place API (places are created only via events).
    #[test]
    fn renaming_a_place_reindexes_everyone_with_an_event_there() {
        let store = Store::open_in_memory().expect("open store");
        let place = store
            .create_place(&NewPlace {
                name: "Oldtown".to_owned(),
                latitude: None,
                longitude: None,
                parent: None,
            })
            .expect("place");
        for _ in 0..2 {
            let who = store
                .create_individual(&NewIndividual::default())
                .expect("person")
                .id;
            store
                .add_event(&NewEvent {
                    subject: EventSubject::Individual(who),
                    kind: EventKind::Birth,
                    date: None,
                    place: Some(place),
                    notes: None,
                })
                .expect("birth");
        }
        assert_eq!(store.search("Oldtown", 50).expect("search").len(), 2);

        store
            .conn()
            .expect("conn")
            .execute("UPDATE places SET name = 'Newtown' WHERE id = ?1", [place])
            .expect("rename place");
        assert_eq!(store.search("Newtown", 50).expect("search").len(), 2);
        assert!(
            store.search("Oldtown", 50).expect("search").is_empty(),
            "the stale place name no longer matches"
        );
    }

    #[test]
    fn match_expr_quotes_and_prefixes_each_term() {
        assert_eq!(
            to_match_expr("Ada Lov").as_deref(),
            Some("\"Ada\"* \"Lov\"*")
        );
    }

    #[test]
    fn match_expr_escapes_embedded_quotes_and_drops_punctuation_only_terms() {
        // A stray quote is doubled (a valid FTS string literal), and a term that
        // is all punctuation is dropped rather than forming a zero-token phrase.
        assert_eq!(
            to_match_expr("a\"b !!! c").as_deref(),
            Some("\"a\"\"b\"* \"c\"*")
        );
    }

    #[test]
    fn match_expr_is_none_for_empty_or_punctuation_only_queries() {
        assert!(to_match_expr("").is_none());
        assert!(to_match_expr("   ").is_none());
        assert!(to_match_expr("*** ???").is_none());
    }
}
