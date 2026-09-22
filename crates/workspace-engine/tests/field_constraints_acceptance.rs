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

use tachiko_semantic_core::{Expression, FieldRef};
use tachiko_workspace_engine::{diagnostic_codes, validation_report};

fn formula_document(result: f64) -> Document {
    document(
        FieldType::Number,
        range(0.0, 10.0),
        Some(Value::Formula(Expression::Number(
            Number::new(result).unwrap(),
        ))),
    )
}

#[test]
fn complete_calculation_enforces_formula_range_at_inclusive_bounds() {
    for (value, valid) in [(0.0, true), (10.0, true), (-1.0, false), (11.0, false)] {
        let candidate = formula_document(value);
        let report = validation_report(&candidate);
        assert_eq!(
            report.is_valid(),
            valid,
            "{:?}",
            report.stable_observations()
        );
        if !valid {
            assert!(report.diagnostics().iter().any(|d| d.code.as_str()
                == "core.field_value_constraint_mismatch"
                && d.path == "entities.entity.fields.field"));
        }
    }
}

#[test]
fn any_failed_formula_suppresses_all_dependent_formula_range_checks() {
    let mut candidate = formula_document(20.0);
    candidate.schemas.get_mut("schema").unwrap().fields.insert(
        "broken".into(),
        FieldDefinition {
            id: "broken".into(),
            key: "broken".into(),
            field_type: FieldType::Number,
            required: true,
            constraint: FieldConstraint::None,
        },
    );
    candidate.entities.get_mut("entity").unwrap().fields.insert(
        "broken".into(),
        Value::Formula(Expression::Reference(FieldRef::new("entity", "broken"))),
    );
    let report = validation_report(&candidate);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|d| d.code == diagnostic_codes::FORMULA_CYCLE)
    );
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "core.field_value_constraint_mismatch")
    );
    // Direct stored values still receive Stage2 validation despite an unrelated cycle.
    candidate
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("field".into(), Value::Number(Number::new(20.0).unwrap()));
    let report = validation_report(&candidate);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|d| d.code.as_str() == "core.field_value_constraint_mismatch")
    );
}
