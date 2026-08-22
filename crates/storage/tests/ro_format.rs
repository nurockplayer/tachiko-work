use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, FieldDefinition, FieldId, FieldKey, FieldType, Number,
    Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{
    FORMAT_VERSION, FormatError, V2_MAX_INPUT_BYTES, from_bytes, from_str, load, save,
    to_canonical_string,
};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

fn document_with_order(reverse: bool) -> Document {
    let field_entries = [
        (
            FieldId::from("damage"),
            FieldDefinition {
                id: FieldId::from("damage"),
                key: FieldKey::from("damage"),
                field_type: FieldType::Number,
                required: true,
            },
        ),
        (
            FieldId::from("name"),
            FieldDefinition {
                id: FieldId::from("name"),
                key: FieldKey::from("name"),
                field_type: FieldType::Text,
                required: true,
            },
        ),
    ];
    let value_entries = [
        (
            FieldId::from("damage"),
            Value::Number(Number::new(100.0).unwrap()),
        ),
        (FieldId::from("name"), Value::Text("Sword".to_owned())),
    ];

    let fields = if reverse {
        field_entries.into_iter().rev().collect()
    } else {
        field_entries.into_iter().collect()
    };
    let values = if reverse {
        value_entries.into_iter().rev().collect()
    } else {
        value_entries.into_iter().collect()
    };

    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                id: SchemaId::from("weapon"),
                key: SchemaKey::from("weapon"),
                fields,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("sword"),
            Entity {
                id: EntityId::from("sword"),
                key: "sword".into(),
                schema: SchemaId::from("weapon"),
                fields: values,
            },
        )]),
    }
}

fn document_with_canonical_size(bytes: usize) -> Document {
    let mut document = document_with_order(false);
    document.title.clear();
    document.title.push('x');
    let fixed_bytes = to_canonical_string(&document).unwrap().len();
    assert!(bytes >= fixed_bytes);
    document.title = "x".repeat(1 + bytes - fixed_bytes);
    document
}

#[test]
fn valid_document_round_trips() {
    let document = document_with_order(false);

    let encoded = to_canonical_string(&document).unwrap();
    let decoded = from_str(&encoded).unwrap();

    assert_eq!(decoded, document);
}

#[test]
fn v2_writer_admits_the_exact_input_boundary_and_round_trips() {
    let document = document_with_canonical_size(V2_MAX_INPUT_BYTES);

    let encoded = to_canonical_string(&document).unwrap();

    assert_eq!(encoded.len(), V2_MAX_INPUT_BYTES);
    assert_eq!(from_bytes(encoded.as_bytes()).unwrap(), document);
}

#[test]
fn v2_writer_rejects_canonical_output_one_byte_over_the_input_limit() {
    let document = document_with_canonical_size(V2_MAX_INPUT_BYTES + 1);

    let error = match to_canonical_string(&document) {
        Err(error) => error,
        Ok(encoded) => panic!(
            "writer admitted {} bytes, expected a {}-byte limit",
            encoded.len(),
            V2_MAX_INPUT_BYTES
        ),
    };

    assert!(matches!(
        error,
        FormatError::ResourceLimit {
            resource: "input",
            limit: V2_MAX_INPUT_BYTES,
            actual,
        } if actual == V2_MAX_INPUT_BYTES + 1
    ));
}

#[test]
fn save_rejects_oversized_v2_before_creating_the_destination() {
    let path = temporary_file("oversized-v2.ro");
    let document = document_with_canonical_size(V2_MAX_INPUT_BYTES + 1);

    let result = save(&path, &document);
    let destination_was_created = path.exists();
    if destination_was_created {
        fs::remove_file(&path).unwrap();
    }
    let Err(error) = result else {
        panic!("save admitted an oversized v2 document");
    };

    assert!(!destination_was_created);
    assert!(matches!(
        error,
        FormatError::ResourceLimit {
            resource: "input",
            limit: V2_MAX_INPUT_BYTES,
            actual,
        } if actual == V2_MAX_INPUT_BYTES + 1
    ));
}

#[test]
fn equivalent_documents_have_identical_canonical_bytes() {
    let forward = to_canonical_string(&document_with_order(false)).unwrap();
    let reverse = to_canonical_string(&document_with_order(true)).unwrap();

    assert_eq!(forward, reverse);
    assert!(forward.ends_with('\n'));
    assert!(!forward.ends_with("\n\n"));
}

#[test]
fn unsupported_version_is_explicit() {
    let encoded = to_canonical_string(&document_with_order(false)).unwrap();
    let encoded = encoded.replacen(
        &format!("\"format_version\": {FORMAT_VERSION}"),
        &format!("\"format_version\": {}", FORMAT_VERSION + 1),
        1,
    );

    let error = from_str(&encoded).unwrap_err();

    assert!(matches!(
        error,
        FormatError::UnsupportedVersion {
            found,
            supported
        } if found == FORMAT_VERSION + 1
            && supported == FORMAT_VERSION
    ));
}

#[test]
fn unknown_document_fields_are_rejected() {
    let encoded = to_canonical_string(&document_with_order(false)).unwrap();
    let with_unknown = encoded.replacen('{', "{\n  \"spreadsheet_mode\": true,", 1);

    let error = from_str(&with_unknown).unwrap_err();

    assert!(matches!(error, FormatError::InvalidRepresentation { .. }));
}

#[test]
fn invalid_semantic_content_cannot_be_serialized() {
    let mut document = document_with_order(false);
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .remove("name");

    let error = to_canonical_string(&document).unwrap_err();

    assert!(matches!(
        error,
        FormatError::InvalidDocument { ref diagnostics }
            if diagnostics.iter().any(|diagnostic| diagnostic.path == "entities.sword.fields.name")
    ));
}

#[test]
fn invalid_v2_relationship_cannot_be_serialized() {
    let document = Document {
        id: DocumentId::from("doc"),
        title: "Document".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("source"),
            Schema {
                id: SchemaId::from("source"),
                key: SchemaKey::from("source"),
                fields: BTreeMap::from([(
                    FieldId::from("target"),
                    FieldDefinition {
                        id: FieldId::from("target"),
                        key: FieldKey::from("target"),
                        field_type: FieldType::Reference {
                            schema: SchemaId::from("missing"),
                        },
                        required: false,
                    },
                )]),
            },
        )]),
        entities: BTreeMap::new(),
    };

    let error = to_canonical_string(&document).unwrap_err();

    assert!(matches!(error, FormatError::InvalidDocument { .. }));
}

#[test]
fn path_helpers_preserve_the_canonical_document() {
    let path = temporary_file("roundtrip.ro");
    let document = document_with_order(false);

    save(&path, &document).unwrap();
    let loaded = load(&path).unwrap();

    assert_eq!(loaded, document);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        to_canonical_string(&document).unwrap()
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn save_refuses_to_overwrite_an_existing_file() {
    let path = temporary_file("existing.ro");
    fs::write(&path, "preserve me").unwrap();

    let error = save(&path, &document_with_order(false)).unwrap_err();

    assert!(matches!(
        error,
        FormatError::AlreadyExists { path: ref error_path } if error_path == &path
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), "preserve me");
    fs::remove_file(path).unwrap();
}

fn temporary_file(suffix: &str) -> PathBuf {
    let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "tachiko-storage-{}-{sequence}-{suffix}",
        std::process::id()
    ))
}
