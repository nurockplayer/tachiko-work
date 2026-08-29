use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{
    CalculationFailure, CalculationFailures, CalculationOutcome, ExpressionComplexityError,
    ReferenceFailure, RetainedCalculationState, calculate, calculate_complete,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn field(id: &str, field_type: FieldType, value: Value) -> (FieldDefinition, Value) {
    (
        FieldDefinition {
            id: FieldId::from(id),
            key: FieldKey::from(id),
            field_type,
            required: true,
        },
        value,
    )
}

fn document(fields: Vec<(&str, FieldType, Value)>) -> Document {
    let mut definitions = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (id, field_type, value) in fields {
        let (definition, value) = field(id, field_type, value);
        definitions.insert(FieldId::from(id), definition);
        values.insert(FieldId::from(id), value);
    }

    Document {
        id: DocumentId::from("document-stable"),
        title: "Complete formula oracle".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema-stable"),
            Schema {
                id: SchemaId::from("schema-stable"),
                key: SchemaKey::from("schema"),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity-stable"),
            Entity {
                id: EntityId::from("entity-stable"),
                key: "entity".into(),
                schema: SchemaId::from("schema-stable"),
                fields: values,
            },
        )]),
    }
}

fn formula(expression: Expression) -> Value {
    Value::Formula(expression)
}

fn numeric(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn reference(field: &str) -> Expression {
    Expression::Reference(node(field))
}

fn node(field: &str) -> FieldRef {
    FieldRef::new("entity-stable", field)
}

fn divide(left: Expression, right: Expression) -> Expression {
    Expression::Divide {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn add(left: Expression, right: Expression) -> Expression {
    Expression::Add {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn next_random(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn failed(outcome: CalculationOutcome) -> CalculationFailures {
    let CalculationOutcome::Failed(failures) = outcome else {
        panic!("counterexample must produce a complete failure outcome")
    };
    failures
}

#[test]
fn complete_oracle_preserves_disjoint_sccs_and_an_independent_evaluation_failure() {
    let document = document(vec![
        (
            "cycle-a-1",
            FieldType::Number,
            formula(reference("cycle-a-2")),
        ),
        (
            "cycle-a-2",
            FieldType::Number,
            formula(reference("cycle-a-1")),
        ),
        (
            "cycle-b-1",
            FieldType::Number,
            formula(reference("cycle-b-2")),
        ),
        (
            "cycle-b-2",
            FieldType::Number,
            formula(reference("cycle-b-3")),
        ),
        (
            "cycle-b-3",
            FieldType::Number,
            formula(reference("cycle-b-1")),
        ),
        (
            "evaluation",
            FieldType::Number,
            formula(divide(numeric(1.0), numeric(0.0))),
        ),
    ]);

    let failures = failed(calculate_complete(&document));
    let cycle_a = BTreeSet::from([node("cycle-a-1"), node("cycle-a-2")]);
    let cycle_b = BTreeSet::from([node("cycle-b-1"), node("cycle-b-2"), node("cycle-b-3")]);

    assert_eq!(
        failures.failures(),
        &BTreeMap::from([
            (
                node("cycle-a-1"),
                CalculationFailure::Cycle {
                    members: cycle_a.clone(),
                },
            ),
            (
                node("cycle-a-2"),
                CalculationFailure::Cycle { members: cycle_a },
            ),
            (
                node("cycle-b-1"),
                CalculationFailure::Cycle {
                    members: cycle_b.clone(),
                },
            ),
            (
                node("cycle-b-2"),
                CalculationFailure::Cycle {
                    members: cycle_b.clone(),
                },
            ),
            (
                node("cycle-b-3"),
                CalculationFailure::Cycle { members: cycle_b },
            ),
            (node("evaluation"), CalculationFailure::DivisionByZero,),
        ])
    );
}

#[test]
fn binding_failures_accumulate_all_direct_stable_targets_before_cycle_and_evaluation() {
    let mut document = document(vec![
        ("cycle-1", FieldType::Number, formula(reference("cycle-2"))),
        ("cycle-2", FieldType::Number, formula(reference("cycle-1"))),
        (
            "invalid-references",
            FieldType::Number,
            formula(add(reference("missing-stable"), reference("text-stable"))),
        ),
        (
            "text-stable",
            FieldType::Text,
            Value::Text("not numeric".to_owned()),
        ),
        (
            "evaluation",
            FieldType::Number,
            formula(divide(numeric(1.0), numeric(0.0))),
        ),
    ]);
    document
        .schemas
        .get_mut("schema-stable")
        .unwrap()
        .fields
        .remove("missing-stable");

    let failures = failed(calculate_complete(&document));
    let cycle = BTreeSet::from([node("cycle-1"), node("cycle-2")]);

    assert_eq!(
        failures.failures(),
        &BTreeMap::from([
            (
                node("cycle-1"),
                CalculationFailure::Cycle {
                    members: cycle.clone(),
                },
            ),
            (
                node("cycle-2"),
                CalculationFailure::Cycle { members: cycle },
            ),
            (node("evaluation"), CalculationFailure::DivisionByZero,),
            (
                node("invalid-references"),
                CalculationFailure::InvalidReferences {
                    targets: BTreeMap::from([
                        (node("missing-stable"), ReferenceFailure::Missing),
                        (node("text-stable"), ReferenceFailure::NonNumeric),
                    ]),
                },
            ),
        ])
    );
}

#[test]
fn failed_dependency_subjects_are_direct_and_take_precedence_over_local_evaluation() {
    let document = document(vec![
        (
            "root-a",
            FieldType::Number,
            formula(divide(numeric(1.0), numeric(0.0))),
        ),
        (
            "root-b",
            FieldType::Number,
            formula(Expression::Multiply {
                left: Box::new(numeric(f64::MAX)),
                right: Box::new(numeric(2.0)),
            }),
        ),
        (
            "middle",
            FieldType::Number,
            formula(add(reference("root-a"), reference("root-b"))),
        ),
        ("leaf", FieldType::Number, formula(reference("middle"))),
        (
            "local-and-failed",
            FieldType::Number,
            formula(add(divide(numeric(1.0), numeric(0.0)), reference("root-a"))),
        ),
    ]);

    let failures = failed(calculate_complete(&document));

    assert_eq!(
        failures.failures(),
        &BTreeMap::from([
            (
                node("leaf"),
                CalculationFailure::FailedDependencies {
                    dependencies: BTreeSet::from([node("middle")]),
                },
            ),
            (
                node("local-and-failed"),
                CalculationFailure::FailedDependencies {
                    dependencies: BTreeSet::from([node("root-a")]),
                },
            ),
            (
                node("middle"),
                CalculationFailure::FailedDependencies {
                    dependencies: BTreeSet::from([node("root-a"), node("root-b")]),
                },
            ),
            (node("root-a"), CalculationFailure::DivisionByZero),
            (node("root-b"), CalculationFailure::NonFiniteResult),
        ])
    );
}

#[test]
fn failed_outcome_is_stably_ordered_deterministic_and_contains_no_partial_calculation() {
    let document = document(vec![
        (
            "z-failure",
            FieldType::Number,
            formula(divide(numeric(1.0), numeric(0.0))),
        ),
        (
            "a-success",
            FieldType::Number,
            formula(add(numeric(1.0), numeric(2.0))),
        ),
        (
            "m-failure",
            FieldType::Number,
            formula(Expression::Multiply {
                left: Box::new(numeric(f64::MAX)),
                right: Box::new(numeric(2.0)),
            }),
        ),
    ]);

    let first = calculate_complete(&document);
    for _ in 0..32 {
        assert_eq!(calculate_complete(&document), first);
    }

    let failures = failed(first);
    assert_eq!(
        failures.failures().keys().cloned().collect::<Vec<_>>(),
        vec![node("m-failure"), node("z-failure")]
    );
    assert_eq!(
        failures.dependencies(),
        &BTreeMap::from([
            (node("a-success"), BTreeSet::new()),
            (node("m-failure"), BTreeSet::new()),
            (node("z-failure"), BTreeSet::new()),
        ])
    );
}

#[test]
fn primary_failure_precedence_excludes_earlier_failed_nodes_from_cycle_authority() {
    let mut structurally_invalid = reference("structural");
    for _ in 0..64 {
        structurally_invalid = add(structurally_invalid, numeric(1.0));
    }
    let document = document(vec![
        (
            "structural",
            FieldType::Number,
            formula(structurally_invalid),
        ),
        (
            "structural-dependent",
            FieldType::Number,
            formula(reference("structural")),
        ),
        (
            "binding-a",
            FieldType::Number,
            formula(add(reference("binding-b"), reference("missing"))),
        ),
        (
            "binding-b",
            FieldType::Number,
            formula(reference("binding-a")),
        ),
        (
            "cycle-a",
            FieldType::Number,
            formula(add(reference("cycle-b"), reference("local-root"))),
        ),
        ("cycle-b", FieldType::Number, formula(reference("cycle-a"))),
        (
            "local-root",
            FieldType::Number,
            formula(divide(numeric(1.0), numeric(0.0))),
        ),
    ]);

    let failures = failed(calculate_complete(&document));
    assert_eq!(
        failures.failures().get(&node("structural")),
        Some(&CalculationFailure::InvalidExpression {
            error: ExpressionComplexityError::DepthLimit,
        })
    );
    assert_eq!(
        failures.failures().get(&node("structural-dependent")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([node("structural")]),
        })
    );
    assert_eq!(
        failures.failures().get(&node("binding-a")),
        Some(&CalculationFailure::InvalidReferences {
            targets: BTreeMap::from([(node("missing"), ReferenceFailure::Missing)]),
        })
    );
    assert_eq!(
        failures.failures().get(&node("binding-b")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([node("binding-a")]),
        })
    );
    let cycle = BTreeSet::from([node("cycle-a"), node("cycle-b")]);
    assert_eq!(
        failures.failures().get(&node("cycle-a")),
        Some(&CalculationFailure::Cycle {
            members: cycle.clone(),
        })
    );
    assert_eq!(
        failures.failures().get(&node("cycle-b")),
        Some(&CalculationFailure::Cycle { members: cycle })
    );
    assert_eq!(
        failures.failures().get(&node("local-root")),
        Some(&CalculationFailure::DivisionByZero)
    );
    assert_eq!(
        failures.dependencies().get(&node("structural")),
        Some(&BTreeSet::from([node("structural")]))
    );
}

#[test]
fn local_evaluation_failure_selection_remains_left_to_right() {
    let division = divide(numeric(1.0), numeric(0.0));
    let overflow = Expression::Multiply {
        left: Box::new(numeric(f64::MAX)),
        right: Box::new(numeric(2.0)),
    };
    let document = document(vec![
        (
            "division-first",
            FieldType::Number,
            formula(add(division.clone(), overflow.clone())),
        ),
        (
            "overflow-first",
            FieldType::Number,
            formula(add(overflow, division)),
        ),
    ]);

    assert_eq!(
        failed(calculate_complete(&document)).failures(),
        &BTreeMap::from([
            (node("division-first"), CalculationFailure::DivisionByZero,),
            (node("overflow-first"), CalculationFailure::NonFiniteResult,),
        ])
    );
}

#[test]
fn branching_dag_with_a_shared_child_is_not_a_false_positive_scc() {
    let document = document(vec![
        (
            "a",
            FieldType::Number,
            formula(add(reference("b"), reference("c"))),
        ),
        ("b", FieldType::Number, formula(reference("c"))),
        ("c", FieldType::Number, formula(numeric(1.0))),
    ]);

    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("branching DAG must not be classified as cyclic")
    };
    assert_eq!(
        calculation.value(&node("a")),
        Some(Number::new(2.0).unwrap())
    );
    assert_eq!(
        calculation.value(&node("b")),
        Some(Number::new(1.0).unwrap())
    );
    assert_eq!(
        calculation.value(&node("c")),
        Some(Number::new(1.0).unwrap())
    );
}

fn generated_reference_document(targets: &[usize]) -> Document {
    let mut definitions = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (index, target) in targets.iter().copied().enumerate() {
        let id = format!("field-{index:03}");
        definitions.insert(FieldId::from(id.as_str()), number_field(&id));
        let value = if index == 0 {
            Value::Number(Number::new(1.0).unwrap())
        } else {
            formula(reference(&format!("field-{target:03}")))
        };
        values.insert(FieldId::from(id), value);
    }

    Document {
        id: DocumentId::from("generated-document"),
        title: "Generated reference graph".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema-stable"),
            Schema {
                id: SchemaId::from("schema-stable"),
                key: SchemaKey::from("schema"),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity-stable"),
            Entity {
                id: EntityId::from("entity-stable"),
                key: "entity".into(),
                schema: SchemaId::from("schema-stable"),
                fields: values,
            },
        )]),
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

fn reaches(targets: &[usize], from: usize, target: usize) -> bool {
    if from == target {
        return true;
    }
    let mut current = from;
    let mut visited = BTreeSet::new();
    while current != 0 && visited.insert(current) {
        current = targets[current];
        if current == target {
            return true;
        }
    }
    false
}

#[test]
fn generated_acyclic_and_cyclic_graphs_match_an_independent_reachability_oracle() {
    const GRAPH_COUNT: usize = 48;
    const FIELD_COUNT: usize = 40;

    for graph in 0..GRAPH_COUNT {
        let mut seed = 0xd1b5_4a32_d192_ed03_u64 ^ graph as u64;
        let cyclic = graph % 2 == 1;
        let mut targets = vec![0; FIELD_COUNT];
        for (index, target) in targets.iter_mut().enumerate().skip(1) {
            *target = if cyclic && index == 1 {
                1
            } else if cyclic {
                usize::try_from(next_random(&mut seed) % FIELD_COUNT as u64).unwrap()
            } else {
                usize::try_from(next_random(&mut seed) % index as u64).unwrap()
            };
        }
        let document = generated_reference_document(&targets);
        let dependencies = (1..FIELD_COUNT)
            .map(|index| {
                (
                    node(&format!("field-{index:03}")),
                    BTreeSet::from([node(&format!("field-{:03}", targets[index]))]),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let first = calculate_complete(&document);
        assert_eq!(calculate_complete(&document), first, "graph {graph}");

        if !cyclic {
            let CalculationOutcome::Complete(calculation) = first else {
                panic!("generated DAG {graph} must calculate")
            };
            assert_eq!(calculation.dependencies(), &dependencies, "graph {graph}");
            assert_eq!(calculation.values().len(), FIELD_COUNT, "graph {graph}");
            assert!(
                calculation
                    .values()
                    .values()
                    .all(|value| *value == Number::new(1.0).unwrap()),
                "graph {graph}"
            );
            continue;
        }

        let cycle_memberships = (1..FIELD_COUNT)
            .filter_map(|index| {
                let members = (1..FIELD_COUNT)
                    .filter(|candidate| {
                        reaches(&targets, index, *candidate) && reaches(&targets, *candidate, index)
                    })
                    .collect::<BTreeSet<_>>();
                (members.len() > 1 || targets[index] == index).then_some((index, members))
            })
            .collect::<BTreeMap<_, _>>();
        let cyclic_nodes = cycle_memberships.keys().copied().collect::<BTreeSet<_>>();
        let mut expected_failures = BTreeMap::new();
        for index in 1..FIELD_COUNT {
            if let Some(members) = cycle_memberships.get(&index) {
                expected_failures.insert(
                    node(&format!("field-{index:03}")),
                    CalculationFailure::Cycle {
                        members: members
                            .iter()
                            .map(|member| node(&format!("field-{member:03}")))
                            .collect(),
                    },
                );
                continue;
            }
            let target = targets[index];
            if cyclic_nodes
                .iter()
                .any(|cycle| reaches(&targets, target, *cycle))
            {
                expected_failures.insert(
                    node(&format!("field-{index:03}")),
                    CalculationFailure::FailedDependencies {
                        dependencies: BTreeSet::from([node(&format!("field-{target:03}"))]),
                    },
                );
            }
        }

        let failures = failed(first);
        assert_eq!(failures.dependencies(), &dependencies, "graph {graph}");
        assert_eq!(failures.failures(), &expected_failures, "graph {graph}");
    }
}

#[test]
fn compatibility_projection_of_a_long_failed_chain_is_stack_safe() {
    const CHAIN_LENGTH: usize = 20_000;

    let mut definitions = BTreeMap::new();
    let mut values = BTreeMap::new();
    for index in 0..CHAIN_LENGTH {
        let id = format!("field-{index:05}");
        definitions.insert(FieldId::from(id.as_str()), number_field(&id));
        let value = if index + 1 == CHAIN_LENGTH {
            formula(divide(numeric(1.0), numeric(0.0)))
        } else {
            formula(reference(&format!("field-{:05}", index + 1)))
        };
        values.insert(FieldId::from(id), value);
    }
    let document = Document {
        id: DocumentId::from("long-failed-chain"),
        title: "Long failed dependency chain".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("schema-stable"),
            Schema {
                id: SchemaId::from("schema-stable"),
                key: SchemaKey::from("schema"),
                fields: definitions,
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("entity-stable"),
            Entity {
                id: EntityId::from("entity-stable"),
                key: "entity".into(),
                schema: SchemaId::from("schema-stable"),
                fields: values,
            },
        )]),
    };

    let failures = failed(calculate_complete(&document));
    assert_eq!(failures.failures().len(), CHAIN_LENGTH);
    assert_eq!(
        failures.failures().get(&node("field-00000")),
        Some(&CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([node("field-00001")]),
        })
    );
    assert!(matches!(
        calculate(&document),
        Err(tachiko_formula_engine::CalculationError::DivisionByZero { formula })
            if formula == node("field-19999")
    ));
}

#[test]
fn retained_state_matches_the_full_oracle_across_dependent_multi_revision_mutations() {
    let mut current = document(vec![
        (
            "a",
            FieldType::Number,
            Value::Number(Number::new(2.0).unwrap()),
        ),
        ("b", FieldType::Number, formula(reference("a"))),
        ("c", FieldType::Number, formula(reference("b"))),
        (
            "independent",
            FieldType::Number,
            Value::Number(Number::new(7.0).unwrap()),
        ),
        (
            "independent-formula",
            FieldType::Number,
            formula(reference("independent")),
        ),
    ]);
    let (mut retained, initial_work) = RetainedCalculationState::rebuild(&current);
    assert_eq!(retained.outcome(), calculate_complete(&current));
    assert_eq!(initial_work.full_rebuilds, 1);

    let revisions = [
        (
            "normalized-zero",
            node("a"),
            Value::Number(Number::new(-0.0).unwrap()),
            None,
        ),
        ("introduce-cycle", node("b"), formula(reference("c")), None),
        ("break-cycle", node("b"), formula(reference("a")), None),
        (
            "delete-target",
            node("a"),
            Value::Number(Number::new(0.0).unwrap()),
            Some(false),
        ),
        (
            "restore-target",
            node("a"),
            Value::Number(Number::new(f64::from_bits(1)).unwrap()),
            Some(true),
        ),
        (
            "output-equal-replacement",
            node("b"),
            formula(Expression::Multiply {
                left: Box::new(reference("a")),
                right: Box::new(numeric(0.0)),
            }),
            None,
        ),
    ];

    for (revision, root, replacement, presence) in revisions {
        let fields = &mut current.entities.get_mut("entity-stable").unwrap().fields;
        match presence {
            Some(false) => {
                fields.remove(&root.field);
            }
            Some(true) | None => {
                fields.insert(root.field.clone(), replacement);
            }
        }
        let transition = retained.update(&current, &BTreeSet::from([root]));
        assert_eq!(
            retained.outcome(),
            calculate_complete(&current),
            "revision {revision}"
        );
        assert_eq!(
            transition.work.incremental_updates, 1,
            "revision {revision}"
        );
        assert!(transition.work.nodes_reused >= 2, "revision {revision}");
    }
}
