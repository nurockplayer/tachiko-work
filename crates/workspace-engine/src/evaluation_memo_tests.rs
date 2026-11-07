//! One evaluation serves propose, preview and execute while the inputs are unchanged.
use std::collections::{BTreeMap, BTreeSet};

use super::{
    AssociatedWriteRequirement, AuthorizationAction, Document, DocumentScopeId, Entity,
    FieldConstraint, FieldDefinition, FieldRef, FieldType, Grant, GrantRequirement, MutationClass,
    OperationFamily, PatchLifecycle, PatchLifecycleError, PrincipalKind, ProposalId,
    ScopedSemanticSubject, SemanticCommand, SemanticPatch, SemanticPatchBody,
    SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision, SemanticScope,
    TrustedInstant, V2ReceiptEvidence, Value,
};
use crate::{
    Expression, Number, Schema, WorkspaceError, evaluation_counters,
    resident_session::ResidentWorkspaceSession,
};

fn text_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        field_type: FieldType::Text,
        required: true,
        constraint: FieldConstraint::None,
    }
}

fn document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        "s".into(),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([("f".into(), text_field("f")), ("g".into(), text_field("g"))]),
        },
    );
    document.entities.insert(
        "e1".into(),
        Entity {
            id: "e1".into(),
            key: "row".into(),
            schema: "s".into(),
            fields: BTreeMap::from([
                ("f".into(), Value::Text("old".into())),
                ("g".into(), Value::Text("g0".into())),
            ]),
        },
    );
    document
}

fn named_document() -> Document {
    let mut document = document();
    document.schemas.get_mut("s").unwrap().display_name = Some("Table".to_owned());
    document
}

fn set_f(value: &str) -> SemanticPatchBody {
    SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "f"),
        value: Value::Text(value.to_owned()),
    })
}

fn lifecycle(v2: bool) -> PatchLifecycle {
    let mut lifecycle = if v2 {
        PatchLifecycle::new_v2(
            "domain".into(),
            "scope".into(),
            "doc".into(),
            "api".into(),
            "policy".into(),
            "meaning".into(),
        )
    } else {
        PatchLifecycle::new(
            "domain".into(),
            "scope".into(),
            "doc".into(),
            "api".into(),
            "policy".into(),
            "meaning".into(),
        )
    };
    lifecycle
        .register_principal("human".into(), PrincipalKind::Human)
        .unwrap();
    let field_scope = ScopedSemanticSubject::new(
        "scope".into(),
        "doc".into(),
        SemanticScope::EntityField {
            entity: "e1".into(),
            schema: "s".into(),
            field: "f".into(),
        },
    );
    let document_scope =
        ScopedSemanticSubject::new("scope".into(), "doc".into(), SemanticScope::Document);
    let mut requirements = vec![
        GrantRequirement::query(OperationFamily::SetFieldValue, document_scope),
        GrantRequirement::query(OperationFamily::SetFieldValue, field_scope.clone()),
    ];
    for action in [AuthorizationAction::Propose, AuthorizationAction::Execute] {
        requirements.push(
            GrantRequirement::mutation(
                action,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                field_scope.clone(),
            )
            .unwrap(),
        );
    }
    lifecycle
        .provision_grant(Grant::new(
            "grant".into(),
            "human".into(),
            "human".into(),
            requirements,
            None,
        ))
        .unwrap();
    lifecycle
}

fn propose(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    id: &str,
    body: SemanticPatchBody,
) -> Result<ProposalId, PatchLifecycleError> {
    lifecycle
        .propose(
            &"scope".into(),
            document,
            &"r1".into(),
            super::ProposalRequest::new(id.into(), "r1".into(), body, "human".into()),
            TrustedInstant::new(1),
        )
        .map(|patch| patch.id().clone())
}

struct Host {
    snapshot: Document,
    revision: SemanticRevision,
    published: Option<Document>,
    publications: usize,
}

impl Host {
    fn new(snapshot: Document) -> Self {
        Self {
            snapshot,
            revision: "r1".into(),
            published: None,
            publications: 0,
        }
    }
}

impl SemanticPublicationAuthority for Host {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        ("scope".into(), self.snapshot.clone(), self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, Document, SemanticRevision, A), SemanticPublicationError> {
        if expected_scope != &DocumentScopeId::from("scope") || expected_revision != &self.revision
        {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(30))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        self.publications += 1;
        self.published = Some(candidate.clone());
        Ok(("scope".into(), candidate, "r2".into(), authorization))
    }
}

fn with_f_set(mut document: Document, value: &str) -> Document {
    document
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("f".into(), Value::Text(value.to_owned()));
    document
}

#[test]
fn propose_preview_execute_evaluates_once() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let preview = lifecycle
        .preview(
            &"scope".into(),
            &document,
            &"r1".into(),
            &proposal,
            &"human".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    let mut host = Host::new(document.clone());
    let receipt = lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap();

    assert_eq!(lifecycle.evaluations.get(), 1);
    assert_eq!(host.publications, 1);
    assert_eq!(host.published, Some(with_f_set(document, "new")));
    assert_eq!(receipt.semantic_changes, preview.semantic_changes);
    assert!(receipt.verified);
    assert_eq!(receipt.resulting_revision, SemanticRevision::from("r2"));
    assert!(lifecycle.evaluation_memo.is_none());
}

#[test]
fn preview_with_a_different_document_reevaluates_without_replacing_the_memo() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    // A different document under the same revision: v1 rejects display names
    // during final evaluation, after planning the same commands.
    let other = named_document();

    let error = lifecycle
        .preview(
            &"scope".into(),
            &other,
            &"r1".into(),
            &proposal,
            &"human".into(),
            TrustedInstant::new(10),
        )
        .unwrap_err();

    assert!(
        matches!(error, PatchLifecycleError::CommandRejected { .. }),
        "{error:?}"
    );
    assert_eq!(lifecycle.evaluations.get(), 2);
    lifecycle
        .preview(
            &"scope".into(),
            &document,
            &"r1".into(),
            &proposal,
            &"human".into(),
            TrustedInstant::new(11),
        )
        .unwrap();
    assert_eq!(lifecycle.evaluations.get(), 2);
}

#[test]
fn execute_with_a_changed_snapshot_reevaluates_the_new_base() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let mut changed = document.clone();
    changed
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("g".into(), Value::Text("g1".into()));
    let mut host = Host::new(changed.clone());
    let receipt = lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap();

    assert_eq!(lifecycle.evaluations.get(), 2);
    assert_eq!(host.publications, 1);
    assert_eq!(host.published, Some(with_f_set(changed, "new")));
    assert!(receipt.verified);
}

#[test]
fn failed_propose_leaves_no_retained_evaluation() {
    let mut lifecycle = lifecycle(false);
    let error = propose(&mut lifecycle, &named_document(), "proposal", set_f("new")).unwrap_err();

    assert!(
        matches!(error, PatchLifecycleError::CommandRejected { .. }),
        "{error:?}"
    );
    assert_eq!(lifecycle.evaluations.get(), 1);
    assert!(lifecycle.evaluation_memo.is_none());
}

#[test]
fn execute_clears_the_retained_evaluation_whatever_the_outcome() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    assert!(lifecycle.evaluation_memo.is_some());

    let mut stale = Host::new(document.clone());
    stale.revision = "r9".into();
    let error = lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut stale,
            TrustedInstant::new(20),
        )
        .unwrap_err();
    assert!(matches!(error, PatchLifecycleError::Stale), "{error:?}");
    assert!(lifecycle.evaluation_memo.is_none());

    let next = propose(&mut lifecycle, &document, "next", set_f("new")).unwrap();
    assert!(lifecycle.evaluation_memo.is_some());
    let mut host = Host::new(document);
    lifecycle
        .execute(
            &next,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(21),
        )
        .unwrap();
    assert!(lifecycle.evaluation_memo.is_none());
}

#[test]
fn revoked_grant_still_denies_execution_with_a_retained_evaluation() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    assert!(lifecycle.evaluation_memo.is_some());
    lifecycle.revoke_grant(&"grant".into()).unwrap();

    let mut host = Host::new(document);
    let error = lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap_err();

    assert!(
        matches!(
            error,
            PatchLifecycleError::InsufficientCapability {
                action: AuthorizationAction::Execute
            }
        ),
        "{error:?}"
    );
    assert_eq!(host.publications, 0);
    assert_eq!(lifecycle.evaluations.get(), 1);
}

#[test]
fn published_receipt_reports_the_installed_document() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let mut host = Host::new(document);
    let receipt = lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap();
    let installed = host.published.expect("the host installs the candidate");

    assert_eq!(
        receipt.validation_report,
        Some(crate::validation_report(&installed))
    );
}

#[test]
fn published_v2_receipt_reports_the_installed_document() {
    let mut lifecycle = lifecycle(true);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let mut host = Host::new(document);
    let receipt = lifecycle
        .execute_v2(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap();
    let installed = host.published.expect("the host installs the candidate");

    assert_eq!(
        receipt.validation_report,
        Some(crate::validation_report(&installed))
    );
}

struct InstallingHost {
    snapshot: Document,
    revision: SemanticRevision,
    installed: Document,
}

impl SemanticPublicationAuthority for InstallingHost {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        ("scope".into(), self.snapshot.clone(), self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        _candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, Document, SemanticRevision, A), SemanticPublicationError> {
        if expected_scope != &DocumentScopeId::from("scope") || expected_revision != &self.revision
        {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(30))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        Ok((
            "scope".into(),
            self.installed.clone(),
            "r2".into(),
            authorization,
        ))
    }
}

#[test]
fn a_host_that_installs_another_document_fails_verification() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let mut host = InstallingHost {
        snapshot: document.clone(),
        revision: "r1".into(),
        installed: with_f_set(document, "different"),
    };

    let error = lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap_err();

    assert!(
        matches!(error, PatchLifecycleError::VerificationFailed),
        "{error:?}"
    );
}

#[test]
fn a_host_that_installs_another_document_fails_v2_verification() {
    let mut lifecycle = lifecycle(true);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let mut host = InstallingHost {
        snapshot: document.clone(),
        revision: "r1".into(),
        installed: with_f_set(document, "different"),
    };

    let error = lifecycle
        .execute_v2(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap_err();

    assert!(
        matches!(error, PatchLifecycleError::VerificationFailed),
        "{error:?}"
    );
}

#[test]
fn v2_propose_preview_execute_evaluates_once() {
    let mut lifecycle = lifecycle(true);
    let document = document();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    let preview = lifecycle
        .preview_v2(
            &"scope".into(),
            &document,
            &"r1".into(),
            &proposal,
            &"human".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    let mut host = Host::new(document);
    let receipt = lifecycle
        .execute_v2(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap();

    assert_eq!(lifecycle.evaluations.get(), 1);
    assert_eq!(
        receipt.evidence(),
        &V2ReceiptEvidence::Disclosed(preview.delta.clone())
    );
    assert_eq!(receipt.delta().unwrap(), &preview.delta);
    assert!(receipt.verified);
}

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        field_type: FieldType::Number,
        required: true,
        constraint: FieldConstraint::None,
    }
}

/// `f` is Text, `g` is a Number and `h` is a formula reading `g`.
fn formula_document() -> Document {
    let mut document = Document::empty("doc", "Sheet");
    document.schemas.insert(
        "s".into(),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("f".into(), text_field("f")),
                ("g".into(), number_field("g")),
                ("h".into(), number_field("h")),
            ]),
        },
    );
    document.entities.insert(
        "e1".into(),
        Entity {
            id: "e1".into(),
            key: "row".into(),
            schema: "s".into(),
            fields: BTreeMap::from([
                ("f".into(), Value::Text("old".into())),
                ("g".into(), Value::Number(Number::new(1.0).unwrap())),
                (
                    "h".into(),
                    Value::Formula(Expression::Reference(FieldRef::new("e1", "g"))),
                ),
            ]),
        },
    );
    document
}

/// The same document with `h` dividing by zero, so its base calculation fails.
fn failing_formula_document() -> Document {
    let mut document = formula_document();
    document.entities.get_mut("e1").unwrap().fields.insert(
        "h".into(),
        Value::Formula(Expression::Divide {
            left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
        }),
    );
    document
}

fn formula_lifecycle() -> PatchLifecycle {
    let mut lifecycle = lifecycle(false);
    let mut requirements = Vec::new();
    for field in ["f", "g", "h"] {
        let scope = ScopedSemanticSubject::new(
            "scope".into(),
            "doc".into(),
            SemanticScope::EntityField {
                entity: "e1".into(),
                schema: "s".into(),
                field: field.into(),
            },
        );
        requirements.push(GrantRequirement::query(
            OperationFamily::SetFieldValue,
            scope.clone(),
        ));
        for action in [AuthorizationAction::Propose, AuthorizationAction::Execute] {
            for mutation_class in [
                MutationClass::Value,
                MutationClass::Formula,
                MutationClass::Destructive,
            ] {
                requirements.push(
                    GrantRequirement::mutation(
                        action,
                        OperationFamily::SetFieldValue,
                        mutation_class,
                        scope.clone(),
                    )
                    .unwrap(),
                );
            }
        }
    }
    lifecycle
        .provision_grant(Grant::new(
            "grant.fields".into(),
            "human".into(),
            "human".into(),
            requirements,
            None,
        ))
        .unwrap();
    lifecycle
}

fn set_f_text(value: &str) -> SemanticPatchBody {
    SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "f"),
        value: Value::Text(value.to_owned()),
    })
}

fn set_h_formula() -> SemanticPatchBody {
    SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "h"),
        value: Value::Formula(Expression::Number(Number::new(3.0).unwrap())),
    })
}

fn set_g_number(value: f64) -> SemanticPatchBody {
    SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "g"),
        value: Value::Number(Number::new(value).unwrap()),
    })
}

fn request(revision: &str, body: SemanticPatchBody) -> super::ProposalRequest {
    super::ProposalRequest::new("proposal".into(), revision.into(), body, "human".into())
}

fn propose_plain(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    revision: &str,
    body: SemanticPatchBody,
) -> Result<SemanticPatch, PatchLifecycleError> {
    lifecycle.propose(
        &"scope".into(),
        document,
        &revision.into(),
        request(revision, body),
        TrustedInstant::new(1),
    )
}

fn propose_with(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    revision: &str,
    body: SemanticPatchBody,
    prepared: &crate::PreparedEvaluation,
) -> Result<SemanticPatch, PatchLifecycleError> {
    lifecycle.propose_prepared(
        &"scope".into(),
        document,
        &revision.into(),
        request(revision, body),
        TrustedInstant::new(1),
        Some(prepared),
    )
}

fn assert_same_evaluation(left: &PatchLifecycle, right: &PatchLifecycle) {
    let left = &left.evaluation_memo.as_ref().unwrap().evaluated;
    let right = &right.evaluation_memo.as_ref().unwrap().evaluated;
    assert_eq!(left.document, right.document);
    assert_eq!(left.evidence, right.evidence);
    assert_eq!(left.formula_impacts, right.formula_impacts);
    assert_eq!(left.validation_report, right.validation_report);
    assert_eq!(left.footprint, right.footprint);
}

/// Acceptance 1 and 5: one value, formula or dependent-changing edit through
/// the prepared path evaluates the candidate once, never recalculates a
/// complete base, and yields the same evidence as `propose`.
#[test]
fn prepared_proposal_reuses_both_calculations_and_matches_propose() {
    for (body, label) in [
        (set_f_text("new"), "value"),
        (set_h_formula(), "formula"),
        (set_g_number(2.0), "dependent"),
    ] {
        let document = formula_document();
        let planner = formula_lifecycle();
        let candidate = planner.plan_commands(&document, &body).unwrap().0;
        let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
        let revision = session.revision().clone();

        let mut plain_lifecycle = formula_lifecycle();
        let plain_result = propose_plain(
            &mut plain_lifecycle,
            &document,
            revision.as_str(),
            body.clone(),
        );

        evaluation_counters::reset();
        let prepared = session.prepare_evaluation(candidate);
        let mut prepared_lifecycle = formula_lifecycle();
        let prepared_result = propose_with(
            &mut prepared_lifecycle,
            &document,
            revision.as_str(),
            body,
            &prepared,
        );

        assert!(plain_result.is_ok(), "{label}: {plain_result:?}");
        assert!(prepared_result.is_ok(), "{label}: {prepared_result:?}");
        assert_eq!(
            evaluation_counters::counts(),
            (1, 0),
            "{label}: the candidate is calculated once and the complete base never"
        );
        assert_eq!(prepared_lifecycle.evaluations.get(), 1);
        assert_same_evaluation(&plain_lifecycle, &prepared_lifecycle);
    }
}

/// Acceptance 2: a prepared evaluation of another candidate is ignored and the
/// proposal recalculates exactly like `propose`.
#[test]
fn a_prepared_evaluation_for_another_candidate_is_ignored() {
    let document = formula_document();
    let planner = formula_lifecycle();
    let prepared_candidate = planner
        .plan_commands(&document, &set_f_text("new"))
        .unwrap()
        .0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let revision = session.revision().clone();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(prepared_candidate);
    let mut lifecycle = formula_lifecycle();
    let result = propose_with(
        &mut lifecycle,
        &document,
        revision.as_str(),
        set_h_formula(),
        &prepared,
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        evaluation_counters::counts(),
        (2, 1),
        "the proposal recalculates its own candidate and base"
    );
    let mut plain_lifecycle = formula_lifecycle();
    propose_plain(
        &mut plain_lifecycle,
        &document,
        revision.as_str(),
        set_h_formula(),
    )
    .unwrap();
    assert_same_evaluation(&plain_lifecycle, &lifecycle);
}

/// Acceptance 2: a prepared evaluation from another revision is ignored.
#[test]
fn a_prepared_evaluation_for_another_revision_is_ignored() {
    let document = formula_document();
    let planner = formula_lifecycle();
    let candidate = planner
        .plan_commands(&document, &set_f_text("new"))
        .unwrap()
        .0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());

    let mut lifecycle = formula_lifecycle();
    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate);
    let result = lifecycle.propose_prepared(
        &"scope".into(),
        &document,
        &"r2".into(),
        super::ProposalRequest::new(
            "proposal".into(),
            "r2".into(),
            set_f_text("new"),
            "human".into(),
        ),
        TrustedInstant::new(1),
        Some(&prepared),
    );

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(evaluation_counters::counts(), (2, 1));
    let mut plain_lifecycle = formula_lifecycle();
    plain_lifecycle
        .propose(
            &"scope".into(),
            &document,
            &"r2".into(),
            super::ProposalRequest::new(
                "proposal".into(),
                "r2".into(),
                set_f_text("new"),
                "human".into(),
            ),
            TrustedInstant::new(1),
        )
        .unwrap();
    assert_same_evaluation(&plain_lifecycle, &lifecycle);
}

/// Acceptance 3: a failed base outcome keeps `DiffError::BeforeCalculation`.
#[test]
fn a_failed_base_keeps_the_before_calculation_refusal() {
    let document = failing_formula_document();
    let planner = formula_lifecycle();
    let candidate = planner
        .plan_commands(&document, &set_h_formula())
        .unwrap()
        .0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let revision = session.revision().clone();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate);
    let mut lifecycle = formula_lifecycle();
    let prepared_result = propose_with(
        &mut lifecycle,
        &document,
        revision.as_str(),
        set_h_formula(),
        &prepared,
    );
    let mut plain_lifecycle = formula_lifecycle();
    let plain_result = propose_plain(
        &mut plain_lifecycle,
        &document,
        revision.as_str(),
        set_h_formula(),
    );

    // Both proposals attempt the base recalculation; it fails, so nothing is
    // counted as a completed base calculation.
    assert_eq!(evaluation_counters::counts(), (2, 0));
    let prepared_error = prepared_result.unwrap_err();
    let plain_error = plain_result.unwrap_err();
    for error in [&prepared_error, &plain_error] {
        let PatchLifecycleError::CommandRejected { source } = error else {
            panic!("expected command rejection, got {error:?}");
        };
        assert!(matches!(
            source.as_ref(),
            WorkspaceError::Diff(crate::DiffError::BeforeCalculation(_))
        ));
    }
    assert_eq!(format!("{prepared_error:?}"), format!("{plain_error:?}"));
}

/// Acceptance 4: an invalid candidate evaluated by the session is refused with
/// the same `ValidationFailed` report as `propose`, without a base
/// recalculation.
#[test]
fn an_invalid_prepared_candidate_is_refused_like_propose() {
    let document = formula_document();
    let cyclic = SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "h"),
        value: Value::Formula(Expression::Reference(FieldRef::new("e1", "h"))),
    });
    let planner = formula_lifecycle();
    let candidate = planner.plan_commands(&document, &cyclic).unwrap().0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let revision = session.revision().clone();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate);
    let mut lifecycle = formula_lifecycle();
    let prepared_result = propose_with(
        &mut lifecycle,
        &document,
        revision.as_str(),
        cyclic.clone(),
        &prepared,
    );
    let mut plain_lifecycle = formula_lifecycle();
    let plain_result = propose_plain(&mut plain_lifecycle, &document, revision.as_str(), cyclic);

    assert_eq!(evaluation_counters::counts(), (2, 0));
    let prepared_error = prepared_result.unwrap_err();
    let plain_error = plain_result.unwrap_err();
    let PatchLifecycleError::ValidationFailed { report } = &prepared_error else {
        panic!("expected validation failure, got {prepared_error:?}");
    };
    let PatchLifecycleError::ValidationFailed {
        report: plain_report,
    } = &plain_error
    else {
        panic!("expected validation failure, got {plain_error:?}");
    };
    assert_eq!(report, plain_report);
    assert!(!report.is_valid());
}

// --- One batch walks each document-wide structure once (#678 stage 4) ---

fn optional_text_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        required: false,
        ..text_field(id)
    }
}

/// `formula_document` plus an optional `note` slot that starts filled.
fn batch_document() -> Document {
    let mut document = formula_document();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .insert("note".into(), optional_text_field("note"));
    document
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("note".into(), Value::Text("note0".into()));
    document
}

/// An accepted entity the planner can append to or remove from.
fn second_entity() -> Entity {
    Entity {
        id: "e2".into(),
        key: "second".into(),
        schema: "s".into(),
        fields: BTreeMap::from([
            ("f".into(), Value::Text("row2".into())),
            ("g".into(), Value::Number(Number::new(4.0).unwrap())),
            (
                "h".into(),
                Value::Formula(Expression::Number(Number::new(1.0).unwrap())),
            ),
        ]),
    }
}

/// A balanced sum of 256 leaves: 511 nodes, over the structure limit.
fn oversized_formula() -> Expression {
    let mut level = (0..256)
        .map(|_| Expression::Number(Number::new(1.0).unwrap()))
        .collect::<Vec<_>>();
    while level.len() > 1 {
        let mut parents = Vec::new();
        let mut leaves = level.into_iter();
        while let (Some(left), Some(right)) = (leaves.next(), leaves.next()) {
            parents.push(Expression::Add {
                left: Box::new(left),
                right: Box::new(right),
            });
        }
        level = parents;
    }
    level.pop().unwrap()
}

fn planning_lifecycle() -> PatchLifecycle {
    PatchLifecycle::new(
        "domain".into(),
        "scope".into(),
        "doc".into(),
        "api".into(),
        "policy".into(),
        "meaning".into(),
    )
}

/// The reference behaviour: one proposal per command, each planning from the
/// previous result, exactly as before the batch state existed.
fn plan_one_at_a_time(
    lifecycle: &PatchLifecycle,
    document: &Document,
    commands: &[SemanticCommand],
) -> Result<(Document, BTreeSet<AssociatedWriteRequirement>), WorkspaceError> {
    let mut candidate = document.clone();
    let mut writes = BTreeSet::new();
    for command in commands {
        let body = SemanticPatchBody::command(command.clone());
        let (next, command_writes) = lifecycle.plan_commands(&candidate, &body)?;
        candidate = next;
        writes.extend(command_writes);
    }
    Ok((candidate, writes))
}

/// Assert that one atomic batch plans like the same commands applied one
/// proposal at a time, including the refusal.
fn assert_batch_plans_like_one_at_a_time(document: &Document, commands: &[SemanticCommand]) {
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands.to_vec()).unwrap();
    let batch = lifecycle.plan_commands(document, &body);
    let one_at_a_time = plan_one_at_a_time(&lifecycle, document, commands);
    match (batch, one_at_a_time) {
        (Ok((batch_candidate, batch_writes)), Ok((one_candidate, one_writes))) => {
            assert_eq!(batch_candidate, one_candidate);
            assert_eq!(batch_writes, one_writes);
        }
        (Err(batch_error), Err(one_error)) => {
            assert_eq!(format!("{batch_error:?}"), format!("{one_error:?}"));
        }
        (batch, one_at_a_time) => {
            panic!("batch and one-command plans disagree: {batch:?} vs {one_at_a_time:?}");
        }
    }
}

fn set_value(field: &str, value: Value) -> SemanticCommand {
    SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", field),
        value,
    }
}

fn set_number(field: &str, value: f64) -> SemanticCommand {
    set_value(field, Value::Number(Number::new(value).unwrap()))
}

fn set_formula(field: &str, expression: Expression) -> SemanticCommand {
    set_value(field, Value::Formula(expression))
}

fn unset(field: &str) -> SemanticCommand {
    SemanticCommand::UnsetField {
        field: FieldRef::new("e1", field),
    }
}

#[test]
fn a_field_edit_batch_plans_like_one_command_per_proposal() {
    let document = batch_document();
    let commands = vec![
        set_value("f", Value::Text("first".into())),
        set_formula("h", Expression::Number(Number::new(3.0).unwrap())),
        unset("note"),
        set_number("g", 2.0),
        set_value("note", Value::Text("again".into())),
        unset("note"),
        set_formula("h", Expression::Reference(FieldRef::new("e1", "g"))),
    ];

    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands.clone()).unwrap();
    let (candidate, _) = lifecycle.plan_commands(&document, &body).unwrap();

    assert_eq!(
        candidate.entities["e1"].fields["f"],
        Value::Text("first".into())
    );
    assert_eq!(
        candidate.entities["e1"].fields["h"],
        Value::Formula(Expression::Reference(FieldRef::new("e1", "g")))
    );
    assert!(!candidate.entities["e1"].fields.contains_key("note"));
    assert_eq!(candidate.schemas, document.schemas);
    assert_batch_plans_like_one_at_a_time(&document, &commands);
}

#[test]
fn a_later_edit_failure_keeps_todays_error() {
    let document = batch_document();
    let allowed = set_value("f", Value::Text("first".into()));
    for failing in [
        // NoChange on the second command.
        set_value("g", document.entities["e1"].fields["g"].clone()),
        // A missing entity.
        SemanticCommand::SetFieldValue {
            field: FieldRef::new("missing", "g"),
            value: Value::Number(Number::new(2.0).unwrap()),
        },
        // A value of the wrong kind for the field.
        set_value("f", Value::Number(Number::new(2.0).unwrap())),
        // A formula over the structure limit.
        set_formula("h", oversized_formula()),
    ] {
        let commands = vec![allowed.clone(), failing];
        assert_batch_plans_like_one_at_a_time(&document, &commands);
    }
}

#[test]
fn a_formula_over_the_structure_limit_is_refused_at_todays_point() {
    let mut document = formula_document();
    document
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("h".into(), Value::Formula(oversized_formula()));

    // The first command's own refusal still comes first.
    let earlier_failure = vec![
        set_value("g", document.entities["e1"].fields["g"].clone()),
        set_value("f", Value::Text("first".into())),
    ];
    assert_batch_plans_like_one_at_a_time(&document, &earlier_failure);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(earlier_failure).unwrap();
    assert!(matches!(
        lifecycle.plan_commands(&document, &body).unwrap_err(),
        WorkspaceError::NoChange { .. }
    ));

    // Otherwise the structure preflight refuses on the first command that
    // reaches it.
    let reaches_preflight = vec![
        set_value("f", Value::Text("first".into())),
        set_number("g", 2.0),
    ];
    assert_batch_plans_like_one_at_a_time(&document, &reaches_preflight);
    let body = SemanticPatchBody::atomic_batch(reaches_preflight).unwrap();
    let error = lifecycle.plan_commands(&document, &body).unwrap_err();
    let WorkspaceError::ExpressionComplexity { field, .. } = error else {
        panic!("expected the structure refusal, got {error:?}");
    };
    assert_eq!(field, FieldRef::new("e1", "h"));
}

#[test]
fn a_structural_command_resets_the_batch_state() {
    let document = batch_document();

    // A formula before the append builds the index; the append changes the
    // addresses, so the later formula must project against the new document.
    let append_then_project = vec![
        set_formula("h", Expression::Number(Number::new(3.0).unwrap())),
        SemanticCommand::AppendEntity {
            entity: second_entity(),
        },
        set_formula("h", Expression::Reference(FieldRef::new("e2", "g"))),
    ];
    assert_batch_plans_like_one_at_a_time(&document, &append_then_project);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(append_then_project).unwrap();
    let (candidate, _) = lifecycle.plan_commands(&document, &body).unwrap();
    assert_eq!(
        candidate.entities["e1"].fields["h"],
        Value::Formula(Expression::Reference(FieldRef::new("e2", "g")))
    );

    // A reference the structural command removed keeps today's refusal.
    let mut with_second = document.clone();
    with_second.entities.insert("e2".into(), second_entity());
    let remove_then_project = vec![
        set_formula("h", Expression::Number(Number::new(3.0).unwrap())),
        SemanticCommand::RemoveEntity {
            entity: "e2".into(),
        },
        set_formula("h", Expression::Reference(FieldRef::new("e2", "g"))),
    ];
    assert_batch_plans_like_one_at_a_time(&with_second, &remove_then_project);
    let body = SemanticPatchBody::atomic_batch(remove_then_project).unwrap();
    assert!(matches!(
        lifecycle.plan_commands(&with_second, &body).unwrap_err(),
        WorkspaceError::FormulaProjection { .. }
    ));
}

#[test]
fn unset_keeps_todays_absent_and_required_behaviour() {
    let mut document = batch_document();
    document
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .remove("note");

    let commands = vec![unset("note"), set_number("g", 2.0)];
    assert_batch_plans_like_one_at_a_time(&document, &commands);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands).unwrap();
    let (candidate, _) = lifecycle.plan_commands(&document, &body).unwrap();
    assert!(!candidate.entities["e1"].fields.contains_key("note"));

    let required = vec![unset("f")];
    assert_batch_plans_like_one_at_a_time(&document, &required);
    let body = SemanticPatchBody::atomic_batch(required).unwrap();
    assert!(matches!(
        lifecycle.plan_commands(&document, &body).unwrap_err(),
        WorkspaceError::MissingField { .. }
    ));
}
