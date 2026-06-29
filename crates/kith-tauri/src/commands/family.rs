//! Family commands: list / get / create / update / delete + add-partner /
//! add-child / remove-child.
//!
//! The write-composition the CLI keeps in its command fns is replicated thin
//! here (an accepted stopgap): `add_partner` fills the first empty slot and
//! errors `validation` when full; `add_child` defaults the sort order to an
//! append. `create` takes a `NewFamily` whose two partner slots make `> 2`
//! structurally impossible. No new core method is introduced.

use kith_core::prelude::{
    ChildLink, ChildRelation, CoreError, DeleteTarget, Family, FamilyId, NewFamily, PersonId,
};
use kith_core::query::FamilyView;
use tauri::State;

use crate::error::CommandError;
use crate::state::AppState;

/// Lists every family.
///
/// # Errors
/// [`CommandError`] if no database is open or the read fails.
pub async fn family_list_impl(state: &AppState) -> Result<Vec<Family>, CommandError> {
    state.with_store(|store| store.list_families()).await
}

/// Loads a family with partners and children resolved.
///
/// # Errors
/// [`CommandError`] (`not_found` / `database` / `io`).
pub async fn family_get_impl(state: &AppState, id: FamilyId) -> Result<FamilyView, CommandError> {
    state
        .with_store(move |store| FamilyView::load(&store, id))
        .await
}

/// Creates a family.
///
/// # Errors
/// [`CommandError`] if the insert fails (e.g. a non-existent partner FK).
pub async fn family_create_impl(
    state: &AppState,
    draft: NewFamily,
) -> Result<Family, CommandError> {
    state
        .with_store(move |store| store.create_family(&draft))
        .await
}

/// Updates a family from the full edited record (set/clear partners, union type,
/// notes).
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if the row is gone.
pub async fn family_update_impl(state: &AppState, record: Family) -> Result<Family, CommandError> {
    state
        .with_store(move |store| {
            store.update_family(&record)?;
            Ok(record)
        })
        .await
}

/// Deletes a family (cascades memberships + family events; partners untouched),
/// snapshotting the cascade onto the session undo stack first.
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if the row is gone.
pub async fn family_delete_impl(state: &AppState, id: FamilyId) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| store.delete_undoable(DeleteTarget::Family(id)))
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// Adds a partner to the first empty slot.
///
/// # Errors
/// [`CommandError`] with `kind: validation` if the family already has two
/// partners; `not_found` if the family is gone.
pub async fn family_add_partner_impl(
    state: &AppState,
    fid: FamilyId,
    pid: PersonId,
) -> Result<Family, CommandError> {
    state
        .with_store(move |store| {
            let mut family = store.get_family(fid)?;
            match (family.partner1, family.partner2) {
                (None, _) => family.partner1 = Some(pid),
                (Some(_), None) => family.partner2 = Some(pid),
                (Some(_), Some(_)) => {
                    return Err(CoreError::Validation(format!(
                        "family {fid} already has two partners"
                    )));
                }
            }
            store.update_family(&family)?;
            Ok(family)
        })
        .await
}

/// Adds a child, defaulting the sort order to an append.
///
/// # Errors
/// [`CommandError`] if the insert fails (e.g. a duplicate membership or a
/// non-existent family/child).
pub async fn family_add_child_impl(
    state: &AppState,
    fid: FamilyId,
    pid: PersonId,
    relation: ChildRelation,
    order: Option<i64>,
) -> Result<ChildLink, CommandError> {
    state
        .with_store(move |store| {
            let order = match order {
                Some(n) => n,
                None => i64::try_from(store.list_children(fid)?.len()).unwrap_or(i64::MAX),
            };
            store.add_child(fid, pid, relation, order)
        })
        .await
}

/// Removes a child membership, snapshotting it onto the session undo stack first.
///
/// # Errors
/// [`CommandError`] with `kind: not_found` if no such membership exists.
pub async fn family_remove_child_impl(
    state: &AppState,
    fid: FamilyId,
    pid: PersonId,
) -> Result<(), CommandError> {
    let deletion = state
        .with_store(move |store| {
            store.delete_undoable(DeleteTarget::Child {
                family: fid,
                child: pid,
            })
        })
        .await?;
    state.push_undo(deletion);
    Ok(())
}

/// IPC: list families.
///
/// # Errors
/// See [`family_list_impl`].
#[tauri::command]
pub async fn family_list(state: State<'_, AppState>) -> Result<Vec<Family>, CommandError> {
    family_list_impl(state.inner()).await
}

/// IPC: load a family view by id.
///
/// # Errors
/// See [`family_get_impl`].
#[tauri::command]
pub async fn family_get(state: State<'_, AppState>, id: i64) -> Result<FamilyView, CommandError> {
    family_get_impl(state.inner(), FamilyId::new(id)).await
}

/// IPC: create a family.
///
/// # Errors
/// See [`family_create_impl`].
#[tauri::command]
pub async fn family_create(
    state: State<'_, AppState>,
    draft: NewFamily,
) -> Result<Family, CommandError> {
    family_create_impl(state.inner(), draft).await
}

/// IPC: update a family from the full record.
///
/// # Errors
/// See [`family_update_impl`].
#[tauri::command]
pub async fn family_update(
    state: State<'_, AppState>,
    record: Family,
) -> Result<Family, CommandError> {
    family_update_impl(state.inner(), record).await
}

/// IPC: delete a family by id.
///
/// # Errors
/// See [`family_delete_impl`].
#[tauri::command]
pub async fn family_delete(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    family_delete_impl(state.inner(), FamilyId::new(id)).await
}

/// IPC: add a partner to a family.
///
/// # Errors
/// See [`family_add_partner_impl`].
#[tauri::command]
pub async fn family_add_partner(
    state: State<'_, AppState>,
    family_id: i64,
    person_id: i64,
) -> Result<Family, CommandError> {
    family_add_partner_impl(
        state.inner(),
        FamilyId::new(family_id),
        PersonId::new(person_id),
    )
    .await
}

/// IPC: add a child to a family (order defaults to append).
///
/// # Errors
/// See [`family_add_child_impl`].
#[tauri::command]
pub async fn family_add_child(
    state: State<'_, AppState>,
    family_id: i64,
    person_id: i64,
    relation: ChildRelation,
    order: Option<i64>,
) -> Result<ChildLink, CommandError> {
    family_add_child_impl(
        state.inner(),
        FamilyId::new(family_id),
        PersonId::new(person_id),
        relation,
        order,
    )
    .await
}

/// IPC: remove a child membership.
///
/// # Errors
/// See [`family_remove_child_impl`].
#[tauri::command]
pub async fn family_remove_child(
    state: State<'_, AppState>,
    family_id: i64,
    person_id: i64,
) -> Result<(), CommandError> {
    family_remove_child_impl(
        state.inner(),
        FamilyId::new(family_id),
        PersonId::new(person_id),
    )
    .await
}
