use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use tachiko_formula_engine::{
    CalculationError, CalculationOutcome, FormulaFailure, calculate, calculate_full,
};
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

fn cycle_failure(members: BTreeSet<FieldRef>) -> FormulaFailure {
    FormulaFailure::Cycle {
        members: Arc::new(members),
    }
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
        assert_eq!(failures.get(member), Some(&cycle_failure(cycle.clone())));
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
    assert_eq!(
        failures.keys().cloned().collect::<Vec<_>>(),
        vec![
            field("binding"),
            field("cycle-a"),
            field("cycle-b"),
            field("cycle-c"),
            field("depends-earlier"),
            field("depends-zero"),
            field("structural"),
            field("zero"),
        ]
    );
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

#[test]
fn standalone_numeric_values_obey_schema_authority_in_full_and_compatibility_outcomes() {
    let mut non_numeric = document();
    insert(
        &mut non_numeric,
        "value",
        FieldType::Text,
        Value::Number(number(1.0)),
    );

    let mut undeclared = document();
    undeclared
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert(FieldId::from("value"), Value::Number(number(1.0)));

    let mut missing_schema = document();
    insert(
        &mut missing_schema,
        "value",
        FieldType::Number,
        Value::Number(number(1.0)),
    );
    missing_schema.entities.get_mut("entity").unwrap().schema = "missing-schema".into();

    for (document, expected_failure, expected_compatibility) in [
        (
            non_numeric,
            FormulaFailure::InvalidReferences {
                missing: BTreeSet::new(),
                non_numeric: BTreeSet::from([field("value")]),
            },
            CalculationError::NonNumericReference {
                reference: field("value"),
            },
        ),
        (
            undeclared,
            FormulaFailure::InvalidReferences {
                missing: BTreeSet::from([field("value")]),
                non_numeric: BTreeSet::new(),
            },
            CalculationError::MissingReference {
                reference: field("value"),
            },
        ),
        (
            missing_schema,
            FormulaFailure::InvalidReferences {
                missing: BTreeSet::from([field("value")]),
                non_numeric: BTreeSet::new(),
            },
            CalculationError::MissingReference {
                reference: field("value"),
            },
        ),
    ] {
        let report = failed(&document);
        assert_eq!(report.failure(&field("value")), Some(&expected_failure));
        assert!(report.dependencies().is_empty());
        assert_eq!(calculate(&document), Err(expected_compatibility));
    }
}

#[test]
fn compatibility_projection_keeps_earlier_legacy_failure_before_later_invalid_reference() {
    let mut local_first = document();
    formula(
        &mut local_first,
        "formula",
        Expression::Add {
            left: Box::new(Expression::Divide {
                left: Box::new(literal(1.0)),
                right: Box::new(literal(0.0)),
            }),
            right: Box::new(reference("missing")),
        },
    );
    assert!(matches!(
        failed(&local_first).failure(&field("formula")),
        Some(FormulaFailure::InvalidReferences { .. })
    ));
    assert_eq!(
        calculate(&local_first),
        Err(CalculationError::DivisionByZero {
            formula: field("formula"),
        })
    );

    let mut dependency_first = document();
    formula(
        &mut dependency_first,
        "a-formula",
        Expression::Add {
            left: Box::new(reference("z-failed")),
            right: Box::new(reference("missing")),
        },
    );
    formula(
        &mut dependency_first,
        "z-failed",
        Expression::Divide {
            left: Box::new(literal(1.0)),
            right: Box::new(literal(0.0)),
        },
    );
    assert!(matches!(
        failed(&dependency_first).failure(&field("a-formula")),
        Some(FormulaFailure::InvalidReferences { .. })
    ));
    assert_eq!(
        calculate(&dependency_first),
        Err(CalculationError::DivisionByZero {
            formula: field("z-failed"),
        })
    );
}

fn disjoint_cycle_document(reverse_insertion: bool) -> Document {
    let mut document = document();
    let mut formulas = vec![
        ("a-cycle-1", reference("a-cycle-2")),
        ("a-cycle-2", reference("a-cycle-1")),
        ("b-cycle-1", reference("b-cycle-2")),
        ("b-cycle-2", reference("b-cycle-3")),
        ("b-cycle-3", reference("b-cycle-1")),
        ("depends-a", reference("a-cycle-2")),
        ("depends-b", reference("b-cycle-1")),
        (
            "depends-all",
            Expression::Add {
                left: Box::new(reference("a-cycle-1")),
                right: Box::new(Expression::Add {
                    left: Box::new(reference("b-cycle-2")),
                    right: Box::new(reference("zero")),
                }),
            },
        ),
        (
            "independent",
            Expression::Add {
                left: Box::new(literal(2.0)),
                right: Box::new(literal(3.0)),
            },
        ),
        (
            "zero",
            Expression::Divide {
                left: Box::new(literal(1.0)),
                right: Box::new(literal(0.0)),
            },
        ),
    ];
    if reverse_insertion {
        formulas.reverse();
    }
    for (name, expression) in formulas {
        formula(&mut document, name, expression);
    }
    document
}

#[test]
fn disjoint_sccs_and_direct_failures_are_complete_and_repeatable() {
    let forward = failed(&disjoint_cycle_document(false));
    let reversed = failed(&disjoint_cycle_document(true));
    assert_eq!(forward, reversed);
    assert_eq!(forward, failed(&disjoint_cycle_document(false)));

    let a_cycle = BTreeSet::from([field("a-cycle-1"), field("a-cycle-2")]);
    let b_cycle = BTreeSet::from([field("b-cycle-1"), field("b-cycle-2"), field("b-cycle-3")]);
    for member in &a_cycle {
        assert_eq!(
            forward.failure(member),
            Some(&cycle_failure(a_cycle.clone()))
        );
    }
    for member in &b_cycle {
        assert_eq!(
            forward.failure(member),
            Some(&cycle_failure(b_cycle.clone()))
        );
    }
    assert_eq!(
        forward.failure(&field("depends-a")),
        Some(&FormulaFailure::FailedDependency {
            dependencies: BTreeSet::from([field("a-cycle-2")]),
        })
    );
    assert_eq!(
        forward.failure(&field("depends-b")),
        Some(&FormulaFailure::FailedDependency {
            dependencies: BTreeSet::from([field("b-cycle-1")]),
        })
    );
    assert_eq!(
        forward.failure(&field("depends-all")),
        Some(&FormulaFailure::FailedDependency {
            dependencies: BTreeSet::from([field("a-cycle-1"), field("b-cycle-2"), field("zero"),]),
        })
    );
    assert_eq!(
        forward.failure(&field("zero")),
        Some(&FormulaFailure::DivisionByZero)
    );
    assert!(!forward.failures().contains_key(&field("independent")));

    let expected_dependencies = disjoint_cycle_document(false)
        .entities
        .remove("entity")
        .unwrap()
        .fields
        .into_iter()
        .filter_map(|(field_id, value)| match value {
            Value::Formula(expression) => Some((
                FieldRef::new("entity", field_id),
                tachiko_formula_engine::extract_dependencies(&expression),
            )),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(forward.dependencies(), &expected_dependencies);
}

#[test]
fn generated_cyclic_families_repeat_exact_failures_dependencies_and_memberships() {
    for seed in 0_u8..16 {
        let mut document = document();
        let component_count = 2 + (usize::from(seed) % 4);
        let mut definitions = Vec::new();
        let mut expected_cycles = Vec::new();
        let mut expected_dependents = BTreeMap::new();
        let mut expected_dependencies = BTreeMap::new();

        for component in 0..component_count {
            let member_count = 2 + ((usize::from(seed) + component * 3) % 5);
            let members = (0..member_count)
                .map(|member| {
                    field(&format!(
                        "seed-{seed:02}-cycle-{component:02}-member-{member:02}"
                    ))
                })
                .collect::<BTreeSet<_>>();
            let ordered = members.iter().cloned().collect::<Vec<_>>();
            for (index, member) in ordered.iter().enumerate() {
                let dependency = ordered[(index + 1) % ordered.len()].clone();
                definitions.push((
                    member.field.to_string(),
                    Expression::Reference(dependency.clone()),
                ));
                expected_dependencies.insert(member.clone(), BTreeSet::from([dependency]));
            }

            let dependent = field(&format!("seed-{seed:02}-dependent-{component:02}"));
            let dependency = ordered[(usize::from(seed) + component) % ordered.len()].clone();
            definitions.push((
                dependent.field.to_string(),
                Expression::Reference(dependency.clone()),
            ));
            expected_dependencies.insert(dependent.clone(), BTreeSet::from([dependency.clone()]));
            expected_dependents.insert(dependent, dependency);
            expected_cycles.push(members);
        }

        let zero = field(&format!("seed-{seed:02}-zero"));
        definitions.push((
            zero.field.to_string(),
            Expression::Divide {
                left: Box::new(literal(1.0)),
                right: Box::new(literal(0.0)),
            },
        ));
        expected_dependencies.insert(zero.clone(), BTreeSet::new());
        let success = field(&format!("seed-{seed:02}-success"));
        definitions.push((
            success.field.to_string(),
            Expression::Add {
                left: Box::new(literal(f64::from(seed))),
                right: Box::new(literal(1.0)),
            },
        ));
        expected_dependencies.insert(success.clone(), BTreeSet::new());

        if seed % 2 == 1 {
            definitions.reverse();
        } else if definitions.len() > 3 {
            let rotation = usize::from(seed) % definitions.len();
            definitions.rotate_left(rotation);
        }
        for (name, expression) in definitions {
            formula(&mut document, &name, expression);
        }

        let first = failed(&document);
        assert_eq!(first, failed(&document), "seed {seed} must repeat exactly");
        assert_eq!(first.dependencies(), &expected_dependencies);
        for members in &expected_cycles {
            for member in members {
                assert_eq!(
                    first.failure(member),
                    Some(&cycle_failure(members.clone())),
                    "seed {seed} member {member}"
                );
            }
        }
        for (dependent, dependency) in expected_dependents {
            assert_eq!(
                first.failure(&dependent),
                Some(&FormulaFailure::FailedDependency {
                    dependencies: BTreeSet::from([dependency]),
                }),
                "seed {seed} dependent {dependent}"
            );
        }
        assert_eq!(first.failure(&zero), Some(&FormulaFailure::DivisionByZero));
        assert!(!first.failures().contains_key(&success));
        assert_eq!(
            first.failures().len(),
            expected_cycles.iter().map(BTreeSet::len).sum::<usize>() + component_count + 1
        );
    }
}

#[test]
fn large_scc_membership_is_shared_without_quadratic_cloning() {
    const MEMBER_COUNT: usize = 4_096;

    let mut document = document();
    for index in 0..MEMBER_COUNT {
        let current = format!("cycle-{index:04}");
        let next = format!("cycle-{:04}", (index + 1) % MEMBER_COUNT);
        formula(&mut document, &current, reference(&next));
    }

    let report = failed(&document);
    let mut shared_members = None;
    for failure in report.failures().values() {
        let FormulaFailure::Cycle { members } = failure else {
            panic!("large cycle must contain only cycle failures");
        };
        assert_eq!(members.len(), MEMBER_COUNT);
        if let Some(expected) = shared_members {
            assert!(Arc::ptr_eq(expected, members));
        } else {
            shared_members = Some(members);
        }
    }
    assert_eq!(report.failures().len(), MEMBER_COUNT);
}
