//! One evaluation serves propose, preview and execute while the inputs are unchanged.
use std::collections::{BTreeMap, BTreeSet};

use super::{
    AssociatedWriteRequirement, AuthorizationAction, Document, DocumentScopeId, Entity,
    FieldConstraint, FieldDefinition, FieldId, FieldRef, FieldType, FormulaUpdateCommand, Grant,
    GrantRequirement, KeyedGroupedSumDefinition, MutationClass, OperationFamily, PatchLifecycle,
    PatchLifecycleError, PatchLifecycleState, PrincipalKind, ProposalId, ScopedSemanticSubject,
    SemanticCommand, SemanticPatch, SemanticPatchBody, SemanticPublicationAuthority,
    SemanticPublicationError, SemanticRevision, SemanticScope, TrustedInstant, V2ReceiptEvidence,
    Value,
};
use crate::{
    Expression, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, NumberFormat,
    NumberFormatKind, Schema, WorkspaceError, evaluation_counters,
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
    fn current_snapshot(&self) -> (DocumentScopeId, &Document, SemanticRevision) {
        ("scope".into(), &self.snapshot, self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: &Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, &Document, SemanticRevision, A), SemanticPublicationError> {
        if expected_scope != &DocumentScopeId::from("scope") || expected_revision != &self.revision
        {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(30))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        self.publications += 1;
        self.published = Some(candidate.clone());
        Ok((
            "scope".into(),
            self.published
                .as_ref()
                .expect("successful publication installs a document"),
            "r2".into(),
            authorization,
        ))
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
    fn current_snapshot(&self) -> (DocumentScopeId, &Document, SemanticRevision) {
        ("scope".into(), &self.snapshot, self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        _candidate: &Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, &Document, SemanticRevision, A), SemanticPublicationError> {
        if expected_scope != &DocumentScopeId::from("scope") || expected_revision != &self.revision
        {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(30))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        Ok(("scope".into(), &self.installed, "r2".into(), authorization))
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
    prepared: &crate::PreparedEvaluation<'_>,
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

// --- Formula updates keep the batch state too (#678 stage 4) ---

fn optional_number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        required: false,
        ..number_field(id)
    }
}

/// `formula_document` plus an absent optional Number slot for a restore.
fn formula_update_document() -> Document {
    let mut document = formula_document();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .insert("spare".into(), optional_number_field("spare"));
    document
}

fn formula_update(document: &Document, field: &str, expression: Expression) -> SemanticCommand {
    SemanticCommand::FormulaUpdate(FormulaUpdateCommand::new(
        document,
        FieldRef::new("e1", field),
        expression,
    ))
}

#[test]
fn a_formula_update_batch_plans_like_one_command_per_proposal() {
    let document = formula_update_document();
    let commands = vec![
        set_value("f", Value::Text("first".into())),
        // Present Number target.
        formula_update(
            &document,
            "g",
            Expression::Number(Number::new(2.0).unwrap()),
        ),
        // Present formula target.
        formula_update(
            &document,
            "h",
            Expression::Number(Number::new(7.0).unwrap()),
        ),
        // Absent optional Number target.
        formula_update(
            &document,
            "spare",
            Expression::Number(Number::new(4.0).unwrap()),
        ),
        set_value("f", Value::Text("second".into())),
        unset("spare"),
        // Absent again after the unset.
        formula_update(
            &document,
            "spare",
            Expression::Number(Number::new(5.0).unwrap()),
        ),
    ];

    assert_batch_plans_like_one_at_a_time(&document, &commands);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands).unwrap();
    let (candidate, _) = lifecycle.plan_commands(&document, &body).unwrap();
    assert_eq!(
        candidate.entities["e1"].fields["f"],
        Value::Text("second".into())
    );
    assert_eq!(
        candidate.entities["e1"].fields["spare"],
        Value::Formula(Expression::Number(Number::new(5.0).unwrap()))
    );
}

#[test]
fn a_later_formula_update_failure_keeps_todays_error() {
    let document = formula_update_document();
    let first = set_value("f", Value::Text("first".into()));
    for (field, expression) in [
        // A formula over a Text field.
        ("f", Expression::Number(Number::new(2.0).unwrap())),
        // A formula over the structure limit.
        ("g", oversized_formula()),
        // A reference nothing resolves.
        ("g", Expression::Reference(FieldRef::new("e1", "missing"))),
    ] {
        let commands = vec![first.clone(), formula_update(&document, field, expression)];
        assert_batch_plans_like_one_at_a_time(&document, &commands);
    }
}

// --- Formula restores keep the batch state too (#678 stage 4) ---

/// `formula_update_document` plus a second absent optional Number slot.
fn formula_restore_document() -> Document {
    let mut document = formula_update_document();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .insert("spare2".into(), optional_number_field("spare2"));
    document
}

fn restore(field: &str, expression: Expression) -> SemanticCommand {
    SemanticCommand::formula_restore(FieldRef::new("e1", field), expression)
}

fn inverse_restore(field: &str, value: f64) -> SemanticCommand {
    SemanticCommand::formula_inverse_restore(
        FieldRef::new("e1", field),
        Number::new(value).unwrap(),
    )
}

#[test]
fn a_formula_restore_batch_plans_like_one_command_per_proposal() {
    let document = formula_restore_document();
    let commands = vec![
        set_value("f", Value::Text("first".into())),
        restore("spare", Expression::Number(Number::new(1.0).unwrap())),
        set_value("f", Value::Text("second".into())),
        restore("spare2", Expression::Number(Number::new(2.0).unwrap())),
        unset("spare"),
        restore("spare", Expression::Number(Number::new(3.0).unwrap())),
    ];

    assert_batch_plans_like_one_at_a_time(&document, &commands);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands).unwrap();
    let (candidate, _) = lifecycle.plan_commands(&document, &body).unwrap();
    assert_eq!(
        candidate.entities["e1"].fields["spare"],
        Value::Formula(Expression::Number(Number::new(3.0).unwrap()))
    );
    assert_eq!(
        candidate.entities["e1"].fields["spare2"],
        Value::Formula(Expression::Number(Number::new(2.0).unwrap()))
    );
}

#[test]
fn a_formula_inverse_restore_batch_plans_like_one_command_per_proposal() {
    let document = formula_restore_document();
    let commands = vec![
        set_value("f", Value::Text("first".into())),
        inverse_restore("h", 9.0),
        set_value("f", Value::Text("second".into())),
        formula_update(
            &document,
            "g",
            Expression::Number(Number::new(4.0).unwrap()),
        ),
        inverse_restore("g", 5.0),
    ];

    assert_batch_plans_like_one_at_a_time(&document, &commands);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands).unwrap();
    let (candidate, _) = lifecycle.plan_commands(&document, &body).unwrap();
    assert_eq!(
        candidate.entities["e1"].fields["g"],
        Value::Number(Number::new(5.0).unwrap())
    );
    assert_eq!(
        candidate.entities["e1"].fields["h"],
        Value::Number(Number::new(9.0).unwrap())
    );
}

#[test]
fn a_later_formula_restore_failure_keeps_todays_error() {
    let document = formula_restore_document();
    let first = set_value("f", Value::Text("first".into()));
    for failing in [
        // Restore refusals: a missing entity, a non-Number field, a required
        // field.
        restore("missing", Expression::Number(Number::new(1.0).unwrap())),
        restore("f", Expression::Number(Number::new(1.0).unwrap())),
        restore("g", Expression::Number(Number::new(1.0).unwrap())),
        // Inverse-restore refusals: a non-Number field, a non-formula value,
        // an absent optional slot, and a missing entity.
        inverse_restore("f", 1.0),
        inverse_restore("g", 1.0),
        inverse_restore("spare", 1.0),
        SemanticCommand::formula_inverse_restore(
            FieldRef::new("missing", "g"),
            Number::new(1.0).unwrap(),
        ),
    ] {
        let commands = vec![first.clone(), failing];
        assert_batch_plans_like_one_at_a_time(&document, &commands);
    }

    // A target the first command filled refuses with today's NoChange.
    let commands = vec![
        restore("spare", Expression::Number(Number::new(1.0).unwrap())),
        restore("spare", Expression::Number(Number::new(2.0).unwrap())),
    ];
    assert_batch_plans_like_one_at_a_time(&document, &commands);
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands).unwrap();
    assert!(matches!(
        lifecycle.plan_commands(&document, &body).unwrap_err(),
        WorkspaceError::NoChange { .. }
    ));
}

#[test]
fn undoing_a_restore_batch_returns_the_original_document() {
    let document = formula_restore_document();
    let forward = vec![
        inverse_restore("h", 9.0),
        restore("spare", Expression::Number(Number::new(3.0).unwrap())),
        set_value("f", Value::Text("changed".into())),
    ];
    let inverse = vec![
        set_value(
            "h",
            Value::Formula(Expression::Reference(FieldRef::new("e1", "g"))),
        ),
        unset("spare"),
        set_value("f", Value::Text("old".into())),
    ];

    let lifecycle = planning_lifecycle();
    let forward_body = SemanticPatchBody::atomic_batch(forward).unwrap();
    let (changed, _) = lifecycle.plan_commands(&document, &forward_body).unwrap();
    assert_ne!(changed, document);

    let inverse_body = SemanticPatchBody::atomic_batch(inverse).unwrap();
    let (restored, _) = lifecycle.plan_commands(&changed, &inverse_body).unwrap();
    assert_eq!(restored, document);
}

// --- V2 evidence reuses the prepared evaluation (#678) ---

/// A lifecycle that may propose every command the v2 cases need.
fn v2_lifecycle() -> PatchLifecycle {
    let mut lifecycle = PatchLifecycle::new_v2(
        "domain".into(),
        "scope".into(),
        "doc".into(),
        "api".into(),
        "policy".into(),
        "meaning".into(),
    );
    lifecycle
        .register_principal("human".into(), PrincipalKind::Human)
        .unwrap();
    let scope = ScopedSemanticSubject::new("scope".into(), "doc".into(), SemanticScope::Document);
    let mut requirements = Vec::new();
    for (family, classes) in [
        (
            OperationFamily::SetFieldValue,
            [
                MutationClass::Value,
                MutationClass::Formula,
                MutationClass::Destructive,
            ]
            .as_slice(),
        ),
        (
            OperationFamily::SchemaFieldMutation,
            [
                MutationClass::Schema,
                MutationClass::Structure,
                MutationClass::Destructive,
            ]
            .as_slice(),
        ),
        (
            OperationFamily::AppendEntity,
            [MutationClass::Structure, MutationClass::Formula].as_slice(),
        ),
        (
            OperationFamily::RemoveEntity,
            [
                MutationClass::Structure,
                MutationClass::Destructive,
                MutationClass::Formula,
            ]
            .as_slice(),
        ),
        (
            OperationFamily::FormulaUpdate,
            [MutationClass::Formula].as_slice(),
        ),
        (
            OperationFamily::KeyedGroupedSumDefinition,
            [MutationClass::Structure, MutationClass::Destructive].as_slice(),
        ),
    ] {
        requirements.push(GrantRequirement::query(family, scope.clone()));
        for action in [AuthorizationAction::Propose, AuthorizationAction::Execute] {
            for mutation_class in classes {
                requirements.push(
                    GrantRequirement::mutation(action, family, *mutation_class, scope.clone())
                        .unwrap(),
                );
            }
        }
    }
    lifecycle
        .provision_grant(Grant::new(
            "grant.v2".into(),
            "human".into(),
            "human".into(),
            requirements,
            None,
        ))
        .unwrap();
    lifecycle
}

fn multi_field_schema(id: &str, key: &str, fields: &[(&str, FieldType)]) -> Schema {
    Schema {
        field_order: None,
        id: id.into(),
        key: key.into(),
        display_name: None,
        fields: fields
            .iter()
            .map(|(field, field_type)| {
                let definition = match field_type {
                    FieldType::Number => number_field(field),
                    _ => text_field(field),
                };
                (FieldId::from(*field), definition)
            })
            .collect(),
    }
}

fn keyed_definition() -> KeyedGroupedSumDefinition {
    KeyedGroupedSumDefinition {
        id: "summary".into(),
        orders: KeyedGroupedSumOrdersBinding {
            schema: "orders".into(),
            lookup_key_field: "lookup".into(),
            quantity_field: "quantity".into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: "products".into(),
            key_field: "key".into(),
            category_field: "category".into(),
            price_field: "price".into(),
        },
    }
}

/// `formula_document` plus both schemas a saved keyed grouped sum binds.
fn keyed_document() -> Document {
    let mut document = formula_document();
    document.schemas.insert(
        "orders".into(),
        multi_field_schema(
            "orders",
            "orders",
            &[("lookup", FieldType::Text), ("quantity", FieldType::Number)],
        ),
    );
    document.schemas.insert(
        "products".into(),
        multi_field_schema(
            "products",
            "products",
            &[
                ("key", FieldType::Text),
                ("category", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
    );
    document
}

/// `formula_document` plus a second schema whose column `h` reads.
fn cross_schema_document() -> Document {
    let mut document = formula_document();
    document.schemas.insert(
        "s2".into(),
        multi_field_schema("s2", "other", &[("n", FieldType::Number)]),
    );
    document.entities.insert(
        "e2".into(),
        Entity {
            id: "e2".into(),
            key: "row2".into(),
            schema: "s2".into(),
            fields: BTreeMap::from([("n".into(), Value::Number(Number::new(2.0).unwrap()))]),
        },
    );
    document.entities.get_mut("e1").unwrap().fields.insert(
        "h".into(),
        Value::Formula(Expression::Reference(FieldRef::new("e2", "n"))),
    );
    document
}

/// A valid document whose `h` projects past the canonical formula length: the
/// referenced entity key alone is 4,090 bytes. Renaming `input` shortens it
/// enough to pass the projection preflight.
fn unprojectable_document() -> Document {
    let mut document = formula_document();
    document.schemas.insert(
        "t".into(),
        Schema {
            field_order: None,
            id: "t".into(),
            key: "targets".into(),
            display_name: None,
            fields: BTreeMap::from([("input".into(), optional_number_field("input"))]),
        },
    );
    document.entities.insert(
        "target".into(),
        Entity {
            id: "target".into(),
            key: "e".repeat(4_090).into(),
            schema: "t".into(),
            fields: BTreeMap::from([("input".into(), Value::Number(Number::new(2.0).unwrap()))]),
        },
    );
    document.entities.get_mut("e1").unwrap().fields.insert(
        "h".into(),
        Value::Formula(Expression::Reference(FieldRef::new("target", "input"))),
    );
    document
}

/// Compare `propose_prepared` with `propose` for one document and body.
///
/// The prepared proposal runs first, so the returned evaluation counters are
/// the prepared path's; the proposal payload and every evaluated result must
/// be identical, and a refusal must keep its exact error.
fn assert_prepared_matches_unprepared(
    document: &Document,
    body: SemanticPatchBody,
    label: &str,
) -> ((usize, usize), Result<SemanticPatch, PatchLifecycleError>) {
    let planner = v2_lifecycle();
    let candidate = planner
        .plan_commands(document, &body)
        .unwrap_or_else(|error| panic!("{label}: planning refused: {error:?}"))
        .0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let revision = session.revision().clone();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate);
    let mut prepared_lifecycle = v2_lifecycle();
    let prepared_result = propose_with(
        &mut prepared_lifecycle,
        document,
        revision.as_str(),
        body.clone(),
        &prepared,
    );
    let counts = evaluation_counters::counts();

    let mut plain_lifecycle = v2_lifecycle();
    let plain_result = propose_plain(&mut plain_lifecycle, document, revision.as_str(), body);

    match (&plain_result, &prepared_result) {
        (Ok(plain), Ok(reused)) => {
            assert_eq!(plain, reused, "{label}: proposal payloads differ");
            assert_same_evaluation(&plain_lifecycle, &prepared_lifecycle);
        }
        (Err(plain), Err(reused)) => {
            assert_eq!(
                format!("{plain:?}"),
                format!("{reused:?}"),
                "{label}: refusals differ"
            );
        }
        (plain, reused) => panic!("{label}: outcomes differ: {plain:?} vs {reused:?}"),
    }
    (counts, prepared_result)
}

fn acceptance_cases() -> Vec<(&'static str, Document, SemanticPatchBody)> {
    let formula = formula_document();
    let update = formula_update_document();
    let mut with_second = formula_document();
    with_second.entities.insert("e2".into(), second_entity());
    let formula_update_body = SemanticPatchBody::command(formula_update(
        &formula,
        "h",
        Expression::Number(Number::new(3.0).unwrap()),
    ));
    let restore_body = SemanticPatchBody::command(restore(
        "spare",
        Expression::Number(Number::new(1.0).unwrap()),
    ));
    let batch_body = SemanticPatchBody::atomic_batch(vec![
        set_value("f", Value::Text("batch".into())),
        set_number("g", 2.0),
    ])
    .unwrap();
    vec![
        (
            "a value edit that reaches a formula",
            formula.clone(),
            set_g_number(2.0),
        ),
        ("a formula update", formula.clone(), formula_update_body),
        ("a formula restore", update, restore_body),
        ("a batch of edits", formula.clone(), batch_body),
        (
            "a row insert",
            formula.clone(),
            SemanticPatchBody::command(SemanticCommand::AppendEntity {
                entity: second_entity(),
            }),
        ),
        (
            "a row remove",
            with_second,
            SemanticPatchBody::command(SemanticCommand::RemoveEntity {
                entity: "e2".into(),
            }),
        ),
        (
            "a display name change",
            formula.clone(),
            SemanticPatchBody::command(SemanticCommand::SetSchemaFieldDisplayName {
                schema: "s".into(),
                field: "f".into(),
                display_name: Some("Name".into()),
            }),
        ),
        (
            "a number format change",
            formula.clone(),
            SemanticPatchBody::command(SemanticCommand::SetFieldNumberFormat {
                schema: "s".into(),
                field: "g".into(),
                number_format: Some(NumberFormat {
                    decimals: 2,
                    kind: NumberFormatKind::Percent,
                    thousands_separator: true,
                }),
            }),
        ),
        (
            "a field order change",
            formula.clone(),
            SemanticPatchBody::command(SemanticCommand::SetSchemaFieldOrder {
                schema: "s".into(),
                order: Some(vec![
                    FieldId::from("h"),
                    FieldId::from("f"),
                    FieldId::from("g"),
                ]),
            }),
        ),
        (
            "a constraint set",
            formula.clone(),
            SemanticPatchBody::command(SemanticCommand::SetFieldConstraint {
                schema: "s".into(),
                field: "f".into(),
                constraint: FieldConstraint::TextLiteralSet {
                    values: vec!["new".into(), "old".into()],
                },
            }),
        ),
        (
            "a keyed grouped sum definition change",
            keyed_document(),
            SemanticPatchBody::command(SemanticCommand::UpsertKeyedGroupedSumDefinition {
                definition: keyed_definition(),
            }),
        ),
        (
            "a formula reading another schema",
            cross_schema_document(),
            set_f_text("new"),
        ),
    ]
}

/// Acceptance 1: a prepared proposal equals an unprepared one for every kind
/// of v2 evidence.
#[test]
fn prepared_v2_proposals_match_unprepared_proposals() {
    for (label, document, body) in acceptance_cases() {
        let (counts, result) = assert_prepared_matches_unprepared(&document, body, label);
        assert!(result.is_ok(), "{label}: {result:?}");
        assert_eq!(
            counts,
            (1, 0),
            "{label}: the proposal must only calculate in `prepare_evaluation`"
        );
    }
}

/// Acceptance 2: every refusal keeps its exact error with a prepared
/// evaluation, including a base whose retained calculation failed.
#[test]
fn prepared_v2_refusals_match_unprepared_refusals() {
    let formula = formula_document();
    let cyclic = SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "h"),
        value: Value::Formula(Expression::Reference(FieldRef::new("e1", "h"))),
    });
    let failing = SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "h"),
        value: Value::Formula(Expression::Divide {
            left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
        }),
    });
    let violated_constraint = set_f_text("not allowed");
    let invalid_definition =
        SemanticPatchBody::command(SemanticCommand::UpsertKeyedGroupedSumDefinition {
            definition: KeyedGroupedSumDefinition {
                id: "summary".into(),
                orders: KeyedGroupedSumOrdersBinding {
                    schema: "missing".into(),
                    lookup_key_field: "lookup".into(),
                    quantity_field: "quantity".into(),
                },
                products: KeyedGroupedSumProductsBinding {
                    schema: "missing".into(),
                    key_field: "key".into(),
                    category_field: "category".into(),
                    price_field: "price".into(),
                },
            },
        });
    let mut constrained = formula_document();
    constrained
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("f")
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: vec!["old".into()],
    };

    let invalid = vec![
        ("an invalid candidate", formula.clone(), cyclic),
        (
            "an edit that makes a formula fail",
            formula.clone(),
            failing,
        ),
        ("a constraint violation", constrained, violated_constraint),
        ("invalid definitions", formula.clone(), invalid_definition),
    ];
    for (label, document, body) in invalid {
        let _ = assert_prepared_matches_unprepared(&document, body, label);
    }

    // The retained base outcome failed, so `base_calculation` is `None` and
    // the prepared evaluation cannot replace today's refusal.
    let _ = assert_prepared_matches_unprepared(
        &failing_formula_document(),
        set_g_number(2.0),
        "a base whose retained calculation failed",
    );
}

/// An edit that repairs an invalid base keeps today's refusal: the prepared
/// path needs the base to pass everything the unprepared path checks.
#[test]
fn prepared_v2_keeps_refusing_an_edit_that_repairs_its_base() {
    // The base still fails validation that `ResidentWorkspaceSession::new`
    // accepts, while the edit repairs it and the candidate is valid. Each
    // refusal must keep today's exact variant, not merely look alike.
    let mut violating_base = formula_document();
    violating_base
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("f")
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: vec!["old".into()],
    };
    violating_base
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("f".into(), Value::Text("not allowed".into()));
    let (_, refused_base_constraint) = assert_prepared_matches_unprepared(
        &violating_base,
        set_f_text("old"),
        "a base whose stored value violates its constraint",
    );
    assert!(
        matches!(
            refused_base_constraint,
            Err(PatchLifecycleError::ValidationFailed { .. })
        ),
        "{refused_base_constraint:?}"
    );

    let mut broken_definitions = keyed_document();
    broken_definitions.keyed_grouped_sum_definitions.insert(
        "broken".into(),
        KeyedGroupedSumDefinition {
            id: "broken".into(),
            orders: KeyedGroupedSumOrdersBinding {
                schema: "missing".into(),
                lookup_key_field: "lookup".into(),
                quantity_field: "quantity".into(),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: "missing".into(),
                key_field: "key".into(),
                category_field: "category".into(),
                price_field: "price".into(),
            },
        },
    );
    let (_, refused_base_definitions) = assert_prepared_matches_unprepared(
        &broken_definitions,
        SemanticPatchBody::command(SemanticCommand::RemoveKeyedGroupedSumDefinition {
            definition: "broken".into(),
        }),
        "a base with an invalid saved definition",
    );
    let PatchLifecycleError::CommandRejected {
        source: refused_base_definitions,
    } = refused_base_definitions.expect_err("an invalid base definition keeps today's refusal")
    else {
        panic!("expected command rejection");
    };
    assert!(
        matches!(
            refused_base_definitions.as_ref(),
            WorkspaceError::InvalidKeyedGroupedSumDefinition(_)
        ),
        "{refused_base_definitions:?}"
    );

    let (_, refused_base_projection) = assert_prepared_matches_unprepared(
        &unprojectable_document(),
        SemanticPatchBody::command(SemanticCommand::RenameSchemaField {
            schema: "t".into(),
            field: "input".into(),
            key: "i".into(),
        }),
        "a base that fails the formula projection preflight",
    );
    let PatchLifecycleError::CommandRejected {
        source: refused_base_projection,
    } = refused_base_projection.expect_err("an unprojectable base keeps today's refusal")
    else {
        panic!("expected command rejection");
    };
    assert!(
        matches!(
            refused_base_projection.as_ref(),
            WorkspaceError::FormulaProjection { .. }
        ),
        "{refused_base_projection:?}"
    );
}

/// Acceptance 3: a prepared evaluation for a different candidate is ignored,
/// by the counters and by the resulting evaluation.
#[test]
fn a_prepared_v2_evaluation_for_another_candidate_is_ignored() {
    let document = formula_document();
    let planner = v2_lifecycle();
    let prepared_candidate = planner
        .plan_commands(&document, &set_f_text("new"))
        .unwrap()
        .0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let revision = session.revision().clone();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(prepared_candidate);
    let mut lifecycle = v2_lifecycle();
    propose_with(
        &mut lifecycle,
        &document,
        revision.as_str(),
        set_h_formula(),
        &prepared,
    )
    .unwrap();

    // One calculation for the ignored prepared candidate, then the same two
    // document validations and base record as the unprepared path.
    assert_eq!(evaluation_counters::counts(), (3, 1));
    let mut plain_lifecycle = v2_lifecycle();
    propose_plain(
        &mut plain_lifecycle,
        &document,
        revision.as_str(),
        set_h_formula(),
    )
    .unwrap();
    assert_same_evaluation(&plain_lifecycle, &lifecycle);
}

/// Acceptance 4: the prepared path only calculates in `prepare_evaluation`;
/// the unprepared path validates both documents and records the base.
#[test]
fn the_prepared_v2_path_reuses_every_calculation() {
    let document = formula_document();
    let body = set_g_number(2.0);
    let candidate = v2_lifecycle().plan_commands(&document, &body).unwrap().0;
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let revision = session.revision().clone();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate);
    let mut prepared_lifecycle = v2_lifecycle();
    propose_with(
        &mut prepared_lifecycle,
        &document,
        revision.as_str(),
        body.clone(),
        &prepared,
    )
    .unwrap();
    assert_eq!(
        evaluation_counters::counts(),
        (1, 0),
        "the proposal reuses the prepared candidate and base"
    );

    evaluation_counters::reset();
    let mut plain_lifecycle = v2_lifecycle();
    propose_plain(&mut plain_lifecycle, &document, revision.as_str(), body).unwrap();
    assert_eq!(
        evaluation_counters::counts(),
        (2, 1),
        "the unprepared path validates both documents and records its base"
    );
    assert_same_evaluation(&plain_lifecycle, &prepared_lifecycle);
}

/// Revision tokens repeat across sessions, so a prepared evaluation from a
/// session holding another document, with the same candidate and token, must
/// not supply this document's base results. On both evidence profiles.
#[test]
fn a_prepared_evaluation_from_another_base_document_is_ignored() {
    let body = set_g_number(3.0);
    let mut other_base = formula_document();
    other_base
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("g".into(), Value::Number(Number::new(7.0).unwrap()));
    assert_ne!(other_base, formula_document());
    for lifecycle in [formula_lifecycle as fn() -> PatchLifecycle, v2_lifecycle] {
        let document = formula_document();
        let candidate = lifecycle().plan_commands(&document, &body).unwrap().0;
        assert_eq!(
            candidate,
            lifecycle().plan_commands(&other_base, &body).unwrap().0
        );
        let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
        let other_session = ResidentWorkspaceSession::new("scope".into(), other_base.clone());
        let revision = session.revision().clone();
        assert_eq!(&revision, other_session.revision());

        let prepared = other_session.prepare_evaluation(candidate);
        evaluation_counters::reset();
        let mut reused = lifecycle();
        propose_with(
            &mut reused,
            &document,
            revision.as_str(),
            body.clone(),
            &prepared,
        )
        .unwrap();
        assert_ne!(
            evaluation_counters::counts(),
            (0, 0),
            "the other session's results must not be reused"
        );

        let mut plain = lifecycle();
        propose_plain(&mut plain, &document, revision.as_str(), body.clone()).unwrap();
        assert_same_evaluation(&plain, &reused);
    }
}

/// An entity the planner can append to the `s` collection.
fn appended_entity(id: &str, key: &str, formula: Option<Expression>) -> Entity {
    let mut entity = Entity {
        id: id.into(),
        key: key.into(),
        schema: "s".into(),
        fields: BTreeMap::from([
            ("f".into(), Value::Text(format!("row-{id}"))),
            ("g".into(), Value::Number(Number::new(2.0).unwrap())),
        ]),
    };
    if let Some(expression) = formula {
        entity.fields.insert("h".into(), Value::Formula(expression));
    }
    entity
}

fn append(entity: Entity) -> SemanticCommand {
    SemanticCommand::AppendEntity { entity }
}

#[test]
fn an_append_batch_plans_like_one_command_per_proposal() {
    let document = batch_document();
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(appended_entity("e3", "third", None)),
            append(appended_entity("e4", "fourth", None)),
        ],
    );
    // Formulas at the first, middle and last positions of the run.
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity(
                "e2",
                "second",
                Some(Expression::Reference(FieldRef::new("e1", "g"))),
            )),
            append(appended_entity(
                "e3",
                "third",
                Some(Expression::Number(Number::new(3.0).unwrap())),
            )),
            append(appended_entity(
                "e4",
                "fourth",
                Some(Expression::Add {
                    left: Box::new(Expression::Reference(FieldRef::new("e2", "g"))),
                    right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                }),
            )),
        ],
    );
}

#[test]
fn an_append_batch_refuses_keys_like_one_command_per_proposal() {
    let document = batch_document();
    // An existing row's key.
    assert_batch_plans_like_one_at_a_time(&document, &[append(appended_entity("e2", "row", None))]);
    // A key already appended earlier in the batch, and at a later position.
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(appended_entity("e3", "second", None)),
        ],
    );
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(appended_entity("e3", "third", None)),
            append(appended_entity("e4", "second", None)),
        ],
    );
}

#[test]
fn an_append_batch_refuses_ids_schemas_and_keys_like_one_command_per_proposal() {
    let document = batch_document();
    // Id collision with the base row and between two rows of the batch.
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[append(appended_entity("e1", "second", None))],
    );
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(appended_entity("e2", "third", None)),
        ],
    );
    // Missing schema at a later position.
    let mut missing = appended_entity("e3", "third", None);
    missing.schema = "missing".into();
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(missing),
        ],
    );
    // Invalid key at a later position.
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(appended_entity("e3", "not a key", None)),
        ],
    );
}

#[test]
fn an_append_batch_refuses_complex_formulas_like_one_command_per_proposal() {
    let document = batch_document();
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", Some(oversized_formula()))),
            append(appended_entity("e3", "third", None)),
        ],
    );
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            append(appended_entity("e3", "third", Some(oversized_formula()))),
        ],
    );
}

#[test]
fn a_remove_resets_the_append_proof_like_one_command_per_proposal() {
    let document = batch_document();
    // Remove the base row, then reuse its key in the same batch.
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            SemanticCommand::RemoveEntity {
                entity: "e1".into(),
            },
            append(appended_entity("e2", "row", None)),
        ],
    );
    // Append a row, remove it, then append the same key again.
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            SemanticCommand::RemoveEntity {
                entity: "e2".into(),
            },
            append(appended_entity("e3", "second", None)),
        ],
    );
}

#[test]
fn a_cell_edit_or_collection_append_resets_the_append_proof() {
    let document = batch_document();
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            append(appended_entity("e2", "second", None)),
            set_number("g", 2.0),
            append(appended_entity("e3", "third", None)),
        ],
    );

    let schema = Schema {
        field_order: None,
        id: "s2".into(),
        key: "second_table".into(),
        display_name: None,
        fields: BTreeMap::from([("f".into(), text_field("f"))]),
    };
    let entity = Entity {
        id: "x1".into(),
        key: "x".into(),
        schema: "s2".into(),
        fields: BTreeMap::from([("f".into(), Value::Text("new".into()))]),
    };
    assert_batch_plans_like_one_at_a_time(
        &document,
        &[
            SemanticCommand::AppendCollection {
                schema,
                entities: vec![entity],
            },
            append(appended_entity("e2", "second", None)),
        ],
    );
}

/// A host that keeps publishing: every publication advances the revision.
struct AdvancingHost {
    snapshot: Document,
    revision: SemanticRevision,
    publications: usize,
}

impl AdvancingHost {
    fn new(snapshot: Document) -> Self {
        Self {
            snapshot,
            revision: "r1".into(),
            publications: 0,
        }
    }
}

impl SemanticPublicationAuthority for AdvancingHost {
    fn current_snapshot(&self) -> (DocumentScopeId, &Document, SemanticRevision) {
        ("scope".into(), &self.snapshot, self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: &Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, &Document, SemanticRevision, A), SemanticPublicationError> {
        if expected_scope != &DocumentScopeId::from("scope") || expected_revision != &self.revision
        {
            return Err(SemanticPublicationError::Stale);
        }
        let authorization = authorize(TrustedInstant::new(30))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        self.publications += 1;
        self.snapshot = candidate.clone();
        self.revision = SemanticRevision::from(format!("r{}", self.publications + 1));
        Ok((
            "scope".into(),
            &self.snapshot,
            self.revision.clone(),
            authorization,
        ))
    }
}

#[test]
fn discard_settled_records_keeps_one_receipt_and_the_pending_proposal() {
    let mut lifecycle = lifecycle(false);
    let mut document = document();
    let mut host = AdvancingHost::new(document.clone());
    let mut executed = Vec::new();
    let mut receipts = Vec::new();
    for (index, value) in ["a", "b", "c"].into_iter().enumerate() {
        let proposal = lifecycle
            .propose(
                &"scope".into(),
                &document,
                &host.revision.clone(),
                super::ProposalRequest::new(
                    format!("proposal-{index}").into(),
                    host.revision.clone(),
                    set_f(value),
                    "human".into(),
                ),
                TrustedInstant::new(1),
            )
            .map(|patch| patch.id().clone())
            .unwrap();
        let receipt = lifecycle
            .execute(
                &proposal,
                None,
                &"human".into(),
                &mut host,
                TrustedInstant::new(20),
            )
            .unwrap();
        executed.push(proposal);
        receipts.push(receipt);
        document = host.snapshot.clone();
    }
    let pending = lifecycle
        .propose(
            &"scope".into(),
            &document,
            &host.revision.clone(),
            super::ProposalRequest::new(
                "pending".into(),
                host.revision.clone(),
                set_f("d"),
                "human".into(),
            ),
            TrustedInstant::new(1),
        )
        .map(|patch| patch.id().clone())
        .unwrap();
    // Without the call the registry still keeps every publication.
    assert_eq!(lifecycle.execution_receipts().len(), 3);
    assert_eq!(lifecycle.execution_receipts_v2(), []);
    assert_eq!(lifecycle.proposals.len(), 4);

    lifecycle.discard_settled_records(1);

    assert_eq!(lifecycle.execution_receipts(), [receipts[2].clone()]);
    assert_eq!(lifecycle.execution_receipts_v2(), []);
    assert_eq!(lifecycle.proposals.len(), 2);
    assert!(!lifecycle.proposals.contains_key(&executed[0]));
    assert!(!lifecycle.proposals.contains_key(&executed[1]));
    assert!(super::proposal_is_terminal(
        &lifecycle.proposals[&executed[2]]
    ));
    assert!(!super::proposal_is_terminal(&lifecycle.proposals[&pending]));
    assert_eq!(
        lifecycle.proposal_history(&pending).unwrap().last(),
        Some(&PatchLifecycleState::Planned)
    );
}
