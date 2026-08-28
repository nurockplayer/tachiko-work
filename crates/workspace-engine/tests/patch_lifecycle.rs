mod common;

use std::collections::BTreeSet;

use common::game_balance_document;
use tachiko_workspace_engine::{
    Document, DocumentId, Expression, FieldRef, Number, SemanticChange, Value, diagnostic_codes,
    patch_lifecycle::{
        ApprovalId, ApprovalRequest, ApprovalStatus, AuthorizationAction, AuthorizationDomainId,
        AuthorizationPolicyVersion, DocumentScopeId, ExecutionReceipt, Grant, GrantId,
        GrantRequirement, MutationClass, OperationFamily, PatchLifecycle, PatchLifecycleError,
        PatchLifecycleState, PatchPreview, PolicyMeaningId, PrincipalId, PrincipalKind, ProposalId,
        ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant,
    },
};

const NOW: TrustedInstant = TrustedInstant::new(10);
const EXPIRY: TrustedInstant = TrustedInstant::new(20);

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn revision(value: &str) -> SemanticRevision {
    SemanticRevision::from(value)
}

fn proposal_id(value: &str) -> ProposalId {
    ProposalId::from(value)
}

fn principal(value: &str) -> PrincipalId {
    PrincipalId::from(value)
}

fn document_scope_id() -> DocumentScopeId {
    DocumentScopeId::from("game-occurrence")
}

fn document_scope() -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        document_scope_id(),
        DocumentId::from("game"),
        SemanticScope::Document,
    )
}

fn field_scope(entity: &str, schema: &str, field: &str) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::EntityField {
            entity: entity.into(),
            schema: schema.into(),
            field: field.into(),
        },
    )
}

fn entity_scope(entity: &str, schema: &str) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::Entity {
            entity: entity.into(),
            schema: schema.into(),
        },
    )
}

fn schema_scope(schema: &str) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::Schema(schema.into()),
    )
}

fn lifecycle() -> PatchLifecycle {
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from("local-domain"),
        document_scope_id(),
        DocumentId::from("game"),
        SemanticApiContract::from("tachiko-sem-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("policy-v1-meaning"),
    );
    lifecycle
        .register_principal(principal("authority"), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal(principal("agent"), PrincipalKind::Delegated)
        .unwrap();
    lifecycle
        .register_principal(principal("reviewer"), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal(principal("other-agent"), PrincipalKind::Delegated)
        .unwrap();
    lifecycle
        .register_principal(principal("human-editor"), PrincipalKind::Human)
        .unwrap();
    lifecycle
}

fn grant(
    lifecycle: &mut PatchLifecycle,
    id: &str,
    subject: &str,
    requirements: Vec<GrantRequirement>,
) {
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from(id),
            principal("authority"),
            principal(subject),
            requirements,
            None,
        ))
        .unwrap();
}

fn query_requirement() -> GrantRequirement {
    GrantRequirement::query(OperationFamily::SetFieldValue, document_scope())
}

fn mutation_requirement(
    action: AuthorizationAction,
    mutation_class: MutationClass,
) -> GrantRequirement {
    GrantRequirement::mutation(
        action,
        OperationFamily::SetFieldValue,
        mutation_class,
        document_scope(),
    )
    .unwrap()
}

fn provision_standard_authority(lifecycle: &mut PatchLifecycle) {
    grant(
        lifecycle,
        "agent-authority",
        "agent",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Formula),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Formula),
        ],
    );
    grant(
        lifecycle,
        "reviewer-authority",
        "reviewer",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Formula),
        ],
    );
    grant(
        lifecycle,
        "human-editor-authority",
        "human-editor",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Formula),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Formula),
        ],
    );
}

fn field_command(entity: &str, field: &str, value: Value) -> SemanticCommand {
    SemanticCommand::set_field_value(FieldRef::new(entity, field), value)
}

fn propose(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    id: &str,
    body: SemanticPatchBody,
    originator: &str,
) -> ProposalId {
    let proposal_id = proposal_id(id);
    lifecycle
        .propose(
            &document_scope_id(),
            document,
            &revision("r1"),
            ProposalRequest::new(
                proposal_id.clone(),
                revision("r1"),
                body,
                principal(originator),
            ),
            NOW,
        )
        .unwrap();
    proposal_id
}

fn preview_and_approve(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    proposal: &ProposalId,
    approval: &str,
    executor: &str,
) -> ApprovalId {
    lifecycle
        .preview(
            &document_scope_id(),
            document,
            &revision("r1"),
            proposal,
            &principal("reviewer"),
            NOW,
        )
        .unwrap();
    let approval_id = ApprovalId::from(approval);
    lifecycle
        .approve(
            &document_scope_id(),
            document,
            &revision("r1"),
            ApprovalRequest::new(
                approval_id.clone(),
                proposal.clone(),
                principal("reviewer"),
                principal(executor),
                EXPIRY,
            ),
            NOW,
        )
        .unwrap();
    approval_id
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishMode {
    Normal,
    RaceStale,
    Conflict,
    ReplaceOccurrence,
    TamperAfterSuccess,
}

struct TestPublication {
    document_scope: DocumentScopeId,
    document: Document,
    revision: SemanticRevision,
    next_revision: SemanticRevision,
    publish_calls: usize,
    mode: PublishMode,
    publication_time: TrustedInstant,
}

impl TestPublication {
    fn new(document: Document, current: &str, next: &str) -> Self {
        Self {
            document_scope: document_scope_id(),
            document,
            revision: revision(current),
            next_revision: revision(next),
            publish_calls: 0,
            mode: PublishMode::Normal,
            publication_time: TrustedInstant::new(11),
        }
    }

    fn with_mode(mut self, mode: PublishMode) -> Self {
        self.mode = mode;
        self
    }

    fn with_publication_time(mut self, publication_time: TrustedInstant) -> Self {
        self.publication_time = publication_time;
        self
    }
}

impl SemanticPublicationAuthority for TestPublication {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        (
            self.document_scope.clone(),
            self.document.clone(),
            self.revision.clone(),
        )
    }

    fn publish_if_current<Authorization>(
        &mut self,
        expected_document_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<Authorization>,
    ) -> Result<(SemanticRevision, Authorization), SemanticPublicationError> {
        self.publish_calls += 1;
        if self.mode == PublishMode::RaceStale {
            self.revision = revision("r-raced");
        }
        if self.mode == PublishMode::ReplaceOccurrence {
            self.document_scope = DocumentScopeId::from("replacement-occurrence");
        }
        if &self.document_scope != expected_document_scope {
            return Err(SemanticPublicationError::DocumentScopeMismatch);
        }
        let authorization = authorize(self.publication_time)
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        match self.mode {
            PublishMode::RaceStale => Err(SemanticPublicationError::Stale),
            PublishMode::Conflict => Err(SemanticPublicationError::Conflict),
            PublishMode::Normal
            | PublishMode::ReplaceOccurrence
            | PublishMode::TamperAfterSuccess => {
                if &self.revision != expected_revision {
                    return Err(SemanticPublicationError::Stale);
                }
                self.document = candidate;
                if self.mode == PublishMode::TamperAfterSuccess {
                    self.document.title.push_str(" (tampered)");
                }
                self.revision = self.next_revision.clone();
                Ok((self.next_revision.clone(), authorization))
            }
        }
    }
}

fn assert_one_field_preview(
    preview: &PatchPreview,
    lifecycle: &PatchLifecycle,
    proposal: &ProposalId,
) {
    assert!(preview.validation_report.is_valid());
    assert_eq!(
        preview.risk.mutation_classes,
        BTreeSet::from([MutationClass::Value])
    );
    assert!(preview.semantic_changes.iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, before, after }
            if field == &FieldRef::new("iron_sword", "damage")
                && before == &number(36.0)
                && after == &number(45.0)
    )));
    assert_eq!(preview.formula_impacts.len(), 1);
    assert_eq!(
        preview.formula_impacts[0].field,
        FieldRef::new("iron_sword", "dps")
    );
    assert_eq!(
        preview.formula_impacts[0].causes,
        vec![FieldRef::new("iron_sword", "damage")]
    );
    assert_eq!(
        lifecycle.proposal_history(proposal).unwrap(),
        [
            PatchLifecycleState::Draft,
            PatchLifecycleState::Planned,
            PatchLifecycleState::Previewed,
            PatchLifecycleState::Validated,
            PatchLifecycleState::AwaitingApproval,
        ]
    );
}

fn assert_one_field_receipt(
    lifecycle: &PatchLifecycle,
    original: &Document,
    publication: &TestPublication,
    receipt: &ExecutionReceipt,
    proposal: &ProposalId,
) {
    assert_eq!(
        original.entities["iron_sword"].fields["damage"],
        number(36.0)
    );
    assert_eq!(
        publication.document.entities["iron_sword"].fields["damage"],
        number(45.0)
    );
    assert_eq!(publication.revision, revision("r2"));
    assert_eq!(publication.publish_calls, 1);
    assert_eq!(receipt.base_revision, revision("r1"));
    assert_eq!(receipt.resulting_revision, revision("r2"));
    assert!(receipt.verified);
    assert_eq!(receipt.originator, principal("agent"));
    assert_eq!(receipt.executor, principal("agent"));
    assert_eq!(
        receipt.policy_version,
        AuthorizationPolicyVersion::from("policy-v1")
    );
    let approval_evidence = receipt.approval.as_ref().unwrap();
    assert_eq!(
        approval_evidence.approval_id,
        ApprovalId::from("approval-one")
    );
    assert_eq!(approval_evidence.approver, principal("reviewer"));
    assert_eq!(approval_evidence.status, ApprovalStatus::Consumed);
    assert_eq!(
        lifecycle
            .approval_status(&ApprovalId::from("approval-one"))
            .unwrap(),
        ApprovalStatus::Consumed
    );
    assert_eq!(
        lifecycle.execution_receipts(),
        std::slice::from_ref(receipt)
    );
    assert_eq!(
        lifecycle.proposal_history(proposal).unwrap(),
        [
            PatchLifecycleState::Draft,
            PatchLifecycleState::Planned,
            PatchLifecycleState::Previewed,
            PatchLifecycleState::Validated,
            PatchLifecycleState::AwaitingApproval,
            PatchLifecycleState::Approved,
            PatchLifecycleState::Applied,
            PatchLifecycleState::Verified,
        ]
    );
}

#[test]
fn approved_one_field_patch_previews_applies_verifies_and_records_provenance() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-one",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );

    let preview = lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("reviewer"),
            NOW,
        )
        .unwrap();

    assert_one_field_preview(&preview, &lifecycle, &proposal);
    assert_eq!(
        lifecycle
            .proposal_provenance(&proposal, &principal("reviewer"), NOW)
            .unwrap()
            .exact_change,
        preview.proposal.exact_change().clone()
    );

    let approval = lifecycle
        .approve(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ApprovalRequest::new(
                ApprovalId::from("approval-one"),
                proposal.clone(),
                principal("reviewer"),
                principal("agent"),
                EXPIRY,
            ),
            NOW,
        )
        .unwrap();
    let mut publication = TestPublication::new(document, "r1", "r2");

    let receipt = lifecycle
        .execute(
            &proposal,
            Some(approval.id()),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();

    assert_one_field_receipt(&lifecycle, &original, &publication, &receipt, &proposal);
}

#[test]
fn ordered_multi_operation_patch_publishes_one_final_candidate() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let body = SemanticPatchBody::atomic_batch(vec![
        field_command("iron_sword", "damage", number(45.0)),
        field_command("shop", "gold_per_match", number(60.0)),
    ])
    .unwrap();
    let proposal = propose(&mut lifecycle, &document, "proposal-batch", body, "agent");
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-batch",
        "agent",
    );
    let mut publication = TestPublication::new(document, "r1", "r2");

    let receipt = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();

    assert_eq!(publication.publish_calls, 1);
    assert_eq!(
        publication.document.entities["iron_sword"].fields["damage"],
        number(45.0)
    );
    assert_eq!(
        publication.document.entities["shop"].fields["gold_per_match"],
        number(60.0)
    );
    assert_eq!(
        receipt
            .semantic_changes
            .iter()
            .filter(|change| matches!(change, SemanticChange::FieldChanged { .. }))
            .count(),
        2
    );
}

#[test]
fn final_validation_failure_records_failure_without_publication() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = proposal_id("proposal-invalid");

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal.clone(),
                revision("r1"),
                SemanticPatchBody::command(field_command(
                    "iron_sword",
                    "attack_interval",
                    number(0.0),
                )),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    let PatchLifecycleError::ValidationFailed { report } = error else {
        panic!("expected authoritative gate failure, got {error:?}");
    };
    assert!(!report.is_valid());
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap(),
        [
            PatchLifecycleState::Draft,
            PatchLifecycleState::Planned,
            PatchLifecycleState::ValidationFailed,
        ]
    );
    assert!(lifecycle.execution_receipts().is_empty());
    assert_eq!(
        document.entities["iron_sword"].fields["attack_interval"],
        number(0.9)
    );
    assert!(matches!(
        lifecycle.preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("reviewer"),
            NOW,
        ),
        Err(PatchLifecycleError::ProposalNotExecutable)
    ));
    assert!(matches!(
        lifecycle.proposal_provenance(&proposal, &principal("reviewer"), NOW),
        Err(PatchLifecycleError::ProposalNotExecutable)
    ));
}

#[test]
fn validation_failure_details_require_independent_query_authority() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-propose-without-query",
        "agent",
        vec![mutation_requirement(
            AuthorizationAction::Propose,
            MutationClass::Value,
        )],
    );
    let proposal = proposal_id("proposal-private-validation");

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal.clone(),
                revision("r1"),
                SemanticPatchBody::command(field_command(
                    "iron_sword",
                    "attack_interval",
                    number(0.0),
                )),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::DisclosureDenied));
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::ValidationFailed)
    );
}

#[test]
fn disclosure_gated_entry_points_hide_proposal_registry_state() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let retained_failure = proposal_id("proposal-retained-private-failure");
    let failure = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                retained_failure.clone(),
                revision("r1"),
                SemanticPatchBody::command(field_command(
                    "iron_sword",
                    "attack_interval",
                    number(0.0),
                )),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();
    assert!(matches!(
        failure,
        PatchLifecycleError::ValidationFailed { .. }
    ));
    let executable = propose(
        &mut lifecycle,
        &document,
        "proposal-private-executable",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let missing = proposal_id("proposal-private-missing");
    let retained_history = lifecycle
        .proposal_history(&retained_failure)
        .unwrap()
        .to_vec();
    let executable_history = lifecycle.proposal_history(&executable).unwrap().to_vec();

    for viewer in [principal("other-agent"), principal("unregistered")] {
        for proposal in [&missing, &retained_failure, &executable] {
            assert!(matches!(
                lifecycle.preview(
                    &document_scope_id(),
                    &document,
                    &revision("r1"),
                    proposal,
                    &viewer,
                    NOW,
                ),
                Err(PatchLifecycleError::DisclosureDenied)
            ));
            assert!(matches!(
                lifecycle.proposal_provenance(proposal, &viewer, NOW),
                Err(PatchLifecycleError::DisclosureDenied)
            ));
        }
    }
    for (index, proposal) in [&missing, &retained_failure, &executable]
        .into_iter()
        .enumerate()
    {
        assert!(matches!(
            lifecycle.approve(
                &document_scope_id(),
                &document,
                &revision("r1"),
                ApprovalRequest::new(
                    ApprovalId::from(format!("approval-private-probe-{index}")),
                    proposal.clone(),
                    principal("authority"),
                    principal("agent"),
                    EXPIRY,
                ),
                NOW,
            ),
            Err(PatchLifecycleError::DisclosureDenied)
        ));
    }

    assert_eq!(
        lifecycle.proposal_history(&retained_failure).unwrap(),
        retained_history
    );
    assert_eq!(
        lifecycle.proposal_history(&executable).unwrap(),
        executable_history
    );
}

#[test]
fn individually_valid_formula_commands_that_form_a_cycle_fail_as_one_batch() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = proposal_id("proposal-formula-cycle");
    let body = SemanticPatchBody::atomic_batch(vec![
        field_command(
            "iron_sword",
            "dps",
            Value::Formula(Expression::Reference(FieldRef::new(
                "shop",
                "matches_for_sword",
            ))),
        ),
        field_command(
            "shop",
            "matches_for_sword",
            Value::Formula(Expression::Reference(FieldRef::new("iron_sword", "dps"))),
        ),
    ])
    .unwrap();

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(proposal.clone(), revision("r1"), body, principal("agent")),
            NOW,
        )
        .unwrap_err();

    let PatchLifecycleError::ValidationFailed { report } = error else {
        panic!("expected final batch validation failure, got {error:?}");
    };
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == diagnostic_codes::FORMULA_CYCLE)
    );
    assert_eq!(document, original);
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::ValidationFailed)
    );
}

#[test]
fn middle_command_failure_never_publishes_a_successful_prefix() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = proposal_id("proposal-middle-failure");
    let body = SemanticPatchBody::atomic_batch(vec![
        field_command("iron_sword", "damage", number(45.0)),
        field_command("missing-entity", "damage", number(99.0)),
    ])
    .unwrap();

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(proposal.clone(), revision("r1"), body, principal("agent")),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::CommandRejected { .. }));
    assert_eq!(document, original);
    assert!(matches!(
        lifecycle.proposal_history(&proposal),
        Err(PatchLifecycleError::ProposalNotFound)
    ));
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn stale_base_after_preview_denies_without_consuming_approval() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-stale",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-stale",
        "agent",
    );
    let mut publication = TestPublication::new(document, "r2", "r3");

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::Stale));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 0);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::Stale)
    );
}

#[test]
fn unauthorized_executor_cannot_probe_missing_or_issued_proposal_ids() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-probe-resistant",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-probe-resistant",
        "agent",
    );
    let missing = proposal_id("proposal-never-issued");
    let history_before = lifecycle.proposal_history(&proposal).unwrap().to_vec();
    let mut publication = TestPublication::new(document, "r1", "r2");

    for (executor, supplied_approval) in [
        (principal("unregistered"), Some(&approval)),
        (principal("other-agent"), None),
        (principal("agent"), None),
    ] {
        for target in [&proposal, &missing] {
            let error = lifecycle
                .execute(
                    target,
                    supplied_approval,
                    &executor,
                    &mut publication,
                    TrustedInstant::new(11),
                )
                .unwrap_err();

            assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
        }
    }

    assert_eq!(publication.publish_calls, 0);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap(),
        history_before
    );
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn preview_authorizes_disclosure_before_revealing_stale_state() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-stale-preview",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let history_before = lifecycle.proposal_history(&proposal).unwrap().to_vec();

    let error = lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r2"),
            &proposal,
            &principal("other-agent"),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::DisclosureDenied));
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap(),
        history_before
    );
}

#[test]
fn publication_compare_and_swap_closes_the_post_validation_race() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-race",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-race",
        "agent",
    );
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_mode(PublishMode::RaceStale);

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::Stale));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 1);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
}

#[test]
fn document_occurrence_replacement_does_not_inherit_authority() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-replaced-occurrence",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-replaced-occurrence",
        "agent",
    );
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_mode(PublishMode::ReplaceOccurrence);

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
    assert_eq!(
        publication.document_scope,
        DocumentScopeId::from("replacement-occurrence")
    );
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 1);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn approval_for_one_proposal_cannot_execute_another() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let first = propose(
        &mut lifecycle,
        &document,
        "proposal-first",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let second = propose(
        &mut lifecycle,
        &document,
        "proposal-second",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(46.0))),
        "agent",
    );
    let approval =
        preview_and_approve(&mut lifecycle, &document, &first, "approval-first", "agent");
    let mut publication = TestPublication::new(document, "r1", "r2");

    let error = lifecycle
        .execute(
            &second,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 0);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
}

#[test]
fn proposal_identity_cannot_be_reused_even_for_equal_contents() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let body = SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0)));
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-non-reusable",
        body.clone(),
        "agent",
    );

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(proposal, revision("r1"), body, principal("agent")),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::ProposalIdAlreadyExists
    ));
}

#[test]
fn missing_propose_capability_fails_closed() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-query-only",
        "agent",
        vec![query_requirement()],
    );

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal_id("proposal-denied"),
                revision("r1"),
                SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Propose
        }
    ));
}

#[test]
fn unauthorized_failed_attempts_do_not_reserve_proposal_ids() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-query-only",
        "agent",
        vec![query_requirement()],
    );
    let stale_id = proposal_id("unauthorized-stale-attempt");
    let rejected_id = proposal_id("unauthorized-rejected-attempt");

    let stale = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                stale_id.clone(),
                revision("r0"),
                SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();
    let rejected = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                rejected_id.clone(),
                revision("r1"),
                SemanticPatchBody::command(field_command("missing-entity", "damage", number(45.0))),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(stale, PatchLifecycleError::Stale));
    assert!(matches!(
        rejected,
        PatchLifecycleError::CommandRejected { .. }
    ));
    assert!(matches!(
        lifecycle.proposal_history(&stale_id),
        Err(PatchLifecycleError::ProposalNotFound)
    ));
    assert!(matches!(
        lifecycle.proposal_history(&rejected_id),
        Err(PatchLifecycleError::ProposalNotFound)
    ));

    grant(
        &mut lifecycle,
        "agent-propose-after-failures",
        "agent",
        vec![mutation_requirement(
            AuthorizationAction::Propose,
            MutationClass::Value,
        )],
    );
    for id in [stale_id, rejected_id] {
        lifecycle
            .propose(
                &document_scope_id(),
                &document,
                &revision("r1"),
                ProposalRequest::new(
                    id,
                    revision("r1"),
                    SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
                    principal("agent"),
                ),
                NOW,
            )
            .unwrap();
    }
}

#[test]
fn propose_only_authority_issues_inert_patch_but_reveals_no_preview() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-propose-only",
        "agent",
        vec![mutation_requirement(
            AuthorizationAction::Propose,
            MutationClass::Value,
        )],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-inert",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );

    let error = lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("agent"),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::DisclosureDenied));
    assert!(matches!(
        lifecycle.proposal_provenance(&proposal, &principal("agent"), NOW),
        Err(PatchLifecycleError::DisclosureDenied)
    ));
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap(),
        [PatchLifecycleState::Draft, PatchLifecycleState::Planned]
    );
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn intermediate_batch_reference_still_derives_disclosure_from_the_exact_command_body() {
    let mut document = game_balance_document("game", "Game");
    for id in ["bronze_sword", "steel_sword"] {
        let mut weapon = document.entities["iron_sword"].clone();
        weapon.id = id.into();
        weapon.key = id.into();
        document.entities.insert(weapon.id.clone(), weapon);
    }
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-intermediate-reference",
        "agent",
        vec![
            GrantRequirement::query(
                OperationFamily::SetFieldValue,
                field_scope("alric", "characters", "weapon"),
            ),
            GrantRequirement::query(
                OperationFamily::SetFieldValue,
                entity_scope("iron_sword", "weapons"),
            ),
            GrantRequirement::query(
                OperationFamily::SetFieldValue,
                entity_scope("steel_sword", "weapons"),
            ),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
        ],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-intermediate-reference",
        SemanticPatchBody::atomic_batch(vec![
            field_command("alric", "weapon", Value::Reference("bronze_sword".into())),
            field_command("alric", "weapon", Value::Reference("steel_sword".into())),
        ])
        .unwrap(),
        "agent",
    );

    let error = lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("agent"),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::DisclosureDenied));
    assert!(matches!(
        lifecycle.proposal_provenance(&proposal, &principal("agent"), NOW),
        Err(PatchLifecycleError::DisclosureDenied)
    ));
}

#[test]
fn executor_without_query_publishes_but_receives_no_semantic_projection() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-write-only",
        "agent",
        vec![
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Value),
        ],
    );
    grant(
        &mut lifecycle,
        "reviewer-review",
        "reviewer",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Value),
        ],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-write-only",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-write-only",
        "agent",
    );
    let mut publication = TestPublication::new(document, "r1", "r2");

    let receipt = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();

    assert_eq!(
        publication.document.entities["iron_sword"].fields["damage"],
        number(45.0)
    );
    assert!(receipt.verified);
    assert!(receipt.authorization_footprint.is_none());
    assert!(receipt.semantic_changes.is_empty());
    assert!(receipt.formula_impacts.is_empty());
    assert!(receipt.validation_report.is_none());
    assert!(
        lifecycle.execution_receipts()[0]
            .authorization_footprint
            .is_some()
    );
    assert!(
        !lifecycle.execution_receipts()[0]
            .semantic_changes
            .is_empty()
    );
    assert!(matches!(
        lifecycle.proposal_provenance(&proposal, &principal("agent"), NOW),
        Err(PatchLifecycleError::DisclosureDenied)
    ));
    assert_eq!(
        lifecycle
            .proposal_provenance(&proposal, &principal("reviewer"), NOW)
            .unwrap()
            .proposal_id,
        proposal
    );
}

#[test]
fn approve_authority_alone_neither_discloses_nor_completes_review() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-propose",
        "agent",
        vec![mutation_requirement(
            AuthorizationAction::Propose,
            MutationClass::Value,
        )],
    );
    grant(
        &mut lifecycle,
        "reviewer-approve-only",
        "reviewer",
        vec![mutation_requirement(
            AuthorizationAction::Approve,
            MutationClass::Value,
        )],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-approve-no-read",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );

    let preview_error = lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("reviewer"),
            NOW,
        )
        .unwrap_err();
    let approval_error = lifecycle
        .approve(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ApprovalRequest::new(
                ApprovalId::from("approval-no-review"),
                proposal,
                principal("reviewer"),
                principal("agent"),
                EXPIRY,
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        preview_error,
        PatchLifecycleError::DisclosureDenied
    ));
    assert!(matches!(
        approval_error,
        PatchLifecycleError::DisclosureDenied
    ));
}

#[test]
fn relational_grant_bindings_cannot_cross_mutation_class_and_scope() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-cross-paired",
        "agent",
        vec![
            query_requirement(),
            GrantRequirement::mutation(
                AuthorizationAction::Propose,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                field_scope("iron_sword", "weapons", "damage"),
            )
            .unwrap(),
            GrantRequirement::mutation(
                AuthorizationAction::Propose,
                OperationFamily::SetFieldValue,
                MutationClass::Formula,
                field_scope("iron_sword", "weapons", "price"),
            )
            .unwrap(),
        ],
    );
    let formula = Value::Formula(Expression::Reference(FieldRef::new("iron_sword", "price")));

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal_id("proposal-cross-paired"),
                revision("r1"),
                SemanticPatchBody::command(field_command("iron_sword", "damage", formula)),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Propose
        }
    ));
}

#[test]
fn schema_scope_covers_its_entity_instances_and_fields() {
    let mut document = game_balance_document("game", "Game");
    let mut steel_sword = document.entities["iron_sword"].clone();
    steel_sword.id = "steel_sword".into();
    steel_sword.key = "steel_sword".into();
    document
        .entities
        .insert(steel_sword.id.clone(), steel_sword);
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-schema-scopes",
        "agent",
        vec![
            GrantRequirement::query(OperationFamily::SetFieldValue, schema_scope("characters")),
            GrantRequirement::query(OperationFamily::SetFieldValue, schema_scope("weapons")),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
        ],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-schema-scope",
        SemanticPatchBody::command(field_command(
            "alric",
            "weapon",
            Value::Reference("steel_sword".into()),
        )),
        "agent",
    );

    let preview = lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("agent"),
            NOW,
        )
        .unwrap();

    assert_eq!(preview.semantic_changes.len(), 1);
}

#[test]
fn matching_semantic_ids_in_another_document_occurrence_grant_nothing() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    let other_occurrence = ScopedSemanticSubject::new(
        DocumentScopeId::from("other-game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::Document,
    );
    grant(
        &mut lifecycle,
        "agent-other-occurrence",
        "agent",
        vec![
            GrantRequirement::query(OperationFamily::SetFieldValue, other_occurrence.clone()),
            GrantRequirement::mutation(
                AuthorizationAction::Propose,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                other_occurrence,
            )
            .unwrap(),
        ],
    );

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal_id("proposal-wrong-occurrence"),
                revision("r1"),
                SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Propose
        }
    ));
}

#[test]
fn replacement_occurrence_snapshot_is_rejected_even_with_the_same_document_id() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = proposal_id("proposal-replacement-snapshot");

    let error = lifecycle
        .propose(
            &DocumentScopeId::from("replacement-occurrence"),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal.clone(),
                revision("r1"),
                SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::DocumentScopeMismatch));
    assert!(matches!(
        lifecycle.proposal_history(&proposal),
        Err(PatchLifecycleError::ProposalNotFound)
    ));
    lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal,
                revision("r1"),
                SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap();
}

#[test]
fn empty_atomic_batch_is_not_a_semantic_proposal_body() {
    assert!(matches!(
        SemanticPatchBody::atomic_batch(Vec::new()),
        Err(PatchLifecycleError::EmptyAtomicBatch)
    ));
}

#[test]
fn provision_grant_rejects_a_manually_malformed_query_mutation_requirement() {
    let mut lifecycle = lifecycle();
    let malformed = GrantRequirement::Mutation {
        action: AuthorizationAction::Query,
        family: OperationFamily::SetFieldValue,
        mutation_class: MutationClass::Value,
        scope: document_scope(),
    };

    let error = lifecycle
        .provision_grant(Grant::new(
            GrantId::from("malformed-query-mutation"),
            principal("authority"),
            principal("agent"),
            vec![malformed],
            None,
        ))
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::InvalidGrant));
}

#[test]
fn delegated_principal_cannot_issue_or_delegate_grants() {
    let mut lifecycle = lifecycle();
    let cases = [
        ("delegated-self-grant", principal("agent")),
        ("delegated-transitive-grant", principal("other-agent")),
    ];

    for (id, subject) in &cases {
        let error = lifecycle
            .provision_grant(Grant::new(
                GrantId::from(*id),
                principal("agent"),
                subject.clone(),
                vec![query_requirement()],
                None,
            ))
            .unwrap_err();
        assert!(matches!(error, PatchLifecycleError::InvalidGrant));
    }

    for (id, subject) in cases {
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from(id),
                principal("authority"),
                subject,
                vec![query_requirement()],
                None,
            ))
            .unwrap();
    }
}

#[test]
fn disabled_human_cannot_issue_grants() {
    let mut lifecycle = lifecycle();
    let grant_id = GrantId::from("disabled-issuer-grant");
    lifecycle
        .disable_principal(&principal("authority"))
        .unwrap();

    let error = lifecycle
        .provision_grant(Grant::new(
            grant_id.clone(),
            principal("authority"),
            principal("agent"),
            vec![query_requirement()],
            None,
        ))
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::InvalidGrant));
    lifecycle
        .provision_grant(Grant::new(
            grant_id,
            principal("reviewer"),
            principal("agent"),
            vec![query_requirement()],
            None,
        ))
        .unwrap();
}

#[test]
fn principal_occurrence_identity_and_kind_are_immutable() {
    let mut lifecycle = lifecycle();

    let duplicate = lifecycle
        .register_principal(principal("agent"), PrincipalKind::Delegated)
        .unwrap_err();
    let reclassification = lifecycle
        .register_principal(principal("agent"), PrincipalKind::Human)
        .unwrap_err();

    assert!(matches!(
        duplicate,
        PatchLifecycleError::PrincipalIdAlreadyExists
    ));
    assert!(matches!(
        reclassification,
        PatchLifecycleError::PrincipalKindMismatch
    ));
}

#[test]
fn policy_version_identity_cannot_be_reused_for_changed_meaning() {
    let mut lifecycle = lifecycle();

    let error = lifecycle
        .transition_effective_policy(
            AuthorizationPolicyVersion::from("policy-v1"),
            PolicyMeaningId::from("changed-policy-v1-meaning"),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::PolicyMeaningConflict));
}

#[test]
fn value_authority_does_not_authorize_formula_mutation() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-value-only",
        "agent",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
        ],
    );
    let formula = Value::Formula(Expression::Reference(FieldRef::new("iron_sword", "damage")));

    let error = lifecycle
        .propose(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ProposalRequest::new(
                proposal_id("proposal-formula-denied"),
                revision("r1"),
                SemanticPatchBody::command(field_command("iron_sword", "price", formula)),
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Propose
        }
    ));
}

#[test]
fn expired_and_revoked_approvals_never_publish() {
    for (proposal_name, approval_name, revoke, expected) in [
        (
            "proposal-expired",
            "approval-expired",
            false,
            ApprovalStatus::Expired,
        ),
        (
            "proposal-revoked",
            "approval-revoked",
            true,
            ApprovalStatus::Revoked,
        ),
    ] {
        let document = game_balance_document("game", "Game");
        let original = document.clone();
        let mut lifecycle = lifecycle();
        provision_standard_authority(&mut lifecycle);
        let proposal = propose(
            &mut lifecycle,
            &document,
            proposal_name,
            SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
            "agent",
        );
        lifecycle
            .preview(
                &document_scope_id(),
                &document,
                &revision("r1"),
                &proposal,
                &principal("reviewer"),
                NOW,
            )
            .unwrap();
        let approval_id = ApprovalId::from(approval_name);
        lifecycle
            .approve(
                &document_scope_id(),
                &document,
                &revision("r1"),
                ApprovalRequest::new(
                    approval_id.clone(),
                    proposal.clone(),
                    principal("reviewer"),
                    principal("agent"),
                    TrustedInstant::new(12),
                ),
                NOW,
            )
            .unwrap();
        if revoke {
            lifecycle.revoke_approval(&approval_id).unwrap();
        }
        let mut publication = TestPublication::new(document, "r1", "r2");

        let error = lifecycle
            .execute(
                &proposal,
                Some(&approval_id),
                &principal("agent"),
                &mut publication,
                TrustedInstant::new(13),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            PatchLifecycleError::ApprovalExpired | PatchLifecycleError::ApprovalRevoked
        ));
        assert_eq!(lifecycle.approval_status(&approval_id).unwrap(), expected);
        assert_eq!(publication.document, original);
        assert_eq!(publication.publish_calls, 0);
    }
}

#[test]
fn approval_expiring_inside_the_publication_guard_never_publishes() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-boundary-approval-expiry",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("reviewer"),
            NOW,
        )
        .unwrap();
    let approval = ApprovalId::from("approval-boundary-expiry");
    lifecycle
        .approve(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ApprovalRequest::new(
                approval.clone(),
                proposal.clone(),
                principal("reviewer"),
                principal("agent"),
                TrustedInstant::new(12),
            ),
            NOW,
        )
        .unwrap();
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_publication_time(TrustedInstant::new(12));

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::ApprovalExpired));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 1);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Expired
    );
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn execute_grant_expiring_inside_the_publication_guard_never_publishes() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from("agent-expiring-authority"),
            principal("authority"),
            principal("agent"),
            vec![
                query_requirement(),
                mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
                mutation_requirement(AuthorizationAction::Execute, MutationClass::Value),
            ],
            Some(TrustedInstant::new(12)),
        ))
        .unwrap();
    grant(
        &mut lifecycle,
        "reviewer-boundary-authority",
        "reviewer",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Value),
        ],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-boundary-grant-expiry",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-boundary-grant-expiry",
        "agent",
    );
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_publication_time(TrustedInstant::new(12));

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Execute
        }
    ));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 1);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn consumed_approval_replay_is_distinct_and_cannot_publish_twice() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-replay",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-replay",
        "agent",
    );
    let mut publication = TestPublication::new(document, "r1", "r2");
    lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(12),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::ApprovalConsumed));
    assert_eq!(publication.publish_calls, 1);
}

#[test]
fn verified_proposal_history_remains_terminal_across_later_entry_points() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-verified-terminal",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "human-editor",
    );
    lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("reviewer"),
            NOW,
        )
        .unwrap();
    let mut publication = TestPublication::new(document, "r1", "r2");
    lifecycle
        .execute(
            &proposal,
            None,
            &principal("human-editor"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();
    let verified_history = lifecycle.proposal_history(&proposal).unwrap().to_vec();
    assert_eq!(
        verified_history.last(),
        Some(&PatchLifecycleState::Verified)
    );
    let installed = publication.document.clone();

    let preview = lifecycle
        .preview(
            &document_scope_id(),
            &installed,
            &revision("r2"),
            &proposal,
            &principal("reviewer"),
            TrustedInstant::new(12),
        )
        .unwrap_err();
    let approval = lifecycle
        .approve(
            &document_scope_id(),
            &installed,
            &revision("r2"),
            ApprovalRequest::new(
                ApprovalId::from("approval-after-verified"),
                proposal.clone(),
                principal("reviewer"),
                principal("human-editor"),
                TrustedInstant::new(20),
            ),
            TrustedInstant::new(12),
        )
        .unwrap_err();
    let execute = lifecycle
        .execute(
            &proposal,
            None,
            &principal("human-editor"),
            &mut publication,
            TrustedInstant::new(12),
        )
        .unwrap_err();

    for error in [preview, approval, execute] {
        assert!(matches!(error, PatchLifecycleError::ProposalNotExecutable));
    }
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap(),
        verified_history
    );
    assert_eq!(publication.publish_calls, 1);
}

#[test]
fn loss_of_live_execute_grant_denies_and_leaves_approval_active() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-grant-loss",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-grant-loss",
        "agent",
    );
    lifecycle
        .revoke_grant(&GrantId::from("agent-authority"))
        .unwrap();
    let mut publication = TestPublication::new(document, "r1", "r2");

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Execute
        }
    ));
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert_eq!(publication.document, original);
}

#[test]
fn disabled_executor_or_approver_blocks_publication() {
    for (disabled, suffix) in [("agent", "executor"), ("reviewer", "approver")] {
        let document = game_balance_document("game", "Game");
        let original = document.clone();
        let mut lifecycle = lifecycle();
        provision_standard_authority(&mut lifecycle);
        let proposal = propose(
            &mut lifecycle,
            &document,
            &format!("proposal-disabled-{suffix}"),
            SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
            "agent",
        );
        let approval = preview_and_approve(
            &mut lifecycle,
            &document,
            &proposal,
            &format!("approval-disabled-{suffix}"),
            "agent",
        );
        lifecycle.disable_principal(&principal(disabled)).unwrap();
        let mut publication = TestPublication::new(document, "r1", "r2");

        let error = lifecycle
            .execute(
                &proposal,
                Some(&approval),
                &principal("agent"),
                &mut publication,
                TrustedInstant::new(11),
            )
            .unwrap_err();

        if disabled == "agent" {
            assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
        } else {
            assert!(matches!(error, PatchLifecycleError::PrincipalDisabled));
        }
        assert_eq!(publication.document, original);
        assert_eq!(publication.publish_calls, 0);
        assert_eq!(
            lifecycle.approval_status(&approval).unwrap(),
            ApprovalStatus::Active
        );
    }
}

#[test]
fn replacement_authority_cannot_revive_a_revoked_approval_grant_reference() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-approve-grant-loss",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-approve-grant-loss",
        "agent",
    );
    lifecycle
        .revoke_grant(&GrantId::from("reviewer-authority"))
        .unwrap();
    grant(
        &mut lifecycle,
        "reviewer-equivalent-replacement",
        "reviewer",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Value),
        ],
    );
    let mut publication = TestPublication::new(document, "r1", "r2");

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Approve
        }
    ));
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert_eq!(publication.document, original);
}

#[test]
fn policy_transition_and_rollback_do_not_revive_approval() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-policy",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-policy",
        "agent",
    );
    lifecycle
        .transition_effective_policy(
            AuthorizationPolicyVersion::from("policy-v2"),
            PolicyMeaningId::from("policy-v2-meaning"),
        )
        .unwrap();
    lifecycle
        .transition_effective_policy(
            AuthorizationPolicyVersion::from("policy-v1"),
            PolicyMeaningId::from("policy-v1-meaning"),
        )
        .unwrap();
    let mut publication = TestPublication::new(document, "r1", "r2");

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::AuthorizationPolicyChanged
    ));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 0);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
}

#[test]
fn wrong_executor_cannot_use_or_probe_an_approval() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-wrong-executor",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-wrong-executor",
        "agent",
    );
    let mut publication = TestPublication::new(document, "r2", "r3");

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("other-agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
    assert_eq!(publication.document, original);
    assert_eq!(publication.publish_calls, 0);
}

#[test]
fn directly_authenticated_human_execution_fabricates_no_approval_evidence() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-human",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "human-editor",
    );
    lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            &proposal,
            &principal("human-editor"),
            NOW,
        )
        .unwrap();
    let mut publication = TestPublication::new(document, "r1", "r2");

    let receipt = lifecycle
        .execute(
            &proposal,
            None,
            &principal("human-editor"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();

    assert!(receipt.approval.is_none());
    assert!(receipt.approve_grants.is_empty());
    assert_eq!(receipt.executor, principal("human-editor"));
}

#[test]
fn publisher_conflict_does_not_consume_approval_or_install_candidate() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-conflict",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-conflict",
        "agent",
    );
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_mode(PublishMode::Conflict);

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::Conflict));
    assert_eq!(publication.document, original);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::Conflict)
    );
}

#[test]
fn query_expiry_at_failed_publication_hides_the_semantic_conflict() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from("agent-expiring-query"),
            principal("authority"),
            principal("agent"),
            vec![query_requirement()],
            Some(TrustedInstant::new(12)),
        ))
        .unwrap();
    grant(
        &mut lifecycle,
        "agent-mutation-authority",
        "agent",
        vec![
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Value),
        ],
    );
    grant(
        &mut lifecycle,
        "reviewer-authority",
        "reviewer",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Value),
        ],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-conflict-after-query-expiry",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-conflict-after-query-expiry",
        "agent",
    );
    let mut publication = TestPublication::new(document, "r1", "r2")
        .with_mode(PublishMode::Conflict)
        .with_publication_time(TrustedInstant::new(12));

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
    assert_eq!(publication.document, original);
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Active
    );
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::Conflict)
    );
}

#[test]
fn post_publication_verification_detects_installed_state_mismatch() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    provision_standard_authority(&mut lifecycle);
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-verification",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-verification",
        "agent",
    );
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_mode(PublishMode::TamperAfterSuccess);

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::VerificationFailed));
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Consumed
    );
    assert_eq!(lifecycle.execution_receipts().len(), 1);
    assert!(!lifecycle.execution_receipts()[0].verified);
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::Conflict)
    );
}

#[test]
fn write_only_executor_cannot_observe_verification_failure() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-write-only",
        "agent",
        vec![
            mutation_requirement(AuthorizationAction::Propose, MutationClass::Value),
            mutation_requirement(AuthorizationAction::Execute, MutationClass::Value),
        ],
    );
    grant(
        &mut lifecycle,
        "reviewer-review",
        "reviewer",
        vec![
            query_requirement(),
            mutation_requirement(AuthorizationAction::Approve, MutationClass::Value),
        ],
    );
    let proposal = propose(
        &mut lifecycle,
        &document,
        "proposal-private-verification",
        SemanticPatchBody::command(field_command("iron_sword", "damage", number(45.0))),
        "agent",
    );
    let approval = preview_and_approve(
        &mut lifecycle,
        &document,
        &proposal,
        "approval-private-verification",
        "agent",
    );
    let mut publication =
        TestPublication::new(document, "r1", "r2").with_mode(PublishMode::TamperAfterSuccess);

    let error = lifecycle
        .execute(
            &proposal,
            Some(&approval),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::AuthorizationDenied));
    assert_eq!(
        lifecycle.approval_status(&approval).unwrap(),
        ApprovalStatus::Consumed
    );
    assert_eq!(lifecycle.execution_receipts().len(), 1);
    assert!(!lifecycle.execution_receipts()[0].verified);
    assert_eq!(
        lifecycle.proposal_history(&proposal).unwrap().last(),
        Some(&PatchLifecycleState::Conflict)
    );
}
