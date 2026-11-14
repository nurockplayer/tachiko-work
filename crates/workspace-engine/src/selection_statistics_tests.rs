//! Selection statistics read stored values and retained calculation outcomes.
use std::collections::BTreeMap;

use super::{ResidentSelectionStatistics, ResidentWorkspaceSession};
use crate::{
    Date, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType,
    Number, NumberFormat, NumberFormatKind, Schema, Value,
};

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn definition(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format: None,
    }
}

fn document(cells: Vec<(&str, FieldType, Option<Value>)>) -> Document {
    let mut document = Document::empty("selection", "Selection");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "items".into(),
            display_name: None,
            field_order: None,
            fields: cells
                .iter()
                .map(|(id, kind, _)| ((*id).into(), definition(id, kind.clone())))
                .collect(),
        },
    );
    document.entities.insert(
        "e".into(),
        Entity {
            id: "e".into(),
            key: "row".into(),
            schema: "s".into(),
            fields: cells
                .into_iter()
                .filter_map(|(id, _, value)| value.map(|value| (id.into(), value)))
                .collect(),
        },
    );
    document
}

fn query(document: Document, fields: &[&str]) -> ResidentSelectionStatistics {
    let session = ResidentWorkspaceSession::new("selection".into(), document);
    let targets = fields
        .iter()
        .map(|field| FieldRef::new("e", *field))
        .collect::<Vec<_>>();
    let before = session.runtime_measurements();
    let result = session.query_selection_statistics(&targets).unwrap();
    assert_eq!(result.revision(), session.revision());
    assert_eq!(session.runtime_measurements(), before);
    result.value().clone()
}

fn aggregates(stats: &ResidentSelectionStatistics) -> [Option<f64>; 4] {
    [stats.sum, stats.average, stats.min, stats.max]
}

#[test]
fn selection_statistics_cell_classes_never_coerce() {
    let cases = [
        (FieldType::Text, None, 0, 0),
        (FieldType::Text, Some(Value::Text(String::new())), 0, 0),
        (FieldType::Text, Some(Value::Text("123".into())), 1, 0),
        (FieldType::Text, Some(Value::Text("hello".into())), 1, 0),
        (FieldType::Boolean, Some(Value::Boolean(false)), 1, 0),
        (FieldType::Boolean, Some(Value::Boolean(true)), 1, 0),
        (
            FieldType::Date,
            Some(Value::Date(Date::parse("2026-11-14").unwrap())),
            1,
            0,
        ),
        (
            FieldType::Reference { schema: "s".into() },
            Some(Value::Reference("e".into())),
            1,
            0,
        ),
        (FieldType::Number, Some(number(3.0)), 1, 1),
        (
            FieldType::Number,
            Some(Value::Formula(Expression::Number(
                Number::new(3.0).unwrap(),
            ))),
            1,
            1,
        ),
    ];
    for (kind, value, count, numerical_count) in cases {
        let stats = query(document(vec![("f", kind, value)]), &["f"]);
        assert_eq!(stats.count, count);
        assert_eq!(stats.numerical_count, numerical_count);
        assert_eq!(stats.error_count, 0);
        let expected = if numerical_count == 0 {
            [Some(0.0), None, None, None]
        } else {
            [Some(3.0); 4]
        };
        assert_eq!(aggregates(&stats), expected);
    }
}

#[test]
fn selection_statistics_empty_and_nonnumeric_selections() {
    let doc = document(vec![(
        "f",
        FieldType::Text,
        Some(Value::Text("123".into())),
    )]);
    for fields in [vec![], vec!["f"]] {
        let stats = query(doc.clone(), &fields);
        assert_eq!(stats.count, fields.len());
        assert_eq!(stats.numerical_count, 0);
        assert_eq!(stats.error_count, 0);
        assert_eq!(aggregates(&stats), [Some(0.0), None, None, None]);
    }
}

#[test]
fn selection_statistics_failure_and_unavailable_null_all_aggregates() {
    let doc = document(vec![
        (
            "bad",
            FieldType::Number,
            Some(Value::Formula(Expression::Divide {
                left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
            })),
        ),
        (
            "unavailable",
            FieldType::Number,
            Some(Value::Formula(Expression::Number(
                Number::new(9.0).unwrap(),
            ))),
        ),
        ("stored", FieldType::Number, Some(number(4.0))),
    ]);
    for error in ["bad", "unavailable"] {
        let stats = query(doc.clone(), &[error, "stored"]);
        assert_eq!(stats.count, 2);
        assert_eq!(stats.numerical_count, 1);
        assert_eq!(stats.error_count, 1);
        assert_eq!(aggregates(&stats), [None; 4]);
    }
    let stats = query(doc, &["bad", "unavailable", "stored"]);
    assert_eq!(stats.error_count, 2);
    assert_eq!(aggregates(&stats), [None; 4]);
}

#[test]
fn selection_statistics_negative_zero_and_overflow_safe_average() {
    let cases = [
        (vec![-6.0, 0.0, 3.0], Some(-3.0), -1.0, -6.0, 3.0),
        (vec![0.0, 0.0], Some(0.0), 0.0, 0.0, 0.0),
        (vec![1e308, 1e308], None, 1e308, 1e308, 1e308),
        (vec![-1e308, -1e308], None, -1e308, -1e308, -1e308),
        (vec![-1e308, 1e308], Some(0.0), 0.0, -1e308, 1e308),
        (vec![1e308, 1e308, -1e308, -1e308], None, 0.0, -1e308, 1e308),
    ];
    for (values, sum, average, min, max) in cases {
        let names = (0..values.len())
            .map(|index| format!("f{index}"))
            .collect::<Vec<_>>();
        let cells = names
            .iter()
            .zip(&values)
            .map(|(id, value)| (id.as_str(), FieldType::Number, Some(number(*value))))
            .collect();
        let fields = names.iter().map(String::as_str).collect::<Vec<_>>();
        let stats = query(document(cells), &fields);
        assert_eq!(stats.numerical_count, values.len());
        assert_eq!(
            aggregates(&stats),
            [sum, Some(average), Some(min), Some(max)]
        );
    }
}

#[test]
fn selection_statistics_exact_sum_retains_cancellation_remainder() {
    let doc = document(vec![
        ("a", FieldType::Number, Some(number(1e16))),
        ("b", FieldType::Number, Some(number(1.0))),
        ("c", FieldType::Number, Some(number(-1e16))),
    ]);
    let stats = query(doc, &["a", "b", "c"]);
    assert_eq!(stats.sum.map(f64::to_bits), Some(1.0_f64.to_bits()));
    assert_eq!(
        stats.average.map(f64::to_bits),
        Some((1.0_f64 / 3.0).to_bits())
    );
}

#[test]
fn selection_statistics_sum_leaving_f64_is_null_like_formula_sum() {
    // The shared exact reducer is not reordered here: a sum that leaves f64
    // on the way is null, as SUM is, while the average stays finite.
    let doc = document(vec![
        ("a", FieldType::Number, Some(number(1e308))),
        ("b", FieldType::Number, Some(number(1e308))),
        ("c", FieldType::Number, Some(number(-1e308))),
    ]);
    let stats = query(doc, &["a", "b", "c"]);
    assert_eq!(stats.sum, None);
    assert_eq!(
        stats.average.map(f64::to_bits),
        Some((1e308_f64 / 3.0).to_bits())
    );
}

#[test]
fn selection_statistics_mixed_collections_raw_formats_and_canonical_order() {
    let mut doc = document(vec![
        ("a", FieldType::Number, Some(number(1e16))),
        ("b", FieldType::Number, Some(number(-1e16))),
        ("c", FieldType::Number, Some(number(0.25))),
        ("text", FieldType::Text, Some(Value::Text("123".into()))),
    ]);
    let schema = doc.schemas.get_mut("s").unwrap();
    for (id, kind) in [
        ("a", NumberFormatKind::Currency { code: "JPY".into() }),
        ("c", NumberFormatKind::Percent),
    ] {
        schema.fields.get_mut(id).unwrap().number_format = Some(NumberFormat {
            decimals: 0,
            kind,
            thousands_separator: false,
        });
    }
    let mut other = doc.schemas["s"].clone();
    other.id = "other".into();
    other.key = "other".into();
    other.fields = BTreeMap::from([("d".into(), definition("d", FieldType::Number))]);
    doc.schemas.insert(other.id.clone(), other);
    doc.entities.insert(
        "z".into(),
        Entity {
            id: "z".into(),
            key: "other".into(),
            schema: "other".into(),
            fields: BTreeMap::from([("d".into(), number(1.0))]),
        },
    );
    let session = ResidentWorkspaceSession::new("selection".into(), doc);
    let mut targets = vec![
        FieldRef::new("e", "c"),
        FieldRef::new("e", "a"),
        FieldRef::new("z", "d"),
        FieldRef::new("e", "b"),
        FieldRef::new("e", "text"),
    ];
    let forward = session.query_selection_statistics(&targets).unwrap();
    targets.reverse();
    let reverse = session.query_selection_statistics(&targets).unwrap();
    assert_eq!(forward, reverse);
    assert_eq!(
        aggregates(forward.value()).map(|value| value.map(f64::to_bits)),
        aggregates(reverse.value()).map(|value| value.map(f64::to_bits)),
    );
    let stats = forward.value();
    assert_eq!(stats.count, 5);
    assert_eq!(stats.numerical_count, 4);
    assert_eq!(stats.sum.map(f64::to_bits), Some(1.25_f64.to_bits()));
    assert_eq!(stats.average.map(f64::to_bits), Some(0.3125_f64.to_bits()));
}
