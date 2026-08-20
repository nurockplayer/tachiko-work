use std::collections::BTreeMap;

use tachiko_ai_api::{
    Suggestion, SuggestionError, describe_document, explain_formula, explain_impact,
    suggest_field_change,
};
use tachiko_diff_engine::SemanticChange;
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};

fn balance_document(damage: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("character"),
                Schema {
                    id: SchemaId::from("character"),
                    fields: BTreeMap::from([(FieldId::from("name"), text_field())]),
                },
            ),
            (
                SchemaId::from("weapon"),
                Schema {
                    id: SchemaId::from("weapon"),
                    fields: BTreeMap::from([
                        (FieldId::from("name"), text_field()),
                        (FieldId::from("damage"), number_field()),
                        (FieldId::from("attack_interval"), number_field()),
                        (FieldId::from("dps"), number_field()),
                    ]),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("aria"),
                Entity {
                    id: EntityId::from("aria"),
                    schema: SchemaId::from("character"),
                    fields: BTreeMap::from([(
                        FieldId::from("name"),
                        Value::Text("Aria".to_owned()),
                    )]),
                },
            ),
            (
                EntityId::from("sword"),
                Entity {
                    id: EntityId::from("sword"),
                    schema: SchemaId::from("weapon"),
                    fields: BTreeMap::from([
                        (FieldId::from("name"), Value::Text("Sword".to_owned())),
                        (FieldId::from("damage"), Value::Number(damage)),
                        (FieldId::from("attack_interval"), Value::Number(1.25)),
                        (
                            FieldId::from("dps"),
                            Value::Formula(Expression::Divide {
                                left: Box::new(reference("sword", "damage")),
                                right: Box::new(reference("sword", "attack_interval")),
                            }),
                        ),
                    ]),
                },
            ),
        ]),
    }
}

fn number_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Number,
        required: true,
    }
}

fn text_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Text,
        required: true,
    }
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn left_deep_sum(depth: usize) -> Expression {
    let mut expression = Expression::Number(1.0);
    for _ in 1..depth {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(Expression::Number(1.0)),
        };
    }
    expression
}

fn balanced_sum(leaves: usize) -> Expression {
    if leaves == 1 {
        return Expression::Number(1.0);
    }
    let left = leaves / 2;
    Expression::Add {
        left: Box::new(balanced_sum(left)),
        right: Box::new(balanced_sum(leaves - left)),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9);
}

#[test]
fn describe_document_returns_sorted_schema_and_entity_structure() {
    let description = describe_document(&balance_document(100.0));

    assert_eq!(description.id, DocumentId::from("balance"));
    assert_eq!(description.schemas[0].id, SchemaId::from("character"));
    assert_eq!(description.schemas[1].id, SchemaId::from("weapon"));
    assert_eq!(
        description.schemas[1]
            .fields
            .iter()
            .map(|field| &field.id)
            .collect::<Vec<_>>(),
        vec![
            &FieldId::from("attack_interval"),
            &FieldId::from("damage"),
            &FieldId::from("dps"),
            &FieldId::from("name"),
        ]
    );
    assert_eq!(
        description
            .entities
            .iter()
            .map(|entity| &entity.id)
            .collect::<Vec<_>>(),
        vec![&EntityId::from("aria"), &EntityId::from("sword")]
    );
}

#[test]
fn explain_formula_returns_calculated_value_and_sorted_direct_dependencies() {
    let explanation = explain_formula(&balance_document(100.0), &FieldRef::new("sword", "dps"))
        .expect("formula should be explainable");

    assert_eq!(explanation.field, FieldRef::new("sword", "dps"));
    assert_close(explanation.value, 80.0);
    assert_eq!(
        explanation.dependencies,
        vec![
            FieldRef::new("sword", "attack_interval"),
            FieldRef::new("sword", "damage"),
        ]
    );
}

#[test]
fn explain_impact_projects_direct_changes_and_derived_formula_impacts() {
    let impact = explain_impact(&balance_document(100.0), &balance_document(120.0))
        .expect("documents should be comparable");

    assert!(impact.changes.iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, .. } if field == &FieldRef::new("sword", "damage")
    )));
    assert!(impact.changes.iter().any(|change| matches!(
        change,
        SemanticChange::FormulaImpact { field, before, after, causes }
            if field == &FieldRef::new("sword", "dps")
                && (*before - 80.0).abs() < 1e-9
                && (*after - 96.0).abs() < 1e-9
                && causes == &vec![FieldRef::new("sword", "damage")]
    )));
}

#[test]
fn suggest_field_change_is_inert_and_requires_approval() {
    let document = balance_document(100.0);
    let original = document.clone();

    let suggestion: Suggestion = suggest_field_change(
        &document,
        FieldRef::new("sword", "damage"),
        Value::Number(120.0),
    )
    .expect("a typed existing input should be suggestible");

    assert_eq!(document, original);
    assert!(suggestion.requires_approval);
    assert_eq!(suggestion.field, FieldRef::new("sword", "damage"));
    assert_eq!(suggestion.value, Value::Number(120.0));
}

#[test]
fn typed_formula_suggestions_are_inert_validated_and_require_approval() {
    let document = balance_document(100.0);
    let original = document.clone();
    let proposed = Value::Formula(Expression::Minimum {
        left: Box::new(Expression::Number(100.0)),
        right: Box::new(Expression::Multiply {
            left: Box::new(reference("sword", "damage")),
            right: Box::new(Expression::Number(0.75)),
        }),
    });

    let suggestion =
        suggest_field_change(&document, FieldRef::new("sword", "dps"), proposed.clone())
            .expect("a valid typed formula should be approval-ready");

    assert_eq!(document, original);
    assert_eq!(suggestion.field, FieldRef::new("sword", "dps"));
    assert_eq!(suggestion.value, proposed);
    assert!(suggestion.requires_approval);

    let input_to_formula = suggest_field_change(
        &document,
        FieldRef::new("sword", "damage"),
        Value::Formula(Expression::Multiply {
            left: Box::new(reference("sword", "attack_interval")),
            right: Box::new(Expression::Number(80.0)),
        }),
    )
    .expect("a stored numeric input may become an approval-required formula");
    assert!(matches!(input_to_formula.value, Value::Formula(_)));
}

#[test]
fn typed_formula_suggestions_share_human_authoring_complexity_limits() {
    let document = balance_document(100.0);

    for expression in [left_deep_sum(64), balanced_sum(128)] {
        let suggestion = suggest_field_change(
            &document,
            FieldRef::new("sword", "dps"),
            Value::Formula(expression),
        )
        .expect("an AI formula exactly at a complexity boundary should be valid");
        assert!(suggestion.requires_approval);
    }

    for expression in [
        left_deep_sum(65),
        balanced_sum(129),
        Expression::Reference(FieldRef::new("a".repeat(4_094), "x")),
    ] {
        let error = suggest_field_change(
            &document,
            FieldRef::new("sword", "dps"),
            Value::Formula(expression),
        )
        .expect_err("AI formulas must not bypass authoring resource limits");

        assert!(
            matches!(error, SuggestionError::ExpressionComplexity { .. }),
            "unexpected error: {error:?}"
        );
    }
}

#[test]
fn typed_formula_suggestions_reject_noops_types_semantics_and_calculation() {
    let document = balance_document(100.0);
    let existing = document.entities["sword"].fields["dps"].clone();

    let no_op = suggest_field_change(&document, FieldRef::new("sword", "dps"), existing)
        .expect_err("identical formulas should not be approval-ready");
    assert!(matches!(no_op, SuggestionError::NoChange { .. }));

    let wrong_type = suggest_field_change(
        &document,
        FieldRef::new("sword", "name"),
        Value::Formula(Expression::Number(1.0)),
    )
    .expect_err("text fields cannot accept formula suggestions");
    assert!(matches!(wrong_type, SuggestionError::TypeMismatch { .. }));

    let missing_reference = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Formula(reference("missing", "damage")),
    )
    .expect_err("missing references must fail semantic validation");
    assert!(matches!(
        missing_reference,
        SuggestionError::InvalidDocument { .. }
    ));

    let cycle = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Formula(Expression::Add {
            left: Box::new(reference("sword", "dps")),
            right: Box::new(Expression::Number(1.0)),
        }),
    )
    .expect_err("cycles must fail calculation");
    assert!(matches!(cycle, SuggestionError::Calculation(_)));

    let division_by_zero = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Formula(Expression::Divide {
            left: Box::new(Expression::Number(1.0)),
            right: Box::new(Expression::Number(0.0)),
        }),
    )
    .expect_err("division by zero must fail calculation");
    assert!(matches!(division_by_zero, SuggestionError::Calculation(_)));
}

#[test]
fn suggestions_validate_fields_types_and_formula_permissions() {
    let document = balance_document(100.0);

    let missing = suggest_field_change(
        &document,
        FieldRef::new("sword", "missing"),
        Value::Number(120.0),
    )
    .expect_err("missing fields must not produce approval-ready suggestions");
    assert!(matches!(missing, SuggestionError::MissingField { .. }));

    let wrong_type = suggest_field_change(
        &document,
        FieldRef::new("sword", "damage"),
        Value::Text("high".to_owned()),
    )
    .expect_err("schema type mismatches must be rejected");
    assert!(matches!(wrong_type, SuggestionError::TypeMismatch { .. }));

    let formula = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Number(100.0),
    )
    .expect_err("computed fields must not be directly suggested");
    assert!(matches!(formula, SuggestionError::FormulaEdit { .. }));

    let no_change = suggest_field_change(
        &document,
        FieldRef::new("sword", "damage"),
        Value::Number(100.0),
    )
    .expect_err("no-op suggestions should not be presented for approval");
    assert!(matches!(no_change, SuggestionError::NoChange { .. }));
}
