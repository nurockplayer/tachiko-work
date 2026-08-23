use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn balance_document() -> Document {
    let schema = Schema {
        id: SchemaId::from("weapon"),
        key: SchemaKey::from("weapon"),
        fields: BTreeMap::from([
            (FieldId::from("damage"), number_field("damage")),
            (
                FieldId::from("attack_interval"),
                number_field("attack_interval"),
            ),
            (FieldId::from("dps"), number_field("dps")),
            (FieldId::from("burst"), number_field("burst")),
            (
                FieldId::from("name"),
                FieldDefinition {
                    id: FieldId::from("name"),
                    key: FieldKey::from("name"),
                    field_type: FieldType::Text,
                    required: true,
                },
            ),
        ]),
    };
    let weapon = Entity {
        id: EntityId::from("sword"),
        key: "sword".into(),
        schema: SchemaId::from("weapon"),
        fields: BTreeMap::from([
            (FieldId::from("damage"), number(100.0)),
            (FieldId::from("attack_interval"), number(1.25)),
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
                    right: Box::new(numeric(2.0)),
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

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Number,
        required: true,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn numeric(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn expected(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

#[test]
fn formulas_resolve_numeric_and_formula_references() {
    let calculation = calculate(&balance_document()).unwrap();

    assert_eq!(
        calculation.value(&FieldRef::new("sword", "damage")),
        Some(expected(100.0))
    );
    assert_eq!(
        calculation.value(&FieldRef::new("sword", "dps")),
        Some(expected(80.0))
    );
    assert_eq!(
        calculation.value(&FieldRef::new("sword", "burst")),
        Some(expected(160.0))
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
        .insert(FieldId::from("damage"), number(120.0));

    let recalculated = calculate(&changed).unwrap();

    assert_eq!(
        recalculated.value(&FieldRef::new("sword", "dps")),
        Some(expected(96.0))
    );
    assert_eq!(
        recalculated.value(&FieldRef::new("sword", "burst")),
        Some(expected(192.0))
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
        .insert(FieldId::from("attack_interval"), number(0.0));

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
fn long_acyclic_dependency_chains_are_stack_safe() {
    const CHAIN_LENGTH: usize = 20_000;

    let mut fields = BTreeMap::new();
    let mut definitions = BTreeMap::new();
    for index in 0..CHAIN_LENGTH {
        let field_id = format!("field-{index:05}");
        let value = if index + 1 == CHAIN_LENGTH {
            number(1.0)
        } else {
            Value::Formula(reference("chain", &format!("field-{:05}", index + 1)))
        };
        definitions.insert(FieldId::from(field_id.as_str()), number_field(&field_id));
        fields.insert(FieldId::from(field_id), value);
    }
    let document = Document {
        id: DocumentId::from("chain"),
        title: "Long dependency chain".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("chain"),
            Schema {
                id: SchemaId::from("chain"),
                key: SchemaKey::from("chain"),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("chain"),
            Entity {
                id: EntityId::from("chain"),
                key: "chain".into(),
                schema: SchemaId::from("chain"),
                fields,
            },
        )]),
    };

    let calculation = calculate(&document).unwrap();

    assert_eq!(
        calculation.value(&FieldRef::new("chain", "field-00000")),
        Some(expected(1.0))
    );
    assert_eq!(calculation.values().len(), CHAIN_LENGTH);
    assert_eq!(
        calculation
            .affected_by(&FieldRef::new(
                "chain",
                format!("field-{:05}", CHAIN_LENGTH - 1),
            ))
            .len(),
        CHAIN_LENGTH - 1
    );
}

#[test]
fn arithmetic_minimum_and_maximum_are_supported() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Subtract {
            left: Box::new(Expression::Maximum {
                left: Box::new(Expression::Add {
                    left: Box::new(numeric(10.0)),
                    right: Box::new(numeric(5.0)),
                }),
                right: Box::new(numeric(12.0)),
            }),
            right: Box::new(Expression::Minimum {
                left: Box::new(numeric(4.0)),
                right: Box::new(numeric(7.0)),
            }),
        }),
    );

    let calculation = calculate(&document).unwrap();

    assert_eq!(
        calculation.value(&FieldRef::new("sword", "burst")),
        Some(expected(11.0))
    );
}

#[test]
fn arithmetic_preserves_gradual_underflow_and_normalizes_zero() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Multiply {
            left: Box::new(numeric(f64::from_bits(2))),
            right: Box::new(numeric(0.5)),
        }),
    );
    let subnormal = calculate(&document)
        .unwrap()
        .value(&FieldRef::new("sword", "burst"))
        .unwrap();
    assert_eq!(subnormal.to_bits(), 1);

    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Multiply {
            left: Box::new(numeric(f64::from_bits(1))),
            right: Box::new(numeric(0.5)),
        }),
    );
    let zero = calculate(&document)
        .unwrap()
        .value(&FieldRef::new("sword", "burst"))
        .unwrap();
    assert_eq!(zero.to_bits(), 0);
}

#[test]
fn arithmetic_overflow_is_a_typed_non_finite_result() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Multiply {
            left: Box::new(numeric(f64::MAX)),
            right: Box::new(numeric(2.0)),
        }),
    );

    assert!(matches!(
        calculate(&document).unwrap_err(),
        CalculationError::NonFiniteResult { field }
            if field == FieldRef::new("sword", "burst")
    ));
}

#[test]
fn evaluation_obeys_the_bound_tree_without_reassociation() {
    let mut document = balance_document();
    let left_associated = Expression::Add {
        left: Box::new(Expression::Add {
            left: Box::new(numeric(1e16)),
            right: Box::new(numeric(-1e16)),
        }),
        right: Box::new(numeric(1.0)),
    };
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("burst"), Value::Formula(left_associated));
    assert_eq!(
        calculate(&document)
            .unwrap()
            .value(&FieldRef::new("sword", "burst")),
        Some(expected(1.0))
    );

    let right_associated = Expression::Add {
        left: Box::new(numeric(1e16)),
        right: Box::new(Expression::Add {
            left: Box::new(numeric(-1e16)),
            right: Box::new(numeric(1.0)),
        }),
    };
    document
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("burst"), Value::Formula(right_associated));
    assert_eq!(
        calculate(&document)
            .unwrap()
            .value(&FieldRef::new("sword", "burst")),
        Some(expected(0.0))
    );
}
