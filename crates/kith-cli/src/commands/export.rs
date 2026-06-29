//! `kith export html` — render a chart to a single self-contained `.html` file.
//!
//! A thin noun over [`compute_layout`] + [`kith_core::render::html`] + a guarded
//! write. **No rendering/redaction/layout logic lives here** — redaction is applied
//! inside `render::html`, so `--include-living` is the only opt-out and
//! both the CLI and the GUI share one path.

use anyhow::Context as _;
use kith_core::prelude::{
    CoreError, HtmlExportOptions, PersonId, Store, compute_layout, media_root_for,
};
use kith_core::render;

use crate::cli::{ExportCommand, ExportGedcomArgs, ExportHtmlArgs, GlobalArgs};
use crate::commands::db::guard_overwrite; // one shared `--force` overwrite policy
use crate::context::resolve_db_path;
use crate::output;

/// Dispatches the `export` subcommand against an open store.
///
/// # Errors
/// Propagates layout failures (missing root → `NotFound` → 3; over-budget
/// `--generations` → `Validation` → 4), the overwrite guard (`Validation` → 4),
/// and the write (`Io` → 5).
pub fn run(global: &GlobalArgs, store: &Store, command: &ExportCommand) -> anyhow::Result<()> {
    match command {
        ExportCommand::Html(args) => html(global, store, args),
        ExportCommand::Gedcom(args) => gedcom(global, store, args),
    }
}

/// Computes the layout, renders the self-contained HTML, and writes it to
/// `args.out` behind the overwrite guard.
fn html(global: &GlobalArgs, store: &Store, args: &ExportHtmlArgs) -> anyhow::Result<()> {
    // 1. Compute the layout (probes the root first; rejects over-budget generations).
    let model = compute_layout(store, PersonId::new(args.root), args.mode, args.generations)
        .with_context(|| format!("computing the {:?} layout for {}", args.mode, args.root))?;

    // 2. Render the self-contained HTML (infallible; redaction applied inside).
    //    `HtmlExportOptions` is `#[non_exhaustive]`, so build it from `default()`
    //    and set the public fields (no struct literal from outside the core crate).
    let mut options = HtmlExportOptions::default();
    options.theme = args.theme;
    options.include_living = args.include_living;
    // `options.title` stays `None` → the renderer derives a title from focus + mode.

    // 2a. Portraits (opt-in): resolve each embeddable portrait id to a base64
    //     `data:` URL over the media folder beside the DB. The resolver runs
    //     only over ids that survive redaction (`export_portrait_ids`), so a
    //     living person's bytes are never read; `render::html` stays pure.
    if args.portraits {
        let media_root = media_root_for(&resolve_db_path(global)?);
        let ids = render::export_portrait_ids(&model, args.include_living);
        options.portrait_urls = store
            .portrait_data_urls(&media_root, &ids)
            .context("resolving portrait images for the export")?;
        options.portraits = true;
    }

    let doc = render::html(&model, &options);

    // 3. Guard the destination, then write. The io::Error MUST be carried as
    //    CoreError::Io or `exit::code_for` maps the failure to 1, not 5.
    guard_overwrite(&args.out, args.force, "export destination")?;
    std::fs::write(&args.out, doc)
        .map_err(CoreError::Io)
        .with_context(|| format!("writing the export to {}", args.out.display()))?;

    output::report_export(global, &args.out, args.root, args.mode);
    Ok(())
}

/// Serializes the whole database to GEDCOM 5.5.1 and writes it to `args.out`
/// behind the overwrite guard. Whole-tree, deterministic, un-redacted — no
/// chart args, no `--include-living` (there is nothing to redact). One core call;
/// no GEDCOM tag logic lives here.
///
/// # Errors
/// Propagates the export read failure, the overwrite guard (`Validation` → 4), and
/// the write (`Io` → 5).
fn gedcom(global: &GlobalArgs, store: &Store, args: &ExportGedcomArgs) -> anyhow::Result<()> {
    // Whole-DB, deterministic, un-redacted. One core call — no tag logic here.
    let doc = kith_core::gedcom::export(store).context("exporting the database to GEDCOM")?;

    // Guard the destination, then write. The io::Error MUST be carried as
    // CoreError::Io or `exit::code_for` maps the failure to 1, not 5.
    guard_overwrite(&args.out, args.force, "export destination")?;
    std::fs::write(&args.out, doc)
        .map_err(CoreError::Io)
        .with_context(|| format!("writing the export to {}", args.out.display()))?;

    output::report_export_gedcom(global, &args.out);
    Ok(())
}
