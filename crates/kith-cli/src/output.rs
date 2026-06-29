//! Rendering: human-readable tables / confirmations on stdout, machine JSON on
//! stdout, and errors on stderr. The `--json` flag and `--quiet`/`-v` come from
//! [`GlobalArgs`]; data always goes to stdout, diagnostics to stderr.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use kith_core::prelude::{
    ChartMode, CitationItem, CoreError, EventSubject, Family, ImportSummary, Individual, MediaItem,
    Name, NodeRef, PersonNode, RelEdge, RelativeGraph, SearchHit, Source,
};
use serde::Serialize;

use crate::cli::GlobalArgs;
use kith_core::query::{EventView, FamilyView, PersonView, SourceView};

/// Prints `value` as pretty JSON on stdout.
fn print_json<T: Serialize>(value: &T) {
    // serde_json only fails here on a non-string map key or a custom
    // Serialize that errors — neither occurs for our record types.
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: failed to serialize output: {e}"),
    }
}

/// Reports a successful `init`: a confirmation line (human) or a small object
/// (`--json`). Suppressed in human mode by `--quiet`.
pub fn report_init(global: &GlobalArgs, path: &Path, schema_version: i64) {
    if global.json {
        print_json(&serde_json::json!({
            "path": path.display().to_string(),
            "schema_version": schema_version,
        }));
    } else if !global.quiet {
        println!(
            "Initialized kith database at {} (schema v{schema_version})",
            path.display()
        );
    }
}

/// Reports a `dev seed`: the focal id (`--json`) or a confirmation line. Dev-only,
/// so the helper is gated behind the same feature as the noun.
#[cfg(feature = "dev")]
pub fn report_dev_seed(global: &GlobalArgs, focal: kith_core::prelude::PersonId, individuals: u32) {
    if global.json {
        print_json(&serde_json::json!({
            "focal": focal.get(),
            "individuals": individuals,
        }));
    } else if !global.quiet {
        println!(
            "Seeded ~{individuals} synthetic individuals; focal person id = {}",
            focal.get()
        );
    }
}

/// Reports a created/updated record: the full record as JSON (round-trippable),
/// or "`{verb} {summary}`" on stdout in human mode (suppressed by `--quiet`).
pub fn report_record<T: Serialize>(global: &GlobalArgs, verb: &str, record: &T, summary: &str) {
    if global.json {
        print_json(record);
    } else if !global.quiet {
        println!("{verb} {summary}");
    }
}

/// Reports a single-id removal: `{"removed": <noun>, "id": <id>}` (JSON) or
/// "Removed {noun} {id}" (human, suppressed by `--quiet`).
pub fn report_removed(global: &GlobalArgs, noun: &str, id: i64) {
    if global.json {
        print_json(&serde_json::json!({ "removed": noun, "id": id }));
    } else if !global.quiet {
        println!("Removed {noun} {id}");
    }
}

/// Reports a completed action with a caller-supplied JSON body and human line —
/// for confirmations whose shape doesn't fit [`report_record`]/[`report_removed`]
/// (e.g. removing a child membership, which names two ids).
pub fn report_action(global: &GlobalArgs, human: &str, json: serde_json::Value) {
    if global.json {
        print_json(&json);
    } else if !global.quiet {
        println!("{human}");
    }
}

/// A short, human summary of an individual ("individual {id}: {name}"), used to
/// label [`report_record`] for `person add`/`edit`.
pub(crate) fn person_summary(p: &Individual) -> String {
    format!("individual {}: {}", p.id, display_name(p))
}

/// Reports a completed `db` maintenance action: a confirmation line (human,
/// suppressed by `--quiet`) or a small JSON object `{"action","path"}`.
pub fn report_db_action(global: &GlobalArgs, action: &str, path: &Path) {
    if global.json {
        print_json(&serde_json::json!({
            "action": action,
            "path": path.display().to_string(),
        }));
    } else if !global.quiet {
        let verb = match action {
            "backup" => "Backed up to",
            "restore" => "Restored",
            "vacuum" => "Vacuumed",
            _ => "Done:",
        };
        println!("{verb} {}", path.display());
    }
}

/// Reports a completed export: a confirmation line (human, suppressed by `--quiet`)
/// or a small JSON object `{"action":"export","path","root","mode"}` (the export
/// *data* is the written file, not stdout — this is only the confirmation).
pub fn report_export(global: &GlobalArgs, out: &Path, root: i64, mode: ChartMode) {
    if global.json {
        print_json(&serde_json::json!({
            "action": "export",
            "path": out.display().to_string(),
            "root": root,
            "mode": mode, // ChartMode: Serialize → "Descendants" etc.
        }));
    } else if !global.quiet {
        println!("Exported {mode:?} chart of {root} to {}", out.display());
    }
}

/// Reports a completed GEDCOM export: a confirmation line (human, suppressed by
/// `--quiet`) or a small JSON object `{"action":"export","format":"gedcom","path"}`.
pub fn report_export_gedcom(global: &GlobalArgs, out: &Path) {
    if global.json {
        print_json(&serde_json::json!({
            "action": "export",
            "format": "gedcom",
            "path": out.display().to_string(),
        }));
    } else if !global.quiet {
        println!("Exported GEDCOM to {}", out.display());
    }
}

/// Reports a completed GEDCOM import: the [`ImportSummary`] as JSON (round-trippable),
/// or a human counts line plus, when non-empty, a skipped-tags line (the records
/// the importer defers — surfaced so a lossy import never reads as complete).
pub fn report_import(global: &GlobalArgs, summary: &ImportSummary) {
    if global.json {
        print_json(summary); // ImportSummary: Serialize
    } else if !global.quiet {
        println!(
            "Imported {} individuals, {} families, {} events, {} names, {} places",
            summary.individuals, summary.families, summary.events, summary.names, summary.places
        );
        if !summary.skipped_tags.is_empty() {
            let skipped = summary
                .skipped_tags
                .iter()
                .map(|(tag, n)| format!("{tag}×{n}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("Skipped unsupported records: {skipped}");
        }
    }
}

/// Renders a list of individuals: an aligned table (human) or a JSON array.
pub fn render_individuals(global: &GlobalArgs, people: &[Individual]) {
    if global.json {
        print_json(&people);
    } else if people.is_empty() {
        println!("No individuals.");
    } else {
        print!("{}", individual_table(people));
    }
}

/// Renders ranked search hits: a table of `ID · NAME · WHY-MATCHED` (human) or a
/// JSON array of [`SearchHit`] (round-trippable). A no-match prints an empty
/// table header / `[]` — search is a read, so this is success (exit 0).
pub fn render_search_hits(global: &GlobalArgs, hits: &[SearchHit]) {
    if global.json {
        print_json(&hits);
    } else if hits.is_empty() {
        println!("No matches.");
    } else {
        let rows: Vec<Vec<String>> = hits
            .iter()
            .map(|h| {
                vec![
                    h.individual.id.to_string(),
                    display_name(&h.individual),
                    h.context.clone().unwrap_or_default(),
                ]
            })
            .collect();
        print!("{}", table(&["ID", "NAME", "WHY-MATCHED"], &rows));
    }
}

/// Renders one person: the composite view as JSON, or a labelled detail block.
pub fn render_person_view(global: &GlobalArgs, view: &PersonView) {
    if global.json {
        print_json(view);
        return;
    }
    let p = &view.individual;
    println!("Individual {}", p.id);
    println!("  Name:    {}", display_name(p));
    println!("  Sex:     {}", p.sex);
    println!("  Living:  {}", if p.living { "yes" } else { "no" });
    if let Some(notes) = &p.notes {
        println!("  Notes:   {notes}");
    }
    if !view.names.is_empty() {
        println!("  Names:");
        for n in &view.names {
            println!("    [{}] {}", n.kind, name_label(n));
        }
    }
    if !view.events.is_empty() {
        println!("  Events:");
        for e in &view.events {
            let date = e
                .date
                .as_ref()
                .map_or_else(|| "—".to_owned(), ToString::to_string);
            println!("    {:<10} {}", e.kind.to_string(), date);
        }
    }
    print_id_list("  Partner in:", &view.partner_in);
    print_id_list("  Child in:  ", &view.child_in);
}

/// Renders one family: the composite view as JSON, or a labelled detail block.
pub fn render_family_view(global: &GlobalArgs, view: &FamilyView) {
    if global.json {
        print_json(view);
        return;
    }
    println!("Family {}", view.family.id);
    println!("  Type:     {}", view.family.union_type);
    println!("  Partner1: {}", partner_label(view.partner1.as_ref()));
    println!("  Partner2: {}", partner_label(view.partner2.as_ref()));
    if let Some(notes) = &view.family.notes {
        println!("  Notes:    {notes}");
    }
    if view.children.is_empty() {
        println!("  Children: (none)");
    } else {
        println!("  Children:");
        for c in &view.children {
            println!(
                "    {}. {} [{}]",
                c.link.sort_order,
                display_name(&c.individual),
                c.link.relation
            );
        }
    }
    if !view.events.is_empty() {
        println!("  Events:");
        for e in &view.events {
            let date = e
                .date
                .as_ref()
                .map_or_else(|| "—".to_owned(), ToString::to_string);
            println!("    {:<10} {}", e.kind.to_string(), date);
        }
    }
}

/// Renders one event: the composite view as JSON, or a labelled detail block.
pub fn render_event_view(global: &GlobalArgs, view: &EventView) {
    if global.json {
        print_json(view);
        return;
    }
    let e = &view.event;
    println!("Event {}", e.id);
    println!("  Subject: {}", subject_label(e.subject));
    println!("  Kind:    {}", e.kind);
    let date = e
        .date
        .as_ref()
        .map_or_else(|| "—".to_owned(), ToString::to_string);
    println!("  Date:    {date}");
    let place = view
        .place
        .as_ref()
        .map_or_else(|| "—".to_owned(), |p| format!("{}: {}", p.id, p.name));
    println!("  Place:   {place}");
    if let Some(notes) = &e.notes {
        println!("  Notes:   {notes}");
    }
}

/// Renders a relationship graph: the serializable graph as JSON (`--json`,
/// round-trippable into [`RelativeGraph`]), or an indented human tree walked from
/// the focus in the graph's own deterministic order. **No positions are
/// surfaced** — the walk has no geometry.
pub fn render_relative_graph(global: &GlobalArgs, graph: &RelativeGraph) {
    if global.json {
        print_json(graph);
        return;
    }
    let persons: HashMap<NodeRef, &PersonNode> =
        graph.persons.iter().map(|p| (p.node, p)).collect();
    let union_partners: HashMap<NodeRef, &[NodeRef]> = graph
        .unions
        .iter()
        .map(|u| (u.node, u.partners.as_slice()))
        .collect();
    let Some(focus) = graph.persons.iter().find(|p| p.focal) else {
        println!("(empty)");
        return;
    };

    // Build adjacency by following the deterministically-ordered edge vector;
    // the maps are for O(1) lookup only and are never iterated to emit output.
    let mut out = String::new();
    match graph.mode {
        ChartMode::Ancestors => {
            let mut child_unions: HashMap<NodeRef, Vec<NodeRef>> = HashMap::new();
            for edge in &graph.edges {
                if let RelEdge::Descent { union, child } = edge {
                    child_unions.entry(*child).or_default().push(*union);
                }
            }
            ancestor_tree(
                &mut out,
                focus.node,
                &persons,
                &child_unions,
                &union_partners,
                0,
            );
        }
        // Descendants (the only other mode the CLI produces).
        _ => {
            let mut partner_unions: HashMap<NodeRef, Vec<NodeRef>> = HashMap::new();
            let mut union_children: HashMap<NodeRef, Vec<NodeRef>> = HashMap::new();
            for edge in &graph.edges {
                match edge {
                    RelEdge::Partner { person, union } => {
                        partner_unions.entry(*person).or_default().push(*union);
                    }
                    RelEdge::Descent { union, child } => {
                        union_children.entry(*union).or_default().push(*child);
                    }
                }
            }
            descendant_tree(
                &mut out,
                focus.node,
                &persons,
                &partner_unions,
                &union_children,
                &union_partners,
                0,
            );
        }
    }
    print!("{out}");
}

/// Recursively renders a person and their ancestors (parents above, indented).
fn ancestor_tree(
    out: &mut String,
    node: NodeRef,
    persons: &HashMap<NodeRef, &PersonNode>,
    child_unions: &HashMap<NodeRef, Vec<NodeRef>>,
    union_partners: &HashMap<NodeRef, &[NodeRef]>,
    depth: usize,
) {
    let Some(person) = persons.get(&node) else {
        return;
    };
    let _ = writeln!(out, "{}{}", "  ".repeat(depth), person_line(person));
    for union in child_unions.get(&node).into_iter().flatten() {
        for parent in union_partners.get(union).copied().into_iter().flatten() {
            ancestor_tree(
                out,
                *parent,
                persons,
                child_unions,
                union_partners,
                depth + 1,
            );
        }
    }
}

/// Recursively renders a person and their descendants, grouping children under a
/// `+ spouse` line per union (indented).
fn descendant_tree(
    out: &mut String,
    node: NodeRef,
    persons: &HashMap<NodeRef, &PersonNode>,
    partner_unions: &HashMap<NodeRef, Vec<NodeRef>>,
    union_children: &HashMap<NodeRef, Vec<NodeRef>>,
    union_partners: &HashMap<NodeRef, &[NodeRef]>,
    depth: usize,
) {
    let Some(person) = persons.get(&node) else {
        return;
    };
    let _ = writeln!(out, "{}{}", "  ".repeat(depth), person_line(person));
    for union in partner_unions.get(&node).into_iter().flatten() {
        // The other partner of this union (if recorded) is shown as a spouse.
        for partner in union_partners.get(union).copied().into_iter().flatten() {
            if *partner != node {
                if let Some(spouse) = persons.get(partner) {
                    let _ = writeln!(out, "{}+ {}", "  ".repeat(depth + 1), person_line(spouse));
                }
            }
        }
        for child in union_children.get(union).into_iter().flatten() {
            descendant_tree(
                out,
                *child,
                persons,
                partner_unions,
                union_children,
                union_partners,
                depth + 2,
            );
        }
    }
}

/// "Given Surname (1850–1915)" for a graph person — the name plus a lifespan
/// built from the walk's vitals (formatting two integers, not date math).
fn person_line(p: &PersonNode) -> String {
    match lifespan(p) {
        Some(span) => format!("{} ({span})", p.display_name),
        None => p.display_name.clone(),
    }
}

/// "1850–1915", "1850–", or "–1915"; `None` when neither year is known.
fn lifespan(p: &PersonNode) -> Option<String> {
    match (p.birth_year, p.death_year) {
        (None, None) => None,
        (birth, death) => {
            let fmt = |y: Option<i32>| y.map(|y| y.to_string()).unwrap_or_default();
            Some(format!("{}\u{2013}{}", fmt(birth), fmt(death)))
        }
    }
}

/// "person:<id>" / "family:<id>" for an event subject (mirrors the input form
/// `--subject` accepts).
fn subject_label(subject: EventSubject) -> String {
    match subject {
        EventSubject::Individual(id) => format!("person:{id}"),
        EventSubject::Family(id) => format!("family:{id}"),
    }
}

/// Renders a families list: aligned table (human) or a JSON array of [`Family`].
pub fn render_families(global: &GlobalArgs, families: &[Family]) {
    if global.json {
        print_json(&families);
    } else if families.is_empty() {
        println!("No families.");
    } else {
        let rows: Vec<Vec<String>> = families
            .iter()
            .map(|f| {
                vec![
                    f.id.to_string(),
                    f.partner1.map_or_else(|| "—".to_owned(), |p| p.to_string()),
                    f.partner2.map_or_else(|| "—".to_owned(), |p| p.to_string()),
                    f.union_type.to_string(),
                ]
            })
            .collect();
        print!("{}", table(&["ID", "PARTNER1", "PARTNER2", "TYPE"], &rows));
    }
}

/// Renders an individual's alternate names: aligned table or a JSON array of [`Name`].
pub fn render_names(global: &GlobalArgs, names: &[Name]) {
    if global.json {
        print_json(&names);
    } else if names.is_empty() {
        println!("No alternate names.");
    } else {
        let rows: Vec<Vec<String>> = names
            .iter()
            .map(|n| {
                vec![
                    n.id.to_string(),
                    n.kind.to_string(),
                    name_label(n),
                    n.sort_order.to_string(),
                ]
            })
            .collect();
        print!("{}", table(&["ID", "KIND", "NAME", "ORDER"], &rows));
    }
}

/// Renders a subject's media as a table (ID / PRIMARY / MIME / PATH / CAPTION),
/// or the raw `MediaItem[]` as JSON.
pub fn render_media(global: &GlobalArgs, items: &[MediaItem]) {
    if global.json {
        print_json(&items);
    } else if items.is_empty() {
        println!("No media.");
    } else {
        let rows: Vec<Vec<String>> = items
            .iter()
            .map(|i| {
                vec![
                    i.media.id.to_string(),
                    if i.is_primary { "yes" } else { "" }.to_owned(),
                    i.media.mime.clone().unwrap_or_default(),
                    i.media.path.clone(),
                    i.media.caption.clone().unwrap_or_default(),
                ]
            })
            .collect();
        print!(
            "{}",
            table(&["ID", "PRIMARY", "MIME", "PATH", "CAPTION"], &rows)
        );
    }
}

/// Renders a list of sources: an aligned table (human) or a JSON array.
pub fn render_sources(global: &GlobalArgs, sources: &[Source]) {
    if global.json {
        print_json(&sources);
    } else if sources.is_empty() {
        println!("No sources.");
    } else {
        let rows: Vec<Vec<String>> = sources
            .iter()
            .map(|s| {
                vec![
                    s.id.to_string(),
                    s.title.clone(),
                    s.author.clone().unwrap_or_default(),
                    s.repository.clone().unwrap_or_default(),
                ]
            })
            .collect();
        print!("{}", table(&["ID", "TITLE", "AUTHOR", "REPOSITORY"], &rows));
    }
}

/// Renders one source: the [`SourceView`] as JSON, or a labelled block listing the
/// facts it supports.
pub fn render_source_view(global: &GlobalArgs, view: &SourceView) {
    if global.json {
        print_json(view);
        return;
    }
    let s = &view.source;
    println!("Source {}", s.id);
    println!("  Title:       {}", s.title);
    if let Some(author) = &s.author {
        println!("  Author:      {author}");
    }
    if let Some(publication) = &s.publication {
        println!("  Publication: {publication}");
    }
    if let Some(repository) = &s.repository {
        println!("  Repository:  {repository}");
    }
    if let Some(notes) = &s.notes {
        println!("  Notes:       {notes}");
    }
    if view.citations.is_empty() {
        println!("  Cited by:    (none)");
    } else {
        println!("  Cited by:");
        for c in &view.citations {
            let page = c
                .page
                .as_deref()
                .map_or(String::new(), |p| format!(" ({p})"));
            println!(
                "    citation {} → {}{page}",
                c.id,
                citation_subject_label(c.subject)
            );
        }
    }
}

/// Renders a subject's citations as a table (ID / SOURCE / TITLE / PAGE /
/// CONFIDENCE), or the raw `CitationItem[]` as JSON.
pub fn render_citations(global: &GlobalArgs, items: &[CitationItem]) {
    if global.json {
        print_json(&items);
    } else if items.is_empty() {
        println!("No citations.");
    } else {
        let rows: Vec<Vec<String>> = items
            .iter()
            .map(|i| {
                vec![
                    i.citation.id.to_string(),
                    i.citation.source.to_string(),
                    i.source.title.clone(),
                    i.citation.page.clone().unwrap_or_default(),
                    i.citation
                        .confidence
                        .map(|c| c.to_string())
                        .unwrap_or_default(),
                ]
            })
            .collect();
        print!(
            "{}",
            table(&["ID", "SOURCE", "TITLE", "PAGE", "CONFIDENCE"], &rows)
        );
    }
}

/// "event:<id>" / "person:<id>" / "family:<id>" for a citation subject (mirrors
/// the `--subject` input form).
fn citation_subject_label(subject: kith_core::prelude::CitationSubject) -> String {
    use kith_core::prelude::CitationSubject;
    match subject {
        CitationSubject::Individual(id) => format!("person:{id}"),
        CitationSubject::Family(id) => format!("family:{id}"),
        CitationSubject::Event(id) => format!("event:{id}"),
    }
}

/// A person's display name: "Given Surname", a single part, or "(unnamed)".
fn display_name(p: &Individual) -> String {
    match (p.given_name.as_deref(), p.surname.as_deref()) {
        (Some(g), Some(s)) => format!("{g} {s}"),
        (Some(g), None) => g.to_owned(),
        (None, Some(s)) => s.to_owned(),
        (None, None) => "(unnamed)".to_owned(),
    }
}

/// "Given Surname" for a [`Name`] row (falls back like [`display_name`]).
fn name_label(n: &Name) -> String {
    match (n.given_name.as_deref(), n.surname.as_deref()) {
        (Some(g), Some(s)) => format!("{g} {s}"),
        (Some(g), None) => g.to_owned(),
        (None, Some(s)) => s.to_owned(),
        (None, None) => "(unnamed)".to_owned(),
    }
}

/// "id: Name" for a resolved partner, or "—" when the slot is empty.
fn partner_label(p: Option<&Individual>) -> String {
    p.map_or_else(
        || "—".to_owned(),
        |i| format!("{}: {}", i.id, display_name(i)),
    )
}

/// Prints a `label` followed by a comma-separated id list, or "(none)".
fn print_id_list<T: std::fmt::Display>(label: &str, ids: &[T]) {
    if ids.is_empty() {
        println!("{label} (none)");
    } else {
        let joined = ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("{label} {joined}");
    }
}

/// Hand-rolled left-aligned table (no table crate). Each column is padded to
/// its widest cell; columns are separated by a two-space gutter; trailing pad is
/// trimmed. ASCII-width alignment is fine for our short id/name/code columns.
fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.len());
        }
    }
    let mut out = String::with_capacity((rows.len() + 1) * widths.iter().sum::<usize>().max(16));
    let header_cells: Vec<String> = headers.iter().map(|h| (*h).to_owned()).collect();
    write_row(&mut out, &header_cells, &widths);
    for row in rows {
        write_row(&mut out, row, &widths);
    }
    out
}

/// Rows for the individuals list table (lean: id / name / sex / living).
fn individual_table(people: &[Individual]) -> String {
    let rows: Vec<Vec<String>> = people
        .iter()
        .map(|p| {
            vec![
                p.id.to_string(),
                display_name(p),
                p.sex.to_string(),
                if p.living { "yes" } else { "no" }.to_owned(),
            ]
        })
        .collect();
    table(&["ID", "NAME", "SEX", "LIVING"], &rows)
}

/// Writes one padded row (two-space gutters), trimming trailing pad.
fn write_row(out: &mut String, cells: &[String], widths: &[usize]) {
    for (i, (cell, width)) in cells.iter().zip(widths).enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        let _ = write!(out, "{cell:<width$}", width = *width);
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out.push('\n');
}

/// Renders an error on **stderr**: a JSON object (`--json`) or a message plus,
/// under `-v`, the source chain. Never panics; never touches stdout.
pub fn render_error(err: &anyhow::Error, global: &GlobalArgs) {
    if global.json {
        print_json_err(&serde_json::json!({
            "error": { "kind": error_kind(err), "message": err.to_string() },
        }));
    } else {
        eprintln!("error: {err}");
        if global.verbose > 0 {
            for cause in err.chain().skip(1) {
                eprintln!("  caused by: {cause}");
            }
        }
    }
}

/// Like [`print_json`] but to stderr (for the `--json` error channel).
fn print_json_err<T: Serialize>(value: &T) {
    if let Ok(s) = serde_json::to_string(value) {
        eprintln!("{s}");
    }
}

/// A stable machine string for an error's category (mirrors
/// [`code_for`](crate::exit::code_for)).
fn error_kind(err: &anyhow::Error) -> &'static str {
    match err.downcast_ref::<CoreError>() {
        Some(CoreError::NotFound { .. }) => "not_found",
        Some(CoreError::Validation(_)) => "validation",
        Some(CoreError::Io(_)) => "io",
        Some(CoreError::Database(_) | CoreError::Pool(_) | CoreError::Migration(_)) => "database",
        _ => "error",
    }
}
