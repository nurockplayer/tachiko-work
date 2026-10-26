//! `[.field]` survives the live storage formats and is refused by frozen ones.
use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldId, FieldRef, FieldType,
    Number, Schema, Value,
};
use tachiko_storage::{
    decode_roproj_v3, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3, from_str,
    to_canonical_string,
};

fn field(name: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(name),
        key: name.into(),
        display_name: None,
        field_type,
        required: true,
        constraint: FieldConstraint::None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn document() -> Document {
    let mut document = Document::empty("monthly-plan", "Monthly plan");
    document.schemas.insert(
        "plan".into(),
        Schema {
            id: "plan".into(),
            key: "plan".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("item".into(), field("item", FieldType::Text)),
                ("expense".into(), field("expense", FieldType::Number)),
                ("reserve".into(), field("reserve", FieldType::Number)),
            ]),
        },
    );
    document.schemas.insert(
        "settings".into(),
        Schema {
            id: "settings".into(),
            key: "settings".into(),
            display_name: None,
            fields: BTreeMap::from([("rate".into(), field("rate", FieldType::Number))]),
        },
    );
    document.entities.insert(
        "rent".into(),
        Entity {
            id: "rent".into(),
            key: "rent".into(),
            schema: "plan".into(),
            fields: BTreeMap::from([
                ("item".into(), Value::Text("Rent".to_owned())),
                ("expense".into(), number(80_000.0)),
                (
                    "reserve".into(),
                    Value::Formula(Expression::Multiply {
                        left: Box::new(Expression::SameRowReference(FieldId::from("expense"))),
                        right: Box::new(Expression::Reference(FieldRef::new("global", "rate"))),
                    }),
                ),
            ]),
        },
    );
    document.entities.insert(
        "global".into(),
        Entity {
            id: "global".into(),
            key: "global".into(),
            schema: "settings".into(),
            fields: BTreeMap::from([("rate".into(), number(0.1))]),
        },
    );
    document
}

#[test]
fn direct_ro_v2_round_trips_the_same_row_reference() {
    let document = document();
    let encoded = to_canonical_string(&document).unwrap();
    assert!(encoded.contains("\"same_row_reference\""), "{encoded}");
    assert!(encoded.contains("\"expense\""), "{encoded}");
    assert_eq!(from_str(&encoded).unwrap(), document);
}

#[test]
fn roproj_v3_round_trips_the_same_row_reference() {
    let document = document();
    let tree = encode_roproj_v3(&document).unwrap();
    assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
}

#[test]
fn frozen_formats_refuse_to_silently_drop_the_entity() {
    let document = document();
    for error in [
        encode_roproj_v1(&document).unwrap_err(),
        encode_roproj_v2(&document).unwrap_err(),
    ] {
        let message = error.to_string();
        assert!(
            message.contains("same-row references are not representable"),
            "{message}"
        );
        assert!(
            message.contains("use .roproj/v3 or direct-ro/v2"),
            "{message}"
        );
    }
}
