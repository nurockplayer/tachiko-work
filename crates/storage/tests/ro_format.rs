use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, FieldDefinition, FieldId, FieldType, Schema, SchemaId,
    Value,
};
use tachiko_storage::{FORMAT_VERSION, FormatError, from_str, load, save, to_canonical_string};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

fn document_with_order(reverse: bool) -> Document {
    let field_entries = [
        (
            FieldId::from("damage"),
            FieldDefinition {
                field_type: FieldType::Number,
                required: true,
            },
        ),
        (
            FieldId::from("name"),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
            },
        ),
    ];
    let value_entries = [
        (FieldId::from("damage"), Value::Number(100.0)),
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
                fields,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("sword"),
            Entity {
                id: EntityId::from("sword"),
                schema: SchemaId::from("weapon"),
                fields: values,
            },
        )]),
    }
}

#[test]
fn valid_document_round_trips() {
    let document = document_with_order(false);

    let encoded = to_canonical_string(&document).unwrap();
    let decoded = from_str(&encoded).unwrap();

    assert_eq!(decoded, document);
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
            supported: FORMAT_VERSION
        } if found == FORMAT_VERSION + 1
    ));
}

#[test]
fn unknown_document_fields_are_rejected() {
    let encoded = to_canonical_string(&document_with_order(false)).unwrap();
    let with_unknown = encoded.replacen('{', "{\n  \"spreadsheet_mode\": true,", 1);

    let error = from_str(&with_unknown).unwrap_err();

    assert!(matches!(error, FormatError::Json(_)));
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
