use std::fs;

use tachiko_semantic_core::{Expression, FieldType, Number, Value};
use tachiko_storage::{
    FORMAT_VERSION, FormatError, V2_MAX_INPUT_BYTES, V2_MAX_NUMBER_TOKEN_BYTES, from_bytes,
    from_str, load, to_canonical_string,
};

const LEGACY_GRAPH: &str = r#"{
  "format_version": 1,
  "id": "legacy-doc",
  "title": "Cafe\u0301 inventory",
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
      "fields": {"number": {"kind": "number", "value": 0.000001}}
    }
  }
}"#;

fn v2_number_source(number: &str, title: &str) -> String {
    let title = serde_json::to_string(title).unwrap();
    r#"{"format_version":2,"id":"doc","title":TITLE,"schemas":{"schema":{"id":"schema","key":"schema","fields":{"number":{"id":"number","key":"number","field_type":{"type":"number"},"required":true}}}},"entities":{"entity":{"id":"entity","key":"entity","schema":"schema","fields":{"number":{"kind":"number","value":NUMBER}}}}}"#
        .replace("TITLE", &title)
        .replace("NUMBER", number)
}

#[test]
fn v1_migration_rewrites_all_twelve_id_occurrences_from_complete_maps() {
    let document = from_bytes(LEGACY_GRAPH.as_bytes()).unwrap();

    let document_id = "1213a728-1f70-5425-a330-20a8797f5e82";
    let source_schema_id = "ff71fea8-d907-5234-a6be-819f6e6fdf07";
    let target_schema_id = "973e1ae1-72e3-588b-b1ed-0c66d85aba41";
    let calc_field_id = "32c7bf4d-e5e4-5ea0-ab43-0d42c6878cce";
    let link_field_id = "12e89225-43f4-579e-9d06-53f88b895ab0";
    let number_field_id = "c1789012-4f4f-5da3-983f-731ab96b20f4";
    let source_entity_id = "1832624c-a6ad-55fb-b96a-8617af123e7f";
    let target_entity_id = "0e969c64-fd36-56ca-8b78-3abb9ca821a1";

    assert_eq!(document.id.as_str(), document_id);
    let source_schema = &document.schemas[source_schema_id];
    assert_eq!(source_schema.id.as_str(), source_schema_id);
    assert_eq!(source_schema.key.as_str(), "source");
    assert_eq!(
        source_schema.fields[calc_field_id].id.as_str(),
        calc_field_id
    );
    assert_eq!(source_schema.fields[calc_field_id].key.as_str(), "calc");
    assert_eq!(
        source_schema.fields[link_field_id].id.as_str(),
        link_field_id
    );
    assert!(matches!(
        source_schema.fields[link_field_id].field_type,
        FieldType::Reference { ref schema } if schema.as_str() == target_schema_id
    ));
    let target_schema = &document.schemas[target_schema_id];
    assert_eq!(target_schema.fields[number_field_id].key.as_str(), "number");

    let source_entity = &document.entities[source_entity_id];
    assert_eq!(source_entity.id.as_str(), source_entity_id);
    assert_eq!(source_entity.key.as_str(), "source-entity");
    assert_eq!(source_entity.schema.as_str(), source_schema_id);
    assert!(matches!(
        source_entity.fields[link_field_id],
        Value::Reference(ref entity) if entity.as_str() == target_entity_id
    ));
    assert!(matches!(
        source_entity.fields[calc_field_id],
        Value::Formula(Expression::Reference(ref reference))
            if reference.entity.as_str() == target_entity_id
                && reference.field.as_str() == number_field_id
    ));
    let target_entity = &document.entities[target_entity_id];
    assert_eq!(target_entity.schema.as_str(), target_schema_id);
    assert_eq!(
        target_entity.fields[number_field_id],
        Value::Number(Number::new(0.000_001).unwrap())
    );
}

#[test]
fn migration_and_v2_materialization_are_repeatable_and_byte_stable() {
    assert_eq!(FORMAT_VERSION, 2);
    let first = from_bytes(LEGACY_GRAPH.as_bytes()).unwrap();
    let second = from_bytes(LEGACY_GRAPH.as_bytes()).unwrap();
    assert_eq!(first, second);

    let first_bytes = to_canonical_string(&first).unwrap();
    let second_bytes = to_canonical_string(&second).unwrap();
    assert_eq!(first_bytes, second_bytes);
    assert!(first_bytes.starts_with("{\n  \"format_version\": 2,"));
    assert!(first_bytes.contains("\"value\": 0.000001"));
    assert_eq!(from_bytes(first_bytes.as_bytes()).unwrap(), first);
    assert_eq!(
        to_canonical_string(&from_bytes(first_bytes.as_bytes()).unwrap()).unwrap(),
        first_bytes
    );
}

#[test]
fn reading_legacy_v1_never_rewrites_the_durable_source() {
    let directory = std::env::temp_dir().join(format!(
        "tachiko-identity-migration-v2-{}",
        std::process::id()
    ));
    let path = directory.join("legacy.ro");
    fs::create_dir_all(&directory).unwrap();
    fs::write(&path, LEGACY_GRAPH).unwrap();

    let _document = load(&path).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), LEGACY_GRAPH);
    fs::remove_file(&path).unwrap();
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn v2_collection_order_uses_stable_ids_not_mutable_keys() {
    let mut document = from_bytes(LEGACY_GRAPH.as_bytes()).unwrap();
    document.entities.values_mut().next().unwrap().key = "zzzz".into();
    document.entities.values_mut().next_back().unwrap().key = "aaaa".into();

    let encoded = to_canonical_string(&document).unwrap();
    let first_id = document.entities.keys().next().unwrap().as_str();
    let last_id = document.entities.keys().next_back().unwrap().as_str();

    assert!(encoded.find(first_id).unwrap() < encoded.find(last_id).unwrap());
}

#[test]
fn v2_binary64_reader_and_writer_match_the_accepted_numeric_vectors() {
    for (input, expected_bits, canonical) in [
        ("0", 0x0000_0000_0000_0000, "0"),
        ("-0", 0x0000_0000_0000_0000, "0"),
        ("5e-324", 0x0000_0000_0000_0001, "5e-324"),
        ("-5e-324", 0x8000_0000_0000_0001, "-5e-324"),
        (
            "1.7976931348623157e308",
            0x7fef_ffff_ffff_ffff,
            "1.7976931348623157e+308",
        ),
        (
            "-1.7976931348623157e308",
            0xffef_ffff_ffff_ffff,
            "-1.7976931348623157e+308",
        ),
        ("1e21", 0x444b_1ae4_d6e2_ef50, "1e+21"),
        ("1e-6", 0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
        (
            "9007199254740993",
            0x4340_0000_0000_0000,
            "9007199254740992",
        ),
        ("1e-4000", 0x0000_0000_0000_0000, "0"),
        ("-1e-4000", 0x0000_0000_0000_0000, "0"),
    ] {
        let document = from_str(&v2_number_source(input, "Document")).unwrap();
        let Value::Number(number) = document.entities["entity"].fields["number"] else {
            panic!("numeric fixture must decode as Number")
        };
        assert_eq!(number.to_bits(), expected_bits, "input {input}");

        let encoded = to_canonical_string(&document).unwrap();
        assert!(
            encoded.contains(&format!("\"value\": {canonical}\n")),
            "input {input} encoded unexpectedly:\n{encoded}"
        );
        assert_eq!(
            to_canonical_string(&from_str(&encoded).unwrap()).unwrap(),
            encoded
        );
    }

    for input in ["1e400", "-1e400"] {
        let error = from_str(&v2_number_source(input, "Document")).unwrap_err();
        assert!(
            matches!(error, FormatError::InvalidRepresentation { .. }),
            "{input}: {error:?}"
        );
    }
}

#[test]
fn v2_resource_limits_admit_the_exact_boundary_and_reject_one_byte_more() {
    let admitted_number = format!("0.{}", "0".repeat(V2_MAX_NUMBER_TOKEN_BYTES - 2));
    assert_eq!(admitted_number.len(), V2_MAX_NUMBER_TOKEN_BYTES);
    assert!(from_str(&v2_number_source(&admitted_number, "Document")).is_ok());

    let blocked_number = format!("{admitted_number}0");
    let error = from_str(&v2_number_source(&blocked_number, "Document")).unwrap_err();
    assert!(matches!(
        error,
        FormatError::ResourceLimit {
            resource: "number token",
            limit: V2_MAX_NUMBER_TOKEN_BYTES,
            actual,
        } if actual == V2_MAX_NUMBER_TOKEN_BYTES + 1
    ));

    let base = v2_number_source("0", "Document");
    let exact_input = format!("{base}{}", " ".repeat(V2_MAX_INPUT_BYTES - base.len()));
    assert_eq!(exact_input.len(), V2_MAX_INPUT_BYTES);
    assert!(from_str(&exact_input).is_ok());

    let oversized_input = format!("{exact_input} ");
    let error = from_str(&oversized_input).unwrap_err();
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
fn v2_preserves_unicode_scalar_sequences_without_normalization() {
    let decomposed = "Cafe\u{301}";
    let document = from_str(&v2_number_source("1", decomposed)).unwrap();
    let encoded = to_canonical_string(&document).unwrap();

    assert_eq!(document.title, decomposed);
    assert!(encoded.contains(decomposed));
    assert!(!encoded.contains("Café"));
    assert_eq!(from_str(&encoded).unwrap().title, decomposed);
}

#[test]
fn every_legacy_identity_mapping_class_rejects_corrupt_or_unresolvable_input() {
    let invalid_before_mapping = [
        (
            "document",
            r#"{"format_version":1,"id":"","title":"Document","schemas":{},"entities":{}}"#,
        ),
        (
            "schema",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"map":{"id":"nested","fields":{}}},"entities":{}}"#,
        ),
        (
            "field",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"e":{"id":"e","schema":"s","fields":{"missing":{"kind":"number","value":1}}}}}"#,
        ),
        (
            "entity",
            r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{}}},"entities":{"map":{"id":"nested","schema":"s","fields":{}}}}"#,
        ),
    ];
    for (mapping, source) in invalid_before_mapping {
        let error = from_str(source).unwrap_err();
        assert!(
            matches!(error, FormatError::InvalidRepresentation { .. }),
            "{mapping}: {error:?}"
        );
    }

    let unresolved_relationships = [
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"f":{"field_type":{"type":"reference","schema":"s"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"f":{"kind":"reference","value":"missing"}}}}}"#,
        r#"{"format_version":1,"id":"doc","title":"Document","schemas":{"s":{"id":"s","fields":{"f":{"field_type":{"type":"number"},"required":true}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"f":{"kind":"formula","value":{"op":"reference","args":{"entity":"e","field":"missing"}}}}}}}"#,
    ];
    for source in unresolved_relationships {
        assert!(matches!(
            from_str(source).unwrap_err(),
            FormatError::MigrationFailed { .. }
        ));
    }
}
