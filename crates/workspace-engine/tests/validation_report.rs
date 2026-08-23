use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    CanonicalAuthoringProjectionError, DiagnosticCode, DiagnosticFact, DiagnosticProvider,
    DiagnosticSeverity, Document, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, SemanticSubject,
    StableDiagnosticObservation, ValidationRole, Value, WorkspaceError, compare_documents,
    diagnostic_codes, merge_documents, rename_entity, rename_schema, validate, validation_report,
};

fn document() -> Document {
    Document {
        id: "document".into(),
        title: "Validation report".to_owned(),
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

fn define(document: &mut Document, field: &str, field_type: FieldType, required: bool) {
    document.schemas.get_mut("schema").unwrap().fields.insert(
        FieldId::from(field),
        FieldDefinition {
            id: FieldId::from(field),
            key: FieldKey::from(field),
            field_type,
            required,
        },
    );
}

fn set(document: &mut Document, entity: &str, field: &str, value: Value) {
    document
        .entities
        .get_mut(entity)
        .unwrap()
        .fields
        .insert(FieldId::from(field), value);
}

fn formula(document: &mut Document, field: &str, expression: Expression) {
    define(document, field, FieldType::Number, false);
    set(document, "entity", field, Value::Formula(expression));
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn number(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn cycle_document() -> Document {
    let mut document = document();
    formula(&mut document, "cycle-a", reference("entity", "cycle-b"));
    formula(&mut document, "cycle-b", reference("entity", "cycle-a"));
    formula(
        &mut document,
        "dependent",
        Expression::Add {
            left: Box::new(reference("entity", "cycle-a")),
            right: Box::new(reference("entity", "cycle-b")),
        },
    );
    document
}

fn merge_document() -> Document {
    let mut document = document();
    define(&mut document, "damage", FieldType::Number, false);
    define(&mut document, "interval", FieldType::Number, false);
    define(&mut document, "bonus", FieldType::Number, false);
    define(&mut document, "dps", FieldType::Number, false);
    set(
        &mut document,
        "entity",
        "damage",
        Value::Number(Number::new(10.0).unwrap()),
    );
    set(
        &mut document,
        "entity",
        "interval",
        Value::Number(Number::new(2.0).unwrap()),
    );
    set(
        &mut document,
        "entity",
        "bonus",
        Value::Number(Number::new(3.0).unwrap()),
    );
    set(
        &mut document,
        "entity",
        "dps",
        Value::Formula(Expression::Divide {
            left: Box::new(reference("entity", "damage")),
            right: Box::new(reference("entity", "damage")),
        }),
    );
    document
}

fn assert_swapped_id_mismatches(
    document: &Document,
    expected_subjects: &[SemanticSubject],
    first_id: &str,
    second_id: &str,
) {
    let observations = validation_report(document)
        .stable_observations()
        .into_iter()
        .filter(|observation| observation.code == DiagnosticCode::KEY_MISMATCH)
        .collect::<Vec<_>>();

    assert_eq!(observations.len(), 2);
    assert!(
        observations
            .iter()
            .all(|observation| observation.subjects == expected_subjects)
    );
    assert!(
        observations
            .iter()
            .all(|observation| observation.severity == DiagnosticSeverity::Error)
    );
    assert!(
        observations
            .iter()
            .all(|observation| observation.related_subjects.is_empty())
    );
    assert!(observations.iter().all(|observation| {
        observation.provider == DiagnosticProvider::new("tachiko.semantic-core")
    }));
    assert_eq!(
        observations
            .into_iter()
            .map(|observation| observation.facts)
            .collect::<Vec<_>>(),
        vec![
            vec![
                DiagnosticFact::new("declared_id", first_id),
                DiagnosticFact::new("store_id", second_id),
            ],
            vec![
                DiagnosticFact::new("declared_id", second_id),
                DiagnosticFact::new("store_id", first_id),
            ],
        ]
    );
}

#[test]
fn swapped_schema_ids_preserve_both_directional_mismatches() {
    let mut document = document();
    document.entities.clear();
    document.schemas = BTreeMap::from([
        (
            SchemaId::from("schema-a"),
            Schema {
                id: "schema-b".into(),
                key: "schema-a".into(),
                fields: BTreeMap::new(),
            },
        ),
        (
            SchemaId::from("schema-b"),
            Schema {
                id: "schema-a".into(),
                key: "schema-b".into(),
                fields: BTreeMap::new(),
            },
        ),
    ]);

    assert_swapped_id_mismatches(
        &document,
        &[
            SemanticSubject::Schema(SchemaId::from("schema-a")),
            SemanticSubject::Schema(SchemaId::from("schema-b")),
        ],
        "schema-a",
        "schema-b",
    );
}

#[test]
fn swapped_entity_ids_preserve_both_directional_mismatches() {
    let mut document = document();
    document.entities = BTreeMap::from([
        (
            EntityId::from("entity-a"),
            Entity {
                id: "entity-b".into(),
                key: "entity-a".into(),
                schema: "schema".into(),
                fields: BTreeMap::new(),
            },
        ),
        (
            EntityId::from("entity-b"),
            Entity {
                id: "entity-a".into(),
                key: "entity-b".into(),
                schema: "schema".into(),
                fields: BTreeMap::new(),
            },
        ),
    ]);

    assert_swapped_id_mismatches(
        &document,
        &[
            SemanticSubject::Entity(EntityId::from("entity-a")),
            SemanticSubject::Entity(EntityId::from("entity-b")),
        ],
        "entity-a",
        "entity-b",
    );
}

#[test]
fn swapped_schema_field_ids_preserve_both_directional_mismatches() {
    let mut document = document();
    document.entities.clear();
    document.schemas.get_mut("schema").unwrap().fields = BTreeMap::from([
        (
            FieldId::from("field-a"),
            FieldDefinition {
                id: "field-b".into(),
                key: "field-a".into(),
                field_type: FieldType::Number,
                required: false,
            },
        ),
        (
            FieldId::from("field-b"),
            FieldDefinition {
                id: "field-a".into(),
                key: "field-b".into(),
                field_type: FieldType::Number,
                required: false,
            },
        ),
    ]);

    assert_swapped_id_mismatches(
        &document,
        &[
            SemanticSubject::SchemaField {
                schema: "schema".into(),
                field: "field-a".into(),
            },
            SemanticSubject::SchemaField {
                schema: "schema".into(),
                field: "field-b".into(),
            },
        ],
        "field-a",
        "field-b",
    );
}

#[test]
fn independent_findings_accumulate_while_missing_schema_suppresses_cascades() {
    let mut document = document();
    define(&mut document, "required", FieldType::Number, true);
    formula(
        &mut document,
        "binding",
        reference("entity", "missing-target"),
    );
    formula(
        &mut document,
        "zero",
        Expression::Divide {
            left: Box::new(number(1.0)),
            right: Box::new(number(0.0)),
        },
    );
    document.entities.insert(
        EntityId::from("orphan"),
        Entity {
            id: "orphan".into(),
            key: "orphan".into(),
            schema: "missing-schema".into(),
            fields: BTreeMap::from([(
                FieldId::from("unknown"),
                Value::Text("not speculatively typed".to_owned()),
            )]),
        },
    );

    let report = validation_report(&document);
    assert_eq!(
        report.stable_observations(),
        vec![
            StableDiagnosticObservation {
                code: DiagnosticCode::MISSING_REQUIRED_FIELD,
                severity: DiagnosticSeverity::Error,
                subjects: vec![SemanticSubject::EntityField(FieldRef::new(
                    "entity", "required",
                ))],
                related_subjects: vec![],
                facts: vec![],
                provider: DiagnosticProvider::new("tachiko.semantic-core"),
            },
            StableDiagnosticObservation {
                code: DiagnosticCode::MISSING_SCHEMA,
                severity: DiagnosticSeverity::Error,
                subjects: vec![SemanticSubject::Entity(EntityId::from("orphan"))],
                related_subjects: vec![SemanticSubject::Schema(SchemaId::from("missing-schema",))],
                facts: vec![],
                provider: DiagnosticProvider::new("tachiko.semantic-core"),
            },
            StableDiagnosticObservation {
                code: diagnostic_codes::FORMULA_DIVISION_BY_ZERO,
                severity: DiagnosticSeverity::Error,
                subjects: vec![SemanticSubject::EntityField(FieldRef::new(
                    "entity", "zero",
                ))],
                related_subjects: vec![],
                facts: vec![],
                provider: DiagnosticProvider::new("tachiko.formula-engine"),
            },
            StableDiagnosticObservation {
                code: diagnostic_codes::FORMULA_INVALID_REFERENCES,
                severity: DiagnosticSeverity::Error,
                subjects: vec![SemanticSubject::EntityField(FieldRef::new(
                    "entity", "binding",
                ))],
                related_subjects: vec![SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "missing-target",
                ))],
                facts: vec![DiagnosticFact::new(
                    "missing_target",
                    "6:entity14:missing-target",
                )],
                provider: DiagnosticProvider::new("tachiko.formula-engine"),
            },
        ]
    );
    let codes = report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    assert!(codes.contains(&DiagnosticCode::MISSING_REQUIRED_FIELD));
    assert!(codes.contains(&DiagnosticCode::MISSING_SCHEMA));
    assert!(codes.contains(&diagnostic_codes::FORMULA_INVALID_REFERENCES));
    assert!(codes.contains(&diagnostic_codes::FORMULA_DIVISION_BY_ZERO));
    assert!(!report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .subjects
            .contains(&SemanticSubject::EntityField(FieldRef::new(
                "orphan", "unknown",
            )))
    }));

    let mut sorted = report.diagnostics().to_vec();
    sorted.sort();
    assert_eq!(report.diagnostics(), sorted);
}

#[test]
fn stable_observations_survive_human_key_renames() {
    let mut before = document();
    formula(
        &mut before,
        "formula",
        reference("entity", "missing-target"),
    );
    before.entities.insert(
        EntityId::from("second"),
        Entity {
            id: "second".into(),
            key: "entity".into(),
            schema: "schema".into(),
            fields: BTreeMap::new(),
        },
    );

    let mut after = before.clone();
    after.schemas.get_mut("schema").unwrap().key = "renamed-schema".into();
    after
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("formula")
        .unwrap()
        .key = "renamed-formula".into();
    after.entities.get_mut("entity").unwrap().key = "renamed-entity".into();
    after.entities.get_mut("second").unwrap().key = "renamed-entity".into();

    let before = validation_report(&before);
    let after = validation_report(&after);

    assert_eq!(before.stable_observations(), after.stable_observations());
    assert_ne!(
        before
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.path.clone())
            .collect::<Vec<_>>(),
        after
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn core_invalid_formula_prerequisites_suppress_cascades_but_not_independent_failures() {
    let mut document = document();
    define(&mut document, "required-input", FieldType::Number, true);
    define(&mut document, "typed-input", FieldType::Number, false);
    set(
        &mut document,
        "entity",
        "typed-input",
        Value::Text("not numeric".to_owned()),
    );
    formula(
        &mut document,
        "missing-dependent",
        reference("entity", "required-input"),
    );
    formula(
        &mut document,
        "typed-dependent",
        reference("entity", "typed-input"),
    );
    formula(
        &mut document,
        "blocked-owner",
        Expression::Divide {
            left: Box::new(number(1.0)),
            right: Box::new(number(0.0)),
        },
    );
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("blocked-owner")
        .unwrap()
        .id = "different-stable-id".into();
    formula(
        &mut document,
        "independent-zero",
        Expression::Divide {
            left: Box::new(number(1.0)),
            right: Box::new(number(0.0)),
        },
    );

    let report = validation_report(&document);
    let codes = report
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&DiagnosticCode::MISSING_REQUIRED_FIELD));
    assert!(codes.contains(&DiagnosticCode::TYPE_MISMATCH));
    assert!(codes.contains(&DiagnosticCode::KEY_MISMATCH));

    let division_failures = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_DIVISION_BY_ZERO)
        .collect::<Vec<_>>();
    assert_eq!(division_failures.len(), 1);
    assert_eq!(
        division_failures[0].subjects,
        [SemanticSubject::EntityField(FieldRef::new(
            "entity",
            "independent-zero",
        ))]
    );
}

#[test]
fn cascade_suppression_is_specific_to_the_authoritative_formula_failure() {
    let mut document = document();
    define(&mut document, "required-input", FieldType::Number, true);
    define(&mut document, "text-target", FieldType::Text, false);
    set(
        &mut document,
        "entity",
        "text-target",
        Value::Number(Number::new(1.0).unwrap()),
    );
    formula(
        &mut document,
        "cascade-only",
        reference("entity", "required-input"),
    );
    formula(
        &mut document,
        "static-type-error",
        reference("entity", "text-target"),
    );
    formula(
        &mut document,
        "local-first",
        Expression::Add {
            left: Box::new(Expression::Divide {
                left: Box::new(number(1.0)),
                right: Box::new(number(0.0)),
            }),
            right: Box::new(number(1.0)),
        },
    );
    formula(
        &mut document,
        "cycle-a",
        Expression::Add {
            left: Box::new(reference("entity", "cycle-b")),
            right: Box::new(number(1.0)),
        },
    );
    formula(&mut document, "cycle-b", reference("entity", "cycle-a"));

    let report = validation_report(&document);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::MISSING_REQUIRED_FIELD
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "required-input",
                ))]
    }));
    assert!(!report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "cascade-only",
                ))]
    }));
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::TYPE_MISMATCH
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "text-target",
                ))]
    }));
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "static-type-error",
                ))]
            && diagnostic.related_subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "text-target",
                ))]
    }));
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::FORMULA_DIVISION_BY_ZERO
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "local-first",
                ))]
    }));
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::FORMULA_CYCLE
            && diagnostic.subjects
                == [
                    SemanticSubject::EntityField(FieldRef::new("entity", "cycle-a")),
                    SemanticSubject::EntityField(FieldRef::new("entity", "cycle-b")),
                ]
    }));
}

#[test]
fn cascade_suppression_filters_each_binding_fact_by_its_actual_prerequisite() {
    let mut document = document();
    define(
        &mut document,
        "reference-target",
        FieldType::Reference {
            schema: "missing-target-schema".into(),
        },
        false,
    );
    formula(
        &mut document,
        "mixed-binding",
        Expression::Add {
            left: Box::new(reference("orphan", "value")),
            right: Box::new(reference("ghost", "value")),
        },
    );
    formula(
        &mut document,
        "reference-type-error",
        reference("entity", "reference-target"),
    );
    document.entities.insert(
        EntityId::from("orphan"),
        Entity {
            id: "orphan".into(),
            key: "orphan".into(),
            schema: "missing-entity-schema".into(),
            fields: BTreeMap::new(),
        },
    );

    let report = validation_report(&document);
    let mixed = report
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "mixed-binding",
                    ))]
        })
        .expect("the independent missing target remains diagnosable");
    assert_eq!(
        mixed.related_subjects,
        [SemanticSubject::EntityField(FieldRef::new(
            "ghost", "value"
        ))]
    );
    assert_eq!(
        mixed.facts,
        [DiagnosticFact::new("missing_target", "5:ghost5:value")]
    );

    let non_numeric = report
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "reference-type-error",
                    ))]
        })
        .expect("the declared non-number kind remains diagnosable");
    assert_eq!(
        non_numeric.related_subjects,
        [SemanticSubject::EntityField(FieldRef::new(
            "entity",
            "reference-target",
        ))]
    );
    assert_eq!(
        non_numeric.facts,
        [DiagnosticFact::new(
            "non_numeric_target",
            "6:entity16:reference-target",
        )]
    );
}

#[test]
fn cascade_suppression_filters_each_direct_failed_dependency() {
    let mut document = document();
    formula(
        &mut document,
        "blocked-dependency",
        Expression::Divide {
            left: Box::new(number(1.0)),
            right: Box::new(number(0.0)),
        },
    );
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("blocked-dependency")
        .unwrap()
        .id = "different-stable-id".into();
    formula(
        &mut document,
        "independent-zero",
        Expression::Divide {
            left: Box::new(number(1.0)),
            right: Box::new(number(0.0)),
        },
    );
    formula(
        &mut document,
        "dependent",
        Expression::Add {
            left: Box::new(reference("entity", "blocked-dependency")),
            right: Box::new(reference("entity", "independent-zero")),
        },
    );

    let report = validation_report(&document);
    assert!(!report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == diagnostic_codes::FORMULA_DIVISION_BY_ZERO
            && diagnostic.subjects
                == [SemanticSubject::EntityField(FieldRef::new(
                    "entity",
                    "blocked-dependency",
                ))]
    }));
    let dependent = report
        .diagnostics()
        .iter()
        .find(|diagnostic| {
            diagnostic.code == diagnostic_codes::FORMULA_FAILED_DEPENDENCY
                && diagnostic.subjects
                    == [SemanticSubject::EntityField(FieldRef::new(
                        "entity",
                        "dependent",
                    ))]
        })
        .expect("the independent direct failed dependency remains diagnosable");
    assert_eq!(
        dependent.related_subjects,
        [SemanticSubject::EntityField(FieldRef::new(
            "entity",
            "independent-zero",
        ))]
    );
}

#[test]
fn large_scc_projects_one_complete_workspace_diagnostic() {
    const MEMBER_COUNT: usize = 4_096;

    let mut document = document();
    for index in 0..MEMBER_COUNT {
        let current = format!("cycle-{index:04}");
        let next = format!("cycle-{:04}", (index + 1) % MEMBER_COUNT);
        formula(&mut document, &current, reference("entity", &next));
    }

    let report = validation_report(&document);
    let cycles = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
        .collect::<Vec<_>>();
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].subjects.len(), MEMBER_COUNT);
}

#[test]
fn cycle_and_failed_dependency_diagnostics_preserve_all_stable_subjects() {
    let report = validation_report(&cycle_document());
    let cycle_members = vec![
        SemanticSubject::EntityField(FieldRef::new("entity", "cycle-a")),
        SemanticSubject::EntityField(FieldRef::new("entity", "cycle-b")),
    ];
    let cycles = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
        .collect::<Vec<_>>();

    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].subjects, cycle_members);
    assert_eq!(cycles[0].severity, DiagnosticSeverity::Error);
    assert_eq!(
        cycles[0].provider,
        DiagnosticProvider::new("tachiko.formula-engine")
    );

    let dependency = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_FAILED_DEPENDENCY)
        .expect("failed dependency diagnostic");
    assert_eq!(
        dependency.subjects,
        vec![SemanticSubject::EntityField(FieldRef::new(
            "entity",
            "dependent"
        ))]
    );
    assert_eq!(dependency.related_subjects, cycles[0].subjects);
}

#[test]
fn validation_and_finalization_share_the_same_semantic_report() {
    let document = cycle_document();
    let oracle = validation_report(&document);

    let WorkspaceError::InvalidDocument {
        report: validate_report,
        role: validate_role,
        ..
    } = validate(&document).unwrap_err()
    else {
        panic!("validate must use the semantic report");
    };
    let WorkspaceError::InvalidDocument {
        report: finalization_report,
        role: finalization_role,
        ..
    } = rename_entity(&document, "entity", "renamed").unwrap_err()
    else {
        panic!("finalization must use the semantic report");
    };

    assert_eq!(validate_role, ValidationRole::Current);
    assert_eq!(finalization_role, ValidationRole::Candidate);
    assert_eq!(oracle, validate_report);
    assert_eq!(
        oracle.stable_observations(),
        finalization_report.stable_observations()
    );
}

#[test]
fn comparison_and_merge_validation_errors_preserve_operand_roles() {
    let valid = merge_document();
    let invalid = cycle_document();

    for (result, expected) in [
        (
            compare_documents(&invalid, &valid),
            ValidationRole::ComparisonBefore,
        ),
        (
            compare_documents(&valid, &invalid),
            ValidationRole::ComparisonAfter,
        ),
    ] {
        let WorkspaceError::InvalidDocument { role, report, .. } = result.unwrap_err() else {
            panic!("comparison input must return its validation role");
        };
        assert_eq!(role, expected);
        assert_eq!(report, validation_report(&invalid));
    }

    for (result, expected) in [
        (
            merge_documents(&invalid, &valid, &valid),
            ValidationRole::MergeBase,
        ),
        (
            merge_documents(&valid, &invalid, &valid),
            ValidationRole::MergeOurs,
        ),
        (
            merge_documents(&valid, &valid, &invalid),
            ValidationRole::MergeTheirs,
        ),
    ] {
        let WorkspaceError::InvalidDocument { role, report, .. } = result.unwrap_err() else {
            panic!("merge input must return its validation role");
        };
        assert_eq!(role, expected);
        assert_eq!(report, validation_report(&invalid));
    }
}

#[test]
fn authoring_projection_remains_an_explicit_operation_gate() {
    let mut document = document();
    define(&mut document, "input", FieldType::Number, false);
    define(&mut document, "formula", FieldType::Number, false);
    let long_key = "e".repeat(4_090);
    document.entities.insert(
        EntityId::from("target"),
        Entity {
            id: "target".into(),
            key: long_key.into(),
            schema: "schema".into(),
            fields: BTreeMap::from([(
                FieldId::from("input"),
                Value::Number(Number::new(2.0).unwrap()),
            )]),
        },
    );
    set(
        &mut document,
        "entity",
        "formula",
        Value::Formula(reference("target", "input")),
    );

    assert!(validation_report(&document).is_valid());
    assert!(validate(&document).is_ok());
    assert!(matches!(
        rename_schema(&document, "schema", "renamed-schema"),
        Err(WorkspaceError::FormulaProjection {
            source: CanonicalAuthoringProjectionError::Complexity(_),
            ..
        })
    ));
}

#[test]
fn merge_candidate_uses_the_shared_semantic_report() {
    let base = merge_document();
    let mut ours = base.clone();
    set(
        &mut ours,
        "entity",
        "dps",
        Value::Formula(reference("entity", "bonus")),
    );
    let mut theirs = base.clone();
    theirs
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .remove("bonus");
    theirs
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .remove("bonus");

    let WorkspaceError::InvalidDocument { role, report, .. } =
        merge_documents(&base, &ours, &theirs).unwrap_err()
    else {
        panic!("merged semantic failure must use ValidationReport");
    };
    assert_eq!(role, ValidationRole::MergeCandidate);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == diagnostic_codes::FORMULA_INVALID_REFERENCES })
    );
}

#[test]
fn merge_projection_preflight_is_a_workspace_operation_gate() {
    let base = merge_document();
    let mut ours = base.clone();
    ours.entities.get_mut("entity").unwrap().key = "a".repeat(2_032).into();
    let mut theirs = base.clone();
    theirs
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("damage")
        .unwrap()
        .key = "b".repeat(4_050).into();

    assert!(matches!(
        merge_documents(&base, &ours, &theirs),
        Err(WorkspaceError::FormulaProjection { .. })
    ));
}
