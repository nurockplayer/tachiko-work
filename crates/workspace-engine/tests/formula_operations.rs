mod common;

use std::collections::BTreeSet;

use common::game_balance_document;
use tachiko_workspace_engine::{
    Document, DocumentId, Expression, FieldId, FieldRef, Number, Value,
    formula_operations::{
        FormulaCalculationOutcome, FormulaOperationError, FormulaReasoningOutcome,
        FormulaUpdateRequest, NumberOverride, ScenarioEnvelopeError, ScenarioOutcome,
        ScenarioOverrideFailure, ScenarioRequest, ScenarioTargetOutcome, ValidatorConfiguration,
    },
    patch_lifecycle::{
        ApprovalId, ApprovalRequest, ApprovalStatus, AuthorizationAction, AuthorizationDomainId,
        AuthorizationPolicyVersion, DocumentScopeId, Grant, GrantId, GrantRequirement,
        MutationClass, OperationFamily, PatchLifecycle, PatchLifecycleError, PolicyMeaningId,
        PrincipalId, PrincipalKind, ProposalId, ScopedSemanticSubject, SemanticApiContract,
        SemanticCommand, SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant,
    },
};

const NOW: TrustedInstant = TrustedInstant::new(10);

fn principal(value: &str) -> PrincipalId {
    PrincipalId::from(value)
}

fn revision(value: &str) -> SemanticRevision {
    SemanticRevision::from(value)
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
        document_scope_id(),
        DocumentId::from("game"),
        SemanticScope::EntityField {
            entity: entity.into(),
            schema: schema.into(),
            field: field.into(),
        },
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
    for (id, kind) in [
        ("authority", PrincipalKind::Human),
        ("agent", PrincipalKind::Delegated),
        ("reviewer", PrincipalKind::Human),
    ] {
        lifecycle.register_principal(principal(id), kind).unwrap();
    }
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

fn query_requirement(family: OperationFamily, scope: ScopedSemanticSubject) -> GrantRequirement {
    GrantRequirement::query(family, scope)
}

fn mutation_requirement(
    action: AuthorizationAction,
    family: OperationFamily,
    mutation_class: MutationClass,
    scope: ScopedSemanticSubject,
) -> GrantRequirement {
    GrantRequirement::mutation(action, family, mutation_class, scope).unwrap()
}

#[test]
fn formula_reasoning_returns_bound_meaning_dependencies_dependents_and_validation() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "reasoning-query",
        "agent",
        vec![query_requirement(
            OperationFamily::FormulaReasoning,
            document_scope(),
        )],
    );

    let result = lifecycle
        .query_formula_reasoning(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &FieldRef::new("iron_sword", "dps"),
            &principal("agent"),
            NOW,
        )
        .unwrap();

    assert_eq!(result.context.source_revision(), &revision("r1"));
    assert_eq!(
        result.context.validator_configuration(),
        ValidatorConfiguration::WorkspaceFull
    );
    let FormulaReasoningOutcome::Formula(facts) = result.outcome else {
        panic!("expected formula facts");
    };
    assert!(matches!(facts.expression, Expression::Divide { .. }));
    assert_eq!(
        facts.direct_inputs,
        [
            FieldRef::new("iron_sword", "attack_interval"),
            FieldRef::new("iron_sword", "damage"),
        ]
    );
    assert!(facts.direct_dependents.is_empty());
    assert!(facts.affected_subjects.is_empty());
    assert_eq!(
        facts.calculation,
        FormulaCalculationOutcome::Value(Number::new(40.0).unwrap())
    );
    assert!(
        facts
            .validation_report
            .expect("document-wide Query exposes the complete report")
            .is_valid()
    );
}

#[test]
fn formula_reasoning_requires_query_coverage_for_every_exposed_fact() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "target-only",
        "agent",
        vec![query_requirement(
            OperationFamily::FormulaReasoning,
            field_scope("iron_sword", "weapons", "dps"),
        )],
    );

    let error = lifecycle
        .query_formula_reasoning(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &FieldRef::new("iron_sword", "dps"),
            &principal("agent"),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        FormulaOperationError::Lifecycle(PatchLifecycleError::DisclosureDenied)
    ));
}

#[test]
fn scenario_is_repeatable_and_leaves_exact_source_state_unchanged() {
    let document = game_balance_document("game", "Game");
    let original = document.clone();
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "scenario-query",
        "agent",
        vec![query_requirement(
            OperationFamily::NumberOverrideScenario,
            document_scope(),
        )],
    );
    let request = ScenarioRequest::new(
        vec![NumberOverride::new(
            FieldRef::new("iron_sword", "damage"),
            45.0,
        )],
        vec![
            FieldRef::new("iron_sword", "dps"),
            FieldRef::new("iron_sword", "dps"),
        ],
    );

    let first = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &request,
            &principal("agent"),
            NOW,
        )
        .unwrap();
    let second = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &request,
            &principal("agent"),
            NOW,
        )
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(document, original);
    assert_eq!(first.normalized_overrides.len(), 1);
    let ScenarioOutcome::Evaluated(evaluation) = first.outcome else {
        panic!("expected evaluated scenario");
    };
    assert!(
        evaluation
            .baseline_validation
            .as_ref()
            .expect("document-wide Query exposes baseline validation")
            .is_valid()
    );
    assert!(
        evaluation
            .candidate_validation
            .as_ref()
            .expect("document-wide Query exposes candidate validation")
            .is_valid()
    );
    assert_eq!(evaluation.targets.len(), 1, "duplicate targets normalize");
    let ScenarioTargetOutcome::Formula(comparison) = &evaluation.targets[0].outcome else {
        panic!("expected formula comparison");
    };
    assert_eq!(
        comparison.baseline,
        FormulaCalculationOutcome::Value(Number::new(40.0).unwrap())
    );
    assert_eq!(
        comparison.candidate,
        FormulaCalculationOutcome::Value(Number::new(50.0).unwrap())
    );
    let impact = evaluation.impact.expect("valid scenarios have diff impact");
    assert_eq!(
        impact.changed_fields,
        [
            FieldRef::new("iron_sword", "damage"),
            FieldRef::new("iron_sword", "dps"),
        ]
    );
    assert_eq!(impact.affected_fields, [FieldRef::new("iron_sword", "dps")]);
}

#[test]
fn scenario_envelope_rejects_duplicates_and_non_finite_values_before_lookup() {
    let document = game_balance_document("game", "Game");
    let lifecycle = lifecycle();
    let missing = FieldRef::new("missing", "damage");
    let duplicate = ScenarioRequest::new(
        vec![
            NumberOverride::new(missing.clone(), 1.0),
            NumberOverride::new(missing.clone(), 2.0),
        ],
        vec![],
    );

    let duplicate_error = lifecycle
        .query_number_override_scenario(
            &DocumentScopeId::from("wrong-occurrence"),
            &document,
            (
                &revision("untrusted-request-label"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &duplicate,
            &principal("agent"),
            NOW,
        )
        .unwrap_err();
    assert!(matches!(
        duplicate_error,
        FormulaOperationError::ScenarioEnvelope(ScenarioEnvelopeError::DuplicateOverride {
            target
        }) if target == missing
    ));

    let non_finite = ScenarioRequest::new(
        vec![NumberOverride::new(
            FieldRef::new("missing", "damage"),
            f64::INFINITY,
        )],
        vec![],
    );
    let non_finite_error = lifecycle
        .query_number_override_scenario(
            &DocumentScopeId::from("wrong-occurrence"),
            &document,
            (
                &revision("untrusted-request-label"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &non_finite,
            &principal("agent"),
            NOW,
        )
        .unwrap_err();
    assert!(matches!(
        non_finite_error,
        FormulaOperationError::ScenarioEnvelope(ScenarioEnvelopeError::NonFiniteOverride { .. })
    ));
}

#[test]
fn scenario_without_source_query_returns_no_evaluated_source_context() {
    let document = game_balance_document("game", "Game");
    let lifecycle = lifecycle();

    let error = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(vec![], vec![FieldRef::new("iron_sword", "dps")]),
            &principal("agent"),
            NOW,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        FormulaOperationError::Lifecycle(PatchLifecycleError::DisclosureDenied)
    ));
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn scenario_returns_structured_override_and_target_outcomes_without_publication() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "scenario-query",
        "agent",
        vec![query_requirement(
            OperationFamily::NumberOverrideScenario,
            document_scope(),
        )],
    );

    let missing_override = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(
                vec![NumberOverride::new(FieldRef::new("missing", "damage"), 1.0)],
                vec![FieldRef::new("iron_sword", "dps")],
            ),
            &principal("agent"),
            NOW,
        )
        .unwrap();
    assert!(matches!(
        missing_override.outcome,
        ScenarioOutcome::InvalidOverrides(ref failures)
            if failures == &[ScenarioOverrideFailure::UnresolvedTarget {
                target: FieldRef::new("missing", "damage")
            }]
    ));

    let wrong_kind = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(
                vec![NumberOverride::new(FieldRef::new("iron_sword", "dps"), 1.0)],
                vec![],
            ),
            &principal("agent"),
            NOW,
        )
        .unwrap();
    assert!(matches!(
        wrong_kind.outcome,
        ScenarioOutcome::InvalidOverrides(ref failures)
            if matches!(failures.as_slice(), [ScenarioOverrideFailure::UnsupportedKind { target, .. }]
                if target == &FieldRef::new("iron_sword", "dps"))
    ));

    let targets = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(
                vec![],
                vec![
                    FieldRef::new("missing", "value"),
                    FieldRef::new("iron_sword", "damage"),
                ],
            ),
            &principal("agent"),
            NOW,
        )
        .unwrap();
    let ScenarioOutcome::Evaluated(targets) = targets.outcome else {
        panic!("expected target outcomes");
    };
    assert!(matches!(
        targets.targets[0].outcome,
        ScenarioTargetOutcome::UnsupportedKind { .. }
    ));
    assert!(matches!(
        targets.targets[1].outcome,
        ScenarioTargetOutcome::UnresolvedTarget
    ));
    assert_eq!(document, game_balance_document("game", "Game"));
}

#[test]
fn scenario_formula_failure_uses_authoritative_validation_and_has_no_diff_impact() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "scenario-query",
        "agent",
        vec![query_requirement(
            OperationFamily::NumberOverrideScenario,
            document_scope(),
        )],
    );
    let result = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(
                vec![NumberOverride::new(
                    FieldRef::new("iron_sword", "attack_interval"),
                    0.0,
                )],
                vec![FieldRef::new("iron_sword", "dps")],
            ),
            &principal("agent"),
            NOW,
        )
        .unwrap();

    let ScenarioOutcome::Evaluated(evaluation) = result.outcome else {
        panic!("expected evaluated failure");
    };
    assert!(
        !evaluation
            .candidate_validation
            .as_ref()
            .expect("document-wide Query exposes candidate validation")
            .is_valid()
    );
    assert!(evaluation.impact.is_none());
    let ScenarioTargetOutcome::Formula(comparison) = &evaluation.targets[0].outcome else {
        panic!("expected formula outcome");
    };
    assert!(matches!(
        comparison.candidate,
        FormulaCalculationOutcome::Failure(
            tachiko_workspace_engine::CalculationFailure::DivisionByZero
        )
    ));
    assert_eq!(
        document.entities["iron_sword"].fields["attack_interval"],
        Value::Number(Number::new(0.9).unwrap())
    );
}

#[test]
fn formula_update_binds_before_proposal_and_uses_formula_authority() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-formula",
        "agent",
        vec![
            query_requirement(OperationFamily::FormulaUpdate, document_scope()),
            mutation_requirement(
                AuthorizationAction::Propose,
                OperationFamily::FormulaUpdate,
                MutationClass::Formula,
                document_scope(),
            ),
        ],
    );
    let patch = lifecycle
        .propose_formula_update(
            &document_scope_id(),
            &document,
            &revision("r1"),
            FormulaUpdateRequest::new(
                ProposalId::from("formula-one"),
                revision("r1"),
                FieldRef::new("iron_sword", "dps"),
                "[iron_sword.damage] + 5",
                principal("agent"),
            ),
            NOW,
        )
        .unwrap();

    assert_eq!(patch.exact_change().base_revision(), &revision("r1"));
    let SemanticPatchBody::Command(SemanticCommand::FormulaUpdate(command)) =
        patch.exact_change().body()
    else {
        panic!("expected typed FormulaUpdate command");
    };
    assert_eq!(command.target(), &FieldRef::new("iron_sword", "dps"));
    assert_eq!(
        command.references(),
        &BTreeSet::from([FieldRef::new("iron_sword", "damage")])
    );
    assert!(matches!(command.expression(), Expression::Add { .. }));
}

#[test]
fn formula_update_admission_and_gate_failures_preserve_proposal_boundary() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-formula",
        "agent",
        vec![
            query_requirement(OperationFamily::FormulaUpdate, document_scope()),
            mutation_requirement(
                AuthorizationAction::Propose,
                OperationFamily::FormulaUpdate,
                MutationClass::Formula,
                document_scope(),
            ),
        ],
    );

    let invalid_id = ProposalId::from("invalid-authoring");
    let invalid = lifecycle
        .propose_formula_update(
            &document_scope_id(),
            &document,
            &revision("r1"),
            FormulaUpdateRequest::new(
                invalid_id.clone(),
                revision("r1"),
                FieldRef::new("iron_sword", "dps"),
                "min(1,",
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();
    assert!(matches!(
        invalid,
        PatchLifecycleError::CommandRejected { .. }
    ));
    assert!(matches!(
        lifecycle.proposal_history(&invalid_id),
        Err(PatchLifecycleError::ProposalNotFound)
    ));

    let cycle_id = ProposalId::from("cycle-update");
    let cycle = lifecycle
        .propose_formula_update(
            &document_scope_id(),
            &document,
            &revision("r1"),
            FormulaUpdateRequest::new(
                cycle_id.clone(),
                revision("r1"),
                FieldRef::new("iron_sword", "dps"),
                "[iron_sword.dps] + 1",
                principal("agent"),
            ),
            NOW,
        )
        .unwrap_err();
    assert!(matches!(
        cycle,
        PatchLifecycleError::ValidationFailed { .. }
    ));
    assert!(lifecycle.proposal_history(&cycle_id).is_ok());
    assert!(lifecycle.execution_receipts().is_empty());
    assert_eq!(document, game_balance_document("game", "Game"));
}

#[test]
fn propose_without_query_reveals_nothing_and_issues_no_formula_proposal() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "propose-only",
        "agent",
        vec![mutation_requirement(
            AuthorizationAction::Propose,
            OperationFamily::FormulaUpdate,
            MutationClass::Formula,
            document_scope(),
        )],
    );
    let proposal_id = ProposalId::from("no-query");
    let request = FormulaUpdateRequest::new(
        proposal_id.clone(),
        revision("r1"),
        FieldRef::new("iron_sword", "dps"),
        "[missing.damage] + 1",
        principal("agent"),
    );

    let error = lifecycle
        .propose_formula_update(
            &document_scope_id(),
            &document,
            &revision("r1"),
            request,
            NOW,
        )
        .unwrap_err();

    assert!(matches!(error, PatchLifecycleError::DisclosureDenied));
    assert!(lifecycle.execution_receipts().is_empty());
    assert!(matches!(
        lifecycle.proposal_history(&proposal_id),
        Err(PatchLifecycleError::ProposalNotFound)
    ));
}

struct TestPublication {
    document: Document,
    revision: SemanticRevision,
}

impl SemanticPublicationAuthority for TestPublication {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        (
            document_scope_id(),
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
    ) -> Result<
        (DocumentScopeId, Document, SemanticRevision, Authorization),
        SemanticPublicationError,
    > {
        if expected_document_scope != &document_scope_id() {
            return Err(SemanticPublicationError::DocumentScopeMismatch);
        }
        if expected_revision != &self.revision {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(11))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        self.document = candidate;
        self.revision = revision("r2");
        Ok((
            document_scope_id(),
            self.document.clone(),
            self.revision.clone(),
            authorization,
        ))
    }
}

#[test]
fn delegated_formula_update_executes_with_existing_exact_human_approval() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "agent-formula",
        "agent",
        vec![
            query_requirement(OperationFamily::FormulaUpdate, document_scope()),
            mutation_requirement(
                AuthorizationAction::Propose,
                OperationFamily::FormulaUpdate,
                MutationClass::Formula,
                document_scope(),
            ),
            mutation_requirement(
                AuthorizationAction::Execute,
                OperationFamily::FormulaUpdate,
                MutationClass::Formula,
                document_scope(),
            ),
        ],
    );
    grant(
        &mut lifecycle,
        "reviewer-formula",
        "reviewer",
        vec![
            query_requirement(OperationFamily::FormulaUpdate, document_scope()),
            mutation_requirement(
                AuthorizationAction::Approve,
                OperationFamily::FormulaUpdate,
                MutationClass::Formula,
                document_scope(),
            ),
        ],
    );
    let patch = lifecycle
        .propose_formula_update(
            &document_scope_id(),
            &document,
            &revision("r1"),
            FormulaUpdateRequest::new(
                ProposalId::from("approved-formula"),
                revision("r1"),
                FieldRef::new("iron_sword", "dps"),
                "[iron_sword.damage] + 5",
                principal("agent"),
            ),
            NOW,
        )
        .unwrap();
    lifecycle
        .preview(
            &document_scope_id(),
            &document,
            &revision("r1"),
            patch.id(),
            &principal("reviewer"),
            NOW,
        )
        .unwrap();
    let approval = lifecycle
        .approve(
            &document_scope_id(),
            &document,
            &revision("r1"),
            ApprovalRequest::new(
                ApprovalId::from("formula-approval"),
                patch.id().clone(),
                principal("reviewer"),
                principal("agent"),
                TrustedInstant::new(20),
            ),
            NOW,
        )
        .unwrap();
    let mut publication = TestPublication {
        document: document.clone(),
        revision: revision("r1"),
    };

    let receipt = lifecycle
        .execute(
            patch.id(),
            Some(approval.id()),
            &principal("agent"),
            &mut publication,
            TrustedInstant::new(11),
        )
        .unwrap();

    assert_eq!(
        publication.document.entities["iron_sword"].fields["dps"],
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(
                FieldRef::new("iron_sword", "damage",)
            )),
            right: Box::new(Expression::Number(Number::new(5.0).unwrap())),
        })
    );
    assert_eq!(receipt.approval.unwrap().status, ApprovalStatus::Consumed);
    assert_eq!(document, game_balance_document("game", "Game"));
}

#[test]
fn scenario_requested_target_without_query_coverage_gets_one_safe_denial() {
    let document = game_balance_document("game", "Game");
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "scenario-fields",
        "agent",
        vec![
            query_requirement(
                OperationFamily::NumberOverrideScenario,
                field_scope("iron_sword", "weapons", "damage"),
            ),
            query_requirement(
                OperationFamily::NumberOverrideScenario,
                field_scope("iron_sword", "weapons", "attack_interval"),
            ),
            query_requirement(
                OperationFamily::NumberOverrideScenario,
                field_scope("iron_sword", "weapons", "dps"),
            ),
        ],
    );
    let result = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(
                vec![NumberOverride::new(
                    FieldRef::new("iron_sword", "damage"),
                    45.0,
                )],
                vec![
                    FieldRef::new("iron_sword", "dps"),
                    FieldRef::new("missing", "secret"),
                ],
            ),
            &principal("agent"),
            NOW,
        )
        .unwrap();

    let ScenarioOutcome::Evaluated(evaluation) = result.outcome else {
        panic!("expected evaluated scenario");
    };
    assert_eq!(evaluation.targets.len(), 2);
    assert!(matches!(
        evaluation.targets[0].outcome,
        ScenarioTargetOutcome::Formula(_)
    ));
    assert!(matches!(
        evaluation.targets[1].outcome,
        ScenarioTargetOutcome::DisclosureDenied
    ));
}

#[test]
fn scenario_source_cycle_returns_structured_formula_failure_without_mutation() {
    let mut document = game_balance_document("game", "Game");
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(
            FieldId::from("dps"),
            Value::Formula(Expression::Reference(FieldRef::new("iron_sword", "dps"))),
        );
    let original = document.clone();
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "scenario-query",
        "agent",
        vec![query_requirement(
            OperationFamily::NumberOverrideScenario,
            document_scope(),
        )],
    );

    let result = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(vec![], vec![FieldRef::new("iron_sword", "dps")]),
            &principal("agent"),
            NOW,
        )
        .unwrap();

    let ScenarioOutcome::Evaluated(evaluation) = result.outcome else {
        panic!("expected structured source failure");
    };
    let ScenarioTargetOutcome::Formula(comparison) = &evaluation.targets[0].outcome else {
        panic!("expected formula failure");
    };
    assert!(matches!(
        comparison.baseline,
        FormulaCalculationOutcome::Failure(
            tachiko_workspace_engine::CalculationFailure::Cycle { .. }
        )
    ));
    assert_eq!(document, original);
    assert!(lifecycle.execution_receipts().is_empty());
}

#[test]
fn scenario_cycle_failure_does_not_expose_uncovered_members() {
    let mut document = game_balance_document("game", "Game");
    let fields = document.entities.get_mut("iron_sword").unwrap();
    for (target, dependency) in [
        ("dps", "damage"),
        ("damage", "attack_interval"),
        ("attack_interval", "price"),
        ("price", "dps"),
    ] {
        fields.fields.insert(
            FieldId::from(target),
            Value::Formula(Expression::Reference(FieldRef::new(
                "iron_sword",
                dependency,
            ))),
        );
    }
    let mut lifecycle = lifecycle();
    grant(
        &mut lifecycle,
        "partial-cycle-query",
        "agent",
        ["dps", "damage", "price"]
            .into_iter()
            .map(|field| {
                query_requirement(
                    OperationFamily::NumberOverrideScenario,
                    field_scope("iron_sword", "weapons", field),
                )
            })
            .collect(),
    );

    let result = lifecycle
        .query_number_override_scenario(
            &document_scope_id(),
            &document,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &ScenarioRequest::new(vec![], vec![FieldRef::new("iron_sword", "dps")]),
            &principal("agent"),
            NOW,
        )
        .unwrap();

    let ScenarioOutcome::Evaluated(evaluation) = result.outcome else {
        panic!("expected evaluated scenario");
    };
    assert!(matches!(
        evaluation.targets[0].outcome,
        ScenarioTargetOutcome::DisclosureDenied
    ));
}
