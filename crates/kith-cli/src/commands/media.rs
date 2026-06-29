//! `kith media add | list | set-primary | rm` — a subject's photos/media.
//!
//! Files are copied into the media folder beside the DB (`<db-stem>.media/`, via
//! [`media_root_for`]). All media logic — the copy, the single-primary invariant,
//! the mime check — lives in `kith_core::db`; this is a thin noun over it.

use anyhow::Context as _;
use kith_core::prelude::{MediaId, Store, media_root_for};

use crate::cli::{
    GlobalArgs, MediaAddArgs, MediaCommand, MediaListArgs, MediaRmArgs, MediaSetPrimaryArgs,
};
use crate::context::resolve_db_path;
use crate::output;

/// Dispatches the `media` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error` (a bad mime / missing
/// source / missing id surfaces as the mapped exit code).
pub fn run(global: &GlobalArgs, store: &Store, command: &MediaCommand) -> anyhow::Result<()> {
    match command {
        MediaCommand::Add(args) => add(global, store, args),
        MediaCommand::List(args) => list(global, store, args),
        MediaCommand::SetPrimary(args) => set_primary(global, store, args),
        MediaCommand::Rm(args) => rm(global, store, args),
    }
}

fn add(global: &GlobalArgs, store: &Store, args: &MediaAddArgs) -> anyhow::Result<()> {
    let media_root = media_root_for(&resolve_db_path(global)?);
    let media = store
        .import_media(&media_root, &args.file, args.subject, args.primary)
        .with_context(|| format!("importing {}", args.file.display()))?;
    output::report_record(
        global,
        "Added",
        &media,
        &format!("media {} ({})", media.id, media.path),
    );
    Ok(())
}

fn list(global: &GlobalArgs, store: &Store, args: &MediaListArgs) -> anyhow::Result<()> {
    let items = store
        .list_media_for(args.subject)
        .context("listing media")?;
    output::render_media(global, &items);
    Ok(())
}

fn set_primary(
    global: &GlobalArgs,
    store: &Store,
    args: &MediaSetPrimaryArgs,
) -> anyhow::Result<()> {
    let media = MediaId::new(args.media);
    store
        .set_primary(media, args.subject)
        .with_context(|| format!("setting media {media} as primary"))?;
    output::report_action(
        global,
        &format!("Set media {media} as primary"),
        serde_json::json!({ "primary": media.get() }),
    );
    Ok(())
}

fn rm(global: &GlobalArgs, store: &Store, args: &MediaRmArgs) -> anyhow::Result<()> {
    let media = MediaId::new(args.media);
    store
        .delete_media(media)
        .with_context(|| format!("deleting media {media}"))?;
    output::report_removed(global, "media", media.get());
    Ok(())
}
