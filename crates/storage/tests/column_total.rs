//! `[schema.field:total]` survives the live storage formats and is refused by
//! frozen ones.
use std::collections::BTreeMap;

use tachiko_semantic_core::{
    ColumnRef, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldId, FieldType,
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

fn total_expression() -> Expression {
    Expression::ColumnTotal(ColumnRef::new("plan", "reserve"))
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
                ("reserve".into(), field("reserve", FieldType::Number)),
            ]),
        },
    );
    document.schemas.insert(
        "summary".into(),
        Schema {
            id: "summary".into(),
            key: "summary".into(),
            display_name: None,
            fields: BTreeMap::from([("total".into(), field("total", FieldType::Number))]),
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
                ("reserve".into(), number(12_000.0)),
            ]),
        },
    );
    document.entities.insert(
        "summary".into(),
        Entity {
            id: "summary".into(),
            key: "summary".into(),
            schema: "summary".into(),
            fields: BTreeMap::from([("total".into(), Value::Formula(total_expression()))]),
        },
    );
    document
}

#[test]
fn direct_ro_v2_round_trips_the_column_total() {
    let document = document();
    let encoded = to_canonical_string(&document).unwrap();
    assert!(encoded.contains("\"column_total\""), "{encoded}");
    assert!(encoded.contains("\"plan\""), "{encoded}");
    assert_eq!(from_str(&encoded).unwrap(), document);
}

#[test]
fn roproj_v3_round_trips_the_column_total() {
    let document = document();
    let tree = encode_roproj_v3(&document).unwrap();
    assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
}

#[test]
fn frozen_formats_refuse_to_silently_drop_the_column() {
    let document = document();
    for error in [
        encode_roproj_v1(&document).unwrap_err(),
        encode_roproj_v2(&document).unwrap_err(),
    ] {
        let message = error.to_string();
        assert!(
            message.contains("column totals are not representable"),
            "{message}"
        );
        assert!(
            message.contains("use .roproj/v3 or direct-ro/v2"),
            "{message}"
        );
    }
}
