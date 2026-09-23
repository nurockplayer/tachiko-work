//! Steward-owned acceptance seed for #454. The v2 lifecycle API below is the
//! pre-Ready contract; absence of these seams on the baseline is not a RED test.

use std::collections::BTreeMap;

use tachiko_diff_engine::CanonicalDirectFact;
use tachiko_workspace_engine::{
    Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType, Number,
    Schema, Value,
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId,
        Grant, GrantId, GrantRequirement, MutationClass, OperationFamily, PatchLifecycle,
        PatchLifecycleError, PolicyMeaningId, PrincipalId, PrincipalKind, ProposalId,
        ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticPublicationAuthority, SemanticPublicationError,
        SemanticRevision, SemanticScope, TrustedInstant,
    },
};

fn rev(n: &str) -> SemanticRevision {
    SemanticRevision::from(n)
}

fn range(min: f64, max: f64) -> FieldConstraint {
    FieldConstraint::NumberInclusiveRange {
        min: Number::new(min).unwrap(),
        max: Number::new(max).unwrap(),
    }
}

fn document(value: f64) -> Document {
    let mut doc = Document::empty("doc", "Constraint publication");
    doc.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields: BTreeMap::from([(
                "field".into(),
                FieldDefinition {
                    id: "field".into(),
                    key: "value".into(),
                    field_type: FieldType::Number,
                    required: true,
                    constraint: FieldConstraint::None,
                },
            )]),
        },
    );
    doc.entities.insert(
        "entity".into(),
        Entity {
            id: "entity".into(),
            key: "item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([("field".into(), Value::Number(Number::new(value).unwrap()))]),
        },
    );
    doc
}

fn scope() -> DocumentScopeId {
    DocumentScopeId::from("occurrence")
}

fn subject(semantic_scope: SemanticScope) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(scope(), "doc".into(), semantic_scope)
}

fn setup(v2: bool, query: bool, schema_write: bool) -> PatchLifecycle {
    let args = (
        AuthorizationDomainId::from("domain"),
        scope(),
        "doc".into(),
        SemanticApiContract::from("semantic-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("meaning-v1"),
    );
    let mut lifecycle = if v2 {
        PatchLifecycle::new_v2(args.0, args.1, args.2, args.3, args.4, args.5)
    } else {
        PatchLifecycle::new(args.0, args.1, args.2, args.3, args.4, args.5)
    };
    lifecycle
        .register_principal("editor".into(), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal("authority".into(), PrincipalKind::Human)
        .unwrap();
    let mut requirements = Vec::new();
    if query {
        requirements.push(GrantRequirement::query(
            OperationFamily::SchemaFieldMutation,
            subject(SemanticScope::Document),
        ));
    }
    if schema_write {
        for action in [AuthorizationAction::Propose, AuthorizationAction::Execute] {
            requirements.push(
                GrantRequirement::mutation(
                    action,
                    OperationFamily::SchemaFieldMutation,
                    MutationClass::Schema,
                    subject(SemanticScope::Schema("schema".into())),
                )
                .unwrap(),
            );
        }
    }
    if !requirements.is_empty() {
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("editor-grant"),
                PrincipalId::from("authority"),
                PrincipalId::from("editor"),
                requirements,
                None,
            ))
            .unwrap();
    }
    lifecycle
}

fn constraint(constraint: FieldConstraint) -> SemanticCommand {
    SemanticCommand::SetFieldConstraint {
        schema: "schema".into(),
        field: "field".into(),
        constraint,
    }
}

fn grant_value_authority(lifecycle: &mut PatchLifecycle) {
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from("value-grant"),
            "authority".into(),
            "editor".into(),
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
}

fn propose(
    lifecycle: &mut PatchLifecycle,
    doc: &Document,
    revision: &str,
    id: &str,
    body: SemanticPatchBody,
) -> Result<ProposalId, PatchLifecycleError> {
    let id = ProposalId::from(id);
    lifecycle.propose(
        &scope(),
        doc,
        &rev(revision),
        ProposalRequest::new(id.clone(), rev(revision), body, "editor".into()),
        TrustedInstant::new(10),
    )?;
    Ok(id)
}

struct Host {
    document: Document,
    revision: SemanticRevision,
    publications: usize,
}

impl Host {
    fn new(document: Document) -> Self {
        Self {
            document,
            revision: rev("r1"),
            publications: 0,
        }
    }
}

impl SemanticPublicationAuthority for Host {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        (scope(), self.document.clone(), self.revision.clone())
    }

    fn publish_if_current<A>(
        &mut self,
        expected_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, Document, SemanticRevision, A), SemanticPublicationError> {
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
        self.revision = rev(&format!("r{}", self.publications + 1));
        Ok((
            scope(),
            self.document.clone(),
            self.revision.clone(),
            authorization,
        ))
    }
}

fn fact(delta: &tachiko_workspace_engine::CanonicalSemanticDelta) {
    assert_eq!(delta.facts().len(), 1);
    assert!(matches!(
        &delta.facts()[0],
        CanonicalDirectFact::SchemaFieldConstraintChanged { schema, field, before, after }
            if schema.as_str() == "schema" && field.as_str() == "field"
                && before == &FieldConstraint::None && after == &range(0.0, 10.0)
    ));
}

#[test]
#[allow(clippy::too_many_lines)] // One current-base set, inverse, and redo journey.
fn valid_set_clear_and_inverse_publish_one_complete_fact_each() {
    let original = document(7.0);
    let mut host = Host::new(original.clone());
    let mut lifecycle = setup(true, true, true);
    let id = propose(
        &mut lifecycle,
        &host.document,
        "r1",
        "set",
        SemanticPatchBody::command(constraint(range(0.0, 10.0))),
    )
    .unwrap();
    let preview = lifecycle
        .preview_v2(
            &scope(),
            &host.document,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    fact(&preview.delta);
    assert_eq!(
        preview.risk.mutation_classes,
        [MutationClass::Schema].into()
    );
    let receipt = lifecycle
        .execute_v2(
            &id,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert!(receipt.verified);
    assert_eq!(receipt.delta().unwrap(), &preview.delta);
    assert_eq!(host.revision, rev("r2"));
    assert_eq!(host.document.entities, original.entities);
    assert_eq!(
        host.document.schemas["schema"].fields["field"].id,
        original.schemas["schema"].fields["field"].id
    );
    assert_eq!(
        host.document.schemas["schema"].fields["field"].constraint,
        range(0.0, 10.0)
    );

    let clear = propose(
        &mut lifecycle,
        &host.document,
        "r2",
        "clear",
        SemanticPatchBody::command(constraint(FieldConstraint::None)),
    )
    .unwrap();
    let clear_preview = lifecycle
        .preview_v2(
            &scope(),
            &host.document,
            &rev("r2"),
            &clear,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    let clear_fact: &CanonicalDirectFact = &clear_preview.delta.facts()[0];
    assert!(
        matches!(clear_fact, CanonicalDirectFact::SchemaFieldConstraintChanged { before, after, .. } if before == &range(0.0, 10.0) && after == &FieldConstraint::None)
    );
    lifecycle
        .execute_v2(
            &clear,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(host.document, original);
    assert_eq!(host.revision, rev("r3"));

    let redo = propose(
        &mut lifecycle,
        &host.document,
        "r3",
        "redo",
        SemanticPatchBody::command(constraint(range(0.0, 10.0))),
    )
    .unwrap();
    let redo_preview = lifecycle
        .preview_v2(
            &scope(),
            &host.document,
            &rev("r3"),
            &redo,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    fact(&redo_preview.delta);
    lifecycle
        .execute_v2(
            &redo,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(host.revision, rev("r4"));
    assert_eq!(
        host.document.schemas["schema"].fields["field"].constraint,
        range(0.0, 10.0)
    );
}

#[test]
fn text_literal_payload_survives_review_and_publication_exactly() {
    let mut original = document(7.0);
    original
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("field")
        .unwrap()
        .field_type = FieldType::Text;
    original
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("field".into(), Value::Text("alpha".into()));
    let literal_set = FieldConstraint::TextLiteralSet {
        values: vec!["alpha".into(), "beta".into()],
    };
    let mut host = Host::new(original.clone());
    let mut lifecycle = setup(true, true, true);
    let id = propose(
        &mut lifecycle,
        &original,
        "r1",
        "text",
        SemanticPatchBody::command(constraint(literal_set.clone())),
    )
    .unwrap();
    let preview = lifecycle
        .preview_v2(
            &scope(),
            &original,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    assert!(
        matches!(&preview.delta.facts()[0], CanonicalDirectFact::SchemaFieldConstraintChanged { before, after, .. } if before == &FieldConstraint::None && after == &literal_set)
    );
    let receipt = lifecycle
        .execute_v2(
            &id,
            None,
            &"editor".into(),
            &mut host,
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(receipt.delta().unwrap(), &preview.delta);
    assert_eq!(
        host.document.schemas["schema"].fields["field"].constraint,
        literal_set
    );
    assert_eq!(host.document.entities, original.entities);
}

#[test]
fn narrowing_invalid_existing_value_is_atomic_and_identifies_the_value() {
    let original = document(20.0);
    let mut lifecycle = setup(true, true, true);
    let error = propose(
        &mut lifecycle,
        &original,
        "r1",
        "narrow",
        SemanticPatchBody::command(constraint(range(0.0, 10.0))),
    )
    .unwrap_err();
    let PatchLifecycleError::ValidationFailed { report } = error else {
        panic!("expected complete validation report")
    };
    assert!(report.diagnostics().iter().any(|d| d.code.as_str()
        == "core.field_value_constraint_mismatch"
        && d.path == "entities.entity.fields.field"));
    let host = Host::new(original.clone());
    assert_eq!(host.document, original);
    assert_eq!(host.revision, rev("r1"));
    assert_eq!(host.publications, 0);
}

#[test]
fn v1_stays_closed_and_v2_evaluates_ordinary_commands_on_constrained_base() {
    let mut constrained = document(7.0);
    constrained
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("field")
        .unwrap()
        .constraint = range(0.0, 10.0);
    let body = SemanticPatchBody::command(SemanticCommand::set_field_value(
        FieldRef::new("entity", "field"),
        Value::Number(Number::new(8.0).unwrap()),
    ));
    let mut v1 = setup(false, true, true);
    grant_value_authority(&mut v1);
    assert!(matches!(
        propose(&mut v1, &constrained, "r1", "v1", body.clone()),
        Err(PatchLifecycleError::CommandRejected { source })
            if matches!(*source, tachiko_workspace_engine::WorkspaceError::Diff(tachiko_diff_engine::DiffError::UnsupportedFieldConstraintChange))
    ));
    let mut v2 = setup(true, true, true);
    // Ordinary value mutation still needs its pre-existing Value grant; a Schema grant cannot substitute.
    assert!(matches!(
        propose(&mut v2, &constrained, "r1", "no-value-grant", body.clone()),
        Err(PatchLifecycleError::InsufficientCapability {
            action: AuthorizationAction::Propose
        })
    ));
    let mut v2 = setup(true, true, true);
    grant_value_authority(&mut v2);
    let id = propose(&mut v2, &constrained, "r1", "v2", body).unwrap();
    let preview = v2
        .preview_v2(
            &scope(),
            &constrained,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    let facts: &[CanonicalDirectFact] = preview.delta.facts();
    assert!(
        matches!(facts, [CanonicalDirectFact::EntityFieldValueChanged { before, after, .. }] if before == &Value::Number(Number::new(7.0).unwrap()) && after == &Value::Number(Number::new(8.0).unwrap()))
    );
    assert!(matches!(
        v2.preview(
            &scope(),
            &constrained,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10)
        ),
        Err(PatchLifecycleError::EvidenceProfileMismatch)
    ));
}

#[test]
fn duplicate_stale_and_missing_query_or_schema_authority_refuse_without_publication() {
    let original = document(7.0);
    let command = constraint(range(0.0, 10.0));
    let mut full = setup(true, true, true);
    assert!(
        propose(
            &mut full,
            &original,
            "r1",
            "duplicate",
            SemanticPatchBody::atomic_batch(vec![command.clone(), command.clone()]).unwrap()
        )
        .is_err()
    );
    assert!(matches!(
        full.propose(
            &scope(),
            &original,
            &rev("r2"),
            ProposalRequest::new(
                "stale".into(),
                rev("r1"),
                SemanticPatchBody::command(command.clone()),
                "editor".into()
            ),
            TrustedInstant::new(10)
        ),
        Err(PatchLifecycleError::Stale)
    ));
    for (query, write, expected) in [(false, true, "query"), (true, false, "write")] {
        let mut lifecycle = setup(true, query, write);
        let result = propose(
            &mut lifecycle,
            &original,
            "r1",
            expected,
            SemanticPatchBody::command(command.clone()),
        );
        if query {
            assert!(matches!(
                result,
                Err(PatchLifecycleError::InsufficientCapability {
                    action: AuthorizationAction::Propose
                })
            ));
        } else {
            let id = result.expect("covered Propose without Query issues an inert proposal");
            assert!(matches!(
                lifecycle.preview_v2(
                    &scope(),
                    &original,
                    &rev("r1"),
                    &id,
                    &"editor".into(),
                    TrustedInstant::new(10),
                ),
                Err(PatchLifecycleError::DisclosureDenied)
            ));
        }
    }
    let host = Host::new(original.clone());
    assert_eq!(host.document, original);
    assert_eq!(host.revision, rev("r1"));
    assert_eq!(host.publications, 0);
}

#[test]
fn constraint_review_shows_every_affected_value_before_review_credit() {
    let mut original = document(7.0);
    original.entities.insert(
        "second".into(),
        Entity {
            id: "second".into(),
            key: "second_item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([("field".into(), Value::Number(Number::new(9.0).unwrap()))]),
        },
    );
    let formula = Value::Formula(Expression::Reference(FieldRef::new("entity", "field")));
    original.entities.insert(
        "formula".into(),
        Entity {
            id: "formula".into(),
            key: "formula_item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([("field".into(), formula.clone())]),
        },
    );

    let mut lifecycle = setup(true, true, true);
    let id = propose(
        &mut lifecycle,
        &original,
        "r1",
        "review-values",
        SemanticPatchBody::command(constraint(range(0.0, 10.0))),
    )
    .unwrap();
    let preview = lifecycle
        .preview_v2(
            &scope(),
            &original,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    fact(&preview.delta);
    assert_eq!(preview.constraint_reviews.len(), 1);
    let review = &preview.constraint_reviews[0];
    assert_eq!(review.schema.as_str(), "schema");
    assert_eq!(review.field.as_str(), "field");
    assert_eq!(review.before, review.after);
    let observed: BTreeMap<_, _> = review
        .before
        .iter()
        .map(|entry| {
            (
                entry.field.clone(),
                (entry.stored.clone(), entry.calculated_number),
            )
        })
        .collect();
    assert_eq!(
        observed,
        BTreeMap::from([
            (
                FieldRef::new("entity", "field"),
                (
                    Value::Number(Number::new(7.0).unwrap()),
                    Some(Number::new(7.0).unwrap()),
                ),
            ),
            (
                FieldRef::new("formula", "field"),
                (formula, Some(Number::new(7.0).unwrap())),
            ),
            (
                FieldRef::new("second", "field"),
                (
                    Value::Number(Number::new(9.0).unwrap()),
                    Some(Number::new(9.0).unwrap()),
                ),
            ),
        ])
    );
}

#[test]
fn constraint_review_requires_query_for_stored_formula_dependencies() {
    let mut original = document(7.0);
    original.schemas.get_mut("schema").unwrap().fields.insert(
        "secret".into(),
        FieldDefinition {
            id: "secret".into(),
            key: "secret".into(),
            field_type: FieldType::Number,
            required: false,
            constraint: FieldConstraint::None,
        },
    );
    original
        .entities
        .get_mut("entity")
        .unwrap()
        .fields
        .insert("secret".into(), Value::Number(Number::new(5.0).unwrap()));
    original.entities.insert(
        "formula".into(),
        Entity {
            id: "formula".into(),
            key: "formula_item".into(),
            schema: "schema".into(),
            fields: BTreeMap::from([(
                "field".into(),
                Value::Formula(Expression::Reference(FieldRef::new("entity", "secret"))),
            )]),
        },
    );

    let mut lifecycle = setup(true, false, true);
    lifecycle
        .provision_grant(Grant::new(
            "target-query".into(),
            "authority".into(),
            "editor".into(),
            vec![GrantRequirement::query(
                OperationFamily::SchemaFieldMutation,
                subject(SemanticScope::SchemaField {
                    schema: "schema".into(),
                    field: "field".into(),
                }),
            )],
            None,
        ))
        .unwrap();
    let id = propose(
        &mut lifecycle,
        &original,
        "r1",
        "narrow-review",
        SemanticPatchBody::command(constraint(range(0.0, 10.0))),
    )
    .unwrap();
    assert!(matches!(
        lifecycle.preview_v2(
            &scope(),
            &original,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10),
        ),
        Err(PatchLifecycleError::DisclosureDenied)
    ));
    lifecycle
        .provision_grant(Grant::new(
            "dependency-query".into(),
            "authority".into(),
            "editor".into(),
            vec![GrantRequirement::query(
                OperationFamily::SchemaFieldMutation,
                subject(SemanticScope::EntityField {
                    entity: "entity".into(),
                    schema: "schema".into(),
                    field: "secret".into(),
                }),
            )],
            None,
        ))
        .unwrap();
    let preview = lifecycle
        .preview_v2(
            &scope(),
            &original,
            &rev("r1"),
            &id,
            &"editor".into(),
            TrustedInstant::new(10),
        )
        .unwrap();
    assert_eq!(preview.constraint_reviews.len(), 1);
}
