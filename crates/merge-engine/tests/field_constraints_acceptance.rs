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

use tachiko_merge_engine::{MergeOutcome, merge};

#[test]
fn v1_merge_refuses_constraints_in_every_input_before_noop_or_structural_merge() {
    let empty = Document::empty("doc", "Constraints acceptance");
    let constrained = document(FieldType::Number, range(0.0, 10.0), None);
    let none = document(FieldType::Number, FieldConstraint::None, None);
    for (base, ours, theirs) in [
        (&constrained, &constrained, &constrained),
        (&empty, &constrained, &empty),
        (&empty, &empty, &constrained),
        (&constrained, &empty, &empty),
        (&none, &constrained, &none),
        (&none, &none, &constrained),
    ] {
        assert!(matches!(
            merge(base, ours, theirs),
            MergeOutcome::UnsupportedFieldConstraintChange
        ));
    }
    assert!(matches!(
        merge(&none, &none, &none),
        MergeOutcome::Merged(_)
    ));
}
