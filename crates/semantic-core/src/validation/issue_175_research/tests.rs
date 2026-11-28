use std::collections::BTreeMap;

use crate::{
    Diagnostic, DiagnosticCode, Entity, EntityId, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldRef, FieldType, Number, Schema, SchemaId, Value,
};

use super::{Document, validate_document_cancellable, validate_expression};

#[test]
fn completed_research_validation_matches_the_accepted_validator() {
    let mut document = Document::empty("", " ");
    document.schemas.insert(
        SchemaId::from("schema-store-id"),
        Schema {
            headerless: false,
            field_order: None,
            id: SchemaId::from("schema-declared-id"),
            key: "Invalid Schema Key".into(),
            display_name: None,
            fields: BTreeMap::from([
                (
                    FieldId::from("name"),
                    FieldDefinition {
                        id: FieldId::from("name"),
                        key: "name".into(),
                        display_name: None,
                        number_format: None,
                        date_format: None,
                        field_type: FieldType::Text,
                        required: true,
                        constraint: crate::FieldConstraint::None,
                    },
                ),
                (
                    FieldId::from("amount"),
                    FieldDefinition {
                        id: FieldId::from("different-amount-id"),
                        key: "Invalid Amount Key".into(),
                        display_name: None,
                        number_format: None,
                        date_format: None,
                        field_type: FieldType::Number,
                        required: true,
                        constraint: crate::FieldConstraint::None,
                    },
                ),
                (
                    FieldId::from("calc"),
                    FieldDefinition {
                        id: FieldId::from("calc"),
                        key: "calc".into(),
                        display_name: None,
                        number_format: None,
                        date_format: None,
                        field_type: FieldType::Number,
                        required: true,
                        constraint: crate::FieldConstraint::None,
                    },
                ),
            ]),
        },
    );
    document.entities.insert(
        EntityId::from("entity-store-id"),
        Entity {
            rank: None,
            id: EntityId::from("entity-declared-id"),
            key: "Invalid Entity Key".into(),
            schema: SchemaId::from("schema-store-id"),
            fields: BTreeMap::from([
                (FieldId::from("amount"), Value::Text("wrong".to_owned())),
                (
                    FieldId::from("calc"),
                    Value::Formula(Expression::Reference(FieldRef::new(
                        "entity-store-id",
                        "name",
                    ))),
                ),
                (FieldId::from("unexpected"), Value::Boolean(true)),
            ]),
        },
    );

    assert_eq!(
        validate_document_cancellable(&document, || false),
        Some(super::super::validate_document(&document)),
    );
}

#[test]
fn cancelled_research_validation_never_publishes_diagnostics() {
    let document = Document::empty("document", "Document");

    assert_eq!(validate_document_cancellable(&document, || true), None);
}

#[test]
fn constrained_declarations_and_stored_values_match_or_cancel_without_formula_oracles() {
    let valid = constrained_text_document(
        FieldConstraint::TextLiteralSet {
            values: vec!["allowed".to_owned()],
        },
        "allowed",
    );
    assert_eq!(
        validate_document_cancellable(&valid, || false),
        Some(super::super::validate_document(&valid)),
    );

    let invalid = constrained_text_document(
        FieldConstraint::TextLiteralSet { values: Vec::new() },
        "disallowed",
    );
    let completed = validate_document_cancellable(&invalid, || false)
        .expect("uncancelled validation completes");
    assert_eq!(completed, super::super::validate_document(&invalid));
    assert!(
        completed
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::FIELD_CONSTRAINT_DECLARATION)
    );
    assert!(
        completed.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::FIELD_VALUE_CONSTRAINT_MISMATCH
        })
    );
    assert_eq!(validate_document_cancellable(&invalid, || true), None);
}

fn constrained_text_document(constraint: FieldConstraint, value: &str) -> Document {
    let mut document = Document::empty("document", "Document");
    document.schemas.insert(
        SchemaId::from("schema"),
        Schema {
            headerless: false,
            field_order: None,
            id: SchemaId::from("schema"),
            key: "items".into(),
            display_name: None,
            fields: BTreeMap::from([(
                FieldId::from("name"),
                FieldDefinition {
                    id: FieldId::from("name"),
                    key: "name".into(),
                    display_name: None,
                    number_format: None,
                    date_format: None,
                    field_type: FieldType::Text,
                    required: true,
                    constraint,
                },
            )]),
        },
    );
    document.entities.insert(
        EntityId::from("entity"),
        Entity {
            rank: None,
            id: EntityId::from("entity"),
            key: "item".into(),
            schema: SchemaId::from("schema"),
            fields: BTreeMap::from([(FieldId::from("name"), Value::Text(value.to_owned()))]),
        },
    );
    document
}

#[test]
fn formula_traversal_polls_cancellation_at_the_bounded_node_interval() {
    let document = Document::empty("document", "Document");
    let owner = FieldRef::new("entity", "field");
    let one = || Expression::Number(Number::new(1.0).unwrap());
    let mut expression = one();
    for _ in 0..65 {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(one()),
        };
    }
    let mut diagnostics = Vec::<Diagnostic>::new();
    let mut polls = 0_usize;

    let result = validate_expression(
        &document,
        &owner,
        &expression,
        "entities.entity.fields.field",
        &mut diagnostics,
        &mut || {
            polls += 1;
            polls == 2
        },
    );

    assert!(result.is_err());
    assert_eq!(polls, 2);
    assert_eq!(diagnostics.as_slice(), []);
}
