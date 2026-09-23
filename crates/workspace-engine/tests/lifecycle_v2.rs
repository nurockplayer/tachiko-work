mod common;

use common::game_balance_document;
use tachiko_diff_engine::{CanonicalDeltaError, CanonicalDirectFact};
use tachiko_workspace_engine::{
    Document, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType,
    KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding,
    KeyedGroupedSumProductsBinding, Number, Schema, Value, WorkspaceError,
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion,
        DisclosureRequirement, DocumentScopeId, Grant, GrantId, GrantRequirement, MutationClass,
        OperationFamily, PatchLifecycle, PatchLifecycleError, PolicyMeaningId, PrincipalId,
        PrincipalKind, ProposalId, ProposalRequest, ScopedSemanticSubject, SemanticApiContract,
        SemanticCommand, SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant, V2ReceiptEvidence,
    },
};

fn revision(value: &str) -> SemanticRevision {
    SemanticRevision::from(value)
}

fn scope() -> DocumentScopeId {
    DocumentScopeId::from("game-occurrence")
}

fn subject(semantic_scope: SemanticScope) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(scope(), "game".into(), semantic_scope)
}

fn lifecycle(v2: bool) -> PatchLifecycle {
    let args = (
        AuthorizationDomainId::from("domain"),
        scope(),
        "game".into(),
        SemanticApiContract::from("semantic-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("meaning-v1"),
    );
    let mut lifecycle = if v2 {
        PatchLifecycle::new_v2(args.0, args.1, args.2, args.3, args.4, args.5)
    } else {
        PatchLifecycle::new(args.0, args.1, args.2, args.3, args.4, args.5)
    };
    for (principal, kind) in [
        ("editor", PrincipalKind::Human),
        ("runner", PrincipalKind::Human),
        ("authority", PrincipalKind::Human),
    ] {
        lifecycle
            .register_principal(principal.into(), kind)
            .unwrap();
    }
    lifecycle
}

fn grant(
    lifecycle: &mut PatchLifecycle,
    grant_id: &str,
    principal: &str,
    requirements: Vec<GrantRequirement>,
) {
    lifecycle
        .provision_grant(Grant::new(
            grant_id.into(),
            "authority".into(),
            principal.into(),
            requirements,
            None,
        ))
        .unwrap();
}

fn query(family: OperationFamily) -> GrantRequirement {
    GrantRequirement::query(family, subject(SemanticScope::Document))
}

fn write(
    action: AuthorizationAction,
    family: OperationFamily,
    class: MutationClass,
    semantic_scope: SemanticScope,
) -> GrantRequirement {
    GrantRequirement::mutation(action, family, class, subject(semantic_scope)).unwrap()
}

fn propose_body(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    id: &str,
    body: SemanticPatchBody,
) -> Result<ProposalId, PatchLifecycleError> {
    propose_body_at(lifecycle, document, "r1", id, body)
}

fn propose_body_at(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    base_revision: &str,
    id: &str,
    body: SemanticPatchBody,
) -> Result<ProposalId, PatchLifecycleError> {
    let id = ProposalId::from(id);
    lifecycle.propose(
        &scope(),
        document,
        &revision(base_revision),
        ProposalRequest::new(id.clone(), revision(base_revision), body, "editor".into()),
        TrustedInstant::new(10),
    )?;
    Ok(id)
}

struct Host {
    document: tachiko_workspace_engine::Document,
    revision: SemanticRevision,
    publications: usize,
}

impl SemanticPublicationAuthority for Host {
    fn current_snapshot(
        &self,
    ) -> (
        DocumentScopeId,
        tachiko_workspace_engine::Document,
        SemanticRevision,
    ) {
        (scope(), self.document.clone(), self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: tachiko_workspace_engine::Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<
        (
            DocumentScopeId,
            tachiko_workspace_engine::Document,
            SemanticRevision,
            A,
        ),
        SemanticPublicationError,
    > {
        self.publications += 1;
        if expected_scope != &scope() || expected_revision != &self.revision {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(11))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        if candidate == self.document {
            return Err(SemanticPublicationError::NoChange);
        }
        self.document = candidate;
        self.revision = revision(&format!("r{}", self.publications + 1));
        Ok((
            scope(),
            self.document.clone(),
            self.revision.clone(),
            authorization,
        ))
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Keep the constrained formula scenario reviewable in one test.
fn v2_value_edit_on_constrained_base_keeps_formula_impact_and_query_disclosure() {
    let mut document = game_balance_document("game", "Game");
    document
        .schemas
        .get_mut("weapons")
        .unwrap()
        .fields
        .get_mut("damage")
        .unwrap()
        .constraint = FieldConstraint::NumberInclusiveRange {
        min: Number::new(0.0).unwrap(),
        max: Number::new(50.0).unwrap(),
    };
    let mut lifecycle = PatchLifecycle::new_v2(
        AuthorizationDomainId::from("domain"),
        scope(),
        "game".into(),
        SemanticApiContract::from("semantic-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        "meaning-v1".into(),
    );
    lifecycle
        .register_principal("editor".into(), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal("authority".into(), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from("editor-grant"),
            PrincipalId::from("authority"),
            PrincipalId::from("editor"),
            vec![
                GrantRequirement::query(
                    OperationFamily::SetFieldValue,
                    subject(SemanticScope::Document),
                ),
                GrantRequirement::mutation(
                    AuthorizationAction::Propose,
                    OperationFamily::SetFieldValue,
                    MutationClass::Value,
                    subject(SemanticScope::Document),
                )
                .unwrap(),
                GrantRequirement::mutation(
                    AuthorizationAction::Execute,
                    OperationFamily::SetFieldValue,
                    MutationClass::Value,
                    subject(SemanticScope::Document),
                )
                .unwrap(),
            ],
            None,
        ))
        .unwrap();

    let proposal_id = ProposalId::from("edit-damage");
    lifecycle
        .propose(
            &scope(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal_id.clone(),
                revision("r1"),
                SemanticPatchBody::command(SemanticCommand::set_field_value(
                    FieldRef::new("iron_sword", "damage"),
                    Value::Number(Number::new(45.0).unwrap()),
                )),
                "editor".into(),
            ),
            TrustedInstant::new(10),
        )
        .unwrap();

    let preview = lifecycle
        .preview_v2(
            &scope(),
            &document,
            &revision("r1"),
            &proposal_id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    assert!(matches!(
        preview.delta.facts(),
        [CanonicalDirectFact::EntityFieldValueChanged { before, after, .. }]
            if before == &Value::Number(Number::new(36.0).unwrap())
                && after == &Value::Number(Number::new(45.0).unwrap())
    ));
    assert_eq!(preview.formula_impacts.len(), 1);
    assert_eq!(
        preview.formula_impacts[0].field,
        FieldRef::new("iron_sword", "dps")
    );
    assert_eq!(
        preview.formula_impacts[0].causes,
        [FieldRef::new("iron_sword", "damage")]
    );
    let query_disclosures = &preview.authorization_footprint.disclosure_requirements;
    for field in ["damage", "dps"] {
        assert!(query_disclosures.contains(&DisclosureRequirement {
            family: OperationFamily::SetFieldValue,
            scope: subject(SemanticScope::EntityField {
                entity: "iron_sword".into(),
                schema: "weapons".into(),
                field: field.into(),
            }),
        }));
    }

    let mut host = Host {
        document,
        revision: revision("r1"),
        publications: 0,
    };
    let receipt = lifecycle
        .execute_v2(
            &proposal_id,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(receipt.formula_impacts, preview.formula_impacts);
    assert_eq!(receipt.delta().unwrap(), &preview.delta);
    assert_eq!(host.revision, revision("r2"));
    assert_eq!(
        host.document.entities["iron_sword"].fields["damage"],
        Value::Number(Number::new(45.0).unwrap())
    );
}

#[test]
fn execute_without_query_returns_redacted_receipt_but_keeps_full_trusted_evidence() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle(true);
    let damage_scope = SemanticScope::SchemaField {
        schema: "weapons".into(),
        field: "damage".into(),
    };
    grant(
        &mut lifecycle,
        "editor-grant",
        "editor",
        vec![
            GrantRequirement::query(
                OperationFamily::SchemaFieldMutation,
                subject(damage_scope.clone()),
            ),
            write(
                AuthorizationAction::Propose,
                OperationFamily::SchemaFieldMutation,
                MutationClass::Schema,
                damage_scope.clone(),
            ),
        ],
    );
    grant(
        &mut lifecycle,
        "runner-grant",
        "runner",
        vec![write(
            AuthorizationAction::Execute,
            OperationFamily::SchemaFieldMutation,
            MutationClass::Schema,
            damage_scope,
        )],
    );
    let proposal_id = propose_body(
        &mut lifecycle,
        &document,
        "redacted",
        SemanticPatchBody::command(SemanticCommand::SetFieldConstraint {
            schema: "weapons".into(),
            field: "damage".into(),
            constraint: FieldConstraint::NumberInclusiveRange {
                min: Number::new(0.0).unwrap(),
                max: Number::new(50.0).unwrap(),
            },
        }),
    )
    .unwrap();
    let mut host = Host {
        document,
        revision: revision("r1"),
        publications: 0,
    };

    let receipt = lifecycle
        .execute_v2(
            &proposal_id,
            None,
            &"runner".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();

    assert!(matches!(receipt.evidence(), V2ReceiptEvidence::Redacted));
    assert!(matches!(
        receipt.delta(),
        Err(PatchLifecycleError::DisclosureDenied)
    ));
    assert_eq!(host.publications, 1);
    let trusted = &lifecycle.execution_receipts_v2()[0];
    assert!(matches!(
        trusted.evidence(),
        V2ReceiptEvidence::Disclosed(delta) if delta.facts().len() == 1
    ));
    assert_eq!(trusted.delta().unwrap().facts().len(), 1);
}

#[test]
#[allow(clippy::too_many_lines)] // Pins both profile crossings against publication.
fn wrong_profile_preview_and_execute_refuse_without_publication_or_fake_evidence() {
    let document = game_balance_document("game", "Game");
    let mut v2 = lifecycle(true);
    grant(
        &mut v2,
        "v2-editor",
        "editor",
        vec![
            query(OperationFamily::SchemaFieldMutation),
            write(
                AuthorizationAction::Propose,
                OperationFamily::SchemaFieldMutation,
                MutationClass::Schema,
                SemanticScope::Schema("weapons".into()),
            ),
            write(
                AuthorizationAction::Execute,
                OperationFamily::SchemaFieldMutation,
                MutationClass::Schema,
                SemanticScope::Schema("weapons".into()),
            ),
        ],
    );
    let v2_id = propose_body(
        &mut v2,
        &document,
        "v2-proposal",
        SemanticPatchBody::command(SemanticCommand::SetFieldConstraint {
            schema: "weapons".into(),
            field: "damage".into(),
            constraint: FieldConstraint::NumberInclusiveRange {
                min: Number::new(0.0).unwrap(),
                max: Number::new(50.0).unwrap(),
            },
        }),
    )
    .unwrap();
    let mut v2_host = Host {
        document: document.clone(),
        revision: revision("r1"),
        publications: 0,
    };
    assert!(matches!(
        v2.preview(
            &scope(),
            &document,
            &revision("r1"),
            &v2_id,
            &"editor".into(),
            TrustedInstant::new(10),
        ),
        Err(PatchLifecycleError::EvidenceProfileMismatch)
    ));
    assert!(matches!(
        v2.execute(
            &v2_id,
            None,
            &"editor".into(),
            &mut v2_host,
            TrustedInstant::new(10),
        ),
        Err(PatchLifecycleError::EvidenceProfileMismatch)
    ));
    assert_eq!(v2_host.publications, 0);
    assert!(v2.execution_receipts().is_empty());
    assert!(v2.execution_receipts_v2().is_empty());

    let mut v1 = lifecycle(false);
    grant(
        &mut v1,
        "v1-editor",
        "editor",
        vec![
            query(OperationFamily::SetFieldValue),
            write(
                AuthorizationAction::Propose,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                SemanticScope::Document,
            ),
            write(
                AuthorizationAction::Execute,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                SemanticScope::Document,
            ),
        ],
    );
    let v1_id = propose_body(
        &mut v1,
        &document,
        "v1-proposal",
        SemanticPatchBody::command(SemanticCommand::set_field_value(
            FieldRef::new("iron_sword", "damage"),
            Value::Number(Number::new(45.0).unwrap()),
        )),
    )
    .unwrap();
    let mut v1_host = Host {
        document: document.clone(),
        revision: revision("r1"),
        publications: 0,
    };
    assert!(matches!(
        v1.preview_v2(
            &scope(),
            &document,
            &revision("r1"),
            &v1_id,
            &"editor".into(),
            TrustedInstant::new(10),
        ),
        Err(PatchLifecycleError::EvidenceProfileMismatch)
    ));
    assert!(matches!(
        v1.execute_v2(
            &v1_id,
            None,
            &"editor".into(),
            &mut v1_host,
            TrustedInstant::new(10),
        ),
        Err(PatchLifecycleError::EvidenceProfileMismatch)
    ));
    assert_eq!(v1_host.publications, 0);
    assert!(v1.execution_receipts().is_empty());
    assert!(v1.execution_receipts_v2().is_empty());
}

#[test]
fn constraint_write_requires_schema_class_even_when_other_classes_are_granted() {
    let document = game_balance_document("game", "Game");
    for granted_class in [
        MutationClass::Value,
        MutationClass::Formula,
        MutationClass::Structure,
    ] {
        let mut lifecycle = lifecycle(true);
        grant(
            &mut lifecycle,
            "non-schema-editor",
            "editor",
            vec![
                query(OperationFamily::SchemaFieldMutation),
                write(
                    AuthorizationAction::Propose,
                    OperationFamily::SchemaFieldMutation,
                    granted_class,
                    SemanticScope::Schema("weapons".into()),
                ),
            ],
        );
        let error = propose_body(
            &mut lifecycle,
            &document,
            "wrong-mutation-class",
            SemanticPatchBody::command(SemanticCommand::SetFieldConstraint {
                schema: "weapons".into(),
                field: "damage".into(),
                constraint: FieldConstraint::NumberInclusiveRange {
                    min: Number::new(0.0).unwrap(),
                    max: Number::new(50.0).unwrap(),
                },
            }),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            PatchLifecycleError::InsufficientCapability {
                action: AuthorizationAction::Propose
            }
        ));
    }
}

fn keyed_definition_document() -> Document {
    fn schema(id: &str, fields: &[(&str, FieldType)]) -> Schema {
        Schema {
            id: id.into(),
            key: id.into(),
            fields: fields
                .iter()
                .map(|(field_id, field_type)| {
                    (
                        (*field_id).into(),
                        FieldDefinition {
                            id: (*field_id).into(),
                            key: (*field_id).into(),
                            field_type: field_type.clone(),
                            required: true,
                            constraint: FieldConstraint::None,
                        },
                    )
                })
                .collect(),
        }
    }

    let mut document = Document::empty("game", "Grouped summary");
    document.schemas.insert(
        "orders".into(),
        schema(
            "orders",
            &[
                ("product-key", FieldType::Text),
                ("quantity", FieldType::Number),
            ],
        ),
    );
    document.schemas.insert(
        "products".into(),
        schema(
            "products",
            &[
                ("product-key", FieldType::Text),
                ("category", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
    );
    let id = KeyedGroupedSumDefinitionId::from("summary");
    document.keyed_grouped_sum_definitions.insert(
        id.clone(),
        KeyedGroupedSumDefinition {
            id,
            orders: KeyedGroupedSumOrdersBinding {
                schema: "orders".into(),
                lookup_key_field: "product-key".into(),
                quantity_field: "quantity".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "products".into(),
                key_field: "product-key".into(),
                category_field: "category".into(),
                price_field: "price".into(),
            },
        },
    );
    document
}

#[test]
fn v2_keyed_definition_change_refuses_with_canonical_delta_error() {
    let document = keyed_definition_document();
    let mut lifecycle = lifecycle(true);
    grant(
        &mut lifecycle,
        "summary-editor",
        "editor",
        vec![
            query(OperationFamily::KeyedGroupedSumDefinition),
            write(
                AuthorizationAction::Propose,
                OperationFamily::KeyedGroupedSumDefinition,
                MutationClass::Structure,
                SemanticScope::Document,
            ),
            write(
                AuthorizationAction::Propose,
                OperationFamily::KeyedGroupedSumDefinition,
                MutationClass::Destructive,
                SemanticScope::Document,
            ),
        ],
    );
    let error = propose_body(
        &mut lifecycle,
        &document,
        "remove-summary",
        SemanticPatchBody::command(SemanticCommand::RemoveKeyedGroupedSumDefinition {
            definition: "summary".into(),
        }),
    )
    .unwrap_err();

    let PatchLifecycleError::CommandRejected { source } = error else {
        panic!("expected the typed canonical-delta refusal");
    };
    let WorkspaceError::CanonicalDelta(source) = *source else {
        panic!("keyed-definition change must be refused by canonical delta");
    };
    assert!(matches!(
        *source,
        CanonicalDeltaError::UnsupportedKeyedGroupedSumDefinitionChange
    ));
    assert!(lifecycle.execution_receipts_v2().is_empty());
}

#[test]
fn global_formula_calculation_failure_blocks_constraint_tightening() {
    let mut document = game_balance_document("game", "Game");
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(
            "attack_interval".into(),
            Value::Number(Number::new(0.0).unwrap()),
        );
    let original = document.clone();
    let mut lifecycle = lifecycle(true);
    grant(
        &mut lifecycle,
        "constraint-editor",
        "editor",
        vec![
            query(OperationFamily::SchemaFieldMutation),
            write(
                AuthorizationAction::Propose,
                OperationFamily::SchemaFieldMutation,
                MutationClass::Schema,
                SemanticScope::Schema("weapons".into()),
            ),
        ],
    );

    let error = propose_body(
        &mut lifecycle,
        &document,
        "tighten-with-invalid-calculation",
        SemanticPatchBody::command(SemanticCommand::SetFieldConstraint {
            schema: "weapons".into(),
            field: "damage".into(),
            constraint: FieldConstraint::NumberInclusiveRange {
                min: Number::new(0.0).unwrap(),
                max: Number::new(50.0).unwrap(),
            },
        }),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::ValidationFailed { .. }
    ));
    assert_eq!(document, original);
    assert!(lifecycle.execution_receipts_v2().is_empty());
}

#[test]
#[allow(clippy::too_many_lines)] // Exercise both supported command families through the same constrained state.
fn v2_entity_and_schema_commands_publish_complete_facts_on_constrained_base() {
    let mut document = game_balance_document("game", "Game");
    document
        .schemas
        .get_mut("weapons")
        .unwrap()
        .fields
        .get_mut("damage")
        .unwrap()
        .constraint = FieldConstraint::NumberInclusiveRange {
        min: Number::new(0.0).unwrap(),
        max: Number::new(50.0).unwrap(),
    };
    let mut lifecycle = lifecycle(true);
    let damage_scope = SemanticScope::SchemaField {
        schema: "weapons".into(),
        field: "damage".into(),
    };
    grant(
        &mut lifecycle,
        "ordinary-command-editor",
        "editor",
        vec![
            query(OperationFamily::AppendEntity),
            write(
                AuthorizationAction::Propose,
                OperationFamily::AppendEntity,
                MutationClass::Structure,
                SemanticScope::Schema("weapons".into()),
            ),
            write(
                AuthorizationAction::Propose,
                OperationFamily::AppendEntity,
                MutationClass::Formula,
                SemanticScope::Schema("weapons".into()),
            ),
            write(
                AuthorizationAction::Execute,
                OperationFamily::AppendEntity,
                MutationClass::Structure,
                SemanticScope::Schema("weapons".into()),
            ),
            write(
                AuthorizationAction::Execute,
                OperationFamily::AppendEntity,
                MutationClass::Formula,
                SemanticScope::Schema("weapons".into()),
            ),
            query(OperationFamily::SchemaFieldMutation),
            write(
                AuthorizationAction::Propose,
                OperationFamily::SchemaFieldMutation,
                MutationClass::Schema,
                damage_scope.clone(),
            ),
            write(
                AuthorizationAction::Execute,
                OperationFamily::SchemaFieldMutation,
                MutationClass::Schema,
                damage_scope,
            ),
        ],
    );
    let mut host = Host {
        document: document.clone(),
        revision: revision("r1"),
        publications: 0,
    };

    let mut appended = host.document.entities["iron_sword"].clone();
    appended.id = "steel_sword".into();
    appended.key = "steel_sword".into();
    appended.fields.insert(
        "dps".into(),
        Value::Formula(Expression::Reference(FieldRef::new(
            "steel_sword",
            "damage",
        ))),
    );
    let entity_id = propose_body_at(
        &mut lifecycle,
        &host.document,
        "r1",
        "append-steel-sword",
        SemanticPatchBody::command(SemanticCommand::AppendEntity { entity: appended }),
    )
    .unwrap();
    let entity_preview = lifecycle
        .preview_v2(
            &scope(),
            &host.document,
            &revision("r1"),
            &entity_id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    assert!(matches!(
        entity_preview.delta.facts(),
        [CanonicalDirectFact::EntityCreated { entity, definition }]
            if entity.as_str() == "steel_sword"
                && definition.schema.as_str() == "weapons"
                && definition.fields["damage"] == Value::Number(Number::new(36.0).unwrap())
    ));
    let entity_receipt = lifecycle
        .execute_v2(
            &entity_id,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(entity_receipt.delta().unwrap(), &entity_preview.delta);
    assert!(host.document.entities.contains_key("steel_sword"));
    assert_eq!(host.revision, revision("r2"));

    let schema_id = propose_body_at(
        &mut lifecycle,
        &host.document,
        "r2",
        "rename-damage-field",
        SemanticPatchBody::command(SemanticCommand::RenameSchemaField {
            schema: "weapons".into(),
            field: "damage".into(),
            key: "power".into(),
        }),
    )
    .unwrap();
    let schema_preview = lifecycle
        .preview_v2(
            &scope(),
            &host.document,
            &revision("r2"),
            &schema_id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    assert!(matches!(
        schema_preview.delta.facts(),
        [CanonicalDirectFact::SchemaFieldKeyChanged { schema, field, before, after }]
            if schema.as_str() == "weapons" && field.as_str() == "damage"
                && before.as_str() == "damage" && after.as_str() == "power"
    ));
    let schema_receipt = lifecycle
        .execute_v2(
            &schema_id,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(schema_receipt.delta().unwrap(), &schema_preview.delta);
    assert_eq!(host.revision, revision("r3"));
    assert_eq!(host.publications, 2);
    assert_eq!(
        host.document.schemas["weapons"].fields["damage"].constraint,
        FieldConstraint::NumberInclusiveRange {
            min: Number::new(0.0).unwrap(),
            max: Number::new(50.0).unwrap(),
        }
    );
    assert_eq!(
        host.document.schemas["weapons"].fields["damage"]
            .key
            .as_str(),
        "power"
    );
}
