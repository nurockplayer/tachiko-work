use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    CanonicalAuthoringProjectionError, DiagnosticCode, DiagnosticProvider, DiagnosticSeverity,
    Document, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, Number, Schema, SchemaId, SchemaKey, SemanticSubject, Value, WorkspaceError,
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
        ..
    } = validate(&document).unwrap_err()
    else {
        panic!("validate must use the semantic report");
    };
    let WorkspaceError::InvalidDocument {
        report: finalization_report,
        ..
    } = rename_entity(&document, "entity", "renamed").unwrap_err()
    else {
        panic!("finalization must use the semantic report");
    };

    assert_eq!(oracle, validate_report);
    assert_eq!(
        oracle.stable_observations(),
        finalization_report.stable_observations()
    );
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

    let WorkspaceError::InvalidDocument { report, .. } =
        merge_documents(&base, &ours, &theirs).unwrap_err()
    else {
        panic!("merged semantic failure must use ValidationReport");
    };
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
