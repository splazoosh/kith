//! CRUD over the `names` table (an individual's alternate names).

use rusqlite::params;

use crate::error::{CoreError, Result};
use crate::model::{Name, NameId, NewName, PersonId};

use super::Store;

const COLUMNS: &str =
    "id, individual_id, kind, given_name, surname, name_prefix, name_suffix, sort_order";

/// Maps a full `names` row (selected with [`COLUMNS`]) to a [`Name`].
fn row_to_name(row: &rusqlite::Row<'_>) -> rusqlite::Result<Name> {
    Ok(Name {
        id: row.get("id")?,
        individual_id: row.get("individual_id")?,
        kind: row.get("kind")?,
        given_name: row.get("given_name")?,
        surname: row.get("surname")?,
        name_prefix: row.get("name_prefix")?,
        name_suffix: row.get("name_suffix")?,
        sort_order: row.get("sort_order")?,
    })
}

impl Store {
    /// Attaches a new alternate name to an individual and returns it with its
    /// assigned id.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert
    /// fails (e.g. a foreign-key violation on a non-existent individual).
    pub fn add_name(&self, draft: &NewName) -> Result<Name> {
        let conn = self.conn()?;
        Self::add_name_in(&conn, draft)
    }

    /// The `add_name` INSERT body, callable on any connection (a pooled
    /// connection or a transaction) — the transactional twin behind the public
    /// auto-commit method.
    ///
    /// # Errors
    /// Returns [`CoreError`] if the insert fails (e.g. a foreign-key violation on
    /// a non-existent individual).
    pub(crate) fn add_name_in(conn: &rusqlite::Connection, draft: &NewName) -> Result<Name> {
        conn.execute(
            "INSERT INTO names
                (individual_id, kind, given_name, surname, name_prefix, name_suffix, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                draft.individual_id,
                draft.kind,
                draft.given_name,
                draft.surname,
                draft.name_prefix,
                draft.name_suffix,
                draft.sort_order,
            ],
        )?;
        Ok(Name {
            id: NameId::new(conn.last_insert_rowid()),
            individual_id: draft.individual_id,
            kind: draft.kind,
            given_name: draft.given_name.clone(),
            surname: draft.surname.clone(),
            name_prefix: draft.name_prefix.clone(),
            name_suffix: draft.name_suffix.clone(),
            sort_order: draft.sort_order,
        })
    }

    /// Lists an individual's alternate names, ordered by sort order then id.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn list_names(&self, individual: PersonId) -> Result<Vec<Name>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM names WHERE individual_id = ?1 ORDER BY sort_order, id"
        ))?;
        let out = stmt
            .query_map([individual], row_to_name)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(out)
    }

    /// Removes an alternate name by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `id`, or another
    /// [`CoreError`] on failure.
    pub fn remove_name(&self, id: NameId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM names WHERE id=?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: NameId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NameKind, NewIndividual};

    #[test]
    fn add_list_remove_round_trip() {
        let store = Store::open_in_memory().expect("open store");
        let person = store
            .create_individual(&NewIndividual::default())
            .expect("create individual")
            .id;

        let maiden = store
            .add_name(&NewName {
                individual_id: person,
                kind: NameKind::Birth,
                given_name: Some("Jane".to_owned()),
                surname: Some("Smith".to_owned()),
                name_prefix: None,
                name_suffix: None,
                sort_order: 1,
            })
            .expect("add birth name");
        store
            .add_name(&NewName {
                individual_id: person,
                kind: NameKind::Married,
                given_name: Some("Jane".to_owned()),
                surname: Some("Doe".to_owned()),
                name_prefix: None,
                name_suffix: None,
                sort_order: 0,
            })
            .expect("add married name");

        let names = store.list_names(person).expect("list");
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].kind, NameKind::Married, "sort_order 0 comes first");
        assert_eq!(names[1].kind, NameKind::Birth);

        store.remove_name(maiden.id).expect("remove");
        assert_eq!(store.list_names(person).expect("list").len(), 1);
        assert!(matches!(
            store.remove_name(maiden.id),
            Err(CoreError::NotFound { .. })
        ));
    }
}
