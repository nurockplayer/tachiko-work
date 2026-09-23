//! Steward-owned acceptance for the explicitly selected, read-only v2 preview.
use std::collections::BTreeMap;

use tachiko_diff_engine::CanonicalDirectFact;
use tachiko_semantic_core::{KeyedGroupedSumBindingRole, KeyedGroupedSumDefinitionError};
use tachiko_workspace_engine::{
    ConflictFacetV2 as Facet, ConflictFactV2 as Fact, ConflictKind, ConflictTarget, Document,
    Entity, Expression, FieldConstraint, FieldDefinition, FieldType, KeyedGroupedSumDefinition,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, MergeConflictV2, MergePreviewV2,
    MergeValueV2 as MergeValue, Number, SEMANTIC_CONFLICT_V2, Schema, SchemaFieldSubjectV2,
    SchemaSubjectV2, ValidationRole, Value, WorkspaceError, WorkspaceMergeOutcome,
    WorkspaceMergeOutcomeV2, merge_documents, merge_documents_v2, validation_report,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn range(max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: number(0.0),
        max: number(max),
    }
}

fn document() -> Document {
    let mut document = Document::empty("document", "Base");
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields: BTreeMap::from([(
                "amount".into(),
                FieldDefinition {
                    id: "amount".into(),
                    key: "amount".into(),
                    field_type: FieldType::Number,
                    required: false,
                    constraint: FieldConstraint::None,
                },
            )]),
        },
    );
    document.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "row".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([("amount".into(), Value::Number(number(2.0)))]),
        },
    );
    document
}

fn document_with_keyed_definition() -> Document {
    let mut document = document();
    document.schemas.get_mut("schema").unwrap().fields.insert(
        "label".into(),
        FieldDefinition {
            id: "label".into(),
            key: "label".into(),
            field_type: FieldType::Text,
            required: false,
            constraint: FieldConstraint::None,
        },
    );
    document
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("label".into(), Value::Text("sku".into()));
    document.keyed_grouped_sum_definitions.insert(
        "summary".into(),
        KeyedGroupedSumDefinition {
            id: "summary".into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: "schema".into(),
                lookup_key_field: "label".into(),
                quantity_field: "amount".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "schema".into(),
                key_field: "label".into(),
                category_field: "label".into(),
                price_field: "amount".into(),
            },
        },
    );
    document
}

fn invalidate_keyed_binding_without_core_error(document: &mut Document) {
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("label")
        .unwrap()
        .field_type = FieldType::Number;
    document
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("label".into(), Value::Number(number(3.0)));
    assert!(validation_report(document).is_valid());
}

fn set_constraint(document: &mut Document, constraint: FieldConstraint) {
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .constraint = constraint;
}

fn set_value(document: &mut Document, value: Value) {
    document
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("amount".into(), value);
}

fn merged(base: &Document, left: &Document, right: &Document) -> MergePreviewV2 {
    match merge_documents_v2(SEMANTIC_CONFLICT_V2, base, left, right).unwrap() {
        WorkspaceMergeOutcomeV2::Merged(preview) => *preview,
        WorkspaceMergeOutcomeV2::Conflicted(conflicts) => panic!("unexpected: {conflicts:?}"),
    }
}

fn conflicted(base: &Document, left: &Document, right: &Document) -> Vec<MergeConflictV2> {
    match merge_documents_v2(SEMANTIC_CONFLICT_V2, base, left, right).unwrap() {
        WorkspaceMergeOutcomeV2::Conflicted(conflicts) => conflicts,
        WorkspaceMergeOutcomeV2::Merged(_) => panic!("expected conflict"),
    }
}

fn field_subject(document: &Document) -> SchemaFieldSubjectV2 {
    let field = &document.schemas["schema"].fields["amount"];
    SchemaFieldSubjectV2 {
        key: field.key.clone(),
        field_type: field.field_type.clone(),
        required: field.required,
        constraint: field.constraint.clone(),
    }
}

#[test]
fn unchanged_equal_and_one_sided_constraints_produce_canonical_v2_delta() {
    let base = document();
    let unchanged = merged(&base, &base, &base);
    assert_eq!(unchanged.document, base);
    assert_eq!(unchanged.delta.contract(), "tachiko.semantic-delta/v2");
    assert!(unchanged.delta.facts().is_empty());

    let mut changed = base.clone();
    set_constraint(&mut changed, range(10.0));
    for (left, right) in [(&changed, &base), (&base, &changed), (&changed, &changed)] {
        let preview = merged(&base, left, right);
        assert_eq!(preview.document, changed);
        assert_eq!(preview.delta.document_id(), &base.id);
        assert_eq!(
            preview.delta.facts(),
            &[CanonicalDirectFact::SchemaFieldConstraintChanged {
                schema: "schema".into(),
                field: "amount".into(),
                before: FieldConstraint::None,
                after: range(10.0),
            }]
        );
    }
    let cleared = merged(&changed, &base, &changed);
    assert_eq!(cleared.document, base);
    assert_eq!(
        cleared.delta.facts(),
        &[CanonicalDirectFact::SchemaFieldConstraintChanged {
            schema: "schema".into(),
            field: "amount".into(),
            before: range(10.0),
            after: FieldConstraint::None,
        }]
    );
}

#[test]
fn unequal_complete_constraints_are_one_atomic_typed_conflict() {
    let base = document();
    let mut left = base.clone();
    let mut right = base.clone();
    set_constraint(&mut left, range(10.0));
    set_constraint(&mut right, range(20.0));
    let conflicts = conflicted(&base, &left, &right);
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.contract(), "tachiko.semantic-conflict/v2");
    assert_eq!(conflict.document(), &base.id);
    assert_eq!(
        conflict.target(),
        &ConflictTarget::SchemaField {
            schema: "schema".into(),
            field: "amount".into(),
        }
    );
    assert_eq!(conflict.facet(), Facet::Constraint);
    assert_eq!(conflict.kind(), ConflictKind::ConcurrentChange);
    assert_eq!(
        conflict.base(),
        &Fact::Present(MergeValue::Constraint(FieldConstraint::None))
    );
    assert_eq!(
        conflict.left(),
        &Fact::Present(MergeValue::Constraint(range(10.0)))
    );
    assert_eq!(
        conflict.right(),
        &Fact::Present(MergeValue::Constraint(range(20.0)))
    );
    assert_eq!(conflicted(&base, &left, &right), conflicts);
}

#[test]
fn concurrent_field_addition_uses_complete_subject_and_suppresses_child_facets() {
    let mut base = document();
    base.entities.clear();
    base.schemas.get_mut("schema").unwrap().fields.clear();
    let mut left = document();
    left.entities.clear();
    let mut right = left.clone();
    set_constraint(&mut right, range(20.0));
    let conflicts = conflicted(&base, &left, &right);
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.facet(), Facet::Subject);
    assert_eq!(conflict.kind(), ConflictKind::ConcurrentAddition);
    assert_eq!(conflict.base(), &Fact::Absent);
    assert_eq!(
        conflict.left(),
        &Fact::Present(MergeValue::SchemaFieldSubject(field_subject(&left)))
    );
    assert_eq!(
        conflict.right(),
        &Fact::Present(MergeValue::SchemaFieldSubject(field_subject(&right)))
    );
}

#[test]
fn schema_delete_modify_conflict_carries_constraint_and_suppresses_descendants() {
    let mut base = document();
    base.entities.clear();
    let mut left = base.clone();
    left.schemas.clear();
    let mut right = base.clone();
    set_constraint(&mut right, range(20.0));
    let conflicts = conflicted(&base, &left, &right);
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.target(), &ConflictTarget::Schema("schema".into()));
    assert_eq!(conflict.facet(), Facet::Subject);
    assert_eq!(conflict.kind(), ConflictKind::DeleteModify);
    assert_eq!(conflict.left(), &Fact::Absent);
    for (fact, source) in [(conflict.base(), &base), (conflict.right(), &right)] {
        assert_eq!(
            fact,
            &Fact::Present(MergeValue::SchemaSubject(SchemaSubjectV2 {
                key: "items".into(),
                fields: BTreeMap::from([("amount".into(), field_subject(source))]),
            }))
        );
    }
}

#[test]
fn field_delete_vs_constraint_edit_uses_complete_subject_and_suppresses_constraint_facet() {
    let base = document();
    let mut left = base.clone();
    left.schemas.get_mut("schema").unwrap().fields.clear();
    left.entities.get_mut("entity").unwrap().fields.clear();
    let mut right = base.clone();
    set_constraint(&mut right, range(10.0));

    let conflicts = conflicted(&base, &left, &right);
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(
        conflict.target(),
        &ConflictTarget::SchemaField {
            schema: "schema".into(),
            field: "amount".into(),
        }
    );
    assert_eq!(conflict.facet(), Facet::Subject);
    assert_eq!(conflict.kind(), ConflictKind::DeleteModify);
    assert_eq!(
        conflict.base(),
        &Fact::Present(MergeValue::SchemaFieldSubject(field_subject(&base)))
    );
    assert_eq!(conflict.left(), &Fact::Absent);
    assert_eq!(
        conflict.right(),
        &Fact::Present(MergeValue::SchemaFieldSubject(field_subject(&right)))
    );
}

#[test]
fn conflict_order_is_target_then_facet_with_constraint_after_existing_field_facets() {
    let base = document();
    let mut left = base.clone();
    let mut right = base.clone();
    for (side, name, max, value) in [
        (&mut left, "left", 10.0, 3.0),
        (&mut right, "right", 20.0, 4.0),
    ] {
        side.title = name.into();
        let schema = side.schemas.get_mut("schema").unwrap();
        schema.key = name.into();
        schema.fields.get_mut("amount").unwrap().key = name.into();
        set_constraint(side, range(max));
        side.entities.get_mut("entity").unwrap().key = name.into();
        set_value(side, Value::Number(number(value)));
    }
    let conflicts = conflicted(&base, &left, &right);
    assert_eq!(
        conflicts
            .iter()
            .map(|c| (c.target().clone(), c.facet()))
            .collect::<Vec<_>>(),
        vec![
            (ConflictTarget::Document(base.id.clone()), Facet::Title),
            (ConflictTarget::Schema("schema".into()), Facet::Key),
            (
                ConflictTarget::SchemaField {
                    schema: "schema".into(),
                    field: "amount".into()
                },
                Facet::Key
            ),
            (
                ConflictTarget::SchemaField {
                    schema: "schema".into(),
                    field: "amount".into()
                },
                Facet::Constraint
            ),
            (ConflictTarget::Entity("entity".into()), Facet::Key),
            (
                ConflictTarget::StoredEntityField {
                    entity: "entity".into(),
                    schema: "schema".into(),
                    field: "amount".into()
                },
                Facet::StoredValue
            ),
        ]
    );
}

#[test]
fn constraint_facet_is_valid_only_for_a_schema_field_target() {
    assert!(
        Facet::Constraint
            .validate_target(&ConflictTarget::SchemaField {
                schema: "schema".into(),
                field: "amount".into(),
            })
            .is_ok()
    );
    for target in [
        ConflictTarget::Document("document".into()),
        ConflictTarget::Schema("schema".into()),
        ConflictTarget::Entity("entity".into()),
        ConflictTarget::StoredEntityField {
            entity: "entity".into(),
            schema: "schema".into(),
            field: "amount".into(),
        },
    ] {
        assert!(Facet::Constraint.validate_target(&target).is_err());
    }
}

#[test]
fn selector_and_document_identity_are_admission_boundaries() {
    let base = document();
    let mut wrong = base.clone();
    wrong.id = "another-document".into();
    for selector in [
        "tachiko.semantic-conflict/v1",
        "tachiko.semantic-conflict/v999",
        "",
    ] {
        let error = merge_documents_v2(selector, &base, &wrong, &base).unwrap_err();
        let WorkspaceError::UnsupportedSemanticConflictContract { contract } = error else {
            panic!("wrong selector refusal: {error:?}");
        };
        assert_eq!(contract, selector);
    }
    assert!(matches!(
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &wrong, &base),
        Err(WorkspaceError::DifferentMergeDocument { .. })
    ));
}

#[test]
fn invalid_inputs_preserve_each_role_and_complete_shared_diagnostics() {
    let base = document();
    let mut invalid = base.clone();
    set_constraint(&mut invalid, range(1.0));
    let expected = validation_report(&invalid).stable_observations();
    assert!(!expected.is_empty());
    for (a, b, c, expected_role) in [
        (&invalid, &base, &base, ValidationRole::MergeBase),
        (&base, &invalid, &base, ValidationRole::MergeOurs),
        (&base, &base, &invalid, ValidationRole::MergeTheirs),
    ] {
        let error = merge_documents_v2(SEMANTIC_CONFLICT_V2, a, b, c).unwrap_err();
        let WorkspaceError::InvalidDocument { role, report, .. } = error else {
            panic!("wrong refusal: {error:?}");
        };
        assert_eq!(role, expected_role);
        assert_eq!(report.stable_observations(), expected);
    }
}

#[test]
fn unchanged_invalid_keyed_definition_refuses_before_preview_or_delta() {
    let mut base = document_with_keyed_definition();
    base.keyed_grouped_sum_definitions
        .get_mut(&"summary".into())
        .unwrap()
        .orders
        .schema = "missing".into();
    let mut right = base.clone();
    right.title = "Right".into();
    assert!(validation_report(&base).is_valid());
    assert!(matches!(
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &base, &right),
        Err(WorkspaceError::InvalidMergeKeyedGroupedSumDefinitions {
            role: ValidationRole::MergeBase,
            source: KeyedGroupedSumDefinitionError::MissingSchema {
                role: "orders",
                schema,
            },
        }) if schema == "missing".into()
    ));
}

#[test]
fn unchanged_keyed_definition_checks_each_input_binding_with_its_role() {
    let valid = document_with_keyed_definition();
    let mut invalid = valid.clone();
    invalidate_keyed_binding_without_core_error(&mut invalid);
    for (base, left, right, expected_role) in [
        (&invalid, &valid, &valid, ValidationRole::MergeBase),
        (&valid, &invalid, &valid, ValidationRole::MergeOurs),
        (&valid, &valid, &invalid, ValidationRole::MergeTheirs),
    ] {
        assert!(matches!(
            merge_documents_v2(SEMANTIC_CONFLICT_V2, base, left, right),
            Err(WorkspaceError::InvalidMergeKeyedGroupedSumDefinitions {
                role,
                source: KeyedGroupedSumDefinitionError::WrongFieldType {
                    role: KeyedGroupedSumBindingRole::OrdersLookupKey,
                    schema,
                    field,
                    expected: FieldType::Text,
                    actual: FieldType::Number,
                },
            }) if role == expected_role && schema == "schema".into() && field == "label".into()
        ));
    }
}

#[test]
fn changed_keyed_definition_map_refuses_before_input_admission() {
    let base = document_with_keyed_definition();
    let mut left = base.clone();
    left.keyed_grouped_sum_definitions.clear();
    set_constraint(&mut left, range(1.0));
    assert!(!validation_report(&left).is_valid());
    assert!(matches!(
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &left, &base),
        Err(WorkspaceError::UnsupportedKeyedGroupedSumDefinitionMerge)
    ));
}

#[test]
fn combined_stored_or_formula_range_failure_is_candidate_diagnostics_not_conflict() {
    for formula in [false, true] {
        let mut base = document();
        if formula {
            set_value(&mut base, Value::Formula(Expression::Number(number(2.0))));
        }
        let mut left = base.clone();
        set_constraint(&mut left, range(5.0));
        let mut right = base.clone();
        let value = if formula {
            Value::Formula(Expression::Number(number(9.0)))
        } else {
            Value::Number(number(9.0))
        };
        set_value(&mut right, value.clone());
        for source in [&base, &left, &right] {
            assert!(validation_report(source).is_valid());
        }
        let originals = (base.clone(), left.clone(), right.clone());
        let mut invalid_candidate = left.clone();
        set_value(&mut invalid_candidate, value);
        let expected = validation_report(&invalid_candidate).stable_observations();
        let error = merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &left, &right).unwrap_err();
        let WorkspaceError::InvalidDocument { role, report, .. } = error else {
            panic!("candidate must fail validation: {error:?}");
        };
        assert_eq!(role, ValidationRole::MergeCandidate);
        assert_eq!(report.stable_observations(), expected);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.code.as_str() == "core.field_value_constraint_mismatch")
        );
        assert_eq!((base, left, right), originals);
    }
}

#[test]
fn explicit_v2_preview_does_not_reinterpret_frozen_v1_entry() {
    let base = document();
    assert!(matches!(
        merge_documents(&base, &base, &base),
        Ok(WorkspaceMergeOutcome::Merged(_))
    ));
    let mut constrained = base.clone();
    set_constraint(&mut constrained, range(10.0));
    assert!(matches!(
        merge_documents(&constrained, &constrained, &constrained),
        Err(WorkspaceError::UnsupportedFieldConstraintMerge)
    ));
    let result = merged(&constrained, &constrained, &constrained);
    assert_eq!(result.document, constrained);
    assert!(result.delta.facts().is_empty());
}
