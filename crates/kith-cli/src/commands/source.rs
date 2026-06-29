//! `kith source add | list | show | edit | rm` and `kith citation add | list | rm`
//! — the evidence layer.
//!
//! All source/citation logic lives in `kith_core::db`; this is a thin noun over
//! it. Deleting a source cascades its citations (the schema's `ON DELETE CASCADE`);
//! `source rm` notes that.

use anyhow::Context as _;
use kith_core::prelude::{CitationId, NewCitation, NewSource, SourceId, Store};
use kith_core::query::SourceView;

use crate::cli::{
    CitationAddArgs, CitationCommand, CitationListArgs, CitationRmArgs, GlobalArgs, SourceAddArgs,
    SourceCommand, SourceEditArgs, SourceRmArgs, SourceShowArgs,
};
use crate::output;

/// Dispatches the `source` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error`.
pub fn run_source(
    global: &GlobalArgs,
    store: &Store,
    command: &SourceCommand,
) -> anyhow::Result<()> {
    match command {
        SourceCommand::Add(args) => add(global, store, args),
        SourceCommand::List => list(global, store),
        SourceCommand::Show(args) => show(global, store, args),
        SourceCommand::Edit(args) => edit(global, store, args),
        SourceCommand::Rm(args) => rm(global, store, args),
    }
}

/// Dispatches the `citation` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error`.
pub fn run_citation(
    global: &GlobalArgs,
    store: &Store,
    command: &CitationCommand,
) -> anyhow::Result<()> {
    match command {
        CitationCommand::Add(args) => citation_add(global, store, args),
        CitationCommand::List(args) => citation_list(global, store, args),
        CitationCommand::Rm(args) => citation_rm(global, store, args),
    }
}

fn add(global: &GlobalArgs, store: &Store, args: &SourceAddArgs) -> anyhow::Result<()> {
    let source = store
        .create_source(&NewSource {
            title: args.title.clone(),
            author: args.author.clone(),
            publication: args.publication.clone(),
            repository: args.repository.clone(),
            notes: args.notes.clone(),
        })
        .context("adding source")?;
    output::report_record(
        global,
        "Added",
        &source,
        &format!("source {}: {}", source.id, source.title),
    );
    Ok(())
}

fn list(global: &GlobalArgs, store: &Store) -> anyhow::Result<()> {
    let sources = store.list_sources().context("listing sources")?;
    output::render_sources(global, &sources);
    Ok(())
}

fn show(global: &GlobalArgs, store: &Store, args: &SourceShowArgs) -> anyhow::Result<()> {
    let view = SourceView::load(store, SourceId::new(args.id))
        .with_context(|| format!("loading source {}", args.id))?;
    output::render_source_view(global, &view);
    Ok(())
}

fn edit(global: &GlobalArgs, store: &Store, args: &SourceEditArgs) -> anyhow::Result<()> {
    let id = SourceId::new(args.id);
    let mut existing = store
        .get_source(id)
        .with_context(|| format!("loading source {id} to edit"))?;

    // Overlay only the flags that were provided.
    if let Some(v) = &args.title {
        existing.title = v.clone();
    }
    if let Some(v) = &args.author {
        existing.author = Some(v.clone());
    }
    if let Some(v) = &args.publication {
        existing.publication = Some(v.clone());
    }
    if let Some(v) = &args.repository {
        existing.repository = Some(v.clone());
    }
    if let Some(v) = &args.notes {
        existing.notes = Some(v.clone());
    }

    let updated = store
        .update_source(
            id,
            &NewSource {
                title: existing.title,
                author: existing.author,
                publication: existing.publication,
                repository: existing.repository,
                notes: existing.notes,
            },
        )
        .with_context(|| format!("updating source {id}"))?;
    output::report_record(global, "Updated", &updated, &format!("source {id}"));
    Ok(())
}

fn rm(global: &GlobalArgs, store: &Store, args: &SourceRmArgs) -> anyhow::Result<()> {
    let id = SourceId::new(args.id);
    store
        .delete_source(id)
        .with_context(|| format!("removing source {id}"))?;
    output::report_action(
        global,
        &format!("Removed source {id} and its citations"),
        serde_json::json!({ "removed": "source", "id": id.get() }),
    );
    Ok(())
}

fn citation_add(global: &GlobalArgs, store: &Store, args: &CitationAddArgs) -> anyhow::Result<()> {
    let citation = store
        .add_citation(&NewCitation {
            source: SourceId::new(args.source),
            subject: args.subject,
            page: args.page.clone(),
            detail: args.detail.clone(),
            confidence: args.confidence,
        })
        .context("adding citation")?;
    output::report_record(
        global,
        "Added",
        &citation,
        &format!("citation {} citing source {}", citation.id, citation.source),
    );
    Ok(())
}

fn citation_list(
    global: &GlobalArgs,
    store: &Store,
    args: &CitationListArgs,
) -> anyhow::Result<()> {
    let items = store
        .citations_for(args.subject)
        .context("listing citations")?;
    output::render_citations(global, &items);
    Ok(())
}

fn citation_rm(global: &GlobalArgs, store: &Store, args: &CitationRmArgs) -> anyhow::Result<()> {
    let id = CitationId::new(args.id);
    store
        .delete_citation(id)
        .with_context(|| format!("deleting citation {id}"))?;
    output::report_removed(global, "citation", id.get());
    Ok(())
}
