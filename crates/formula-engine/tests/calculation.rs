use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};

fn balance_document() -> Document {
    let schema = Schema {
        id: SchemaId::from("weapon"),
        fields: BTreeMap::from([
            (FieldId::from("damage"), number_field()),
            (FieldId::from("attack_interval"), number_field()),
            (FieldId::from("dps"), number_field()),
            (FieldId::from("burst"), number_field()),
            (
                FieldId::from("name"),
                FieldDefinition {
                    field_type: FieldType::Text,
                    required: true,
                },
            ),
        ]),
    };
    let weapon = Entity {
        id: EntityId::from("sword"),
        schema: SchemaId::from("weapon"),
        fields: BTreeMap::from([
            (FieldId::from("damage"), Value::Number(100.0)),
            (FieldId::from("attack_interval"), Value::Number(1.25)),
            (
                FieldId::from("dps"),
                Value::Formula(Expression::Divide {
                    left: Box::new(reference("sword", "damage")),
                    right: Box::new(reference("sword", "attack_interval")),
                }),
            ),
            (
                FieldId::from("burst"),
                Value::Formula(Expression::Multiply {
                    left: Box::new(reference("sword", "dps")),
                    right: Box::new(Expression::Number(2.0)),
                }),
            ),
            (FieldId::from("name"), Value::Text("Sword".to_owned())),
        ]),
    };

    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(SchemaId::from("weapon"), schema)]),
        entities: BTreeMap::from([(EntityId::from("sword"), weapon)]),
    }
}

fn number_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Number,
        required: true,
    }
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

#[test]
fn formulas_resolve_numeric_and_formula_references() {
    let calculation = calculate(&balance_document()).unwrap();

    assert_eq!(
        calculation.value(&FieldRef::new("sword", "damage")),
        Some(100.0)
    );
    assert_eq!(
        calculation.value(&FieldRef::new("sword", "dps")),
        Some(80.0)
    );
    assert_eq!(
        calculation.value(&FieldRef::new("sword", "burst")),
        Some(160.0)
    );
}

#[test]
fn dependency_impact_is_direct_and_transitive() {
    let calculation = calculate(&balance_document()).unwrap();

    assert_eq!(
        calculation.affected_by(&FieldRef::new("sword", "damage")),
        vec![
            FieldRef::new("sword", "burst"),
            FieldRef::new("sword", "dps")
        ]
    );
    assert_eq!(
        calculation.dependencies_of(&FieldRef::new("sword", "burst")),
        Some(&BTreeSet::from([FieldRef::new("sword", "dps")]))
    );
}

#[test]
fn recalculation_reflects_changed_inputs() {
    let mut changed = balance_document();
    changed
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), Value::Number(120.0));

    let recalculated = calculate(&changed).unwrap();

    assert_eq!(
        recalculated.value(&FieldRef::new("sword", "dps")),
        Some(96.0)
    );
    assert_eq!(
        recalculated.value(&FieldRef::new("sword", "burst")),
        Some(192.0)
    );
}

#[test]
fn calculation_order_is_stable() {
    let calculation = calculate(&balance_document()).unwrap();
    let fields: Vec<_> = calculation.values().keys().cloned().collect();

    assert_eq!(
        fields,
        vec![
            FieldRef::new("sword", "attack_interval"),
            FieldRef::new("sword", "burst"),
            FieldRef::new("sword", "damage"),
            FieldRef::new("sword", "dps"),
        ]
    );
}

#[test]
fn division_by_zero_identifies_the_formula() {
    let mut document = balance_document();
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), Value::Number(0.0));

    let error = calculate(&document).unwrap_err();

    assert!(matches!(
        error,
        CalculationError::DivisionByZero { formula }
            if formula == FieldRef::new("sword", "dps")
    ));
}

#[test]
fn non_numeric_reference_is_explicit() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(reference("sword", "name")),
    );

    let error = calculate(&document).unwrap_err();

    assert!(matches!(
        error,
        CalculationError::NonNumericReference { reference }
            if reference == FieldRef::new("sword", "name")
    ));
}

#[test]
fn missing_reference_is_explicit() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(reference("sword", "missing")),
    );

    let error = calculate(&document).unwrap_err();

    assert!(matches!(
        error,
        CalculationError::MissingReference { reference }
            if reference == FieldRef::new("sword", "missing")
    ));
}

#[test]
fn cycles_report_a_deterministic_dependency_path() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(reference("sword", "burst")),
    );

    let error = calculate(&document).unwrap_err();

    assert!(matches!(
        error,
        CalculationError::Cycle { path }
            if path == vec![
                FieldRef::new("sword", "burst"),
                FieldRef::new("sword", "dps"),
                FieldRef::new("sword", "burst"),
            ]
    ));
}

#[test]
fn arithmetic_minimum_and_maximum_are_supported() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Subtract {
            left: Box::new(Expression::Maximum {
                left: Box::new(Expression::Add {
                    left: Box::new(Expression::Number(10.0)),
                    right: Box::new(Expression::Number(5.0)),
                }),
                right: Box::new(Expression::Number(12.0)),
            }),
            right: Box::new(Expression::Minimum {
                left: Box::new(Expression::Number(4.0)),
                right: Box::new(Expression::Number(7.0)),
            }),
        }),
    );

    let calculation = calculate(&document).unwrap();

    assert_eq!(
        calculation.value(&FieldRef::new("sword", "burst")),
        Some(11.0)
    );
}
