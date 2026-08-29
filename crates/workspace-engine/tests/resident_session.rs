mod common;

use common::game_balance_document;
use tachiko_workspace_engine::{
    DocumentId, FieldRef, Number, Value,
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId,
        ExecutionReceipt, Grant, GrantId, GrantRequirement, MutationClass, OperationFamily,
        PatchLifecycle, PatchLifecycleError, PolicyMeaningId, PrincipalId, PrincipalKind,
        ProposalId, ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant,
    },
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
};

const NOW: TrustedInstant = TrustedInstant::new(10);

fn principal() -> PrincipalId {
    PrincipalId::from("human-editor")
}

fn document_scope_id() -> DocumentScopeId {
    DocumentScopeId::from("game-occurrence")
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
        .register_principal(PrincipalId::from("authority"), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal(principal(), PrincipalKind::Human)
        .unwrap();
    let scope = ScopedSemanticSubject::new(
        document_scope_id(),
        DocumentId::from("game"),
        SemanticScope::Document,
    );
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from("human-editor-authority"),
            PrincipalId::from("authority"),
            principal(),
            vec![
                GrantRequirement::query(OperationFamily::SetFieldValue, scope.clone()),
                GrantRequirement::mutation(
                    AuthorizationAction::Propose,
                    OperationFamily::SetFieldValue,
                    MutationClass::Value,
                    scope.clone(),
                )
                .unwrap(),
                GrantRequirement::mutation(
                    AuthorizationAction::Execute,
                    OperationFamily::SetFieldValue,
                    MutationClass::Value,
                    scope,
                )
                .unwrap(),
            ],
            None,
        ))
        .unwrap();
    lifecycle
}

fn value(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn body(value: f64) -> SemanticPatchBody {
    SemanticPatchBody::command(SemanticCommand::set_field_value(
        FieldRef::new("iron_sword", "damage"),
        self::value(value),
    ))
}

struct FixedTrustedTime {
    calls: usize,
}

impl TrustedPublicationTimeSource for FixedTrustedTime {
    fn now(&mut self) -> TrustedInstant {
        self.calls += 1;
        TrustedInstant::new(11)
    }
}

fn execute_damage(
    session: &mut ResidentWorkspaceSession,
    proposal: &str,
    damage: f64,
) -> (ExecutionReceipt, usize) {
    let before = session.export_snapshot();
    let mut lifecycle = lifecycle();
    let proposal = ProposalId::from(proposal);
    lifecycle
        .propose(
            before.document_scope(),
            before.document(),
            before.revision(),
            ProposalRequest::new(
                proposal.clone(),
                before.revision().clone(),
                body(damage),
                principal(),
            ),
            NOW,
        )
        .unwrap();
    lifecycle
        .preview(
            before.document_scope(),
            before.document(),
            before.revision(),
            &proposal,
            &principal(),
            NOW,
        )
        .unwrap();
    let mut time = FixedTrustedTime { calls: 0 };
    let receipt = {
        let mut publication = session.publication_authority(&mut time);
        lifecycle
            .execute(&proposal, None, &principal(), &mut publication, NOW)
            .unwrap()
    };
    (receipt, time.calls)
}

#[test]
fn validation_query_is_revision_pinned_without_advancing_session() {
    let session = ResidentWorkspaceSession::new(
        DocumentScopeId::from("game-occurrence"),
        game_balance_document("game", "Game"),
    );
    let before = session.revision().clone();

    let query = session.validation_report();

    assert!(query.value().is_valid());
    assert_eq!(query.revision(), &before);
    assert_eq!(session.revision(), &before);
}

#[test]
fn explicit_snapshot_is_detached_from_resident_state() {
    let session = ResidentWorkspaceSession::new(
        DocumentScopeId::from("game-occurrence"),
        game_balance_document("game", "Game"),
    );

    let snapshot = session.export_snapshot();
    assert_eq!(
        snapshot.document_scope(),
        &DocumentScopeId::from("game-occurrence")
    );
    assert_eq!(snapshot.revision(), session.revision());
    assert_eq!(snapshot.document().title, "Game");

    let mut detached = snapshot.into_document();
    detached.title = "Detached export".to_owned();

    assert_eq!(session.export_snapshot().document().title, "Game");
}

#[test]
fn successful_mutation_installs_once_and_advances_one_revision() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let (receipt, publication_time_calls) = execute_damage(&mut session, "resident-success", 45.0);

    let after = session.export_snapshot();
    assert_eq!(publication_time_calls, 1);
    assert_eq!(receipt.base_revision, *before.revision());
    assert_eq!(receipt.resulting_revision, *after.revision());
    assert_ne!(after.revision(), before.revision());
    assert_eq!(
        after.document().entities["iron_sword"].fields["damage"],
        value(45.0)
    );
}

#[test]
fn repeated_commands_produce_deterministic_state_revision_and_receipt() {
    let document = game_balance_document("game", "Game");
    let mut left = ResidentWorkspaceSession::new(document_scope_id(), document.clone());
    let mut right = ResidentWorkspaceSession::new(document_scope_id(), document);

    let left_result = execute_damage(&mut left, "resident-repeat", 45.0);
    let right_result = execute_damage(&mut right, "resident-repeat", 45.0);

    assert_eq!(left_result, right_result);
    assert_eq!(left.export_snapshot(), right.export_snapshot());
}

#[test]
fn stale_expected_revision_rejects_without_installing_candidate() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let mut candidate = before.document().clone();
    candidate.title = "Must not install".to_owned();
    let mut time = FixedTrustedTime { calls: 0 };

    let error = {
        let mut publication = session.publication_authority(&mut time);
        publication
            .publish_if_current(
                before.document_scope(),
                &SemanticRevision::from("stale-revision"),
                candidate,
                |_| Some(()),
            )
            .unwrap_err()
    };

    assert_eq!(error, SemanticPublicationError::Stale);
    assert_eq!(time.calls, 1);
    assert_eq!(session.export_snapshot(), before);
}

#[test]
fn replaced_document_occurrence_rejects_before_authorization_or_installation() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let mut candidate = before.document().clone();
    candidate.title = "Must not install".to_owned();
    let mut time = FixedTrustedTime { calls: 0 };

    let error = {
        let mut publication = session.publication_authority(&mut time);
        publication
            .publish_if_current(
                &DocumentScopeId::from("replacement-occurrence"),
                before.revision(),
                candidate,
                |_| Some(()),
            )
            .unwrap_err()
    };

    assert_eq!(error, SemanticPublicationError::DocumentScopeMismatch);
    assert_eq!(time.calls, 0);
    assert_eq!(session.export_snapshot(), before);
}

#[test]
fn candidate_cannot_replace_the_resident_document_identity() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let mut candidate = before.document().clone();
    candidate.id = DocumentId::from("replacement-document");
    let mut time = FixedTrustedTime { calls: 0 };

    let error = {
        let mut publication = session.publication_authority(&mut time);
        publication
            .publish_if_current(
                before.document_scope(),
                before.revision(),
                candidate,
                |_| Some(()),
            )
            .unwrap_err()
    };

    assert_eq!(error, SemanticPublicationError::Conflict);
    assert_eq!(time.calls, 1);
    assert_eq!(session.export_snapshot(), before);
}

#[test]
fn publication_authorization_denial_leaves_state_and_revision_unchanged() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let mut candidate = before.document().clone();
    candidate.title = "Must not install".to_owned();
    let mut time = FixedTrustedTime { calls: 0 };

    let error = {
        let mut publication = session.publication_authority(&mut time);
        publication
            .publish_if_current(
                before.document_scope(),
                before.revision(),
                candidate,
                |_| None::<()>,
            )
            .unwrap_err()
    };

    assert_eq!(error, SemanticPublicationError::AuthorizationDenied);
    assert_eq!(time.calls, 1);
    assert_eq!(session.export_snapshot(), before);
}

#[test]
fn failed_calculation_query_leaves_state_and_revision_unchanged() {
    let mut document = game_balance_document("game", "Game");
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert("attack_interval".into(), value(0.0));
    let session = ResidentWorkspaceSession::new(document_scope_id(), document);
    let before = session.export_snapshot();

    assert!(session.calculate_fields().is_err());

    assert_eq!(session.export_snapshot(), before);
}

#[test]
fn validation_rejected_mutation_leaves_state_and_revision_unchanged() {
    let session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let mut lifecycle = lifecycle();

    let error = lifecycle
        .propose(
            before.document_scope(),
            before.document(),
            before.revision(),
            ProposalRequest::new(
                ProposalId::from("resident-invalid"),
                before.revision().clone(),
                SemanticPatchBody::command(SemanticCommand::set_field_value(
                    FieldRef::new("iron_sword", "attack_interval"),
                    value(0.0),
                )),
                principal(),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        PatchLifecycleError::ValidationFailed { .. }
    ));
    assert_eq!(session.export_snapshot(), before);
}

#[test]
fn rejected_command_leaves_state_and_revision_unchanged() {
    let session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.export_snapshot();
    let mut lifecycle = lifecycle();

    let error = lifecycle
        .propose(
            before.document_scope(),
            before.document(),
            before.revision(),
            ProposalRequest::new(
                ProposalId::from("resident-rejected"),
                before.revision().clone(),
                SemanticPatchBody::command(SemanticCommand::set_field_value(
                    FieldRef::new("missing-entity", "damage"),
                    value(45.0),
                )),
                principal(),
            ),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::CommandRejected { .. }));
    assert_eq!(session.export_snapshot(), before);
}
