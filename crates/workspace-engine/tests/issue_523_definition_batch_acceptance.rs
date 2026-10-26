//! #523 Steward acceptance: truthful V1 evidence when saved grouped-sum
//! definitions change.
//!
//! Decision: <https://github.com/nurockplayer/tachiko-work/issues/523>
//! Definition-only batches keep their supported lifecycle. A V1 batch that
//! changes definitions together with any other command must be refused,
//! because V1 evidence cannot represent the definition change and the early
//! path would otherwise omit ordinary evidence and per-change admission.
use std::collections::BTreeMap;
use tachiko_semantic_core::*;
use tachiko_workspace_engine::{
    patch_lifecycle::*,
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
};
#[allow(clippy::too_many_lines)] // One literal two-schema fixture.
fn keyed_grouped_sum_document() -> Document {
    let mut document = Document::empty("document", "Grouped summary");
    let orders = SchemaId::from("orders");
    let products = SchemaId::from("products");
    document.schemas.insert(
        orders.clone(),
        Schema {
            id: orders.clone(),
            key: SchemaKey::from("orders"),
            display_name: None,
            fields: BTreeMap::from([
                (
                    FieldId::from("order-key"),
                    FieldDefinition {
                        id: FieldId::from("order-key"),
                        key: FieldKey::from("product_key"),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Text,
                        required: true,
                        constraint: tachiko_semantic_core::FieldConstraint::None,
                    },
                ),
                (
                    FieldId::from("quantity"),
                    FieldDefinition {
                        id: FieldId::from("quantity"),
                        key: FieldKey::from("quantity"),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Number,
                        required: true,
                        constraint: tachiko_semantic_core::FieldConstraint::None,
                    },
                ),
            ]),
        },
    );
    document.schemas.insert(
        products.clone(),
        Schema {
            id: products.clone(),
            key: SchemaKey::from("products"),
            display_name: None,
            fields: BTreeMap::from([
                (
                    FieldId::from("product-key"),
                    FieldDefinition {
                        id: FieldId::from("product-key"),
                        key: FieldKey::from("product_key"),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Text,
                        required: true,
                        constraint: tachiko_semantic_core::FieldConstraint::None,
                    },
                ),
                (
                    FieldId::from("category"),
                    FieldDefinition {
                        id: FieldId::from("category"),
                        key: FieldKey::from("category"),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Text,
                        required: true,
                        constraint: tachiko_semantic_core::FieldConstraint::None,
                    },
                ),
                (
                    FieldId::from("price"),
                    FieldDefinition {
                        id: FieldId::from("price"),
                        key: FieldKey::from("price"),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Number,
                        required: true,
                        constraint: tachiko_semantic_core::FieldConstraint::None,
                    },
                ),
            ]),
        },
    );
    document.entities.insert(
        EntityId::from("order"),
        Entity {
            id: EntityId::from("order"),
            key: EntityKey::from("order"),
            schema: orders.clone(),
            fields: BTreeMap::from([
                (FieldId::from("order-key"), Value::Text("P-100".to_owned())),
                (
                    FieldId::from("quantity"),
                    Value::Number(Number::new(1.0).unwrap()),
                ),
            ]),
        },
    );
    document.entities.insert(
        EntityId::from("product"),
        Entity {
            id: EntityId::from("product"),
            key: EntityKey::from("product"),
            schema: products.clone(),
            fields: BTreeMap::from([
                (
                    FieldId::from("product-key"),
                    Value::Text("P-100".to_owned()),
                ),
                (
                    FieldId::from("category"),
                    Value::Text("hardware".to_owned()),
                ),
                (
                    FieldId::from("price"),
                    Value::Number(Number::new(2.0).unwrap()),
                ),
            ]),
        },
    );
    let definition = KeyedGroupedSumDefinitionId::from("summary");
    document.keyed_grouped_sum_definitions.insert(
        definition.clone(),
        KeyedGroupedSumDefinition {
            id: definition,
            orders: KeyedGroupedSumOrdersBinding {
                schema: orders,
                lookup_key_field: FieldId::from("order-key"),
                quantity_field: FieldId::from("quantity"),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: products,
                key_field: FieldId::from("product-key"),
                category_field: FieldId::from("category"),
                price_field: FieldId::from("price"),
            },
        },
    );
    document
}

fn scope(subject: SemanticScope) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("document-occurrence"),
        DocumentId::from("document"),
        subject,
    )
}
fn fixture() -> Document {
    let mut d = keyed_grouped_sum_document();
    for (field, required) in [("alt-price", true), ("derived", true), ("optional", false)] {
        d.schemas.get_mut("products").unwrap().fields.insert(
            FieldId::from(field),
            FieldDefinition {
                id: FieldId::from(field),
                key: FieldKey::from(field),
                display_name: None,
                number_format: None,
                field_type: FieldType::Number,
                required,
                constraint: FieldConstraint::None,
            },
        );
    }
    let fields = &mut d.entities.get_mut("product").unwrap().fields;
    fields.insert(
        FieldId::from("alt-price"),
        Value::Number(Number::new(7.0).unwrap()),
    );
    fields.insert(
        FieldId::from("derived"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new("product", "price"))),
            right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
        }),
    );
    tachiko_workspace_engine::validate(&d).unwrap();
    d
}
fn upsert(d: &Document, change: bool) -> SemanticCommand {
    let mut definition =
        d.keyed_grouped_sum_definitions[&KeyedGroupedSumDefinitionId::from("summary")].clone();
    if change {
        definition.products.price_field = FieldId::from("alt-price");
    }
    SemanticCommand::UpsertKeyedGroupedSumDefinition { definition }
}
fn price() -> SemanticCommand {
    SemanticCommand::SetFieldValue {
        field: FieldRef::new("product", "price"),
        value: Value::Number(Number::new(3.0).unwrap()),
    }
}
fn setup(v2: bool, narrow: bool) -> PatchLifecycle {
    let constructor = if v2 {
        PatchLifecycle::new_v2
    } else {
        PatchLifecycle::new
    };
    let mut l = constructor(
        AuthorizationDomainId::from("domain"),
        DocumentScopeId::from("document-occurrence"),
        DocumentId::from("document"),
        SemanticApiContract::from("semantic-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("policy-meaning"),
    );
    for (name, kind) in [
        ("authority", PrincipalKind::Human),
        ("editor", PrincipalKind::Human),
        ("agent", PrincipalKind::Delegated),
    ] {
        l.register_principal(PrincipalId::from(name), kind).unwrap();
    }
    for who in ["editor", "agent"] {
        for family in [
            OperationFamily::SetFieldValue,
            OperationFamily::KeyedGroupedSumDefinition,
        ] {
            let query_scope = if narrow && family == OperationFamily::SetFieldValue {
                SemanticScope::EntityField {
                    entity: EntityId::from("product"),
                    schema: SchemaId::from("products"),
                    field: FieldId::from("price"),
                }
            } else {
                SemanticScope::Document
            };
            let mut requirements = vec![GrantRequirement::query(family, scope(query_scope))];
            let classes = if family == OperationFamily::SetFieldValue {
                vec![
                    MutationClass::Value,
                    MutationClass::Formula,
                    MutationClass::Destructive,
                ]
            } else {
                vec![MutationClass::Structure, MutationClass::Destructive]
            };
            for class in classes {
                for action in [
                    AuthorizationAction::Propose,
                    AuthorizationAction::Approve,
                    AuthorizationAction::Execute,
                ] {
                    requirements.push(
                        GrantRequirement::mutation(
                            action,
                            family,
                            class,
                            scope(SemanticScope::Document),
                        )
                        .unwrap(),
                    );
                }
            }
            l.provision_grant(Grant::new(
                GrantId::from(format!("{who}-{family:?}")),
                PrincipalId::from("authority"),
                PrincipalId::from(who),
                requirements,
                None,
            ))
            .unwrap();
        }
    }
    l
}
struct Clock;
impl TrustedPublicationTimeSource for Clock {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(10)
    }
}

#[allow(clippy::large_enum_variant)] // Short-lived test outcome.
enum Outcome {
    Refused(PatchLifecycleError),
    PreviewDenied(PatchLifecycleError),
    Executed {
        preview: PatchPreview,
        receipt: Result<ExecutionReceipt, PatchLifecycleError>,
    },
}

/// Drive propose -> preview -> (Human approval) -> execute on a real resident
/// session and report whether the installed document changed.
fn run(
    d: Document,
    commands: Vec<SemanticCommand>,
    narrow: bool,
    delegated: bool,
) -> (Outcome, bool) {
    let mut l = setup(false, narrow);
    let mut session =
        ResidentWorkspaceSession::new(DocumentScopeId::from("document-occurrence"), d);
    let snap = session.export_snapshot();
    let id = ProposalId::from("proposal");
    let actor = PrincipalId::from(if delegated { "agent" } else { "editor" });
    let body = SemanticPatchBody::atomic_batch(commands).unwrap();
    if let Err(error) = l.propose(
        snap.document_scope(),
        snap.document(),
        snap.revision(),
        ProposalRequest::new(id.clone(), snap.revision().clone(), body, actor.clone()),
        TrustedInstant::new(1),
    ) {
        assert_eq!(session.export_snapshot(), snap);
        return (Outcome::Refused(error), false);
    }
    let preview = match l.preview(
        snap.document_scope(),
        snap.document(),
        snap.revision(),
        &id,
        &actor,
        TrustedInstant::new(2),
    ) {
        Ok(preview) => preview,
        Err(error) => {
            assert_eq!(session.export_snapshot(), snap);
            return (Outcome::PreviewDenied(error), false);
        }
    };
    let approval = delegated.then(|| {
        let human = PrincipalId::from("editor");
        l.preview(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            &id,
            &human,
            TrustedInstant::new(3),
        )
        .unwrap();
        l.approve(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ApprovalRequest::new(
                ApprovalId::from("approval"),
                id.clone(),
                human,
                actor.clone(),
                TrustedInstant::new(100),
            ),
            TrustedInstant::new(4),
        )
        .unwrap()
    });
    let mut clock = Clock;
    let receipt = l.execute(
        &id,
        approval.as_ref().map(Approval::id),
        &actor,
        &mut session.publication_authority(&mut clock),
        TrustedInstant::new(5),
    );
    let changed = session.export_snapshot().document() != snap.document();
    (Outcome::Executed { preview, receipt }, changed)
}

fn assert_refused_without_mutation(label: &str, (outcome, changed): (Outcome, bool)) {
    assert!(!changed, "{label}: document was installed");
    match outcome {
        Outcome::Refused(PatchLifecycleError::CommandRejected { .. }) => {}
        // Existing disclosure-safe masking: an actor without Query over the
        // affected subjects learns only that disclosure was denied.
        Outcome::Refused(PatchLifecycleError::DisclosureDenied) if label == "narrow-query" => {}
        Outcome::Refused(other) => {
            panic!("{label}: refusal must be a command rejection: {other:?}")
        }
        Outcome::PreviewDenied(error) => {
            panic!("{label}: admitted, then denied at preview: {error:?}")
        }
        Outcome::Executed { preview, receipt } => panic!(
            "{label}: admitted with {} changes / {} impacts; receipt {:?}",
            preview.semantic_changes.len(),
            preview.formula_impacts.len(),
            receipt.map(|r| r.semantic_changes.len())
        ),
    }
}

#[test]
fn ac1_definition_only_batches_keep_the_supported_lifecycle() {
    let d = fixture();
    let mut absent = d.clone();
    absent.keyed_grouped_sum_definitions.clear();
    let remove = SemanticCommand::RemoveKeyedGroupedSumDefinition {
        definition: KeyedGroupedSumDefinitionId::from("summary"),
    };
    for delegated in [false, true] {
        for (label, base, command) in [
            ("create", absent.clone(), upsert(&d, false)),
            ("change", d.clone(), upsert(&d, true)),
            ("delete", d.clone(), remove.clone()),
        ] {
            let label = format!("{label} delegated={delegated}");
            let (outcome, changed) = run(base, vec![command], false, delegated);
            let Outcome::Executed { preview, receipt } = outcome else {
                panic!("{label}: definition-only batch was not admitted");
            };
            let receipt = receipt.unwrap_or_else(|e| panic!("{label}: {e:?}"));
            assert!(receipt.verified, "{label}");
            assert!(changed, "{label}: definition change was not installed");
            // V1 has no definition vocabulary and formulas cannot read definitions.
            assert_eq!(
                receipt.semantic_changes, preview.semantic_changes,
                "{label}"
            );
            assert_eq!(preview.formula_impacts.len(), 0, "{label}");
        }
    }
    // A true no-op never yields a receipt.
    let (outcome, changed) = run(d.clone(), vec![upsert(&d, false)], false, false);
    assert!(!changed);
    assert!(matches!(
        outcome,
        Outcome::Executed {
            receipt: Err(PatchLifecycleError::NoChange),
            ..
        }
    ));
}

#[test]
fn ac2_mixed_ordinary_and_definition_batches_are_refused_without_mutation() {
    let d = fixture();
    for (label, narrow, delegated) in [
        ("human", false, false),
        ("narrow-query", true, false),
        ("delegated", false, true),
    ] {
        for commands in [
            vec![price(), upsert(&d, true)],
            vec![upsert(&d, true), price()],
        ] {
            assert_refused_without_mutation(label, run(d.clone(), commands, narrow, delegated));
        }
    }
    let remove = SemanticCommand::RemoveKeyedGroupedSumDefinition {
        definition: KeyedGroupedSumDefinitionId::from("summary"),
    };
    for commands in [vec![price(), remove.clone()], vec![remove, price()]] {
        assert_refused_without_mutation("remove", run(d.clone(), commands, false, false));
    }
}

#[test]
fn ac3_definition_commands_cannot_admit_an_otherwise_rejected_batch() {
    let d = fixture();
    let initialize = SemanticCommand::SetFieldValue {
        field: FieldRef::new("product", "optional"),
        value: Value::Number(Number::new(0.0).unwrap()),
    };
    let formula = SemanticCommand::SetFieldValue {
        field: FieldRef::new("product", "optional"),
        value: Value::Formula(Expression::Reference(FieldRef::new("product", "price"))),
    };
    let (alone, changed) = run(
        d.clone(),
        vec![initialize.clone(), formula.clone()],
        false,
        false,
    );
    assert!(!changed);
    assert!(
        matches!(alone, Outcome::Refused(_)),
        "control must be refused alone"
    );
    assert_refused_without_mutation(
        "optional-chain",
        run(
            d.clone(),
            vec![initialize, formula, upsert(&d, true)],
            false,
            false,
        ),
    );
}

#[test]
fn ac4_ordinary_evidence_and_disclosure_are_unchanged() {
    let d = fixture();
    let (outcome, changed) = run(d.clone(), vec![price()], false, false);
    let Outcome::Executed { preview, receipt } = outcome else {
        panic!("ordinary edit was not admitted");
    };
    assert!(changed);
    let receipt = receipt.unwrap();
    // product.price 2 -> 3 and product.derived (= price + 1) 3 -> 4.
    assert_eq!(preview.semantic_changes.len(), 2);
    assert_eq!(preview.formula_impacts.len(), 1);
    assert_eq!(receipt.semantic_changes, preview.semantic_changes);
    let (outcome, changed) = run(d, vec![price()], true, false);
    assert!(!changed);
    assert!(matches!(
        outcome,
        Outcome::PreviewDenied(PatchLifecycleError::DisclosureDenied)
    ));
}

#[test]
fn ac5_v2_admits_complete_definition_changes() {
    let d = fixture();
    for commands in [vec![upsert(&d, true)], vec![price(), upsert(&d, true)]] {
        let mut l = setup(true, false);
        let mut session =
            ResidentWorkspaceSession::new(DocumentScopeId::from("document-occurrence"), d.clone());
        let snap = session.export_snapshot();
        let result = l.propose(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ProposalRequest::new(
                ProposalId::from("v2"),
                snap.revision().clone(),
                SemanticPatchBody::atomic_batch(commands).unwrap(),
                PrincipalId::from("editor"),
            ),
            TrustedInstant::new(1),
        );
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(session.export_snapshot(), snap);
        let preview = l
            .preview_v2(
                snap.document_scope(),
                snap.document(),
                snap.revision(),
                &"v2".into(),
                &"editor".into(),
                TrustedInstant::new(2),
            )
            .unwrap();
        assert!(preview.delta.facts().iter().any(|fact| matches!(
            fact,
            tachiko_workspace_engine::CanonicalDirectFact::KeyedGroupedSumDefinitionChanged { .. }
        )));
        let receipt = l
            .execute_v2(
                &"v2".into(),
                None,
                &"editor".into(),
                &mut session.publication_authority(&mut Clock),
                TrustedInstant::new(3),
            )
            .unwrap();
        assert_eq!(receipt.delta().unwrap(), &preview.delta);
        assert_eq!(
            receipt.delta().unwrap(),
            &tachiko_workspace_engine::canonical_delta(
                "tachiko.semantic-delta/v2",
                snap.document(),
                session.export_snapshot().document()
            )
            .unwrap()
        );
    }
}

// ADR-0044: the #530 named-state refusal scenarios now succeed under v2.
// Frozen-v1 name admission is checked separately below.
#[allow(clippy::too_many_lines)] // Exercise both evidence profiles at each lifecycle phase.
fn named_definition_case(phase: &str, operation: &str, v2: bool) {
    for schema_name in [false, true] {
        let mut base = fixture();
        let command = match operation {
            "create" => {
                let command = upsert(&base, false);
                base.keyed_grouped_sum_definitions.clear();
                command
            }
            "change" => upsert(&base, true),
            "delete" => SemanticCommand::RemoveKeyedGroupedSumDefinition {
                definition: "summary".into(),
            },
            _ => unreachable!(),
        };
        let mut named = base.clone();
        if schema_name {
            named.schemas.get_mut("products").unwrap().display_name = Some("Products Name".into());
        } else {
            named
                .schemas
                .get_mut("products")
                .unwrap()
                .fields
                .get_mut("price")
                .unwrap()
                .display_name = Some("Price Name".into());
        }
        let mut lifecycle = setup(v2, false);
        let mut session =
            ResidentWorkspaceSession::new("document-occurrence".into(), named.clone());
        let snapshot = session.export_snapshot();
        let proposal = ProposalId::from("named-definition");
        let editor = PrincipalId::from("editor");
        let admitted = lifecycle.propose(
            snapshot.document_scope(),
            if v2 || phase == "proposal" {
                &named
            } else {
                &base
            },
            snapshot.revision(),
            ProposalRequest::new(
                proposal.clone(),
                snapshot.revision().clone(),
                SemanticPatchBody::command(command),
                editor.clone(),
            ),
            TrustedInstant::new(1),
        );
        let result = match (phase, v2) {
            ("proposal", _) => admitted.map(|_| ()),
            ("preview", true) => {
                admitted.unwrap();
                lifecycle
                    .preview_v2(
                        snapshot.document_scope(),
                        &named,
                        snapshot.revision(),
                        &proposal,
                        &editor,
                        TrustedInstant::new(2),
                    )
                    .map(|_| ())
            }
            ("preview", false) => {
                admitted.unwrap();
                lifecycle
                    .preview(
                        snapshot.document_scope(),
                        &named,
                        snapshot.revision(),
                        &proposal,
                        &editor,
                        TrustedInstant::new(2),
                    )
                    .map(|_| ())
            }
            ("execution", true) => {
                admitted.unwrap();
                lifecycle
                    .execute_v2(
                        &proposal,
                        None,
                        &editor,
                        &mut session.publication_authority(&mut Clock),
                        TrustedInstant::new(3),
                    )
                    .map(|_| ())
            }
            ("execution", false) => {
                admitted.unwrap();
                lifecycle
                    .execute(
                        &proposal,
                        None,
                        &editor,
                        &mut session.publication_authority(&mut Clock),
                        TrustedInstant::new(3),
                    )
                    .map(|_| ())
            }
            _ => unreachable!(),
        };
        assert!(
            if v2 {
                result.is_ok()
            } else {
                format!("{result:?}").contains("UnsupportedDisplayName")
            },
            "{phase}/{operation}/schema={schema_name}: {result:?}"
        );
        if v2 && phase == "execution" {
            assert_ne!(session.export_snapshot(), snapshot);
        } else {
            assert_eq!(session.export_snapshot(), snapshot);
        }
    }
}

#[test]
fn named_definition_create_proposal_succeeds_v2() {
    named_definition_case("proposal", "create", true);
}

#[test]
fn named_definition_change_proposal_succeeds_v2() {
    named_definition_case("proposal", "change", true);
}

#[test]
fn named_definition_delete_proposal_succeeds_v2() {
    named_definition_case("proposal", "delete", true);
}

#[test]
fn named_definition_create_preview_succeeds_v2() {
    named_definition_case("preview", "create", true);
}

#[test]
fn named_definition_change_preview_succeeds_v2() {
    named_definition_case("preview", "change", true);
}

#[test]
fn named_definition_delete_preview_succeeds_v2() {
    named_definition_case("preview", "delete", true);
}

#[test]
fn named_definition_create_execution_succeeds_v2() {
    named_definition_case("execution", "create", true);
}

#[test]
fn named_definition_change_execution_succeeds_v2() {
    named_definition_case("execution", "change", true);
}

#[test]
fn named_definition_delete_execution_succeeds_v2() {
    named_definition_case("execution", "delete", true);
}

// #533 restores the supported commands with complete evidence; frozen-v1
// name admission is still separately enforced before its empty shortcut.
#[test]
fn frozen_v1_named_definition_commands_still_refuse() {
    for phase in ["proposal", "preview", "execution"] {
        for operation in ["create", "change", "delete"] {
            named_definition_case(phase, operation, false);
        }
    }
}

fn publish_v2(
    lifecycle: &mut PatchLifecycle,
    session: &mut ResidentWorkspaceSession,
    id: &str,
    command: SemanticCommand,
    delegated: bool,
) -> PatchPreviewV2 {
    let snap = session.export_snapshot();
    let id = ProposalId::from(id);
    let actor = PrincipalId::from(if delegated { "agent" } else { "editor" });
    lifecycle
        .propose(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ProposalRequest::new(
                id.clone(),
                snap.revision().clone(),
                SemanticPatchBody::command(command),
                actor.clone(),
            ),
            TrustedInstant::new(1),
        )
        .unwrap();
    let preview = lifecycle
        .preview_v2(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            &id,
            &actor,
            TrustedInstant::new(2),
        )
        .unwrap();
    let approval = delegated.then(|| {
        lifecycle
            .preview_v2(
                snap.document_scope(),
                snap.document(),
                snap.revision(),
                &id,
                &"editor".into(),
                TrustedInstant::new(3),
            )
            .unwrap();
        lifecycle
            .approve(
                snap.document_scope(),
                snap.document(),
                snap.revision(),
                ApprovalRequest::new(
                    ApprovalId::from(format!("approval-{id:?}")),
                    id.clone(),
                    "editor".into(),
                    actor.clone(),
                    TrustedInstant::new(100),
                ),
                TrustedInstant::new(4),
            )
            .unwrap()
    });
    let receipt = lifecycle
        .execute_v2(
            &id,
            approval.as_ref().map(Approval::id),
            &actor,
            &mut session.publication_authority(&mut Clock),
            TrustedInstant::new(5),
        )
        .unwrap();
    assert!(receipt.verified);
    assert_eq!(receipt.delta().unwrap(), &preview.delta);
    assert_eq!(
        receipt.delta().unwrap(),
        &tachiko_workspace_engine::canonical_delta(
            "tachiko.semantic-delta/v2",
            snap.document(),
            session.export_snapshot().document()
        )
        .unwrap()
    );
    preview
}

#[test]
fn named_nameless_and_admitted_constrained_definition_lifecycles_match() {
    use tachiko_workspace_engine::CanonicalDirectFact as F;
    for kind in ["nameless", "named", "constrained"] {
        for delegated in [false, true] {
            let mut d = fixture();
            if kind == "named" {
                d.schemas.get_mut("products").unwrap().display_name = Some("My Expenses".into());
            }
            if kind == "constrained" {
                d.schemas
                    .get_mut("orders")
                    .unwrap()
                    .fields
                    .get_mut("quantity")
                    .unwrap()
                    .constraint = FieldConstraint::NumberInclusiveRange {
                    min: Number::new(0.0).unwrap(),
                    max: Number::new(10.0).unwrap(),
                };
            }
            let create = upsert(&d, false);
            let change = upsert(&d, true);
            d.keyed_grouped_sum_definitions.clear();
            let mut session = ResidentWorkspaceSession::new("document-occurrence".into(), d);
            let mut lifecycle = setup(true, false);
            let created = publish_v2(&mut lifecycle, &mut session, "create", create, delegated);
            assert!(matches!(
                created.delta.facts(),
                [F::KeyedGroupedSumDefinitionCreated { .. }]
            ));
            let changed = publish_v2(&mut lifecycle, &mut session, "change", change, delegated);
            assert!(matches!(
                changed.delta.facts(),
                [F::KeyedGroupedSumDefinitionChanged { .. }]
            ));
            let edited = publish_v2(&mut lifecycle, &mut session, "edit", price(), delegated);
            assert!(matches!(
                edited.delta.facts(),
                [F::EntityFieldValueChanged { .. }]
            ));
            assert_eq!(edited.formula_impacts.len(), 1); // derived = price + 1 stays visible.
            let removed = publish_v2(
                &mut lifecycle,
                &mut session,
                "remove",
                SemanticCommand::RemoveKeyedGroupedSumDefinition {
                    definition: "summary".into(),
                },
                delegated,
            );
            assert!(matches!(
                removed.delta.facts(),
                [F::KeyedGroupedSumDefinitionDeleted { .. }]
            ));
            assert!(
                session
                    .export_snapshot()
                    .document()
                    .keyed_grouped_sum_definitions
                    .is_empty()
            );
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One occurrence proves refusals preserve state and Approval boundaries.
fn v2_definition_invalid_unauthorized_approval_and_stale_refusals_remain_atomic() {
    let mut d = fixture();
    d.schemas.get_mut("products").unwrap().display_name = Some("Products".into());
    let mut session = ResidentWorkspaceSession::new("document-occurrence".into(), d.clone());
    let mut lifecycle = setup(true, false);
    lifecycle
        .register_principal("stranger".into(), PrincipalKind::Human)
        .unwrap();
    let snap = session.export_snapshot();
    for (label, actor, command) in [
        ("unauthorized", "stranger", upsert(&d, true)),
        ("invalid", "editor", {
            let mut definition = d.keyed_grouped_sum_definitions[&"summary".into()].clone();
            definition.products.price_field = "category".into();
            SemanticCommand::UpsertKeyedGroupedSumDefinition { definition }
        }),
    ] {
        let result = lifecycle.propose(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ProposalRequest::new(
                label.into(),
                snap.revision().clone(),
                SemanticPatchBody::command(command),
                actor.into(),
            ),
            TrustedInstant::new(1),
        );
        assert!(result.is_err(), "{label}");
        assert_eq!(session.export_snapshot(), snap);
        assert_eq!(lifecycle.execution_receipts_v2(), []);
    }
    let id = ProposalId::from("delegated");
    lifecycle
        .propose(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ProposalRequest::new(
                id.clone(),
                snap.revision().clone(),
                SemanticPatchBody::command(upsert(&d, true)),
                "agent".into(),
            ),
            TrustedInstant::new(1),
        )
        .unwrap();
    lifecycle
        .preview_v2(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            &id,
            &"agent".into(),
            TrustedInstant::new(2),
        )
        .unwrap();
    let result = lifecycle.execute_v2(
        &id,
        None,
        &"agent".into(),
        &mut session.publication_authority(&mut Clock),
        TrustedInstant::new(3),
    );
    assert!(
        matches!(result, Err(PatchLifecycleError::AuthorizationDenied)),
        "{result:?}"
    );
    assert_eq!(session.export_snapshot(), snap);
    lifecycle
        .preview_v2(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            &id,
            &"editor".into(),
            TrustedInstant::new(3),
        )
        .unwrap();
    let approval = lifecycle
        .approve(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ApprovalRequest::new(
                "stale-approval".into(),
                id.clone(),
                "editor".into(),
                "agent".into(),
                TrustedInstant::new(100),
            ),
            TrustedInstant::new(4),
        )
        .unwrap();
    publish_v2(&mut lifecycle, &mut session, "intervening", price(), false);
    let current = session.export_snapshot();
    let result = lifecycle.execute_v2(
        &id,
        Some(approval.id()),
        &"agent".into(),
        &mut session.publication_authority(&mut Clock),
        TrustedInstant::new(5),
    );
    assert!(matches!(result, Err(PatchLifecycleError::Stale)));
    assert_eq!(session.export_snapshot(), current);
    assert_eq!(lifecycle.execution_receipts_v2().len(), 1); // only the intervening edit
}

#[test]
fn v2_complete_definition_evidence_does_not_waive_per_command_batch_admission() {
    let d = fixture();
    let initialize = SemanticCommand::SetFieldValue {
        field: FieldRef::new("product", "optional"),
        value: Value::Number(Number::new(0.0).unwrap()),
    };
    let formula = SemanticCommand::SetFieldValue {
        field: FieldRef::new("product", "optional"),
        value: Value::Formula(Expression::Reference(FieldRef::new("product", "price"))),
    };
    for commands in [
        vec![initialize.clone(), formula.clone()],
        vec![initialize, formula, upsert(&d, true)],
    ] {
        let mut lifecycle = setup(true, false);
        let session = ResidentWorkspaceSession::new("document-occurrence".into(), d.clone());
        let snap = session.export_snapshot();
        let result = lifecycle.propose(
            snap.document_scope(),
            snap.document(),
            snap.revision(),
            ProposalRequest::new(
                "invalid-batch".into(),
                snap.revision().clone(),
                SemanticPatchBody::atomic_batch(commands).unwrap(),
                "editor".into(),
            ),
            TrustedInstant::new(1),
        );
        assert!(result.is_err(), "{result:?}");
        assert_eq!(session.export_snapshot(), snap);
        assert_eq!(lifecycle.execution_receipts_v2(), []);
    }
}
