//! `kith family new | add-partner | add-child | show | list | rm | remove-child`.
//!
//! The partner-count cap and the full-slot guard surface as
//! `CoreError::Validation` (exit `4`) — the same sanctioned CLI precondition
//! channel the overwrite guards use. No new core method is needed:
//! setting a partner *is* `update_family`.

use anyhow::Context as _;
use kith_core::prelude::{CoreError, FamilyId, NewFamily, PersonId, Store};

use crate::cli::{
    FamilyAddChildArgs, FamilyAddPartnerArgs, FamilyCommand, FamilyNewArgs, FamilyRemoveChildArgs,
    FamilyRmArgs, FamilyShowArgs, GlobalArgs,
};
use crate::output;
use kith_core::query::FamilyView;

/// Dispatches the `family` subcommand against an open store.
///
/// # Errors
/// Propagates any `Store` failure (and the validation guards) as an
/// `anyhow::Error`.
pub fn run(global: &GlobalArgs, store: &Store, command: &FamilyCommand) -> anyhow::Result<()> {
    match command {
        FamilyCommand::New(args) => new(global, store, args),
        FamilyCommand::AddPartner(args) => add_partner(global, store, args),
        FamilyCommand::AddChild(args) => add_child(global, store, args),
        FamilyCommand::Show(args) => show(global, store, args),
        FamilyCommand::List => list(global, store),
        FamilyCommand::Rm(args) => rm(global, store, args),
        FamilyCommand::RemoveChild(args) => remove_child(global, store, args),
    }
}

fn new(global: &GlobalArgs, store: &Store, args: &FamilyNewArgs) -> anyhow::Result<()> {
    if args.partner.len() > 2 {
        return Err(CoreError::Validation(format!(
            "a family has at most two partners, but {} were given",
            args.partner.len()
        ))
        .into());
    }
    let mut partners = args.partner.iter().map(|id| PersonId::new(*id));
    let draft = NewFamily {
        partner1: partners.next(),
        partner2: partners.next(),
        union_type: args.union_type,
        notes: args.notes.clone(),
    };
    let family = store.create_family(&draft).context("creating family")?;
    output::report_record(global, "Added", &family, &format!("family {}", family.id));
    Ok(())
}

fn add_partner(
    global: &GlobalArgs,
    store: &Store,
    args: &FamilyAddPartnerArgs,
) -> anyhow::Result<()> {
    let fid = FamilyId::new(args.family_id);
    let pid = PersonId::new(args.person_id);
    let mut family = store
        .get_family(fid)
        .with_context(|| format!("loading family {fid}"))?;
    match (family.partner1, family.partner2) {
        (None, _) => family.partner1 = Some(pid),
        (Some(_), None) => family.partner2 = Some(pid),
        (Some(_), Some(_)) => {
            return Err(
                CoreError::Validation(format!("family {fid} already has two partners")).into(),
            );
        }
    }
    store
        .update_family(&family)
        .with_context(|| format!("adding partner {pid} to family {fid}"))?;
    output::report_record(
        global,
        "Updated",
        &family,
        &format!("family {fid}: added partner {pid}"),
    );
    Ok(())
}

fn add_child(global: &GlobalArgs, store: &Store, args: &FamilyAddChildArgs) -> anyhow::Result<()> {
    let fid = FamilyId::new(args.family_id);
    let pid = PersonId::new(args.person_id);
    let order = match args.order {
        Some(n) => n,
        None => {
            let count = store
                .list_children(fid)
                .with_context(|| format!("counting children of family {fid}"))?
                .len();
            i64::try_from(count).unwrap_or(i64::MAX) // append; never panics
        }
    };
    let link = store
        .add_child(fid, pid, args.relation, order)
        .with_context(|| format!("adding child {pid} to family {fid}"))?;
    output::report_record(
        global,
        "Added",
        &link,
        &format!("child {pid} to family {fid}"),
    );
    Ok(())
}

fn show(global: &GlobalArgs, store: &Store, args: &FamilyShowArgs) -> anyhow::Result<()> {
    let view = FamilyView::load(store, FamilyId::new(args.id))
        .with_context(|| format!("loading family {}", args.id))?;
    output::render_family_view(global, &view);
    Ok(())
}

fn list(global: &GlobalArgs, store: &Store) -> anyhow::Result<()> {
    let families = store.list_families().context("listing families")?;
    output::render_families(global, &families);
    Ok(())
}

fn rm(global: &GlobalArgs, store: &Store, args: &FamilyRmArgs) -> anyhow::Result<()> {
    let id = FamilyId::new(args.id);
    store
        .delete_family(id)
        .with_context(|| format!("removing family {id}"))?;
    output::report_removed(global, "family", id.get());
    Ok(())
}

fn remove_child(
    global: &GlobalArgs,
    store: &Store,
    args: &FamilyRemoveChildArgs,
) -> anyhow::Result<()> {
    let fid = FamilyId::new(args.family_id);
    let cid = PersonId::new(args.child_id);
    store
        .remove_child(fid, cid)
        .with_context(|| format!("removing child {cid} from family {fid}"))?;
    output::report_action(
        global,
        &format!("Removed child {cid} from family {fid}"),
        serde_json::json!({ "removed": "child", "family": fid.get(), "child": cid.get() }),
    );
    Ok(())
}
