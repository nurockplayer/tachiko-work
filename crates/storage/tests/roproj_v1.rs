use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, MAX_EXPRESSION_DEPTH, Number, Schema, SchemaId, SchemaKey, Value,
};
use tachiko_storage::{
    CanonicalRoProjectV1, FormatError, ROPROJ_V1_PATHS, decode_roproj_v1, encode_roproj_v1,
};

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

#[test]
fn full_shape_round_trip_is_exact() {
    let document = full_shape_document();

    let encoded = encode_roproj_v1(&document).unwrap();
    let decoded = decode_roproj_v1(&encoded).unwrap();
    let reencoded = encode_roproj_v1(&decoded).unwrap();

    assert_eq!(decoded, document);
    assert_eq!(reencoded.files(), encoded.files());
    assert!(!encoded.file("entities/6.jsonl").unwrap().is_empty());
    assert!(!encoded.file("entities/b.jsonl").unwrap().is_empty());
}

#[test]
fn canonical_encode_output_always_constructs() {
    for document in [
        Document::empty("doc-empty", "Empty"),
        character_document(),
        full_shape_document(),
    ] {
        let encoded = encode_roproj_v1(&document).unwrap();
        let reconstructed = CanonicalRoProjectV1::try_from_files(owned_files(&encoded)).unwrap();
        assert_eq!(reconstructed, encoded);
    }
}

#[test]
fn canonical_constructor_rejects_path_set_and_order_defects() {
    let encoded = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();

    let mut missing = owned_files(&encoded);
    missing.pop();
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(missing),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut extra = owned_files(&encoded);
    extra.push(("entities/extra.jsonl".to_owned(), Vec::new()));
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(extra),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut duplicate = owned_files(&encoded);
    duplicate[17].0 = duplicate[16].0.clone();
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(duplicate),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut wrong_order = owned_files(&encoded);
    wrong_order.swap(0, 1);
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(wrong_order),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn manifest_dispatch_precedes_invalid_body_data() {
    let encoded = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    let cases = [
        (
            br#"{"format":"tachiko.roproj","document":{"id":"doc-empty","title":"Empty"}}"#
                .as_slice(),
            "missing",
        ),
        (
            br#"{"format":"tachiko.roproj","format_version":1.0,"document":{"id":"doc-empty","title":"Empty"}}"#
                .as_slice(),
            "malformed",
        ),
        (
            br#"{"format":"tachiko.roproj","format_version":2,"document":{"id":"doc-empty","title":"Empty"}}"#
                .as_slice(),
            "unsupported",
        ),
    ];
    for (manifest, expected) in cases {
        let mut files = owned_files(&encoded);
        replace_file(&mut files, "manifest.json", manifest.to_vec());
        replace_file(&mut files, "schemas.json", b"not JSON".to_vec());
        let error = CanonicalRoProjectV1::try_from_files(files).unwrap_err();
        assert!(
            matches!(
                (&error, expected),
                (FormatError::RoProjectVersionMissing, "missing")
                    | (FormatError::RoProjectVersionMalformed, "malformed")
                    | (
                        FormatError::UnsupportedRoProjectVersion {
                            found: 2,
                            supported: 1
                        },
                        "unsupported"
                    )
            ),
            "unexpected {expected} error: {error:?}"
        );
    }
}

#[test]
fn manifest_format_and_json_failures_are_explicit() {
    let encoded = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    let mut missing = owned_files(&encoded);
    replace_file(
        &mut missing,
        "manifest.json",
        br#"{"format_version":1,"document":{"id":"doc-empty","title":"Empty"}}"#.to_vec(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(missing),
        Err(FormatError::RoProjectFormatMissing)
    ));

    let mut malformed = owned_files(&encoded);
    replace_file(
        &mut malformed,
        "manifest.json",
        br#"{"format":"other","format_version":1,"document":{"id":"doc-empty","title":"Empty"}}"#
            .to_vec(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(malformed),
        Err(FormatError::RoProjectFormatMalformed)
    ));

    let mut invalid_json = owned_files(&encoded);
    replace_file(&mut invalid_json, "manifest.json", b"{".to_vec());
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(invalid_json),
        Err(FormatError::InvalidRoProjectJson { .. })
    ));

    let mut invalid_utf8 = owned_files(&encoded);
    replace_file(&mut invalid_utf8, "manifest.json", vec![0xff]);
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(invalid_utf8),
        Err(FormatError::InvalidRoProjectUtf8 { .. })
    ));
}

#[test]
fn recursive_duplicates_unknown_members_and_unknown_tags_are_rejected() {
    let encoded = encode_roproj_v1(&full_shape_document()).unwrap();

    let mut duplicate = owned_files(&encoded);
    let schemas = String::from_utf8(file_bytes(&duplicate, "schemas.json").to_vec()).unwrap();
    replace_file(
        &mut duplicate,
        "schemas.json",
        schemas
            .replacen(
                "    \"key\": \"record\",",
                "    \"key\": \"record\",\n    \"\\u006bey\": \"duplicate\",",
                1,
            )
            .into_bytes(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(duplicate),
        Err(FormatError::DuplicateRoProjectMember { member, .. }) if member == "key"
    ));

    let mut unknown_member = owned_files(&encoded);
    let entity = String::from_utf8(file_bytes(&unknown_member, "entities/6.jsonl").to_vec())
        .unwrap()
        .replacen("\"args\":8}", "\"args\":8,\"extra\":false}", 1);
    replace_file(&mut unknown_member, "entities/6.jsonl", entity.into_bytes());
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(unknown_member),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut unknown_tag = owned_files(&encoded);
    let entity = String::from_utf8(file_bytes(&unknown_tag, "entities/6.jsonl").to_vec())
        .unwrap()
        .replacen("\"kind\":\"boolean\"", "\"kind\":\"mystery\"", 1);
    replace_file(&mut unknown_tag, "entities/6.jsonl", entity.into_bytes());
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(unknown_tag),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn malformed_jsonl_invalid_numbers_and_blank_records_are_rejected() {
    let encoded = encode_roproj_v1(&full_shape_document()).unwrap();

    let mut malformed = owned_files(&encoded);
    replace_file(&mut malformed, "entities/6.jsonl", b"{\n".to_vec());
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(malformed),
        Err(FormatError::InvalidRoProjectJson { .. })
    ));

    let mut invalid_number = owned_files(&encoded);
    let entity = String::from_utf8(file_bytes(&invalid_number, "entities/6.jsonl").to_vec())
        .unwrap()
        .replacen("\"value\":40", "\"value\":1e400", 1);
    replace_file(&mut invalid_number, "entities/6.jsonl", entity.into_bytes());
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(invalid_number),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut blank = owned_files(&encoded);
    replace_file(&mut blank, "entities/0.jsonl", b"\n".to_vec());
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(blank),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut invalid_body_utf8 = owned_files(&encoded);
    replace_file(&mut invalid_body_utf8, "schemas.json", vec![0xff]);
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(invalid_body_utf8),
        Err(FormatError::InvalidRoProjectUtf8 { .. })
    ));
}

#[test]
fn canonical_constructor_rejects_noncanonical_bytes_and_wrong_shards() {
    let encoded = encode_roproj_v1(&full_shape_document()).unwrap();

    let mut missing_final_lf = owned_files(&encoded);
    file_bytes_mut(&mut missing_final_lf, "manifest.json").pop();
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(missing_final_lf),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut extra_final_lf = owned_files(&encoded);
    file_bytes_mut(&mut extra_final_lf, "schemas.json").push(b'\n');
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(extra_final_lf),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut wrong_shard = owned_files(&encoded);
    let record = std::mem::take(file_bytes_mut(&mut wrong_shard, "entities/6.jsonl"));
    *file_bytes_mut(&mut wrong_shard, "entities/0.jsonl") = record;
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(wrong_shard),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut noncanonical_number = owned_files(&encoded);
    let entity = String::from_utf8(file_bytes(&noncanonical_number, "entities/6.jsonl").to_vec())
        .unwrap()
        .replacen("\"value\":40", "\"value\":40.0", 1);
    replace_file(
        &mut noncanonical_number,
        "entities/6.jsonl",
        entity.into_bytes(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(noncanonical_number),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut missing_entity_lf = owned_files(&encoded);
    file_bytes_mut(&mut missing_entity_lf, "entities/6.jsonl").pop();
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(missing_entity_lf),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut extra_entity_lf = owned_files(&encoded);
    file_bytes_mut(&mut extra_entity_lf, "entities/6.jsonl").push(b'\n');
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(extra_entity_lf),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn duplicate_out_of_order_and_empty_ids_are_rejected() {
    let encoded = encode_roproj_v1(&Document::empty("doc-empty", "Empty")).unwrap();
    for schemas in [
        br#"[{"id":"schema-a","key":"a","fields":[]},{"id":"schema-a","key":"b","fields":[]}]
"#
            .to_vec(),
        br#"[{"id":"schema-b","key":"b","fields":[]},{"id":"schema-a","key":"a","fields":[]}]
"#
            .to_vec(),
        "[{\"id\":\"é\",\"key\":\"a\",\"fields\":[]},{\"id\":\"e\u{301}\",\"key\":\"b\",\"fields\":[]}]\n"
            .as_bytes()
            .to_vec(),
        br#"[{"id":"","key":"a","fields":[]}]
"#
            .to_vec(),
    ] {
        let mut files = owned_files(&encoded);
        replace_file(&mut files, "schemas.json", schemas);
        assert!(matches!(
            CanonicalRoProjectV1::try_from_files(files),
            Err(FormatError::InvalidRoProjectRepresentation { .. })
        ));
    }

    let mut field_order = owned_files(&encoded);
    replace_file(
        &mut field_order,
        "schemas.json",
        br#"[{"id":"schema-a","key":"a","fields":[{"id":"field-b","key":"b","field_type":{"type":"number"},"required":false},{"id":"field-a","key":"a","field_type":{"type":"number"},"required":false}]}]
"#
        .to_vec(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(field_order),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));

    let mut same_shard = character_document();
    let mut entity_p = same_shard.entities["entity-a"].clone();
    entity_p.id = EntityId::from("entity-p");
    entity_p.key = "villain".into();
    same_shard
        .entities
        .insert(EntityId::from("entity-p"), entity_p);
    let encoded = encode_roproj_v1(&same_shard).unwrap();
    let mut entity_order = owned_files(&encoded);
    let shard = String::from_utf8(file_bytes(&entity_order, "entities/6.jsonl").to_vec()).unwrap();
    let mut lines = shard.lines().collect::<Vec<_>>();
    lines.reverse();
    replace_file(
        &mut entity_order,
        "entities/6.jsonl",
        format!("{}\n", lines.join("\n")).into_bytes(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(entity_order),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn semantic_type_reference_and_required_field_failures_are_rejected() {
    let encoded = encode_roproj_v1(&full_shape_document()).unwrap();
    let cases = [
        (
            "\"kind\":\"number\",\"value\":40",
            "\"kind\":\"text\",\"value\":\"wrong\"",
            vec!["core.type_mismatch"],
        ),
        (
            "\"kind\":\"reference\",\"value\":\"entity-b\"",
            "\"kind\":\"reference\",\"value\":\"missing\"",
            vec!["core.missing_reference"],
        ),
        (
            "\"field\":\"field-base\"",
            "\"field\":\"field-label\"",
            vec!["core.formula_reference_type_mismatch"],
        ),
    ];
    for (from, to, expected_codes) in cases {
        let mut files = owned_files(&encoded);
        let entity = String::from_utf8(file_bytes(&files, "entities/6.jsonl").to_vec())
            .unwrap()
            .replacen(from, to, 1);
        replace_file(&mut files, "entities/6.jsonl", entity.into_bytes());
        assert_invalid_document_codes(files, &expected_codes);
    }

    let mut missing_required = owned_files(&encoded);
    let entity = String::from_utf8(file_bytes(&missing_required, "entities/6.jsonl").to_vec())
        .unwrap()
        .replacen(
            "\"field-active\":{\"kind\":\"boolean\",\"value\":true},",
            "",
            1,
        );
    replace_file(
        &mut missing_required,
        "entities/6.jsonl",
        entity.into_bytes(),
    );
    assert_invalid_document_codes(missing_required, &["core.missing_required_field"]);

    let mut missing_reference_schema = owned_files(&encoded);
    let schemas = String::from_utf8(file_bytes(&missing_reference_schema, "schemas.json").to_vec())
        .unwrap()
        .replacen(
            "          \"schema\": \"schema-record\"",
            "          \"schema\": \"missing\"",
            1,
        );
    replace_file(
        &mut missing_reference_schema,
        "schemas.json",
        schemas.into_bytes(),
    );
    assert_invalid_document_codes(
        missing_reference_schema,
        &[
            "core.missing_schema",
            "core.reference_type_mismatch",
            "core.reference_type_mismatch",
        ],
    );
}

#[test]
fn formula_structural_limits_are_rejected_before_encoding() {
    let mut document = full_shape_document();
    let mut expression = Expression::Number(Number::new(1.0).unwrap());
    for _ in 0..64 {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
        };
    }
    document
        .entities
        .get_mut("entity-a")
        .unwrap()
        .fields
        .insert(FieldId::from("field-result"), Value::Formula(expression));
    assert!(matches!(
        encode_roproj_v1(&document),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn maximum_expression_depth_round_trips_through_canonical_construction() {
    let mut document = full_shape_document();
    document
        .entities
        .get_mut("entity-a")
        .unwrap()
        .fields
        .insert(
            FieldId::from("field-result"),
            Value::Formula(left_deep_add_expression(MAX_EXPRESSION_DEPTH)),
        );

    let encoded = encode_roproj_v1(&document).unwrap();
    let reconstructed = CanonicalRoProjectV1::try_from_files(owned_files(&encoded)).unwrap();
    assert_eq!(decode_roproj_v1(&reconstructed).unwrap(), document);
}

#[test]
fn expression_depth_above_limit_is_rejected_during_decode() {
    let mut document = full_shape_document();
    document
        .entities
        .get_mut("entity-a")
        .unwrap()
        .fields
        .insert(
            FieldId::from("field-result"),
            Value::Formula(left_deep_add_expression(MAX_EXPRESSION_DEPTH)),
        );
    let encoded = encode_roproj_v1(&document).unwrap();
    let mut files = owned_files(&encoded);
    let entity = String::from_utf8(file_bytes(&files, "entities/6.jsonl").to_vec()).unwrap();
    let admitted = left_deep_add_json(MAX_EXPRESSION_DEPTH);
    let rejected = left_deep_add_json(MAX_EXPRESSION_DEPTH + 1);
    assert!(entity.contains(&admitted));
    replace_file(
        &mut files,
        "entities/6.jsonl",
        entity.replacen(&admitted, &rejected, 1).into_bytes(),
    );
    assert!(matches!(
        CanonicalRoProjectV1::try_from_files(files),
        Err(FormatError::InvalidRoProjectRepresentation { message })
            if message.contains("64-depth limit")
    ));
}

#[test]
fn equivalent_construction_order_has_identical_ordered_files() {
    let document = full_shape_document();
    let mut reversed = document.clone();
    reversed.schemas = reversed.schemas.into_iter().rev().collect();
    reversed.entities = reversed.entities.into_iter().rev().collect();
    for schema in reversed.schemas.values_mut() {
        schema.fields = std::mem::take(&mut schema.fields)
            .into_iter()
            .rev()
            .collect();
    }
    for entity in reversed.entities.values_mut() {
        entity.fields = std::mem::take(&mut entity.fields)
            .into_iter()
            .rev()
            .collect();
    }
    assert_eq!(
        encode_roproj_v1(&document).unwrap().files(),
        encode_roproj_v1(&reversed).unwrap().files()
    );
}

#[test]
fn mutable_key_rename_changes_only_the_containing_record_and_not_its_shard() {
    let before_document = full_shape_document();
    let mut after_document = before_document.clone();
    after_document.entities.get_mut("entity-a").unwrap().key = "renamed".into();
    let before = encode_roproj_v1(&before_document).unwrap();
    let after = encode_roproj_v1(&after_document).unwrap();
    let changed = before
        .files()
        .iter()
        .zip(after.files())
        .filter_map(|(left, right)| (left.bytes() != right.bytes()).then_some(left.path()))
        .collect::<Vec<_>>();
    assert_eq!(changed, vec!["entities/6.jsonl"]);
    assert!(
        after
            .file("entities/6.jsonl")
            .unwrap()
            .windows(8)
            .any(|bytes| bytes == b"renamed\"")
    );
}

#[test]
fn unicode_scalar_sequences_and_published_sha_placement_are_pinned() {
    let document = full_shape_document();
    let encoded = encode_roproj_v1(&document).unwrap();
    let decoded = decode_roproj_v1(&encoded).unwrap();
    assert_eq!(
        decoded.entities["entity-a"].fields["field-label"],
        Value::Text("Caf\u{e9}".to_owned())
    );
    assert_eq!(
        decoded.entities["entity-b"].fields["field-label"],
        Value::Text("Cafe\u{301}".to_owned())
    );
    assert_ne!(
        decoded.entities["entity-a"].fields["field-label"],
        decoded.entities["entity-b"].fields["field-label"]
    );
    assert!(
        encoded
            .file("entities/6.jsonl")
            .unwrap()
            .starts_with(b"{\"id\":\"entity-a\"")
    );
    assert!(
        encoded
            .file("entities/b.jsonl")
            .unwrap()
            .starts_with(b"{\"id\":\"entity-b\"")
    );
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

fn full_shape_document() -> Document {
    let formula = full_shape_formula();
    let fields = BTreeMap::from([
        (
            FieldId::from("field-active"),
            field("field-active", "active", FieldType::Boolean, true),
        ),
        (
            FieldId::from("field-base"),
            field("field-base", "base", FieldType::Number, true),
        ),
        (
            FieldId::from("field-label"),
            field("field-label", "label", FieldType::Text, true),
        ),
        (
            FieldId::from("field-result"),
            field("field-result", "result", FieldType::Number, false),
        ),
        (
            FieldId::from("field-target"),
            field(
                "field-target",
                "target",
                FieldType::Reference {
                    schema: SchemaId::from("schema-record"),
                },
                true,
            ),
        ),
    ]);
    let entity_a = Entity {
        id: EntityId::from("entity-a"),
        key: "alpha".into(),
        schema: SchemaId::from("schema-record"),
        fields: BTreeMap::from([
            (FieldId::from("field-active"), Value::Boolean(true)),
            (
                FieldId::from("field-base"),
                Value::Number(Number::new(40.0).unwrap()),
            ),
            (
                FieldId::from("field-label"),
                Value::Text("Caf\u{e9}".to_owned()),
            ),
            (FieldId::from("field-result"), Value::Formula(formula)),
            (
                FieldId::from("field-target"),
                Value::Reference(EntityId::from("entity-b")),
            ),
        ]),
    };
    let entity_b = Entity {
        id: EntityId::from("entity-b"),
        key: "beta".into(),
        schema: SchemaId::from("schema-record"),
        fields: BTreeMap::from([
            (FieldId::from("field-active"), Value::Boolean(false)),
            (
                FieldId::from("field-base"),
                Value::Number(Number::new(7.0).unwrap()),
            ),
            (
                FieldId::from("field-label"),
                Value::Text("Cafe\u{301}".to_owned()),
            ),
            (
                FieldId::from("field-target"),
                Value::Reference(EntityId::from("entity-a")),
            ),
        ]),
    };

    Document {
        id: "doc-full".into(),
        title: "Composed Caf\u{e9} / decomposed Cafe\u{301}".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema-record"),
            Schema {
                id: SchemaId::from("schema-record"),
                key: SchemaKey::from("record"),
                fields,
            },
        )]),
        entities: BTreeMap::from([
            (EntityId::from("entity-a"), entity_a),
            (EntityId::from("entity-b"), entity_b),
        ]),
    }
}

fn full_shape_formula() -> Expression {
    Expression::Maximum {
        left: Box::new(Expression::Minimum {
            left: Box::new(Expression::Divide {
                left: Box::new(Expression::Multiply {
                    left: Box::new(Expression::Subtract {
                        left: Box::new(Expression::Add {
                            left: Box::new(Expression::Number(Number::new(8.0).unwrap())),
                            right: Box::new(Expression::Reference(FieldRef::new(
                                "entity-a",
                                "field-base",
                            ))),
                        }),
                        right: Box::new(Expression::Number(Number::new(2.0).unwrap())),
                    }),
                    right: Box::new(Expression::Number(Number::new(3.0).unwrap())),
                }),
                right: Box::new(Expression::Number(Number::new(2.0).unwrap())),
            }),
            right: Box::new(Expression::Number(Number::new(100.0).unwrap())),
        }),
        right: Box::new(Expression::Number(Number::new(-100.0).unwrap())),
    }
}

fn left_deep_add_expression(depth: usize) -> Expression {
    let mut expression = Expression::Number(Number::new(1.0).unwrap());
    for _ in 1..depth {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
        };
    }
    expression
}

fn left_deep_add_json(depth: usize) -> String {
    let mut expression = r#"{"op":"number","args":1}"#.to_owned();
    for _ in 1..depth {
        expression = format!(
            r#"{{"op":"add","args":{{"left":{expression},"right":{{"op":"number","args":1}}}}}}"#
        );
    }
    expression
}

fn field(id: &str, key: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(key),
        field_type,
        required,
    }
}

fn owned_files(tree: &CanonicalRoProjectV1) -> Vec<(String, Vec<u8>)> {
    tree.files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect()
}

fn replace_file(files: &mut [(String, Vec<u8>)], path: &str, bytes: Vec<u8>) {
    *file_bytes_mut(files, path) = bytes;
}

fn file_bytes<'a>(files: &'a [(String, Vec<u8>)], path: &str) -> &'a [u8] {
    &files.iter().find(|(name, _)| name == path).unwrap().1
}

fn file_bytes_mut<'a>(files: &'a mut [(String, Vec<u8>)], path: &str) -> &'a mut Vec<u8> {
    &mut files.iter_mut().find(|(name, _)| name == path).unwrap().1
}

fn assert_invalid_document_codes(files: Vec<(String, Vec<u8>)>, expected: &[&str]) {
    let error = CanonicalRoProjectV1::try_from_files(files).unwrap_err();
    let FormatError::InvalidDocument { diagnostics } = error else {
        panic!("expected InvalidDocument, found {error:?}");
    };
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(codes, expected);
}
