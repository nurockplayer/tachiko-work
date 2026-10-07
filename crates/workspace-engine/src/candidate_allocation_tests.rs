//! Frozen acceptance for request-local owned candidate reuse.
use std::collections::BTreeMap;

use super::{
    AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId, MutationClass,
    OperationFamily, PatchLifecycle, PolicyMeaningId, SemanticApiContract, SemanticCommand,
    SemanticPatchBody,
};
use crate::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType, Number,
    Schema, Value, WorkspaceError, field_value_candidate,
};

fn fixture() -> Document {
    let mut document = Document::empty("doc", "Owned candidate");
    let fields = [
        ("text", FieldType::Text, true),
        ("number", FieldType::Number, true),
        ("optional", FieldType::Boolean, false),
        (
            "reference",
            FieldType::Reference { schema: "s".into() },
            false,
        ),
    ]
    .into_iter()
    .map(|(id, field_type, required)| {
        (
            id.into(),
            FieldDefinition {
                id: id.into(),
                key: id.into(),
                field_type,
                required,
                constraint: FieldConstraint::None,
            },
        )
    })
    .collect();
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            fields,
        },
    );
    document.entities.insert(
        "e".into(),
        Entity {
            id: "e".into(),
            key: "e".into(),
            schema: "s".into(),
            fields: BTreeMap::from([
                ("text".into(), Value::Text("old".into())),
                ("number".into(), Value::Number(Number::new(1.0).unwrap())),
            ]),
        },
    );
    document
}

fn lifecycle() -> PatchLifecycle {
    PatchLifecycle::new(
        AuthorizationDomainId::from("domain"),
        DocumentScopeId::from("scope"),
        "doc".into(),
        SemanticApiContract::from("contract"),
        AuthorizationPolicyVersion::from("policy"),
        PolicyMeaningId::from("meaning"),
    )
}

#[test]
fn scalar_optional_reference_and_formula_candidates_match_borrowed_route() {
    let base = fixture();
    let original = base.clone();
    for (field, value, class) in [
        (
            "text",
            Value::Text("雪\r\n001".into()),
            MutationClass::Value,
        ),
        (
            "number",
            Value::Number(Number::new(2.0).unwrap()),
            MutationClass::Value,
        ),
        ("optional", Value::Boolean(false), MutationClass::Value),
        (
            "reference",
            Value::Reference("e".into()),
            MutationClass::Value,
        ),
        (
            "number",
            Value::Formula(Expression::Number(Number::new(3.0).unwrap())),
            MutationClass::Formula,
        ),
    ] {
        let target = FieldRef::new("e", field);
        let mut expected = base.clone();
        expected
            .entities
            .get_mut("e")
            .unwrap()
            .fields
            .insert(target.field.clone(), value.clone());
        assert_eq!(
            field_value_candidate(&base, &target, &value).unwrap(),
            expected
        );
        let body =
            SemanticPatchBody::atomic_batch(vec![SemanticCommand::set_field_value(target, value)])
                .unwrap();
        let (candidate, writes) = lifecycle().plan_commands(&base, &body).unwrap();
        assert_eq!(candidate, expected);
        assert_eq!(writes.len(), 1);
        let write = writes.into_iter().next().unwrap();
        assert_eq!(write.family, OperationFamily::SetFieldValue);
        assert_eq!(write.mutation_class, class);
        assert_eq!(base, original);
    }
}

#[test]
fn atomic_repeated_writes_use_intermediate_candidate_and_preserve_input() {
    let base = fixture();
    let original = base.clone();
    let target = FieldRef::new("e", "text");
    let body = SemanticPatchBody::atomic_batch(vec![
        SemanticCommand::set_field_value(target.clone(), Value::Text("first".into())),
        SemanticCommand::set_field_value(target.clone(), Value::Text("second".into())),
    ])
    .unwrap();
    let (candidate, writes) = lifecycle().plan_commands(&base, &body).unwrap();
    assert_eq!(
        candidate.entities["e"].fields["text"],
        Value::Text("second".into())
    );
    assert_eq!(writes.len(), 1);
    assert_eq!(base, original);
    let failed = SemanticPatchBody::atomic_batch(vec![
        SemanticCommand::set_field_value(target.clone(), Value::Text("first".into())),
        SemanticCommand::set_field_value(target.clone(), Value::Text("first".into())),
        SemanticCommand::set_field_value(
            FieldRef::new("absent", "text"),
            Value::Text("later".into()),
        ),
    ])
    .unwrap();
    let error = lifecycle().plan_commands(&base, &failed).unwrap_err();
    assert!(matches!(error, WorkspaceError::NoChange { field } if field == target));
    assert_eq!(base, original);
}

#[test]
fn refused_candidates_preserve_exact_borrowed_error_and_earlier_input() {
    let base = fixture();
    for (target, value, expected) in [
        (
            FieldRef::new("absent", "text"),
            Value::Text("changed".into()),
            WorkspaceError::MissingEntityId {
                entity: "absent".into(),
            },
        ),
        (
            FieldRef::new("e", "absent"),
            Value::Text("changed".into()),
            WorkspaceError::MissingField {
                field: FieldRef::new("e", "absent"),
            },
        ),
        (
            FieldRef::new("e", "text"),
            Value::Boolean(true),
            WorkspaceError::TypeMismatch {
                field: FieldRef::new("e", "text"),
            },
        ),
        (
            FieldRef::new("e", "text"),
            Value::Text("old".into()),
            WorkspaceError::NoChange {
                field: FieldRef::new("e", "text"),
            },
        ),
        (
            FieldRef::new("e", "optional"),
            Value::Formula(Expression::Number(Number::new(2.0).unwrap())),
            WorkspaceError::MissingField {
                field: FieldRef::new("e", "optional"),
            },
        ),
    ] {
        let original = base.clone();
        let borrowed = field_value_candidate(&base, &target, &value).unwrap_err();
        assert_eq!(format!("{borrowed:?}"), format!("{expected:?}"));
        let body =
            SemanticPatchBody::atomic_batch(vec![SemanticCommand::set_field_value(target, value)])
                .unwrap();
        let actual = lifecycle().plan_commands(&base, &body).unwrap_err();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        assert_eq!(base, original);
    }
}
