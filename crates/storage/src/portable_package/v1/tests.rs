use super::*;

#[test]
fn crc32_matches_the_standard_zip_check_vector() {
    assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
}

#[test]
fn package_paths_are_exactly_the_v3_paths_with_one_manifest() {
    assert_eq!(PACKAGE_PATHS.len(), usize::from(PACKAGE_ENTRY_COUNT));
    assert_eq!(PACKAGE_PATHS[0], "package.json");
    assert_eq!(
        PACKAGE_PATHS[1..]
            .iter()
            .map(|path| path.strip_prefix("payload/").unwrap())
            .collect::<Vec<_>>(),
        ROPROJ_V3_PATHS
    );
}

#[test]
fn empty_v3_roundtrip_keeps_definitions_and_all_empty_shards() {
    let tree =
        crate::encode_roproj_v3(&tachiko_semantic_core::Document::empty("empty", "Empty")).unwrap();
    assert_eq!(tree.file("definitions.json").unwrap(), b"[]\n");
    for path in ROPROJ_V3_PATHS.iter().skip(3) {
        assert_eq!(tree.file(path).unwrap(), b"");
    }
    let bytes = encode(&tree).unwrap();
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.tree(), &tree);
    assert_eq!(encode(decoded.tree()).unwrap(), bytes);
    assert_ne!(
        payload_root(&tree),
        calculate_payload_root(
            tree.files()
                .iter()
                .filter(|file| file.path() != "definitions.json")
                .map(|file| (file.path(), file.bytes()))
        )
    );
}

#[test]
fn invalid_saved_definition_is_semantic_while_noncanonical_bytes_remain_representation_errors() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/fixtures/issue-519-portable-package-v1-v3/input.roproj");
    let original = ROPROJ_V3_PATHS
        .iter()
        .map(|path| ((*path).to_owned(), std::fs::read(root.join(path)).unwrap()))
        .collect::<Vec<_>>();
    for semantic_failure in [true, false] {
        let mut files = original.clone();
        let definitions = &mut files[2].1;
        if semantic_failure {
            let source = std::str::from_utf8(definitions).unwrap();
            // The first schema binding is orders.schema; retain the DTO's
            // canonical rendering while changing only its semantic target.
            *definitions = source
                .replacen(
                    "\"schema\": \"import_00000000-0000-4000-8000-000000000001_0002\"",
                    "\"schema\": \"missing-schema\"",
                    1,
                )
                .into_bytes();
            assert_ne!(*definitions, original[2].1);
        } else {
            definitions.push(b'\n');
        }
        // Rebuild headers/CRCs and the root so admission reaches payload
        // validation rather than an earlier integrity refusal.
        let manifest = render_manifest(&payload_root_from_files(&files));
        let mut entries = vec![Entry {
            name: PACKAGE_PATHS[0],
            body: manifest.as_bytes(),
        }];
        entries.extend(files.iter().enumerate().map(|(index, (_, body))| Entry {
            name: PACKAGE_PATHS[index + 1],
            body,
        }));
        let error = decode(&build_zip(&entries).unwrap()).unwrap_err();
        if semantic_failure {
            let FormatError::PortablePackage(PortablePackageError::InvalidSemanticPayload {
                diagnostics,
            }) = error
            else {
                panic!("expected invalid_semantic_payload, found {error}");
            };
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("missing-schema"));
        } else {
            assert!(matches!(
                error,
                FormatError::PortablePackage(PortablePackageError::NonCanonicalPayload { .. })
            ));
        }
    }
}

#[test]
fn payload_versions_are_lexical_positive_integers_and_never_fall_back() {
    let manifest = render_manifest(&[0; 32]);
    for token in ["0", "-1", "3.0", "3e0", "\"3\"", "null", "true", "[]", "{}"] {
        let changed = manifest.replace(
            "\"payload_format_version\": 3",
            &format!("\"payload_format_version\": {token}"),
        );
        assert!(
            matches!(
                parse_manifest(changed.as_bytes()),
                Err(PortablePackageError::InvalidManifest { .. })
            ),
            "{token}"
        );
    }
    for token in [
        "1",
        "2",
        "4",
        "4294967296",
        "999999999999999999999999999999999999",
        &"9".repeat(400),
    ] {
        let changed = manifest.replace(
            "\"payload_format_version\": 3",
            &format!("\"payload_format_version\": {token}"),
        );
        assert!(
            matches!(parse_manifest(changed.as_bytes()), Err(PortablePackageError::UnsupportedPayloadRepresentation { format, version }) if format == PAYLOAD_FORMAT && version == token),
            "{token}"
        );
    }
    assert!(parse_manifest(manifest.as_bytes()).is_ok());
}

#[test]
fn payload_profile_refuses_unknown_and_malformed_claims_separately() {
    let manifest = render_manifest(&[0; 32]);
    let unknown = manifest.replace("tachiko.roproj", "other.roproj");
    assert!(matches!(
        parse_manifest(unknown.as_bytes()),
        Err(PortablePackageError::UnsupportedPayloadRepresentation { .. })
    ));
    for token in ["null", "3", "true", "[]"] {
        let changed = manifest.replace(
            "\"payload_format\": \"tachiko.roproj\"",
            &format!("\"payload_format\": {token}"),
        );
        assert!(matches!(
            parse_manifest(changed.as_bytes()),
            Err(PortablePackageError::InvalidManifest { .. })
        ));
    }
}

#[test]
fn zip32_sentinel_and_total_overflow_fail_before_allocation() {
    require_zip32(0xffff_fffe, "test field").unwrap();
    assert!(matches!(
        require_zip32(0xffff_ffff, "test field"),
        Err(PortablePackageError::CapacityExceeded {
            resource: "test field"
        })
    ));
    assert!(matches!(
        preflight_zip_lengths(0xffff, []),
        Err(PortablePackageError::CapacityExceeded {
            resource: "entry count"
        })
    ));
    assert!(matches!(
        preflight_zip_lengths(1, [(1, 0xffff_ffff)]),
        Err(PortablePackageError::CapacityExceeded {
            resource: "entry body length"
        })
    ));
    assert!(matches!(
        preflight_zip_lengths(1, [(1, 0xffff_fffe)]),
        Err(PortablePackageError::CapacityExceeded {
            resource: "local record section"
        })
    ));
    // Local records fit, but central/end overhead crosses the complete-length bound.
    assert!(matches!(
        preflight_zip_lengths(1, [(1, 0xffff_fffe - 31)]),
        Err(PortablePackageError::CapacityExceeded {
            resource: "complete archive"
        })
    ));
    assert!(matches!(
        checked_sum(u64::MAX, 1),
        Err(PortablePackageError::CapacityExceeded { .. })
    ));
}

#[test]
fn archive_budget_includes_zip_and_manifest_overhead() {
    let overhead = 30 + 1 + 46 + 1 + 22;
    assert_eq!(
        preflight_zip_lengths(1, [(1, PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES - overhead)]).unwrap(),
        PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES
    );
    assert!(
        matches!(preflight_zip_lengths(1, [(1, PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES - overhead + 1)]), Err(PortablePackageError::ResourceLimit { actual, .. }) if actual == PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES + 1)
    );
}
