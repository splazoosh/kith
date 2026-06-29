//! `kith person add | list | show | edit | rm`.

use anyhow::Context as _;
use kith_core::prelude::{EventKind, EventSubject, NewEvent, NewIndividual, PersonId, Store};

use crate::cli::{
    GlobalArgs, PersonAddArgs, PersonCommand, PersonEditArgs, PersonListArgs, PersonRmArgs,
    PersonShowArgs,
};
use crate::output;
use kith_core::query::PersonView;

/// Dispatches the `person` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure as an `anyhow::Error`.
pub fn run(global: &GlobalArgs, store: &Store, command: &PersonCommand) -> anyhow::Result<()> {
    match command {
        PersonCommand::Add(args) => add(global, store, args),
        PersonCommand::List(args) => list(global, store, args),
        PersonCommand::Show(args) => show(global, store, args),
        PersonCommand::Edit(args) => edit(global, store, args),
        PersonCommand::Rm(args) => rm(global, store, args),
    }
}

fn add(global: &GlobalArgs, store: &Store, args: &PersonAddArgs) -> anyhow::Result<()> {
    let draft = NewIndividual {
        given_name: args.given.clone(),
        surname: args.surname.clone(),
        name_prefix: args.prefix.clone(),
        name_suffix: args.suffix.clone(),
        nickname: args.nickname.clone(),
        sex: args.sex,
        living: args.living,
        notes: args.notes.clone(),
    };
    let person = store
        .create_individual(&draft)
        .context("adding individual")?;

    // Convenience birth/death events. The dates were validated at clap
    // time, so the only failure left here is DB/IO — best-effort,
    // non-atomic, documented. `person add` still reports the Individual;
    // the events are visible via `person show`.
    for (label, date, kind) in [
        ("birth", args.birth, EventKind::Birth),
        ("death", args.death, EventKind::Death),
    ] {
        let Some(date) = date else { continue };
        store
            .add_event(&NewEvent {
                subject: EventSubject::Individual(person.id),
                kind,
                date: Some(date),
                place: None,
                notes: None,
            })
            .with_context(|| format!("adding {label} event to individual {}", person.id))?;
    }

    output::report_record(global, "Added", &person, &output::person_summary(&person));
    Ok(())
}

fn list(global: &GlobalArgs, store: &Store, args: &PersonListArgs) -> anyhow::Result<()> {
    let mut people = store.list_individuals().context("listing individuals")?;
    if let Some(needle) = &args.surname {
        let needle = needle.to_lowercase();
        people.retain(|p| {
            p.surname
                .as_deref()
                .is_some_and(|s| s.to_lowercase().contains(&needle))
        });
    }
    output::render_individuals(global, &people);
    Ok(())
}

fn show(global: &GlobalArgs, store: &Store, args: &PersonShowArgs) -> anyhow::Result<()> {
    let view = PersonView::load(store, PersonId::new(args.id))
        .with_context(|| format!("loading individual {}", args.id))?;
    output::render_person_view(global, &view);
    Ok(())
}

fn edit(global: &GlobalArgs, store: &Store, args: &PersonEditArgs) -> anyhow::Result<()> {
    let id = PersonId::new(args.id);
    let mut ind = store
        .get_individual(id)
        .with_context(|| format!("loading individual {id} to edit"))?;

    // Overlay only the flags that were provided.
    if let Some(v) = &args.given {
        ind.given_name = Some(v.clone());
    }
    if let Some(v) = &args.surname {
        ind.surname = Some(v.clone());
    }
    if let Some(v) = &args.prefix {
        ind.name_prefix = Some(v.clone());
    }
    if let Some(v) = &args.suffix {
        ind.name_suffix = Some(v.clone());
    }
    if let Some(v) = &args.nickname {
        ind.nickname = Some(v.clone());
    }
    if let Some(v) = args.sex {
        ind.sex = v;
    }
    if let Some(v) = args.living {
        ind.living = v;
    }
    if let Some(v) = &args.notes {
        ind.notes = Some(v.clone());
    }

    store
        .update_individual(&ind)
        .with_context(|| format!("updating individual {id}"))?;
    output::report_record(global, "Updated", &ind, &format!("individual {id}"));
    Ok(())
}

fn rm(global: &GlobalArgs, store: &Store, args: &PersonRmArgs) -> anyhow::Result<()> {
    let id = PersonId::new(args.id);
    store
        .delete_individual(id)
        .with_context(|| format!("removing individual {id}"))?;
    output::report_removed(global, "individual", id.get());
    Ok(())
}
