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

use tachiko_diff_engine::{DiffError, diff};

#[test]
fn v1_diff_refuses_equal_added_removed_and_changed_constraint_bearing_inputs() {
    let empty = Document::empty("doc", "Constraints acceptance");
    let constrained = document(FieldType::Number, range(0.0, 10.0), None);
    let other = document(FieldType::Number, range(0.0, 20.0), None);
    let none = document(FieldType::Number, FieldConstraint::None, None);
    for (before, after) in [
        (&constrained, &constrained),
        (&empty, &constrained),
        (&constrained, &empty),
        (&none, &constrained),
        (&constrained, &none),
        (&constrained, &other),
    ] {
        assert!(matches!(
            diff(before, after),
            Err(DiffError::UnsupportedFieldConstraintChange)
        ));
    }
    assert!(diff(&none, &none).is_ok());
}
