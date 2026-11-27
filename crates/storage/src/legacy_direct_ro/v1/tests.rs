use crate::{FormatError, decode_v1_dto_for_migration};

use super::{EntityIdV1, ExpressionV1, FieldIdV1, FieldTypeV1, SchemaIdV1, ValueV1};

#[test]
fn migration_dto_seam_rejects_duplicate_map_keys_before_collapse() {
    let cases = [
        (
            "schema map key",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}},"s":{"id":"s","fields":{}}},"entities":{}}"#,
        ),
        (
            "entity map key",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"e":{"id":"e","schema":"s","fields":{}},"e":{"id":"e","schema":"s","fields":{}}}}"#,
        ),
        (
            "schema field map key",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":false},"n":{"field_type":{"type":"number"},"required":false}}}},"entities":{}}"#,
        ),
        (
            "entity field map key",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"n":{"field_type":{"type":"number"},"required":false}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"n":{"kind":"number","value":1.0},"n":{"kind":"number","value":2.0}}}}}"#,
        ),
        (
            "escaped-equivalent schema map key",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}},"\u0073":{"id":"s","fields":{}}},"entities":{}}"#,
        ),
    ];

    for (name, source) in cases {
        let error = decode_v1_dto_for_migration(source.as_bytes()).unwrap_err();
        assert!(
            matches!(error, FormatError::DuplicateMember { .. }),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn dto_seam_exposes_every_typed_id_occurrence_before_semantic_conversion() {
    let source = r#"{
  "format_version": 1,
  "id": "legacy-doc",
  "title": "Inventory",
  "schemas": {
    "source": {
      "id": "source",
      "fields": {
        "calc": {"field_type": {"type": "number"}, "required": true},
        "link": {"field_type": {"type": "reference", "schema": "target"}, "required": true}
      }
    },
    "target": {
      "id": "target",
      "fields": {
        "number": {"field_type": {"type": "number"}, "required": true}
      }
    }
  },
  "entities": {
    "source-entity": {
      "id": "source-entity",
      "schema": "source",
      "fields": {
        "calc": {"kind": "formula", "value": {"op": "reference", "args": {"entity": "target-entity", "field": "number"}}},
        "link": {"kind": "reference", "value": "target-entity"}
      }
    },
    "target-entity": {
      "id": "target-entity",
      "schema": "target",
      "fields": {"number": {"kind": "number", "value": 1.0}}
    }
  }
}"#;

    let dto = decode_v1_dto_for_migration(source.as_bytes()).unwrap();
    assert_eq!(dto.id.0, "legacy-doc");

    let source_schema = dto.schemas.get(&SchemaIdV1("source".to_owned())).unwrap();
    assert_eq!(source_schema.id.0, "source");
    let link_definition = source_schema
        .fields
        .get(&FieldIdV1("link".to_owned()))
        .unwrap();
    assert!(matches!(
        &link_definition.field_type,
        FieldTypeV1::Reference { schema } if schema.0 == "target"
    ));

    let source_entity = dto
        .entities
        .get(&EntityIdV1("source-entity".to_owned()))
        .unwrap();
    assert_eq!(source_entity.id.0, "source-entity");
    assert_eq!(source_entity.schema.0, "source");
    assert!(matches!(
        source_entity.fields.get(&FieldIdV1("link".to_owned())),
        Some(ValueV1::Reference(entity)) if entity.0 == "target-entity"
    ));
    assert!(matches!(
        source_entity.fields.get(&FieldIdV1("calc".to_owned())),
        Some(ValueV1::Formula(ExpressionV1::Reference(reference)))
            if reference.entity.0 == "target-entity" && reference.field.0 == "number"
    ));
}
