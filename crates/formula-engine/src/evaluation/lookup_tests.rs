use std::collections::BTreeMap;

use tachiko_semantic_core::{
    ColumnRef, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType,
    Number, Schema, Value,
};

use crate::{CalculationOutcome, calculate_complete};

use super::LOOKUP_INDEX_BUILDS;

fn field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format: None,
    }
}

#[test]
fn a_thousand_lookup_formulas_share_one_index_each_pass() {
    let mut doc = Document::empty("indexed-lookup", "Indexed lookup");
    for (id, fields) in [("table", vec!["key", "value"]), ("summary", vec!["result"])] {
        doc.schemas.insert(
            id.into(),
            Schema {
                id: id.into(),
                key: id.into(),
                display_name: None,
                field_order: None,
                fields: fields
                    .into_iter()
                    .map(|id| (id.into(), field(id)))
                    .collect(),
            },
        );
    }
    for index in 0..1_000 {
        let number = Number::new(f64::from(index)).unwrap();
        let row = format!("row-{index:04}");
        doc.entities.insert(
            row.clone().into(),
            Entity {
                id: row.clone().into(),
                key: row.into(),
                schema: "table".into(),
                fields: BTreeMap::from([
                    ("key".into(), Value::Number(number)),
                    ("value".into(), Value::Number(number)),
                ]),
            },
        );
        let owner = format!("lookup-{index:04}");
        doc.entities.insert(
            owner.clone().into(),
            Entity {
                id: owner.clone().into(),
                key: owner.into(),
                schema: "summary".into(),
                fields: BTreeMap::from([(
                    "result".into(),
                    Value::Formula(Expression::Lookup {
                        key: Box::new(Expression::Number(number)),
                        key_column: ColumnRef::new("table", "key"),
                        result_column: ColumnRef::new("table", "value"),
                        if_missing: None,
                    }),
                )]),
            },
        );
    }
    LOOKUP_INDEX_BUILDS.with(|builds| builds.set(0));
    for _ in 0..2 {
        let CalculationOutcome::Complete(calculation) = calculate_complete(&doc) else {
            panic!("all keys match")
        };
        for index in 0..1_000 {
            assert_eq!(
                calculation.values()[&FieldRef::new(format!("lookup-{index:04}"), "result")],
                Number::new(f64::from(index)).unwrap()
            );
        }
        assert_eq!(LOOKUP_INDEX_BUILDS.with(|builds| builds.replace(0)), 1);
    }
}
