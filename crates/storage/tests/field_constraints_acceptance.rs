use std::collections::BTreeMap;
use tachiko_semantic_core::{
    Document, Entity, FieldConstraint, FieldDefinition, FieldType, Number, Schema, Value,
};

fn range(min: f64, max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: Number::new(min).unwrap(),
        max: Number::new(max).unwrap(),
    }
}

fn document(field_type: FieldType, constraint: FieldConstraint, value: Option<Value>) -> Document {
    let mut document = Document::empty("doc", "Constraints acceptance");
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields: BTreeMap::from([(
                "field".into(),
                FieldDefinition {
                    id: "field".into(),
                    key: "value".into(),
                    field_type,
                    required: false,
                    constraint,
                },
            )]),
        },
    );
    if let Some(value) = value {
        document.entities.insert(
            "entity".into(),
            Entity {
                id: "entity".into(),
                key: "item".into(),
                schema: "schema".into(),
                fields: BTreeMap::from([("field".into(), value)]),
            },
        );
    }
    document
}

use tachiko_storage::{
    decode_roproj_v1, decode_roproj_v2, encode_portable_package_v1, encode_roproj_v1,
    encode_roproj_v2, from_str, to_canonical_string,
};

#[test]
fn every_frozen_writer_refuses_a_valid_non_none_facet_before_output() {
    let candidates = [
        document(
            FieldType::Number,
            range(0.0, 10.0),
            Some(Value::Number(Number::new(5.0).unwrap())),
        ),
        document(
            FieldType::Text,
            FieldConstraint::TextLiteralSet {
                values: vec![String::new()],
            },
            None,
        ),
    ];
    for candidate in candidates {
        assert!(tachiko_semantic_core::validate_document(&candidate).is_empty());
        assert!(to_canonical_string(&candidate).is_err());
        assert!(encode_roproj_v1(&candidate).is_err());
        assert!(encode_roproj_v2(&candidate).is_err());
        // package/v1 can only be reached through a admitted frozen v1 tree.
        assert!(
            encode_roproj_v1(&candidate)
                .and_then(|tree| encode_portable_package_v1(&tree))
                .is_err()
        );
    }
}

#[test]
fn none_roundtrips_through_frozen_version_owned_codecs_without_byte_drift() {
    let candidate = document(
        FieldType::Number,
        FieldConstraint::None,
        Some(Value::Number(Number::new(5.0).unwrap())),
    );
    let direct = to_canonical_string(&candidate).unwrap();
    assert!(!direct.contains("constraint"));
    let decoded = from_str(&direct).unwrap();
    assert_eq!(decoded, candidate);
    assert_eq!(to_canonical_string(&decoded).unwrap(), direct);
    let v1 = encode_roproj_v1(&candidate).unwrap();
    let decoded = decode_roproj_v1(&v1).unwrap();
    assert_eq!(decoded, candidate);
    assert_eq!(encode_roproj_v1(&decoded).unwrap(), v1);
    let v2 = encode_roproj_v2(&candidate).unwrap();
    let decoded = decode_roproj_v2(&v2).unwrap();
    assert_eq!(decoded, candidate);
    assert_eq!(encode_roproj_v2(&decoded).unwrap(), v2);
}
