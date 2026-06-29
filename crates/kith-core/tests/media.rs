//! Integration tests for the media surface: atomic import (bytes
//! copied + rows written), gallery listing, the single-primary invariant,
//! cascade-on-delete, the base64 `data:` URL read, and the typed-error paths
//! (unsupported mime / missing source write nothing).

use std::path::Path;

use kith_core::prelude::*;

/// A 1×1 transparent PNG (a minimal valid image; the CRUD path only copies the
/// bytes, so any content works — this keeps the fixture self-contained).
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Helper: a default individual.
fn person(store: &Store) -> Individual {
    store
        .create_individual(&NewIndividual::default())
        .expect("create individual")
}

/// Helper: write the tiny PNG into `dir` as `name` and return its path.
fn source_image(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, TINY_PNG).expect("write source image");
    path
}

#[test]
fn import_copies_bytes_and_writes_rows_atomically() {
    // Arrange
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let src = source_image(tmp.path(), "portrait.png");
    let p = person(&store);

    // Act
    let media = store
        .import_media(&media_root, &src, MediaSubject::Individual(p.id), true)
        .expect("import media");

    // Assert — the row, the derived relative path + mime, and the copied file.
    assert_eq!(media.path, format!("{}.png", media.id.get()));
    assert_eq!(media.mime.as_deref(), Some("image/png"));
    let copied = media_root.join(&media.path);
    assert_eq!(std::fs::read(&copied).expect("read copied"), TINY_PNG);
    // The link is primary, and the person's portrait resolves to it.
    assert_eq!(
        store.primary_portrait(p.id).expect("portrait"),
        Some(media.id)
    );
    let items = store
        .list_media_for(MediaSubject::Individual(p.id))
        .expect("list");
    assert_eq!(items.len(), 1);
    assert!(items[0].is_primary);
}

#[test]
fn unsupported_extension_is_validation_and_writes_nothing() {
    // Arrange
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let src = source_image(tmp.path(), "notes.txt");
    let p = person(&store);

    // Act
    let err = store
        .import_media(&media_root, &src, MediaSubject::Individual(p.id), true)
        .expect_err("unsupported type must error");

    // Assert — typed Validation, no row, no media folder created.
    assert!(matches!(err, CoreError::Validation(_)), "got {err:?}");
    assert!(
        store
            .list_media_for(MediaSubject::Individual(p.id))
            .expect("list")
            .is_empty()
    );
    assert!(
        !media_root.exists(),
        "nothing should be written on a bad mime"
    );
}

#[test]
fn missing_source_is_io_error() {
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let p = person(&store);
    let err = store
        .import_media(
            &media_root,
            &tmp.path().join("does-not-exist.png"),
            MediaSubject::Individual(p.id),
            false,
        )
        .expect_err("missing source must error");
    assert!(matches!(err, CoreError::Io(_)), "got {err:?}");
}

#[test]
fn set_primary_keeps_a_single_primary_per_subject() {
    // Arrange — two images on one person, the first primary.
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let p = person(&store);
    let subject = MediaSubject::Individual(p.id);
    let first = store
        .import_media(
            &media_root,
            &source_image(tmp.path(), "a.png"),
            subject,
            true,
        )
        .expect("import a");
    let second = store
        .import_media(
            &media_root,
            &source_image(tmp.path(), "b.jpg"),
            subject,
            false,
        )
        .expect("import b");

    // Act — promote the second.
    store.set_primary(second.id, subject).expect("set primary");

    // Assert — exactly one primary, and it is the second.
    assert_eq!(
        store.primary_portrait(p.id).expect("portrait"),
        Some(second.id)
    );
    let primaries = store
        .list_media_for(subject)
        .expect("list")
        .into_iter()
        .filter(|i| i.is_primary)
        .count();
    assert_eq!(primaries, 1, "at most one primary per subject");
    assert_eq!(first.mime.as_deref(), Some("image/png"));
    assert_eq!(second.mime.as_deref(), Some("image/jpeg"));
}

#[test]
fn delete_media_cascades_its_links() {
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let p = person(&store);
    let subject = MediaSubject::Individual(p.id);
    let media = store
        .import_media(
            &media_root,
            &source_image(tmp.path(), "x.png"),
            subject,
            true,
        )
        .expect("import");

    store.delete_media(media.id).expect("delete");

    assert!(store.get_media(media.id).is_err(), "row is gone");
    assert!(
        store.list_media_for(subject).expect("list").is_empty(),
        "the link cascaded with the media row"
    );
    assert_eq!(store.primary_portrait(p.id).expect("portrait"), None);
}

#[test]
fn read_media_data_url_is_well_formed_base64() {
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let p = person(&store);
    let media = store
        .import_media(
            &media_root,
            &source_image(tmp.path(), "y.png"),
            MediaSubject::Individual(p.id),
            true,
        )
        .expect("import");

    let url = store
        .read_media_data_url(&media_root, media.id)
        .expect("data url");

    assert!(url.starts_with("data:image/png;base64,"), "got {url}");
    // The payload is non-empty and uses only the base64 alphabet + padding.
    let payload = url.strip_prefix("data:image/png;base64,").expect("prefix");
    assert!(!payload.is_empty());
    assert!(
        payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')),
        "payload is base64: {payload}"
    );
}

#[test]
fn portrait_data_urls_skips_a_missing_file() {
    let store = Store::open_in_memory().expect("open store");
    let tmp = tempfile::tempdir().expect("temp dir");
    let media_root = tmp.path().join("tree.media");
    let p = person(&store);
    let media = store
        .import_media(
            &media_root,
            &source_image(tmp.path(), "z.png"),
            MediaSubject::Individual(p.id),
            true,
        )
        .expect("import");

    // Remove the file behind the row — the export resolver must skip it, not fail.
    std::fs::remove_file(media_root.join(&media.path)).expect("remove file");
    let urls = store
        .portrait_data_urls(&media_root, &[media.id])
        .expect("resolve");
    assert!(urls.is_empty(), "a missing file is skipped, not an error");
}
