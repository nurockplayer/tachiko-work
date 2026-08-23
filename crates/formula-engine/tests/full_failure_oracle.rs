use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationOutcome, FormulaFailure, calculate_full};
use tachiko_semantic_core::{
    Document, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn literal(value: f64) -> Expression {
    Expression::Number(number(value))
}

fn reference(field: &str) -> Expression {
    Expression::Reference(FieldRef::new("entity", field))
}

fn document() -> Document {
    Document {
        id: "document".into(),
        title: "Formula oracle".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema"),
            Schema {
                id: "schema".into(),
                key: SchemaKey::from("schema"),
                fields: BTreeMap::new(),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity"),
            Entity {
                id: "entity".into(),
                key: "entity".into(),
                schema: "schema".into(),
                fields: BTreeMap::new(),
            },
        )]),
    }
}

fn insert(document: &mut Document, field: &str, field_type: FieldType, value: Value) {
    document.schemas.get_mut("schema").unwrap().fields.insert(
        FieldId::from(field),
        FieldDefinition {
            id: FieldId::from(field),
            key: FieldKey::from(field),
            field_type,
            required: true,
        },
    );
    document
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert(FieldId::from(field), value);
}

fn formula(document: &mut Document, field: &str, expression: Expression) {
    insert(
        document,
        field,
        FieldType::Number,
        Value::Formula(expression),
    );
}

fn field(field: &str) -> FieldRef {
    FieldRef::new("entity", field)
}

fn failed(document: &Document) -> tachiko_formula_engine::CalculationFailureReport {
    match calculate_full(document) {
        CalculationOutcome::Complete(_) => panic!("expected complete failure report"),
        CalculationOutcome::Failed(report) => report,
    }
}

fn comprehensive_failure_document() -> Document {
    let mut document = document();
    insert(
        &mut document,
        "name",
        FieldType::Text,
        Value::Text("text".to_owned()),
    );

    let mut oversized = reference("missing-structural");
    for _ in 0..65 {
        oversized = Expression::Add {
            left: Box::new(oversized),
            right: Box::new(literal(1.0)),
        };
    }
    formula(&mut document, "structural", oversized);
    formula(
        &mut document,
        "binding",
        Expression::Add {
            left: Box::new(reference("missing-binding")),
            right: Box::new(reference("name")),
        },
    );

    formula(&mut document, "cycle-a", reference("cycle-b"));
    formula(&mut document, "cycle-b", reference("cycle-c"));
    formula(
        &mut document,
        "cycle-c",
        Expression::Divide {
            left: Box::new(reference("cycle-a")),
            right: Box::new(literal(0.0)),
        },
    );
    formula(
        &mut document,
        "depends-earlier",
        Expression::Add {
            left: Box::new(reference("cycle-a")),
            right: Box::new(reference("binding")),
        },
    );
    formula(
        &mut document,
        "zero",
        Expression::Divide {
            left: Box::new(literal(1.0)),
            right: Box::new(literal(0.0)),
        },
    );
    formula(&mut document, "depends-zero", reference("zero"));
    formula(
        &mut document,
        "independent",
        Expression::Add {
            left: Box::new(literal(2.0)),
            right: Box::new(literal(3.0)),
        },
    );
    document
}

#[test]
fn full_oracle_accumulates_failures_with_phase_precedence() {
    let document = comprehensive_failure_document();

    let report = failed(&document);
    let failures = report.failures();

    assert!(matches!(
        failures.get(&field("structural")),
        Some(FormulaFailure::Structural { .. })
    ));
    assert_eq!(
        failures.get(&field("binding")),
        Some(&FormulaFailure::InvalidReferences {
            missing: BTreeSet::from([field("missing-binding")]),
            non_numeric: BTreeSet::from([field("name")]),
        })
    );

    let cycle = BTreeSet::from([field("cycle-a"), field("cycle-b"), field("cycle-c")]);
    for member in &cycle {
        assert_eq!(
            failures.get(member),
            Some(&FormulaFailure::Cycle {
                members: cycle.clone()
            })
        );
    }
    assert_eq!(
        failures.get(&field("depends-earlier")),
        Some(&FormulaFailure::FailedDependency {
            dependencies: BTreeSet::from([field("binding"), field("cycle-a")]),
        })
    );
    assert_eq!(
        failures.get(&field("zero")),
        Some(&FormulaFailure::DivisionByZero)
    );
    assert_eq!(
        failures.get(&field("depends-zero")),
        Some(&FormulaFailure::FailedDependency {
            dependencies: BTreeSet::from([field("zero")]),
        })
    );
    assert!(!failures.contains_key(&field("independent")));

    assert_eq!(
        report.dependencies().get(&field("structural")),
        Some(&BTreeSet::from([field("missing-structural")]))
    );
    assert_eq!(failures.keys().cloned().collect::<Vec<_>>(), {
        let mut keys = failures.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        keys
    });
}

#[test]
fn binding_precedes_self_cycle_and_collects_every_direct_invalid_target() {
    let mut document = document();
    insert(
        &mut document,
        "text",
        FieldType::Text,
        Value::Text("text".to_owned()),
    );
    formula(
        &mut document,
        "formula",
        Expression::Add {
            left: Box::new(reference("formula")),
            right: Box::new(Expression::Add {
                left: Box::new(reference("missing")),
                right: Box::new(reference("text")),
            }),
        },
    );

    assert_eq!(
        failed(&document).failures().get(&field("formula")),
        Some(&FormulaFailure::InvalidReferences {
            missing: BTreeSet::from([field("missing")]),
            non_numeric: BTreeSet::from([field("text")]),
        })
    );
}

#[test]
fn local_evaluation_selects_the_first_left_to_right_failure() {
    let mut document = document();
    formula(
        &mut document,
        "formula",
        Expression::Add {
            left: Box::new(Expression::Divide {
                left: Box::new(literal(1.0)),
                right: Box::new(literal(0.0)),
            }),
            right: Box::new(Expression::Multiply {
                left: Box::new(literal(f64::MAX)),
                right: Box::new(literal(2.0)),
            }),
        },
    );

    assert_eq!(
        failed(&document).failures().get(&field("formula")),
        Some(&FormulaFailure::DivisionByZero)
    );
}

#[test]
fn missing_declared_input_is_a_local_evaluation_failure() {
    let mut document = document();
    document.schemas.get_mut("schema").unwrap().fields.insert(
        FieldId::from("input"),
        FieldDefinition {
            id: "input".into(),
            key: "input".into(),
            field_type: FieldType::Number,
            required: false,
        },
    );
    formula(&mut document, "formula", reference("input"));

    assert_eq!(
        failed(&document).failures().get(&field("formula")),
        Some(&FormulaFailure::MissingInput {
            reference: field("input")
        })
    );
}
