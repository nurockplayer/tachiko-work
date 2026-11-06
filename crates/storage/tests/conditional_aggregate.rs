//! Conditional aggregates survive `.roproj/v3`; frozen and direct formats
//! refuse them instead of dropping them.
use std::collections::BTreeMap;

use tachiko_semantic_core::{
    ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate, Document, Entity,
    Expression, FieldConstraint, FieldDefinition, FieldId, FieldType, Number, Schema, Value,
};
use tachiko_storage::{
    CanonicalRoProjectV3, decode_roproj_v3, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3,
    to_canonical_string,
};

fn field(name: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(name),
        key: name.into(),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn column(field: &str) -> ColumnRef {
    ColumnRef::new("plan", field)
}

fn conditional(aggregate: ConditionalAggregate, criteria: Vec<ColumnCriterion>) -> Expression {
    Expression::ColumnConditional {
        aggregate: Box::new(aggregate),
        criteria,
    }
}

fn document(expression: Expression) -> Document {
    let mut document = Document::empty("conditional-aggregates", "Conditional aggregates");
    document.schemas.insert(
        "plan".into(),
        Schema {
            field_order: None,
            id: "plan".into(),
            key: "plan".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("category".into(), field("category", FieldType::Text)),
                ("reserve".into(), field("reserve", FieldType::Number)),
                ("active".into(), field("active", FieldType::Boolean)),
                ("quota".into(), field("quota", FieldType::Number)),
            ]),
        },
    );
    document.schemas.insert(
        "summary".into(),
        Schema {
            field_order: None,
            id: "summary".into(),
            key: "summary".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("result".into(), field("result", FieldType::Number)),
                ("criterion".into(), field("criterion", FieldType::Number)),
                (
                    "criterion_text".into(),
                    field("criterion_text", FieldType::Text),
                ),
            ]),
        },
    );
    document.entities.insert(
        "rent".into(),
        Entity {
            id: "rent".into(),
            key: "rent".into(),
            schema: "plan".into(),
            fields: BTreeMap::from([
                ("category".into(), Value::Text("housing".to_owned())),
                ("reserve".into(), number(12_000.0)),
                ("active".into(), Value::Boolean(true)),
                ("quota".into(), number(2.0)),
            ]),
        },
    );
    document.entities.insert(
        "summary".into(),
        Entity {
            id: "summary".into(),
            key: "summary".into(),
            schema: "summary".into(),
            fields: BTreeMap::from([
                ("result".into(), Value::Formula(expression)),
                ("criterion".into(), number(2.0)),
                ("criterion_text".into(), Value::Text("housing".to_owned())),
            ]),
        },
    );
    document
}

fn reference_criterion() -> ColumnCriterion {
    ColumnCriterion {
        column: column("category"),
        operator: ComparisonOperator::Equal,
        value: Some(Box::new(Expression::Reference(
            tachiko_semantic_core::FieldRef::new("summary", "criterion_text"),
        ))),
    }
}

#[test]
fn roproj_v3_round_trips_every_aggregate_and_criterion_shape() {
    let same_row = ColumnCriterion {
        column: column("category"),
        operator: ComparisonOperator::Equal,
        value: Some(Box::new(Expression::SameRowReference(FieldId::from(
            "criterion_text",
        )))),
    };
    let cases = [
        (
            "sum with a cell criterion",
            conditional(
                ConditionalAggregate::Sum(column("reserve")),
                vec![reference_criterion()],
            ),
        ),
        (
            "average with a relational number",
            conditional(
                ConditionalAggregate::Average(column("reserve")),
                vec![ColumnCriterion {
                    column: column("quota"),
                    operator: ComparisonOperator::Greater,
                    value: Some(Box::new(Expression::Number(Number::new(1.0).unwrap()))),
                }],
            ),
        ),
        (
            "count with a blank test",
            conditional(
                ConditionalAggregate::Count,
                vec![ColumnCriterion {
                    column: column("category"),
                    operator: ComparisonOperator::NotEqual,
                    value: None,
                }],
            ),
        ),
        (
            "count with a same-row criterion",
            conditional(ConditionalAggregate::Count, vec![same_row]),
        ),
    ];
    for (name, expression) in cases {
        let document = document(expression);
        let tree = encode_roproj_v3(&document).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(decode_roproj_v3(&tree).unwrap(), document, "{name}");
        let encoded = tree
            .files()
            .iter()
            .filter_map(|file| std::str::from_utf8(file.bytes()).ok())
            .collect::<String>();
        assert!(encoded.contains("column_conditional"), "{name}");
        assert!(encoded.contains("\"operator\""), "{name}");
        assert!(!encoded.contains("\"text\":"), "{name}: {encoded}");
    }
}

#[test]
fn frozen_and_direct_formats_refuse_conditional_aggregates() {
    let document = document(conditional(
        ConditionalAggregate::Count,
        vec![reference_criterion()],
    ));
    for error in [
        encode_roproj_v1(&document).unwrap_err(),
        encode_roproj_v2(&document).unwrap_err(),
    ] {
        let message = error.to_string();
        assert!(
            message.contains("conditional aggregates are not representable"),
            "{message}"
        );
    }
    let message = to_canonical_string(&document).unwrap_err().to_string();
    assert!(
        message.contains("conditional aggregates are not representable in direct .ro/v2"),
        "{message}"
    );
}

fn replace_in_tree(tree: &CanonicalRoProjectV3, old: &str, new: &str) -> Vec<(String, Vec<u8>)> {
    let mut files = tree
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    let (_, bytes) = files
        .iter_mut()
        .find(|(_, bytes)| std::str::from_utf8(bytes).is_ok_and(|source| source.contains(old)))
        .unwrap_or_else(|| panic!("missing mutation anchor: {old}"));
    let source = String::from_utf8(bytes.clone()).unwrap();
    assert!(source.contains(old), "missing mutation anchor: {old}");
    *bytes = source.replacen(old, new, 1).into_bytes();
    files
}

#[test]
fn roproj_v3_decoding_refuses_impossible_criteria() {
    // A blank test with a relational operator.
    let blank = document(conditional(
        ConditionalAggregate::Count,
        vec![ColumnCriterion {
            column: column("category"),
            operator: ComparisonOperator::Equal,
            value: None,
        }],
    ));
    let tree = encode_roproj_v3(&blank).unwrap();
    let candidate = replace_in_tree(&tree, "\"operator\":\"=\"", "\"operator\":\">\"");
    let message = CanonicalRoProjectV3::try_from_files(candidate)
        .unwrap_err()
        .to_string();
    assert!(message.contains("blank criterion"), "{message}");

    // A relational operator with something other than a number literal.
    let number = document(conditional(
        ConditionalAggregate::Count,
        vec![ColumnCriterion {
            column: column("quota"),
            operator: ComparisonOperator::Greater,
            value: Some(Box::new(Expression::Number(Number::new(1.0).unwrap()))),
        }],
    ));
    let tree = encode_roproj_v3(&number).unwrap();
    let candidate = replace_in_tree(
        &tree,
        "{\"op\":\"number\",\"args\":1}",
        "{\"op\":\"reference\",\"args\":{\"entity\":\"summary\",\"field\":\"criterion\"}}",
    );
    let message = CanonicalRoProjectV3::try_from_files(candidate)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("relational criterion requires a number literal"),
        "{message}"
    );
}
