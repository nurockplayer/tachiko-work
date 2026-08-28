//! Provisional M04 formula Query, scenario Query, and `FormulaUpdate` mapping.
//!
//! These Rust shapes are in-process implementation details. They preserve the
//! Accepted logical behavior without defining a public wire or SDK contract.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::patch_lifecycle::{
    DisclosureRequirement, DocumentScopeId, FormulaUpdateCommand, OperationFamily, PatchLifecycle,
    PatchLifecycleError, PrincipalId, ProposalId, ProposalRequest, ScopedSemanticSubject,
    SemanticCommand, SemanticPatch, SemanticPatchBody, SemanticRevision, SemanticScope,
    TrustedInstant,
};
use crate::{
    CalculationFailure, CalculationOutcome, Document, DocumentId, Expression, FieldRef, Number,
    SemanticChange, ValidationReport, Value, WorkspaceError, bind_formula_update_unbound,
    calculate_complete, validation_report,
};

/// Current finite scenario envelope profile. The exact threshold is
/// intentionally Provisional under the M04 contract.
pub const MAX_SCENARIO_OVERRIDES: usize = 64;

/// Current finite requested-target profile. The exact threshold is
/// intentionally Provisional under the M04 contract.
pub const MAX_SCENARIO_TARGETS: usize = 64;

/// Pinned deterministic validator configuration supported by this slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidatorConfiguration {
    WorkspaceFull,
}

/// Exact source-revision and validator evidence supplied by the trusted host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormulaQueryContext {
    source_revision: SemanticRevision,
    validator_configuration: ValidatorConfiguration,
}

impl FormulaQueryContext {
    fn trusted(
        source_revision: SemanticRevision,
        validator_configuration: ValidatorConfiguration,
    ) -> Self {
        Self {
            source_revision,
            validator_configuration,
        }
    }

    #[must_use]
    pub fn source_revision(&self) -> &SemanticRevision {
        &self.source_revision
    }

    #[must_use]
    pub const fn validator_configuration(&self) -> ValidatorConfiguration {
        self.validator_configuration
    }
}

/// One typed formula calculation observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormulaCalculationOutcome {
    Value(Number),
    Failure(CalculationFailure),
    Unavailable,
}

/// Structured formula meaning and engine-derived reasoning facts.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaReasoningFacts {
    pub target: FieldRef,
    pub expression: Expression,
    pub direct_inputs: Vec<FieldRef>,
    pub direct_dependents: Vec<FieldRef>,
    pub affected_subjects: Vec<FieldRef>,
    pub calculation: FormulaCalculationOutcome,
    /// A complete report is projected only with document-wide Query coverage.
    pub validation_report: Option<ValidationReport>,
}

/// One formula-reasoning target outcome from the exact source snapshot.
#[derive(Clone, Debug, PartialEq)]
pub enum FormulaReasoningOutcome {
    Formula(FormulaReasoningFacts),
    UnresolvedTarget {
        target: FieldRef,
    },
    UnsupportedKind {
        target: FieldRef,
        actual: SemanticValueKind,
    },
}

/// Formula-reasoning result with exact source evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaReasoningResult {
    pub document: DocumentId,
    pub context: FormulaQueryContext,
    pub outcome: FormulaReasoningOutcome,
}

/// Request-local override representation admitted into an ADR-0018 Number.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberOverride {
    pub target: FieldRef,
    pub value: f64,
}

impl NumberOverride {
    #[must_use]
    pub fn new(target: FieldRef, value: f64) -> Self {
        Self { target, value }
    }
}

/// One normalized typed Number override preserving request order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedNumberOverride {
    pub target: FieldRef,
    pub value: Number,
}

/// Exact-snapshot scenario request. Membership is deliberately absent.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioRequest {
    pub overrides: Vec<NumberOverride>,
    pub requested_targets: Vec<FieldRef>,
}

impl ScenarioRequest {
    #[must_use]
    pub fn new(overrides: Vec<NumberOverride>, requested_targets: Vec<FieldRef>) -> Self {
        Self {
            overrides,
            requested_targets,
        }
    }
}

/// Disclosure-independent request-local scenario admission failure.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ScenarioEnvelopeError {
    #[error("scenario has too many overrides")]
    TooManyOverrides,
    #[error("scenario has too many requested targets")]
    TooManyTargets,
    #[error("scenario override target '{target}' occurs more than once")]
    DuplicateOverride { target: FieldRef },
    #[error("scenario override for '{target}' is not a finite Number")]
    NonFiniteOverride { target: FieldRef },
}

/// Formula/scenario boundary error before a safe structured semantic result.
#[derive(Debug, Error)]
pub enum FormulaOperationError {
    #[error(transparent)]
    ScenarioEnvelope(#[from] ScenarioEnvelopeError),
    #[error(transparent)]
    Lifecycle(#[from] PatchLifecycleError),
}

/// Existing semantic value classification for structured unsupported outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticValueKind {
    Number,
    Formula,
    Text,
    Boolean,
    Reference,
}

/// Authorized semantic classification failure for one override.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenarioOverrideFailure {
    UnresolvedTarget {
        target: FieldRef,
    },
    UnsupportedKind {
        target: FieldRef,
        actual: SemanticValueKind,
    },
}

/// Formula comparison for one supported requested target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioFormulaComparison {
    pub expression: Expression,
    pub direct_inputs: Vec<FieldRef>,
    pub direct_dependents: Vec<FieldRef>,
    pub baseline: FormulaCalculationOutcome,
    pub candidate: FormulaCalculationOutcome,
}

/// Exactly one disclosure-safe outcome for one normalized requested target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenarioTargetOutcome {
    Formula(ScenarioFormulaComparison),
    UnresolvedTarget,
    UnsupportedKind { actual: SemanticValueKind },
    DisclosureDenied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioTargetResult {
    pub target: FieldRef,
    pub outcome: ScenarioTargetOutcome,
}

/// Existing diff-derived scenario impact. Absence never claims an empty set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioImpact {
    pub changed_fields: Vec<FieldRef>,
    pub affected_fields: Vec<FieldRef>,
}

/// Authoritative baseline/candidate evaluation over one transient candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioEvaluation {
    /// Complete reports are projected only with document-wide Query coverage.
    pub baseline_validation: Option<ValidationReport>,
    pub candidate_validation: Option<ValidationReport>,
    pub impact: Option<ScenarioImpact>,
    pub targets: Vec<ScenarioTargetResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScenarioOutcome {
    InvalidOverrides(Vec<ScenarioOverrideFailure>),
    Evaluated(ScenarioEvaluation),
}

/// Scenario result preserving exact source and normalized override evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioResult {
    pub document: DocumentId,
    pub context: FormulaQueryContext,
    pub normalized_overrides: Vec<NormalizedNumberOverride>,
    pub outcome: ScenarioOutcome,
}

struct ScenarioTargetProjection<'a> {
    document_scope: &'a DocumentScopeId,
    document: &'a Document,
    principal: &'a PrincipalId,
    now: TrustedInstant,
    baseline: &'a CalculationOutcome,
    candidate: &'a CalculationOutcome,
    dependencies: &'a BTreeMap<FieldRef, BTreeSet<FieldRef>>,
}

/// Formula authoring request admitted before the immutable proposal occurrence.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaUpdateRequest {
    proposal_id: ProposalId,
    base_revision: SemanticRevision,
    target: FieldRef,
    source: String,
    originator: PrincipalId,
}

impl FormulaUpdateRequest {
    #[must_use]
    pub fn new(
        proposal_id: ProposalId,
        base_revision: SemanticRevision,
        target: FieldRef,
        source: impl Into<String>,
        originator: PrincipalId,
    ) -> Self {
        Self {
            proposal_id,
            base_revision,
            target,
            source: source.into(),
            originator,
        }
    }
}

impl PatchLifecycle {
    /// Query one formula through existing formula, validation, and Grant authority.
    ///
    /// # Errors
    ///
    /// Returns a disclosure-safe lifecycle denial when the protected occurrence,
    /// principal, or complete result footprint is not authorized.
    pub fn query_formula_reasoning(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        trusted_source: (&SemanticRevision, ValidatorConfiguration),
        target: &FieldRef,
        principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<FormulaReasoningResult, FormulaOperationError> {
        self.require_document(document_scope, document)?;
        self.require_active_principal(principal)?;
        let (current_revision, validator_configuration) = trusted_source;
        let context =
            FormulaQueryContext::trusted(current_revision.clone(), validator_configuration);

        let Some(value) = field_value(document, target) else {
            let requirements =
                document_requirements(document_scope, document, OperationFamily::FormulaReasoning);
            self.require_query_projection(principal, &requirements, now)?;
            return Ok(FormulaReasoningResult {
                document: document.id.clone(),
                context,
                outcome: FormulaReasoningOutcome::UnresolvedTarget {
                    target: target.clone(),
                },
            });
        };

        let mut requirements = BTreeSet::from([self
            .field_requirement(document, OperationFamily::FormulaReasoning, target)
            .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?]);
        let outcome = match value {
            Value::Formula(expression) => {
                let calculation = calculate_complete(document);
                let dependencies = calculation_dependencies(&calculation);
                let direct_inputs = dependencies
                    .get(target)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<Vec<_>>();
                let direct_dependents = direct_dependents(dependencies, target);
                let affected_subjects = affected_by(dependencies, target);
                for field in direct_inputs
                    .iter()
                    .chain(&direct_dependents)
                    .chain(&affected_subjects)
                {
                    requirements.insert(self.field_or_document_requirement(
                        document_scope,
                        document,
                        OperationFamily::FormulaReasoning,
                        field,
                    ));
                }
                let calculation = calculation_for(&calculation, target);
                for field in calculation_evidence_subjects(&calculation) {
                    requirements.insert(self.field_or_document_requirement(
                        document_scope,
                        document,
                        OperationFamily::FormulaReasoning,
                        &field,
                    ));
                }
                self.require_query_projection(principal, &requirements, now)?;
                let report = validation_report(document);
                let complete_validation = self.complete_validation_projection(
                    document_scope,
                    document,
                    OperationFamily::FormulaReasoning,
                    principal,
                    now,
                    report,
                );
                FormulaReasoningOutcome::Formula(FormulaReasoningFacts {
                    target: target.clone(),
                    expression: expression.clone(),
                    direct_inputs,
                    direct_dependents,
                    affected_subjects,
                    calculation,
                    validation_report: complete_validation,
                })
            }
            other => {
                self.require_query_projection(principal, &requirements, now)?;
                FormulaReasoningOutcome::UnsupportedKind {
                    target: target.clone(),
                    actual: value_kind(other),
                }
            }
        };

        Ok(FormulaReasoningResult {
            document: document.id.clone(),
            context,
            outcome,
        })
    }

    /// Evaluate one bounded Number-override scenario on one transient candidate.
    ///
    /// # Errors
    ///
    /// Envelope defects fail before document occurrence or semantic lookup.
    /// Authorization failures expose no semantic classification.
    pub fn query_number_override_scenario(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        trusted_source: (&SemanticRevision, ValidatorConfiguration),
        request: &ScenarioRequest,
        principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<ScenarioResult, FormulaOperationError> {
        let (normalized_overrides, requested_targets) = admit_scenario_envelope(request)?;
        self.require_document(document_scope, document)?;
        self.require_active_principal(principal)?;
        let (current_revision, validator_configuration) = trusted_source;
        let context =
            FormulaQueryContext::trusted(current_revision.clone(), validator_configuration);

        let override_failures = self.authorize_and_classify_overrides(
            document_scope,
            document,
            &normalized_overrides,
            principal,
            now,
        )?;
        if !override_failures.is_empty() {
            return Ok(ScenarioResult {
                document: document.id.clone(),
                context,
                normalized_overrides,
                outcome: ScenarioOutcome::InvalidOverrides(override_failures),
            });
        }

        let candidate = scenario_candidate(document, &normalized_overrides)
            .map_err(|()| PatchLifecycleError::ScopeDerivationFailed)?;

        let baseline_calculation = calculate_complete(document);
        let candidate_calculation = calculate_complete(&candidate);
        let baseline_report = validation_report(document);
        let candidate_report = validation_report(&candidate);
        let mut impact = scenario_impact(document, &candidate);
        if let Some(projected) = &impact {
            let requirements = projected
                .changed_fields
                .iter()
                .chain(&projected.affected_fields)
                .map(|field| {
                    self.field_requirement(document, OperationFamily::NumberOverrideScenario, field)
                })
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?;
            if self.authorize_query(principal, &requirements, now).is_err() {
                impact = None;
            }
        }

        let baseline_validation = self.complete_validation_projection(
            document_scope,
            document,
            OperationFamily::NumberOverrideScenario,
            principal,
            now,
            baseline_report,
        );
        let candidate_validation = self.complete_validation_projection(
            document_scope,
            document,
            OperationFamily::NumberOverrideScenario,
            principal,
            now,
            candidate_report,
        );
        let target_projection = ScenarioTargetProjection {
            document_scope,
            document,
            principal,
            now,
            baseline: &baseline_calculation,
            candidate: &candidate_calculation,
            dependencies: calculation_dependencies(&baseline_calculation),
        };
        let targets = requested_targets
            .into_iter()
            .map(|target| ScenarioTargetResult {
                outcome: self.scenario_target_outcome(&target_projection, &target),
                target,
            })
            .collect::<Vec<_>>();

        Ok(ScenarioResult {
            document: document.id.clone(),
            context,
            normalized_overrides,
            outcome: ScenarioOutcome::Evaluated(ScenarioEvaluation {
                baseline_validation,
                candidate_validation,
                impact,
                targets,
            }),
        })
    }

    /// Parse, bind, type-check, authorize, and propose one `FormulaUpdate`.
    ///
    /// Request-local syntax failure issues no proposal. Base-dependent target
    /// or binding evidence is returned only with live `FormulaUpdate` Query
    /// coverage. Successful admission delegates to the existing proposal path.
    ///
    /// # Errors
    ///
    /// Returns the existing lifecycle/admission family without publication.
    pub fn propose_formula_update(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        request: FormulaUpdateRequest,
        now: TrustedInstant,
    ) -> Result<SemanticPatch, PatchLifecycleError> {
        let unbound =
            tachiko_formula_engine::parse_expression(&request.source).map_err(|source| {
                PatchLifecycleError::CommandRejected {
                    source: Box::new(WorkspaceError::InvalidFormula {
                        field: request.target.clone(),
                        source,
                    }),
                }
            })?;
        self.require_document(document_scope, document)?;
        self.require_active_principal(&request.originator)?;
        if request.base_revision != *current_revision {
            let disclosure =
                document_requirements(document_scope, document, OperationFamily::FormulaUpdate);
            return if self
                .authorize_query(&request.originator, &disclosure, now)
                .is_ok()
            {
                Err(PatchLifecycleError::Stale)
            } else {
                Err(PatchLifecycleError::AuthorizationDenied)
            };
        }

        let expression = match bind_formula_update_unbound(document, &request.target, &unbound) {
            Ok(expression) => expression,
            Err(source) => {
                let disclosure =
                    document_requirements(document_scope, document, OperationFamily::FormulaUpdate);
                self.require_query_for_admission(&request.originator, &disclosure, now)?;
                return Err(PatchLifecycleError::CommandRejected {
                    source: Box::new(source),
                });
            }
        };
        let command = FormulaUpdateCommand::new(request.target, expression);
        let mut disclosure = BTreeSet::new();
        for field in std::iter::once(command.target()).chain(command.references()) {
            disclosure.insert(
                self.field_requirement(document, OperationFamily::FormulaUpdate, field)
                    .unwrap_or_else(|_| {
                        document_requirement(
                            document_scope,
                            document,
                            OperationFamily::FormulaUpdate,
                        )
                    }),
            );
        }
        self.require_query_for_admission(&request.originator, &disclosure, now)?;

        self.propose(
            document_scope,
            document,
            current_revision,
            ProposalRequest::new(
                request.proposal_id,
                request.base_revision,
                SemanticPatchBody::command(SemanticCommand::FormulaUpdate(command)),
                request.originator,
            ),
            now,
        )
    }

    fn authorize_and_classify_overrides(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        overrides: &[NormalizedNumberOverride],
        principal: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<Vec<ScenarioOverrideFailure>, FormulaOperationError> {
        let requirements = overrides
            .iter()
            .map(|overrode| {
                self.field_requirement(
                    document,
                    OperationFamily::NumberOverrideScenario,
                    &overrode.target,
                )
                .unwrap_or_else(|_| {
                    document_requirement(
                        document_scope,
                        document,
                        OperationFamily::NumberOverrideScenario,
                    )
                })
            })
            .collect::<BTreeSet<_>>();
        self.require_query_projection(principal, &requirements, now)?;
        Ok(overrides
            .iter()
            .filter_map(|overrode| match field_value(document, &overrode.target) {
                None => Some(ScenarioOverrideFailure::UnresolvedTarget {
                    target: overrode.target.clone(),
                }),
                Some(Value::Number(_)) => None,
                Some(value) => Some(ScenarioOverrideFailure::UnsupportedKind {
                    target: overrode.target.clone(),
                    actual: value_kind(value),
                }),
            })
            .collect())
    }

    fn scenario_target_outcome(
        &self,
        projection: &ScenarioTargetProjection<'_>,
        target: &FieldRef,
    ) -> ScenarioTargetOutcome {
        let Some(value) = field_value(projection.document, target) else {
            let requirement = BTreeSet::from([document_requirement(
                projection.document_scope,
                projection.document,
                OperationFamily::NumberOverrideScenario,
            )]);
            return if self
                .authorize_query(projection.principal, &requirement, projection.now)
                .is_ok()
            {
                ScenarioTargetOutcome::UnresolvedTarget
            } else {
                ScenarioTargetOutcome::DisclosureDenied
            };
        };
        let Ok(target_requirement) = self.field_requirement(
            projection.document,
            OperationFamily::NumberOverrideScenario,
            target,
        ) else {
            return ScenarioTargetOutcome::DisclosureDenied;
        };
        if !matches!(value, Value::Formula(_)) {
            return if self
                .authorize_query(
                    projection.principal,
                    &BTreeSet::from([target_requirement]),
                    projection.now,
                )
                .is_ok()
            {
                ScenarioTargetOutcome::UnsupportedKind {
                    actual: value_kind(value),
                }
            } else {
                ScenarioTargetOutcome::DisclosureDenied
            };
        }

        let direct_inputs = projection
            .dependencies
            .get(target)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let direct_dependents = direct_dependents(projection.dependencies, target);
        let mut requirements = BTreeSet::from([target_requirement]);
        for field in direct_inputs.iter().chain(&direct_dependents) {
            requirements.insert(self.field_or_document_requirement(
                projection.document_scope,
                projection.document,
                OperationFamily::NumberOverrideScenario,
                field,
            ));
        }
        let baseline = calculation_for(projection.baseline, target);
        let candidate = calculation_for(projection.candidate, target);
        for field in calculation_evidence_subjects(&baseline)
            .into_iter()
            .chain(calculation_evidence_subjects(&candidate))
        {
            requirements.insert(self.field_or_document_requirement(
                projection.document_scope,
                projection.document,
                OperationFamily::NumberOverrideScenario,
                &field,
            ));
        }
        if self
            .authorize_query(projection.principal, &requirements, projection.now)
            .is_err()
        {
            return ScenarioTargetOutcome::DisclosureDenied;
        }
        let Value::Formula(expression) = value else {
            unreachable!("formula kind checked above")
        };
        ScenarioTargetOutcome::Formula(ScenarioFormulaComparison {
            expression: expression.clone(),
            direct_inputs,
            direct_dependents,
            baseline,
            candidate,
        })
    }

    fn field_requirement(
        &self,
        document: &Document,
        family: OperationFamily,
        field: &FieldRef,
    ) -> Result<DisclosureRequirement, WorkspaceError> {
        Ok(DisclosureRequirement {
            family,
            scope: self.field_scope(document, field)?,
        })
    }

    fn field_or_document_requirement(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        family: OperationFamily,
        field: &FieldRef,
    ) -> DisclosureRequirement {
        self.field_requirement(document, family, field)
            .unwrap_or_else(|_| document_requirement(document_scope, document, family))
    }

    fn require_query_projection(
        &self,
        principal: &PrincipalId,
        requirements: &BTreeSet<DisclosureRequirement>,
        now: TrustedInstant,
    ) -> Result<(), FormulaOperationError> {
        self.authorize_query(principal, requirements, now)
            .map(|_| ())
            .map_err(|error| match error {
                PatchLifecycleError::InsufficientCapability { .. } => {
                    FormulaOperationError::Lifecycle(PatchLifecycleError::DisclosureDenied)
                }
                other => FormulaOperationError::Lifecycle(other),
            })
    }

    fn require_query_for_admission(
        &self,
        principal: &PrincipalId,
        requirements: &BTreeSet<DisclosureRequirement>,
        now: TrustedInstant,
    ) -> Result<(), PatchLifecycleError> {
        self.authorize_query(principal, requirements, now)
            .map(|_| ())
            .map_err(|error| match error {
                PatchLifecycleError::InsufficientCapability { .. } => {
                    PatchLifecycleError::DisclosureDenied
                }
                other => other,
            })
    }

    fn complete_validation_projection(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
        family: OperationFamily,
        principal: &PrincipalId,
        now: TrustedInstant,
        report: ValidationReport,
    ) -> Option<ValidationReport> {
        let requirements = document_requirements(document_scope, document, family);
        self.authorize_query(principal, &requirements, now)
            .is_ok()
            .then_some(report)
    }
}

fn admit_scenario_envelope(
    request: &ScenarioRequest,
) -> Result<(Vec<NormalizedNumberOverride>, Vec<FieldRef>), ScenarioEnvelopeError> {
    if request.overrides.len() > MAX_SCENARIO_OVERRIDES {
        return Err(ScenarioEnvelopeError::TooManyOverrides);
    }
    if request.requested_targets.len() > MAX_SCENARIO_TARGETS {
        return Err(ScenarioEnvelopeError::TooManyTargets);
    }
    let mut seen = BTreeSet::new();
    for overrode in &request.overrides {
        if !seen.insert(overrode.target.clone()) {
            return Err(ScenarioEnvelopeError::DuplicateOverride {
                target: overrode.target.clone(),
            });
        }
    }
    let normalized = request
        .overrides
        .iter()
        .map(|overrode| {
            Number::new(overrode.value)
                .map(|value| NormalizedNumberOverride {
                    target: overrode.target.clone(),
                    value,
                })
                .map_err(|_| ScenarioEnvelopeError::NonFiniteOverride {
                    target: overrode.target.clone(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let targets = request
        .requested_targets
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok((normalized, targets))
}

fn field_value<'document>(
    document: &'document Document,
    field: &FieldRef,
) -> Option<&'document Value> {
    document
        .entities
        .get(&field.entity)
        .and_then(|entity| entity.fields.get(&field.field))
}

fn scenario_candidate(
    document: &Document,
    overrides: &[NormalizedNumberOverride],
) -> Result<Document, ()> {
    let mut candidate = document.clone();
    for overrode in overrides {
        let entity = candidate
            .entities
            .get_mut(&overrode.target.entity)
            .ok_or(())?;
        if !matches!(
            entity.fields.get(&overrode.target.field),
            Some(Value::Number(_))
        ) {
            return Err(());
        }
        entity
            .fields
            .insert(overrode.target.field.clone(), Value::Number(overrode.value));
    }
    Ok(candidate)
}

fn value_kind(value: &Value) -> SemanticValueKind {
    match value {
        Value::Number(_) => SemanticValueKind::Number,
        Value::Formula(_) => SemanticValueKind::Formula,
        Value::Text(_) => SemanticValueKind::Text,
        Value::Boolean(_) => SemanticValueKind::Boolean,
        Value::Reference(_) => SemanticValueKind::Reference,
    }
}

fn calculation_dependencies(
    outcome: &CalculationOutcome,
) -> &BTreeMap<FieldRef, BTreeSet<FieldRef>> {
    match outcome {
        CalculationOutcome::Complete(calculation) => calculation.dependencies(),
        CalculationOutcome::Failed(failures) => failures.dependencies(),
    }
}

fn calculation_for(outcome: &CalculationOutcome, target: &FieldRef) -> FormulaCalculationOutcome {
    match outcome {
        CalculationOutcome::Complete(calculation) => calculation.value(target).map_or(
            FormulaCalculationOutcome::Unavailable,
            FormulaCalculationOutcome::Value,
        ),
        CalculationOutcome::Failed(failures) => failures
            .failures()
            .get(target)
            .map_or(FormulaCalculationOutcome::Unavailable, |failure| {
                FormulaCalculationOutcome::Failure(failure.clone())
            }),
    }
}

fn calculation_evidence_subjects(outcome: &FormulaCalculationOutcome) -> BTreeSet<FieldRef> {
    let FormulaCalculationOutcome::Failure(failure) = outcome else {
        return BTreeSet::new();
    };
    match failure {
        CalculationFailure::InvalidReferences { targets } => targets.keys().cloned().collect(),
        CalculationFailure::Cycle { members } => members.clone(),
        CalculationFailure::FailedDependencies { dependencies } => dependencies.clone(),
        CalculationFailure::InvalidExpression { .. }
        | CalculationFailure::DivisionByZero
        | CalculationFailure::NonFiniteResult => BTreeSet::new(),
    }
}

fn direct_dependents(
    dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    target: &FieldRef,
) -> Vec<FieldRef> {
    dependencies
        .iter()
        .filter(|(_, inputs)| inputs.contains(target))
        .map(|(formula, _)| formula.clone())
        .collect()
}

fn affected_by(
    dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    target: &FieldRef,
) -> Vec<FieldRef> {
    let mut frontier = BTreeSet::from([target.clone()]);
    let mut affected = BTreeSet::new();
    loop {
        let next = dependencies
            .iter()
            .filter(|(formula, inputs)| {
                !affected.contains(*formula) && !inputs.is_disjoint(&frontier)
            })
            .map(|(formula, _)| formula.clone())
            .collect::<BTreeSet<_>>();
        if next.is_empty() {
            break;
        }
        frontier.clone_from(&next);
        affected.extend(next);
    }
    affected.remove(target);
    affected.into_iter().collect()
}

fn scenario_impact(before: &Document, after: &Document) -> Option<ScenarioImpact> {
    let semantic_diff = tachiko_diff_engine::diff(before, after).ok()?;
    let mut changed = BTreeSet::new();
    let mut affected = BTreeSet::new();
    for change in semantic_diff.changes() {
        match change {
            SemanticChange::FieldChanged { field, .. } => {
                changed.insert(field.clone());
            }
            SemanticChange::FormulaImpact { field, .. } => {
                changed.insert(field.clone());
                affected.insert(field.clone());
            }
            _ => return None,
        }
    }
    Some(ScenarioImpact {
        changed_fields: changed.into_iter().collect(),
        affected_fields: affected.into_iter().collect(),
    })
}

fn document_requirements(
    document_scope: &DocumentScopeId,
    document: &Document,
    family: OperationFamily,
) -> BTreeSet<DisclosureRequirement> {
    BTreeSet::from([document_requirement(document_scope, document, family)])
}

fn document_requirement(
    document_scope: &DocumentScopeId,
    document: &Document,
    family: OperationFamily,
) -> DisclosureRequirement {
    DisclosureRequirement {
        family,
        scope: ScopedSemanticSubject::new(
            document_scope.clone(),
            document.id.clone(),
            SemanticScope::Document,
        ),
    }
}
