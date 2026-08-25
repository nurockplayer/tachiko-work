mod common;

use common::game_balance_document;
use tachiko_workspace_engine::{
    DiagnosticCode, EntityId, Expression, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType,
    Number, SchemaId, SemanticChange, WorkspaceError, analyze_changes, analyze_field,
    analyze_validation, inspect_document,
};

fn add_power_formula(document: &mut tachiko_workspace_engine::Document) {
    document.schemas.get_mut("weapons").unwrap().fields.insert(
        FieldId::from("power"),
        FieldDefinition {
            id: FieldId::from("power"),
            key: FieldKey::from("power"),
            field_type: FieldType::Number,
            required: true,
        },
    );
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(
            FieldId::from("power"),
            tachiko_workspace_engine::Value::Formula(Expression::Add {
                left: Box::new(Expression::Reference(FieldRef::new("iron_sword", "dps"))),
                right: Box::new(Expression::Number(Number::new(5.0).unwrap())),
            }),
        );
}

#[test]
fn document_inspection_preserves_source_state_and_sorted_structure() {
    let document = game_balance_document("game", "Game");

    let inspection = inspect_document(&document, "main@abc123");

    assert_eq!(inspection.source.document_id, document.id);
    assert_eq!(inspection.source.source_label, "main@abc123");
    assert_eq!(inspection.title, "Game");
    assert_eq!(
        inspection
            .schemas
            .iter()
            .map(|schema| schema.id.clone())
            .collect::<Vec<_>>(),
        ["characters", "economy", "items", "weapons"]
            .map(SchemaId::from)
            .to_vec()
    );
    assert_eq!(
        inspection
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect::<Vec<_>>(),
        ["alric", "iron_sword", "shop", "tempered_blade"]
            .map(EntityId::from)
            .to_vec()
    );
}

#[test]
fn formula_analysis_returns_source_ast_value_and_transitive_upstream_trace() {
    let mut document = game_balance_document("game", "Game");
    add_power_formula(&mut document);

    let analysis = analyze_field(
        &document,
        "balance-v1",
        &FieldRef::new("iron_sword", "power"),
    )
    .expect("formula field should be analyzable");

    assert_eq!(analysis.source.document_id, document.id);
    assert_eq!(analysis.source.source_label, "balance-v1");
    assert_eq!(analysis.calculated_value, Some(Number::new(45.0).unwrap()));
    assert_eq!(
        analysis.formula_source.as_deref(),
        Some("([iron_sword.dps] + 5)")
    );
    assert!(matches!(
        analysis.stored_value,
        tachiko_workspace_engine::Value::Formula(_)
    ));
    assert_eq!(
        analysis.direct_dependencies,
        [FieldRef::new("iron_sword", "dps")]
    );
    assert_eq!(
        analysis.upstream_dependencies,
        [
            FieldRef::new("iron_sword", "attack_interval"),
            FieldRef::new("iron_sword", "damage"),
            FieldRef::new("iron_sword", "dps"),
        ]
    );
}

#[test]
fn field_analysis_returns_transitive_downstream_impact_trace() {
    let mut document = game_balance_document("game", "Game");
    add_power_formula(&mut document);

    let analysis = analyze_field(
        &document,
        "balance-v1",
        &FieldRef::new("iron_sword", "damage"),
    )
    .expect("stored input should be analyzable");

    assert_eq!(
        analysis
            .downstream_impacts
            .iter()
            .map(|impact| (impact.field.clone(), impact.value))
            .collect::<Vec<_>>(),
        vec![
            (
                FieldRef::new("iron_sword", "dps"),
                Number::new(40.0).unwrap(),
            ),
            (
                FieldRef::new("iron_sword", "power"),
                Number::new(45.0).unwrap(),
            ),
        ]
    );
}

#[test]
fn change_analysis_returns_semantic_diff_and_affected_areas() {
    let before = game_balance_document("game", "Game");
    let mut after = before.clone();
    after.entities.get_mut("iron_sword").unwrap().fields.insert(
        FieldId::from("damage"),
        tachiko_workspace_engine::Value::Number(Number::new(45.0).unwrap()),
    );

    let analysis = analyze_changes(&before, "main@base", &after, "main@buffed")
        .expect("valid snapshots should compare");

    assert_eq!(analysis.before.source_label, "main@base");
    assert_eq!(analysis.after.source_label, "main@buffed");
    assert!(analysis.changes.iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, .. }
            if field == &FieldRef::new("iron_sword", "damage")
    )));
    assert!(analysis.changes.iter().any(|change| matches!(
        change,
        SemanticChange::FormulaImpact { field, before, after, causes }
            if field == &FieldRef::new("iron_sword", "dps")
                && *before == Number::new(40.0).unwrap()
                && *after == Number::new(50.0).unwrap()
                && causes.as_slice() == [FieldRef::new("iron_sword", "damage")]
    )));
    assert_eq!(
        analysis.affected_fields,
        [
            FieldRef::new("iron_sword", "damage"),
            FieldRef::new("iron_sword", "dps"),
        ]
    );
    assert_eq!(analysis.affected_entities, [EntityId::from("iron_sword")]);
    assert_eq!(analysis.affected_schemas, [SchemaId::from("weapons")]);
}

#[test]
fn validation_analysis_explains_current_failures_without_requiring_calculation_success() {
    let mut document = game_balance_document("game", "Game");
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(
            FieldId::from("attack_interval"),
            tachiko_workspace_engine::Value::Number(Number::new(0.0).unwrap()),
        );

    let analysis = analyze_validation(&document, "working-tree");

    assert_eq!(analysis.source.document_id, document.id);
    assert_eq!(analysis.source.source_label, "working-tree");
    assert!(!analysis.is_valid);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(
        analysis.diagnostics[0].code,
        DiagnosticCode::new("formula.division_by_zero")
    );
    assert!(analysis.diagnostics[0].message.contains("divided by zero"));
}

#[test]
fn field_analysis_reports_missing_stable_target_explicitly() {
    let document = game_balance_document("game", "Game");
    let missing = FieldRef::new("missing", "damage");

    let error = analyze_field(&document, "balance-v1", &missing)
        .expect_err("unknown stable targets must not produce fabricated analysis");

    assert!(matches!(error, WorkspaceError::MissingField { field } if field == missing));
}
