//! CRUD over the `media` table and its `media_links` join, plus the read-side
//! helpers the portrait seam needs.
//!
//! A media object's bytes live in a *media folder* beside the database (a
//! sibling directory). `Store` holds **no path** — the shell derives the
//! media-folder's absolute path and passes it in,
//! so `Store` stays stateless and cheap to clone. Within that folder a file is
//! named `<id>.<ext>` and `media.path` records the **relative** name, keeping
//! the database portable.
//!
//! [`Store::import_media`] is the atomic add: it copies the source bytes in and
//! writes the `media` + `media_links` rows in **one transaction**, so the
//! failure mode to avoid — a `media` row with no file — cannot occur (a rolled
//! back import may leave an orphan file on disk, the harmless direction). The
//! GEDCOM importer takes the path-only seam ([`Store::create_media_in`])
//! instead, recording an `OBJE` `FILE` path verbatim with no copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use crate::error::{CoreError, Result};
use crate::model::{Media, MediaId, MediaItem, MediaSubject, NewMedia, PersonId};
use crate::util::base64;

use super::Store;

const COLUMNS: &str = "id, path, caption, mime";

/// The media folder for a database at `db_path`: a sibling directory named
/// `<db-stem>.media` (`tree.db` → `tree.media`).
///
/// This is the **one** definition of the media-folder location, shared by the
/// CLI and the desktop shell so they never diverge (a mismatch would write a
/// portrait under one name and read it from another). `Store` itself stays
/// path-free — the shell derives this from the open DB path and passes it into
/// the media fns.
#[must_use]
pub fn media_root_for(db_path: &Path) -> PathBuf {
    let mut name = db_path.file_stem().unwrap_or_default().to_os_string();
    name.push(".media");
    db_path.with_file_name(name)
}

/// Maps a recognized image file extension (case-insensitive) to its MIME type,
/// or `None` for an unsupported extension — the import path turns that into a
/// [`CoreError::Validation`] rather than storing a file Kith cannot render.
fn mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// The `media_links` column and raw id for a subject (the "exactly one subject"
/// discipline lives in [`MediaSubject`]; this is the SQL-side projection).
fn subject_column(subject: MediaSubject) -> (&'static str, i64) {
    match subject {
        MediaSubject::Individual(p) => ("individual_id", p.get()),
        MediaSubject::Family(f) => ("family_id", f.get()),
        MediaSubject::Event(e) => ("event_id", e.get()),
    }
}

/// Reconstructs a [`Media`] from a row selected as [`COLUMNS`].
fn media_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<Media> {
    Ok(Media {
        id: row.get("id")?,
        path: row.get("path")?,
        caption: row.get("caption")?,
        mime: row.get("mime")?,
    })
}

impl Store {
    /// Imports `source` as a new media object for `subject`, copying the bytes
    /// into `media_root` and writing the `media` + `media_links` rows in one
    /// transaction. When `is_primary`, any existing primary for the subject is
    /// cleared first, so a subject has at most one portrait.
    ///
    /// The source must be a supported image (jpg/jpeg/png/gif/webp); the stored
    /// file is named `<id>.<ext>` and `media.path` is that relative name.
    ///
    /// # Errors
    /// - [`CoreError::Validation`] if `source` has no extension or an
    ///   unsupported one.
    /// - [`CoreError::Io`] if the source cannot be read or the copy fails (the
    ///   transaction rolls back, so no row is written).
    /// - [`CoreError::Database`] on a SQL failure.
    pub fn import_media(
        &self,
        media_root: &Path,
        source: &Path,
        subject: MediaSubject,
        is_primary: bool,
    ) -> Result<Media> {
        // Validate the source is a supported image *before* touching disk or DB.
        let ext = source.extension().and_then(|e| e.to_str()).ok_or_else(|| {
            CoreError::Validation(format!("media source {source:?} has no file extension"))
        })?;
        let mime = mime_for_extension(ext)
            .ok_or_else(|| {
                CoreError::Validation(format!(
                    "unsupported media type `.{ext}` (expected jpg, jpeg, png, gif, or webp)"
                ))
            })?
            .to_owned();
        let ext = ext.to_ascii_lowercase();
        let bytes = std::fs::read(source)?; // Io on a missing/unreadable source

        self.transaction(|conn| {
            conn.execute("INSERT INTO media (path) VALUES ('')", [])?;
            let id = MediaId::new(conn.last_insert_rowid());
            let filename = format!("{}.{ext}", id.get());
            std::fs::create_dir_all(media_root)?;
            std::fs::write(media_root.join(&filename), &bytes)?;
            conn.execute(
                "UPDATE media SET path = ?1, mime = ?2 WHERE id = ?3",
                params![filename, mime, id],
            )?;
            Self::link_media_in(conn, id, subject, is_primary)?;
            Ok(Media {
                id,
                path: filename,
                caption: None,
                mime: Some(mime),
            })
        })
    }

    /// Inserts a `media` row recording `draft`'s path verbatim, **without** any
    /// file copy. The path-only seam the GEDCOM importer uses for an `OBJE`
    /// `FILE` reference; callable inside a [`Store::transaction`].
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub(crate) fn create_media_in(conn: &rusqlite::Connection, draft: &NewMedia) -> Result<Media> {
        conn.execute(
            "INSERT INTO media (path, caption, mime) VALUES (?1, ?2, ?3)",
            params![draft.path, draft.caption, draft.mime],
        )?;
        Ok(Media {
            id: MediaId::new(conn.last_insert_rowid()),
            path: draft.path.clone(),
            caption: draft.caption.clone(),
            mime: draft.mime.clone(),
        })
    }

    /// Inserts a `media_links` row attaching `media` to `subject`. When
    /// `is_primary`, the subject's prior primary is cleared first (so a subject
    /// has at most one). Callable inside a [`Store::transaction`].
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub(crate) fn link_media_in(
        conn: &rusqlite::Connection,
        media: MediaId,
        subject: MediaSubject,
        is_primary: bool,
    ) -> Result<()> {
        if is_primary {
            Self::clear_primary_in(conn, subject)?;
        }
        conn.execute(
            "INSERT INTO media_links (media_id, individual_id, family_id, event_id, is_primary)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                media,
                subject.individual_id(),
                subject.family_id(),
                subject.event_id(),
                is_primary,
            ],
        )?;
        Ok(())
    }

    /// Clears the `is_primary` flag on every link of `subject` (the single-
    /// primary invariant, enforced in the same transaction as a set).
    fn clear_primary_in(conn: &rusqlite::Connection, subject: MediaSubject) -> Result<()> {
        let (col, id) = subject_column(subject);
        conn.execute(
            &format!("UPDATE media_links SET is_primary = 0 WHERE {col} = ?1 AND is_primary = 1"),
            [id],
        )?;
        Ok(())
    }

    /// Fetches a single media row by id.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such row exists, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn get_media(&self, id: MediaId) -> Result<Media> {
        let conn = self.conn()?;
        conn.query_row(
            &format!("SELECT {COLUMNS} FROM media WHERE id = ?1"),
            [id],
            media_columns,
        )
        .optional()?
        .ok_or(CoreError::NotFound {
            entity: MediaId::ENTITY,
            id: id.get(),
        })
    }

    /// Lists a subject's media as [`MediaItem`]s, primary first then ascending
    /// media id (a stable gallery order).
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub fn list_media_for(&self, subject: MediaSubject) -> Result<Vec<MediaItem>> {
        let conn = self.conn()?;
        let (col, id) = subject_column(subject);
        let mut stmt = conn.prepare(&format!(
            "SELECT m.id, m.path, m.caption, m.mime, l.is_primary
             FROM media m JOIN media_links l ON l.media_id = m.id
             WHERE l.{col} = ?1
             ORDER BY l.is_primary DESC, m.id"
        ))?;
        let rows = stmt
            .query_map([id], |row| {
                Ok(MediaItem {
                    media: media_columns(row)?,
                    is_primary: row.get("is_primary")?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Marks `media` the primary (portrait) for `subject`, clearing any prior
    /// primary on the same subject in one transaction.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if `media` is not linked to `subject`, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn set_primary(&self, media: MediaId, subject: MediaSubject) -> Result<()> {
        self.transaction(|conn| {
            Self::clear_primary_in(conn, subject)?;
            let (col, id) = subject_column(subject);
            let n = conn.execute(
                &format!(
                    "UPDATE media_links SET is_primary = 1 WHERE media_id = ?1 AND {col} = ?2"
                ),
                params![media, id],
            )?;
            if n == 0 {
                return Err(CoreError::NotFound {
                    entity: MediaId::ENTITY,
                    id: media.get(),
                });
            }
            Ok(())
        })
    }

    /// Deletes a media row; its `media_links` rows cascade (the table's
    /// `ON DELETE CASCADE`). The on-disk file is left as-is (`Store` holds no
    /// media-folder path); a sweep of orphaned files is a follow-on.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if no such row exists, or
    /// [`CoreError::Database`] on a SQL failure.
    pub fn delete_media(&self, id: MediaId) -> Result<()> {
        let conn = self.conn()?;
        let n = conn.execute("DELETE FROM media WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(CoreError::NotFound {
                entity: MediaId::ENTITY,
                id: id.get(),
            });
        }
        Ok(())
    }

    /// Reads a media file and returns it as a base64 `data:` URL
    /// (`data:<mime>;base64,<…>`) — the self-contained form the HTML export
    /// embeds. `media_root` is the absolute media-folder path.
    ///
    /// # Errors
    /// Returns [`CoreError::NotFound`] if the row is missing, or
    /// [`CoreError::Io`] if its file cannot be read.
    pub fn read_media_data_url(&self, media_root: &Path, id: MediaId) -> Result<String> {
        let media = self.get_media(id)?;
        let bytes = std::fs::read(media_root.join(&media.path))?;
        let mime = media.mime.as_deref().unwrap_or("application/octet-stream");
        Ok(format!("data:{mime};base64,{}", base64::encode(&bytes)))
    }

    /// Resolves `ids` to base64 `data:` URLs for the HTML export, **skipping**
    /// any id whose row or file is missing (a missing portrait omits the image
    /// rather than failing the whole export). Returns a [`BTreeMap`] so the
    /// caller embeds in deterministic id order.
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure (a missing file is not
    /// an error — it is simply skipped).
    pub fn portrait_data_urls(
        &self,
        media_root: &Path,
        ids: &[MediaId],
    ) -> Result<BTreeMap<MediaId, String>> {
        let mut map = BTreeMap::new();
        for &id in ids {
            let media = match self.get_media(id) {
                Ok(m) => m,
                Err(CoreError::NotFound { .. }) => continue,
                Err(e) => return Err(e),
            };
            if let Ok(bytes) = std::fs::read(media_root.join(&media.path)) {
                let mime = media.mime.as_deref().unwrap_or("application/octet-stream");
                map.insert(id, format!("data:{mime};base64,{}", base64::encode(&bytes)));
            }
        }
        Ok(map)
    }

    /// Resolves `ids` to absolute filesystem path strings under `media_root` —
    /// the input the desktop shell hands to Tauri's `convertFileSrc` so the live
    /// canvas streams a portrait over the asset protocol (no base64-in-IPC).
    /// A missing row is skipped. Returns a [`BTreeMap`] for stable order.
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub fn media_paths(
        &self,
        media_root: &Path,
        ids: &[MediaId],
    ) -> Result<BTreeMap<MediaId, String>> {
        let mut map = BTreeMap::new();
        for &id in ids {
            let media = match self.get_media(id) {
                Ok(m) => m,
                Err(CoreError::NotFound { .. }) => continue,
                Err(e) => return Err(e),
            };
            if let Some(abs) = media_root.join(&media.path).to_str() {
                map.insert(id, abs.to_owned());
            }
        }
        Ok(map)
    }

    /// Every media row, ascending id — the GEDCOM writer's top-level `OBJE`
    /// source (deterministic emission order).
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub(crate) fn list_all_media(&self) -> Result<Vec<Media>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM media ORDER BY id"))?;
        let rows = stmt
            .query_map([], media_columns)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The media ids linked to `subject`, ascending — the GEDCOM writer's `OBJE`
    /// pointer source (deterministic per-subject order).
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub(crate) fn media_ids_for(&self, subject: MediaSubject) -> Result<Vec<MediaId>> {
        let conn = self.conn()?;
        let (col, id) = subject_column(subject);
        let mut stmt = conn.prepare(&format!(
            "SELECT media_id FROM media_links WHERE {col} = ?1 ORDER BY media_id"
        ))?;
        let ids = stmt
            .query_map([id], |row| row.get::<_, MediaId>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }

    /// The media id of a person's primary (portrait) image, if any — their
    /// `media_links` row with `is_primary = 1` (the layout walk reads this per
    /// person, beside [`Store::vital_years`]).
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub fn primary_portrait(&self, person: PersonId) -> Result<Option<MediaId>> {
        let conn = self.conn()?;
        Self::primary_portrait_on(&conn, person)
    }

    /// The [`primary_portrait`](Self::primary_portrait) read on a caller-supplied
    /// connection, using a **cached** prepared statement — the read twin a
    /// relationship walk routes its per-person portrait lookup through.
    /// Same SQL → identical result.
    ///
    /// # Errors
    /// Returns [`CoreError::Database`] on a SQL failure.
    pub(crate) fn primary_portrait_on(
        conn: &rusqlite::Connection,
        person: PersonId,
    ) -> Result<Option<MediaId>> {
        let mut stmt = conn.prepare_cached(
            "SELECT media_id FROM media_links
             WHERE individual_id = ?1 AND is_primary = 1
             ORDER BY media_id LIMIT 1",
        )?;
        let id = stmt
            .query_row([person], |row| row.get::<_, MediaId>(0))
            .optional()?;
        Ok(id)
    }
}
