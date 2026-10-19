//! ADR-0045 commands, v2 evidence and final merge validation.
use std::collections::BTreeMap;
use tachiko_diff_engine::{CanonicalDirectFact as Fact, CanonicalSemanticDelta};
use tachiko_workspace_engine::{
    ConflictFactV2, ConflictKind, Document, Entity, Expression, FieldConstraint, FieldDefinition,
    FieldRef, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumOrdersBinding,
    KeyedGroupedSumProductsBinding, Number, SEMANTIC_CONFLICT_V2, Schema, ValidationRole, Value,
    WorkspaceError, WorkspaceMergeOutcomeV2,
    keyed_grouped_sum_operations::{KeyedGroupedSumOutcome, evaluate_keyed_grouped_sum},
    merge_documents_v2,
    patch_lifecycle::{
        AuthorizationAction, Grant, GrantRequirement, MutationClass, OperationFamily,
        PatchLifecycle, PatchLifecycleError, PrincipalKind, ProposalRequest, ScopedSemanticSubject,
        SemanticCommand, SemanticPatchBody, TrustedInstant, V2ReceiptEvidence,
    },
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
    validation_report,
};

fn number(n: f64) -> Value {
    Value::Number(Number::new(n).unwrap())
}

fn document() -> Document {
    let mut doc = Document::empty("doc", "Optional fields");
    let fields = [("label", FieldType::Text), ("amount", FieldType::Number)]
        .into_iter()
        .map(|(id, kind)| {
            let field = FieldDefinition {
                id: id.into(),
                key: id.into(),
                display_name: None,
                field_type: kind,
                required: true,
                constraint: FieldConstraint::None,
            };
            (id.into(), field)
        })
        .collect();
    doc.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "items".into(),
            display_name: None,
            fields,
        },
    );
    for id in ["a", "b"] {
        doc.entities.insert(
            id.into(),
            Entity {
                id: id.into(),
                key: id.into(),
                schema: "s".into(),
                fields: BTreeMap::from([
                    ("label".into(), Value::Text(id.into())),
                    ("amount".into(), number(2.0)),
                ]),
            },
        );
    }
    doc
}

fn required(required: bool) -> SemanticCommand {
    SemanticCommand::SetSchemaFieldRequiredness {
        schema: "s".into(),
        field: "amount".into(),
        required,
    }
}

fn clear() -> SemanticCommand {
    SemanticCommand::UnsetField {
        field: FieldRef::new("a", "amount"),
    }
}

struct Clock;
impl TrustedPublicationTimeSource for Clock {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(20)
    }
}

fn lifecycle() -> PatchLifecycle {
    let mut lifecycle = PatchLifecycle::new_v2(
        "domain".into(),
        "scope".into(),
        "doc".into(),
        "api".into(),
        "policy".into(),
        "meaning".into(),
    );
    for id in ["editor", "authority"] {
        lifecycle
            .register_principal(id.into(), PrincipalKind::Human)
            .unwrap();
    }
    let scope = ScopedSemanticSubject::new(
        "scope".into(),
        "doc".into(),
        tachiko_workspace_engine::patch_lifecycle::SemanticScope::Document,
    );
    let mut requirements = Vec::new();
    for family in [
        OperationFamily::SchemaFieldMutation,
        OperationFamily::SetFieldValue,
        OperationFamily::AppendEntity,
        OperationFamily::RemoveEntity,
    ] {
        requirements.push(GrantRequirement::query(family, scope.clone()));
        for action in [AuthorizationAction::Propose, AuthorizationAction::Execute] {
            for class in [
                MutationClass::Schema,
                MutationClass::Structure,
                MutationClass::Value,
                MutationClass::Destructive,
            ] {
                requirements.push(
                    GrantRequirement::mutation(action, family, class, scope.clone()).unwrap(),
                );
            }
        }
    }
    lifecycle
        .provision_grant(Grant::new(
            "grant".into(),
            "authority".into(),
            "editor".into(),
            requirements,
            None,
        ))
        .unwrap();
    lifecycle
}

fn apply(
    lifecycle: &mut PatchLifecycle,
    session: &mut ResidentWorkspaceSession,
    commands: Vec<SemanticCommand>,
) -> Result<CanonicalSemanticDelta, PatchLifecycleError> {
    let snapshot = session.export_snapshot();
    let id = format!("p/{}", session.revision().as_str());
    let id = tachiko_workspace_engine::patch_lifecycle::ProposalId::from(id);
    lifecycle.propose(
        snapshot.document_scope(),
        snapshot.document(),
        snapshot.revision(),
        ProposalRequest::new(
            id.clone(),
            snapshot.revision().clone(),
            SemanticPatchBody::atomic_batch(commands).unwrap(),
            "editor".into(),
        ),
        TrustedInstant::new(10),
    )?;
    let preview = lifecycle.preview_v2(
        snapshot.document_scope(),
        snapshot.document(),
        snapshot.revision(),
        &id,
        &"editor".into(),
        TrustedInstant::new(11),
    )?;
    let receipt = lifecycle.execute_v2(
        &id,
        None,
        &"editor".into(),
        &mut session.publication_authority(&mut Clock),
        TrustedInstant::new(12),
    )?;
    assert_eq!(
        receipt.evidence(),
        &V2ReceiptEvidence::Disclosed(preview.delta.clone())
    );
    Ok(preview.delta)
}

fn sole_field_document(field: &str, required: bool, blank: bool) -> Document {
    let mut doc = document();
    let schema = doc.schemas.get_mut("s").unwrap();
    schema.fields.retain(|id, _| id.as_str() == field);
    schema.fields.get_mut(field).unwrap().required = required;
    for entity in doc.entities.values_mut() {
        entity.fields.retain(|id, _| id.as_str() == field);
    }
    if blank {
        doc.entities.get_mut("b").unwrap().fields.remove(field);
    }
    doc
}

fn field_removal_pair(doc: &Document, field: &str) -> (SemanticCommand, SemanticCommand) {
    let definition = doc
        .schemas
        .get("s")
        .unwrap()
        .fields
        .get(field)
        .unwrap()
        .clone();
    let values: BTreeMap<_, _> = doc
        .entities
        .values()
        .filter_map(|entity| {
            entity
                .fields
                .get(field)
                .map(|value| (entity.id.clone(), value.clone()))
        })
        .collect();
    (
        SemanticCommand::RemoveSchemaField {
            schema: "s".into(),
            field: definition.clone(),
            values: values.clone(),
        },
        SemanticCommand::AppendSchemaField {
            schema: "s".into(),
            field: definition,
            values,
        },
    )
}

fn round_trip(
    lifecycle: &mut PatchLifecycle,
    session: &mut ResidentWorkspaceSession,
    forward: Vec<SemanticCommand>,
    inverse: Vec<SemanticCommand>,
) {
    let before = session.export_snapshot().into_document();
    let revision = session.revision().clone();
    apply(lifecycle, session, forward.clone()).unwrap();
    let after = session.export_snapshot().into_document();
    assert_ne!(before, after);
    assert_ne!(session.revision(), &revision);
    let forward_revision = session.revision().clone();
    apply(lifecycle, session, inverse.clone()).unwrap();
    assert_eq!(session.export_snapshot().document(), &before);
    assert_ne!(session.revision(), &revision);
    assert_ne!(session.revision(), &forward_revision);
    apply(lifecycle, session, forward).unwrap();
    assert_eq!(session.export_snapshot().document(), &after);
    apply(lifecycle, session, inverse).unwrap();
    assert_eq!(session.export_snapshot().document(), &before);
}

#[test]
fn sole_optional_field_removal_has_an_exact_sparse_append_inverse() {
    for field in ["label", "amount"] {
        let base = sole_field_document(field, false, true);
        let (remove, append) = field_removal_pair(&base, field);
        let mut session = ResidentWorkspaceSession::new("scope".into(), base.clone());
        let mut lifecycle = lifecycle();
        apply(&mut lifecycle, &mut session, vec![remove.clone()]).unwrap();
        let empty = session.export_snapshot();
        assert!(empty.document().schemas.get("s").unwrap().fields.is_empty());
        assert!(
            empty
                .document()
                .entities
                .values()
                .all(|e| e.fields.is_empty())
        );
        apply(&mut lifecycle, &mut session, vec![append.clone()]).unwrap();
        assert_eq!(session.export_snapshot().document(), &base);
        round_trip(&mut lifecycle, &mut session, vec![remove], vec![append]);
        // A fully blank entity also restores through ordinary append admission.
        let entity = base.entities.get("b").unwrap().clone();
        assert!(entity.fields.is_empty());
        round_trip(
            &mut lifecycle,
            &mut session,
            vec![SemanticCommand::RemoveEntity {
                entity: entity.id.clone(),
            }],
            vec![SemanticCommand::AppendEntity { entity }],
        );
    }
}

#[test]
fn sole_required_field_removal_keeps_its_existing_append_inverse() {
    let base = sole_field_document("label", true, false);
    let (remove, append) = field_removal_pair(&base, "label");
    let mut session = ResidentWorkspaceSession::new("scope".into(), base);
    round_trip(&mut lifecycle(), &mut session, vec![remove], vec![append]);
}

fn non_native_optional_document(kind: FieldType, value: &Value, sparse: bool) -> Document {
    let mut base = document();
    for field in base.schemas.get_mut("s").unwrap().fields.values_mut() {
        field.required = false;
    }
    base.schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .field_type = kind;
    for entity in base.entities.values_mut() {
        entity.fields.insert("amount".into(), value.clone());
    }
    if sparse {
        base.entities.get_mut("b").unwrap().fields.clear();
    }
    assert!(validation_report(&base).is_valid());
    base
}

fn assert_optional_field_type_refusal(
    result: Result<CanonicalSemanticDelta, PatchLifecycleError>,
    target: &str,
) {
    let Err(PatchLifecycleError::CommandRejected { source }) = result else {
        panic!("expected optional field admission refusal: {result:?}")
    };
    assert!(
        matches!(source.as_ref(), WorkspaceError::TypeMismatch { field }
            if field == &FieldRef::new("schema", target)),
        "unexpected refusal: {source:?}"
    );
}

#[test]
fn non_native_optional_removal_refuses_like_its_append_inverse_without_publication() {
    for (kind, value) in [
        (FieldType::Boolean, Value::Boolean(false)),
        (FieldType::Date, Value::Date("2026-10-20".parse().unwrap())),
    ] {
        for sparse in [false, true] {
            let base = non_native_optional_document(kind.clone(), &value, sparse);
            for target in ["label", "amount"] {
                let (remove, append) = field_removal_pair(&base, target);
                let mut restoration_base = base.clone();
                restoration_base
                    .schemas
                    .get_mut("s")
                    .unwrap()
                    .fields
                    .remove(target);
                for entity in restoration_base.entities.values_mut() {
                    entity.fields.remove(target);
                }
                let mut inverse_session =
                    ResidentWorkspaceSession::new("scope".into(), restoration_base.clone());
                assert_optional_field_type_refusal(
                    apply(&mut lifecycle(), &mut inverse_session, vec![append]),
                    target,
                );
                assert_eq!(
                    inverse_session.export_snapshot().document(),
                    &restoration_base
                );
                assert_eq!(inverse_session.revision().as_str(), "resident/0");

                let mut session = ResidentWorkspaceSession::new("scope".into(), base.clone());
                assert_optional_field_type_refusal(
                    apply(&mut lifecycle(), &mut session, vec![remove.clone()]),
                    target,
                );
                assert_eq!(session.export_snapshot().document(), &base);
                assert_eq!(session.revision().as_str(), "resident/0");
                // A valid prefix must not publish when the removal is refused.
                let prefix = SemanticCommand::SetSchemaFieldDisplayName {
                    schema: "s".into(),
                    field: if target == "label" { "amount" } else { "label" }.into(),
                    display_name: Some("Changed".into()),
                };
                assert_optional_field_type_refusal(
                    apply(&mut lifecycle(), &mut session, vec![prefix, remove]),
                    target,
                );
                assert_eq!(session.export_snapshot().document(), &base);
                assert_eq!(session.revision().as_str(), "resident/0");
            }
        }
    }
}

#[test]
fn required_field_removal_from_non_native_schema_keeps_its_append_inverse() {
    for (kind, value) in [
        (FieldType::Boolean, Value::Boolean(false)),
        (FieldType::Date, Value::Date("2026-10-20".parse().unwrap())),
    ] {
        let mut base = non_native_optional_document(kind, &value, false);
        base.schemas
            .get_mut("s")
            .unwrap()
            .fields
            .get_mut("label")
            .unwrap()
            .required = true;
        let (remove, append) = field_removal_pair(&base, "label");
        let mut session = ResidentWorkspaceSession::new("scope".into(), base);
        round_trip(&mut lifecycle(), &mut session, vec![remove], vec![append]);
    }
}

#[test]
fn optional_append_still_checks_values_and_the_complete_resulting_shape() {
    let base = sole_field_document("label", false, true);
    let (remove, mut append) = field_removal_pair(&base, "label");
    let mut session = ResidentWorkspaceSession::new("scope".into(), base);
    apply(&mut lifecycle(), &mut session, vec![remove]).unwrap();
    let empty = session.export_snapshot().into_document();
    let SemanticCommand::AppendSchemaField { values, .. } = &mut append else {
        unreachable!()
    };
    values.insert("a".into(), number(1.0));
    assert!(apply(&mut lifecycle(), &mut session, vec![append.clone()]).is_err());
    assert_eq!(session.export_snapshot().document(), &empty);
    assert_eq!(session.revision().as_str(), "resident/1");

    let mut imported = sole_field_document("amount", false, false);
    imported
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .field_type = FieldType::Boolean;
    for entity in imported.entities.values_mut() {
        entity.fields.insert("amount".into(), Value::Boolean(false));
    }
    let SemanticCommand::AppendSchemaField { values, .. } = &mut append else {
        unreachable!()
    };
    values.clear();
    let mut session = ResidentWorkspaceSession::new("scope".into(), imported.clone());
    assert!(apply(&mut lifecycle(), &mut session, vec![append]).is_err());
    assert_eq!(session.export_snapshot().document(), &imported);
    assert_eq!(session.revision().as_str(), "resident/0");
}

#[test]
fn optional_inverse_pairs_restore_blanks_and_requiredness_exactly() {
    let base = document();
    let mut lifecycle = lifecycle();
    let mut session = ResidentWorkspaceSession::new("scope".into(), base);
    round_trip(
        &mut lifecycle,
        &mut session,
        vec![required(false)],
        vec![required(true)],
    );
    apply(&mut lifecycle, &mut session, vec![required(false)]).unwrap();
    round_trip(
        &mut lifecycle,
        &mut session,
        vec![required(true)],
        vec![required(false)],
    );
    let set = SemanticCommand::set_field_value(FieldRef::new("a", "amount"), number(2.0));
    round_trip(
        &mut lifecycle,
        &mut session,
        vec![clear()],
        vec![set.clone()],
    );
    apply(&mut lifecycle, &mut session, vec![clear()]).unwrap();
    round_trip(&mut lifecycle, &mut session, vec![set], vec![clear()]);
    let sparse = session.export_snapshot().into_document();
    let (remove, append) = field_removal_pair(&sparse, "amount");
    round_trip(&mut lifecycle, &mut session, vec![remove], vec![append]);
    for id in ["a", "b"] {
        let entity = sparse.entities.get(id).unwrap().clone();
        round_trip(
            &mut lifecycle,
            &mut session,
            vec![SemanticCommand::RemoveEntity {
                entity: entity.id.clone(),
            }],
            vec![SemanticCommand::AppendEntity { entity }],
        );
    }
}

#[test]
fn sole_field_remove_append_round_trips_after_relaxation_and_clear() {
    let base = sole_field_document("amount", true, false);
    let mut session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mut lifecycle = lifecycle();
    apply(&mut lifecycle, &mut session, vec![required(false), clear()]).unwrap();
    let sparse = session.export_snapshot().into_document();
    let (remove, append) = field_removal_pair(&sparse, "amount");
    round_trip(&mut lifecycle, &mut session, vec![remove], vec![append]);
    apply(
        &mut lifecycle,
        &mut session,
        vec![
            SemanticCommand::set_field_value(FieldRef::new("a", "amount"), number(2.0)),
            required(true),
        ],
    )
    .unwrap();
    assert_eq!(session.export_snapshot().document(), &base);
}

#[test]
fn requiredness_and_clear_produce_exact_v2_facts_and_forward_inverses() {
    let base = document();
    let mut session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mut lifecycle = lifecycle();
    let delta = apply(&mut lifecycle, &mut session, vec![required(false)]).unwrap();
    assert_eq!(
        delta.facts(),
        &[Fact::SchemaFieldRequirednessChanged {
            schema: "s".into(),
            field: "amount".into(),
            before: true,
            after: false
        }]
    );
    let optional = session.export_snapshot().into_document();
    let delta = apply(&mut lifecycle, &mut session, vec![clear()]).unwrap();
    assert_eq!(
        delta.facts(),
        &[Fact::EntityFieldValueCleared {
            entity: "a".into(),
            schema: "s".into(),
            field: "amount".into(),
            before: number(2.0)
        }]
    );
    let blank = session.export_snapshot().into_document();
    apply(
        &mut lifecycle,
        &mut session,
        vec![SemanticCommand::set_field_value(
            FieldRef::new("a", "amount"),
            number(2.0),
        )],
    )
    .unwrap();
    assert_eq!(session.export_snapshot().document(), &optional);
    apply(&mut lifecycle, &mut session, vec![required(true)]).unwrap();
    assert_eq!(session.export_snapshot().document(), &base);
    apply(&mut lifecycle, &mut session, vec![required(false), clear()]).unwrap();
    assert_eq!(session.export_snapshot().document(), &blank);
    let before = session.revision().clone();
    assert!(matches!(
        apply(&mut lifecycle, &mut session, vec![clear()]),
        Err(PatchLifecycleError::NoChange)
    ));
    assert_eq!(session.revision(), &before);
}

#[test]
fn strengthening_and_formula_consumer_clear_refuse_without_publication() {
    let mut optional = document();
    optional
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .required = false;
    optional
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .remove("amount");
    let mut session = ResidentWorkspaceSession::new("scope".into(), optional.clone());
    assert!(apply(&mut lifecycle(), &mut session, vec![required(true)]).is_err());
    assert_eq!(session.export_snapshot().document(), &optional);
    assert_eq!(session.revision().as_str(), "resident/0");

    let mut formula = document();
    formula
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .required = false;
    formula.entities.get_mut("b").unwrap().fields.insert(
        "amount".into(),
        Value::Formula(Expression::Reference(FieldRef::new("a", "amount"))),
    );
    let mut session = ResidentWorkspaceSession::new("scope".into(), formula.clone());
    assert!(
        apply(
            &mut lifecycle(),
            &mut session,
            vec![
                SemanticCommand::set_field_value(
                    FieldRef::new("a", "label"),
                    Value::Text("changed".into())
                ),
                clear()
            ]
        )
        .is_err()
    );
    assert_eq!(session.export_snapshot().document(), &formula);
}

#[test]
fn requiredness_keeps_keyed_binding_and_blank_makes_whole_sum_unavailable() {
    let mut base = document();
    let definition = KeyedGroupedSumDefinition {
        id: "sum".into(),
        orders: KeyedGroupedSumOrdersBinding {
            schema: "s".into(),
            lookup_key_field: "label".into(),
            quantity_field: "amount".into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: "s".into(),
            key_field: "label".into(),
            category_field: "label".into(),
            price_field: "amount".into(),
        },
    };
    base.keyed_grouped_sum_definitions
        .insert("sum".into(), definition.clone());
    let original = evaluate_keyed_grouped_sum(&base, &"sum".into());
    assert!(matches!(original, KeyedGroupedSumOutcome::Complete(_)));
    let mut session = ResidentWorkspaceSession::new("scope".into(), base);
    let mut lifecycle = lifecycle();
    apply(&mut lifecycle, &mut session, vec![required(false)]).unwrap();
    assert_eq!(
        evaluate_keyed_grouped_sum(session.export_snapshot().document(), &"sum".into()),
        original
    );
    apply(&mut lifecycle, &mut session, vec![clear()]).unwrap();
    let snapshot = session.export_snapshot();
    assert_eq!(
        snapshot.document().keyed_grouped_sum_definitions[&"sum".into()],
        definition
    );
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) =
        evaluate_keyed_grouped_sum(snapshot.document(), &"sum".into())
    else {
        panic!("no partial groups are allowed")
    };
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == "keyed_grouped_sum.missing_input")
    );
}

#[test]
fn constraints_only_validate_present_values_and_clear_merge_stays_structural() {
    let mut base = document();
    let field = base
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap();
    field.required = false;
    field.constraint = FieldConstraint::NumberInclusiveRange {
        min: Number::new(1.0).unwrap(),
        max: Number::new(3.0).unwrap(),
    };
    let mut cleared = base.clone();
    cleared
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .remove("amount");
    assert!(validation_report(&cleared).is_valid());
    let mut changed = base.clone();
    changed
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("amount".into(), number(3.0));
    let WorkspaceMergeOutcomeV2::Conflicted(conflicts) =
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &cleared, &changed).unwrap()
    else {
        panic!("clear versus concurrent set must conflict")
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind(), ConflictKind::DeleteModify);
    assert_eq!(conflicts[0].left(), &ConflictFactV2::Absent);
    let mut strengthened = base.clone();
    strengthened
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .required = true;
    assert!(matches!(
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &strengthened, &cleared),
        Err(WorkspaceError::InvalidDocument {
            role: ValidationRole::MergeCandidate,
            ..
        })
    ));
    changed
        .entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("amount".into(), number(0.0));
    assert!(!validation_report(&changed).is_valid());
}

#[test]
fn requiredness_refuses_absent_targets_unsupported_kinds_and_missing_authority() {
    let base = document();
    for command in [
        SemanticCommand::SetSchemaFieldRequiredness {
            schema: "missing".into(),
            field: "amount".into(),
            required: false,
        },
        SemanticCommand::SetSchemaFieldRequiredness {
            schema: "s".into(),
            field: "missing".into(),
            required: false,
        },
    ] {
        let mut session = ResidentWorkspaceSession::new("scope".into(), base.clone());
        assert!(apply(&mut lifecycle(), &mut session, vec![command]).is_err());
        assert_eq!(session.export_snapshot().document(), &base);
    }
    let mut unsupported = base.clone();
    unsupported
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("amount")
        .unwrap()
        .field_type = FieldType::Boolean;
    for entity in unsupported.entities.values_mut() {
        entity.fields.insert("amount".into(), Value::Boolean(false));
    }
    let mut session = ResidentWorkspaceSession::new("scope".into(), unsupported.clone());
    assert!(apply(&mut lifecycle(), &mut session, vec![required(false)]).is_err());
    assert_eq!(session.export_snapshot().document(), &unsupported);

    let mut lifecycle = lifecycle();
    lifecycle.revoke_grant(&"grant".into()).unwrap();
    let mut session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    assert!(apply(&mut lifecycle, &mut session, vec![required(false)]).is_err());
    assert_eq!(session.export_snapshot().document(), &base);
}
