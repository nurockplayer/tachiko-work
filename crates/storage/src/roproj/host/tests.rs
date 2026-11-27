use std::fs;

use super::{NEXT_STAGING_DIRECTORY, finish_staged_publication};
use std::sync::atomic::Ordering;

#[test]
fn directory_load_refuses_unsupported_versions_before_payload_admission() {
    let sequence = NEXT_STAGING_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "tachiko-storage-read-dispatch-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    // No payload or canonical layout: the manifest must decide first.
    for version in [2, 4] {
        fs::write(
            root.join("manifest.json"),
            format!(r#"{{"format":"tachiko.roproj","format_version":{version}}}"#),
        )
        .unwrap();
        assert!(matches!(
            super::load_roproj_v1_or_v3(&root),
            Err(crate::FormatError::UnsupportedRoProjectVersion { found, supported: 3 })
                if found == version
        ));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exclusive_final_publication_rejects_an_empty_directory_created_by_a_racer() {
    let sequence = NEXT_STAGING_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "tachiko-storage-staging-cleanup-{}-{sequence}",
        std::process::id()
    ));
    let staging = root.join("stage");
    let destination = root.join("destination");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("partial"), b"partial").unwrap();
    fs::create_dir(&destination).unwrap();

    assert!(matches!(
        finish_staged_publication(&staging, &destination),
        Err(crate::FormatError::AlreadyExists { .. })
    ));
    assert!(!staging.exists());
    assert!(destination.is_dir());
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);

    fs::remove_dir_all(root).unwrap();
}
