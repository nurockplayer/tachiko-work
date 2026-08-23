mod common;

use common::game_balance_document;
use tachiko_workspace_engine::{
    FieldAddress, FieldRef, Number, RuntimeValue, SemanticChange, Value, WorkspaceError,
    WorkspaceMergeOutcome, analyze_formula, calculate_fields, compare_documents, diagnostic_codes,
    merge_documents, runtime_export, set_scalar, validate, validate_field_value_suggestion,
};

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

#[test]
fn validation_and_calculation_are_shared_application_queries() {
    let document = game_balance_document("game", "Game");

    validate(&document).expect("starter should be valid and calculable");
    let values = calculate_fields(&document).expect("starter should calculate");
    let addresses = values
        .iter()
        .map(|field| field.address.to_string())
        .collect::<Vec<_>>();

    assert!(addresses.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(values.iter().any(|field| {
        field.address == FieldAddress::new("iron_sword", "dps")
            && field.value == Number::new(40.0).unwrap()
    }));

    let broken = set_scalar(
        &document,
        &FieldAddress::new("iron_sword", "attack_interval"),
        "0",
    )
    .expect_err("the existing edit operation proves this candidate cannot calculate");
    let WorkspaceError::InvalidDocument { report, .. } = broken else {
        panic!("calculation failure must be represented by the shared semantic report");
    };
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_DIVISION_BY_ZERO)
    );
}

#[test]
fn comparison_and_merge_are_workspace_owned_orchestration() {
    let base = game_balance_document("game", "Game");
    let ours = set_scalar(&base, &FieldAddress::new("iron_sword", "damage"), "45")
        .unwrap()
        .document;
    let theirs = set_scalar(
        &base,
        &FieldAddress::new("iron_sword", "attack_interval"),
        "0.8",
    )
    .unwrap()
    .document;

    let impact = compare_documents(&base, &ours).expect("documents should compare");
    assert!(impact.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, .. }
            if field == &FieldRef::new("iron_sword", "damage")
    )));

    let WorkspaceMergeOutcome::Merged(preview) =
        merge_documents(&base, &ours, &theirs).expect("independent changes should merge")
    else {
        panic!("independent changes must not conflict");
    };
    let merged_values = calculate_fields(&preview.document).unwrap();
    assert!(merged_values.iter().any(|field| {
        field.address == FieldAddress::new("iron_sword", "dps")
            && field.value == Number::new(56.25).unwrap()
    }));
    assert!(preview.diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FormulaImpact { field, .. }
            if field == &FieldRef::new("iron_sword", "dps")
    )));

    let conflicting = set_scalar(&base, &FieldAddress::new("iron_sword", "damage"), "50")
        .unwrap()
        .document;
    let WorkspaceMergeOutcome::Conflicted(conflicts) =
        merge_documents(&base, &ours, &conflicting).expect("conflicts are a typed outcome")
    else {
        panic!("divergent changes must conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert!(conflicts[0].path.ends_with(".fields.damage"));
}

#[test]
fn formula_analysis_and_typed_suggestions_share_engine_policy() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let dps = FieldRef::new("iron_sword", "dps");

    let analysis = analyze_formula(&document, &dps).expect("DPS should be analyzable");
    assert_eq!(analysis.field, dps);
    assert_eq!(analysis.value, Number::new(40.0).unwrap());
    assert_eq!(
        analysis.dependencies,
        [
            FieldRef::new("iron_sword", "attack_interval"),
            FieldRef::new("iron_sword", "damage"),
        ]
    );

    let proposal = validate_field_value_suggestion(
        &document,
        FieldRef::new("iron_sword", "damage"),
        number(45.0),
    )
    .expect("typed input should be proposal-ready");
    assert_eq!(proposal.field, FieldRef::new("iron_sword", "damage"));
    assert_eq!(proposal.value, number(45.0));
    assert_eq!(document, original, "proposal validation must remain inert");

    let error = validate_field_value_suggestion(
        &document,
        FieldRef::new("iron_sword", "damage"),
        Value::Text("high".to_owned()),
    )
    .expect_err("schema mismatches must fail inside the shared engine");
    assert!(matches!(error, WorkspaceError::TypeMismatch { .. }));
}

#[test]
fn runtime_export_is_a_portable_engine_projection() {
    let document = game_balance_document("game", "Game");

    let exported = runtime_export(&document).expect("starter should export");

    assert_eq!(exported.format_version, 2);
    assert_eq!(exported.document_id, "game");
    assert_eq!(exported.title, "Game");
    assert_eq!(exported.entities["iron_sword"].schema, "weapons");
    assert_eq!(
        exported.entities["iron_sword"].fields["dps"],
        RuntimeValue::Number(Number::new(40.0).unwrap())
    );
    assert_eq!(
        exported.entities["alric"].fields["weapon"],
        RuntimeValue::Reference {
            reference: "iron_sword".to_owned()
        }
    );
}
