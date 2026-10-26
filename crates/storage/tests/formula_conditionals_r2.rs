use std::collections::BTreeMap;

use tachiko_semantic_core::{
    ComparisonOperator, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey,
    Value,
};
use tachiko_storage::{
    decode_roproj_v3, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3, from_str,
    to_canonical_string,
};

fn conditional_expression() -> Expression {
    Expression::If {
        condition: Box::new(Expression::And(vec![
            Expression::Compare {
                operator: ComparisonOperator::GreaterOrEqual,
                left: Box::new(Expression::Number(Number::new(2.0).unwrap())),
                right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            },
            Expression::Or(vec![
                Expression::Compare {
                    operator: ComparisonOperator::Equal,
                    left: Box::new(Expression::Number(Number::new(3.0).unwrap())),
                    right: Box::new(Expression::Number(Number::new(3.0).unwrap())),
                },
                Expression::Not(Box::new(Expression::IsBlank(Box::new(
                    Expression::Reference(FieldRef::new("row", "rate")),
                )))),
            ]),
        ])),
        then_value: Box::new(Expression::Round {
            value: Box::new(Expression::SameRowReference(FieldId::from("expense"))),
            digits: 2,
        }),
        else_value: Box::new(Expression::Round {
            value: Box::new(Expression::Number(Number::new(-0.5).unwrap())),
            digits: 0,
        }),
    }
}

fn document_with_conditional() -> Document {
    let mut document = Document::empty("monthly-plan", "Monthly plan");
    let fields = BTreeMap::from([
        (
            FieldId::from("expense"),
            FieldDefinition {
                id: FieldId::from("expense"),
                key: FieldKey::from("expense"),
                display_name: None,
                field_type: FieldType::Number,
                required: true,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        ),
        (
            FieldId::from("reserve"),
            FieldDefinition {
                id: FieldId::from("reserve"),
                key: FieldKey::from("reserve"),
                display_name: None,
                field_type: FieldType::Number,
                required: true,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        ),
        (
            FieldId::from("rate"),
            FieldDefinition {
                id: FieldId::from("rate"),
                key: FieldKey::from("rate"),
                display_name: None,
                field_type: FieldType::Number,
                required: true,
                constraint: FieldConstraint::None,
                number_format: None,
            },
        ),
    ]);
    document.schemas.insert(
        SchemaId::from("sheet"),
        Schema {
            id: SchemaId::from("sheet"),
            key: SchemaKey::from("sheet"),
            display_name: None,
            fields: fields.clone(),
        },
    );
    document.entities.insert(
        EntityId::from("row"),
        Entity {
            id: EntityId::from("row"),
            key: EntityKey::from("row"),
            schema: SchemaId::from("sheet"),
            fields: BTreeMap::from([
                (
                    FieldId::from("expense"),
                    Value::Number(Number::new(25.555).unwrap()),
                ),
                (
                    FieldId::from("reserve"),
                    Value::Formula(conditional_expression()),
                ),
                (
                    FieldId::from("rate"),
                    Value::Number(Number::new(0.1).unwrap()),
                ),
            ]),
        },
    );
    document
}

#[test]
fn direct_ro_v2_and_roproj_v3_round_trip_conditional_formula_nodes() {
    let document = document_with_conditional();

    let direct = to_canonical_string(&document).unwrap();
    assert!(direct.contains("\"compare\""), "{direct}");
    assert!(direct.contains("\"round\""), "{direct}");
    assert_eq!(from_str(&direct).unwrap(), document);

    let project = encode_roproj_v3(&document).unwrap();
    assert_eq!(decode_roproj_v3(&project).unwrap(), document);
}

#[test]
fn frozen_roproj_versions_refuse_conditional_formula_nodes() {
    let document = document_with_conditional();

    for error in [
        encode_roproj_v1(&document).unwrap_err(),
        encode_roproj_v2(&document).unwrap_err(),
    ] {
        let message = error.to_string();
        assert!(
            message.contains("conditional and rounding formula expressions"),
            "{message}"
        );
        assert!(
            message.contains("not representable in frozen .roproj/v"),
            "{message}"
        );
    }
}

#[test]
fn legacy_direct_v1_refuses_new_expression_operations() {
    let source = r#"{"format_version":1,"id":"doc","title":"Doc","schemas":{"s":{"id":"s","fields":{"f":{"field_type":{"type":"number"},"required":false}}}},"entities":{"e":{"id":"e","schema":"s","fields":{"f":{"kind":"formula","value":{"op":"round","args":{"value":{"op":"number","args":1},"digits":0}}}}}}}"#;
    let error = from_str(source).unwrap_err().to_string();
    assert!(
        error.contains("unknown v1 expression op 'round'"),
        "{error}"
    );
}
