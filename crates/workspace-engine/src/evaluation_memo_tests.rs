//! One evaluation serves propose, preview and execute while the inputs are unchanged.
use std::collections::{BTreeMap, BTreeSet};

use super::{
    AssociatedWriteRequirement, AuthorizationAction, Document, DocumentScopeId, DocumentWitness,
    Entity, FieldConstraint, FieldDefinition, FieldId, FieldRef, FieldType, FormulaUpdateCommand,
    Grant, GrantRequirement, KeyedGroupedSumDefinition, MutationClass, OperationFamily,
    PatchLifecycle, PatchLifecycleError, PatchLifecycleState, PrincipalId, PrincipalKind,
    ProposalId, ScopedSemanticSubject, SemanticCommand, SemanticPatch, SemanticPatchBody,
    SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision, SemanticScope,
    TrustedInstant, V2ReceiptEvidence, Value,
};
use crate::{
    ColumnRef, EntityId, Expression, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    Number, NumberFormat, NumberFormatKind, Schema, SchemaId, WorkspaceError, evaluation_counters,
    planning_counters,
    resident_session::{ResidentWorkspaceSession, TrustedPublicationTimeSource},
};
use tachiko_diff_engine::{
    CANONICAL_SEMANTIC_DELTA_V2, canonical_delta_scoped, canonical_delta_with_calculations,
    diff_changes_scoped, diff_with_calculations,
};
use tachiko_formula_engine::calculate;

fn text_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        number_format: None,
        date_format: None,
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
        date_format: None,
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

// --- One batch walks each document-wide structure once (#712) ---

/// The most whole-document work one batch may repeat. Equation of the counts
/// across two batch sizes, not this number, is what proves the work does not
/// grow with the batch.
const MAX_PLANNING_WHOLE_DOCUMENT_WORK: usize = 2;

/// Plan one batch and return `(document clones, structure preflights,
/// address-index builds)` as the test-only counters observed them.
fn plan_counts(document: &Document, commands: Vec<SemanticCommand>) -> (usize, usize, usize) {
    let lifecycle = planning_lifecycle();
    let body = SemanticPatchBody::atomic_batch(commands).expect("a batch of commands");
    planning_counters::reset();
    lifecycle
        .plan_commands(document, &body)
        .expect("the batch plans");
    planning_counters::counts()
}

/// Assert one kind of command plans with the same small counts at 10 and 50
/// commands: no whole-document walk runs once per command.
fn assert_bounded_planning(label: &str, build: impl Fn(usize) -> Vec<SemanticCommand>) {
    let document = batch_document();
    let small = plan_counts(&document, build(10));
    let large = plan_counts(&document, build(50));
    assert_eq!(
        small, large,
        "{label}: whole-document work grew with the batch"
    );
    let (clones, preflights, builds) = large;
    assert!(
        clones <= MAX_PLANNING_WHOLE_DOCUMENT_WORK,
        "{label}: {clones} whole-document clones in one batch"
    );
    assert!(
        preflights <= MAX_PLANNING_WHOLE_DOCUMENT_WORK,
        "{label}: {preflights} formula-structure preflights in one batch"
    );
    assert!(
        builds <= MAX_PLANNING_WHOLE_DOCUMENT_WORK,
        "{label}: {builds} address-index builds in one batch"
    );
}

/// One entity an append batch can add, named after its index.
fn batch_appended_entity(index: usize) -> Entity {
    appended_entity(
        &format!("e{}", index + 2),
        &format!("row{}", index + 2),
        None,
    )
}

#[test]
fn a_value_edit_batch_plans_without_whole_document_work_per_command() {
    assert_bounded_planning("SetFieldValue", |count| {
        (0..count)
            .map(|index| set_value("f", Value::Text(format!("value-{index}"))))
            .collect()
    });
}

#[test]
fn a_formula_set_batch_plans_without_whole_document_work_per_command() {
    assert_bounded_planning("formula sets", |count| {
        (0..count)
            .map(|index| {
                let value = f64::from(u32::try_from(index).expect("small batch"));
                set_formula(
                    "h",
                    Expression::Number(Number::new(value).expect("finite literal")),
                )
            })
            .collect()
    });
}

#[test]
fn an_append_run_plans_without_whole_document_work_per_command() {
    assert_bounded_planning("AppendEntity", |count| {
        (0..count)
            .map(|index| SemanticCommand::AppendEntity {
                entity: batch_appended_entity(index),
            })
            .collect()
    });
}

#[test]
fn a_mixed_batch_plans_without_whole_document_work_per_command() {
    assert_bounded_planning("mixed", |count| {
        let half = count / 2;
        (0..half)
            .map(|index| {
                let value = f64::from(u32::try_from(index).expect("small batch"));
                set_formula(
                    "h",
                    Expression::Number(Number::new(value + 1.0).expect("finite literal")),
                )
            })
            .chain(
                (0..count - half).map(|index| SemanticCommand::AppendEntity {
                    entity: batch_appended_entity(index),
                }),
            )
            .collect()
    });
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

/// The edit chain's base: `formula_document` plus the second schema its
/// formula reads, both keyed grouped-sum schemas, and a long entity key whose
/// formula still fits the canonical limit. Renaming that key's field pushes
/// the formula past the limit, so one edit can fail its projection while the
/// rest of the document stays valid.
fn preflight_chain_document() -> Document {
    let mut document = keyed_document();
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
    document.schemas.insert(
        "t".into(),
        multi_field_schema("t", "targets", &[("input", FieldType::Number)]),
    );
    document.entities.insert(
        "target".into(),
        Entity {
            id: "target".into(),
            key: "e".repeat(3_900).into(),
            schema: "t".into(),
            fields: BTreeMap::from([("input".into(), Value::Number(Number::new(2.0).unwrap()))]),
        },
    );
    document
        .schemas
        .get_mut(&SchemaId::from("s"))
        .expect("the chain schema exists")
        .fields
        .insert("extra".into(), optional_number_field("extra"));
    document
        .entities
        .get_mut(&EntityId::from("e1"))
        .expect("the chain row exists")
        .fields
        .insert(
            "extra".into(),
            Value::Formula(Expression::Reference(FieldRef::new("target", "input"))),
        );
    document
}

fn spare_field() -> FieldDefinition {
    FieldDefinition {
        id: "spare".into(),
        key: "spare".into(),
        display_name: None,
        number_format: None,
        date_format: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
    }
}

fn set_h(expression: Expression) -> SemanticPatchBody {
    SemanticPatchBody::command(SemanticCommand::SetFieldValue {
        field: FieldRef::new("e1", "h"),
        value: Value::Formula(expression),
    })
}

fn reference_g() -> Expression {
    Expression::Reference(FieldRef::new("e1", "g"))
}

fn number(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

/// Every edit kind one session has to publish, including structural edits, a
/// key rename, a field added and removed, an entity added and removed, and a
/// formula whose projection is too long for the canonical rendering.
fn fixed_chain_edits() -> Vec<SemanticPatchBody> {
    let percent = NumberFormat {
        decimals: 2,
        kind: NumberFormatKind::Percent,
        thousands_separator: true,
    };
    vec![
        set_f_text("new"),
        set_g_number(2.0),
        set_h(Expression::Multiply {
            left: Box::new(reference_g()),
            right: Box::new(number(2.0)),
        }),
        set_h(Expression::Reference(FieldRef::new("e2", "n"))),
        // `extra` reads a 3,900-byte entity key; lengthening its field key
        // pushes the projected formula past the 4,096-byte canonical limit.
        SemanticPatchBody::command(SemanticCommand::RenameSchemaField {
            schema: "t".into(),
            field: "input".into(),
            key: "i".repeat(200).into(),
        }),
        set_h(Expression::Add {
            left: Box::new(reference_g()),
            right: Box::new(number(1.0)),
        }),
        SemanticPatchBody::command(SemanticCommand::RenameSchemaField {
            schema: "s".into(),
            field: "g".into(),
            key: "amount".into(),
        }),
        SemanticPatchBody::command(SemanticCommand::RenameSchemaField {
            schema: "s".into(),
            field: "g".into(),
            key: "g".into(),
        }),
        SemanticPatchBody::command(SemanticCommand::SetSchemaFieldRequiredness {
            schema: "s".into(),
            field: "f".into(),
            required: false,
        }),
        SemanticPatchBody::command(SemanticCommand::SetSchemaFieldRequiredness {
            schema: "s".into(),
            field: "f".into(),
            required: true,
        }),
        SemanticPatchBody::command(SemanticCommand::AppendSchemaField {
            schema: "s".into(),
            field: spare_field(),
            values: BTreeMap::new(),
        }),
        SemanticPatchBody::command(SemanticCommand::SetFieldValue {
            field: FieldRef::new("e1", "spare"),
            value: Value::Number(Number::new(3.0).unwrap()),
        }),
        SemanticPatchBody::command(SemanticCommand::RemoveSchemaField {
            schema: "s".into(),
            field: spare_field(),
            values: BTreeMap::from([("e1".into(), Value::Number(Number::new(3.0).unwrap()))]),
        }),
        SemanticPatchBody::command(SemanticCommand::SetSchemaFieldDisplayName {
            schema: "s".into(),
            field: "f".into(),
            display_name: Some("Name".into()),
        }),
        SemanticPatchBody::command(SemanticCommand::SetFieldNumberFormat {
            schema: "s".into(),
            field: "g".into(),
            number_format: Some(percent),
        }),
        SemanticPatchBody::command(SemanticCommand::SetSchemaFieldOrder {
            schema: "s".into(),
            order: Some(vec![
                FieldId::from("g"),
                FieldId::from("h"),
                FieldId::from("f"),
                FieldId::from("extra"),
            ]),
        }),
        SemanticPatchBody::command(SemanticCommand::AppendEntity {
            entity: appended_entity("e3", "third", Some(reference_g())),
        }),
        SemanticPatchBody::command(SemanticCommand::SetFieldValue {
            field: FieldRef::new("e3", "f"),
            value: Value::Text("edited".into()),
        }),
        SemanticPatchBody::command(SemanticCommand::RemoveEntity {
            entity: "e3".into(),
        }),
        SemanticPatchBody::command(SemanticCommand::UpsertKeyedGroupedSumDefinition {
            definition: keyed_definition(),
        }),
        SemanticPatchBody::command(SemanticCommand::RemoveKeyedGroupedSumDefinition {
            definition: "summary".into(),
        }),
    ]
}

struct ChainRandom(u64);

impl ChainRandom {
    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

/// One seeded random edit that stays valid whatever the chain's current
/// document looks like: a value, formula, or presentation change.
fn random_chain_edit(random: &mut ChainRandom, index: usize) -> SemanticPatchBody {
    let unique = f64::from(u32::try_from(index + 1).unwrap_or(1)) + 0.5;
    let number_format = NumberFormat {
        decimals: u8::try_from(index % 11).unwrap_or(0),
        kind: NumberFormatKind::Plain,
        thousands_separator: index % 2 == 0,
    };
    let field_order = || {
        if index % 2 == 0 {
            vec![
                FieldId::from("h"),
                FieldId::from("f"),
                FieldId::from("extra"),
                FieldId::from("g"),
            ]
        } else {
            vec![
                FieldId::from("f"),
                FieldId::from("h"),
                FieldId::from("g"),
                FieldId::from("extra"),
            ]
        }
    };
    match random.next() % 8 {
        0 => set_f_text(&format!("value-{index}")),
        1 => set_g_number(unique),
        2 => set_h(Expression::Multiply {
            left: Box::new(reference_g()),
            right: Box::new(number(unique)),
        }),
        3 => set_h(Expression::Add {
            left: Box::new(reference_g()),
            right: Box::new(number(unique)),
        }),
        4 => set_h(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new("e2", "n"))),
            right: Box::new(number(unique)),
        }),
        5 => SemanticPatchBody::command(SemanticCommand::SetSchemaFieldDisplayName {
            schema: "s".into(),
            field: "f".into(),
            display_name: Some(format!("Name {index}")),
        }),
        6 => SemanticPatchBody::command(SemanticCommand::SetFieldNumberFormat {
            schema: "s".into(),
            field: "g".into(),
            number_format: Some(number_format),
        }),
        _ => SemanticPatchBody::command(SemanticCommand::SetSchemaFieldOrder {
            schema: "s".into(),
            order: Some(field_order()),
        }),
    }
}

struct FixedPublicationTime;

impl TrustedPublicationTimeSource for FixedPublicationTime {
    fn now(&mut self) -> TrustedInstant {
        TrustedInstant::new(1)
    }
}

fn publish_chain_step(
    session: &mut ResidentWorkspaceSession,
    candidate: &Document,
    installation: Option<crate::PreparedInstallation>,
) -> Result<(), SemanticPublicationError> {
    let scope = session.document_scope().clone();
    let revision = session.revision().clone();
    let mut time = FixedPublicationTime;
    let mut publication = session.publication_authority(&mut time);
    if let Some(installation) = installation {
        publication = publication.with_prepared(installation);
    }
    publication
        .publish_if_current(&scope, &revision, candidate, |_| Some(()))
        .map(|_| ())
}

fn chain_candidate(base: &Document, body: &SemanticPatchBody, label: &str) -> Document {
    v2_lifecycle()
        .plan_commands(base, body)
        .unwrap_or_else(|error| panic!("{label}: planning refused: {error:?}"))
        .0
}

fn assert_same_chain_proposal(
    label: &str,
    control: &Result<SemanticPatch, PatchLifecycleError>,
    control_lifecycle: &PatchLifecycle,
    prepared: &Result<SemanticPatch, PatchLifecycleError>,
    prepared_lifecycle: &PatchLifecycle,
) {
    match (control, prepared) {
        (Ok(control), Ok(prepared)) => {
            assert_eq!(control, prepared, "{label}: proposals differ");
            assert_same_evaluation(control_lifecycle, prepared_lifecycle);
        }
        (Err(control), Err(prepared)) => {
            assert_eq!(
                format!("{control:?}"),
                format!("{prepared:?}"),
                "{label}: refusals differ"
            );
        }
        _ => panic!("{label}: outcomes differ: {control:?} vs {prepared:?}"),
    }
}

/// Run one edit through `propose_prepared` and plain `propose`, require the
/// same proposal, then publish through each session the way its path does.
fn assert_chain_step(
    label: &str,
    body: &SemanticPatchBody,
    prepared_session: &mut ResidentWorkspaceSession,
    control_session: &mut ResidentWorkspaceSession,
) -> Result<SemanticPatch, PatchLifecycleError> {
    let base = prepared_session.document().clone();
    assert_eq!(&base, control_session.document(), "{label}: bases diverge");
    let revision = prepared_session.revision().clone();
    assert_eq!(
        &revision,
        control_session.revision(),
        "{label}: revisions diverge"
    );
    let candidate = chain_candidate(&base, body, label);

    let prepared = prepared_session.prepare_evaluation(candidate.clone());
    let mut prepared_lifecycle = v2_lifecycle();
    let prepared_result = propose_with(
        &mut prepared_lifecycle,
        &base,
        revision.as_str(),
        body.clone(),
        &prepared,
    );
    let mut control_lifecycle = v2_lifecycle();
    let control_result = propose_plain(
        &mut control_lifecycle,
        &base,
        revision.as_str(),
        body.clone(),
    );
    assert_same_chain_proposal(
        label,
        &control_result,
        &control_lifecycle,
        &prepared_result,
        &prepared_lifecycle,
    );
    let control_patch = control_result?;

    let installation = prepared.into_installable();
    let prepared_publication = publish_chain_step(prepared_session, &candidate, Some(installation));
    let control_publication = publish_chain_step(control_session, &candidate, None);
    assert_eq!(
        control_publication, prepared_publication,
        "{label}: publications differ"
    );
    if let Err(error) = &prepared_publication {
        assert_eq!(
            error,
            &SemanticPublicationError::NoChange,
            "{label}: publication failed"
        );
    }
    Ok(control_patch)
}

/// Differential: acceptance-case edits and 50 seeded random edits published
/// one after another through a resident session propose exactly what a run
/// that always preflights both snapshots in full proposes.
#[test]
fn chained_prepared_proposals_match_full_preflights() {
    let document = preflight_chain_document();
    let mut prepared_session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let mut control_session = ResidentWorkspaceSession::new("scope".into(), document);
    let mut random = ChainRandom(0x0678_0000_0000_0001);
    let fixed = fixed_chain_edits()
        .into_iter()
        .enumerate()
        .map(|(index, body)| (format!("fixed edit {index}"), body));
    let random = (0..50).map(|index| {
        (
            format!("random edit {index}"),
            random_chain_edit(&mut random, index),
        )
    });
    for (label, body) in fixed.chain(random) {
        let _ = assert_chain_step(&label, &body, &mut prepared_session, &mut control_session);
    }
    assert_eq!(
        prepared_session.export_snapshot(),
        control_session.export_snapshot(),
        "the chains must end on the same document and revision"
    );
}

fn publish_prepared_edit(
    session: &mut ResidentWorkspaceSession,
    body: SemanticPatchBody,
    label: &str,
) {
    let base = session.document().clone();
    let candidate = chain_candidate(&base, &body, label);
    let prepared = session.prepare_evaluation(candidate.clone());
    let mut lifecycle = v2_lifecycle();
    let revision = session.revision().clone();
    propose_with(&mut lifecycle, &base, revision.as_str(), body, &prepared)
        .unwrap_or_else(|error| panic!("{label}: proposal refused: {error:?}"));
    let installation = prepared.into_installable();
    assert_eq!(
        publish_chain_step(session, &candidate, Some(installation)),
        Ok(()),
        "{label}: publication"
    );
}

#[test]
fn first_resident_v1_scalar_edit_reuses_preflight_and_publishes_the_same_diff() {
    let document = formula_document();
    evaluation_counters::reset();
    let mut session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    assert_eq!(evaluation_counters::preflight_counts(), (1, 1));
    let body = set_g_number(2.0);
    let candidate = chain_candidate(&document, &body, "first scalar edit");
    let revision = session.revision().clone();
    let mut plain = formula_lifecycle();
    let expected = propose_plain(&mut plain, &document, revision.as_str(), body.clone()).unwrap();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate.clone());
    assert!(prepared.base_preflight_succeeded());
    assert!(prepared.candidate_preflight_succeeded());
    let mut reused = formula_lifecycle();
    let actual = propose_with(&mut reused, &document, revision.as_str(), body, &prepared).unwrap();
    assert_eq!(evaluation_counters::preflight_counts(), (0, 0));
    assert_eq!(actual, expected);
    assert_same_evaluation(&plain, &reused);
    let installation = prepared.into_installable();
    assert_eq!(
        publish_chain_step(&mut session, &candidate, Some(installation)),
        Ok(())
    );
    assert_eq!(
        session.document(),
        &plain.evaluation_memo.unwrap().evaluated.document
    );
}

#[test]
fn first_resident_v2_unset_batch_reuses_the_open_preflight() {
    let mut document = formula_document();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("f")
        .unwrap()
        .required = false;
    let mut second = document.entities["e1"].clone();
    second.id = "e2".into();
    second.key = "row2".into();
    document.entities.insert(second.id.clone(), second);
    let mut session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let body = SemanticPatchBody::atomic_batch(vec![
        SemanticCommand::UnsetField {
            field: FieldRef::new("e1", "f"),
        },
        SemanticCommand::UnsetField {
            field: FieldRef::new("e2", "f"),
        },
    ])
    .unwrap();
    let candidate = chain_candidate(&document, &body, "first clear");
    let revision = session.revision().clone();
    let mut plain = v2_lifecycle();
    let expected = propose_plain(&mut plain, &document, revision.as_str(), body.clone()).unwrap();

    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate.clone());
    assert!(prepared.base_preflight_succeeded());
    let mut reused = v2_lifecycle();
    let actual = propose_with(&mut reused, &document, revision.as_str(), body, &prepared).unwrap();
    assert_eq!(evaluation_counters::preflight_counts(), (0, 0));
    assert_eq!(actual, expected);
    assert_same_evaluation(&plain, &reused);
    let installation = prepared.into_installable();
    assert_eq!(
        publish_chain_step(&mut session, &candidate, Some(installation)),
        Ok(())
    );
    assert_eq!(session.document(), &candidate);
}

#[test]
fn a_prepared_candidate_with_a_projection_failure_keeps_the_exact_error() {
    let candidate = unprojectable_document();
    let mut document = candidate.clone();
    document
        .entities
        .get_mut("e1")
        .unwrap()
        .fields
        .insert("h".into(), Value::Formula(reference_g()));
    let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let prepared = session.prepare_evaluation(candidate.clone());
    assert!(prepared.base_preflight_succeeded());
    assert!(!prepared.candidate_preflight_succeeded());
    let expected =
        crate::finalize_edit_with_validation(&document, candidate.clone(), None).unwrap_err();
    let actual =
        crate::finalize_edit_with_validation(&document, candidate, Some(&prepared)).unwrap_err();
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    let WorkspaceError::FormulaProjection { field, .. } = actual else {
        panic!("expected formula projection failure");
    };
    assert_eq!(field, FieldRef::new("e1", "h"));
}

#[test]
fn a_fresh_resident_with_failing_preflight_keeps_the_first_scalar_edit_error() {
    for v2 in [false, true] {
        let document = unprojectable_document();
        let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
        let body = set_f_text("new");
        let candidate = chain_candidate(&document, &body, "failing initial document");
        let prepared = session.prepare_evaluation(candidate);
        assert!(!prepared.base_preflight_succeeded());
        assert!(!prepared.candidate_preflight_succeeded());
        let revision = session.revision().as_str();
        let mut plain = if v2 {
            v2_lifecycle()
        } else {
            formula_lifecycle()
        };
        let expected = propose_plain(&mut plain, &document, revision, body.clone()).unwrap_err();
        let mut reused = if v2 {
            v2_lifecycle()
        } else {
            formula_lifecycle()
        };
        let actual = propose_with(&mut reused, &document, revision, body, &prepared).unwrap_err();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        let PatchLifecycleError::CommandRejected { source } = actual else {
            panic!("expected command rejection");
        };
        let WorkspaceError::FormulaProjection { field, .. } = source.as_ref() else {
            panic!("expected formula projection failure");
        };
        assert_eq!(field, &FieldRef::new("e1", "h"));
    }
}

/// The preflight counter: a value edit on a known-good base runs none, and a
/// formula edit projects only the formula that changed.
#[test]
fn a_value_edit_and_a_formula_edit_reuse_the_base_preflight() {
    let mut session = ResidentWorkspaceSession::new("scope".into(), formula_document());
    publish_prepared_edit(&mut session, set_g_number(2.0), "the base edit");

    let base = session.document().clone();
    let body = set_f_text("new");
    let candidate = chain_candidate(&base, &body, "a value edit");
    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate.clone());
    let mut lifecycle = v2_lifecycle();
    let revision = session.revision().clone();
    propose_with(&mut lifecycle, &base, revision.as_str(), body, &prepared)
        .expect("the value edit is accepted");
    assert_eq!(
        evaluation_counters::preflight_counts(),
        (0, 0),
        "a value edit on a known-good base must run no preflight"
    );
    let installation = prepared.into_installable();
    assert_eq!(
        publish_chain_step(&mut session, &candidate, Some(installation)),
        Ok(())
    );

    let base = session.document().clone();
    let body = set_h(Expression::Add {
        left: Box::new(reference_g()),
        right: Box::new(number(5.0)),
    });
    let candidate = chain_candidate(&base, &body, "a formula edit");
    evaluation_counters::reset();
    let prepared = session.prepare_evaluation(candidate);
    let mut lifecycle = v2_lifecycle();
    let revision = session.revision().clone();
    propose_with(&mut lifecycle, &base, revision.as_str(), body, &prepared)
        .expect("the formula edit is accepted");
    assert_eq!(
        evaluation_counters::preflight_counts(),
        (1, 1),
        "a formula edit must project only its changed formula"
    );
}

/// A base known not to have passed the preflight keeps today's error, and a
/// publication that never established the outcome leaves it unknown.
#[test]
fn a_published_base_that_fails_preflight_keeps_its_error() {
    let document = formula_document();
    let mut prepared_session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let mut control_session = ResidentWorkspaceSession::new("scope".into(), document);
    let failing = unprojectable_document();
    assert_eq!(
        publish_chain_step(&mut prepared_session, &failing, None),
        Ok(())
    );
    assert_eq!(
        publish_chain_step(&mut control_session, &failing, None),
        Ok(())
    );

    let body = SemanticPatchBody::command(SemanticCommand::RenameSchemaField {
        schema: "t".into(),
        field: "input".into(),
        key: "i".into(),
    });
    let result = assert_chain_step(
        "a published failing base",
        &body,
        &mut prepared_session,
        &mut control_session,
    );
    let PatchLifecycleError::CommandRejected { source } =
        result.expect_err("the base keeps its projection refusal")
    else {
        panic!("expected command rejection");
    };
    assert!(
        matches!(source.as_ref(), WorkspaceError::FormulaProjection { .. }),
        "{source:?}"
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

// --- The scoped formula diff reports exactly the full diff (#678) ---

/// Assert the scoped formula-projection diff equals the full one for one pair.
///
/// The scope comes from the prepared evaluation's projection changes, exactly as
/// `finalize_evaluation_v2` reads it. Both sides compare the same projected
/// snapshots and the same calculations, so any difference is a lost change.
fn assert_scoped_diff_matches_full(base: &Document, candidate: &Document, label: &str) {
    let session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let prepared = session.prepare_evaluation(candidate.clone());
    let scope = prepared.projection_changes().diff_scope();

    let before_projection = super::formula_projection_document(base);
    let after_projection = super::formula_projection_document(candidate);
    let before_calculation = calculate(&before_projection)
        .unwrap_or_else(|error| panic!("{label}: base calculation refused: {error:?}"));
    let after_calculation = calculate(&after_projection)
        .unwrap_or_else(|error| panic!("{label}: candidate calculation refused: {error:?}"));
    let full = diff_with_calculations(
        &before_projection,
        &after_projection,
        &before_calculation,
        &after_calculation,
    )
    .unwrap_or_else(|error| panic!("{label}: full diff refused: {error:?}"));
    let scoped = diff_changes_scoped(
        &before_projection,
        &after_projection,
        &before_calculation,
        &after_calculation,
        &scope,
    );
    assert_eq!(
        scoped.as_slice(),
        full.changes(),
        "{label}: scoped formula diff differs from the full diff"
    );
    // The reuse gate projects only the scope, so that diff must match too.
    let scoped_on_scoped_projection = diff_changes_scoped(
        &super::scoped_formula_projection_document(base, &scope),
        &super::scoped_formula_projection_document(candidate, &scope),
        &before_calculation,
        &after_calculation,
        &scope,
    );
    assert_eq!(
        scoped_on_scoped_projection.as_slice(),
        full.changes(),
        "{label}: the diff on the scoped projection differs from the full diff"
    );
}

/// Acceptance 1: the scoped diff equals the full diff for every v2 evidence
/// case the prepared path accepts.
#[test]
fn scoped_formula_diff_matches_full_diff_for_every_acceptance_case() {
    for (label, document, body) in acceptance_cases() {
        let candidate = v2_lifecycle()
            .plan_commands(&document, &body)
            .unwrap_or_else(|error| panic!("{label}: planning refused: {error:?}"))
            .0;
        assert_scoped_diff_matches_full(&document, &candidate, label);
    }
}

/// One deterministic linear congruential generator for the seeded random edits.
struct SeededRng(u64);

impl SeededRng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("a document-sized bound fits u64");
        usize::try_from(self.next() % bound).expect("the remainder is below the bound")
    }
}

/// A handful of rows with three literal Number columns, two row formulas and a
/// column total, so edits move both row formulas and an aggregate.
fn mid_sized_document() -> Document {
    const ROWS: usize = 24;
    let mut document = Document::empty("doc", "Mid");
    document.schemas.insert(
        "s".into(),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "rows".into(),
            display_name: None,
            fields: BTreeMap::from([
                ("name".into(), text_field("name")),
                ("n0".into(), number_field("n0")),
                ("n1".into(), number_field("n1")),
                ("n2".into(), number_field("n2")),
                ("h0".into(), number_field("h0")),
                ("h1".into(), number_field("h1")),
                ("total".into(), number_field("total")),
            ]),
        },
    );
    for index in 0..ROWS {
        let entity_id = format!("e{index:02}");
        document.entities.insert(
            entity_id.clone().into(),
            mid_sized_entity(&entity_id, index),
        );
    }
    document
}

fn mid_sized_entity(entity_id: &str, index: usize) -> Entity {
    let value = Number::new(f64::from(
        u32::try_from(index).expect("a row index fits u32"),
    ))
    .unwrap();
    Entity {
        id: entity_id.into(),
        key: format!("row{index}").into(),
        schema: "s".into(),
        fields: BTreeMap::from([
            ("name".into(), Value::Text(format!("row {index}"))),
            ("n0".into(), Value::Number(value)),
            ("n1".into(), Value::Number(value)),
            ("n2".into(), Value::Number(value)),
            (
                "h0".into(),
                Value::Formula(Expression::Reference(FieldRef::new(entity_id, "n0"))),
            ),
            (
                "h1".into(),
                Value::Formula(Expression::Add {
                    left: Box::new(Expression::Reference(FieldRef::new(entity_id, "n0"))),
                    right: Box::new(Expression::Reference(FieldRef::new(entity_id, "n1"))),
                }),
            ),
            (
                "total".into(),
                Value::Formula(Expression::ColumnTotal(ColumnRef::new("s", "n0"))),
            ),
        ]),
    }
}

/// Apply one deterministic random edit that keeps the document valid.
fn apply_random_edit(document: &mut Document, rng: &mut SeededRng) {
    let entity_ids = document.entities.keys().cloned().collect::<Vec<_>>();
    let entity_id = entity_ids[rng.below(entity_ids.len())].clone();
    match rng.below(6) {
        0 => {
            let field = ["n0", "n1", "n2"][rng.below(3)];
            let value = f64::from(u32::try_from(rng.below(1_000)).expect("below 1,000")) / 10.0;
            document
                .entities
                .get_mut(&entity_id)
                .expect("the row came from the document")
                .fields
                .insert(field.into(), Value::Number(Number::new(value).unwrap()));
        }
        1 => {
            document
                .entities
                .get_mut(&entity_id)
                .expect("the row came from the document")
                .fields
                .insert(
                    "name".into(),
                    Value::Text(format!("renamed {}", rng.below(10))),
                );
        }
        2 => {
            let field = ["h0", "h1"][rng.below(2)];
            document
                .entities
                .get_mut(&entity_id)
                .expect("the row came from the document")
                .fields
                .insert(
                    field.into(),
                    Value::Formula(random_expression(entity_id.as_str(), rng)),
                );
        }
        3 => {
            let entity_id = format!("x{}", rng.next());
            let mut entity = mid_sized_entity(&entity_id, rng.below(100));
            // The added row's key must not repeat an existing row's key, which
            // document admission requires to be unique.
            entity.key = format!("added{entity_id}").into();
            document.entities.insert(entity_id.into(), entity);
        }
        4 => {
            if entity_ids.len() > 8 {
                document.entities.remove(&entity_id);
            }
        }
        _ => {
            let field = ["n0", "n1", "h0"][rng.below(3)];
            let display_name = Some(format!("Column {}", rng.below(10)));
            document
                .schemas
                .get_mut("s")
                .expect("the schema is present")
                .fields
                .get_mut(field)
                .expect("the field is present")
                .display_name = display_name;
        }
    }
}

fn random_expression(entity_id: &str, rng: &mut SeededRng) -> Expression {
    let reference = |field: &str| Expression::Reference(FieldRef::new(entity_id, field));
    match rng.below(4) {
        0 => Expression::Number(
            Number::new(f64::from(u32::try_from(rng.below(100)).expect("below 100"))).unwrap(),
        ),
        1 => reference("n0"),
        2 => Expression::Add {
            left: Box::new(reference("n0")),
            right: Box::new(reference("n1")),
        },
        _ => Expression::Multiply {
            left: Box::new(reference("n1")),
            right: Box::new(reference("n2")),
        },
    }
}

/// Acceptance 1: 50 seeded random edits keep the scoped diff equal to the full
/// diff on a mid-sized document.
#[test]
fn scoped_formula_diff_matches_full_diff_for_random_edits() {
    let base = mid_sized_document();
    let mut rng = SeededRng(0x0678_0026);
    for case in 0..50 {
        let mut candidate = base.clone();
        for _ in 0..=rng.below(3) {
            apply_random_edit(&mut candidate, &mut rng);
        }
        assert_scoped_diff_matches_full(&base, &candidate, &format!("random edit {case}"));
    }
}

// --- The scoped canonical delta reports exactly the full delta (#678) ---

/// Assert the scoped canonical delta equals the full one for one pair.
///
/// The scope comes from the prepared evaluation's projection changes, exactly
/// as `finalize_evaluation_v2` reads it, and both sides calculate the same two
/// documents, so any difference is a lost or invented fact.
fn assert_scoped_delta_matches_full(base: &Document, candidate: &Document, label: &str) {
    let session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let prepared = session.prepare_evaluation(candidate.clone());
    let scope = prepared.projection_changes().diff_scope();

    let before_calculation = calculate(base)
        .unwrap_or_else(|error| panic!("{label}: base calculation refused: {error:?}"));
    let after_calculation = calculate(candidate)
        .unwrap_or_else(|error| panic!("{label}: candidate calculation refused: {error:?}"));
    let full = canonical_delta_with_calculations(
        CANONICAL_SEMANTIC_DELTA_V2,
        base,
        candidate,
        &before_calculation,
        &after_calculation,
    )
    .unwrap_or_else(|error| panic!("{label}: full delta refused: {error:?}"));
    let scoped = canonical_delta_scoped(
        CANONICAL_SEMANTIC_DELTA_V2,
        base,
        candidate,
        &before_calculation,
        &after_calculation,
        &scope,
    )
    .unwrap_or_else(|error| panic!("{label}: scoped delta refused: {error:?}"));
    assert_eq!(
        scoped, full,
        "{label}: scoped canonical delta differs from the full delta"
    );
}

/// Acceptance 1: the scoped delta equals the full delta for every v2 evidence
/// case the prepared path accepts. The constraint-set case carries the field
/// constraint change; no command changes the title, so the direct title
/// comparison is covered by the diff-engine scoped test instead.
#[test]
fn scoped_canonical_delta_matches_full_delta_for_every_acceptance_case() {
    for (label, document, body) in acceptance_cases() {
        let candidate = v2_lifecycle()
            .plan_commands(&document, &body)
            .unwrap_or_else(|error| panic!("{label}: planning refused: {error:?}"))
            .0;
        assert_scoped_delta_matches_full(&document, &candidate, label);
    }
}

/// Acceptance 1: 50 seeded random edits keep the scoped delta equal to the full
/// delta on a mid-sized document.
#[test]
fn scoped_canonical_delta_matches_full_delta_for_random_edits() {
    let base = mid_sized_document();
    let mut rng = SeededRng(0x0678_0026);
    for case in 0..50 {
        let mut candidate = base.clone();
        for _ in 0..=rng.below(3) {
            apply_random_edit(&mut candidate, &mut rng);
        }
        assert_scoped_delta_matches_full(&base, &candidate, &format!("random edit {case}"));
    }
}

/// `keyed_document` plus a second schema a changed definition can bind.
fn alternate_keyed_document() -> Document {
    let mut document = keyed_document();
    document.schemas.insert(
        "orders2".into(),
        multi_field_schema(
            "orders2",
            "orders2",
            &[("lookup", FieldType::Text), ("quantity", FieldType::Number)],
        ),
    );
    document
}

/// `keyed_definition` bound to the second orders schema, so it differs without
/// needing a new field.
fn changed_keyed_definition() -> KeyedGroupedSumDefinition {
    let mut definition = keyed_definition();
    definition.orders.schema = "orders2".into();
    definition
}

/// Acceptance 1: a saved keyed grouped-sum definition added, changed and
/// removed all keep the scoped delta equal to the full delta. Definitions are
/// not part of `DiffScope`, so this is the part the scoped delta compares in
/// full.
#[test]
fn scoped_canonical_delta_matches_full_delta_for_definition_edits() {
    let added_base = keyed_document();
    let added = v2_lifecycle()
        .plan_commands(
            &added_base,
            &SemanticPatchBody::command(SemanticCommand::UpsertKeyedGroupedSumDefinition {
                definition: keyed_definition(),
            }),
        )
        .expect("adding a definition plans")
        .0;
    assert_scoped_delta_matches_full(&added_base, &added, "a definition added");

    let mut changed_base = alternate_keyed_document();
    changed_base
        .keyed_grouped_sum_definitions
        .insert("summary".into(), keyed_definition());
    let changed = v2_lifecycle()
        .plan_commands(
            &changed_base,
            &SemanticPatchBody::command(SemanticCommand::UpsertKeyedGroupedSumDefinition {
                definition: changed_keyed_definition(),
            }),
        )
        .expect("changing a definition plans")
        .0;
    assert_scoped_delta_matches_full(&changed_base, &changed, "a definition changed");

    let mut removed_base = keyed_document();
    removed_base
        .keyed_grouped_sum_definitions
        .insert("summary".into(), keyed_definition());
    let removed = v2_lifecycle()
        .plan_commands(
            &removed_base,
            &SemanticPatchBody::command(SemanticCommand::RemoveKeyedGroupedSumDefinition {
                definition: "summary".into(),
            }),
        )
        .expect("removing a definition plans")
        .0;
    assert_scoped_delta_matches_full(&removed_base, &removed, "a definition removed");
}

/// Acceptance: a resident edit proves its base by occurrence and revision, so
/// its memo keeps no whole-document copy and preview and execution reuse one
/// evaluation without comparing documents.
#[test]
fn a_resident_value_edit_copies_no_memo_base_and_compares_no_document() {
    let document = document();
    let mut session = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let mut lifecycle = lifecycle(false);
    let body = set_f("new");
    let candidate = lifecycle.plan_commands(&document, &body).unwrap().0;
    let revision = session.revision().clone();

    let prepared = session.prepare_evaluation(candidate);
    evaluation_counters::reset();
    let witness = session.witness();
    let proposal = lifecycle
        .propose_prepared_witnessed(
            &"scope".into(),
            &witness,
            super::ProposalRequest::new("proposal".into(), revision.clone(), body, "human".into()),
            TrustedInstant::new(1),
            Some(&prepared),
        )
        .unwrap();
    lifecycle
        .preview_witnessed(
            &"scope".into(),
            &witness,
            proposal.id(),
            &"human".into(),
            TrustedInstant::new(2),
        )
        .unwrap();
    let installation = prepared.into_installable();
    let mut time = FixedPublicationTime;
    let mut publication = session
        .publication_authority(&mut time)
        .with_prepared(installation);
    let receipt = lifecycle
        .execute(
            proposal.id(),
            None,
            &"human".into(),
            &mut publication,
            TrustedInstant::new(3),
        )
        .unwrap();

    assert_eq!(evaluation_counters::memo_counts(), (0, 0));
    assert_eq!(lifecycle.evaluations.get(), 1);
    assert!(receipt.verified);
    assert_eq!(*session.document(), with_f_set(document, "new"));
}

/// Acceptance: one occurrence's witness never authorizes another occurrence's
/// memo, even when both hold an equal document at the same revision token.
#[test]
fn a_witnessed_memo_is_never_used_by_another_session() {
    let document = document();
    let session_a = ResidentWorkspaceSession::new("scope".into(), document.clone());
    let mut lifecycle = lifecycle(false);
    let body = set_f("new");
    let candidate = lifecycle.plan_commands(&document, &body).unwrap().0;
    let revision = session_a.revision().clone();

    let prepared = session_a.prepare_evaluation(candidate);
    let witness = session_a.witness();
    let proposal = lifecycle
        .propose_prepared_witnessed(
            &"scope".into(),
            &witness,
            super::ProposalRequest::new("proposal".into(), revision.clone(), body, "human".into()),
            TrustedInstant::new(1),
            Some(&prepared),
        )
        .unwrap();
    lifecycle
        .preview_witnessed(
            &"scope".into(),
            &witness,
            proposal.id(),
            &"human".into(),
            TrustedInstant::new(2),
        )
        .unwrap();
    assert_eq!(lifecycle.evaluations.get(), 1);

    // Session B holds the same document and the same revision token, but it is
    // a different occurrence, so the memo is not its base.
    let mut session_b = ResidentWorkspaceSession::new("scope".into(), document.clone());
    assert_eq!(session_b.revision(), &revision);
    let mut time = FixedPublicationTime;
    let mut publication = session_b.publication_authority(&mut time);
    lifecycle
        .execute(
            proposal.id(),
            None,
            &"human".into(),
            &mut publication,
            TrustedInstant::new(3),
        )
        .unwrap();

    assert_eq!(
        lifecycle.evaluations.get(),
        2,
        "session B evaluates because the memo belongs to session A"
    );
    assert_eq!(*session_b.document(), with_f_set(document, "new"));
}

/// Acceptance: a witnessed base names one revision, so equal content at the
/// next revision is not that base.
#[test]
fn a_witnessed_memo_base_never_matches_another_revision_of_equal_content() {
    let document = document();
    let session = ResidentWorkspaceSession::new("scope".into(), document);
    let same_revision = session.revision().clone();
    let later_revision = SemanticRevision::from("resident/1");
    let base = super::MemoBase::Witnessed {
        occurrence: session.witness().occurrence_id(),
        revision: same_revision.clone(),
    };

    let current = crate::resident_session::DocumentWitness::for_test(
        session.occurrence(),
        &same_revision,
        session.document(),
    );
    assert!(base.matches(Some(&current), session.document()));

    let later = crate::resident_session::DocumentWitness::for_test(
        session.occurrence(),
        &later_revision,
        session.document(),
    );
    assert!(
        !base.matches(Some(&later), session.document()),
        "the same content at the next revision is not the memo's base"
    );
}

/// Acceptance: a non-resident caller keeps the copy-and-compare memo path.
#[test]
fn a_non_resident_proposal_copies_and_compares_its_memo_base() {
    let mut lifecycle = lifecycle(false);
    let document = document();
    evaluation_counters::reset();
    let proposal = propose(&mut lifecycle, &document, "proposal", set_f("new")).unwrap();
    assert_eq!(evaluation_counters::memo_counts(), (1, 0));
    lifecycle
        .preview(
            &"scope".into(),
            &document,
            &"r1".into(),
            &proposal,
            &"human".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(evaluation_counters::memo_counts(), (1, 1));
    let mut host = Host::new(document.clone());
    lifecycle
        .execute(
            &proposal,
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(20),
        )
        .unwrap();
    assert_eq!(evaluation_counters::memo_counts(), (1, 2));
    assert_eq!(lifecycle.evaluations.get(), 1);
}

// --- Proposal-only previews skip only the evidence they discard (#678) ---

/// The revision token every fresh resident session starts at.
fn initial_revision(document: &Document) -> SemanticRevision {
    ResidentWorkspaceSession::new("scope".into(), document.clone())
        .revision()
        .clone()
}

fn propose_witnessed_base(
    lifecycle: &mut PatchLifecycle,
    document: &Document,
    revision: &SemanticRevision,
    body: SemanticPatchBody,
) -> ProposalId {
    lifecycle
        .propose(
            &"scope".into(),
            document,
            revision,
            super::ProposalRequest::new("proposal".into(), revision.clone(), body, "human".into()),
            TrustedInstant::new(1),
        )
        .expect("the proposal is authorized")
        .id()
        .clone()
}

fn full_preview_proposal(
    lifecycle: &mut PatchLifecycle,
    v2: bool,
    witness: &DocumentWitness<'_>,
    proposal: &ProposalId,
    viewer: &PrincipalId,
) -> Result<SemanticPatch, PatchLifecycleError> {
    if v2 {
        lifecycle
            .preview_v2_witnessed(
                &"scope".into(),
                witness,
                proposal,
                viewer,
                TrustedInstant::new(2),
            )
            .map(|preview| preview.proposal)
    } else {
        lifecycle
            .preview_witnessed(
                &"scope".into(),
                witness,
                proposal,
                viewer,
                TrustedInstant::new(2),
            )
            .map(|preview| preview.proposal)
    }
}

fn proposal_only_preview(
    lifecycle: &mut PatchLifecycle,
    v2: bool,
    witness: &DocumentWitness<'_>,
    proposal: &ProposalId,
    viewer: &PrincipalId,
) -> Result<SemanticPatch, PatchLifecycleError> {
    if v2 {
        lifecycle.preview_v2_proposal_witnessed(
            &"scope".into(),
            witness,
            proposal,
            viewer,
            TrustedInstant::new(2),
        )
    } else {
        lifecycle.preview_proposal_witnessed(
            &"scope".into(),
            witness,
            proposal,
            viewer,
            TrustedInstant::new(2),
        )
    }
}

fn assert_same_retained_state(
    full: &PatchLifecycle,
    full_proposal: &ProposalId,
    proposal_only: &PatchLifecycle,
    proposal_only_proposal: &ProposalId,
) {
    assert_eq!(
        full.retained_proposal_state(full_proposal).unwrap(),
        proposal_only
            .retained_proposal_state(proposal_only_proposal)
            .unwrap(),
    );
}

/// The formula document with `h = g + k`, so an edit to `g` derives one more
/// disclosure than the same edit on [`formula_document`].
fn formula_document_with_extra_dependency() -> Document {
    let mut document = formula_document();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .insert("k".into(), number_field("k"));
    let entity = document.entities.get_mut("e1").unwrap();
    entity
        .fields
        .insert("k".into(), Value::Number(Number::new(2.0).unwrap()));
    entity.fields.insert(
        "h".into(),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Reference(FieldRef::new("e1", "g"))),
            right: Box::new(Expression::Reference(FieldRef::new("e1", "k"))),
        }),
    );
    document
}

/// Acceptance: for the same proposal, a proposal-only preview returns the
/// same proposal and leaves the same retained lifecycle state as the full
/// preview it stands in for.
#[test]
fn a_proposal_only_preview_matches_the_full_preview() {
    for v2 in [false, true] {
        let document = document();
        let revision = initial_revision(&document);
        let mut full = lifecycle(v2);
        let mut proposal_only = lifecycle(v2);
        let full_proposal = propose_witnessed_base(&mut full, &document, &revision, set_f("new"));
        let proposal_only_proposal =
            propose_witnessed_base(&mut proposal_only, &document, &revision, set_f("new"));

        let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
        let witness = session.witness();
        let full_patch =
            full_preview_proposal(&mut full, v2, &witness, &full_proposal, &"human".into())
                .expect("the full preview is authorized");
        let proposal = proposal_only_preview(
            &mut proposal_only,
            v2,
            &witness,
            &proposal_only_proposal,
            &"human".into(),
        )
        .expect("the proposal-only preview is authorized");

        assert_eq!(full_patch, proposal, "v2 = {v2}");
        assert_same_retained_state(
            &full,
            &full_proposal,
            &proposal_only,
            &proposal_only_proposal,
        );
    }
}

/// Acceptance: a stale base is refused with the same error and state by the
/// full and the proposal-only preview.
#[test]
fn a_proposal_only_preview_refuses_a_stale_base_like_the_full_preview() {
    for v2 in [false, true] {
        let document = document();
        let revision = initial_revision(&document);
        let mut full = lifecycle(v2);
        let mut proposal_only = lifecycle(v2);
        let full_proposal = propose_witnessed_base(&mut full, &document, &revision, set_f("new"));
        let proposal_only_proposal =
            propose_witnessed_base(&mut proposal_only, &document, &revision, set_f("new"));

        let stale_revision = SemanticRevision::from("resident/9");
        let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
        let witness =
            DocumentWitness::for_test(session.occurrence(), &stale_revision, session.document());

        let full_error =
            full_preview_proposal(&mut full, v2, &witness, &full_proposal, &"human".into())
                .unwrap_err();
        let proposal_error = proposal_only_preview(
            &mut proposal_only,
            v2,
            &witness,
            &proposal_only_proposal,
            &"human".into(),
        )
        .unwrap_err();

        assert!(
            matches!(full_error, PatchLifecycleError::Stale),
            "v2 = {v2}: {full_error:?}"
        );
        assert!(
            matches!(proposal_error, PatchLifecycleError::Stale),
            "v2 = {v2}: {proposal_error:?}"
        );
        assert_eq!(
            full.proposal_history(&full_proposal).unwrap().last(),
            Some(&PatchLifecycleState::Stale)
        );
        assert_same_retained_state(
            &full,
            &full_proposal,
            &proposal_only,
            &proposal_only_proposal,
        );
    }
}

/// Acceptance: a base whose fresh evaluation derives a different footprint is
/// refused as a conflict with the same error and state by both previews.
#[test]
fn a_proposal_only_preview_refuses_a_changed_footprint_like_the_full_preview() {
    for v2 in [false, true] {
        let base = formula_document();
        let revision = initial_revision(&base);
        let mut full = if v2 {
            v2_lifecycle()
        } else {
            formula_lifecycle()
        };
        let mut proposal_only = if v2 {
            v2_lifecycle()
        } else {
            formula_lifecycle()
        };
        let full_proposal = propose_witnessed_base(&mut full, &base, &revision, set_g_number(2.0));
        let proposal_only_proposal =
            propose_witnessed_base(&mut proposal_only, &base, &revision, set_g_number(2.0));

        // Another document at the same revision token whose formula reads one
        // more field derives a wider disclosure footprint than the proposal
        // retained.
        let session =
            ResidentWorkspaceSession::new("scope".into(), formula_document_with_extra_dependency());
        let witness = session.witness();

        let full_error =
            full_preview_proposal(&mut full, v2, &witness, &full_proposal, &"human".into())
                .unwrap_err();
        let proposal_error = proposal_only_preview(
            &mut proposal_only,
            v2,
            &witness,
            &proposal_only_proposal,
            &"human".into(),
        )
        .unwrap_err();

        assert!(
            matches!(full_error, PatchLifecycleError::ApprovalBindingMismatch),
            "v2 = {v2}: {full_error:?}"
        );
        assert!(
            matches!(proposal_error, PatchLifecycleError::ApprovalBindingMismatch),
            "v2 = {v2}: {proposal_error:?}"
        );
        assert!(
            full.proposal_history(&full_proposal)
                .unwrap()
                .contains(&PatchLifecycleState::Conflict)
        );
        assert_same_retained_state(
            &full,
            &full_proposal,
            &proposal_only,
            &proposal_only_proposal,
        );
    }
}

/// Acceptance: an undisclosed proposal is refused with the same error and
/// leaves the same state under both previews.
#[test]
fn a_proposal_only_preview_refuses_an_unauthorized_viewer_like_the_full_preview() {
    for v2 in [false, true] {
        let document = document();
        let revision = initial_revision(&document);
        let mut full = lifecycle(v2);
        let mut proposal_only = lifecycle(v2);
        let full_proposal = propose_witnessed_base(&mut full, &document, &revision, set_f("new"));
        let proposal_only_proposal =
            propose_witnessed_base(&mut proposal_only, &document, &revision, set_f("new"));
        for lifecycle in [&mut full, &mut proposal_only] {
            lifecycle
                .register_principal("outsider".into(), PrincipalKind::Human)
                .unwrap();
        }

        let session = ResidentWorkspaceSession::new("scope".into(), document.clone());
        let witness = session.witness();
        let full_error =
            full_preview_proposal(&mut full, v2, &witness, &full_proposal, &"outsider".into())
                .unwrap_err();
        let proposal_error = proposal_only_preview(
            &mut proposal_only,
            v2,
            &witness,
            &proposal_only_proposal,
            &"outsider".into(),
        )
        .unwrap_err();

        assert!(
            matches!(full_error, PatchLifecycleError::DisclosureDenied),
            "v2 = {v2}: {full_error:?}"
        );
        assert!(
            matches!(proposal_error, PatchLifecycleError::DisclosureDenied),
            "v2 = {v2}: {proposal_error:?}"
        );
        assert_same_retained_state(
            &full,
            &full_proposal,
            &proposal_only,
            &proposal_only_proposal,
        );
    }
}

#[test]
fn resident_v1_evidence_and_formula_causes_match_full_diff() {
    let base = formula_document();
    for body in [set_f_text("new"), set_g_number(2.0)] {
        let candidate = chain_candidate(&base, &body, "V1 evidence");
        let expected = diff_with_calculations(
            &base,
            &candidate,
            &calculate(&base).unwrap(),
            &calculate(&candidate).unwrap(),
        )
        .unwrap();
        let session = ResidentWorkspaceSession::new("scope".into(), base.clone());
        let prepared = session.prepare_evaluation(candidate);
        let mut lifecycle = formula_lifecycle();
        propose_with(
            &mut lifecycle,
            &base,
            session.revision().as_str(),
            body,
            &prepared,
        )
        .unwrap();
        let evaluated = &lifecycle.evaluation_memo.as_ref().unwrap().evaluated;
        let super::PatchEvidence::V1(changes) = &evaluated.evidence else {
            panic!("expected V1 evidence");
        };
        assert_eq!(changes.as_slice(), expected.changes());
        assert_eq!(
            evaluated.formula_impacts,
            super::formula_impacts(expected.changes())
        );
        if evaluated.document.entities["e1"].fields["g"] == Value::Number(Number::new(2.0).unwrap())
        {
            assert_eq!(evaluated.formula_impacts.len(), 1);
            assert_eq!(
                evaluated.formula_impacts[0].causes,
                vec![FieldRef::new("e1", "g")]
            );
        } else {
            assert!(evaluated.formula_impacts.is_empty());
        }
    }
}

#[test]
fn unprepared_v1_finalization_preserves_full_evidence_and_report() {
    let base = formula_document();
    let candidate = chain_candidate(&base, &set_g_number(2.0), "fallback");
    let session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let mismatched = session.prepare_evaluation(base.clone());
    let (expected, report) =
        crate::finalize_edit_with_validation(&base, candidate.clone(), None).unwrap();
    for prepared in [None, Some(&mismatched)] {
        let (document, changes, actual_report) =
            crate::finalize_edit_changes_with_validation(&base, candidate.clone(), prepared)
                .unwrap();
        assert_eq!(document, expected.document);
        assert_eq!(changes.as_slice(), expected.diff.changes());
        assert_eq!(actual_report, report);
    }
}

fn assert_v1_finalization_error_matches(base: &Document, candidate: &Document) {
    let expected = crate::finalize_edit_with_validation(base, candidate.clone(), None).unwrap_err();
    let session = ResidentWorkspaceSession::new("scope".into(), base.clone());
    let matched = session.prepare_evaluation(candidate.clone());
    let mismatched = session.prepare_evaluation(base.clone());
    for prepared in [None, Some(&matched), Some(&mismatched)] {
        let actual =
            crate::finalize_edit_changes_with_validation(base, candidate.clone(), prepared)
                .unwrap_err();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }
}

#[test]
fn v1_finalization_preserves_unsupported_pair_errors() {
    for field_order in [false, true] {
        let base = formula_document();
        let mut candidate = chain_candidate(&base, &set_g_number(2.0), "unsupported pair");
        let schema = candidate.schemas.get_mut("s").unwrap();
        let expected = if field_order {
            schema.field_order = Some(vec!["h".into(), "g".into(), "f".into()]);
            crate::DiffError::UnsupportedFieldOrder
        } else {
            schema.fields.get_mut("g").unwrap().number_format = Some(NumberFormat {
                decimals: 2,
                kind: NumberFormatKind::Plain,
                thousands_separator: false,
            });
            crate::DiffError::UnsupportedNumberFormat
        };
        let error = crate::finalize_edit_changes_with_validation(&base, candidate.clone(), None)
            .unwrap_err();
        let WorkspaceError::Diff(actual) = error else {
            panic!("expected unsupported diff pair, got {error:?}");
        };
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        assert_v1_finalization_error_matches(&base, &candidate);
    }
}

#[test]
fn v1_finalization_preserves_validation_preflight_and_base_errors() {
    let base = formula_document();
    let mut invalid = base.clone();
    invalid.entities.get_mut("e1").unwrap().fields.remove("g");
    assert_v1_finalization_error_matches(&base, &invalid);
    assert_v1_finalization_error_matches(&base, &unprojectable_document());
    assert_v1_finalization_error_matches(&failing_formula_document(), &base);
}
