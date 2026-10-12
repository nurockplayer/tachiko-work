//! Steward acceptance seed for #519; ADR-0042 / portable-package-v1.md.
//! Tests the public host/byte boundaries so the v1-only baseline still compiles.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Barrier,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};
use tachiko_semantic_core as semantic;
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES,
    compare_portable_package_with_roproj, decode_portable_package_v1, decode_roproj_v3,
    encode_roproj_v1, encode_roproj_v2, encode_roproj_v3, pack_roproj, publish_roproj_v3,
    read_canonical_roproj_v3, read_portable_package, unpack_roproj,
};
#[path = "../../../acceptance/portable-package-v3/bytes.rs"]
mod bytes;
#[path = "../../../acceptance/portable-package-v3/document.rs"]
mod fixture;
const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/research/fixtures/issue-519-portable-package-v1-v3"
);
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "issue-519-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn assert_class(error: &FormatError, expected: &str) {
    let message = error.to_string();
    assert!(
        message.starts_with(&format!("portable_package.{expected}:")),
        "expected portable_package.{expected}; actual {message}"
    );
}
fn write_files(root: &Path, files: &[(String, Vec<u8>)]) {
    fs::create_dir_all(root.join("entities")).unwrap();
    for (p, b) in files {
        fs::write(root.join(p), b).unwrap();
    }
}
fn assert_files(root: &Path, files: &[(String, Vec<u8>)]) {
    for (p, b) in files {
        assert_eq!(&fs::read(root.join(p)).unwrap(), b, "{p}");
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 4);
    assert_eq!(fs::read_dir(root.join("entities")).unwrap().count(), 16);
}
fn source(temp: &Temp) -> PathBuf {
    let source = temp.path("source.roproj");
    write_files(&source, &bytes::input_files());
    source
}

#[test]
fn accepted_v3_input_matches_captured_encoder_bytes_and_fresh_meaning() {
    let tree = encode_roproj_v3(&fixture::document()).unwrap();
    let files: Vec<_> = tree
        .files()
        .iter()
        .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
        .collect();
    assert_eq!(files, bytes::input_files());
    assert_eq!(
        decode_roproj_v3(&CanonicalRoProjectV3::try_from_files(bytes::input_files()).unwrap())
            .unwrap(),
        fixture::document()
    );
    assert_eq!(tree.files().len(), 19);
    assert_eq!(
        tree.files()
            .iter()
            .skip(3)
            .filter(|f| f.bytes().is_empty())
            .count(),
        14
    );
    assert_eq!(fixture::DEFINITION_ID, "acceptance-orders-summary");
}

#[test]
fn ac1_exact_independent_golden_bytes() {
    let temp = Temp::new();
    let source = source(&temp);
    let package = temp.path("packed.ro");
    let result = pack_roproj(&source, &package);
    assert_files(&source, &bytes::input_files());
    result.expect("canonical v3 source must pack without down-conversion");
    assert_eq!(fs::read(package).unwrap(), bytes::vector("golden"));
}

#[test]
fn ac1_unpack_fresh_reopen_and_repack_are_exact() {
    let temp = Temp::new();
    let package = temp.path("golden.ro");
    let unpacked = temp.path("unpacked.roproj");
    let repacked = temp.path("repacked.ro");
    fs::write(&package, bytes::vector("golden")).unwrap();
    // Independent input reaches the reader even while the baseline writer refuses v3.
    unpack_roproj(&package, &unpacked).expect("first-public v3 golden must unpack");
    assert_files(&unpacked, &bytes::input_files());
    let reopened = read_canonical_roproj_v3(&unpacked).unwrap();
    assert_eq!(decode_roproj_v3(&reopened).unwrap(), fixture::document());
    pack_roproj(&unpacked, &repacked).unwrap();
    assert_eq!(fs::read(repacked).unwrap(), bytes::vector("golden"));
    let verified = read_portable_package(&package).unwrap();
    assert_eq!(
        verified
            .tree()
            .files()
            .iter()
            .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
            .collect::<Vec<_>>(),
        bytes::input_files()
    );
}

fn refusal(name: &str, expected: &str) {
    let input = bytes::vector(name);
    assert_class(
        &decode_portable_package_v1(&input).expect_err("must refuse"),
        expected,
    );
    let temp = Temp::new();
    let package = temp.path("refused.ro");
    let destination = temp.path("absent.roproj");
    fs::write(&package, &input).unwrap();
    let result = unpack_roproj(&package, &destination);
    assert!(
        !destination.exists(),
        "refusal must leave destination absent"
    );
    assert_eq!(fs::read(&package).unwrap(), input);
    assert_eq!(
        fs::read_dir(&temp.0).unwrap().count(),
        1,
        "no staging debris"
    );
    assert_class(
        &result.expect_err("must refuse before publication"),
        expected,
    );
}
macro_rules! refusal_case {
    ($test:ident, $name:literal, $class:literal) => {
        #[test]
        fn $test() {
            refusal($name, $class);
        }
    };
}
refusal_case!(
    ac3_developmental,
    "developmental",
    "unsupported_payload_representation"
);
refusal_case!(ac3_unknown_profile, "unknown-profile", "invalid_manifest");
refusal_case!(
    ac3_unknown_version,
    "unknown-version",
    "unsupported_version"
);
refusal_case!(
    ac3_unknown_payload_profile,
    "unknown-payload-profile",
    "unsupported_payload_representation"
);
refusal_case!(
    ac3_unknown_payload_version,
    "unknown-payload-version",
    "unsupported_payload_representation"
);
refusal_case!(
    ac3_old_payload_claims_before_body,
    "old-payload-claims-before-body",
    "unsupported_payload_representation"
);
refusal_case!(ac3_integrity, "integrity", "integrity_mismatch");
refusal_case!(ac3_missing_entry, "missing-entry", "entry_set_mismatch");
refusal_case!(ac3_extra_entry, "extra-entry", "entry_set_mismatch");
refusal_case!(ac3_duplicate_entry, "duplicate-entry", "entry_set_mismatch");
refusal_case!(ac3_alias_entry, "alias-entry", "entry_set_mismatch");
refusal_case!(ac3_entry_order, "entry-order", "noncanonical_container");
refusal_case!(ac3_crc, "crc", "crc_mismatch");
refusal_case!(
    ac3_inner_manifest,
    "inner-manifest",
    "payload_manifest_mismatch"
);
refusal_case!(
    ac3_noncanonical_payload,
    "noncanonical-payload",
    "noncanonical_payload"
);
refusal_case!(
    ac3_invalid_semantic_payload,
    "invalid-semantic-payload",
    "invalid_semantic_payload"
);
refusal_case!(ac3_compression, "compression", "noncanonical_container");
refusal_case!(ac3_encryption, "encryption", "noncanonical_container");
refusal_case!(
    ac3_data_descriptor,
    "data-descriptor",
    "noncanonical_container"
);
refusal_case!(ac3_timestamp, "timestamp", "noncanonical_container");
refusal_case!(ac3_truncated, "truncated", "invalid_container");
refusal_case!(ac3_trailing, "trailing", "invalid_container");
refusal_case!(ac3_prepended, "prepended", "invalid_container");
refusal_case!(ac3_split, "split", "invalid_container");
refusal_case!(ac3_zip64_sentinel, "zip64-sentinel", "invalid_container");
refusal_case!(
    ac3_manifest_depth_256,
    "manifest-depth-256",
    "unsupported_version"
);
refusal_case!(
    ac3_manifest_depth_257,
    "manifest-depth-257",
    "resource_limit"
);
refusal_case!(
    ac3_duplicate_manifest_member,
    "duplicate-manifest-member",
    "invalid_manifest"
);
refusal_case!(
    ac3_noncanonical_manifest,
    "noncanonical-manifest",
    "noncanonical_container"
);
refusal_case!(
    ac3_malformed_version,
    "malformed-version",
    "invalid_manifest"
);

#[test]
fn ac3_old_source_versions_are_explicitly_unsupported_without_migration() {
    let temp = Temp::new();
    let doc = semantic::Document::empty("legacy-519", "Legacy");
    let v1 = encode_roproj_v1(&doc).unwrap();
    let v2 = encode_roproj_v2(&doc).unwrap();
    for (version, files) in [
        (
            1,
            v1.files()
                .iter()
                .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
                .collect::<Vec<_>>(),
        ),
        (
            2,
            v2.files()
                .iter()
                .map(|f| (f.path().to_owned(), f.bytes().to_vec()))
                .collect(),
        ),
    ] {
        let source = temp.path(&format!("v{version}.roproj"));
        let destination = temp.path(&format!("v{version}.ro"));
        write_files(&source, &files);
        let result = pack_roproj(&source, &destination);
        for (p, b) in &files {
            assert_eq!(&fs::read(source.join(p)).unwrap(), b);
        }
        assert_class(
            &result.expect_err("older source needs explicit migration"),
            "unsupported_payload_representation",
        );
        assert!(!destination.exists());
    }
}

#[test]
fn ac3_noncanonical_source_does_not_repair_or_publish() {
    let temp = Temp::new();
    let source = source(&temp);
    fs::write(source.join("extra"), b"keep").unwrap();
    let destination = temp.path("refused.ro");
    let result = pack_roproj(&source, &destination);
    for (p, b) in bytes::input_files() {
        assert_eq!(fs::read(source.join(p)).unwrap(), b);
    }
    assert_eq!(fs::read(source.join("extra")).unwrap(), b"keep");
    assert!(!destination.exists());
    assert_class(&result.unwrap_err(), "source_not_canonical");
}

#[test]
fn ac3_existing_pack_destination_is_unchanged() {
    let temp = Temp::new();
    let source = source(&temp);
    let dest = temp.path("exists.ro");
    fs::write(&dest, b"sentinel").unwrap();
    let result = pack_roproj(&source, &dest);
    assert_eq!(fs::read(dest).unwrap(), b"sentinel");
    assert_files(&source, &bytes::input_files());
    assert_class(&result.unwrap_err(), "destination_exists");
}

#[test]
fn ac3_existing_unpack_destination_is_unchanged() {
    let temp = Temp::new();
    let package = temp.path("golden.ro");
    fs::write(&package, bytes::vector("golden")).unwrap();
    let dest = temp.path("exists.roproj");
    fs::create_dir(&dest).unwrap();
    fs::write(dest.join("sentinel"), b"sentinel").unwrap();
    let result = unpack_roproj(&package, &dest);
    assert_eq!(fs::read(dest.join("sentinel")).unwrap(), b"sentinel");
    assert_eq!(fs::read_dir(dest).unwrap().count(), 1);
    assert_class(&result.unwrap_err(), "destination_exists");
}

// Simultaneous real public calls, following portable_package_host.rs. Borrowed
// paths/barrier coordinate two owners without any private publication hook.
fn race(operation: impl Fn() -> Result<(), FormatError> + Sync) {
    let barrier = Barrier::new(3);
    let results = thread::scope(|scope| {
        let call = || {
            barrier.wait();
            operation()
        };
        let a = scope.spawn(call);
        let b = scope.spawn(call);
        barrier.wait();
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "exactly one complete publication: {results:?}"
    );
    assert_class(
        results.iter().find_map(|r| r.as_ref().err()).unwrap(),
        "destination_exists",
    );
}
#[test]
fn ac3_concurrent_pack_publication_has_one_winner_and_no_overwrite() {
    let temp = Temp::new();
    let source = source(&temp);
    let dest = temp.path("raced.ro");
    race(|| pack_roproj(&source, &dest));
    assert_eq!(fs::read(dest).unwrap(), bytes::vector("golden"));
    assert_files(&source, &bytes::input_files());
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
}
#[test]
fn ac3_concurrent_unpack_publication_has_one_winner_and_no_overwrite() {
    let temp = Temp::new();
    let package = temp.path("golden.ro");
    fs::write(&package, bytes::vector("golden")).unwrap();
    let dest = temp.path("raced.roproj");
    race(|| unpack_roproj(&package, &dest));
    assert_files(&dest, &bytes::input_files());
    assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
}

#[test]
fn ac3_tracked_source_disagreement_is_read_only_and_consistency_is_exact() {
    let temp = Temp::new();
    let source = source(&temp);
    let package = temp.path("golden.ro");
    let golden = bytes::vector("golden");
    fs::write(&package, &golden).unwrap();
    assert_eq!(
        compare_portable_package_with_roproj(&package, &source).unwrap(),
        read_portable_package(&package).unwrap().payload_root()
    );
    let mut different = fixture::document();
    different.title = "Changed title".into();
    let other = temp.path("other.roproj");
    let tree = encode_roproj_v3(&different).unwrap();
    publish_roproj_v3(&other, &tree).unwrap();
    let result = compare_portable_package_with_roproj(&package, &other);
    assert_eq!(read_canonical_roproj_v3(&other).unwrap(), tree);
    assert_eq!(fs::read(&package).unwrap(), golden);
    assert_files(&source, &bytes::input_files());
    assert_class(&result.unwrap_err(), "source_mismatch");
}

#[test]
fn ac3_archive_bound_reports_resource_and_preserves_capacity_distinction() {
    assert_eq!(PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES, 67_108_864);
    let mut input = vec![0; PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES + 1];
    input[..4].copy_from_slice(b"PK\x03\x04");
    let error = decode_portable_package_v1(&input).unwrap_err();
    assert_class(&error, "resource_limit");
    assert!(error.to_string().contains("67108864"));
    input.pop();
    // Exactly the host bound reaches framing; it is not a valid 64MiB package.
    assert_class(
        &decode_portable_package_v1(&input).unwrap_err(),
        "invalid_container",
    );
}

#[test]
fn ac3_source_byte_budget_is_bounded_before_body_admission() {
    let temp = Temp::new();
    let source = source(&temp);
    fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(source.join("schemas.json"))
        .unwrap()
        .set_len(u64::try_from(PORTABLE_PACKAGE_V1_MAX_ARCHIVE_BYTES).unwrap())
        .unwrap();
    let dest = temp.path("oversized.ro");
    let result = pack_roproj(&source, &dest);
    assert!(!dest.exists());
    assert_class(&result.unwrap_err(), "resource_limit");
}

#[test]
fn ac3_publication_failure_never_exposes_partial_success() {
    let temp = Temp::new();
    let source = source(&temp);
    let dest = temp.path("absent-parent/package.ro");
    let result = pack_roproj(&source, &dest);
    assert!(!dest.exists());
    assert_files(&source, &bytes::input_files());
    assert_class(&result.unwrap_err(), "publication_failed");
}

#[test]
fn semantic_refusal_fixture_is_a_storage_diagnostic_not_malformed_json() {
    let mut files = bytes::input_files();
    let entity = &mut files
        .iter_mut()
        .find(|(p, b)| p.starts_with("entities/") && !b.is_empty())
        .unwrap()
        .1;
    let original = std::str::from_utf8(entity).unwrap();
    let changed = original.replacen(
        "\"schema\":\"import_00000000-0000-4000-8000-000000000001_0002\"",
        "\"schema\":\"missing-schema\"",
        1,
    );
    assert_ne!(original, changed);
    *entity = changed.into_bytes();
    let error = CanonicalRoProjectV3::try_from_files(files).unwrap_err();
    assert!(
        matches!(error, FormatError::InvalidDocument { .. }),
        "{error:?}"
    );
}

#[test]
fn runtime_control_is_also_exact_accepted_v3_storage() {
    let mut control = fixture::document();
    for schema in control.schemas.values_mut() {
        for field in schema.fields.values_mut() {
            field.constraint = semantic::FieldConstraint::None;
        }
    }
    let tree = CanonicalRoProjectV3::try_from_files(bytes::control_files()).unwrap();
    assert_eq!(tree, encode_roproj_v3(&control).unwrap());
    assert_eq!(decode_roproj_v3(&tree).unwrap(), control);
}
