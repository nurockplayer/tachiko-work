use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::Document;
use tachiko_storage::{
    FormatError, decode_roproj_v3, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3,
    migrate_roproj_to_v3, publish_roproj_v3, read_canonical_roproj_v3,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tachiko-v3-acceptance-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_source(root: &Path, files: &[(String, Vec<u8>)]) {
    fs::create_dir(root).unwrap();
    fs::create_dir(root.join("entities")).unwrap();
    for (path, bytes) in files {
        fs::write(root.join(path), bytes).unwrap();
    }
}

fn assert_source_unchanged(root: &Path, files: &[(String, Vec<u8>)]) {
    for (path, bytes) in files {
        assert_eq!(fs::read(root.join(path)).unwrap(), *bytes);
    }
    assert_eq!(fs::read_dir(root.join("entities")).unwrap().count(), 16);
    assert_eq!(fs::read_dir(root).unwrap().count(), files.len() - 15);
}

fn legacy_files(version: u32, document: &Document) -> Vec<(String, Vec<u8>)> {
    match version {
        1 => encode_roproj_v1(document)
            .unwrap()
            .files()
            .iter()
            .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
            .collect(),
        2 => encode_roproj_v2(document)
            .unwrap()
            .files()
            .iter()
            .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
            .collect(),
        _ => unreachable!(),
    }
}

#[test]
fn explicit_host_migration_preserves_source_and_refuses_overwrite_and_overlap() {
    let directory = FixtureDirectory::new();
    let document = Document::empty("legacy", "Explicit migration");
    for version in [1, 2] {
        let source = directory.0.join(format!("v{version}.roproj"));
        let destination = directory.0.join(format!("v{version}-converted.roproj"));
        let source_files = legacy_files(version, &document);
        write_source(&source, &source_files);
        assert!(read_canonical_roproj_v3(&source).is_err());
        assert_source_unchanged(&source, &source_files);

        migrate_roproj_to_v3(&source, &destination).unwrap();
        let published = read_canonical_roproj_v3(&destination).unwrap();
        assert_eq!(decode_roproj_v3(&published).unwrap(), document);
        assert_source_unchanged(&source, &source_files);
        assert!(matches!(
            migrate_roproj_to_v3(&source, &destination),
            Err(FormatError::AlreadyExists { .. })
        ));
        assert_eq!(read_canonical_roproj_v3(&destination).unwrap(), published);

        for overlap in [source.clone(), source.join("inside.roproj")] {
            assert!(matches!(
                migrate_roproj_to_v3(&source, &overlap),
                Err(FormatError::PathOverlap { .. })
            ));
            assert_source_unchanged(&source, &source_files);
        }
        let existing_file = directory.0.join(format!("existing-{version}"));
        fs::write(&existing_file, b"preserve").unwrap();
        assert!(matches!(
            migrate_roproj_to_v3(&source, &existing_file),
            Err(FormatError::AlreadyExists { .. })
        ));
        assert_eq!(fs::read(existing_file).unwrap(), b"preserve");
    }
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 6);
}

#[test]
fn invalid_or_already_v3_sources_never_publish_partial_output() {
    let directory = FixtureDirectory::new();
    let document = Document::empty("legacy", "Refusal");
    let source = directory.0.join("source.roproj");
    let output = directory.0.join("output.roproj");
    let mut invalid = legacy_files(2, &document);
    invalid
        .iter_mut()
        .find(|(path, _)| path == "schemas.json")
        .unwrap()
        .1 = b"malformed".to_vec();
    write_source(&source, &invalid);
    assert!(migrate_roproj_to_v3(&source, &output).is_err());
    assert!(!output.exists());
    assert_source_unchanged(&source, &invalid);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);

    let current = directory.0.join("current.roproj");
    let tree = encode_roproj_v3(&document).unwrap();
    publish_roproj_v3(&current, &tree).unwrap();
    assert!(migrate_roproj_to_v3(&current, &output).is_err());
    assert!(!output.exists());
    assert_eq!(read_canonical_roproj_v3(&current).unwrap(), tree);
    assert!(matches!(
        publish_roproj_v3(&current, &tree),
        Err(FormatError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn migration_refuses_symlink_sources_and_destinations_without_touching_targets() {
    use std::os::unix::fs::symlink;
    let directory = FixtureDirectory::new();
    let document = Document::empty("legacy", "Symlink refusal");
    let source = directory.0.join("source.roproj");
    let files = legacy_files(2, &document);
    write_source(&source, &files);
    let linked_source = directory.0.join("linked-source.roproj");
    let destination = directory.0.join("destination.roproj");
    symlink(&source, &linked_source).unwrap();
    assert!(migrate_roproj_to_v3(&linked_source, &destination).is_err());
    assert!(!destination.exists());
    let target = directory.0.join("absent-target");
    symlink(&target, &destination).unwrap();
    assert!(matches!(
        migrate_roproj_to_v3(&source, &destination),
        Err(FormatError::AlreadyExists { .. })
    ));
    assert!(!target.exists());
    assert!(
        fs::symlink_metadata(&destination)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_source_unchanged(&source, &files);
}
