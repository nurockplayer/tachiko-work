use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Date, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldType,
    KeyedGroupedSumDefinition, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, Schema, Value,
};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, decode_roproj_v3, encode_roproj_v1, encode_roproj_v2,
    encode_roproj_v3, migrate_roproj_v1_to_v2, migrate_roproj_v1_to_v3, migrate_roproj_v2_to_v3,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn fixture() -> Document {
    let mut document = Document::empty("doc-v3", "V3 acceptance");
    let fields = [
        ("date", FieldType::Date, FieldConstraint::None),
        (
            "formula",
            FieldType::Number,
            FieldConstraint::NumberInclusiveRange {
                min: number(0.0),
                max: number(10.0),
            },
        ),
        (
            "number",
            FieldType::Number,
            FieldConstraint::NumberInclusiveRange {
                min: number(0.0),
                max: number(10.0),
            },
        ),
        (
            "text",
            FieldType::Text,
            FieldConstraint::TextLiteralSet {
                values: vec![String::new(), "e\u{301}".into(), "é".into(), "封筒".into()],
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
        "entity-a".into(),
        Entity {
            id: "entity-a".into(),
            key: "item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([
                ("date".into(), Value::Date(Date::new(2024, 2, 29).unwrap())),
                (
                    "formula".into(),
                    Value::Formula(Expression::Number(number(5.0))),
                ),
                ("number".into(), Value::Number(number(10.0))),
                ("text".into(), Value::Text("封筒".into())),
            ]),
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

fn replace(
    tree: &CanonicalRoProjectV3,
    path: &str,
    old: &str,
    new: &str,
) -> Vec<(String, Vec<u8>)> {
    let mut candidate = files(tree);
    let bytes = &mut candidate
        .iter_mut()
        .find(|(name, _)| name == path)
        .unwrap()
        .1;
    let source = String::from_utf8(bytes.clone()).unwrap();
    assert!(source.contains(old), "missing mutation anchor: {old}");
    *bytes = source.replacen(old, new, 1).into_bytes();
    candidate
}

#[test]
fn canonical_v3_has_exact_nineteen_files_and_normative_empty_bytes() {
    let tree = encode_roproj_v3(&Document::empty("doc-empty", "Empty")).unwrap();
    let expected: Vec<String> = ["manifest.json", "schemas.json", "definitions.json"]
        .into_iter()
        .map(str::to_owned)
        .chain(
            "0123456789abcdef"
                .chars()
                .map(|digit| format!("entities/{digit}.jsonl")),
        )
        .collect();
    assert_eq!(
        tree.files()
            .iter()
            .map(tachiko_storage::CanonicalRoProjectFileV3::path)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        tree.file("manifest.json").unwrap(),
        br#"{
  "format": "tachiko.roproj",
  "format_version": 3,
  "document": {
    "id": "doc-empty",
    "title": "Empty"
  }
}
"#
    );
    assert_eq!(tree.file("schemas.json").unwrap(), b"[]\n");
    assert_eq!(tree.file("definitions.json").unwrap(), b"[]\n");
    for path in expected.iter().skip(3) {
        assert_eq!(tree.file(path).unwrap(), b"");
    }
}

#[test]
// The fixed wire oracle deliberately distinguishes decomposed and composed Unicode.
#[allow(clippy::unicode_not_nfc)]
fn field_member_order_and_constraint_bytes_are_fixed() {
    let mut document = fixture();
    document.entities.clear();
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .retain(|id, _| id.as_str() == "text");
    let tree = encode_roproj_v3(&document).unwrap();
    assert_eq!(
        tree.file("schemas.json").unwrap(),
        r#"[
  {
    "id": "schema",
    "key": "items",
    "fields": [
      {
        "id": "text",
        "key": "text",
        "field_type": {
          "type": "text"
        },
        "required": false,
        "constraint": {
          "type": "text_literal_set",
          "values": [
            "",
            "é",
            "é",
            "封筒"
          ]
        }
      }
    ]
  }
]
"#
        .as_bytes()
    );
}

#[test]
fn none_and_number_range_tags_have_fixed_version_owned_bytes() {
    for (field, expected) in [
        (
            "date",
            r#"[
  {
    "id": "schema",
    "key": "items",
    "fields": [
      {
        "id": "date",
        "key": "date",
        "field_type": {
          "type": "date"
        },
        "required": false,
        "constraint": {
          "type": "none"
        }
      }
    ]
  }
]
"#,
        ),
        (
            "number",
            r#"[
  {
    "id": "schema",
    "key": "items",
    "fields": [
      {
        "id": "number",
        "key": "number",
        "field_type": {
          "type": "number"
        },
        "required": false,
        "constraint": {
          "type": "number_inclusive_range",
          "min": 0,
          "max": 10
        }
      }
    ]
  }
]
"#,
        ),
    ] {
        let mut document = fixture();
        document.entities.clear();
        document
            .schemas
            .get_mut("schema")
            .unwrap()
            .fields
            .retain(|id, _| id.as_str() == field);
        let tree = encode_roproj_v3(&document).unwrap();
        assert_eq!(tree.file("schemas.json").unwrap(), expected.as_bytes());
    }
}

#[test]
fn all_constraint_tags_date_formulas_and_stable_ids_roundtrip_without_reinterpretation() {
    let original = fixture();
    assert!(tachiko_semantic_core::validate_document(&original).is_empty());
    let tree = encode_roproj_v3(&original).unwrap();
    let decoded = decode_roproj_v3(&tree).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(encode_roproj_v3(&decoded).unwrap(), tree);
    assert_eq!(
        CanonicalRoProjectV3::try_from_files(files(&tree)).unwrap(),
        tree
    );
    let entity = tree.file("entities/6.jsonl").unwrap();
    assert_eq!(entity, "{\"id\":\"entity-a\",\"key\":\"item\",\"schema\":\"schema\",\"fields\":{\"date\":{\"kind\":\"date\",\"value\":\"2024-02-29\"},\"formula\":{\"kind\":\"formula\",\"value\":{\"op\":\"number\",\"args\":5}},\"number\":{\"kind\":\"number\",\"value\":10},\"text\":{\"kind\":\"text\",\"value\":\"封筒\"}}}\n".as_bytes());
}

#[test]
fn closed_constraints_dates_and_canonical_bytes_are_rejected_without_repair() {
    let tree = encode_roproj_v3(&fixture()).unwrap();
    for (path, old, new) in [
        (
            "schemas.json",
            "        \"required\": false,\n        \"constraint\": {\n          \"type\": \"none\"\n        }",
            "        \"required\": false",
        ),
        (
            "schemas.json",
            "        \"constraint\": {\n          \"type\": \"none\"\n        }",
            "        \"constraint\": null",
        ),
        (
            "schemas.json",
            "\"constraint\": {",
            "\"unknown_constraint\": {",
        ),
        (
            "schemas.json",
            "\"type\": \"none\"",
            "\"type\": \"none\", \"extra\": true",
        ),
        (
            "schemas.json",
            "\"type\": \"none\"",
            "\"type\": \"unknown\"",
        ),
        (
            "schemas.json",
            "\"type\": \"none\"",
            "\"type\": \"none\", \"type\": \"none\"",
        ),
        ("schemas.json", "\"min\": 0", "\"min\": null"),
        ("schemas.json", "\"min\": 0", "\"min\": 11"),
        (
            "schemas.json",
            "\"e\u{301}\",\n            \"é\"",
            "\"é\",\n            \"e\u{301}\"",
        ),
        (
            "schemas.json",
            "\"e\u{301}\",\n            \"é\"",
            "\"é\",\n            \"é\"",
        ),
        ("entities/6.jsonl", "2024-02-29", "2023-02-29"),
        ("entities/6.jsonl", "2024-02-29", "0000-01-01"),
        ("entities/6.jsonl", "2024-02-29", "2024-2-29"),
        ("entities/6.jsonl", "\"value\":10", "\"value\":11"),
        ("entities/6.jsonl", "封筒", "other"),
        (
            "manifest.json",
            "\"format_version\": 3",
            "\"format_version\": 3.0",
        ),
    ] {
        assert!(
            CanonicalRoProjectV3::try_from_files(replace(&tree, path, old, new)).is_err(),
            "admitted {path}: {new}"
        );
    }
    let mut missing = files(&tree);
    missing.retain(|(path, _)| path != "entities/f.jsonl");
    assert!(CanonicalRoProjectV3::try_from_files(missing).is_err());
    let mut extra = files(&tree);
    extra.push(("views.json".into(), b"{}\n".to_vec()));
    assert!(CanonicalRoProjectV3::try_from_files(extra).is_err());
    let mut reordered = files(&tree);
    reordered.swap(0, 1);
    assert!(CanonicalRoProjectV3::try_from_files(reordered).is_err());
    let mut reversed = files(&tree);
    reversed.reverse();
    assert!(CanonicalRoProjectV3::try_from_files(reversed).is_err());
    assert!(
        CanonicalRoProjectV3::try_from_files(replace(
            &tree,
            "manifest.json",
            "  \"format\"",
            " \"format\""
        ))
        .is_err()
    );
}

#[test]
fn wrong_version_dispatch_precedes_schema_interpretation() {
    let tree = encode_roproj_v3(&fixture()).unwrap();
    for version in [1, 2, 4] {
        let mut candidate = replace(
            &tree,
            "manifest.json",
            "\"format_version\": 3",
            &format!("\"format_version\": {version}"),
        );
        candidate
            .iter_mut()
            .find(|(name, _)| name == "schemas.json")
            .unwrap()
            .1 = b"not JSON".to_vec();
        assert!(matches!(CanonicalRoProjectV3::try_from_files(candidate),
            Err(FormatError::UnsupportedRoProjectVersion { found, supported: 3 }) if found == version));
    }
}

fn legacy_fixture() -> Document {
    let mut document = fixture();
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .remove("date");
    document
        .entities
        .get_mut("entity-a")
        .unwrap()
        .fields
        .remove("date");
    for field in document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .values_mut()
    {
        field.constraint = FieldConstraint::None;
    }
    document
}

#[test]
fn explicit_migrations_preserve_meaning_and_the_existing_v1_edge() {
    let document = legacy_fixture();
    let v1 = encode_roproj_v1(&document).unwrap();
    let v2 = migrate_roproj_v1_to_v2(&v1).unwrap();
    let v1_before = v1.clone();
    let v2_before = v2.clone();
    let migrated = migrate_roproj_v2_to_v3(&v2).unwrap();
    assert_eq!(migrate_roproj_v1_to_v3(&v1).unwrap(), migrated);
    assert_eq!(decode_roproj_v3(&migrated).unwrap(), document);
    assert_eq!(migrate_roproj_v2_to_v3(&v2).unwrap(), migrated);
    assert_eq!(v1, v1_before);
    assert_eq!(v2, v2_before);
    assert!(
        decode_roproj_v3(&migrated)
            .unwrap()
            .schemas
            .values()
            .flat_map(|schema| schema.fields.values())
            .all(|field| field.constraint == FieldConstraint::None)
    );
}

#[test]
fn v2_saved_definition_bindings_survive_explicit_conversion() {
    let mut document = legacy_fixture();
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
    let v2 = encode_roproj_v2(&document).unwrap();
    let v3 = migrate_roproj_v2_to_v3(&v2).unwrap();
    assert_eq!(v3.file("definitions.json"), v2.file("definitions.json"));
    assert_eq!(decode_roproj_v3(&v3).unwrap(), document);
}

#[test]
fn writer_checks_shared_semantics_and_preserves_date_extremes() {
    let mut candidate = fixture();
    candidate
        .entities
        .get_mut("entity-a")
        .unwrap()
        .fields
        .insert("number".into(), Value::Number(number(11.0)));
    assert!(matches!(
        encode_roproj_v3(&candidate),
        Err(FormatError::InvalidDocument { .. })
    ));
    candidate
        .entities
        .get_mut("entity-a")
        .unwrap()
        .fields
        .remove("number");
    for date in [
        Date::new(1, 1, 1).unwrap(),
        Date::new(9999, 12, 31).unwrap(),
    ] {
        candidate
            .entities
            .get_mut("entity-a")
            .unwrap()
            .fields
            .insert("date".into(), Value::Date(date));
        assert_eq!(
            decode_roproj_v3(&encode_roproj_v3(&candidate).unwrap()).unwrap(),
            candidate
        );
    }
}
