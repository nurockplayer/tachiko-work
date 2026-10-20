//! ADR-0045: sparse values and requiredness use the existing v3/package codecs.
use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, FieldConstraint, FieldDefinition, FieldType, Number, Schema, Value,
};
use tachiko_storage::{
    decode_portable_package_v1, decode_roproj_v3, encode_portable_package_v1, encode_roproj_v3,
};

fn fixture(sole: bool) -> Document {
    let mut document = Document::empty("optional", "Optional native fields");
    let mut fields = BTreeMap::new();
    for (id, field_type, required) in [
        ("note", FieldType::Text, false),
        ("amount", FieldType::Number, false),
        ("flag", FieldType::Boolean, true),
    ] {
        if sole && id != "note" {
            continue;
        }
        fields.insert(
            id.into(),
            FieldDefinition {
                id: id.into(),
                key: id.into(),
                display_name: None,
                field_type,
                required,
                constraint: FieldConstraint::None,
            },
        );
    }
    document.schemas.insert(
        "items".into(),
        Schema {
            id: "items".into(),
            key: "items".into(),
            display_name: None,
            fields,
        },
    );
    for (id, note, amount) in [
        ("absent", None, None),
        ("empty", Some(""), Some(0.0)),
        ("present", Some("memo"), Some(7.0)),
    ] {
        let mut fields = BTreeMap::new();
        if let Some(note) = note {
            fields.insert("note".into(), Value::Text(note.into()));
        }
        if !sole {
            fields.insert("flag".into(), Value::Boolean(false));
            if let Some(amount) = amount {
                fields.insert("amount".into(), Value::Number(Number::new(amount).unwrap()));
            }
        }
        document.entities.insert(
            id.into(),
            Entity {
                id: id.into(),
                key: id.into(),
                schema: "items".into(),
                fields,
            },
        );
    }
    document
}

fn assert_roundtrip(document: &Document) {
    assert_eq!(
        tachiko_semantic_core::validate_document(document),
        [] as [tachiko_semantic_core::Diagnostic; 0]
    );
    let tree = encode_roproj_v3(document).unwrap();
    let decoded = decode_roproj_v3(&tree).unwrap();
    assert_eq!(&decoded, document);
    assert_eq!(encode_roproj_v3(&decoded).unwrap(), tree);
    let bytes = encode_portable_package_v1(&tree).unwrap();
    let package = decode_portable_package_v1(&bytes).unwrap();
    assert_eq!(package.tree(), &tree);
    assert_eq!(decode_roproj_v3(package.tree()).unwrap(), *document);
    assert_eq!(encode_portable_package_v1(package.tree()).unwrap(), bytes);
    // Inspect the representation itself: absent slots have no fact, while
    // empty Text, zero and false remain ordinary present values.
    let entities: Vec<serde_json::Value> = tree
        .files()
        .iter()
        .filter(|file| file.path().starts_with("entities/"))
        .flat_map(|file| std::str::from_utf8(file.bytes()).unwrap().lines())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let absent = entities
        .iter()
        .find(|entity| entity["id"] == "absent")
        .unwrap();
    assert_eq!(
        absent["fields"].get("note").is_none(),
        !document.entities["absent"].fields.contains_key("note")
    );
    assert!(absent["fields"].get("amount").is_none());
    assert_eq!(
        decoded.entities["empty"].fields["note"],
        Value::Text(String::new())
    );
    if document.entities["empty"].fields.contains_key("amount") {
        assert_eq!(
            decoded.entities["empty"].fields["amount"],
            Value::Number(Number::new(0.0).unwrap())
        );
        assert_eq!(
            decoded.entities["empty"].fields["flag"],
            Value::Boolean(false)
        );
    }
}

#[test]
fn v3_and_package_preserve_mixed_sparse_and_sole_optional_columns() {
    for sole in [false, true] {
        assert_roundtrip(&fixture(sole));
    }
}

#[test]
fn requiredness_roundtrips_independently_of_values_after_strengthen_and_relax() {
    let mut document = fixture(false);
    document
        .entities
        .get_mut("absent")
        .unwrap()
        .fields
        .insert("note".into(), Value::Text("filled".into()));
    let values = document.entities.clone();
    for required in [true, false] {
        document
            .schemas
            .get_mut("items")
            .unwrap()
            .fields
            .get_mut("note")
            .unwrap()
            .required = required;
        assert_roundtrip(&document);
        assert_eq!(document.entities, values);
    }
}
