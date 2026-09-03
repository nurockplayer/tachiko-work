use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Number, Schema, SchemaId, SchemaKey, SemanticValueKind, Value,
    capability_discovery::{
        FieldCapabilities, FieldCapabilityApplicability, FieldCapabilityInapplicability,
        FieldCapabilityInput, FieldCapabilityKind, FieldCapabilityQueryOutcome,
    },
    formula_operations::{
        FormulaReasoningOutcome, FormulaUpdateRequest, NumberOverride, ScenarioOutcome,
        ScenarioOverrideFailure, ScenarioRequest, ValidatorConfiguration,
    },
    patch_lifecycle::{
        AuthorizationAction, AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId,
        Grant, GrantId, GrantRequirement, MutationClass, OperationFamily, PatchLifecycle,
        PatchLifecycleError, PolicyMeaningId, PrincipalId, PrincipalKind, ProposalId,
        ProposalRequest, ScopedSemanticSubject, SemanticApiContract, SemanticCommand,
        SemanticPatchBody, SemanticRevision, SemanticScope, TrustedInstant,
    },
    resident_session::ResidentWorkspaceSession,
    validate_field_value_suggestion,
};

const NOW: TrustedInstant = TrustedInstant::new(10);

fn field(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: id.into(),
        field_type,
        required: true,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn date(value: &str) -> Value {
    Value::Date(value.parse().unwrap())
}

fn date_field() -> (FieldId, FieldDefinition) {
    (
        FieldId::from("published"),
        field("published", FieldType::Date),
    )
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn document() -> Document {
    let source_schema = SchemaId::from("source");
    let target_schema = SchemaId::from("target");
    Document {
        id: DocumentId::from("game"),
        title: "Game".to_owned(),
        schemas: BTreeMap::from([
            (
                source_schema.clone(),
                Schema {
                    id: source_schema.clone(),
                    key: SchemaKey::from("source"),
                    fields: BTreeMap::from([
                        (FieldId::from("amount"), field("amount", FieldType::Number)),
                        (FieldId::from("label"), field("label", FieldType::Text)),
                        (
                            FieldId::from("enabled"),
                            field("enabled", FieldType::Boolean),
                        ),
                        date_field(),
                        (
                            FieldId::from("target"),
                            field(
                                "target",
                                FieldType::Reference {
                                    schema: target_schema.clone(),
                                },
                            ),
                        ),
                        (
                            FieldId::from("computed"),
                            field("computed", FieldType::Number),
                        ),
                    ]),
                },
            ),
            (
                target_schema.clone(),
                Schema {
                    id: target_schema,
                    key: SchemaKey::from("target"),
                    fields: BTreeMap::from([(
                        FieldId::from("name"),
                        field("name", FieldType::Text),
                    )]),
                },
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("row"),
                Entity {
                    id: EntityId::from("row"),
                    key: "row".into(),
                    schema: source_schema,
                    fields: BTreeMap::from([
                        (FieldId::from("amount"), number(10.0)),
                        (FieldId::from("label"), Value::Text("old".to_owned())),
                        (FieldId::from("enabled"), Value::Boolean(true)),
                        (FieldId::from("published"), date("2024-02-29")),
                        (
                            FieldId::from("target"),
                            Value::Reference(EntityId::from("target-row")),
                        ),
                        (
                            FieldId::from("computed"),
                            Value::Formula(Expression::Add {
                                left: Box::new(reference("row", "amount")),
                                right: Box::new(Expression::Number(Number::new(2.0).unwrap())),
                            }),
                        ),
                    ]),
                },
            ),
            (
                EntityId::from("target-row"),
                Entity {
                    id: EntityId::from("target-row"),
                    key: "target-row".into(),
                    schema: SchemaId::from("target"),
                    fields: BTreeMap::from([(
                        FieldId::from("name"),
                        Value::Text("Target A".to_owned()),
                    )]),
                },
            ),
            (
                EntityId::from("target-row-2"),
                Entity {
                    id: EntityId::from("target-row-2"),
                    key: "target-row-2".into(),
                    schema: SchemaId::from("target"),
                    fields: BTreeMap::from([(
                        FieldId::from("name"),
                        Value::Text("Target B".to_owned()),
                    )]),
                },
            ),
        ]),
    }
}

fn field_ref(name: &str) -> FieldRef {
    FieldRef::new("row", name)
}

fn capability(
    capabilities: &FieldCapabilities,
    family: OperationFamily,
    input: FieldCapabilityInput,
) -> &tachiko_workspace_engine::capability_discovery::FieldCapability {
    capabilities
        .capabilities
        .iter()
        .find(|candidate| candidate.family == family && candidate.input == input)
        .expect("bounded projection should include the requested family/input")
}

fn lifecycle() -> PatchLifecycle {
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from("local-domain"),
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticApiContract::from("tachiko-sem-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("policy-v1-meaning"),
    );
    lifecycle
        .register_principal(PrincipalId::from("authority"), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal(PrincipalId::from("agent"), PrincipalKind::Delegated)
        .unwrap();
    lifecycle
}

fn capabilities_for(document: &Document, field: &FieldRef) -> FieldCapabilities {
    let mut lifecycle = lifecycle();
    grant_discovery(&mut lifecycle);
    let snapshot =
        ResidentWorkspaceSession::new(DocumentScopeId::from("game-occurrence"), document.clone())
            .export_snapshot();
    let result = lifecycle
        .query_field_capabilities(&snapshot, field, &PrincipalId::from("agent"), NOW)
        .expect("authorized discovery should project an existing field");
    let FieldCapabilityQueryOutcome::Field(capabilities) = result.outcome else {
        panic!("test fixture field should resolve");
    };
    capabilities
}

fn document_scope() -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::Document,
    )
}

fn source_schema_scope() -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::Schema(SchemaId::from("source")),
    )
}

fn entity_field_scope(field: &FieldRef) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::EntityField {
            entity: field.entity.clone(),
            schema: SchemaId::from("source"),
            field: field.field.clone(),
        },
    )
}

fn schema_field_scope(field: &FieldRef) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        DocumentScopeId::from("game-occurrence"),
        DocumentId::from("game"),
        SemanticScope::SchemaField {
            schema: SchemaId::from("source"),
            field: field.field.clone(),
        },
    )
}

fn grant_discovery(lifecycle: &mut PatchLifecycle) {
    grant_query(
        lifecycle,
        "field-capability-query",
        OperationFamily::FieldCapabilityDiscovery,
    );
}

fn grant(lifecycle: &mut PatchLifecycle, id: &str, requirements: Vec<GrantRequirement>) {
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from(id),
            PrincipalId::from("authority"),
            PrincipalId::from("agent"),
            requirements,
            None,
        ))
        .unwrap();
}

fn grant_query(lifecycle: &mut PatchLifecycle, id: &str, family: OperationFamily) {
    grant(
        lifecycle,
        id,
        vec![GrantRequirement::query(family, document_scope())],
    );
}

fn grant_value_propose(lifecycle: &mut PatchLifecycle) {
    grant_query(lifecycle, "value-query", OperationFamily::SetFieldValue);
    grant(
        lifecycle,
        "value-propose",
        vec![
            GrantRequirement::mutation(
                AuthorizationAction::Propose,
                OperationFamily::SetFieldValue,
                MutationClass::Value,
                document_scope(),
            )
            .unwrap(),
        ],
    );
}

fn grant_formula_propose(lifecycle: &mut PatchLifecycle) {
    grant_query(lifecycle, "formula-query", OperationFamily::FormulaUpdate);
    grant(
        lifecycle,
        "formula-propose",
        vec![
            GrantRequirement::mutation(
                AuthorizationAction::Propose,
                OperationFamily::FormulaUpdate,
                MutationClass::Formula,
                document_scope(),
            )
            .unwrap(),
        ],
    );
}

fn assert_applicable(capabilities: &FieldCapabilities, input: FieldCapabilityInput) {
    assert_eq!(
        &capability(capabilities, OperationFamily::SetFieldValue, input).applicability,
        &FieldCapabilityApplicability::Applicable
    );
}

fn assert_type_mismatch(
    capabilities: &FieldCapabilities,
    input_kind: SemanticValueKind,
    expected: FieldType,
) {
    assert_eq!(
        &capability(
            capabilities,
            OperationFamily::SetFieldValue,
            FieldCapabilityInput::TypedValue { kind: input_kind },
        )
        .applicability,
        &FieldCapabilityApplicability::Inapplicable {
            reason: FieldCapabilityInapplicability::TypeMismatch {
                expected,
                actual: input_kind,
            },
        }
    );
}

fn value_for_kind(kind: SemanticValueKind) -> Value {
    match kind {
        SemanticValueKind::Number => number(11.0),
        SemanticValueKind::Text => Value::Text("wrong kind".to_owned()),
        SemanticValueKind::Boolean => Value::Boolean(false),
        SemanticValueKind::Reference => Value::Reference(EntityId::from("target-row-2")),
        SemanticValueKind::Formula => unreachable!("Formula is not a scalar input"),
        SemanticValueKind::Date => Value::Date("2024-01-01".parse().unwrap()),
    }
}

#[test]
fn discovery_projects_number_text_boolean_date_and_reference_rules() {
    let document = document();
    for (name, expected_type, expected_kind, accepted_value) in [
        (
            "amount",
            FieldType::Number,
            SemanticValueKind::Number,
            number(11.0),
        ),
        (
            "label",
            FieldType::Text,
            SemanticValueKind::Text,
            Value::Text("new".to_owned()),
        ),
        (
            "enabled",
            FieldType::Boolean,
            SemanticValueKind::Boolean,
            Value::Boolean(false),
        ),
        (
            "published",
            FieldType::Date,
            SemanticValueKind::Date,
            Value::Date("2025-01-01".parse().unwrap()),
        ),
        (
            "target",
            FieldType::Reference {
                schema: SchemaId::from("target"),
            },
            SemanticValueKind::Reference,
            Value::Reference(EntityId::from("target-row-2")),
        ),
    ] {
        let field = field_ref(name);
        let capabilities = capabilities_for(&document, &field);
        assert_eq!(capabilities.field, field);
        assert_eq!(capabilities.declared_type, expected_type.clone());
        assert_eq!(capabilities.current_value_kind, expected_kind);
        assert_applicable(
            &capabilities,
            FieldCapabilityInput::TypedValue {
                kind: expected_kind,
            },
        );
        for input_kind in [
            SemanticValueKind::Number,
            SemanticValueKind::Text,
            SemanticValueKind::Boolean,
            SemanticValueKind::Date,
            SemanticValueKind::Reference,
        ] {
            if input_kind != expected_kind {
                assert_type_mismatch(&capabilities, input_kind, expected_type.clone());
            }
        }
        assert!(validate_field_value_suggestion(&document, field, accepted_value).is_ok());
    }
}

#[test]
fn discovery_and_field_propose_share_typed_input_rule() {
    let document = document();
    for (index, (name, expected_type, expected_kind)) in [
        ("amount", FieldType::Number, SemanticValueKind::Number),
        ("label", FieldType::Text, SemanticValueKind::Text),
        ("enabled", FieldType::Boolean, SemanticValueKind::Boolean),
        ("published", FieldType::Date, SemanticValueKind::Date),
        (
            "target",
            FieldType::Reference {
                schema: SchemaId::from("target"),
            },
            SemanticValueKind::Reference,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let field = field_ref(name);
        let capabilities = capabilities_for(&document, &field);
        for (wrong_index, input_kind) in [
            SemanticValueKind::Number,
            SemanticValueKind::Text,
            SemanticValueKind::Boolean,
            SemanticValueKind::Date,
            SemanticValueKind::Reference,
        ]
        .into_iter()
        .enumerate()
        .filter(|(_, input_kind)| *input_kind != expected_kind)
        {
            assert_type_mismatch(&capabilities, input_kind, expected_type.clone());

            let mut lifecycle = lifecycle();
            grant_value_propose(&mut lifecycle);
            let error = lifecycle
                .propose(
                    &DocumentScopeId::from("game-occurrence"),
                    &document,
                    &SemanticRevision::from("r1"),
                    ProposalRequest::new(
                        ProposalId::from(format!("mismatch-{index}-{wrong_index}")),
                        SemanticRevision::from("r1"),
                        SemanticPatchBody::command(SemanticCommand::set_field_value(
                            field.clone(),
                            value_for_kind(input_kind),
                        )),
                        PrincipalId::from("agent"),
                    ),
                    NOW,
                )
                .expect_err("the authoritative proposal path must reject the wrong type");
            assert!(matches!(
                error,
                PatchLifecycleError::CommandRejected { source }
                    if matches!(source.as_ref(), tachiko_workspace_engine::WorkspaceError::TypeMismatch { .. })
            ));
        }
    }
}

#[test]
fn discovery_and_date_propose_share_typed_input_rule() {
    let document = document();
    let date_field = field_ref("published");
    let capabilities = capabilities_for(&document, &date_field);
    assert_eq!(capabilities.current_value_kind, SemanticValueKind::Date);
    assert_applicable(
        &capabilities,
        FieldCapabilityInput::TypedValue {
            kind: SemanticValueKind::Date,
        },
    );

    let mut lifecycle = lifecycle();
    grant_value_propose(&mut lifecycle);
    let patch = lifecycle
        .propose(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            &SemanticRevision::from("r1"),
            ProposalRequest::new(
                ProposalId::from("date-value-parity"),
                SemanticRevision::from("r1"),
                SemanticPatchBody::command(SemanticCommand::set_field_value(
                    date_field,
                    Value::Date("2025-01-01".parse().unwrap()),
                )),
                PrincipalId::from("agent"),
            ),
            NOW,
        )
        .expect("an applicable Date value must reach the authoritative Propose path");
    assert_eq!(
        patch.exact_change().base_revision(),
        &SemanticRevision::from("r1")
    );
}

#[test]
fn discovery_and_mutation_share_formula_edit_and_formula_target_rules() {
    let document = document();
    let formula_field = field_ref("computed");
    let capabilities = capabilities_for(&document, &formula_field);
    assert_eq!(capabilities.current_value_kind, SemanticValueKind::Formula);
    for input_kind in [
        SemanticValueKind::Number,
        SemanticValueKind::Text,
        SemanticValueKind::Boolean,
        SemanticValueKind::Date,
        SemanticValueKind::Reference,
    ] {
        assert_eq!(
            &capability(
                &capabilities,
                OperationFamily::SetFieldValue,
                FieldCapabilityInput::TypedValue { kind: input_kind },
            )
            .applicability,
            &FieldCapabilityApplicability::Inapplicable {
                reason: FieldCapabilityInapplicability::FormulaEdit,
            }
        );
        let value = match input_kind {
            SemanticValueKind::Number => number(11.0),
            SemanticValueKind::Text => Value::Text("blocked".to_owned()),
            SemanticValueKind::Boolean => Value::Boolean(false),
            SemanticValueKind::Reference => Value::Reference(EntityId::from("target-row-2")),
            SemanticValueKind::Formula => unreachable!("Formula is not a scalar input"),
            SemanticValueKind::Date => Value::Date("2024-01-01".parse().unwrap()),
        };
        assert!(matches!(
            validate_field_value_suggestion(&document, formula_field.clone(), value),
            Err(tachiko_workspace_engine::WorkspaceError::FormulaEdit { .. })
        ));
    }

    assert_eq!(
        &capability(
            &capabilities,
            OperationFamily::FormulaUpdate,
            FieldCapabilityInput::Formula,
        )
        .applicability,
        &FieldCapabilityApplicability::Applicable
    );
    assert_eq!(
        &capability(
            &capabilities,
            OperationFamily::FormulaReasoning,
            FieldCapabilityInput::None,
        )
        .applicability,
        &FieldCapabilityApplicability::Applicable
    );
}

#[test]
fn discovery_and_formula_propose_share_formula_target_rule() {
    let document = document();
    let formula_field = field_ref("computed");
    let capabilities = capabilities_for(&document, &formula_field);
    assert_eq!(
        &capability(
            &capabilities,
            OperationFamily::FormulaUpdate,
            FieldCapabilityInput::Formula,
        )
        .applicability,
        &FieldCapabilityApplicability::Applicable
    );

    let mut lifecycle = lifecycle();
    grant_formula_propose(&mut lifecycle);
    let patch = lifecycle
        .propose_formula_update(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            &SemanticRevision::from("r1"),
            FormulaUpdateRequest::new(
                ProposalId::from("formula-update-parity"),
                SemanticRevision::from("r1"),
                formula_field,
                "[row.amount] + 3",
                PrincipalId::from("agent"),
            ),
            NOW,
        )
        .expect("an applicable FormulaUpdate must reach the authoritative Propose path");
    assert_eq!(
        patch.exact_change().base_revision(),
        &SemanticRevision::from("r1")
    );

    let text_field = field_ref("label");
    let text_capabilities = capabilities_for(&document, &text_field);
    assert_eq!(
        &capability(
            &text_capabilities,
            OperationFamily::FormulaUpdate,
            FieldCapabilityInput::Formula,
        )
        .applicability,
        &FieldCapabilityApplicability::Inapplicable {
            reason: FieldCapabilityInapplicability::NonNumericFormulaField,
        }
    );
    let error = lifecycle
        .propose_formula_update(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            &SemanticRevision::from("r1"),
            FormulaUpdateRequest::new(
                ProposalId::from("formula-update-nonnumeric"),
                SemanticRevision::from("r1"),
                text_field,
                "42",
                PrincipalId::from("agent"),
            ),
            NOW,
        )
        .expect_err("a nonnumeric FormulaUpdate target must be rejected");
    assert!(matches!(
        error,
        PatchLifecycleError::CommandRejected { source }
            if matches!(source.as_ref(), tachiko_workspace_engine::WorkspaceError::NonNumericFormulaField { .. })
    ));
}

#[test]
fn discovery_reports_query_applicability_from_current_value_kind() {
    let document = document();
    let number_capabilities = capabilities_for(&document, &field_ref("amount"));
    assert_eq!(
        &capability(
            &number_capabilities,
            OperationFamily::NumberOverrideScenario,
            FieldCapabilityInput::Number,
        )
        .applicability,
        &FieldCapabilityApplicability::Applicable
    );
    assert_eq!(
        &capability(
            &number_capabilities,
            OperationFamily::FormulaReasoning,
            FieldCapabilityInput::None,
        )
        .applicability,
        &FieldCapabilityApplicability::Inapplicable {
            reason: FieldCapabilityInapplicability::NotFormula,
        }
    );

    let formula_capabilities = capabilities_for(&document, &field_ref("computed"));
    assert_eq!(
        &capability(
            &formula_capabilities,
            OperationFamily::NumberOverrideScenario,
            FieldCapabilityInput::Number,
        )
        .applicability,
        &FieldCapabilityApplicability::Inapplicable {
            reason: FieldCapabilityInapplicability::UnsupportedValueKind {
                actual: SemanticValueKind::Formula,
            },
        }
    );
}

#[test]
fn discovery_query_applicability_matches_existing_formula_query_paths() {
    let document = document();
    let amount = field_ref("amount");
    let computed = field_ref("computed");
    let mut lifecycle = lifecycle();
    grant_query(
        &mut lifecycle,
        "formula-reasoning-query",
        OperationFamily::FormulaReasoning,
    );

    let formula_capabilities = capabilities_for(&document, &computed);
    let formula_result = lifecycle
        .query_formula_reasoning(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            (
                &SemanticRevision::from("r1"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &computed,
            &PrincipalId::from("agent"),
            NOW,
        )
        .unwrap();
    assert!(matches!(
        formula_result.outcome,
        FormulaReasoningOutcome::Formula(_)
    ));
    assert!(matches!(
        &capability(
            &formula_capabilities,
            OperationFamily::FormulaReasoning,
            FieldCapabilityInput::None,
        )
        .applicability,
        FieldCapabilityApplicability::Applicable
    ));

    let non_formula_result = lifecycle
        .query_formula_reasoning(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            (
                &SemanticRevision::from("r1"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &amount,
            &PrincipalId::from("agent"),
            NOW,
        )
        .unwrap();
    assert!(matches!(
        non_formula_result.outcome,
        FormulaReasoningOutcome::UnsupportedKind {
            actual: SemanticValueKind::Number,
            ..
        }
    ));
}

#[test]
fn discovery_number_scenario_matches_existing_scenario_query_path() {
    let document = document();
    let amount = field_ref("amount");
    let computed = field_ref("computed");
    let mut lifecycle = lifecycle();
    grant_query(
        &mut lifecycle,
        "number-scenario-query",
        OperationFamily::NumberOverrideScenario,
    );

    let number_capabilities = capabilities_for(&document, &amount);
    let formula_capabilities = capabilities_for(&document, &computed);
    let scenario_result = lifecycle
        .query_number_override_scenario(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            (
                &SemanticRevision::from("r1"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &ScenarioRequest::new(vec![NumberOverride::new(amount.clone(), 11.0)], vec![]),
            &PrincipalId::from("agent"),
            NOW,
        )
        .unwrap();
    assert!(matches!(
        scenario_result.outcome,
        ScenarioOutcome::Evaluated(_)
    ));
    assert!(matches!(
        &capability(
            &number_capabilities,
            OperationFamily::NumberOverrideScenario,
            FieldCapabilityInput::Number,
        )
        .applicability,
        FieldCapabilityApplicability::Applicable
    ));

    let invalid_scenario = lifecycle
        .query_number_override_scenario(
            &DocumentScopeId::from("game-occurrence"),
            &document,
            (
                &SemanticRevision::from("r1"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &ScenarioRequest::new(vec![NumberOverride::new(computed.clone(), 11.0)], vec![]),
            &PrincipalId::from("agent"),
            NOW,
        )
        .unwrap();
    let ScenarioOutcome::InvalidOverrides(failures) = invalid_scenario.outcome else {
        panic!("a Formula target must be a structured invalid override");
    };
    assert!(matches!(
        failures.as_slice(),
        [ScenarioOverrideFailure::UnsupportedKind {
            actual: SemanticValueKind::Formula,
            ..
        }]
    ));
    assert!(matches!(
        &capability(
            &formula_capabilities,
            OperationFamily::NumberOverrideScenario,
            FieldCapabilityInput::Number,
        )
        .applicability,
        FieldCapabilityApplicability::Inapplicable {
            reason: FieldCapabilityInapplicability::UnsupportedValueKind {
                actual: SemanticValueKind::Formula,
            }
        }
    ));
}

#[test]
fn capability_query_authorizes_disclosure_before_classification_and_pins_revision() {
    let document = document();
    let field = field_ref("amount");
    let snapshot =
        ResidentWorkspaceSession::new(DocumentScopeId::from("game-occurrence"), document.clone())
            .export_snapshot();
    let mut lifecycle = lifecycle();
    let error = lifecycle
        .query_field_capabilities(&snapshot, &field, &PrincipalId::from("agent"), NOW)
        .expect_err("discovery is independently authorized");
    assert!(matches!(
        error,
        tachiko_workspace_engine::patch_lifecycle::PatchLifecycleError::DisclosureDenied
    ));

    grant_discovery(&mut lifecycle);
    let result = lifecycle
        .query_field_capabilities(&snapshot, &field, &PrincipalId::from("agent"), NOW)
        .unwrap();
    assert_eq!(result.context.source_revision(), snapshot.revision());
    let FieldCapabilityQueryOutcome::Field(capabilities) = result.outcome else {
        panic!("authorized existing target should disclose capabilities");
    };
    assert_eq!(capabilities.field, field);

    let unresolved = lifecycle
        .query_field_capabilities(
            &snapshot,
            &FieldRef::new("row", "missing"),
            &PrincipalId::from("agent"),
            NOW,
        )
        .unwrap();
    assert!(matches!(
        unresolved.outcome,
        FieldCapabilityQueryOutcome::UnresolvedTarget { .. }
    ));
}

#[test]
fn capability_query_requires_entity_and_schema_field_disclosure() {
    let document = document();
    let field = field_ref("target");
    let snapshot =
        ResidentWorkspaceSession::new(DocumentScopeId::from("game-occurrence"), document)
            .export_snapshot();

    let mut entity_field_only = lifecycle();
    grant(
        &mut entity_field_only,
        "entity-field-only",
        vec![GrantRequirement::query(
            OperationFamily::FieldCapabilityDiscovery,
            entity_field_scope(&field),
        )],
    );
    let error = entity_field_only
        .query_field_capabilities(&snapshot, &field, &PrincipalId::from("agent"), NOW)
        .expect_err("field instance authority must not disclose schema metadata");
    assert!(matches!(error, PatchLifecycleError::DisclosureDenied));

    let mut complete = lifecycle();
    grant(
        &mut complete,
        "entity-and-schema-field",
        vec![
            GrantRequirement::query(
                OperationFamily::FieldCapabilityDiscovery,
                entity_field_scope(&field),
            ),
            GrantRequirement::query(
                OperationFamily::FieldCapabilityDiscovery,
                schema_field_scope(&field),
            ),
        ],
    );
    let result = complete
        .query_field_capabilities(&snapshot, &field, &PrincipalId::from("agent"), NOW)
        .expect("both disclosure requirements should authorize discovery");
    assert!(matches!(
        result.outcome,
        FieldCapabilityQueryOutcome::Field(_)
    ));
}

#[test]
fn capability_query_keeps_document_and_schema_containment() {
    let document = document();
    let field = field_ref("target");
    let snapshot =
        ResidentWorkspaceSession::new(DocumentScopeId::from("game-occurrence"), document)
            .export_snapshot();

    for (grant_id, scope) in [
        ("document", document_scope()),
        ("schema", source_schema_scope()),
    ] {
        let mut lifecycle = lifecycle();
        grant(
            &mut lifecycle,
            grant_id,
            vec![GrantRequirement::query(
                OperationFamily::FieldCapabilityDiscovery,
                scope,
            )],
        );
        let result = lifecycle
            .query_field_capabilities(&snapshot, &field, &PrincipalId::from("agent"), NOW)
            .expect("existing broader scope containment must cover both requirements");
        assert!(matches!(
            result.outcome,
            FieldCapabilityQueryOutcome::Field(_)
        ));
    }
}

#[test]
fn discovery_projection_has_no_presentation_or_conversion_capability() {
    let capabilities = capabilities_for(&document(), &field_ref("target"));
    let actual = capabilities
        .capabilities
        .iter()
        .map(|capability| (capability.family, capability.kind, capability.input))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            (
                OperationFamily::SetFieldValue,
                FieldCapabilityKind::Edit,
                FieldCapabilityInput::TypedValue {
                    kind: SemanticValueKind::Number,
                },
            ),
            (
                OperationFamily::SetFieldValue,
                FieldCapabilityKind::Edit,
                FieldCapabilityInput::TypedValue {
                    kind: SemanticValueKind::Text,
                },
            ),
            (
                OperationFamily::SetFieldValue,
                FieldCapabilityKind::Edit,
                FieldCapabilityInput::TypedValue {
                    kind: SemanticValueKind::Boolean,
                },
            ),
            (
                OperationFamily::SetFieldValue,
                FieldCapabilityKind::Edit,
                FieldCapabilityInput::TypedValue {
                    kind: SemanticValueKind::Date,
                },
            ),
            (
                OperationFamily::SetFieldValue,
                FieldCapabilityKind::Edit,
                FieldCapabilityInput::TypedValue {
                    kind: SemanticValueKind::Reference,
                },
            ),
            (
                OperationFamily::FormulaUpdate,
                FieldCapabilityKind::Edit,
                FieldCapabilityInput::Formula,
            ),
            (
                OperationFamily::FormulaReasoning,
                FieldCapabilityKind::Query,
                FieldCapabilityInput::None,
            ),
            (
                OperationFamily::NumberOverrideScenario,
                FieldCapabilityKind::Query,
                FieldCapabilityInput::Number,
            ),
        ]
    );
}
