//! One evaluation serves propose, preview and execute while the inputs are unchanged.
use std::collections::BTreeMap;

use super::{
    AuthorizationAction, Document, DocumentScopeId, Entity, FieldConstraint, FieldDefinition,
    FieldRef, FieldType, Grant, GrantRequirement, MutationClass, OperationFamily, PatchLifecycle,
    PatchLifecycleError, PrincipalKind, ProposalId, ScopedSemanticSubject, SemanticCommand,
    SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision,
    SemanticScope, TrustedInstant, V2ReceiptEvidence, Value,
};
use crate::Schema;

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
