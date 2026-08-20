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
