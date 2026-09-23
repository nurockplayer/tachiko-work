use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    DiagnosticCode, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef,
    FieldType, Number, Schema, Value, diagnostic_codes, patch_lifecycle::DocumentScopeId,
    resident_session::ResidentWorkspaceSession,
};

fn number(value: f64) -> Number {
    Number::new(value).expect("finite worker fixture")
}

fn formula_constraint_document(with_failed_formula: bool) -> Document {
    let mut document = Document::empty("document", "Constraint resident parity");
    let mut fields = BTreeMap::from([
        (
            "formula".into(),
            FieldDefinition {
                id: "formula".into(),
                key: "formula".into(),
                field_type: FieldType::Number,
                required: true,
                constraint: FieldConstraint::NumberInclusiveRange {
                    min: number(0.0),
                    max: number(10.0),
                },
            },
        ),
        (
            "stored".into(),
            FieldDefinition {
                id: "stored".into(),
                key: "stored".into(),
                field_type: FieldType::Number,
                required: true,
                constraint: FieldConstraint::NumberInclusiveRange {
                    min: number(0.0),
                    max: number(10.0),
                },
            },
        ),
    ]);
    let mut entity_fields = BTreeMap::from([
        (
            "formula".into(),
            Value::Formula(Expression::Number(number(11.0))),
        ),
        ("stored".into(), Value::Number(number(11.0))),
    ]);
    if with_failed_formula {
        fields.insert(
            "broken".into(),
            FieldDefinition {
                id: "broken".into(),
                key: "broken".into(),
                field_type: FieldType::Number,
                required: true,
                constraint: FieldConstraint::None,
            },
        );
        entity_fields.insert(
            "broken".into(),
            Value::Formula(Expression::Reference(FieldRef::new("entity", "broken"))),
        );
    }
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields,
        },
    );
    document.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "item".into(),
            schema: "schema".into(),
            fields: entity_fields,
        },
    );
    document
}

fn mismatch_paths(report: &tachiko_workspace_engine::ValidationReport) -> Vec<String> {
    report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::FIELD_VALUE_CONSTRAINT_MISMATCH)
        .map(|diagnostic| diagnostic.path.as_str().to_owned())
        .collect()
}

#[test]
fn resident_complete_formula_validation_matches_authoritative_direct_and_formula_results() {
    let document = formula_constraint_document(false);
    let authoritative = tachiko_workspace_engine::validation_report(&document);
    let resident = ResidentWorkspaceSession::new(DocumentScopeId::from("occurrence"), document);

    assert_eq!(resident.validation_report().value(), &authoritative);
    assert_eq!(
        mismatch_paths(resident.validation_report().value()),
        vec![
            "entities.entity.fields.formula".to_owned(),
            "entities.entity.fields.stored".to_owned(),
        ]
    );
}

#[test]
fn resident_failed_calculation_suppresses_formula_constraints_but_preserves_direct_violations() {
    let document = formula_constraint_document(true);
    let authoritative = tachiko_workspace_engine::validation_report(&document);
    let resident = ResidentWorkspaceSession::new(DocumentScopeId::from("occurrence"), document);

    assert_eq!(resident.validation_report().value(), &authoritative);
    assert!(
        authoritative
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
    );
    assert_eq!(
        mismatch_paths(resident.validation_report().value()),
        vec!["entities.entity.fields.stored".to_owned()]
    );
}
