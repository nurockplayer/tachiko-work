//! Stage-0 spelling must survive every lifecycle entry point and batch ordering.
use super::*;
use crate::{CommandNameFailure, DisplayNameFailure, Schema, SemanticSubject};

fn document() -> Document {
    let mut document = Document::empty("doc", "Names");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([("f".into(), definition("f", None))]),
        },
    );
    document
}

fn definition(id: &str, name: Option<&str>) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: name.map(str::to_owned),
        number_format: None,
        field_type: FieldType::Text,
        required: true,
        constraint: FieldConstraint::None,
    }
}

fn rename(name: Option<&str>) -> SemanticCommand {
    SemanticCommand::SetSchemaFieldDisplayName {
        schema: "s".into(),
        field: "f".into(),
        display_name: name.map(str::to_owned),
    }
}

fn lifecycle() -> PatchLifecycle {
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
    for family in [
        OperationFamily::SchemaFieldMutation,
        OperationFamily::AppendEntity,
        OperationFamily::RemoveEntity,
    ] {
        requirements.push(GrantRequirement::query(family, scope.clone()));
        for action in [AuthorizationAction::Propose, AuthorizationAction::Execute] {
            for class in [MutationClass::Schema, MutationClass::Structure] {
                requirements.push(
                    GrantRequirement::mutation(action, family, class, scope.clone()).unwrap(),
                );
            }
        }
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
    body: SemanticPatchBody,
) -> Result<SemanticPatch, PatchLifecycleError> {
    lifecycle.propose(
        &"scope".into(),
        &document(),
        &"r1".into(),
        ProposalRequest::new("proposal".into(), "r1".into(), body, "human".into()),
        TrustedInstant::new(1),
    )
}

struct Host {
    document: Document,
    publications: usize,
}
impl SemanticPublicationAuthority for Host {
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        ("scope".into(), self.document.clone(), "r1".into())
    }
    fn publish_if_current<A>(
        &mut self,
        _: &DocumentScopeId,
        _: &SemanticRevision,
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<A>,
    ) -> Result<(DocumentScopeId, Document, SemanticRevision, A), SemanticPublicationError> {
        let authorization = authorize(TrustedInstant::new(3))
            .ok_or(SemanticPublicationError::AuthorizationDenied)?;
        self.publications += 1;
        self.document = candidate;
        Ok((
            "scope".into(),
            self.document.clone(),
            "r2".into(),
            authorization,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Propose,
    Preview,
    Execute,
}

fn attempt(stage: Stage, body: SemanticPatchBody) -> Result<(), PatchLifecycleError> {
    let mut lifecycle = lifecycle();
    if matches!(stage, Stage::Propose) {
        let result = propose(&mut lifecycle, body).map(|_| ());
        if matches!(result, Err(PatchLifecycleError::CommandRejected { .. })) {
            assert!(
                lifecycle.proposals.is_empty(),
                "Stage 0 must not issue a proposal"
            );
        }
        return result;
    }
    // Retain an authorized, executable proposal, then inject the hostile body
    // inside this private unit fixture. Public proposals are immutable; this
    // exercises preview/execution re-admission independently of proposal refusal.
    propose(
        &mut lifecycle,
        SemanticPatchBody::command(rename(Some("Item"))),
    )
    .unwrap();
    lifecycle
        .proposals
        .get_mut(&ProposalId::from("proposal"))
        .unwrap()
        .patch
        .exact_change
        .body = body;
    if matches!(stage, Stage::Preview) {
        return lifecycle
            .preview_v2(
                &"scope".into(),
                &document(),
                &"r1".into(),
                &"proposal".into(),
                &"human".into(),
                TrustedInstant::new(2),
            )
            .map(|_| ());
    }
    let mut host = Host {
        document: document(),
        publications: 0,
    };
    let result = lifecycle
        .execute_v2(
            &"proposal".into(),
            None,
            &"human".into(),
            &mut host,
            TrustedInstant::new(2),
        )
        .map(|_| ());
    if result.is_err() {
        assert_eq!(host.publications, 0);
        assert_eq!(host.document, document());
    }
    result
}

fn failure(
    command_index: usize,
    subject: SemanticSubject,
    reason: DisplayNameFailure,
) -> CommandNameFailure {
    CommandNameFailure {
        command_index,
        subject,
        reason: reason.reason(),
    }
}

fn field_subject(schema: &str, field: &str) -> SemanticSubject {
    SemanticSubject::SchemaField {
        schema: schema.into(),
        field: field.into(),
    }
}

fn assert_refused(stage: Stage, body: SemanticPatchBody, expected: &[CommandNameFailure]) {
    let result = attempt(stage, body);
    let Err(PatchLifecycleError::CommandRejected { source }) = result else {
        panic!("{stage:?} must refuse spelling before candidate validation: {result:?}");
    };
    let WorkspaceError::NameAdmission { failures } = *source else {
        panic!("expected spelling admission, got {source:?}");
    };
    assert_eq!(failures, expected, "{stage:?}");
}

macro_rules! spelling_case {
    ($name:ident, $value:expr, [$($reason:ident),+]) => {
        mod $name {
            use super::*;
            fn check(stage: Stage) {
                let name = $value;
                let expected = vec![$(failure(0, field_subject("s", "f"), DisplayNameFailure::$reason)),+];
                assert_refused(
                    stage,
                    SemanticPatchBody::command(rename(Some(&name))),
                    &expected,
                );
            }
            #[test]
            fn proposal() {
                check(Stage::Propose);
            }
            #[test]
            fn preview() {
                check(Stage::Preview);
            }
            #[test]
            fn execution() {
                check(Stage::Execute);
            }
        }
    };
}
spelling_case!(empty, String::new(), [Empty]);
spelling_case!(
    empty_after_trim,
    " \u{3000} ".to_owned(),
    [Empty, Untrimmed]
);
spelling_case!(control, "A\u{0085}B".to_owned(), [ForbiddenScalar]);
spelling_case!(line_separator, "A\u{2028}B".to_owned(), [ForbiddenScalar]);
spelling_case!(
    paragraph_separator,
    "A\u{2029}B".to_owned(),
    [ForbiddenScalar]
);
spelling_case!(untrimmed, " Item ".to_owned(), [Untrimmed]);
spelling_case!(oversized, "a".repeat(4_097), [TooLong]);
spelling_case!(null_scalar, "\0".to_owned(), [ForbiddenScalar]);

fn overwritten(stage: Stage) {
    assert_refused(
        stage,
        SemanticPatchBody::atomic_batch(vec![rename(Some("\0")), rename(Some("Item"))]).unwrap(),
        &[failure(
            0,
            field_subject("s", "f"),
            DisplayNameFailure::ForbiddenScalar,
        )],
    );
}
#[test]
fn overwritten_proposal() {
    overwritten(Stage::Propose);
}
#[test]
fn overwritten_preview() {
    overwritten(Stage::Preview);
}
#[test]
fn overwritten_execution() {
    overwritten(Stage::Execute);
}

#[test]
fn append_and_inverse_payloads_are_checked_before_any_command() {
    let field = definition("g", Some("\0"));
    let schema = Schema {
        id: "new".into(),
        key: "new".into(),
        display_name: Some(" Item ".into()),
        fields: BTreeMap::from([("g".into(), field.clone())]),
    };
    for (command, expected) in [
        (
            SemanticCommand::AppendCollection {
                schema,
                entities: Vec::new(),
            },
            vec![
                failure(
                    1,
                    SemanticSubject::Schema("new".into()),
                    DisplayNameFailure::Untrimmed,
                ),
                failure(
                    1,
                    field_subject("new", "g"),
                    DisplayNameFailure::ForbiddenScalar,
                ),
            ],
        ),
        (
            SemanticCommand::AppendSchemaField {
                schema: "s".into(),
                field: field.clone(),
                values: BTreeMap::new(),
            },
            vec![failure(
                1,
                field_subject("s", "g"),
                DisplayNameFailure::ForbiddenScalar,
            )],
        ),
        (
            SemanticCommand::RemoveSchemaField {
                schema: "s".into(),
                field,
                values: BTreeMap::new(),
            },
            vec![failure(
                1,
                field_subject("s", "g"),
                DisplayNameFailure::ForbiddenScalar,
            )],
        ),
    ] {
        for stage in [Stage::Propose, Stage::Preview, Stage::Execute] {
            // The first command would fail lookup. Stage 0 must scan the entire
            // request and find the supplied spelling before applying that command.
            let body = SemanticPatchBody::atomic_batch(vec![
                SemanticCommand::RemoveEntity {
                    entity: "missing".into(),
                },
                command.clone(),
            ])
            .unwrap();
            assert_refused(stage, body, &expected);
        }
    }
}

#[test]
fn independent_failures_accumulate_in_request_subject_and_reason_order() {
    let schema = Schema {
        id: "new".into(),
        key: "new".into(),
        display_name: Some(" ".into()),
        // Supply reverse insertion order: engine payload fields are a stable-ID map.
        fields: BTreeMap::from([
            ("z".into(), definition("z", Some("\0"))),
            ("a".into(), definition("a", Some(" Item "))),
        ]),
    };
    let invalid = format!(" {}\0 ", "x".repeat(4097));
    let body = SemanticPatchBody::atomic_batch(vec![
        SemanticCommand::AppendCollection {
            schema,
            entities: Vec::new(),
        },
        rename(Some(&invalid)),
        rename(Some("Item")),
    ])
    .unwrap();
    let expected = [
        failure(
            0,
            SemanticSubject::Schema("new".into()),
            DisplayNameFailure::Empty,
        ),
        failure(
            0,
            SemanticSubject::Schema("new".into()),
            DisplayNameFailure::Untrimmed,
        ),
        failure(0, field_subject("new", "a"), DisplayNameFailure::Untrimmed),
        failure(
            0,
            field_subject("new", "z"),
            DisplayNameFailure::ForbiddenScalar,
        ),
        failure(1, field_subject("s", "f"), DisplayNameFailure::TooLong),
        failure(
            1,
            field_subject("s", "f"),
            DisplayNameFailure::ForbiddenScalar,
        ),
        failure(1, field_subject("s", "f"), DisplayNameFailure::Untrimmed),
    ];
    for stage in [Stage::Propose, Stage::Preview, Stage::Execute] {
        assert_refused(stage, body.clone(), &expected);
    }
}

#[test]
fn admitted_names_clear_and_transient_duplicates_keep_candidate_semantics() {
    for name in ["Item", "項目 🧾", "null", &"🧾".repeat(1024)] {
        for stage in [Stage::Propose, Stage::Preview, Stage::Execute] {
            // Absence is a valid clear, not a supplied string or JSON null.
            let body =
                SemanticPatchBody::atomic_batch(vec![rename(None), rename(Some(name))]).unwrap();
            attempt(stage, body).unwrap();
        }
    }
    let mut document = document();
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .insert("g".into(), definition("g", Some("Item")));
    let mut lifecycle = lifecycle();
    let request = |body| ProposalRequest::new("proposal".into(), "r1".into(), body, "human".into());
    let duplicate = SemanticPatchBody::command(rename(Some("Item")));
    assert!(matches!(
        lifecycle.propose(
            &"scope".into(),
            &document,
            &"r1".into(),
            request(duplicate),
            TrustedInstant::new(1)
        ),
        Err(PatchLifecycleError::ValidationFailed { .. })
    ));
    let mut lifecycle = self::lifecycle();
    let repaired =
        SemanticPatchBody::atomic_batch(vec![rename(Some("Item")), rename(Some("Other"))]).unwrap();
    lifecycle
        .propose(
            &"scope".into(),
            &document,
            &"r1".into(),
            request(repaired),
            TrustedInstant::new(1),
        )
        .unwrap();
}
