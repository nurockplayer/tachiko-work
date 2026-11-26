mod tests {
    use std::{fs, sync::atomic::Ordering};

    use super::{NEXT_STAGING_FILE, finish_staged_file};

    #[test]
    fn exclusive_file_publication_preserves_a_destination_created_by_a_racer() {
        let sequence = NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tachiko-package-file-race-{}-{sequence}",
            std::process::id()
        ));
        let staging = root.join("stage");
        let destination = root.join("destination");
        fs::create_dir(&root).unwrap();
        fs::write(&staging, b"candidate").unwrap();
        fs::write(&destination, b"racer").unwrap();

        assert!(matches!(
            finish_staged_file(&staging, &destination),
            Err(crate::FormatError::PortablePackage(
                crate::PortablePackageError::DestinationExists { .. }
            ))
        ));
        assert!(!staging.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"racer");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn old_source_dispatch_precedes_v3_layout_and_payload_interpretation() {
        let sequence = NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tachiko-package-source-dispatch-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        for version in [1, 2, 4] {
            fs::write(
                root.join("manifest.json"),
                format!("{{\"format\":\"tachiko.roproj\",\"format_version\":{version}}}"),
            )
            .unwrap();
            assert!(matches!(
                super::read_portable_package_source(&root),
                Err(crate::FormatError::PortablePackage(
                    crate::PortablePackageError::UnsupportedPayloadRepresentation { version: found, .. }
                )) if found == version.to_string()
            ));
            assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn manifest_symlink_is_refused_before_dispatch_or_reading_its_target() {
        let sequence = NEXT_STAGING_FILE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "tachiko-package-source-symlink-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(root.join("absent-target"), root.join("manifest.json")).unwrap();
        assert!(matches!(
            super::read_portable_package_source(&root),
            Err(crate::FormatError::PortablePackage(
                crate::PortablePackageError::SourceNotCanonical { .. }
            ))
        ));
        assert!(!root.join("absent-target").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
