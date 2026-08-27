use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{ROPROJ_V1_PATHS, encode_roproj_v1};

const EXPECTED_EMPTY_MANIFEST: &[u8] = br#"{
  "format": "tachiko.roproj",
  "format_version": 1,
  "document": {
    "id": "doc-empty",
    "title": "Empty"
  }
}
"#;

const EXPECTED_NONEMPTY_SCHEMAS: &[u8] = br#"[
  {
    "id": "schema-character",
    "key": "character",
    "fields": [
      {
        "id": "field-active",
        "key": "active",
        "field_type": {
          "type": "boolean"
        },
        "required": false
      },
      {
        "id": "field-base",
        "key": "base",
        "field_type": {
          "type": "number"
        },
        "required": true
      },
      {
        "id": "field-name",
        "key": "name",
        "field_type": {
          "type": "text"
        },
        "required": true
      },
      {
        "id": "field-note",
        "key": "note",
        "field_type": {
          "type": "text"
        },
        "required": false
      },
      {
        "id": "field-power",
        "key": "power",
        "field_type": {
          "type": "number"
        },
        "required": false
      },
      {
        "id": "field-target",
        "key": "target",
        "field_type": {
          "type": "reference",
          "schema": "schema-character"
        },
        "required": false
      }
    ]
  }
]
"#;

const EXPECTED_CHARACTER_ENTITY: &str = r#"{"id":"entity-a","key":"hero","schema":"schema-character","fields":{"field-active":{"kind":"boolean","value":true},"field-base":{"kind":"number","value":40},"field-name":{"kind":"text","value":"Éowyn"},"field-power":{"kind":"formula","value":{"op":"add","args":{"left":{"op":"number","args":2},"right":{"op":"reference","args":{"entity":"entity-a","field":"field-base"}}}}},"field-target":{"kind":"reference","value":"entity-a"}}}
"#;

#[test]
fn empty_document_emits_the_normative_eighteen_file_tree() {
    let tree = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    assert_eq!(tree.files().len(), 18);
    assert_eq!(tree.file("manifest.json").unwrap(), EXPECTED_EMPTY_MANIFEST);
    assert_eq!(tree.file("schemas.json").unwrap(), b"[]\n");
    for path in ROPROJ_V1_PATHS.iter().skip(2) {
        assert_eq!(tree.file(path).unwrap(), b"");
    }
}

#[test]
fn nonempty_document_emits_the_normative_schema_and_entity_golden_bytes() {
    let tree = encode_roproj_v1(&character_document()).unwrap();

    assert_eq!(EXPECTED_NONEMPTY_SCHEMAS.len(), 1_071);
    assert_eq!(
        tree.file("schemas.json").unwrap(),
        EXPECTED_NONEMPTY_SCHEMAS
    );
    assert_eq!(EXPECTED_CHARACTER_ENTITY.len(), 432);
    assert_eq!(
        tree.file("entities/6.jsonl").unwrap(),
        EXPECTED_CHARACTER_ENTITY.as_bytes()
    );
    assert!(EXPECTED_CHARACTER_ENTITY.ends_with('\n'));
    for path in ROPROJ_V1_PATHS
        .iter()
        .skip(2)
        .filter(|path| **path != "entities/6.jsonl")
    {
        assert_eq!(tree.file(path).unwrap(), b"");
    }
}

fn character_document() -> Document {
    Document {
        id: "doc-character".into(),
        title: "Characters".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema-character"),
            Schema {
                id: SchemaId::from("schema-character"),
                key: SchemaKey::from("character"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("field-active"),
                        field("field-active", "active", FieldType::Boolean, false),
                    ),
                    (
                        FieldId::from("field-base"),
                        field("field-base", "base", FieldType::Number, true),
                    ),
                    (
                        FieldId::from("field-name"),
                        field("field-name", "name", FieldType::Text, true),
                    ),
                    (
                        FieldId::from("field-note"),
                        field("field-note", "note", FieldType::Text, false),
                    ),
                    (
                        FieldId::from("field-power"),
                        field("field-power", "power", FieldType::Number, false),
                    ),
                    (
                        FieldId::from("field-target"),
                        field(
                            "field-target",
                            "target",
                            FieldType::Reference {
                                schema: SchemaId::from("schema-character"),
                            },
                            false,
                        ),
                    ),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity-a"),
            Entity {
                id: EntityId::from("entity-a"),
                key: "hero".into(),
                schema: SchemaId::from("schema-character"),
                fields: BTreeMap::from([
                    (FieldId::from("field-active"), Value::Boolean(true)),
                    (
                        FieldId::from("field-base"),
                        Value::Number(Number::new(40.0).unwrap()),
                    ),
                    (FieldId::from("field-name"), Value::Text("Éowyn".to_owned())),
                    (
                        FieldId::from("field-power"),
                        Value::Formula(Expression::Add {
                            left: Box::new(Expression::Number(Number::new(2.0).unwrap())),
                            right: Box::new(Expression::Reference(FieldRef::new(
                                "entity-a",
                                "field-base",
                            ))),
                        }),
                    ),
                    (
                        FieldId::from("field-target"),
                        Value::Reference(EntityId::from("entity-a")),
                    ),
                ]),
            },
        )]),
    }
}

fn field(id: &str, key: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(key),
        field_type,
        required,
    }
}
