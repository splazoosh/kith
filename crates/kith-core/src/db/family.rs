//! CRUD over the `families` table and its `family_children` membership rows.

use rusqlite::{OptionalExtension, params};

use crate::error::{CoreError, Result};
use crate::model::{ChildLink, ChildRelation, Family, FamilyId, NewFamily, PersonId, UnionType};

use super::Store;
use super::now_timestamp;

const COLUMNS: &str = "id, partner1_id, partner2_id, union_type, notes";

/// Maps a full `families` row (selected with [`COLUMNS`]) to a [`Family`].
/// A `NULL` `union_type` coerces to [`UnionType::Unknown`] (the column is
/// nullable; the model field is not).
fn row_to_family(row: &rusqlite::Row<'_>) -> rusqlite::Result<Family> {
    Ok(Family {
        id: row.get("id")?,
        partner1: row.get("partner1_id")?,
        partner2: row.get("partner2_id")?,
        union_type: row
            .get::<_, Option<UnionType>>("union_type")?
            .unwrap_or(UnionType::Unknown),
        notes: row.get("notes")?,
    })
}

/// Maps a `family_children` row to a [`ChildLink`].
fn row_to_child_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChildLink> {
    Ok(ChildLink {
        family_id: row.get("family_id")?,
        child_id: row.get("child_id")?,
        relation: row.get("relation")?,
        sort_order: row.get("sort_order")?,
    })
}

impl Store {
    /// Inserts a new family and returns it with its assigned id.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert
    /// fails (e.g. a foreign-key violation on a non-existent partner).
    pub fn create_family(&self, draft: &NewFamily) -> Result<Family> {
        let conn = self.conn()?;
        Self::create_family_in(&conn, draft)
    }

    /// The `create_family` INSERT body, callable on any connection (a pooled
    /// connection or a transaction) — the transactional twin behind the public
    /// auto-commit method.
    ///
    /// # Errors
    /// Returns [`CoreError`] if the insert fails (e.g. a foreign-key violation on
    /// a non-existent partner).
    pub(crate) fn create_family_in(
        conn: &rusqlite::Connection,
        draft: &NewFamily,
    ) -> Result<Family> {
        let now = now_timestamp();
        conn.execute(
            "INSERT INTO families
                (partner1_id, partner2_id, union_type, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                draft.partner1,
                draft.partner2,
                draft.union_type,
                draft.notes,
                now
            ],
        )?;
        Ok(Family {
            id: FamilyId::new(conn.last_insert_rowid()),
            partner1: draft.partner1,
            partner2: draft.partner2,
            union_type: draft.union_type,
            notes: draft.notes.clone(),
        })
    }

    /// Fetches a family by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such family exists, or another
    /// [`CoreError`] on a connection/query failure.
    pub fn get_family(&self, id: FamilyId) -> Result<Family> {
        let conn = self.conn()?;
        Self::get_family_on(&conn, id)
    }

    /// The [`get_family`](Self::get_family) read on a caller-supplied connection,
    /// using a **cached** prepared statement — the read twin a relationship walk
    /// routes its per-family lookups through. Same SQL → identical row.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such family exists, or another
    /// [`CoreError`] on a query failure.
    pub(crate) fn get_family_on(conn: &rusqlite::Connection, id: FamilyId) -> Result<Family> {
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {COLUMNS} FROM families WHERE id = ?1"))?;
        stmt.query_row([id], row_to_family)
            .optional()?
            .ok_or(CoreError::NotFound {
                entity: FamilyId::ENTITY,
                id: id.get(),
            })
    }

    /// Updates a family's partners, union type, and notes, and bumps
    /// `updated_at`. Setting a partner to `Some`/`None` here *is* the
    /// set/clear-partner operation; no separate method is needed.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `fam.id`, or another
    /// [`CoreError`] on failure.
    pub fn update_family(&self, fam: &Family) -> Result<()> {
        let conn = self.conn()?;
        let now = now_timestamp();
        let n = conn.execute(
            "UPDATE families SET
                partner1_id=?1, partner2_id=?2, union_type=?3, notes=?4, updated_at=?5
             WHERE id=?6",
            params![
                fam.partner1,
                fam.partner2,
                fam.union_type,
                fam.notes,
                now,
                fam.id
            ],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: FamilyId::ENTITY,
                id: fam.id.get(),
            });
        }
        Ok(())
    }

    /// Deletes a family. The schema cascades: its `family_children` rows and
    /// family events are removed. Partner individuals are *not* affected.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `id`, or another
    /// [`CoreError`] on failure.
    pub fn delete_family(&self, id: FamilyId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM families WHERE id=?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: FamilyId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }

    /// Lists all families, ordered by id.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn list_families(&self) -> Result<Vec<Family>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM families ORDER BY id"))?;
        let out = stmt
            .query_map([], row_to_family)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(out)
    }

    /// Adds `child` to `family` with the given relation and birth order,
    /// returning the membership link.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert
    /// fails (e.g. a duplicate membership or a non-existent family/child).
    pub fn add_child(
        &self,
        family: FamilyId,
        child: PersonId,
        relation: ChildRelation,
        sort_order: i64,
    ) -> Result<ChildLink> {
        let conn = self.conn()?;
        Self::add_child_in(&conn, family, child, relation, sort_order)
    }

    /// The `add_child` INSERT body, callable on any connection (a pooled
    /// connection or a transaction) — the transactional twin behind the public
    /// auto-commit method.
    ///
    /// # Errors
    /// Returns [`CoreError`] if the insert fails (e.g. a duplicate membership or a
    /// non-existent family/child).
    pub(crate) fn add_child_in(
        conn: &rusqlite::Connection,
        family: FamilyId,
        child: PersonId,
        relation: ChildRelation,
        sort_order: i64,
    ) -> Result<ChildLink> {
        conn.execute(
            "INSERT INTO family_children (family_id, child_id, relation, sort_order)
             VALUES (?1, ?2, ?3, ?4)",
            params![family, child, relation, sort_order],
        )?;
        Ok(ChildLink {
            family_id: family,
            child_id: child,
            relation,
            sort_order,
        })
    }

    /// Removes `child` from `family`.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] (labelled with the family id) if no such
    /// membership exists, or another [`CoreError`] on failure.
    pub fn remove_child(&self, family: FamilyId, child: PersonId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute(
            "DELETE FROM family_children WHERE family_id=?1 AND child_id=?2",
            params![family, child],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: FamilyId::ENTITY,
                id: family.get(),
            });
        }
        Ok(())
    }

    /// Lists a family's children, ordered by birth order then child id.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn list_children(&self, family: FamilyId) -> Result<Vec<ChildLink>> {
        let conn = self.conn()?;
        Self::list_children_on(&conn, family)
    }

    /// The [`list_children`](Self::list_children) read on a caller-supplied
    /// connection, using a **cached** prepared statement — the read twin a
    /// relationship walk routes its per-family child lookups through.
    /// Same SQL and ordering → identical rows.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a query failure.
    pub(crate) fn list_children_on(
        conn: &rusqlite::Connection,
        family: FamilyId,
    ) -> Result<Vec<ChildLink>> {
        let mut stmt = conn.prepare_cached(
            "SELECT family_id, child_id, relation, sort_order FROM family_children
             WHERE family_id = ?1
             ORDER BY sort_order, child_id",
        )?;
        let out = stmt
            .query_map([family], row_to_child_link)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(out)
    }

    /// The families in which `person` is a partner (the FAMS edge), ordered by
    /// id. The query layer's downward (descendant) step.
    ///
    /// Hits `idx_families_partner1` / `idx_families_partner2`. A person appears
    /// at most once per family even if recorded in both partner slots (the row
    /// is selected once by the `OR`).
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn families_of_partner(&self, person: PersonId) -> Result<Vec<Family>> {
        let conn = self.conn()?;
        Self::families_of_partner_on(&conn, person)
    }

    /// The [`families_of_partner`](Self::families_of_partner) read on a
    /// caller-supplied connection, using a **cached** prepared statement — the
    /// read twin a relationship walk routes its per-person FAMS lookups through.
    /// Same SQL and ordering → identical rows.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a query failure.
    pub(crate) fn families_of_partner_on(
        conn: &rusqlite::Connection,
        person: PersonId,
    ) -> Result<Vec<Family>> {
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM families
             WHERE partner1_id = ?1 OR partner2_id = ?1
             ORDER BY id"
        ))?;
        let out = stmt
            .query_map([person], row_to_family)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(out)
    }

    /// The ids of families in which `person` is a child (the FAMC edge), ordered
    /// by family id. The query layer's upward (ancestor) step.
    ///
    /// A `Vec` because the model permits multiple memberships (e.g. a birth
    /// *and* an adoptive family). Hits `idx_famchildren_child`.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn families_of_child(&self, person: PersonId) -> Result<Vec<FamilyId>> {
        let conn = self.conn()?;
        Self::families_of_child_on(&conn, person)
    }

    /// The [`families_of_child`](Self::families_of_child) read on a
    /// caller-supplied connection, using a **cached** prepared statement — the
    /// read twin a relationship walk routes its per-person FAMC lookups through.
    /// Same SQL and ordering → identical ids.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a query failure.
    pub(crate) fn families_of_child_on(
        conn: &rusqlite::Connection,
        person: PersonId,
    ) -> Result<Vec<FamilyId>> {
        let mut stmt = conn.prepare_cached(
            "SELECT family_id FROM family_children WHERE child_id = ?1 ORDER BY family_id",
        )?;
        let out = stmt
            .query_map([person], |row| row.get::<_, FamilyId>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NewIndividual;

    fn person(store: &Store) -> PersonId {
        store
            .create_individual(&NewIndividual::default())
            .expect("create individual")
            .id
    }

    #[test]
    fn create_get_update_delete_round_trip() {
        let store = Store::open_in_memory().expect("open store");
        let p1 = person(&store);
        let p2 = person(&store);

        let created = store
            .create_family(&NewFamily {
                partner1: Some(p1),
                partner2: Some(p2),
                union_type: UnionType::Marriage,
                ..Default::default()
            })
            .expect("create");
        assert_eq!(store.get_family(created.id).expect("get"), created);

        let updated = Family {
            partner2: None,
            ..created
        };
        store.update_family(&updated).expect("update");
        assert_eq!(store.get_family(created.id).expect("re-get"), updated);

        store.delete_family(created.id).expect("delete");
        assert!(matches!(
            store.get_family(created.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn null_union_type_reads_back_as_unknown() {
        let store = Store::open_in_memory().expect("open store");
        let family = store.create_family(&NewFamily::default()).expect("create");
        assert_eq!(
            store.get_family(family.id).expect("get").union_type,
            UnionType::Unknown
        );
    }

    #[test]
    fn children_are_listed_in_birth_order() {
        let store = Store::open_in_memory().expect("open store");
        let family = store
            .create_family(&NewFamily::default())
            .expect("create family");
        let first = person(&store);
        let second = person(&store);
        store
            .add_child(family.id, second, ChildRelation::Birth, 1)
            .expect("add second");
        store
            .add_child(family.id, first, ChildRelation::Adopted, 0)
            .expect("add first");

        let kids = store.list_children(family.id).expect("list");
        assert_eq!(kids.len(), 2);
        assert_eq!(kids[0].child_id, first);
        assert_eq!(kids[0].relation, ChildRelation::Adopted);
        assert_eq!(kids[1].child_id, second);

        store.remove_child(family.id, first).expect("remove");
        assert_eq!(store.list_children(family.id).expect("list").len(), 1);
        assert!(matches!(
            store.remove_child(family.id, first),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn relationship_reads_return_memberships_in_stable_id_order() {
        // Arrange — a person who is both a partner (in two unions) and a child.
        let store = Store::open_in_memory().expect("open store");
        let subject = person(&store);
        let spouse_a = person(&store);
        let spouse_b = person(&store);
        let parent = person(&store);

        // `subject` is a partner of two families (created in id order).
        let fam_a = store
            .create_family(&NewFamily {
                partner1: Some(subject),
                partner2: Some(spouse_a),
                ..Default::default()
            })
            .expect("family a");
        let fam_b = store
            .create_family(&NewFamily {
                partner1: Some(spouse_b),
                partner2: Some(subject), // subject in the *second* slot this time
                ..Default::default()
            })
            .expect("family b");
        // `subject` is a child of a third family.
        let birth_family = store
            .create_family(&NewFamily {
                partner1: Some(parent),
                ..Default::default()
            })
            .expect("birth family");
        store
            .add_child(birth_family.id, subject, ChildRelation::Birth, 0)
            .expect("add child");

        // Act / Assert — partner memberships, ascending id, matched on either slot.
        let partner_ids: Vec<_> = store
            .families_of_partner(subject)
            .expect("partner reads")
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(partner_ids, vec![fam_a.id, fam_b.id]);

        // Child memberships, ascending family id.
        assert_eq!(
            store.families_of_child(subject).expect("child reads"),
            vec![birth_family.id]
        );

        // A person in neither role gets empty vectors, never an error.
        let stranger = person(&store);
        assert!(
            store
                .families_of_partner(stranger)
                .expect("partner reads")
                .is_empty()
        );
        assert!(
            store
                .families_of_child(stranger)
                .expect("child reads")
                .is_empty()
        );
    }
}
