mod common;

use common::game_balance_document;
use tachiko_workspace_engine::{
    CalculationFailure, DocumentId, EntityId, EntityKey, Expression, FieldAddress, FieldRef,
    Number, Value, diagnostic_codes,
    formula_operations::FormulaCalculationOutcome,
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId,
        ExecutionReceipt, Grant, GrantId, GrantRequirement, MutationClass, OperationFamily,
        PatchLifecycle, PatchLifecycleError, PolicyMeaningId, PrincipalId, PrincipalKind,
        ProposalId, ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant,
    },
    rename_entity, rename_field,
    resident_session::{
        ResidentProjectionInvalidation, ResidentWorkspaceSession, TrustedPublicationTimeSource,
    },
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
) -> (ExecutionReceipt, ResidentProjectionInvalidation, usize) {
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
    let (receipt, invalidation) = {
        let mut publication = session.publication_authority(&mut time);
        let receipt = lifecycle
            .execute(&proposal, None, &principal(), &mut publication, NOW)
            .unwrap();
        let invalidation = publication
            .projection_invalidation_for(&receipt.base_revision, &receipt.resulting_revision)
            .unwrap()
            .clone();
        (receipt, invalidation)
    };
    (receipt, invalidation, time.calls)
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
fn selective_entity_query_returns_only_requested_stable_subjects() {
    let session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let before = session.revision().clone();

    let query = session
        .query_entities(&[EntityId::from("iron_sword")])
        .unwrap();

    assert_eq!(query.revision(), &before);
    assert_eq!(session.revision(), &before);
    assert_eq!(query.value().len(), 1);
    assert_eq!(query.value()[0].id, EntityId::from("iron_sword"));
    assert_eq!(query.value()[0].key, EntityKey::from("iron_sword"));
}

#[test]
fn field_query_keeps_semantic_formula_calculation_and_presentation_distinct() {
    let session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let damage = FieldRef::new("iron_sword", "damage");
    let dps = FieldRef::new("iron_sword", "dps");

    let query = session
        .query_fields(&[dps.clone(), damage.clone()])
        .unwrap();

    assert_eq!(query.value().len(), 2);
    assert_eq!(query.value()[0].field, damage);
    assert_eq!(query.value()[1].field, dps);

    let stored = &query.value()[0];
    assert_eq!(stored.stored_value, Some(value(36.0)));
    assert_eq!(stored.formula_definition, None);
    assert_eq!(stored.calculated_value, None);
    assert!(stored.diagnostics.is_empty());
    assert_eq!(
        stored.presentation_address,
        FieldAddress::new("iron_sword", "damage")
    );

    let formula = &query.value()[1];
    assert_eq!(formula.stored_value, None);
    assert!(formula.formula_definition.is_some());
    assert_eq!(
        formula.calculated_value,
        Some(FormulaCalculationOutcome::Value(Number::new(40.0).unwrap()))
    );
    assert!(formula.diagnostics.is_empty());
    assert_eq!(
        formula.presentation_address,
        FieldAddress::new("iron_sword", "dps")
    );
}

#[test]
fn field_query_preserves_formula_failure_and_stable_subject_diagnostics() {
    let mut document = game_balance_document("game", "Game");
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert("attack_interval".into(), value(0.0));
    let session = ResidentWorkspaceSession::new(document_scope_id(), document);
    let dps = FieldRef::new("iron_sword", "dps");

    let query = session.query_fields(std::slice::from_ref(&dps)).unwrap();
    let projection = &query.value()[0];

    assert_eq!(projection.field, dps);
    assert_eq!(projection.stored_value, None);
    assert!(projection.formula_definition.is_some());
    assert_eq!(
        projection.calculated_value,
        Some(FormulaCalculationOutcome::Failure(
            CalculationFailure::DivisionByZero
        ))
    );
    assert_eq!(projection.diagnostics.len(), 1);
    assert_eq!(
        projection.diagnostics[0].code,
        diagnostic_codes::FORMULA_DIVISION_BY_ZERO
    );
}

#[test]
fn field_query_ignores_unrelated_ambiguous_presentation_keys() {
    let mut document = game_balance_document("game", "Game");
    document.entities.get_mut("tempered_blade").unwrap().key = "iron_sword".into();
    let session = ResidentWorkspaceSession::new(document_scope_id(), document);
    let damage = FieldRef::new("iron_sword", "damage");

    let query = session.query_fields(std::slice::from_ref(&damage)).unwrap();

    assert_eq!(query.value().len(), 1);
    assert_eq!(query.value()[0].field, damage);
    assert_eq!(
        query.value()[0].presentation_address,
        FieldAddress::new("iron_sword", "damage")
    );
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
    let (receipt, _, publication_time_calls) =
        execute_damage(&mut session, "resident-success", 45.0);

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
fn scalar_mutation_invalidates_changed_field_and_downstream_projection_at_new_revision() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let damage = FieldRef::new("iron_sword", "damage");
    let dps = FieldRef::new("iron_sword", "dps");
    let cached = session
        .query_fields(&[damage.clone(), dps.clone()])
        .unwrap();

    let (receipt, invalidation, _) = execute_damage(&mut session, "resident-invalidation", 45.0);

    assert_eq!(receipt.base_revision, invalidation.base_revision);
    assert_eq!(receipt.resulting_revision, invalidation.resulting_revision);
    assert_ne!(cached.revision(), &invalidation.resulting_revision);
    assert!(invalidation.entities.is_empty());
    assert_eq!(invalidation.fields, [damage]);
    assert_eq!(invalidation.affected_calculations, [dps]);
}

#[test]
fn revision_tag_deterministically_identifies_stale_cached_projection() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let damage = FieldRef::new("iron_sword", "damage");
    let cached = session.query_fields(std::slice::from_ref(&damage)).unwrap();

    execute_damage(&mut session, "resident-stale-projection", 45.0);
    let current = session.query_fields(std::slice::from_ref(&damage)).unwrap();

    assert!(cached.is_stale_against(session.revision()));
    assert!(!current.is_stale_against(session.revision()));
}

#[test]
fn rename_projection_preserves_stable_subject_and_changes_presentation_address() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let dps = FieldRef::new("iron_sword", "dps");
    let before = session.query_fields(std::slice::from_ref(&dps)).unwrap();
    let snapshot = session.export_snapshot();
    let candidate = rename_entity(snapshot.document(), "iron_sword", "moonblade")
        .unwrap()
        .document;
    let mut time = FixedTrustedTime { calls: 0 };

    let (resulting_revision, invalidation) = {
        let mut publication = session.publication_authority(&mut time);
        let resulting_revision = publication
            .publish_if_current(
                snapshot.document_scope(),
                snapshot.revision(),
                candidate,
                |_| Some(()),
            )
            .unwrap()
            .2;
        let invalidation = publication
            .projection_invalidation_for(snapshot.revision(), &resulting_revision)
            .unwrap()
            .clone();
        (resulting_revision, invalidation)
    };
    let after = session.query_fields(std::slice::from_ref(&dps)).unwrap();

    assert_eq!(before.value()[0].field, dps);
    assert_eq!(after.value()[0].field, dps);
    assert_eq!(
        before.value()[0].presentation_address,
        FieldAddress::new("iron_sword", "dps")
    );
    assert_eq!(
        after.value()[0].presentation_address,
        FieldAddress::new("moonblade", "dps")
    );
    assert_eq!(invalidation.base_revision, *snapshot.revision());
    assert_eq!(invalidation.resulting_revision, resulting_revision);
    assert_eq!(invalidation.entities, [EntityId::from("iron_sword")]);
    assert_eq!(
        invalidation.fields,
        [
            FieldRef::new("iron_sword", "attack_interval"),
            FieldRef::new("iron_sword", "damage"),
            FieldRef::new("iron_sword", "dps"),
            FieldRef::new("iron_sword", "name"),
            FieldRef::new("iron_sword", "price"),
        ]
    );
    assert!(invalidation.affected_calculations.is_empty());
    assert!(before.is_stale_against(&resulting_revision));
    assert_eq!(after.revision(), &resulting_revision);
}

#[test]
fn field_rename_invalidates_schema_bound_presentations_without_recomputing_dependents() {
    let mut session =
        ResidentWorkspaceSession::new(document_scope_id(), game_balance_document("game", "Game"));
    let damage = FieldRef::new("iron_sword", "damage");
    let snapshot = session.export_snapshot();
    let candidate = rename_field(snapshot.document(), "weapons", "damage", "power")
        .unwrap()
        .document;
    let mut time = FixedTrustedTime { calls: 0 };

    let (resulting_revision, invalidation) = {
        let mut publication = session.publication_authority(&mut time);
        let resulting_revision = publication
            .publish_if_current(
                snapshot.document_scope(),
                snapshot.revision(),
                candidate,
                |_| Some(()),
            )
            .unwrap()
            .2;
        let invalidation = publication
            .projection_invalidation_for(snapshot.revision(), &resulting_revision)
            .unwrap()
            .clone();
        (resulting_revision, invalidation)
    };
    let after = session.query_fields(std::slice::from_ref(&damage)).unwrap();

    assert_eq!(invalidation.base_revision, *snapshot.revision());
    assert_eq!(invalidation.resulting_revision, resulting_revision);
    assert!(invalidation.entities.is_empty());
    assert_eq!(invalidation.fields, [damage]);
    assert!(invalidation.affected_calculations.is_empty());
    assert_eq!(
        after.value()[0].presentation_address,
        FieldAddress::new("iron_sword", "power")
    );
}

#[test]
fn invalidation_follows_transitive_graph_when_calculated_outputs_do_not_change() {
    let damage = FieldRef::new("iron_sword", "damage");
    let dps = FieldRef::new("iron_sword", "dps");
    let matches = FieldRef::new("shop", "matches_for_sword");
    let mut document = game_balance_document("game", "Game");
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(
            "dps".into(),
            Value::Formula(Expression::Multiply {
                left: Box::new(Expression::Reference(damage.clone())),
                right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
            }),
        );
    document.entities.get_mut("shop").unwrap().fields.insert(
        "matches_for_sword".into(),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(dps.clone())),
            right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
        }),
    );
    let mut session = ResidentWorkspaceSession::new(document_scope_id(), document);

    let (receipt, invalidation, _) = execute_damage(&mut session, "resident-equal-output", 45.0);

    assert!(receipt.formula_impacts.is_empty());
    assert_eq!(invalidation.fields, [damage]);
    assert_eq!(invalidation.affected_calculations, [dps, matches]);
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
