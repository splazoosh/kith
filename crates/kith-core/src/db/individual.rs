//! CRUD over the `individuals` table.

use rusqlite::{OptionalExtension, params};

use crate::error::{CoreError, Result};
use crate::model::{Individual, NewIndividual, PersonId, Sex};

use super::Store;
use super::now_timestamp;

const COLUMNS: &str =
    "id, given_name, surname, name_prefix, name_suffix, nickname, sex, living, notes";

/// Maps a full `individuals` row (its [`COLUMNS`], by name — so a prefixed
/// `SELECT i.id, …` works too) to an [`Individual`]. A `NULL` `sex` coerces to
/// [`Sex::Unknown`] (the column is nullable; the model field is not).
/// `pub(super)` so [`search`](super::search) can reuse it.
pub(super) fn row_to_individual(row: &rusqlite::Row<'_>) -> rusqlite::Result<Individual> {
    Ok(Individual {
        id: row.get("id")?,
        given_name: row.get("given_name")?,
        surname: row.get("surname")?,
        name_prefix: row.get("name_prefix")?,
        name_suffix: row.get("name_suffix")?,
        nickname: row.get("nickname")?,
        sex: row.get::<_, Option<Sex>>("sex")?.unwrap_or(Sex::Unknown),
        living: row.get("living")?,
        notes: row.get("notes")?,
    })
}

impl Store {
    /// Inserts a new individual and returns it with its assigned id.
    ///
    /// # Errors
    /// Returns [`CoreError`] if a connection cannot be acquired or the insert fails.
    pub fn create_individual(&self, draft: &NewIndividual) -> Result<Individual> {
        let conn = self.conn()?;
        Self::create_individual_in(&conn, draft)
    }

    /// The `create_individual` INSERT body, callable on any connection (a pooled
    /// connection or a transaction). The transactional twin behind the public
    /// auto-commit method — see [`Store::transaction`](super::Store::transaction).
    ///
    /// # Errors
    /// Returns [`CoreError`] if the insert fails.
    pub(crate) fn create_individual_in(
        conn: &rusqlite::Connection,
        draft: &NewIndividual,
    ) -> Result<Individual> {
        let now = now_timestamp();
        conn.execute(
            "INSERT INTO individuals
                (sex, given_name, surname, name_prefix, name_suffix, nickname,
                 living, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                draft.sex,
                draft.given_name,
                draft.surname,
                draft.name_prefix,
                draft.name_suffix,
                draft.nickname,
                draft.living,
                draft.notes,
                now,
            ],
        )?;
        // Build the result from the draft + the new id rather than re-reading:
        // the in-memory pool has a single connection, and `get_individual`
        // would block on it while this one is still borrowed.
        Ok(Individual {
            id: PersonId::new(conn.last_insert_rowid()),
            given_name: draft.given_name.clone(),
            surname: draft.surname.clone(),
            name_prefix: draft.name_prefix.clone(),
            name_suffix: draft.name_suffix.clone(),
            nickname: draft.nickname.clone(),
            sex: draft.sex,
            living: draft.living,
            notes: draft.notes.clone(),
        })
    }

    /// Fetches an individual by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such individual exists, or another
    /// [`CoreError`] on a connection/query failure.
    pub fn get_individual(&self, id: PersonId) -> Result<Individual> {
        let conn = self.conn()?;
        Self::get_individual_on(&conn, id)
    }

    /// The [`get_individual`](Self::get_individual) read on a caller-supplied
    /// connection, using a **cached** prepared statement. The read twin behind
    /// the public method (the `_in` write-twin pattern, for reads): a relationship
    /// walk holds one connection and routes every per-person lookup through this,
    /// so the statement is compiled once and no per-read pool checkout happens.
    /// Same SQL, same row → byte-identical output.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such individual exists, or another
    /// [`CoreError`] on a query failure.
    pub(crate) fn get_individual_on(
        conn: &rusqlite::Connection,
        id: PersonId,
    ) -> Result<Individual> {
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {COLUMNS} FROM individuals WHERE id = ?1"))?;
        stmt.query_row([id], row_to_individual)
            .optional()?
            .ok_or(CoreError::NotFound {
                entity: PersonId::ENTITY,
                id: id.get(),
            })
    }

    /// Updates every mutable column of an existing individual and bumps
    /// `updated_at`. `created_at` is preserved.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `ind.id`, or another
    /// [`CoreError`] on failure.
    pub fn update_individual(&self, ind: &Individual) -> Result<()> {
        let conn = self.conn()?;
        let now = now_timestamp();
        let n = conn.execute(
            "UPDATE individuals SET
                sex=?1, given_name=?2, surname=?3, name_prefix=?4, name_suffix=?5,
                nickname=?6, living=?7, notes=?8, updated_at=?9
             WHERE id=?10",
            params![
                ind.sex,
                ind.given_name,
                ind.surname,
                ind.name_prefix,
                ind.name_suffix,
                ind.nickname,
                ind.living,
                ind.notes,
                now,
                ind.id,
            ],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: PersonId::ENTITY,
                id: ind.id.get(),
            });
        }
        Ok(())
    }

    /// Deletes an individual. The schema cascades: their `names`,
    /// `family_children` memberships, and individual events are removed, and any
    /// `families.partner{1,2}_id` referencing them is set to `NULL`.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no row has `id`, or another
    /// [`CoreError`] on failure.
    pub fn delete_individual(&self, id: PersonId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM individuals WHERE id=?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: PersonId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }

    /// Lists all individuals, ordered by surname then given name.
    ///
    /// # Errors
    /// Returns [`CoreError`] on a connection/query failure.
    pub fn list_individuals(&self) -> Result<Vec<Individual>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM individuals ORDER BY surname, given_name, id"
        ))?;
        let out = stmt
            .query_map([], row_to_individual)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_get_update_delete_round_trip() {
        let store = Store::open_in_memory().expect("open store");

        let created = store
            .create_individual(&NewIndividual {
                given_name: Some("Ada".to_owned()),
                surname: Some("Lovelace".to_owned()),
                sex: Sex::Female,
                ..Default::default()
            })
            .expect("create");
        assert_eq!(created.surname.as_deref(), Some("Lovelace"));

        let fetched = store.get_individual(created.id).expect("get");
        assert_eq!(fetched, created);

        let updated = Individual {
            surname: Some("Byron".to_owned()),
            ..fetched
        };
        store.update_individual(&updated).expect("update");
        assert_eq!(store.get_individual(created.id).expect("re-get"), updated);

        store.delete_individual(created.id).expect("delete");
        assert!(matches!(
            store.get_individual(created.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn update_and_delete_of_missing_row_are_not_found() {
        let store = Store::open_in_memory().expect("open store");
        let ghost = Individual {
            id: PersonId::new(404),
            given_name: None,
            surname: None,
            name_prefix: None,
            name_suffix: None,
            nickname: None,
            sex: Sex::Unknown,
            living: true,
            notes: None,
        };
        assert!(matches!(
            store.update_individual(&ghost),
            Err(CoreError::NotFound { .. })
        ));
        assert!(matches!(
            store.delete_individual(PersonId::new(404)),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn list_is_ordered_by_surname_then_given_name() {
        let store = Store::open_in_memory().expect("open store");
        for (given, surname) in [("John", "Doe"), ("Jane", "Doe"), ("Zara", "Adams")] {
            store
                .create_individual(&NewIndividual {
                    given_name: Some(given.to_owned()),
                    surname: Some(surname.to_owned()),
                    ..Default::default()
                })
                .expect("create");
        }
        let names: Vec<_> = store
            .list_individuals()
            .expect("list")
            .into_iter()
            .map(|i| {
                (
                    i.surname.unwrap_or_default(),
                    i.given_name.unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                ("Adams".to_owned(), "Zara".to_owned()),
                ("Doe".to_owned(), "Jane".to_owned()),
                ("Doe".to_owned(), "John".to_owned()),
            ]
        );
    }
}
