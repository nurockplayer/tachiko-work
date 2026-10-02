//! Copy into the pinned historical-reader checkout only after candidate bytes
//! have been freshly captured by the native producer acceptance target.
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
    process::{Command, Stdio},
};

use tachiko_semantic_core::{
    Date, Document, FieldType, KeyedGroupedSumDefinitionId, Number, Value,
};
use tachiko_storage::{
    CanonicalRoProjectV1, CanonicalRoProjectV2, FormatError, decode_roproj_v1, decode_roproj_v2,
    from_bytes,
};

const DOCUMENT_ID: &str = "import_00000000-0000-4000-8000-000000000001_0001";
const SCHEMA_ID: &str = "import_00000000-0000-4000-8000-000000000001_0002";
const FIELDS: [&str; 5] = [
    "import_00000000-0000-4000-8000-000000000001_0003",
    "import_00000000-0000-4000-8000-000000000001_0004",
    "import_00000000-0000-4000-8000-000000000001_0005",
    "import_00000000-0000-4000-8000-000000000001_0006",
    "import_00000000-0000-4000-8000-000000000001_0007",
];
const ROWS: [&str; 2] = [
    "import_00000000-0000-4000-8000-000000000001_0008",
    "import_00000000-0000-4000-8000-000000000001_0009",
];

fn framed_files(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut cursor = Cursor::new(bytes);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"TWDPROJ1");
    let mut count = [0; 4];
    cursor.read_exact(&mut count).unwrap();
    let count = u32::from_le_bytes(count);
    assert!(count <= 1024);
    let mut entries = Vec::new();
    for _ in 0..count {
        let mut path_len = [0; 2];
        let mut body_len = [0; 4];
        cursor.read_exact(&mut path_len).unwrap();
        cursor.read_exact(&mut body_len).unwrap();
        let mut path = vec![0; usize::from(u16::from_le_bytes(path_len))];
        let mut body = vec![0; usize::try_from(u32::from_le_bytes(body_len)).unwrap()];
        cursor.read_exact(&mut path).unwrap();
        cursor.read_exact(&mut body).unwrap();
        entries.push((String::from_utf8(path).unwrap(), body));
    }
    assert_eq!(usize::try_from(cursor.position()).unwrap(), bytes.len());
    entries
}

fn decode_historical(bytes: &[u8]) -> Document {
    if let Some(json) = bytes.strip_prefix(b"TWDPROJ2") {
        return from_bytes(json).expect("old direct reader accepts its immutable date control");
    }
    let entries = framed_files(bytes);
    let manifest: serde_json::Value = serde_json::from_slice(
        &entries
            .iter()
            .find(|(path, _)| path == "manifest.json")
            .unwrap()
            .1,
    )
    .unwrap();
    match manifest["format_version"].as_u64().unwrap() {
        1 => decode_roproj_v1(&CanonicalRoProjectV1::try_from_files(entries).unwrap()).unwrap(),
        2 => decode_roproj_v2(&CanonicalRoProjectV2::try_from_files(entries).unwrap()).unwrap(),
        other => panic!("legacy control has unexpected format version {other}"),
    }
}

fn check_legacy_control(document: &Document, date: bool, definition: bool) {
    assert_eq!(document.id.as_str(), DOCUMENT_ID);
    assert_eq!(document.title, "Imported workbook");
    assert_eq!(document.schemas.len(), 1);
    assert_eq!(document.entities.len(), 2);
    let schema = document.schemas.values().next().unwrap();
    assert_eq!(schema.id.as_str(), SCHEMA_ID);
    assert_eq!(schema.key.as_str(), "sheet_1");
    assert_eq!(schema.fields.len(), 5);
    let fields: BTreeMap<_, _> = schema
        .fields
        .values()
        .map(|field| (field.key.as_str(), field))
        .collect();
    for (index, field_type) in [
        FieldType::Text,
        FieldType::Text,
        FieldType::Number,
        FieldType::Number,
        if date {
            FieldType::Date
        } else {
            FieldType::Text
        },
    ]
    .into_iter()
    .enumerate()
    {
        let field = fields[format!("column_{}", index + 1).as_str()];
        assert_eq!(field.id.as_str(), FIELDS[index]);
        assert_eq!(field.field_type, field_type);
        assert!(!field.required);
        // The pinned reader predates durable constraints; its strict legacy
        // representation has exactly these four field members.
        let field_record = serde_json::to_value(field).unwrap();
        let mut members = field_record
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        members.sort_unstable();
        assert_eq!(members, ["field_type", "id", "key", "required"]);
    }
    for (index, code, category, quantity, date_value) in [
        (0, "PEN", "Stationery", 4.0, "2026-09-28"),
        (1, "NOTE", "Paper", 5.0, "2026-09-27"),
    ] {
        let entity = document
            .entities
            .values()
            .find(|entity| entity.id.as_str() == ROWS[index])
            .unwrap();
        assert_eq!(entity.schema, schema.id);
        assert_eq!(entity.key.as_str(), format!("sheet_1_row_{}", index + 1));
        let actual = [&FIELDS[0], &FIELDS[1], &FIELDS[2], &FIELDS[3], &FIELDS[4]];
        let values = [
            Value::Text(code.into()),
            Value::Text(category.into()),
            Value::Number(Number::new(quantity).unwrap()),
            Value::Number(Number::new(200.0).unwrap()),
            if date {
                Value::Date(Date::parse(date_value).unwrap())
            } else {
                Value::Text(date_value.into())
            },
        ];
        for (field, expected) in actual.into_iter().zip(values) {
            let stored = entity
                .fields
                .iter()
                .find(|(id, _)| id.as_str() == *field)
                .unwrap()
                .1;
            assert_eq!(*stored, expected);
        }
    }
    assert_eq!(
        document.keyed_grouped_sum_definitions.len(),
        usize::from(definition)
    );
    if definition {
        let definition = &document.keyed_grouped_sum_definitions
            [&KeyedGroupedSumDefinitionId::from("acceptance-orders-summary")];
        assert_eq!(definition.orders.schema.as_str(), SCHEMA_ID);
        assert_eq!(definition.products.schema.as_str(), SCHEMA_ID);
        assert_eq!(definition.orders.lookup_key_field.as_str(), FIELDS[0]);
        assert_eq!(definition.orders.quantity_field.as_str(), FIELDS[2]);
        assert_eq!(definition.products.key_field.as_str(), FIELDS[0]);
        assert_eq!(definition.products.category_field.as_str(), FIELDS[1]);
        assert_eq!(definition.products.price_field.as_str(), FIELDS[3]);
    }
}

fn v2_reader_order(entries: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    let paths = [
        "manifest.json",
        "schemas.json",
        "entities/0.jsonl",
        "entities/1.jsonl",
        "entities/2.jsonl",
        "entities/3.jsonl",
        "entities/4.jsonl",
        "entities/5.jsonl",
        "entities/6.jsonl",
        "entities/7.jsonl",
        "entities/8.jsonl",
        "entities/9.jsonl",
        "entities/a.jsonl",
        "entities/b.jsonl",
        "entities/c.jsonl",
        "entities/d.jsonl",
        "entities/e.jsonl",
        "entities/f.jsonl",
        "definitions.json",
    ];
    paths
        .iter()
        .map(|path| {
            entries
                .iter()
                .find(|(candidate, _)| candidate == path)
                .unwrap()
                .clone()
        })
        .collect()
}

// No new Rust/product dependency: Node's built-in SHA-256 checks the complete
// seal and kit. Return decoded snapshots; do not reopen a path after verification.
fn sealed_capture_artifacts(
    capture_root: &str,
    candidate_head: &str,
    run_id: &str,
) -> BTreeMap<String, Vec<u8>> {
    let kit_root = std::env::var("TACHIKO_V3_ACCEPTANCE_KIT_DIR").expect(
        "NOTRUN_MISSING_KIT_DIRECTORY: historical qualification requires the exact exported kit",
    );
    let mut child = Command::new("node")
        .args([
            "--input-type=module",
            "-",
            capture_root,
            candidate_head,
            run_id,
            &kit_root,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("NOTRUN_MISSING_NODE: seal verification needs Node built-ins");
    let script = format!(
        "{}\nprocess.stdout.write(JSON.stringify(verifyCaptureSeal(...process.argv.slice(2))));\n",
        include_str!("verify-capture-seal.mjs")
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "capture seal failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let verified: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(verified["candidate_head"].as_str(), Some(candidate_head));
    assert_eq!(verified["run_id"].as_str(), Some(run_id));
    verified["artifacts"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            let hex = value.as_str().unwrap();
            assert_eq!(hex.len() % 2, 0);
            let bytes = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
                .collect();
            (name.clone(), bytes)
        })
        .collect()
}

#[test]
fn historical_reader_controls_are_rechecked_and_candidate_v3_is_truthfully_unsupported() {
    let input_root = std::env::var("TACHIKO_V3_LEGACY_INPUT_DIR").unwrap();
    for (name, date, definition) in [
        ("date-only", true, false),
        ("definition-only", false, true),
        ("neither", false, false),
    ] {
        let bytes = std::fs::read(format!("{input_root}/legacy/{name}.twd")).unwrap();
        check_legacy_control(&decode_historical(&bytes), date, definition);
    }

    let capture_root = std::env::var("TACHIKO_V3_ACCEPTANCE_CAPTURE_DIR").unwrap();
    let candidate_head = std::env::var("TACHIKO_V3_ACCEPTANCE_CANDIDATE_HEAD").unwrap();
    let run_id = std::env::var("TACHIKO_V3_ACCEPTANCE_RUN_ID").unwrap();
    let artifacts = sealed_capture_artifacts(&capture_root, &candidate_head, &run_id);
    for (name, date, definition, expected_version) in [
        ("ordinary-date-only", true, false, 2),
        ("ordinary-definition-only", false, true, 2),
        ("ordinary-neither", false, false, 1),
    ] {
        let bytes = &artifacts[&format!("{name}.twd")];
        let receipt: serde_json::Value =
            serde_json::from_slice(&artifacts[&format!("{name}.source.json")]).unwrap();
        assert_eq!(receipt["producer_head"].as_str().unwrap(), candidate_head);
        assert_eq!(receipt["capture_run_id"].as_str().unwrap(), run_id);
        assert_eq!(receipt["kind"].as_str(), Some("ordinary_legacy_export"));
        assert_eq!(receipt["profile"].as_str(), Some(name));
        let document = decode_historical(&bytes);
        check_legacy_control(&document, date, definition);
        let actual_version = if bytes.starts_with(b"TWDPROJ2") {
            2
        } else {
            let entries = framed_files(&bytes);
            let manifest: serde_json::Value = serde_json::from_slice(
                &entries
                    .iter()
                    .find(|(path, _)| path == "manifest.json")
                    .unwrap()
                    .1,
            )
            .unwrap();
            manifest["format_version"].as_u64().unwrap()
        };
        assert_eq!(actual_version, expected_version);
        println!(
            "HISTORICAL_ORDINARY_PASS candidate={candidate_head} profile={name} v{actual_version}"
        );
    }

    for name in [
        "date-definition",
        "date-only",
        "definition-only",
        "neither",
        "legacy-ingress-date-only",
        "legacy-ingress-definition-only",
        "legacy-ingress-neither",
        "boundary-fresh-65536",
    ] {
        let bytes = &artifacts[&format!("{name}.twd")];
        let identity: serde_json::Value =
            serde_json::from_slice(&artifacts[&format!("{name}.source.json")]).unwrap();
        assert_eq!(identity["producer_head"].as_str().unwrap(), candidate_head);
        assert_eq!(identity["capture_run_id"].as_str().unwrap(), run_id);
        assert_eq!(identity["kind"].as_str(), Some("selected_v3_export"));
        assert_eq!(identity["profile"].as_str(), Some(name));
        assert_eq!(identity["document_id"].as_str().unwrap(), DOCUMENT_ID);
        let entries = framed_files(&bytes);
        let paths: Vec<_> = entries.iter().map(|(path, _)| path.as_str()).collect();
        let mut expected = vec!["manifest.json", "schemas.json", "definitions.json"];
        expected.extend([
            "entities/0.jsonl",
            "entities/1.jsonl",
            "entities/2.jsonl",
            "entities/3.jsonl",
            "entities/4.jsonl",
            "entities/5.jsonl",
            "entities/6.jsonl",
            "entities/7.jsonl",
            "entities/8.jsonl",
            "entities/9.jsonl",
            "entities/a.jsonl",
            "entities/b.jsonl",
            "entities/c.jsonl",
            "entities/d.jsonl",
            "entities/e.jsonl",
            "entities/f.jsonl",
        ]);
        assert_eq!(paths, expected);
        let manifest: serde_json::Value = serde_json::from_slice(
            &entries
                .iter()
                .find(|(path, _)| path == "manifest.json")
                .unwrap()
                .1,
        )
        .unwrap();
        assert_eq!(manifest["format_version"].as_u64(), Some(3));
        match CanonicalRoProjectV2::try_from_files(v2_reader_order(&entries)) {
            Err(FormatError::UnsupportedRoProjectVersion {
                found: 3,
                supported: 2,
            }) => {}
            Err(error) => panic!(
                "well-formed fresh v3 capture must reach the old reader's typed version refusal; got {error}"
            ),
            Ok(_) => panic!("old v2 reader unexpectedly admitted a fresh v3 capture"),
        }
        println!(
            "OLD_READER_UNSUPPORTED_V3 candidate={candidate_head} profile={name} verified_candidate_output=true"
        );
    }
}
