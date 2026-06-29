//! `kith name add | list | rm` — an individual's alternate names.

use anyhow::Context as _;
use kith_core::prelude::{NameId, NewName, PersonId, Store};

use crate::cli::{GlobalArgs, NameAddArgs, NameCommand, NameListArgs, NameRmArgs};
use crate::output;

/// Dispatches the `name` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error`.
pub fn run(global: &GlobalArgs, store: &Store, command: &NameCommand) -> anyhow::Result<()> {
    match command {
        NameCommand::Add(args) => add(global, store, args),
        NameCommand::List(args) => list(global, store, args),
        NameCommand::Rm(args) => rm(global, store, args),
    }
}

fn add(global: &GlobalArgs, store: &Store, args: &NameAddArgs) -> anyhow::Result<()> {
    let draft = NewName {
        individual_id: PersonId::new(args.person_id),
        kind: args.kind,
        given_name: args.given.clone(),
        surname: args.surname.clone(),
        name_prefix: args.prefix.clone(),
        name_suffix: args.suffix.clone(),
        sort_order: args.order,
    };
    let name = store
        .add_name(&draft)
        .with_context(|| format!("adding name to individual {}", args.person_id))?;
    output::report_record(
        global,
        "Added",
        &name,
        &format!("name {} to individual {}", name.id, name.individual_id),
    );
    Ok(())
}

fn list(global: &GlobalArgs, store: &Store, args: &NameListArgs) -> anyhow::Result<()> {
    let names = store
        .list_names(PersonId::new(args.person_id))
        .with_context(|| format!("listing names for individual {}", args.person_id))?;
    output::render_names(global, &names);
    Ok(())
}

fn rm(global: &GlobalArgs, store: &Store, args: &NameRmArgs) -> anyhow::Result<()> {
    let id = NameId::new(args.id);
    store
        .remove_name(id)
        .with_context(|| format!("removing name {id}"))?;
    output::report_removed(global, "name", id.get());
    Ok(())
}
