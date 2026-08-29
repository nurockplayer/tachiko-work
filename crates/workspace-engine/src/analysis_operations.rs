//! Provisional bounded semantic Analysis Query workspace implementation.
//!
//! The Rust and result shapes in this module are replaceable implementation
//! details. The evaluation and disclosure ordering preserve the Accepted M04
//! logical contract without defining a public wire or SDK surface.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use tachiko_formula_engine::{CalculationOutcome, calculate_complete, extract_dependencies};
use thiserror::Error;

use crate::formula_operations::ValidatorConfiguration;
use crate::patch_lifecycle::{
    DisclosureRequirement, DocumentScopeId, OperationFamily, PatchLifecycle, PatchLifecycleError,
    PrincipalId, ScopedSemanticSubject, SemanticRevision, SemanticScope, TrustedInstant,
};
use crate::{
    CalculationFailure, Document, DocumentId, EntityId, FieldId, FieldRef, FieldType, Number,
    SchemaId, Value,
};

/// Current finite explicit-EntityId narrowing profile.
pub const MAX_ANALYSIS_NARROWING_IDS: usize = 64;
/// Current finite AND-predicate profile.
pub const MAX_ANALYSIS_PREDICATES: usize = 16;
/// Current finite requested-result profile.
pub const MAX_ANALYSIS_RESULT_REQUESTS: usize = 16;
/// Current finite complete collection-result profile.
pub const MAX_ANALYSIS_COLLECTION_RESULTS: usize = 64;

/// Caller-facing, context-independent typed analysis definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisDefinition {
    pub schema: SchemaId,
    pub narrowing: Option<Vec<EntityId>>,
    pub predicates: Vec<AnalysisPredicate>,
    pub group_by: Option<FieldId>,
    pub results: Vec<AnalysisResultRequest>,
}

impl AnalysisDefinition {
    #[must_use]
    pub fn new(
        schema: SchemaId,
        narrowing: Option<Vec<EntityId>>,
        predicates: Vec<AnalysisPredicate>,
        group_by: Option<FieldId>,
        results: Vec<AnalysisResultRequest>,
    ) -> Self {
        Self {
            schema,
            narrowing,
            predicates,
            group_by,
            results,
        }
    }

    /// Admit and normalize only disclosure-independent request-local facts.
    ///
    /// # Errors
    ///
    /// Returns a bounded envelope failure without consulting semantic state.
    pub fn admit_envelope(&self) -> Result<NormalizedAnalysisDefinition, AnalysisEnvelopeError> {
        normalize_definition(self)
    }
}

/// Context-independent normalized definition used as reproducibility lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedAnalysisDefinition {
    pub schema: SchemaId,
    pub narrowing: Option<BTreeSet<EntityId>>,
    pub predicates: Vec<AnalysisPredicate>,
    pub group_by: Option<FieldId>,
    pub results: Vec<AnalysisResultRequest>,
}

/// One typed AND predicate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AnalysisPredicate {
    pub field: FieldId,
    pub operator: AnalysisPredicateOperator,
    pub operand: PredicateOperand,
}

impl AnalysisPredicate {
    #[must_use]
    pub fn new(
        field: FieldId,
        operator: AnalysisPredicateOperator,
        operand: PredicateOperand,
    ) -> Self {
        Self {
            field,
            operator,
            operand,
        }
    }
}

/// Provisional finite typed predicate-operator catalogue.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisPredicateOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// Typed predicate operand; no coercive representation matching is supported.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PredicateOperand {
    Number(Number),
    Text(String),
    Boolean(bool),
    Reference(EntityId),
}

/// Supported bounded M04 result requests.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisResultRequest {
    Membership,
    Count,
    Minimum(FieldId),
    Maximum(FieldId),
    Observations(FieldId),
}

/// Disclosure-independent request envelope failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AnalysisEnvelopeError {
    #[error("analysis has too many explicit narrowing identities")]
    TooManyNarrowingIds,
    #[error("analysis has too many predicates")]
    TooManyPredicates,
    #[error("analysis must request at least one result")]
    MissingResults,
    #[error("analysis has too many result requests")]
    TooManyResults,
}

/// Boundary failure before a disclosure-safe structured semantic result.
#[derive(Debug, Error)]
pub enum AnalysisOperationError {
    #[error(transparent)]
    Envelope(#[from] AnalysisEnvelopeError),
    #[error(transparent)]
    Lifecycle(#[from] PatchLifecycleError),
}

/// Semantic role of a field target in a structured target failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisFieldRole {
    Predicate,
    Group,
    Metric,
}

/// Existing semantic value kind exposed only after sufficient Query authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisValueKind {
    Number,
    Formula,
    Text,
    Boolean,
    Reference,
}

/// Why a selected member could not supply a complete Number observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetricIncompleteReason {
    Missing,
    WrongKind(AnalysisValueKind),
}

/// Bounded collection whose complete result exceeded the current profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisCollectionKind {
    Membership,
    Groups,
    Observations,
}

/// Authorized semantic analysis failure. No successful payload accompanies it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisFailure {
    UnresolvedSchema {
        schema: SchemaId,
    },
    UnresolvedField {
        role: AnalysisFieldRole,
        field: FieldId,
    },
    UnresolvedNarrowingEntity {
        entity: EntityId,
    },
    WrongDomainNarrowingEntity {
        entity: EntityId,
        expected: SchemaId,
        actual: SchemaId,
    },
    IncoherentCandidateIdentity {
        key: EntityId,
        entity: EntityId,
    },
    InvalidPredicateType {
        field: FieldId,
        declared: FieldType,
    },
    InvalidMetricType {
        field: FieldId,
        declared: FieldType,
    },
    InvalidPredicateValue {
        entity: EntityId,
        field: FieldId,
        actual: AnalysisValueKind,
    },
    MissingGroupValue {
        entity: EntityId,
        field: FieldId,
    },
    FormulaGroupingUnsupported {
        field: FieldId,
    },
    InvalidGroupValue {
        entity: EntityId,
        field: FieldId,
        actual: AnalysisValueKind,
    },
    CalculationFailed {
        field: FieldRef,
        failure: Option<CalculationFailure>,
    },
    MetricIncomplete {
        entity: EntityId,
        field: FieldId,
        reason: MetricIncompleteReason,
    },
    ResultTooLarge {
        collection: AnalysisCollectionKind,
        limit: usize,
    },
}

/// Exact source context recorded as semantic execution lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisSourceContext {
    pub document: DocumentId,
    pub source_revision: SemanticRevision,
    pub validator_configuration: ValidatorConfiguration,
}

/// Structured derivation meaning for a normalized result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisDerivation {
    Predicate(FieldId),
    GroupedBy(FieldId),
    Membership,
    Count,
    Minimum(FieldId),
    Maximum(FieldId),
    Observations(FieldId),
}

/// Reproducibility evidence shared by one-context and paired results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisLineage {
    pub sources: Vec<AnalysisSourceContext>,
    pub normalized_definition: NormalizedAnalysisDefinition,
    pub formula_calculation_used: bool,
    pub derivations: Vec<AnalysisDerivation>,
}

/// One complete or structured-failure analysis result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisQueryResult {
    pub lineage: AnalysisLineage,
    pub outcome: AnalysisOutcome,
}

/// Same normalized definition evaluated over two explicit exact contexts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedAnalysisQueryResult {
    pub lineage: AnalysisLineage,
    pub first: AnalysisOutcome,
    pub second: AnalysisOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisOutcome {
    Complete(AnalysisProjection),
    Failure(AnalysisFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisProjection {
    Ungrouped(AnalysisBucket),
    Grouped(Vec<AnalysisGroup>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisGroup {
    pub key: AnalysisGroupKey,
    pub bucket: AnalysisBucket,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AnalysisGroupKey {
    Number(Number),
    Text(String),
    Boolean(bool),
    Reference(EntityId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisBucket {
    pub values: Vec<AnalysisResultValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NumericAggregateOutcome {
    Value(Number),
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisResultValue {
    Membership(Vec<EntityId>),
    Count(u64),
    Minimum {
        field: FieldId,
        outcome: NumericAggregateOutcome,
    },
    Maximum {
        field: FieldId,
        outcome: NumericAggregateOutcome,
    },
    Observations {
        field: FieldId,
        values: Vec<(EntityId, Number)>,
    },
}

struct PreparedAnalysis {
    candidates: Vec<EntityId>,
    requirements: BTreeSet<DisclosureRequirement>,
    deferred_failure: Option<AnalysisFailure>,
}

struct EvaluatedAnalysis {
    outcome: AnalysisOutcome,
    requirements: BTreeSet<DisclosureRequirement>,
    formula_calculation_used: bool,
}

impl PatchLifecycle {
    /// Evaluate one bounded Analysis Query against one exact semantic context.
    ///
    /// Envelope admission precedes source lookup. Trusted candidate-domain and
    /// fact-scope resolution plus complete Query authorization precede target
    /// classification, calculation, predicate truth, grouping, and reduction.
    ///
    /// # Errors
    ///
    /// Returns a request-local envelope error or disclosure-safe lifecycle
    /// denial before a safe structured semantic result exists.
    pub fn query_analysis(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        trusted_source: (&SemanticRevision, ValidatorConfiguration),
        definition: &AnalysisDefinition,
        principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<AnalysisQueryResult, AnalysisOperationError> {
        let normalized = definition.admit_envelope()?;
        let evaluated = self.evaluate_analysis(
            document_scope,
            document,
            trusted_source,
            &normalized,
            principal,
            now,
        )?;
        Ok(AnalysisQueryResult {
            lineage: analysis_lineage(
                vec![source_context(document, trusted_source)],
                normalized,
                evaluated.formula_calculation_used,
            ),
            outcome: evaluated.outcome,
        })
    }

    /// Evaluate one normalized definition over two explicit exact contexts.
    ///
    /// The definition is normalized exactly once. Each independently bound
    /// lifecycle authorizes its context, then both halves of the combined
    /// result/lineage footprint are rechecked before one whole projection is
    /// returned. No history, diff, or resident revision lookup is performed.
    ///
    /// # Errors
    ///
    /// Any insufficient Query coverage produces one whole-operation
    /// disclosure-safe denial without returning a one-sided result.
    #[allow(clippy::too_many_arguments)]
    pub fn query_analysis_pair(
        &self,
        first_scope: &DocumentScopeId,
        first_document: &Document,
        first_source: (&SemanticRevision, ValidatorConfiguration),
        second_lifecycle: &PatchLifecycle,
        second_scope: &DocumentScopeId,
        second_document: &Document,
        second_source: (&SemanticRevision, ValidatorConfiguration),
        definition: &AnalysisDefinition,
        first_principal: &PrincipalId,
        second_principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<PairedAnalysisQueryResult, AnalysisOperationError> {
        let normalized = definition.admit_envelope()?;
        let first = self.evaluate_analysis(
            first_scope,
            first_document,
            first_source,
            &normalized,
            first_principal,
            now,
        )?;
        let second = second_lifecycle.evaluate_analysis(
            second_scope,
            second_document,
            second_source,
            &normalized,
            second_principal,
            now,
        )?;
        // A paired projection may span independently bound document
        // occurrences. Rechecking both complete halves is the cross-context
        // conjunction of the combined disclosure footprint; neither half is
        // projected unless both authorities still cover it.
        self.require_analysis_query(first_principal, &first.requirements, now)?;
        second_lifecycle.require_analysis_query(second_principal, &second.requirements, now)?;
        Ok(PairedAnalysisQueryResult {
            lineage: analysis_lineage(
                vec![
                    source_context(first_document, first_source),
                    source_context(second_document, second_source),
                ],
                normalized,
                first.formula_calculation_used || second.formula_calculation_used,
            ),
            first: first.outcome,
            second: second.outcome,
        })
    }

    fn evaluate_analysis(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        _trusted_source: (&SemanticRevision, ValidatorConfiguration),
        definition: &NormalizedAnalysisDefinition,
        principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<EvaluatedAnalysis, AnalysisOperationError> {
        self.require_document(document_scope, document)?;
        self.require_active_principal(principal)?;
        let prepared = self.prepare_analysis(document_scope, document, definition);
        self.require_analysis_query(principal, &prepared.requirements, now)?;

        let (outcome, formula_calculation_used) = if let Some(failure) = prepared.deferred_failure {
            (AnalysisOutcome::Failure(failure), false)
        } else {
            evaluate_authorized(document, definition, &prepared.candidates)
        };

        // This second check is the final complete-result disclosure gate. The
        // conservative preauthorization set is intentionally a safe superset
        // of every actual result/lineage fact this bounded slice can project.
        self.require_analysis_query(principal, &prepared.requirements, now)?;
        Ok(EvaluatedAnalysis {
            outcome,
            requirements: prepared.requirements,
            formula_calculation_used,
        })
    }

    fn prepare_analysis(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        definition: &NormalizedAnalysisDefinition,
    ) -> PreparedAnalysis {
        let document_requirement = analysis_document_requirement(document_scope, document);
        let mut requirements = BTreeSet::new();
        let (candidates, deferred_failure) = resolve_candidate_domain(
            document_scope,
            document,
            definition,
            &document_requirement,
            &mut requirements,
        );
        if document.schemas.contains_key(&definition.schema)
            && (definition.narrowing.is_none()
                || (candidates.is_empty() && deferred_failure.is_none()))
        {
            // An unbounded result asserts complete membership of the requested
            // schema domain. Grants over every currently discovered entity do
            // not authorize that schema-wide completeness claim.
            requirements.insert(analysis_schema_requirement(
                document_scope,
                document,
                &definition.schema,
            ));
        }

        let requested_fields = requested_fields(definition);
        let mut formula_roots = Vec::new();
        for entity_id in &candidates {
            let entity = &document.entities[entity_id];
            requirements.insert(analysis_entity_requirement(
                document_scope,
                document,
                entity_id,
                &entity.schema,
            ));
            for field in &requested_fields {
                let target = FieldRef::new(entity_id.clone(), field.clone());
                requirements.insert(self.field_scope(document, &target).map_or_else(
                    |_| document_requirement.clone(),
                    |scope| DisclosureRequirement {
                        family: OperationFamily::AnalysisQuery,
                        scope,
                    },
                ));
                if matches!(entity.fields.get(field), Some(Value::Formula(_)))
                    && is_calculated_field(definition, field)
                {
                    formula_roots.push(target);
                }
                if let Some(Value::Reference(reference)) = entity.fields.get(field) {
                    let scope = document.entities.get(reference).map_or_else(
                        || document_requirement.clone(),
                        |referenced| {
                            analysis_entity_requirement(
                                document_scope,
                                document,
                                &referenced.id,
                                &referenced.schema,
                            )
                        },
                    );
                    requirements.insert(scope);
                }
            }
        }

        insert_formula_dependency_requirements(
            self,
            document,
            &formula_roots,
            &document_requirement,
            &mut requirements,
        );
        if !formula_roots.is_empty() {
            // ADR-0018's current oracle is atomic across the complete
            // document. Any formula failure can therefore change the query
            // outcome even when it is outside the requested dependency
            // closure, so calculation requires complete document coverage.
            requirements.insert(document_requirement);
        }
        PreparedAnalysis {
            candidates,
            requirements,
            deferred_failure,
        }
    }

    fn require_analysis_query(
        &self,
        principal: &PrincipalId,
        requirements: &BTreeSet<DisclosureRequirement>,
        now: TrustedInstant,
    ) -> Result<(), AnalysisOperationError> {
        self.authorize_query(principal, requirements, now)
            .map(|_| ())
            .map_err(|error| match error {
                PatchLifecycleError::InsufficientCapability { .. } => {
                    AnalysisOperationError::Lifecycle(PatchLifecycleError::DisclosureDenied)
                }
                other => AnalysisOperationError::Lifecycle(other),
            })
    }
}

fn resolve_candidate_domain(
    document_scope: &DocumentScopeId,
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    document_requirement: &DisclosureRequirement,
    requirements: &mut BTreeSet<DisclosureRequirement>,
) -> (Vec<EntityId>, Option<AnalysisFailure>) {
    if !document.schemas.contains_key(&definition.schema) {
        requirements.insert(document_requirement.clone());
        return (
            Vec::new(),
            Some(AnalysisFailure::UnresolvedSchema {
                schema: definition.schema.clone(),
            }),
        );
    }
    let Some(narrowing) = &definition.narrowing else {
        let mut candidates = Vec::new();
        let mut deferred_failure = None;
        for (key, entity) in &document.entities {
            if entity.schema != definition.schema {
                continue;
            }
            if key == &entity.id {
                candidates.push(key.clone());
            } else {
                requirements.insert(document_requirement.clone());
                deferred_failure.get_or_insert_with(|| {
                    AnalysisFailure::IncoherentCandidateIdentity {
                        key: key.clone(),
                        entity: entity.id.clone(),
                    }
                });
            }
        }
        return (candidates, deferred_failure);
    };

    let mut candidates = Vec::new();
    let mut deferred_failure = None;
    for entity_id in narrowing {
        let Some(entity) = document.entities.get(entity_id) else {
            requirements.insert(document_requirement.clone());
            deferred_failure.get_or_insert_with(|| AnalysisFailure::UnresolvedNarrowingEntity {
                entity: entity_id.clone(),
            });
            continue;
        };
        if entity_id != &entity.id {
            requirements.insert(document_requirement.clone());
            deferred_failure.get_or_insert_with(|| AnalysisFailure::IncoherentCandidateIdentity {
                key: entity_id.clone(),
                entity: entity.id.clone(),
            });
            continue;
        }
        requirements.insert(analysis_entity_requirement(
            document_scope,
            document,
            &entity.id,
            &entity.schema,
        ));
        if entity.schema == definition.schema {
            candidates.push(entity.id.clone());
        } else {
            // Exposing the wrong-domain distinction reveals that the requested
            // schema exists. Require that schema's Query disclosure authority
            // in addition to the already-required actual entity authority.
            requirements.insert(analysis_schema_requirement(
                document_scope,
                document,
                &definition.schema,
            ));
            deferred_failure.get_or_insert_with(|| AnalysisFailure::WrongDomainNarrowingEntity {
                entity: entity.id.clone(),
                expected: definition.schema.clone(),
                actual: entity.schema.clone(),
            });
        }
    }
    (candidates, deferred_failure)
}

fn normalize_definition(
    definition: &AnalysisDefinition,
) -> Result<NormalizedAnalysisDefinition, AnalysisEnvelopeError> {
    if definition
        .narrowing
        .as_ref()
        .is_some_and(|ids| ids.len() > MAX_ANALYSIS_NARROWING_IDS)
    {
        return Err(AnalysisEnvelopeError::TooManyNarrowingIds);
    }
    if definition.predicates.len() > MAX_ANALYSIS_PREDICATES {
        return Err(AnalysisEnvelopeError::TooManyPredicates);
    }
    if definition.results.is_empty() {
        return Err(AnalysisEnvelopeError::MissingResults);
    }
    if definition.results.len() > MAX_ANALYSIS_RESULT_REQUESTS {
        return Err(AnalysisEnvelopeError::TooManyResults);
    }
    let mut predicates = definition.predicates.clone();
    predicates.sort();
    predicates.dedup();
    let mut results = definition.results.clone();
    results.sort();
    results.dedup();
    Ok(NormalizedAnalysisDefinition {
        schema: definition.schema.clone(),
        narrowing: definition
            .narrowing
            .as_ref()
            .map(|ids| ids.iter().cloned().collect()),
        predicates,
        group_by: definition.group_by.clone(),
        results,
    })
}

fn requested_fields(definition: &NormalizedAnalysisDefinition) -> BTreeSet<FieldId> {
    definition
        .predicates
        .iter()
        .map(|predicate| predicate.field.clone())
        .chain(definition.group_by.iter().cloned())
        .chain(
            definition
                .results
                .iter()
                .filter_map(|request| match request {
                    AnalysisResultRequest::Minimum(field)
                    | AnalysisResultRequest::Maximum(field)
                    | AnalysisResultRequest::Observations(field) => Some(field.clone()),
                    AnalysisResultRequest::Membership | AnalysisResultRequest::Count => None,
                }),
        )
        .collect()
}

fn is_calculated_field(definition: &NormalizedAnalysisDefinition, field: &FieldId) -> bool {
    definition
        .predicates
        .iter()
        .any(|predicate| predicate.field == *field)
        || definition.results.iter().any(|request| {
            matches!(
                request,
                AnalysisResultRequest::Minimum(metric)
                    | AnalysisResultRequest::Maximum(metric)
                    | AnalysisResultRequest::Observations(metric)
                    if metric == field
            )
        })
}

fn insert_formula_dependency_requirements(
    lifecycle: &PatchLifecycle,
    document: &Document,
    formula_roots: &[FieldRef],
    document_requirement: &DisclosureRequirement,
    requirements: &mut BTreeSet<DisclosureRequirement>,
) {
    let mut queue = VecDeque::from(formula_roots.to_vec());
    let mut visited = BTreeSet::new();
    while let Some(formula) = queue.pop_front() {
        if !visited.insert(formula.clone()) {
            continue;
        }
        let Some(Value::Formula(expression)) = document
            .entities
            .get(&formula.entity)
            .and_then(|entity| entity.fields.get(&formula.field))
        else {
            continue;
        };
        for dependency in extract_dependencies(expression) {
            requirements.insert(lifecycle.field_scope(document, &dependency).map_or_else(
                |_| document_requirement.clone(),
                |scope| DisclosureRequirement {
                    family: OperationFamily::AnalysisQuery,
                    scope,
                },
            ));
            if matches!(
                document
                    .entities
                    .get(&dependency.entity)
                    .and_then(|entity| entity.fields.get(&dependency.field)),
                Some(Value::Formula(_))
            ) {
                queue.push_back(dependency);
            }
        }
    }
}

fn evaluate_authorized(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    candidates: &[EntityId],
) -> (AnalysisOutcome, bool) {
    if let Some(failure) = validate_targets(document, definition) {
        return (AnalysisOutcome::Failure(failure), false);
    }

    let formula_calculation_used = candidates.iter().any(|entity_id| {
        requested_fields(definition).iter().any(|field| {
            document.entities.get(entity_id).is_some_and(|entity| {
                matches!(entity.fields.get(field), Some(Value::Formula(_)))
                    && is_calculated_field(definition, field)
            })
        })
    });
    let calculation = formula_calculation_used.then(|| calculate_complete(document));
    let requested_failure =
        requested_formula_failure(document, definition, candidates, calculation.as_ref());
    if let Err(failure) = validate_formula_predicates(
        document,
        definition,
        candidates,
        calculation.as_ref(),
        requested_failure.as_ref(),
    ) {
        return (AnalysisOutcome::Failure(failure), formula_calculation_used);
    }
    let selected = match select_entities(
        document,
        definition,
        candidates,
        calculation.as_ref(),
        requested_failure.as_ref(),
    ) {
        Ok(selected) => selected,
        Err(failure) => return (AnalysisOutcome::Failure(failure), formula_calculation_used),
    };
    let groups = match group_entities(document, definition, &selected) {
        Ok(groups) => groups,
        Err(failure) => return (AnalysisOutcome::Failure(failure), formula_calculation_used),
    };
    let metrics = match collect_metrics(
        document,
        definition,
        &selected,
        calculation.as_ref(),
        requested_failure.as_ref(),
    ) {
        Ok(metrics) => metrics,
        Err(failure) => return (AnalysisOutcome::Failure(failure), formula_calculation_used),
    };
    if let Some(failure) = collection_limit_failure(definition, &selected, &groups) {
        return (AnalysisOutcome::Failure(failure), formula_calculation_used);
    }
    let projection = if definition.group_by.is_some() {
        AnalysisProjection::Grouped(
            groups
                .into_iter()
                .map(|(key, members)| AnalysisGroup {
                    key,
                    bucket: build_bucket(definition, &members, &metrics),
                })
                .collect(),
        )
    } else {
        AnalysisProjection::Ungrouped(build_bucket(definition, &selected, &metrics))
    };
    (
        AnalysisOutcome::Complete(projection),
        formula_calculation_used,
    )
}

fn requested_formula_failure(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    candidates: &[EntityId],
    calculation: Option<&CalculationOutcome>,
) -> Option<(FieldRef, CalculationFailure)> {
    let Some(CalculationOutcome::Failed(failures)) = calculation else {
        return None;
    };
    let fields = requested_fields(definition);
    candidates.iter().find_map(|entity| {
        fields.iter().find_map(|field| {
            let target = FieldRef::new(entity.clone(), field.clone());
            (is_calculated_field(definition, field)
                && matches!(
                    document.entities[entity].fields.get(field),
                    Some(Value::Formula(_))
                ))
            .then(|| {
                failures
                    .failures()
                    .get(&target)
                    .cloned()
                    .map(|failure| (target, failure))
            })
            .flatten()
        })
    })
}

fn validate_targets(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
) -> Option<AnalysisFailure> {
    let schema = document.schemas.get(&definition.schema)?;
    for predicate in &definition.predicates {
        let Some(field) = schema.fields.get(&predicate.field) else {
            return Some(AnalysisFailure::UnresolvedField {
                role: AnalysisFieldRole::Predicate,
                field: predicate.field.clone(),
            });
        };
        if !predicate_is_typed(field.field_type.clone(), predicate) {
            return Some(AnalysisFailure::InvalidPredicateType {
                field: predicate.field.clone(),
                declared: field.field_type.clone(),
            });
        }
    }
    if let Some(group) = &definition.group_by {
        if !schema.fields.contains_key(group) {
            return Some(AnalysisFailure::UnresolvedField {
                role: AnalysisFieldRole::Group,
                field: group.clone(),
            });
        }
    }
    for request in &definition.results {
        let field = match request {
            AnalysisResultRequest::Minimum(field)
            | AnalysisResultRequest::Maximum(field)
            | AnalysisResultRequest::Observations(field) => field,
            AnalysisResultRequest::Membership | AnalysisResultRequest::Count => continue,
        };
        let Some(definition) = schema.fields.get(field) else {
            return Some(AnalysisFailure::UnresolvedField {
                role: AnalysisFieldRole::Metric,
                field: field.clone(),
            });
        };
        if definition.field_type != FieldType::Number {
            return Some(AnalysisFailure::InvalidMetricType {
                field: field.clone(),
                declared: definition.field_type.clone(),
            });
        }
    }
    None
}

fn predicate_is_typed(field_type: FieldType, predicate: &AnalysisPredicate) -> bool {
    let operand_matches = matches!(
        (field_type, &predicate.operand),
        (FieldType::Number, PredicateOperand::Number(_))
            | (FieldType::Text, PredicateOperand::Text(_))
            | (FieldType::Boolean, PredicateOperand::Boolean(_))
            | (FieldType::Reference { .. }, PredicateOperand::Reference(_))
    );
    operand_matches
        && (matches!(
            predicate.operator,
            AnalysisPredicateOperator::Equal | AnalysisPredicateOperator::NotEqual
        ) || matches!(predicate.operand, PredicateOperand::Number(_)))
}

fn select_entities(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    candidates: &[EntityId],
    calculation: Option<&CalculationOutcome>,
    requested_failure: Option<&(FieldRef, CalculationFailure)>,
) -> Result<Vec<EntityId>, AnalysisFailure> {
    let schema = &document.schemas[&definition.schema];
    let mut selected = Vec::new();
    for entity_id in candidates {
        let entity = &document.entities[entity_id];
        let mut matched = true;
        for predicate in &definition.predicates {
            let Some(value) = entity.fields.get(&predicate.field) else {
                matched = false;
                break;
            };
            if let (FieldType::Reference { schema }, Value::Reference(reference)) =
                (&schema.fields[&predicate.field].field_type, value)
            {
                if !reference_value_is_typed(document, schema, reference) {
                    return Err(AnalysisFailure::InvalidPredicateValue {
                        entity: entity_id.clone(),
                        field: predicate.field.clone(),
                        actual: AnalysisValueKind::Reference,
                    });
                }
            }
            let effective = effective_predicate_value(
                entity_id,
                &predicate.field,
                value,
                calculation,
                requested_failure,
            )?;
            if !operand_kinds_match(&effective, &predicate.operand) {
                return Err(AnalysisFailure::InvalidPredicateValue {
                    entity: entity_id.clone(),
                    field: predicate.field.clone(),
                    actual: value_kind(value),
                });
            }
            if !predicate_matches(&effective, predicate) {
                matched = false;
                break;
            }
        }
        if matched {
            selected.push(entity_id.clone());
        }
    }
    Ok(selected)
}

fn validate_formula_predicates(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    candidates: &[EntityId],
    calculation: Option<&CalculationOutcome>,
    requested_failure: Option<&(FieldRef, CalculationFailure)>,
) -> Result<(), AnalysisFailure> {
    let schema = &document.schemas[&definition.schema];
    for entity_id in candidates {
        let entity = &document.entities[entity_id];
        for predicate in &definition.predicates {
            if schema.fields[&predicate.field].field_type == FieldType::Number
                && matches!(entity.fields.get(&predicate.field), Some(Value::Formula(_)))
            {
                effective_formula_number(
                    &FieldRef::new(entity_id.clone(), predicate.field.clone()),
                    calculation,
                    requested_failure,
                )?;
            }
        }
    }
    Ok(())
}

fn effective_predicate_value(
    entity: &EntityId,
    field: &FieldId,
    value: &Value,
    calculation: Option<&CalculationOutcome>,
    requested_failure: Option<&(FieldRef, CalculationFailure)>,
) -> Result<PredicateOperand, AnalysisFailure> {
    match value {
        Value::Number(value) => Ok(PredicateOperand::Number(*value)),
        Value::Text(value) => Ok(PredicateOperand::Text(value.clone())),
        Value::Boolean(value) => Ok(PredicateOperand::Boolean(*value)),
        Value::Reference(value) => Ok(PredicateOperand::Reference(value.clone())),
        Value::Formula(_) => effective_formula_number(
            &FieldRef::new(entity.clone(), field.clone()),
            calculation,
            requested_failure,
        )
        .map(PredicateOperand::Number),
    }
}

const fn operand_kinds_match(left: &PredicateOperand, right: &PredicateOperand) -> bool {
    matches!(
        (left, right),
        (PredicateOperand::Number(_), PredicateOperand::Number(_))
            | (PredicateOperand::Text(_), PredicateOperand::Text(_))
            | (PredicateOperand::Boolean(_), PredicateOperand::Boolean(_))
            | (
                PredicateOperand::Reference(_),
                PredicateOperand::Reference(_)
            )
    )
}

fn predicate_matches(effective: &PredicateOperand, predicate: &AnalysisPredicate) -> bool {
    match predicate.operator {
        AnalysisPredicateOperator::Equal => effective == &predicate.operand,
        AnalysisPredicateOperator::NotEqual => effective != &predicate.operand,
        AnalysisPredicateOperator::LessThan => effective < &predicate.operand,
        AnalysisPredicateOperator::LessThanOrEqual => effective <= &predicate.operand,
        AnalysisPredicateOperator::GreaterThan => effective > &predicate.operand,
        AnalysisPredicateOperator::GreaterThanOrEqual => effective >= &predicate.operand,
    }
}

fn group_entities(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    selected: &[EntityId],
) -> Result<BTreeMap<AnalysisGroupKey, Vec<EntityId>>, AnalysisFailure> {
    let Some(field) = &definition.group_by else {
        return Ok(BTreeMap::new());
    };
    let mut groups = BTreeMap::<AnalysisGroupKey, Vec<EntityId>>::new();
    for entity_id in selected {
        let entity = &document.entities[entity_id];
        let declared = &document.schemas[&definition.schema].fields[field].field_type;
        let value = entity
            .fields
            .get(field)
            .ok_or_else(|| AnalysisFailure::MissingGroupValue {
                entity: entity_id.clone(),
                field: field.clone(),
            })?;
        let key = match (declared, value) {
            (_, Value::Formula(_)) => {
                return Err(AnalysisFailure::FormulaGroupingUnsupported {
                    field: field.clone(),
                });
            }
            (FieldType::Number, Value::Number(value)) => AnalysisGroupKey::Number(*value),
            (FieldType::Text, Value::Text(value)) => AnalysisGroupKey::Text(value.clone()),
            (FieldType::Boolean, Value::Boolean(value)) => AnalysisGroupKey::Boolean(*value),
            (FieldType::Reference { schema }, Value::Reference(value)) => {
                if !reference_value_is_typed(document, schema, value) {
                    return Err(AnalysisFailure::InvalidGroupValue {
                        entity: entity_id.clone(),
                        field: field.clone(),
                        actual: AnalysisValueKind::Reference,
                    });
                }
                AnalysisGroupKey::Reference(value.clone())
            }
            (_, other) => {
                return Err(AnalysisFailure::InvalidGroupValue {
                    entity: entity_id.clone(),
                    field: field.clone(),
                    actual: value_kind(other),
                });
            }
        };
        groups.entry(key).or_default().push(entity_id.clone());
    }
    Ok(groups)
}

fn reference_value_is_typed(
    document: &Document,
    expected_schema: &SchemaId,
    reference: &EntityId,
) -> bool {
    document
        .entities
        .get(reference)
        .is_some_and(|entity| &entity.schema == expected_schema)
}

fn collect_metrics(
    document: &Document,
    definition: &NormalizedAnalysisDefinition,
    selected: &[EntityId],
    calculation: Option<&CalculationOutcome>,
    requested_failure: Option<&(FieldRef, CalculationFailure)>,
) -> Result<BTreeMap<FieldId, BTreeMap<EntityId, Number>>, AnalysisFailure> {
    let fields = definition
        .results
        .iter()
        .filter_map(|request| match request {
            AnalysisResultRequest::Minimum(field)
            | AnalysisResultRequest::Maximum(field)
            | AnalysisResultRequest::Observations(field) => Some(field.clone()),
            AnalysisResultRequest::Membership | AnalysisResultRequest::Count => None,
        })
        .collect::<BTreeSet<_>>();
    let mut metrics = BTreeMap::new();
    for field in fields {
        let mut values = BTreeMap::new();
        for entity_id in selected {
            let entity = &document.entities[entity_id];
            let value =
                entity
                    .fields
                    .get(&field)
                    .ok_or_else(|| AnalysisFailure::MetricIncomplete {
                        entity: entity_id.clone(),
                        field: field.clone(),
                        reason: MetricIncompleteReason::Missing,
                    })?;
            let number = match value {
                Value::Number(number) => *number,
                Value::Formula(_) => effective_formula_number(
                    &FieldRef::new(entity_id.clone(), field.clone()),
                    calculation,
                    requested_failure,
                )?,
                other => {
                    return Err(AnalysisFailure::MetricIncomplete {
                        entity: entity_id.clone(),
                        field: field.clone(),
                        reason: MetricIncompleteReason::WrongKind(value_kind(other)),
                    });
                }
            };
            values.insert(entity_id.clone(), number);
        }
        metrics.insert(field, values);
    }
    Ok(metrics)
}

fn effective_formula_number(
    field: &FieldRef,
    calculation: Option<&CalculationOutcome>,
    requested_failure: Option<&(FieldRef, CalculationFailure)>,
) -> Result<Number, AnalysisFailure> {
    match calculation {
        Some(CalculationOutcome::Complete(calculation)) => {
            calculation
                .value(field)
                .ok_or_else(|| AnalysisFailure::CalculationFailed {
                    field: field.clone(),
                    failure: None,
                })
        }
        Some(CalculationOutcome::Failed(failures)) => {
            let exact = failures
                .failures()
                .get_key_value(field)
                .map(|(field, failure)| (field.clone(), failure.clone()));
            let (field, failure) = exact
                .or_else(|| requested_failure.cloned())
                .map_or((field.clone(), None), |(field, failure)| {
                    (field, Some(failure))
                });
            Err(AnalysisFailure::CalculationFailed { field, failure })
        }
        None => Err(AnalysisFailure::CalculationFailed {
            field: field.clone(),
            failure: None,
        }),
    }
}

fn collection_limit_failure(
    definition: &NormalizedAnalysisDefinition,
    selected: &[EntityId],
    groups: &BTreeMap<AnalysisGroupKey, Vec<EntityId>>,
) -> Option<AnalysisFailure> {
    if definition.group_by.is_some() && groups.len() > MAX_ANALYSIS_COLLECTION_RESULTS {
        return Some(AnalysisFailure::ResultTooLarge {
            collection: AnalysisCollectionKind::Groups,
            limit: MAX_ANALYSIS_COLLECTION_RESULTS,
        });
    }
    if selected.len() > MAX_ANALYSIS_COLLECTION_RESULTS
        && definition
            .results
            .contains(&AnalysisResultRequest::Membership)
    {
        return Some(AnalysisFailure::ResultTooLarge {
            collection: AnalysisCollectionKind::Membership,
            limit: MAX_ANALYSIS_COLLECTION_RESULTS,
        });
    }
    if selected.len() > MAX_ANALYSIS_COLLECTION_RESULTS
        && definition
            .results
            .iter()
            .any(|request| matches!(request, AnalysisResultRequest::Observations(_)))
    {
        return Some(AnalysisFailure::ResultTooLarge {
            collection: AnalysisCollectionKind::Observations,
            limit: MAX_ANALYSIS_COLLECTION_RESULTS,
        });
    }
    None
}

fn build_bucket(
    definition: &NormalizedAnalysisDefinition,
    members: &[EntityId],
    metrics: &BTreeMap<FieldId, BTreeMap<EntityId, Number>>,
) -> AnalysisBucket {
    let values = definition
        .results
        .iter()
        .map(|request| match request {
            AnalysisResultRequest::Membership => AnalysisResultValue::Membership(members.to_vec()),
            AnalysisResultRequest::Count => AnalysisResultValue::Count(members.len() as u64),
            AnalysisResultRequest::Minimum(field) => AnalysisResultValue::Minimum {
                field: field.clone(),
                outcome: aggregate(members, &metrics[field], true),
            },
            AnalysisResultRequest::Maximum(field) => AnalysisResultValue::Maximum {
                field: field.clone(),
                outcome: aggregate(members, &metrics[field], false),
            },
            AnalysisResultRequest::Observations(field) => AnalysisResultValue::Observations {
                field: field.clone(),
                values: members
                    .iter()
                    .map(|entity| (entity.clone(), metrics[field][entity]))
                    .collect(),
            },
        })
        .collect();
    AnalysisBucket { values }
}

fn aggregate(
    members: &[EntityId],
    values: &BTreeMap<EntityId, Number>,
    minimum: bool,
) -> NumericAggregateOutcome {
    let selected = members.iter().map(|entity| values[entity]);
    let value = if minimum {
        selected.min()
    } else {
        selected.max()
    };
    value.map_or(
        NumericAggregateOutcome::Empty,
        NumericAggregateOutcome::Value,
    )
}

fn analysis_document_requirement(
    document_scope: &DocumentScopeId,
    document: &Document,
) -> DisclosureRequirement {
    DisclosureRequirement {
        family: OperationFamily::AnalysisQuery,
        scope: ScopedSemanticSubject::new(
            document_scope.clone(),
            document.id.clone(),
            SemanticScope::Document,
        ),
    }
}

fn analysis_entity_requirement(
    document_scope: &DocumentScopeId,
    document: &Document,
    entity: &EntityId,
    schema: &SchemaId,
) -> DisclosureRequirement {
    DisclosureRequirement {
        family: OperationFamily::AnalysisQuery,
        scope: ScopedSemanticSubject::new(
            document_scope.clone(),
            document.id.clone(),
            SemanticScope::Entity {
                entity: entity.clone(),
                schema: schema.clone(),
            },
        ),
    }
}

fn analysis_schema_requirement(
    document_scope: &DocumentScopeId,
    document: &Document,
    schema: &SchemaId,
) -> DisclosureRequirement {
    DisclosureRequirement {
        family: OperationFamily::AnalysisQuery,
        scope: ScopedSemanticSubject::new(
            document_scope.clone(),
            document.id.clone(),
            SemanticScope::Schema(schema.clone()),
        ),
    }
}

fn source_context(
    document: &Document,
    trusted_source: (&SemanticRevision, ValidatorConfiguration),
) -> AnalysisSourceContext {
    AnalysisSourceContext {
        document: document.id.clone(),
        source_revision: trusted_source.0.clone(),
        validator_configuration: trusted_source.1,
    }
}

fn analysis_lineage(
    sources: Vec<AnalysisSourceContext>,
    normalized_definition: NormalizedAnalysisDefinition,
    formula_calculation_used: bool,
) -> AnalysisLineage {
    let mut derivations = normalized_definition
        .predicates
        .iter()
        .map(|predicate| AnalysisDerivation::Predicate(predicate.field.clone()))
        .collect::<Vec<_>>();
    if let Some(field) = &normalized_definition.group_by {
        derivations.push(AnalysisDerivation::GroupedBy(field.clone()));
    }
    derivations.extend(
        normalized_definition
            .results
            .iter()
            .map(|request| match request {
                AnalysisResultRequest::Membership => AnalysisDerivation::Membership,
                AnalysisResultRequest::Count => AnalysisDerivation::Count,
                AnalysisResultRequest::Minimum(field) => AnalysisDerivation::Minimum(field.clone()),
                AnalysisResultRequest::Maximum(field) => AnalysisDerivation::Maximum(field.clone()),
                AnalysisResultRequest::Observations(field) => {
                    AnalysisDerivation::Observations(field.clone())
                }
            }),
    );
    AnalysisLineage {
        sources,
        normalized_definition,
        formula_calculation_used,
        derivations,
    }
}

const fn value_kind(value: &Value) -> AnalysisValueKind {
    match value {
        Value::Number(_) => AnalysisValueKind::Number,
        Value::Formula(_) => AnalysisValueKind::Formula,
        Value::Text(_) => AnalysisValueKind::Text,
        Value::Boolean(_) => AnalysisValueKind::Boolean,
        Value::Reference(_) => AnalysisValueKind::Reference,
    }
}
