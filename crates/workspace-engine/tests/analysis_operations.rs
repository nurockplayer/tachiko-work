use std::collections::{BTreeMap, BTreeSet};

use tachiko_workspace_engine::{
    Date, Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
    analysis_operations::{
        AnalysisBucket, AnalysisCollectionKind, AnalysisDefinition, AnalysisDerivation,
        AnalysisFailure, AnalysisGroupKey, AnalysisLineage, AnalysisOperationError,
        AnalysisOutcome, AnalysisPredicate, AnalysisPredicateOperator, AnalysisProjection,
        AnalysisResultRequest, AnalysisResultValue, AnalysisSourceContext, AnalysisValueKind,
        MAX_ANALYSIS_COLLECTION_RESULTS, NormalizedAnalysisDefinition, NumericAggregateOutcome,
        PredicateOperand,
    },
    formula_operations::ValidatorConfiguration,
    patch_lifecycle::{
        AuthorizationDomainId, AuthorizationPolicyVersion, DocumentScopeId, Grant, GrantId,
        GrantRequirement, OperationFamily, PatchLifecycle, PatchLifecycleError, PolicyMeaningId,
        PrincipalId, PrincipalKind, ScopedSemanticSubject, SemanticApiContract, SemanticRevision,
        SemanticScope, TrustedInstant,
    },
};

const NOW: TrustedInstant = TrustedInstant::new(10);

fn number(value: f64) -> Number {
    Number::new(value).expect("fixture Numbers are finite")
}

fn number_field(id: &str, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Number,
        required,
    }
}

fn text_field(id: &str, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type: FieldType::Text,
        required,
    }
}

fn weapon(entity: &str, category: &str, tier: Option<f64>, damage: f64, interval: f64) -> Entity {
    let mut fields = BTreeMap::from([
        (FieldId::from("category"), Value::Text(category.to_owned())),
        (FieldId::from("damage"), Value::Number(number(damage))),
        (
            FieldId::from("attack_interval"),
            Value::Number(number(interval)),
        ),
        (
            FieldId::from("dps"),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Reference(FieldRef::new(entity, "damage"))),
                right: Box::new(Expression::Reference(FieldRef::new(
                    entity,
                    "attack_interval",
                ))),
            }),
        ),
    ]);
    if let Some(tier) = tier {
        fields.insert(FieldId::from("tier"), Value::Number(number(tier)));
    }
    Entity {
        id: EntityId::from(entity),
        key: EntityKey::from(format!("human-{entity}")),
        schema: SchemaId::from("weapons"),
        fields,
    }
}

fn analysis_document() -> Document {
    Document {
        id: DocumentId::from("game"),
        title: "Analysis fixture".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapons"),
            Schema {
                id: SchemaId::from("weapons"),
                key: SchemaKey::from("weapons"),
                fields: BTreeMap::from([
                    (FieldId::from("category"), text_field("category", true)),
                    (FieldId::from("tier"), number_field("tier", false)),
                    (FieldId::from("damage"), number_field("damage", true)),
                    (
                        FieldId::from("attack_interval"),
                        number_field("attack_interval", true),
                    ),
                    (FieldId::from("dps"), number_field("dps", true)),
                ]),
            },
        )]),
        entities: BTreeMap::from([
            (
                EntityId::from("alpha"),
                weapon("alpha", "melee", Some(2.0), 50.0, 1.0),
            ),
            (
                EntityId::from("beta"),
                weapon("beta", "ranged", None, 30.0, 2.0),
            ),
            (
                EntityId::from("gamma"),
                weapon("gamma", "melee", Some(3.0), 60.0, 1.5),
            ),
        ]),
    }
}

fn analysis_document_with_wrong_schema_reference() -> Document {
    let mut document = analysis_document();
    document.schemas.insert(
        SchemaId::from("categories"),
        Schema {
            id: SchemaId::from("categories"),
            key: SchemaKey::from("categories"),
            fields: BTreeMap::new(),
        },
    );
    document.schemas.insert(
        SchemaId::from("characters"),
        Schema {
            id: SchemaId::from("characters"),
            key: SchemaKey::from("characters"),
            fields: BTreeMap::new(),
        },
    );
    document.schemas.get_mut("weapons").unwrap().fields.insert(
        FieldId::from("category_ref"),
        FieldDefinition {
            id: FieldId::from("category_ref"),
            key: FieldKey::from("category_ref"),
            field_type: FieldType::Reference {
                schema: SchemaId::from("categories"),
            },
            required: false,
        },
    );
    document.entities.insert(
        EntityId::from("wrong-category"),
        Entity {
            id: EntityId::from("wrong-category"),
            key: EntityKey::from("wrong-category"),
            schema: SchemaId::from("characters"),
            fields: BTreeMap::new(),
        },
    );
    document.entities.get_mut("alpha").unwrap().fields.insert(
        FieldId::from("category_ref"),
        Value::Reference(EntityId::from("wrong-category")),
    );
    document
}

fn date_analysis_document() -> Document {
    let mut document = analysis_document();
    document.schemas.get_mut("weapons").unwrap().fields.insert(
        FieldId::from("published"),
        FieldDefinition {
            id: FieldId::from("published"),
            key: FieldKey::from("published"),
            field_type: FieldType::Date,
            required: true,
        },
    );
    for (entity, value) in [
        ("alpha", "2024-02-29"),
        ("beta", "2025-01-01"),
        ("gamma", "2024-02-29"),
    ] {
        document.entities.get_mut(entity).unwrap().fields.insert(
            FieldId::from("published"),
            Value::Date(value.parse::<Date>().unwrap()),
        );
    }
    document
}

fn principal() -> PrincipalId {
    PrincipalId::from("agent")
}

fn scope_id() -> DocumentScopeId {
    DocumentScopeId::from("game-occurrence")
}

fn document_scope() -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        scope_id(),
        DocumentId::from("game"),
        SemanticScope::Document,
    )
}

fn entity_scope(entity: &str) -> ScopedSemanticSubject {
    entity_scope_in_schema(entity, "weapons")
}

fn entity_scope_in_schema(entity: &str, schema: &str) -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        scope_id(),
        DocumentId::from("game"),
        SemanticScope::Entity {
            entity: EntityId::from(entity),
            schema: SchemaId::from(schema),
        },
    )
}

fn schema_scope() -> ScopedSemanticSubject {
    ScopedSemanticSubject::new(
        scope_id(),
        DocumentId::from("game"),
        SemanticScope::Schema(SchemaId::from("weapons")),
    )
}

fn lifecycle() -> PatchLifecycle {
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from("local-domain"),
        scope_id(),
        DocumentId::from("game"),
        SemanticApiContract::from("tachiko-sem-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("policy-v1-meaning"),
    );
    lifecycle
        .register_principal(PrincipalId::from("authority"), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal(principal(), PrincipalKind::Delegated)
        .unwrap();
    lifecycle
}

fn grant_query(lifecycle: &mut PatchLifecycle, id: &str, scopes: Vec<ScopedSemanticSubject>) {
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from(id),
            PrincipalId::from("authority"),
            principal(),
            scopes
                .into_iter()
                .map(|scope| GrantRequirement::query(OperationFamily::AnalysisQuery, scope))
                .collect(),
            None,
        ))
        .unwrap();
}

fn query_lifecycle(
    document: &Document,
    scope: &DocumentScopeId,
    principal: &PrincipalId,
    suffix: &str,
    grant_scope: SemanticScope,
) -> PatchLifecycle {
    let authority = PrincipalId::from(format!("{suffix}-authority"));
    let mut lifecycle = PatchLifecycle::new(
        AuthorizationDomainId::from(format!("{suffix}-domain")),
        scope.clone(),
        document.id.clone(),
        SemanticApiContract::from("tachiko-sem-v1"),
        AuthorizationPolicyVersion::from("policy-v1"),
        PolicyMeaningId::from("policy-v1-meaning"),
    );
    lifecycle
        .register_principal(authority.clone(), PrincipalKind::Human)
        .unwrap();
    lifecycle
        .register_principal(principal.clone(), PrincipalKind::Delegated)
        .unwrap();
    lifecycle
        .provision_grant(Grant::new(
            GrantId::from(format!("{suffix}-query")),
            authority,
            principal.clone(),
            vec![GrantRequirement::query(
                OperationFamily::AnalysisQuery,
                ScopedSemanticSubject::new(scope.clone(), document.id.clone(), grant_scope),
            )],
            None,
        ))
        .unwrap();
    lifecycle
}

fn revision(value: &str) -> SemanticRevision {
    SemanticRevision::from(value)
}

fn run(
    lifecycle: &PatchLifecycle,
    document: &Document,
    definition: &AnalysisDefinition,
) -> Result<
    tachiko_workspace_engine::analysis_operations::AnalysisQueryResult,
    AnalysisOperationError,
> {
    lifecycle.query_analysis(
        &scope_id(),
        document,
        (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
        definition,
        &principal(),
        NOW,
    )
}

fn complete_values(
    result: tachiko_workspace_engine::analysis_operations::AnalysisQueryResult,
) -> Vec<AnalysisResultValue> {
    let AnalysisOutcome::Complete(AnalysisProjection::Ungrouped(bucket)) = result.outcome else {
        panic!("expected complete ungrouped analysis")
    };
    bucket.values
}

#[test]
fn typed_filter_uses_optional_missing_as_false_and_stable_ids_as_a_narrowing_intersection() {
    let document = analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha"), EntityId::from("beta")]),
        vec![AnalysisPredicate::new(
            FieldId::from("tier"),
            AnalysisPredicateOperator::GreaterThanOrEqual,
            PredicateOperand::Number(number(2.0)),
        )],
        None,
        vec![
            AnalysisResultRequest::Membership,
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Minimum(FieldId::from("damage")),
            AnalysisResultRequest::Maximum(FieldId::from("dps")),
            AnalysisResultRequest::Observations(FieldId::from("dps")),
        ],
    );

    let result = run(&lifecycle, &document, &definition).unwrap();
    assert_eq!(result.lineage.sources[0].source_revision, revision("r1"));
    assert_eq!(
        result
            .lineage
            .normalized_definition
            .narrowing
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    assert!(result.lineage.formula_calculation_used);
    let values = complete_values(result);
    assert_eq!(
        values,
        vec![
            AnalysisResultValue::Membership(vec![EntityId::from("alpha")]),
            AnalysisResultValue::Count(1),
            AnalysisResultValue::Minimum {
                field: FieldId::from("damage"),
                outcome: NumericAggregateOutcome::Value(number(50.0)),
            },
            AnalysisResultValue::Maximum {
                field: FieldId::from("dps"),
                outcome: NumericAggregateOutcome::Value(number(50.0)),
            },
            AnalysisResultValue::Observations {
                field: FieldId::from("dps"),
                values: vec![(EntityId::from("alpha"), number(50.0))],
            },
        ]
    );
}

#[test]
fn date_predicates_order_and_grouping_use_the_closed_semantic_value() {
    let document = date_analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let threshold = "2025-01-01".parse::<Date>().unwrap();
    let filtered = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("published"),
            AnalysisPredicateOperator::GreaterThanOrEqual,
            PredicateOperand::Date(threshold),
        )],
        None,
        vec![
            AnalysisResultRequest::Membership,
            AnalysisResultRequest::Count,
        ],
    );
    assert_eq!(
        complete_values(run(&lifecycle, &document, &filtered).unwrap()),
        vec![
            AnalysisResultValue::Membership(vec![EntityId::from("beta")]),
            AnalysisResultValue::Count(1),
        ]
    );

    let grouped = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        Some(FieldId::from("published")),
        vec![AnalysisResultRequest::Count],
    );
    let AnalysisOutcome::Complete(AnalysisProjection::Grouped(groups)) =
        run(&lifecycle, &document, &grouped).unwrap().outcome
    else {
        panic!("expected date grouping")
    };
    assert_eq!(groups.len(), 2);
    assert_eq!(
        groups[0].key,
        AnalysisGroupKey::Date("2024-02-29".parse().unwrap())
    );
    assert_eq!(groups[0].bucket.values, vec![AnalysisResultValue::Count(2)]);
    assert_eq!(
        groups[1].key,
        AnalysisGroupKey::Date("2025-01-01".parse().unwrap())
    );
    assert_eq!(groups[1].bucket.values, vec![AnalysisResultValue::Count(1)]);
}

#[test]
fn explicit_empty_narrowing_is_an_empty_intersection_not_an_unbounded_domain() {
    let document = analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![]),
        vec![],
        None,
        vec![
            AnalysisResultRequest::Membership,
            AnalysisResultRequest::Count,
        ],
    );

    let result = run(&lifecycle, &document, &definition).unwrap();
    assert_eq!(
        complete_values(result.clone()),
        vec![
            AnalysisResultValue::Membership(vec![]),
            AnalysisResultValue::Count(0),
        ]
    );
    assert_eq!(
        result.lineage.normalized_definition.narrowing,
        Some(BTreeSet::new())
    );
}

#[test]
fn grouping_partitions_membership_once_and_reduces_count_min_and_max_per_group() {
    let document = analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        Some(FieldId::from("category")),
        vec![
            AnalysisResultRequest::Membership,
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Minimum(FieldId::from("damage")),
            AnalysisResultRequest::Maximum(FieldId::from("dps")),
        ],
    );

    let result = run(&lifecycle, &document, &definition).unwrap();
    let AnalysisOutcome::Complete(AnalysisProjection::Grouped(groups)) = result.outcome else {
        panic!("expected grouped analysis")
    };
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].key, AnalysisGroupKey::Text("melee".to_owned()));
    assert_eq!(groups[1].key, AnalysisGroupKey::Text("ranged".to_owned()));
    assert_eq!(
        groups[0].bucket.values,
        vec![
            AnalysisResultValue::Membership(
                vec![EntityId::from("alpha"), EntityId::from("gamma"),]
            ),
            AnalysisResultValue::Count(2),
            AnalysisResultValue::Minimum {
                field: FieldId::from("damage"),
                outcome: NumericAggregateOutcome::Value(number(50.0)),
            },
            AnalysisResultValue::Maximum {
                field: FieldId::from("dps"),
                outcome: NumericAggregateOutcome::Value(number(50.0)),
            },
        ]
    );
    let membership = groups
        .iter()
        .flat_map(|group| group.bucket.values.iter())
        .filter_map(|value| match value {
            AnalysisResultValue::Membership(members) => Some(members.iter().cloned()),
            _ => None,
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    assert_eq!(membership, document.entities.keys().cloned().collect());
}

#[test]
fn empty_selection_has_zero_count_structured_empty_aggregates_and_no_synthetic_group() {
    let document = analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("damage"),
            AnalysisPredicateOperator::GreaterThan,
            PredicateOperand::Number(number(1_000.0)),
        )],
        None,
        vec![
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Minimum(FieldId::from("damage")),
            AnalysisResultRequest::Maximum(FieldId::from("damage")),
        ],
    );
    assert_eq!(
        complete_values(run(&lifecycle, &document, &definition).unwrap()),
        vec![
            AnalysisResultValue::Count(0),
            AnalysisResultValue::Minimum {
                field: FieldId::from("damage"),
                outcome: NumericAggregateOutcome::Empty,
            },
            AnalysisResultValue::Maximum {
                field: FieldId::from("damage"),
                outcome: NumericAggregateOutcome::Empty,
            },
        ]
    );

    let grouped = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        definition.predicates.clone(),
        Some(FieldId::from("category")),
        vec![AnalysisResultRequest::Count],
    );
    let result = run(&lifecycle, &document, &grouped).unwrap();
    assert!(matches!(
        result.outcome,
        AnalysisOutcome::Complete(AnalysisProjection::Grouped(ref groups)) if groups.is_empty()
    ));
}

#[test]
fn grouping_missing_value_and_formula_value_are_structured_failures() {
    let mut missing = analysis_document();
    missing
        .entities
        .get_mut("beta")
        .unwrap()
        .fields
        .remove("category");
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let missing_definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        Some(FieldId::from("category")),
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&lifecycle, &missing, &missing_definition)
            .unwrap()
            .outcome,
        AnalysisOutcome::Failure(AnalysisFailure::MissingGroupValue { entity, field })
            if entity == EntityId::from("beta") && field == FieldId::from("category")
    ));

    let formula_group = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        Some(FieldId::from("dps")),
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&lifecycle, &analysis_document(), &formula_group)
            .unwrap()
            .outcome,
        AnalysisOutcome::Failure(AnalysisFailure::FormulaGroupingUnsupported { field })
            if field == FieldId::from("dps")
    ));
}

#[test]
fn grouping_rejects_reference_values_from_the_wrong_schema() {
    let document = analysis_document_with_wrong_schema_reference();
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha")]),
        vec![],
        Some(FieldId::from("category_ref")),
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    assert!(matches!(
        run(&lifecycle, &document, &definition).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::InvalidGroupValue {
            entity,
            field,
            actual: AnalysisValueKind::Reference,
        }) if entity == EntityId::from("alpha") && field == FieldId::from("category_ref")
    ));
}

#[test]
fn predicate_rejects_reference_values_from_the_wrong_schema() {
    let document = analysis_document_with_wrong_schema_reference();
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha")]),
        vec![AnalysisPredicate::new(
            FieldId::from("category_ref"),
            AnalysisPredicateOperator::Equal,
            PredicateOperand::Reference(EntityId::from("wrong-category")),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    assert!(matches!(
        run(&lifecycle, &document, &definition).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::InvalidPredicateValue {
            entity,
            field,
            actual: AnalysisValueKind::Reference,
        }) if entity == EntityId::from("alpha") && field == FieldId::from("category_ref")
    ));
}

#[test]
fn formula_predicate_failure_and_metric_incompleteness_return_no_partial_payload() {
    let mut failed_formula = analysis_document();
    failed_formula
        .entities
        .get_mut("beta")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), Value::Number(number(0.0)));
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let formula_predicate = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("dps"),
            AnalysisPredicateOperator::GreaterThan,
            PredicateOperand::Number(number(10.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&lifecycle, &failed_formula, &formula_predicate)
            .unwrap()
            .outcome,
        AnalysisOutcome::Failure(AnalysisFailure::CalculationFailed { field, .. })
            if field == FieldRef::new("beta", "dps")
    ));

    let mut incomplete = analysis_document();
    incomplete
        .entities
        .get_mut("alpha")
        .unwrap()
        .fields
        .remove("damage");
    let metric = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha")]),
        vec![],
        None,
        vec![
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Minimum(FieldId::from("damage")),
        ],
    );
    assert!(matches!(
        run(&lifecycle, &incomplete, &metric).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::MetricIncomplete { entity, field, .. })
            if entity == EntityId::from("alpha") && field == FieldId::from("damage")
    ));
}

#[test]
fn formula_predicate_failure_is_not_masked_by_an_earlier_false_predicate() {
    let mut document = analysis_document();
    document
        .entities
        .get_mut("alpha")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), Value::Number(number(0.0)));
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![
            AnalysisPredicate::new(
                FieldId::from("category"),
                AnalysisPredicateOperator::Equal,
                PredicateOperand::Text("ranged".to_owned()),
            ),
            AnalysisPredicate::new(
                FieldId::from("dps"),
                AnalysisPredicateOperator::GreaterThan,
                PredicateOperand::Number(number(10.0)),
            ),
        ],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    assert!(matches!(
        run(&lifecycle, &document, &definition).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::CalculationFailed {
            field,
            failure: Some(tachiko_workspace_engine::CalculationFailure::DivisionByZero),
        }) if field == FieldRef::new("alpha", "dps")
    ));
}

#[test]
fn formula_failure_evidence_never_names_an_unrequested_ungranted_target() {
    let mut document = analysis_document();
    document
        .entities
        .get_mut("beta")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), Value::Number(number(0.0)));
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha")]),
        vec![AnalysisPredicate::new(
            FieldId::from("dps"),
            AnalysisPredicateOperator::GreaterThan,
            PredicateOperand::Number(number(10.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "alpha-only", vec![entity_scope("alpha")]);

    assert!(matches!(
        run(&lifecycle, &document, &definition),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));
}

#[test]
fn lineage_records_formula_calculation_only_after_the_oracle_runs() {
    let document = analysis_document();
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("dps"),
            AnalysisPredicateOperator::Equal,
            PredicateOperand::Text("not-a-number".to_owned()),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    let result = run(&lifecycle, &document, &definition).unwrap();
    assert!(!result.lineage.formula_calculation_used);
    assert!(matches!(
        result.outcome,
        AnalysisOutcome::Failure(AnalysisFailure::InvalidPredicateType { field, .. })
            if field == FieldId::from("dps")
    ));
}

#[test]
fn authorized_target_domain_and_type_errors_remain_structured() {
    let document = analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    let unresolved = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("missing"),
            AnalysisPredicateOperator::Equal,
            PredicateOperand::Number(number(1.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&lifecycle, &document, &unresolved).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::UnresolvedField { field, .. })
            if field == FieldId::from("missing")
    ));

    let wrong_predicate_type = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("category"),
            AnalysisPredicateOperator::GreaterThan,
            PredicateOperand::Number(number(1.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&lifecycle, &document, &wrong_predicate_type)
            .unwrap()
            .outcome,
        AnalysisOutcome::Failure(AnalysisFailure::InvalidPredicateType { field, .. })
            if field == FieldId::from("category")
    ));

    let wrong_metric_type = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![AnalysisResultRequest::Minimum(FieldId::from("category"))],
    );
    assert!(matches!(
        run(&lifecycle, &document, &wrong_metric_type)
            .unwrap()
            .outcome,
        AnalysisOutcome::Failure(AnalysisFailure::InvalidMetricType { field, .. })
            if field == FieldId::from("category")
    ));

    let wrong_domain = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("outsider")]),
        vec![],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&lifecycle, &document, &wrong_domain).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::UnresolvedNarrowingEntity { entity })
            if entity == EntityId::from("outsider")
    ));

    let mut other_domain = document.clone();
    other_domain.schemas.insert(
        SchemaId::from("characters"),
        Schema {
            id: SchemaId::from("characters"),
            key: SchemaKey::from("characters"),
            fields: BTreeMap::new(),
        },
    );
    other_domain.entities.insert(
        EntityId::from("outsider"),
        Entity {
            id: EntityId::from("outsider"),
            key: EntityKey::from("human-outsider"),
            schema: SchemaId::from("characters"),
            fields: BTreeMap::new(),
        },
    );
    assert!(matches!(
        run(&lifecycle, &other_domain, &wrong_domain).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::WrongDomainNarrowingEntity {
            entity,
            expected,
            actual,
        }) if entity == EntityId::from("outsider")
            && expected == SchemaId::from("weapons")
            && actual == SchemaId::from("characters")
    ));
}

#[test]
fn malformed_runtime_predicate_value_is_not_silently_filtered_out() {
    let mut document = analysis_document();
    document
        .entities
        .get_mut("alpha")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), Value::Text("invalid".to_owned()));
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha")]),
        vec![AnalysisPredicate::new(
            FieldId::from("damage"),
            AnalysisPredicateOperator::Equal,
            PredicateOperand::Number(number(50.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    assert!(matches!(
        run(&lifecycle, &document, &definition).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::InvalidPredicateValue {
            entity,
            field,
            ..
        }) if entity == EntityId::from("alpha") && field == FieldId::from("damage")
    ));
}

#[test]
fn incoherent_candidate_map_identity_fails_closed_without_panicking() {
    let mut document = analysis_document();
    let alpha = document.entities.remove("alpha").unwrap();
    document.entities.insert(EntityId::from("map-key"), alpha);
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);

    assert!(matches!(
        run(&lifecycle, &document, &definition).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::IncoherentCandidateIdentity {
            key,
            entity,
        }) if key == EntityId::from("map-key") && entity == EntityId::from("alpha")
    ));
}

#[test]
fn bounded_envelope_admission_precedes_source_and_principal_lookup() {
    let document = analysis_document();
    let definition = AnalysisDefinition::new(SchemaId::from("weapons"), None, vec![], None, vec![]);
    let error = lifecycle()
        .query_analysis(
            &DocumentScopeId::from("wrong-occurrence"),
            &document,
            (
                &revision("untrusted"),
                ValidatorConfiguration::WorkspaceFull,
            ),
            &definition,
            &PrincipalId::from("unknown"),
            NOW,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        AnalysisOperationError::Envelope(
            tachiko_workspace_engine::analysis_operations::AnalysisEnvelopeError::MissingResults
        )
    ));
}

#[test]
fn complete_candidate_domain_is_authorized_before_filtering_and_narrowing_grants_nothing() {
    let document = analysis_document();
    let mut scoped_lifecycle = lifecycle();
    grant_query(
        &mut scoped_lifecycle,
        "alpha-only",
        vec![entity_scope("alpha")],
    );
    let broad = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![AnalysisPredicate::new(
            FieldId::from("damage"),
            AnalysisPredicateOperator::GreaterThan,
            PredicateOperand::Number(number(45.0)),
        )],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&scoped_lifecycle, &document, &broad),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));

    let narrowed = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("alpha")]),
        broad.predicates.clone(),
        None,
        vec![
            AnalysisResultRequest::Membership,
            AnalysisResultRequest::Count,
        ],
    );
    assert_eq!(
        complete_values(run(&scoped_lifecycle, &document, &narrowed).unwrap()),
        vec![
            AnalysisResultValue::Membership(vec![EntityId::from("alpha")]),
            AnalysisResultValue::Count(1),
        ]
    );

    let no_grant = lifecycle();
    assert!(matches!(
        run(&no_grant, &document, &narrowed),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));

    let empty_narrowing = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![]),
        vec![],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&no_grant, &document, &empty_narrowing),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));

    let mut empty_domain = document.clone();
    empty_domain.entities.clear();
    let count_empty = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        run(&no_grant, &empty_domain, &count_empty),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));
    let mut empty_authorized = lifecycle();
    grant_query(
        &mut empty_authorized,
        "empty-schema-query",
        vec![schema_scope()],
    );
    assert_eq!(
        complete_values(run(&empty_authorized, &document, &empty_narrowing).unwrap()),
        vec![AnalysisResultValue::Count(0)]
    );
    assert_eq!(
        complete_values(run(&empty_authorized, &empty_domain, &count_empty).unwrap()),
        vec![AnalysisResultValue::Count(0)]
    );
}

#[test]
fn unbounded_schema_membership_requires_schema_authority_despite_complete_entity_grants() {
    let document = analysis_document();
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![
            AnalysisResultRequest::Membership,
            AnalysisResultRequest::Count,
        ],
    );
    let mut entity_authorized = lifecycle();
    grant_query(
        &mut entity_authorized,
        "all-current-entities",
        vec![
            entity_scope("alpha"),
            entity_scope("beta"),
            entity_scope("gamma"),
        ],
    );

    assert!(matches!(
        run(&entity_authorized, &document, &definition),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));

    let mut schema_authorized = lifecycle();
    grant_query(
        &mut schema_authorized,
        "weapons-schema",
        vec![schema_scope()],
    );
    assert_eq!(
        complete_values(run(&schema_authorized, &document, &definition).unwrap()),
        vec![
            AnalysisResultValue::Membership(vec![
                EntityId::from("alpha"),
                EntityId::from("beta"),
                EntityId::from("gamma"),
            ]),
            AnalysisResultValue::Count(3),
        ]
    );
}

#[test]
fn wrong_domain_narrowing_requires_requested_schema_authority_before_classification() {
    let document = analysis_document_with_wrong_schema_reference();
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("wrong-category")]),
        vec![],
        None,
        vec![AnalysisResultRequest::Count],
    );
    let mut requested_schema_authorized = lifecycle();
    grant_query(
        &mut requested_schema_authorized,
        "requested-weapons-schema-only",
        vec![schema_scope()],
    );
    assert!(matches!(
        run(&requested_schema_authorized, &document, &definition),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));

    let mut actual_entity_authorized = lifecycle();
    grant_query(
        &mut actual_entity_authorized,
        "outsider-only",
        vec![entity_scope_in_schema("wrong-category", "characters")],
    );

    assert!(matches!(
        run(&actual_entity_authorized, &document, &definition),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));

    grant_query(
        &mut actual_entity_authorized,
        "requested-weapons-schema",
        vec![schema_scope()],
    );
    assert!(matches!(
        run(&actual_entity_authorized, &document, &definition)
            .unwrap()
            .outcome,
        AnalysisOutcome::Failure(AnalysisFailure::WrongDomainNarrowingEntity {
            entity,
            expected,
            actual,
        }) if entity == EntityId::from("wrong-category")
            && expected == SchemaId::from("weapons")
            && actual == SchemaId::from("characters")
    ));
}

#[test]
fn result_bounds_are_classified_only_after_authorization_and_never_truncate() {
    let mut document = analysis_document();
    document.entities.clear();
    for index in 0..=MAX_ANALYSIS_COLLECTION_RESULTS {
        let id = format!("weapon-{index:03}");
        let damage = f64::from(u32::try_from(index).unwrap()) + 1.0;
        document.entities.insert(
            EntityId::from(id.clone()),
            weapon(&id, "melee", Some(1.0), damage, 1.0),
        );
    }
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![AnalysisResultRequest::Membership],
    );

    let mut authorized = lifecycle();
    grant_query(&mut authorized, "document-query", vec![document_scope()]);
    assert!(matches!(
        run(&authorized, &document, &definition).unwrap().outcome,
        AnalysisOutcome::Failure(AnalysisFailure::ResultTooLarge {
            collection: AnalysisCollectionKind::Membership,
            ..
        })
    ));
    assert!(matches!(
        run(&lifecycle(), &document, &definition),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));
}

#[test]
fn paired_contexts_reuse_one_normalized_definition_and_deny_the_whole_pair() {
    let mut first = analysis_document();
    first
        .entities
        .retain(|entity, _| entity == &EntityId::from("alpha"));
    let mut second = first.clone();
    second.id = DocumentId::from("comparison-game");
    second
        .entities
        .get_mut("alpha")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), Value::Number(number(80.0)));
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![
            AnalysisResultRequest::Count,
            AnalysisResultRequest::Maximum(FieldId::from("dps")),
        ],
    );

    let mut authorized = lifecycle();
    grant_query(&mut authorized, "document-query", vec![document_scope()]);
    let second_scope = DocumentScopeId::from("comparison-occurrence");
    let second_principal = PrincipalId::from("comparison-agent");
    let second_authorized = query_lifecycle(
        &second,
        &second_scope,
        &second_principal,
        "comparison",
        SemanticScope::Document,
    );
    let paired = authorized
        .query_analysis_pair(
            &scope_id(),
            &first,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &second_authorized,
            &second_scope,
            &second,
            (&revision("r2"), ValidatorConfiguration::WorkspaceFull),
            &definition,
            &principal(),
            &second_principal,
            NOW,
        )
        .unwrap();
    assert_eq!(paired.lineage.sources.len(), 2);
    assert_eq!(paired.lineage.sources[0].source_revision, revision("r1"));
    assert_eq!(paired.lineage.sources[1].source_revision, revision("r2"));
    assert_eq!(
        paired.lineage.normalized_definition,
        run(&authorized, &first, &definition)
            .unwrap()
            .lineage
            .normalized_definition
    );
    assert_ne!(paired.first, paired.second);

    let mut wider_second = second.clone();
    wider_second.entities.insert(
        EntityId::from("beta"),
        weapon("beta", "ranged", None, 30.0, 2.0),
    );
    let second_limited = query_lifecycle(
        &wider_second,
        &second_scope,
        &second_principal,
        "comparison-limited",
        SemanticScope::Entity {
            entity: EntityId::from("alpha"),
            schema: SchemaId::from("weapons"),
        },
    );
    let count_definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        None,
        vec![],
        None,
        vec![AnalysisResultRequest::Count],
    );
    assert!(matches!(
        authorized.query_analysis_pair(
            &scope_id(),
            &first,
            (&revision("r1"), ValidatorConfiguration::WorkspaceFull),
            &second_limited,
            &second_scope,
            &wider_second,
            (&revision("r2"), ValidatorConfiguration::WorkspaceFull),
            &count_definition,
            &principal(),
            &second_principal,
            NOW,
        ),
        Err(AnalysisOperationError::Lifecycle(
            PatchLifecycleError::DisclosureDenied
        ))
    ));
}

#[test]
fn repeated_equal_query_is_exactly_reproducible_with_structured_lineage() {
    let document = analysis_document();
    let mut lifecycle = lifecycle();
    grant_query(&mut lifecycle, "document-query", vec![document_scope()]);
    let definition = AnalysisDefinition::new(
        SchemaId::from("weapons"),
        Some(vec![EntityId::from("gamma"), EntityId::from("alpha")]),
        vec![AnalysisPredicate::new(
            FieldId::from("damage"),
            AnalysisPredicateOperator::GreaterThanOrEqual,
            PredicateOperand::Number(number(40.0)),
        )],
        None,
        vec![
            AnalysisResultRequest::Observations(FieldId::from("dps")),
            AnalysisResultRequest::Count,
        ],
    );

    let first = run(&lifecycle, &document, &definition).unwrap();
    let second = run(&lifecycle, &document, &definition).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.lineage,
        AnalysisLineage {
            sources: vec![AnalysisSourceContext {
                document: DocumentId::from("game"),
                source_revision: revision("r1"),
                validator_configuration: ValidatorConfiguration::WorkspaceFull,
            }],
            normalized_definition: NormalizedAnalysisDefinition {
                schema: SchemaId::from("weapons"),
                narrowing: Some(BTreeSet::from([
                    EntityId::from("alpha"),
                    EntityId::from("gamma"),
                ])),
                predicates: vec![AnalysisPredicate::new(
                    FieldId::from("damage"),
                    AnalysisPredicateOperator::GreaterThanOrEqual,
                    PredicateOperand::Number(number(40.0)),
                )],
                group_by: None,
                results: vec![
                    AnalysisResultRequest::Count,
                    AnalysisResultRequest::Observations(FieldId::from("dps")),
                ],
            },
            formula_calculation_used: true,
            derivations: vec![
                AnalysisDerivation::Predicate(FieldId::from("damage")),
                AnalysisDerivation::Count,
                AnalysisDerivation::Observations(FieldId::from("dps")),
            ],
        }
    );
    assert_eq!(
        first.outcome,
        AnalysisOutcome::Complete(AnalysisProjection::Ungrouped(AnalysisBucket {
            values: vec![
                AnalysisResultValue::Count(2),
                AnalysisResultValue::Observations {
                    field: FieldId::from("dps"),
                    values: vec![
                        (EntityId::from("alpha"), number(50.0)),
                        (EntityId::from("gamma"), number(40.0)),
                    ],
                },
            ],
        }))
    );
}
