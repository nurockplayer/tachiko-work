use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::{
    Date, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldType,
    KeyedGroupedSumDefinition, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, Schema, Value,
};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, decode_roproj_v3, encode_roproj_v1, encode_roproj_v2,
    encode_roproj_v3, migrate_roproj_to_v3, publish_roproj_v3, read_canonical_roproj_v3,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tachiko-v3-worker-{}-{sequence}",
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

fn fixture() -> Document {
    let mut document = Document::empty("worker-doc", "Worker contract");
    let fields = [
        ("date", FieldType::Date, FieldConstraint::None),
        (
            "formula",
            FieldType::Number,
            FieldConstraint::NumberInclusiveRange {
                min: Number::new(0.0).unwrap(),
                max: Number::new(20.0).unwrap(),
            },
        ),
        (
            "number",
            FieldType::Number,
            FieldConstraint::NumberInclusiveRange {
                min: Number::new(-10.0).unwrap(),
                max: Number::new(20.0).unwrap(),
            },
        ),
        (
            "text",
            FieldType::Text,
            FieldConstraint::TextLiteralSet {
                values: vec!["alpha".into(), "βeta".into()],
            },
        ),
    ]
    .into_iter()
    .map(|(id, field_type, constraint)| {
        (
            id.into(),
            FieldDefinition {
                id: id.into(),
                key: id.into(),
                field_type,
                required: false,
                constraint,
            },
        )
    })
    .collect();
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields,
        },
    );
    document.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([
                ("date".into(), Value::Date(Date::new(1, 1, 1).unwrap())),
                (
                    "formula".into(),
                    Value::Formula(Expression::Number(Number::new(5.0).unwrap())),
                ),
                ("number".into(), Value::Number(Number::new(20.0).unwrap())),
                ("text".into(), Value::Text("βeta".into())),
            ]),
        },
    );
    document.keyed_grouped_sum_definitions.insert(
        "definition".into(),
        KeyedGroupedSumDefinition {
            id: "definition".into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: "schema".into(),
                lookup_key_field: "text".into(),
                quantity_field: "number".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "schema".into(),
                key_field: "text".into(),
                category_field: "text".into(),
                price_field: "number".into(),
            },
        },
    );
    document
}

fn files(tree: &CanonicalRoProjectV3) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect()
}

fn write_tree(root: &std::path::Path, files: &[(String, Vec<u8>)]) {
    fs::create_dir(root).unwrap();
    fs::create_dir(root.join("entities")).unwrap();
    for (path, bytes) in files {
        fs::write(root.join(path), bytes).unwrap();
    }
}

#[test]
fn semantic_oracle_date_extremes_formulas_definitions_and_writer_sorting() {
    let mut document = fixture();
    if let FieldConstraint::TextLiteralSet { values } = &mut document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("text")
        .unwrap()
        .constraint
    {
        values.reverse();
    }
    document.entities.get_mut("entity").unwrap().fields.insert(
        "date".into(),
        Value::Date(Date::new(9_999, 12, 31).unwrap()),
    );
    let tree = encode_roproj_v3(&document).unwrap();
    let decoded = decode_roproj_v3(&tree).unwrap();
    assert_eq!(
        decoded.entities["entity"].fields["date"],
        Value::Date(Date::new(9_999, 12, 31).unwrap())
    );
    assert_eq!(
        decoded.keyed_grouped_sum_definitions,
        document.keyed_grouped_sum_definitions
    );
    assert_eq!(
        decoded.entities["entity"].fields["formula"],
        document.entities["entity"].fields["formula"]
    );
    assert_eq!(tree, encode_roproj_v3(&decoded).unwrap());
}

#[test]
fn parser_rejects_unsorted_duplicate_oversized_and_nonfinite_constraint_input() {
    let tree = encode_roproj_v3(&fixture()).unwrap();
    let base = files(&tree);
    let schemas_index = base
        .iter()
        .position(|(path, _)| path == "schemas.json")
        .unwrap();
    for (from, to) in [
        (
            "\"values\": [\n            \"alpha\",\n            \"βeta\"",
            "\"values\": [\n            \"βeta\",\n            \"alpha\"",
        ),
        (
            "\"values\": [\n            \"alpha\",\n            \"βeta\"",
            "\"values\": [\n            \"alpha\",\n            \"alpha\"",
        ),
        ("\"min\": -10,", "\"min\": 1e9999,"),
        ("\"min\": -10,", "\"min\": 30,"),
    ] {
        let mut candidate = base.clone();
        let text = String::from_utf8(candidate[schemas_index].1.clone()).unwrap();
        assert!(text.contains(from));
        candidate[schemas_index].1 = text.replacen(from, to, 1).into_bytes();
        assert!(CanonicalRoProjectV3::try_from_files(candidate).is_err());
    }
    let mut oversized = base;
    let text = String::from_utf8(oversized[schemas_index].1.clone()).unwrap();
    let too_long = "x".repeat(1025);
    oversized[schemas_index].1 = text
        .replacen("\"alpha\"", &format!("\"{too_long}\""), 1)
        .into_bytes();
    assert!(CanonicalRoProjectV3::try_from_files(oversized).is_err());

    let mut aggregate_oversized = files(&tree);
    let text = String::from_utf8(aggregate_oversized[schemas_index].1.clone()).unwrap();
    let start = text.find("\"values\": [").unwrap();
    let end = text[start..].find(']').unwrap() + start + 1;
    let values = (0..65)
        .map(|index| format!("{:03}{}", index, "x".repeat(1021)))
        .map(|value| format!("            {}", serde_json::to_string(&value).unwrap()))
        .collect::<Vec<_>>()
        .join(",\n");
    let replacement = format!("\"values\": [\n{values}\n          ]");
    aggregate_oversized[schemas_index].1 =
        format!("{}{}{}", &text[..start], replacement, &text[end..]).into_bytes();
    assert!(matches!(
        CanonicalRoProjectV3::try_from_files(aggregate_oversized),
        Err(FormatError::InvalidRoProjectRepresentation { message })
            if message.contains("65536")
    ));
}

#[test]
fn date_decoder_enforces_canonical_gregorian_endpoints() {
    let tree = encode_roproj_v3(&fixture()).unwrap();
    for (from, to) in [
        ("0001-01-01", "0000-01-01"),
        ("0001-01-01", "1900-02-29"),
        ("0001-01-01", "0001-1-01"),
    ] {
        let mut candidate = files(&tree);
        let index = candidate
            .iter()
            .position(|(path, bytes)| path.starts_with("entities/") && !bytes.is_empty())
            .unwrap();
        let bytes = String::from_utf8(candidate[index].1.clone()).unwrap();
        assert!(bytes.contains(from));
        candidate[index].1 = bytes.replacen(from, to, 1).into_bytes();
        assert!(CanonicalRoProjectV3::try_from_files(candidate).is_err());
    }
}

#[test]
fn exact_vector_and_profile_rejection_are_strict() {
    let tree = encode_roproj_v3(&fixture()).unwrap();
    let mut reordered = files(&tree);
    reordered.swap(0, 1);
    assert!(CanonicalRoProjectV3::try_from_files(reordered).is_err());
    let files = files(&tree);
    let mut called = false;
    let result = CanonicalRoProjectV3::try_from_files_with_profile(files, |_| {
        called = true;
        Err("resource profile")
    });
    assert!(result.is_err());
    assert!(called);
}

#[test]
fn manifest_dispatch_precedes_other_v3_file_decoding() {
    let directory = FixtureDirectory::new();
    let root = directory.0.join("source.roproj");
    let mut files = files(&encode_roproj_v3(&Document::empty("doc", "dispatch")).unwrap());
    files.iter_mut().find(|(path, _)| path == "manifest.json").unwrap().1 = br#"{"format":"tachiko.roproj","format_version":2,"document":{"id":"doc","title":"legacy"}}"#.to_vec();
    files
        .iter_mut()
        .find(|(path, _)| path == "schemas.json")
        .unwrap()
        .1 = b"not json".to_vec();
    write_tree(&root, &files);
    assert!(matches!(
        read_canonical_roproj_v3(&root),
        Err(FormatError::UnsupportedRoProjectVersion {
            found: 2,
            supported: 3
        })
    ));
}

#[test]
fn host_rejects_extra_paths_and_overlap_without_staging_artifacts() {
    let directory = FixtureDirectory::new();
    let tree = encode_roproj_v3(&Document::empty("paths", "Paths")).unwrap();
    let source = directory.0.join("source.roproj");
    let source_files = files(&tree);
    write_tree(&source, &source_files);
    fs::write(source.join("entities/cache.jsonl"), b"").unwrap();
    assert!(read_canonical_roproj_v3(&source).is_err());
    fs::remove_file(source.join("entities/cache.jsonl")).unwrap();
    assert!(matches!(
        migrate_roproj_to_v3(&source, source.join("nested.roproj")),
        Err(FormatError::PathOverlap { .. })
    ));
    assert_eq!(
        fs::read(source.join("manifest.json")).unwrap(),
        source_files[0].1
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn host_refuses_symlinked_source_and_destination_paths() {
    use std::os::unix::fs::symlink;

    let directory = FixtureDirectory::new();
    let source = directory.0.join("source.roproj");
    let tree = encode_roproj_v3(&Document::empty("symlink", "Source")).unwrap();
    write_tree(&source, &files(&tree));
    let linked = directory.0.join("linked.roproj");
    symlink(&source, &linked).unwrap();
    assert!(read_canonical_roproj_v3(&linked).is_err());
    let missing_target = directory.0.join("missing-target");
    let destination_link = directory.0.join("destination.roproj");
    symlink(&missing_target, &destination_link).unwrap();
    assert!(matches!(
        publish_roproj_v3(&destination_link, &tree),
        Err(FormatError::AlreadyExists { .. })
    ));
    assert!(!missing_target.exists());
}

#[test]
fn migration_is_deterministic_and_concurrent_publish_has_one_winner() {
    let directory = FixtureDirectory::new();
    let document = Document::empty("migration", "Determinism");
    let v1_files = encode_roproj_v1(&document)
        .unwrap()
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    let v2_files = encode_roproj_v2(&document)
        .unwrap()
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    for (version, files) in [(1, v1_files), (2, v2_files)] {
        let source = directory.0.join(format!("v{version}.roproj"));
        write_tree(&source, &files);
        let out_a = directory.0.join(format!("v{version}-a.roproj"));
        let out_b = directory.0.join(format!("v{version}-b.roproj"));
        migrate_roproj_to_v3(&source, &out_a).unwrap();
        migrate_roproj_to_v3(&source, &out_b).unwrap();
        assert_eq!(
            read_canonical_roproj_v3(out_a).unwrap(),
            read_canonical_roproj_v3(out_b).unwrap()
        );
    }

    let tree = encode_roproj_v3(&document).unwrap();
    let destination = directory.0.join("raced.roproj");
    let left_tree = tree.clone();
    let right_tree = tree.clone();
    let left_path = destination.clone();
    let right_path = destination.clone();
    let left = std::thread::spawn(move || publish_roproj_v3(left_path, &left_tree));
    let right = std::thread::spawn(move || publish_roproj_v3(right_path, &right_tree));
    let outcomes = [left.join().unwrap(), right.join().unwrap()];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(result, Err(FormatError::AlreadyExists { .. })))
            .count(),
        1
    );
    assert_eq!(read_canonical_roproj_v3(destination).unwrap(), tree);
    assert!(fs::read_dir(&directory.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("tachiko-stage")
    }));
}

#[test]
fn native_publish_and_migration_preserve_existing_paths_and_source() {
    let directory = FixtureDirectory::new();
    let document = Document::empty("host-source", "Unconstrained legacy source");
    let tree = encode_roproj_v3(&document).unwrap();

    let existing_file = directory.0.join("existing-file");
    fs::write(&existing_file, b"keep me").unwrap();
    assert!(matches!(
        publish_roproj_v3(&existing_file, &tree),
        Err(FormatError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read(&existing_file).unwrap(), b"keep me");

    let existing_directory = directory.0.join("existing-directory");
    fs::create_dir(&existing_directory).unwrap();
    fs::write(existing_directory.join("marker"), b"keep me too").unwrap();
    assert!(matches!(
        publish_roproj_v3(&existing_directory, &tree),
        Err(FormatError::AlreadyExists { .. })
    ));
    assert_eq!(
        fs::read(existing_directory.join("marker")).unwrap(),
        b"keep me too"
    );

    let legacy_document = Document::empty("legacy-host", "Legacy host migration");
    let v2 = encode_roproj_v2(&legacy_document).unwrap();
    let source = directory.0.join("source-v2.roproj");
    let source_files = v2
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    write_tree(&source, &source_files);
    let source_before = source_files
        .iter()
        .map(|(path, _)| (path.clone(), fs::read(source.join(path)).unwrap()))
        .collect::<Vec<_>>();
    let nested_destination = source.join("nested-output.roproj");
    assert!(matches!(
        migrate_roproj_to_v3(&source, &nested_destination),
        Err(FormatError::PathOverlap { .. })
    ));
    assert!(!nested_destination.exists());
    let source_after = source_files
        .iter()
        .map(|(path, _)| (path.clone(), fs::read(source.join(path)).unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(source_after, source_before);
}

#[cfg(unix)]
#[test]
fn native_publish_refuses_symlink_destinations_without_touching_target() {
    use std::os::unix::fs::symlink;

    let directory = FixtureDirectory::new();
    let tree = encode_roproj_v3(&fixture()).unwrap();
    let target = directory.0.join("target");
    fs::write(&target, b"target bytes").unwrap();
    let link = directory.0.join("destination-link");
    symlink(&target, &link).unwrap();

    assert!(matches!(
        publish_roproj_v3(&link, &tree),
        Err(FormatError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read(&target).unwrap(), b"target bytes");
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
}
