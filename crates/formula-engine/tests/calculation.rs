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

fn next_random(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn generated_formula(
    index: usize,
    other_index: usize,
    expected_values: &[Number],
) -> (Expression, Number, BTreeSet<FieldRef>) {
    let previous_id = format!("field-{:03}", index - 1);
    let other_id = format!("field-{other_index:03}");
    let previous = expected_values[index - 1];
    let other = expected_values[other_index];
    match index % 6 {
        0 => (
            Expression::Add {
                left: Box::new(reference("node", &previous_id)),
                right: Box::new(numeric(0.25)),
            },
            expected(previous.get() + 0.25),
            BTreeSet::from([FieldRef::new("node", previous_id.as_str())]),
        ),
        1 => (
            Expression::Subtract {
                left: Box::new(reference("node", &previous_id)),
                right: Box::new(numeric(0.5)),
            },
            expected(previous.get() - 0.5),
            BTreeSet::from([FieldRef::new("node", previous_id.as_str())]),
        ),
        2 => (
            Expression::Multiply {
                left: Box::new(reference("node", &previous_id)),
                right: Box::new(numeric(0.5)),
            },
            expected(previous.get() * 0.5),
            BTreeSet::from([FieldRef::new("node", previous_id.as_str())]),
        ),
        3 => (
            Expression::Divide {
                left: Box::new(reference("node", &previous_id)),
                right: Box::new(numeric(2.0)),
            },
            expected(previous.get() / 2.0),
            BTreeSet::from([FieldRef::new("node", previous_id.as_str())]),
        ),
        4 => (
            Expression::Minimum {
                left: Box::new(reference("node", &previous_id)),
                right: Box::new(reference("node", &other_id)),
            },
            previous.min(other),
            BTreeSet::from([
                FieldRef::new("node", previous_id.as_str()),
                FieldRef::new("node", other_id.as_str()),
            ]),
        ),
        _ => (
            Expression::Maximum {
                left: Box::new(reference("node", &previous_id)),
                right: Box::new(reference("node", &other_id)),
            },
            previous.max(other),
            BTreeSet::from([
                FieldRef::new("node", previous_id.as_str()),
                FieldRef::new("node", other_id.as_str()),
            ]),
        ),
    }
}

fn rename_generated_keys(document: &mut Document) {
    document.entities.get_mut("node").unwrap().key = "renamed-source".into();
    document.schemas.get_mut("schema").unwrap().key = "renamed-numbers".into();
    for (index, definition) in document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .values_mut()
        .enumerate()
    {
        definition.key = format!("renamed-{index:03}").into();
    }
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
fn schema_declaration_is_authoritative_for_bound_reference_type() {
    let mut document = balance_document();
    document
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .get_mut("damage")
        .unwrap()
        .field_type = FieldType::Text;

    assert!(matches!(
        calculate(&document).unwrap_err(),
        CalculationError::NonNumericReference { reference }
            if reference == FieldRef::new("sword", "damage")
    ));
}

#[test]
fn stale_bound_reference_cannot_read_an_undeclared_entity_value() {
    let mut document = balance_document();
    document
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .remove("damage");

    assert!(matches!(
        calculate(&document).unwrap_err(),
        CalculationError::MissingReference { reference }
            if reference == FieldRef::new("sword", "damage")
    ));
}

#[test]
fn compatibility_cycle_error_preserves_complete_semantic_membership() {
    let mut document = balance_document();
    document.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(reference("sword", "burst")),
    );

    let error = calculate(&document).unwrap_err();

    assert!(matches!(
        error,
        CalculationError::Cycle { members }
            if members == BTreeSet::from([
                FieldRef::new("sword", "burst"),
                FieldRef::new("sword", "dps"),
            ])
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
fn generated_dags_match_an_independent_oracle_and_survive_key_renames() {
    const GRAPH_COUNT: usize = 32;
    const FIELD_COUNT: usize = 64;

    for graph in 0..GRAPH_COUNT {
        let mut seed = 0x94d0_49bb_1331_11eb_u64 ^ graph as u64;
        let mut definitions = BTreeMap::new();
        let mut values = BTreeMap::new();
        let mut expected_values = Vec::new();
        let mut expected_dependencies = BTreeMap::new();

        for index in 0..FIELD_COUNT {
            let field_id = format!("field-{index:03}");
            definitions.insert(FieldId::from(field_id.as_str()), number_field(&field_id));

            if index < 2 {
                let value =
                    f64::from(u32::try_from(next_random(&mut seed) % 2_001).unwrap()) - 1_000.0;
                let value = expected(value);
                expected_values.push(value);
                values.insert(FieldId::from(field_id), Value::Number(value));
                continue;
            }

            let modulus = u64::try_from(index).unwrap();
            let other_index = usize::try_from(next_random(&mut seed) % modulus).unwrap();
            let (expression, result, dependencies) =
                generated_formula(index, other_index, &expected_values);
            expected_values.push(result);
            expected_dependencies.insert(FieldRef::new("node", field_id.as_str()), dependencies);
            values.insert(FieldId::from(field_id), Value::Formula(expression));
        }

        let mut document = Document {
            id: DocumentId::from("generated-dag"),
            title: "Generated formula DAG".to_owned(),
            schemas: BTreeMap::from([(
                SchemaId::from("schema"),
                Schema {
                    id: SchemaId::from("schema"),
                    key: SchemaKey::from("numbers"),
                    fields: definitions,
                },
            )]),
            entities: BTreeMap::from([(
                EntityId::from("node"),
                Entity {
                    id: EntityId::from("node"),
                    key: "source".into(),
                    schema: SchemaId::from("schema"),
                    fields: values,
                },
            )]),
        };

        let calculation = calculate(&document).unwrap();
        for (index, value) in expected_values.iter().enumerate() {
            assert_eq!(
                calculation.value(&FieldRef::new("node", format!("field-{index:03}"))),
                Some(*value),
                "graph {graph}, field {index}"
            );
        }
        assert_eq!(calculation.dependencies(), &expected_dependencies);
        for _ in 0..2 {
            assert_eq!(calculate(&document).unwrap(), calculation, "graph {graph}");
        }

        let mut transitive_dependencies = vec![BTreeSet::new(); FIELD_COUNT];
        for index in 2..FIELD_COUNT {
            let field = FieldRef::new("node", format!("field-{index:03}"));
            for dependency in &expected_dependencies[&field] {
                let dependency_index = dependency
                    .field
                    .as_str()
                    .strip_prefix("field-")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                transitive_dependencies[index].insert(dependency_index);
                let inherited = transitive_dependencies[dependency_index].clone();
                transitive_dependencies[index].extend(inherited);
            }
        }
        for root in 0..FIELD_COUNT {
            let expected_affected = (0..FIELD_COUNT)
                .filter(|index| transitive_dependencies[*index].contains(&root))
                .map(|index| FieldRef::new("node", format!("field-{index:03}")))
                .collect::<Vec<_>>();
            assert_eq!(
                calculation.affected_by(&FieldRef::new("node", format!("field-{root:03}"))),
                expected_affected,
                "graph {graph}, dirty root {root}"
            );
        }

        rename_generated_keys(&mut document);
        assert_eq!(calculate(&document).unwrap(), calculation, "graph {graph}");
    }
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
