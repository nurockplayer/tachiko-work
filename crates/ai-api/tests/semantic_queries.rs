use std::collections::BTreeMap;

use tachiko_ai_api::{
    ImpactExplanationError, Suggestion, SuggestionError, describe_document, explain_formula,
    explain_impact, suggest_field_change,
};
use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldKey,
    FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, SemanticChange, ValidationRole,
    Value,
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
                    key: SchemaKey::from("character"),
                    fields: BTreeMap::from([(FieldId::from("name"), text_field("name"))]),
                },
            ),
            (
                SchemaId::from("weapon"),
                Schema {
                    id: SchemaId::from("weapon"),
                    key: SchemaKey::from("weapon"),
                    fields: BTreeMap::from([
                        (FieldId::from("name"), text_field("name")),
                        (FieldId::from("damage"), number_field("damage")),
                        (
                            FieldId::from("attack_interval"),
                            number_field("attack_interval"),
                        ),
                        (FieldId::from("dps"), number_field("dps")),
                    ]),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("aria"),
                Entity {
                    id: EntityId::from("aria"),
                    key: "aria".into(),
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
                    key: "sword".into(),
                    schema: SchemaId::from("weapon"),
                    fields: BTreeMap::from([
                        (FieldId::from("name"), Value::Text("Sword".to_owned())),
                        (FieldId::from("damage"), number(damage)),
                        (FieldId::from("attack_interval"), number(1.25)),
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

fn field(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type,
        required: true,
    }
}

fn number_field(id: &str) -> FieldDefinition {
    field(id, FieldType::Number)
}

fn text_field(id: &str) -> FieldDefinition {
    field(id, FieldType::Text)
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn numeric(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn left_deep_sum(depth: usize) -> Expression {
    let mut expression = numeric(1.0);
    for _ in 1..depth {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(numeric(1.0)),
        };
    }
    expression
}

fn balanced_sum(leaves: usize) -> Expression {
    if leaves == 1 {
        return numeric(1.0);
    }
    let left = leaves / 2;
    Expression::Add {
        left: Box::new(balanced_sum(left)),
        right: Box::new(balanced_sum(leaves - left)),
    }
}

fn assert_close(actual: Number, expected: f64) {
    assert!((actual.get() - expected).abs() < 1e-9);
}

#[test]
fn describe_document_returns_sorted_schema_and_entity_structure() {
    let description = describe_document(&balance_document(100.0));

    assert_eq!(description.id, DocumentId::from("balance"));
    assert_eq!(description.schemas[0].id, SchemaId::from("character"));
    assert_eq!(description.schemas[0].key, SchemaKey::from("character"));
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
                && (before.get() - 80.0).abs() < 1e-9
                && (after.get() - 96.0).abs() < 1e-9
                && causes == &vec![FieldRef::new("sword", "damage")]
    )));
}

#[test]
fn explain_impact_preserves_invalid_operand_role() {
    let valid = balance_document(100.0);
    let mut invalid = valid.clone();
    invalid
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));

    for (result, expected) in [
        (
            explain_impact(&invalid, &valid),
            ValidationRole::ComparisonBefore,
        ),
        (
            explain_impact(&valid, &invalid),
            ValidationRole::ComparisonAfter,
        ),
    ] {
        assert!(matches!(
            result.unwrap_err(),
            ImpactExplanationError::InvalidDocument { role, .. } if role == expected
        ));
    }
}

#[test]
fn suggest_field_change_is_inert_and_requires_approval() {
    let document = balance_document(100.0);
    let original = document.clone();

    let suggestion: Suggestion =
        suggest_field_change(&document, FieldRef::new("sword", "damage"), number(120.0))
            .expect("a typed existing input should be suggestible");

    assert_eq!(document, original);
    assert!(suggestion.requires_approval);
    assert_eq!(suggestion.field, FieldRef::new("sword", "damage"));
    assert_eq!(suggestion.value, number(120.0));
}

#[test]
fn typed_formula_suggestions_are_inert_validated_and_require_approval() {
    let document = balance_document(100.0);
    let original = document.clone();
    let proposed = Value::Formula(Expression::Minimum {
        left: Box::new(numeric(100.0)),
        right: Box::new(Expression::Multiply {
            left: Box::new(reference("sword", "damage")),
            right: Box::new(numeric(0.75)),
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
            right: Box::new(numeric(80.0)),
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

    for expression in [left_deep_sum(65), balanced_sum(129)] {
        let error = suggest_field_change(
            &document,
            FieldRef::new("sword", "dps"),
            Value::Formula(expression),
        )
        .expect_err("AI formulas must not bypass authoring resource limits");

        assert!(
            matches!(&error, SuggestionError::ExpressionComplexity { .. }),
            "unexpected error: {error:?}"
        );
        assert_eq!(error.clone(), error, "adapter errors remain value types");
        assert_eq!(
            error.to_string().matches("formula for").count(),
            1,
            "workspace delegation must not duplicate the adapter context"
        );
    }

    let error = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Formula(Expression::Reference(FieldRef::new("a".repeat(4_094), "x"))),
    )
    .expect_err("an unresolvable bound reference must not gain fabricated source text");
    assert!(matches!(error, SuggestionError::FormulaProjection { .. }));
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
        Value::Formula(numeric(1.0)),
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
        SuggestionError::FormulaProjection { .. }
    ));

    let cycle = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Formula(Expression::Add {
            left: Box::new(reference("sword", "dps")),
            right: Box::new(numeric(1.0)),
        }),
    )
    .expect_err("cycles must fail calculation");
    assert!(matches!(cycle, SuggestionError::InvalidDocument { .. }));

    let division_by_zero = suggest_field_change(
        &document,
        FieldRef::new("sword", "dps"),
        Value::Formula(Expression::Divide {
            left: Box::new(numeric(1.0)),
            right: Box::new(numeric(0.0)),
        }),
    )
    .expect_err("division by zero must fail calculation");
    assert!(matches!(
        division_by_zero,
        SuggestionError::InvalidDocument { .. }
    ));
}

#[test]
fn suggestions_validate_fields_types_and_formula_permissions() {
    let document = balance_document(100.0);

    let missing = suggest_field_change(&document, FieldRef::new("sword", "missing"), number(120.0))
        .expect_err("missing fields must not produce approval-ready suggestions");
    assert!(matches!(missing, SuggestionError::MissingField { .. }));

    let wrong_type = suggest_field_change(
        &document,
        FieldRef::new("sword", "damage"),
        Value::Text("high".to_owned()),
    )
    .expect_err("schema type mismatches must be rejected");
    assert!(matches!(wrong_type, SuggestionError::TypeMismatch { .. }));

    let formula = suggest_field_change(&document, FieldRef::new("sword", "dps"), number(100.0))
        .expect_err("computed fields must not be directly suggested");
    assert!(matches!(formula, SuggestionError::FormulaEdit { .. }));

    let no_change =
        suggest_field_change(&document, FieldRef::new("sword", "damage"), number(100.0))
            .expect_err("no-op suggestions should not be presented for approval");
    assert!(matches!(no_change, SuggestionError::NoChange { .. }));
}
