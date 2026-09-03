//! Shared application operations over Tachiko Work semantic documents.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use serde::Serialize;
use tachiko_diff_engine::diff;
pub use tachiko_diff_engine::{DiffError, SemanticChange, SemanticDiff};
#[cfg(feature = "issue-175-research")]
pub use tachiko_formula_engine::CalculationOutcome as Issue175CalculationOutcome;
use tachiko_formula_engine::{
    Calculation, CalculationOutcome, FormulaBindError, FormulaParseError, RetainedCalculationState,
    bind_expression, calculate_complete, parse_expression, project_expression,
    validate_expression_structure,
};
pub use tachiko_formula_engine::{
    CalculationError, CalculationFailure, CanonicalAuthoringProjectionError,
    ExpressionComplexityError, ReferenceFailure,
};
pub use tachiko_merge_engine::{
    ConflictFacet, ConflictFact, ConflictKind, ConflictTarget, EntitySubject, MergeConflict,
    MergeValue, SEMANTIC_CONFLICT_V1, SchemaFieldSubject, SchemaSubject, SemanticConflictContract,
    UnsupportedConflictContract, UnsupportedConflictKind, UnsupportedTargetFacet,
};
use tachiko_merge_engine::{MergeOutcome, UnmaterializedStoredFact, merge};
use tachiko_semantic_core::{
    AddressIndex, AddressIndexError, is_valid_identifier, validate_document_core,
};
pub use tachiko_semantic_core::{
    Diagnostic, DiagnosticCode, DiagnosticFact, DiagnosticLocation, DiagnosticProvider,
    DiagnosticSeverity, Document, DocumentId, Entity, EntityId, EntityKey, Expression,
    FieldAddress, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema,
    SchemaId, SchemaKey, SemanticSubject, StableDiagnosticObservation, Value,
};
use thiserror::Error;

pub mod analysis_operations;
pub mod capability_discovery;
pub mod formula_operations;
pub mod patch_lifecycle;
pub mod resident_session;

/// Research-only access to the accepted complete calculation oracle for Issue
/// #175 cross-crate admission parity tests.
#[cfg(feature = "issue-175-research")]
#[must_use]
pub fn issue_175_calculate_complete(document: &Document) -> Issue175CalculationOutcome {
    calculate_complete(document)
}

/// Symbolic codes emitted by workspace composition of formula-engine outcomes.
///
/// The catalog is internal and provisional under ADR-0019; code meanings are
/// stable observations and do not depend on Rust enum ordinals.
pub mod diagnostic_codes {
    use tachiko_semantic_core::DiagnosticCode;

    pub const FORMULA_STRUCTURAL: DiagnosticCode = DiagnosticCode::new("formula.invalid_structure");
    pub const FORMULA_INVALID_REFERENCES: DiagnosticCode =
        DiagnosticCode::new("formula.invalid_references");
    pub const FORMULA_CYCLE: DiagnosticCode = DiagnosticCode::new("formula.cycle");
    pub const FORMULA_FAILED_DEPENDENCY: DiagnosticCode =
        DiagnosticCode::new("formula.failed_dependency");
    pub const FORMULA_DIVISION_BY_ZERO: DiagnosticCode =
        DiagnosticCode::new("formula.division_by_zero");
    pub const FORMULA_NON_FINITE_RESULT: DiagnosticCode =
        DiagnosticCode::new("formula.non_finite_result");
    pub const MERGE_UNMATERIALIZED_QUALIFIED_FIELD: DiagnosticCode =
        DiagnosticCode::new("merge.unmaterialized_qualified_field");
}

/// Authoritative first-party semantic validation result for one snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationReport {
    diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort();
        diagnostics.dedup_by(|left, right| left.stable_observation() == right.stable_observation());
        Self { diagnostics }
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn stable_observations(&self) -> Vec<StableDiagnosticObservation> {
        self.diagnostics
            .iter()
            .map(Diagnostic::stable_observation)
            .collect()
    }

    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Operation-local role of a snapshot whose semantic report blocked a call.
///
/// This context is not part of diagnostic identity or stable observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationRole {
    Current,
    Candidate,
    ComparisonBefore,
    ComparisonAfter,
    MergeBase,
    MergeOurs,
    MergeTheirs,
    MergeCandidate,
}

impl fmt::Display for ValidationRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Current => "current",
            Self::Candidate => "candidate",
            Self::ComparisonBefore => "comparison-before",
            Self::ComparisonAfter => "comparison-after",
            Self::MergeBase => "merge-base",
            Self::MergeOurs => "merge-ours",
            Self::MergeTheirs => "merge-theirs",
            Self::MergeCandidate => "merge-candidate",
        })
    }
}

/// A useful starting point for a newly-created semantic document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StarterTemplate {
    GameBalance,
    Empty,
}

/// The nominal object category requested at the replaceable creation seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticIdKind {
    Document,
    Schema,
    Field,
    Entity,
}

/// Host-supplied stable-ID creation boundary.
pub trait IdGenerator {
    fn generate(&mut self, kind: SemanticIdKind) -> String;
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentOverview {
    pub schema_count: usize,
    pub entity_count: usize,
    pub formula_count: usize,
    pub entities: Vec<EntityOverview>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityOverview {
    pub id: EntityId,
    pub key: EntityKey,
    pub label: String,
    pub schema: SchemaKey,
    pub fields: Vec<FieldOverview>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldOverview {
    pub id: FieldId,
    pub key: FieldKey,
    pub display_value: String,
    pub kind: FieldKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Input,
    Reference { target_schema: SchemaKey },
    Formula,
}

/// Existing finite semantic value categories used by structured operation
/// projections. This is a value-kind observation, not a universal type ID or
/// registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticValueKind {
    Number,
    Formula,
    Text,
    Boolean,
    Reference,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldExplanation {
    pub field: FieldRef,
    pub address: FieldAddress,
    pub display_value: String,
    pub expression: Option<String>,
    pub dependencies: Vec<FieldRef>,
    pub dependency_addresses: Vec<FieldAddress>,
    pub affected_formulas: Vec<AffectedFormula>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AffectedFormula {
    pub field: FieldRef,
    pub address: FieldAddress,
    pub display_value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditPreview {
    pub document: Document,
    pub diff: SemanticDiff,
}

/// One calculated numeric field projected through its current human address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalculatedField {
    pub field: FieldRef,
    pub address: FieldAddress,
    pub value: Number,
}

/// Provider-independent formula facts for a stable field reference.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaAnalysis {
    pub field: FieldRef,
    pub expression: Expression,
    pub value: Number,
    pub dependencies: Vec<FieldRef>,
}

/// Caller-owned evidence identifying the semantic snapshot used by a query.
///
/// `source_label` is an opaque host projection such as a path, commit, branch,
/// or test-fixture name. It is not semantic identity, a revision token, or a
/// concurrency protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceStateEvidence {
    pub document_id: DocumentId,
    pub source_label: String,
}

impl SourceStateEvidence {
    #[must_use]
    pub fn new(document: &Document, source_label: impl Into<String>) -> Self {
        Self {
            document_id: document.id.clone(),
            source_label: source_label.into(),
        }
    }
}

/// Deterministic document/schema/entity inspection for semantic clients.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DocumentInspection {
    pub source: SourceStateEvidence,
    pub title: String,
    pub schemas: Vec<SchemaInspection>,
    pub entities: Vec<EntityInspection>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SchemaInspection {
    pub id: SchemaId,
    pub key: SchemaKey,
    pub fields: Vec<FieldDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EntityInspection {
    pub id: EntityId,
    pub key: EntityKey,
    pub schema: SchemaId,
    pub fields: Vec<FieldId>,
}

/// One downstream formula affected by a queried field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DownstreamImpact {
    pub field: FieldRef,
    pub address: FieldAddress,
    pub value: Number,
}

/// Provider-independent facts for one stable field in one identified snapshot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FieldAnalysis {
    pub source: SourceStateEvidence,
    pub field: FieldRef,
    pub address: FieldAddress,
    pub stored_value: Value,
    pub calculated_value: Option<Number>,
    pub formula_source: Option<String>,
    pub direct_dependencies: Vec<FieldRef>,
    pub upstream_dependencies: Vec<FieldRef>,
    pub downstream_impacts: Vec<DownstreamImpact>,
}

/// Semantic comparison plus deterministic stable-ID affected-area projection.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeAnalysis {
    pub before: SourceStateEvidence,
    pub after: SourceStateEvidence,
    pub changes: Vec<SemanticChange>,
    pub affected_schemas: Vec<SchemaId>,
    pub affected_entities: Vec<EntityId>,
    pub affected_fields: Vec<FieldRef>,
}

/// Full semantic validation findings for one identified snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidationAnalysis {
    pub source: SourceStateEvidence,
    pub is_valid: bool,
    pub diagnostics: Vec<Diagnostic>,
}

/// An inert typed field proposal that passed shared application policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedFieldValue {
    pub field: FieldRef,
    pub value: Value,
}

/// The application-level outcome of model merge plus semantic impact.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceMergeOutcome {
    Merged(Box<EditPreview>),
    Conflicted(Vec<MergeConflict>),
}

/// Current portable runtime export projection.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeExport {
    pub format_version: u32,
    pub document_id: String,
    pub title: String,
    pub entities: BTreeMap<String, RuntimeEntity>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeEntity {
    pub schema: String,
    pub fields: BTreeMap<String, RuntimeValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RuntimeValue {
    Number(Number),
    Text(String),
    Boolean(bool),
    Reference { reference: String },
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("semantic address lookup failed: {0}")]
    Address(#[from] AddressIndexError),
    #[error("entity key '{entity}' does not exist")]
    MissingEntity { entity: EntityKey },
    #[error("entity stable id '{entity}' does not exist")]
    MissingEntityId { entity: EntityId },
    #[error("entity key '{entity}' is not a valid human address")]
    InvalidEntityKey { entity: EntityKey },
    #[error("entity key '{entity}' already exists")]
    EntityKeyAlreadyExists { entity: EntityKey },
    #[error("cannot rename entity key '{entity}' to itself")]
    NoOpEntityRename { entity: EntityKey },
    #[error("schema key '{schema}' does not exist")]
    MissingSchemaKey { schema: SchemaKey },
    #[error("schema key '{schema}' is not a valid human address")]
    InvalidSchemaKey { schema: SchemaKey },
    #[error("schema key '{schema}' already exists")]
    SchemaKeyAlreadyExists { schema: SchemaKey },
    #[error("cannot rename schema key '{schema}' to itself")]
    NoOpSchemaRename { schema: SchemaKey },
    #[error("field key '{field}' already exists in schema '{schema}'")]
    FieldKeyAlreadyExists { schema: SchemaKey, field: FieldKey },
    #[error("field key '{field}' does not exist in schema '{schema}'")]
    MissingFieldKey { schema: SchemaKey, field: FieldKey },
    #[error("field key '{field}' is not a valid human address in schema '{schema}'")]
    InvalidFieldKey { schema: SchemaKey, field: FieldKey },
    #[error("cannot rename field key '{field}' to itself")]
    NoOpFieldRename { field: FieldKey },
    #[error("generated {kind:?} stable id was empty")]
    EmptyGeneratedId { kind: SemanticIdKind },
    #[error("generated {kind:?} stable id '{id}' collides with an existing object")]
    GeneratedIdCollision { kind: SemanticIdKind, id: String },
    #[error(
        "cannot remove entity '{entity}' because it is referenced by {}",
        format_dependent_addresses(.dependent_addresses)
    )]
    EntityReferenced {
        entity: EntityKey,
        dependents: Vec<FieldRef>,
        dependent_addresses: Vec<FieldAddress>,
    },
    #[error("field '{field}' does not exist")]
    MissingField { field: FieldRef },
    #[error("schema stable id '{schema}' does not exist")]
    MissingSchema { schema: SchemaId },
    #[error("field '{field}' is a formula; edit its inputs instead")]
    FormulaEdit { field: FieldRef },
    #[error("field '{field}' is not a formula")]
    NotFormula { field: FieldRef },
    #[error("formula field '{field}' has no calculated value")]
    MissingCalculation { field: FieldRef },
    #[error("value for '{field}' does not match its schema type")]
    TypeMismatch { field: FieldRef },
    #[error("field '{field}' is not numeric; formulas require a numeric field")]
    NonNumericFormulaField { field: FieldRef },
    #[error("invalid formula syntax for '{field}': {source}")]
    InvalidFormula {
        field: FieldRef,
        #[source]
        source: FormulaParseError,
    },
    #[error("formula binding failed for '{field}': {source}")]
    FormulaBinding {
        field: FieldRef,
        #[source]
        source: Box<FormulaBindError>,
    },
    #[error("formula projection failed for '{field}': {source}")]
    FormulaProjection {
        field: FieldRef,
        #[source]
        source: CanonicalAuthoringProjectionError,
    },
    #[error("formula for '{field}' exceeds authoring complexity limits: {source}")]
    ExpressionComplexity {
        field: FieldRef,
        #[source]
        source: ExpressionComplexityError,
    },
    #[error("'{input}' is not a valid {expected} value for '{field}'")]
    InvalidValue {
        field: FieldRef,
        input: String,
        expected: &'static str,
    },
    #[error("{role} document is semantically invalid: {summary}")]
    InvalidDocument {
        role: ValidationRole,
        summary: String,
        report: ValidationReport,
    },
    #[error("'{field}' already has that value")]
    NoChange { field: FieldRef },
    #[error("could not calculate document: {0}")]
    Calculation(#[from] CalculationError),
    #[error("could not compare edited document: {0}")]
    Diff(#[from] DiffError),
    #[error(
        "merge inputs belong to different documents: base '{base}', left '{left}', right '{right}'"
    )]
    DifferentMergeDocument {
        base: DocumentId,
        left: DocumentId,
        right: DocumentId,
    },
}

/// Create a document through the host-supplied stable-ID boundary.
///
/// # Errors
///
/// Returns an error for an empty/colliding generated ID or an invalid template
/// candidate.
pub fn create_document(
    template: StarterTemplate,
    title: impl Into<String>,
    generator: &mut impl IdGenerator,
) -> Result<Document, WorkspaceError> {
    let title = title.into();
    let document_id = next_document_id(generator)?;
    let document = match template {
        StarterTemplate::GameBalance => game_balance_document(document_id, title, generator)?,
        StarterTemplate::Empty => Document::empty(document_id, title),
    };
    require_validated_calculation_for(&document, ValidationRole::Candidate)?;
    preflight_formula_projections(&document)?;
    Ok(document)
}

/// Validate intrinsic semantics and require a complete deterministic
/// calculation without publishing partial results.
///
/// # Errors
///
/// Returns the shared semantic or calculation failure for this snapshot.
pub fn validate(document: &Document) -> Result<(), WorkspaceError> {
    require_validated_calculation(document)?;
    Ok(())
}

/// Calculate every numeric field and project it through current human keys.
///
/// # Errors
///
/// Returns a semantic, address-projection, or calculation failure.
pub fn calculate_fields(document: &Document) -> Result<Vec<CalculatedField>, WorkspaceError> {
    let calculation = require_validated_calculation(document)?;
    let index = AddressIndex::build(document)?;
    let mut fields = calculation
        .values()
        .iter()
        .map(|(field, value)| {
            Ok(CalculatedField {
                field: field.clone(),
                address: index.field_address(document, field)?,
                value: *value,
            })
        })
        .collect::<Result<Vec<_>, WorkspaceError>>()?;
    fields.sort_by(|left, right| left.address.cmp(&right.address));
    Ok(fields)
}

/// Compare two semantic snapshots through the shared diff orchestration.
///
/// # Errors
///
/// Returns the diff engine's typed calculation/comparison failure.
pub fn compare_documents(
    before: &Document,
    after: &Document,
) -> Result<SemanticDiff, WorkspaceError> {
    require_validated_calculation_for(before, ValidationRole::ComparisonBefore)?;
    require_validated_calculation_for(after, ValidationRole::ComparisonAfter)?;
    Ok(diff(before, after)?)
}

/// Analyze one stable formula field without provider or presentation policy.
///
/// # Errors
///
/// Returns a typed lookup, formula-kind, or calculation failure.
pub fn analyze_formula(
    document: &Document,
    field: &FieldRef,
) -> Result<FormulaAnalysis, WorkspaceError> {
    let calculation = require_validated_calculation(document)?;
    let value = field_value(document, field)?;
    let Value::Formula(expression) = value else {
        return Err(WorkspaceError::NotFormula {
            field: field.clone(),
        });
    };
    let value = calculation
        .value(field)
        .ok_or_else(|| WorkspaceError::MissingCalculation {
            field: field.clone(),
        })?;
    let dependencies = calculation
        .dependencies_of(field)
        .map_or_else(Vec::new, |dependencies| {
            dependencies.iter().cloned().collect()
        });

    Ok(FormulaAnalysis {
        field: field.clone(),
        expression: expression.clone(),
        value,
        dependencies,
    })
}

/// Inspect document structure without adding calculation or validation policy.
#[must_use]
pub fn inspect_document(
    document: &Document,
    source_label: impl Into<String>,
) -> DocumentInspection {
    let schemas = document
        .schemas
        .values()
        .map(|schema| SchemaInspection {
            id: schema.id.clone(),
            key: schema.key.clone(),
            fields: schema.fields.values().cloned().collect(),
        })
        .collect();
    let entities = document
        .entities
        .values()
        .map(|entity| EntityInspection {
            id: entity.id.clone(),
            key: entity.key.clone(),
            schema: entity.schema.clone(),
            fields: entity.fields.keys().cloned().collect(),
        })
        .collect();

    DocumentInspection {
        source: SourceStateEvidence::new(document, source_label),
        title: document.title.clone(),
        schemas,
        entities,
    }
}

/// Analyze one stable field through the shared formula/dependency authority.
///
/// # Errors
///
/// Returns an explicit missing-target, semantic validation, formula projection,
/// address projection, or calculation failure.
pub fn analyze_field(
    document: &Document,
    source_label: impl Into<String>,
    field: &FieldRef,
) -> Result<FieldAnalysis, WorkspaceError> {
    let stored_value = field_value(document, field)?.clone();
    let calculation = require_validated_calculation(document)?;
    let index = AddressIndex::build(document)?;
    let address = index.field_address(document, field)?;
    let formula_source = match &stored_value {
        Value::Formula(expression) => {
            Some(project_expression(document, expression).map_err(|source| {
                WorkspaceError::FormulaProjection {
                    field: field.clone(),
                    source,
                }
            })?)
        }
        Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Reference(_) => None,
    };
    let direct_dependencies = calculation
        .dependencies_of(field)
        .map_or_else(Vec::new, |dependencies| {
            dependencies.iter().cloned().collect()
        });
    let upstream_dependencies = transitive_dependencies(&calculation, field);
    let downstream_impacts = calculation
        .affected_by(field)
        .into_iter()
        .filter_map(|affected| calculation.value(&affected).map(|value| (affected, value)))
        .map(|(affected, value)| {
            Ok(DownstreamImpact {
                address: index.field_address(document, &affected)?,
                field: affected,
                value,
            })
        })
        .collect::<Result<Vec<_>, WorkspaceError>>()?;

    Ok(FieldAnalysis {
        source: SourceStateEvidence::new(document, source_label),
        field: field.clone(),
        address,
        stored_value,
        calculated_value: calculation.value(field),
        formula_source,
        direct_dependencies,
        upstream_dependencies,
        downstream_impacts,
    })
}

/// Compare two identified snapshots and project the stable semantic areas
/// touched by the existing deterministic diff.
///
/// # Errors
///
/// Returns the shared comparison validation or diff failure.
pub fn analyze_changes(
    before: &Document,
    before_source_label: impl Into<String>,
    after: &Document,
    after_source_label: impl Into<String>,
) -> Result<ChangeAnalysis, WorkspaceError> {
    let semantic_diff = compare_documents(before, after)?;
    let changes = semantic_diff.changes().to_vec();
    let (affected_schemas, affected_entities, affected_fields) =
        affected_areas(before, after, &changes);

    Ok(ChangeAnalysis {
        before: SourceStateEvidence::new(before, before_source_label),
        after: SourceStateEvidence::new(after, after_source_label),
        changes,
        affected_schemas,
        affected_entities,
        affected_fields,
    })
}

/// Explain every current validation failure without requiring the snapshot to
/// calculate successfully.
#[must_use]
pub fn analyze_validation(
    document: &Document,
    source_label: impl Into<String>,
) -> ValidationAnalysis {
    let report = validation_report(document);
    ValidationAnalysis {
        source: SourceStateEvidence::new(document, source_label),
        is_valid: report.is_valid(),
        diagnostics: report.into_diagnostics(),
    }
}

fn transitive_dependencies(calculation: &Calculation, field: &FieldRef) -> Vec<FieldRef> {
    let mut pending = calculation
        .dependencies_of(field)
        .cloned()
        .unwrap_or_default();
    let mut dependencies = BTreeSet::new();
    while let Some(dependency) = pending.pop_first() {
        if !dependencies.insert(dependency.clone()) {
            continue;
        }
        if let Some(nested) = calculation.dependencies_of(&dependency) {
            pending.extend(
                nested
                    .iter()
                    .filter(|candidate| !dependencies.contains(*candidate))
                    .cloned(),
            );
        }
    }
    dependencies.into_iter().collect()
}

fn affected_areas(
    before: &Document,
    after: &Document,
    changes: &[SemanticChange],
) -> (Vec<SchemaId>, Vec<EntityId>, Vec<FieldRef>) {
    let mut schemas = BTreeSet::new();
    let mut entities = BTreeSet::new();
    let mut fields = BTreeSet::new();

    for change in changes {
        match change {
            SemanticChange::DocumentIdChanged { .. }
            | SemanticChange::DocumentTitleChanged { .. } => {}
            SemanticChange::SchemaAdded { schema, .. }
            | SemanticChange::SchemaRemoved { schema, .. }
            | SemanticChange::SchemaKeyChanged { schema, .. }
            | SemanticChange::SchemaFieldAdded { schema, .. }
            | SemanticChange::SchemaFieldRemoved { schema, .. }
            | SemanticChange::SchemaFieldChanged { schema, .. }
            | SemanticChange::FieldKeyChanged { schema, .. } => {
                schemas.insert(schema.clone());
            }
            SemanticChange::EntityAdded { entity }
            | SemanticChange::EntityRemoved { entity }
            | SemanticChange::EntityKeyChanged { entity, .. } => {
                insert_entity_area(before, after, entity, &mut schemas, &mut entities);
            }
            SemanticChange::EntitySchemaChanged {
                entity,
                before,
                after,
            } => {
                entities.insert(entity.clone());
                schemas.insert(before.clone());
                schemas.insert(after.clone());
            }
            SemanticChange::FieldAdded { field, .. }
            | SemanticChange::FieldRemoved { field, .. }
            | SemanticChange::FieldChanged { field, .. }
            | SemanticChange::FormulaImpact { field, .. } => {
                fields.insert(field.clone());
                insert_entity_area(before, after, &field.entity, &mut schemas, &mut entities);
            }
        }
    }

    (
        schemas.into_iter().collect(),
        entities.into_iter().collect(),
        fields.into_iter().collect(),
    )
}

fn insert_entity_area(
    before: &Document,
    after: &Document,
    entity: &EntityId,
    schemas: &mut BTreeSet<SchemaId>,
    entities: &mut BTreeSet<EntityId>,
) {
    entities.insert(entity.clone());
    if let Some(schema) = after
        .entities
        .get(entity)
        .or_else(|| before.entities.get(entity))
        .map(|entity| entity.schema.clone())
    {
        schemas.insert(schema);
    }
}

/// Validate an inert typed field proposal through shared mutation policy.
///
/// The candidate document is deliberately not returned or persisted; approval
/// and write capabilities remain adapter concerns.
///
/// # Errors
///
/// Returns a typed precondition, authoring, semantic, or calculation failure.
pub fn validate_field_value_suggestion(
    document: &Document,
    field: FieldRef,
    value: Value,
) -> Result<ValidatedFieldValue, WorkspaceError> {
    let candidate = match field_value_candidate(document, &field, &value) {
        Ok(candidate) => candidate,
        Err(error) => {
            drop_value_iteratively(value);
            return Err(error);
        }
    };
    require_validated_calculation_for(&candidate, ValidationRole::Candidate)?;
    Ok(ValidatedFieldValue { field, value })
}

/// Merge three semantic snapshots and calculate base-to-merged impact.
///
/// # Errors
///
/// Returns the shared merge or semantic comparison failure.
pub fn merge_documents(
    base: &Document,
    ours: &Document,
    theirs: &Document,
) -> Result<WorkspaceMergeOutcome, WorkspaceError> {
    if base.id != ours.id || base.id != theirs.id {
        return Err(WorkspaceError::DifferentMergeDocument {
            base: base.id.clone(),
            left: ours.id.clone(),
            right: theirs.id.clone(),
        });
    }
    require_validated_calculation_for(base, ValidationRole::MergeBase)?;
    require_validated_calculation_for(ours, ValidationRole::MergeOurs)?;
    require_validated_calculation_for(theirs, ValidationRole::MergeTheirs)?;
    match merge(base, ours, theirs) {
        MergeOutcome::Merged(candidate) => {
            let (document, unmaterialized_fields) = candidate.into_parts();
            let (report, _calculation) = semantic_validation(&document);
            let mut diagnostics = report.into_diagnostics();
            diagnostics.extend(
                unmaterialized_fields
                    .iter()
                    .map(unmaterialized_qualified_field_diagnostic),
            );
            let report = ValidationReport::new(diagnostics);
            if !report.is_valid() {
                return Err(invalid_document(report, ValidationRole::MergeCandidate));
            }
            preflight_formula_projections(&document)?;
            let diff = diff(base, &document)?;
            Ok(WorkspaceMergeOutcome::Merged(Box::new(EditPreview {
                document,
                diff,
            })))
        }
        MergeOutcome::Conflicted(conflicts) => Ok(WorkspaceMergeOutcome::Conflicted(conflicts)),
    }
}

/// Build the current deterministic runtime projection after shared validation
/// and calculation.
///
/// # Errors
///
/// Returns a semantic lookup or calculation failure.
pub fn runtime_export(document: &Document) -> Result<RuntimeExport, WorkspaceError> {
    let calculation = require_validated_calculation(document)?;
    let mut entities = BTreeMap::new();

    for (entity_id, entity) in &document.entities {
        let schema =
            document
                .schemas
                .get(&entity.schema)
                .ok_or_else(|| WorkspaceError::MissingSchema {
                    schema: entity.schema.clone(),
                })?;
        let mut fields = BTreeMap::new();
        for (field_id, value) in &entity.fields {
            let definition =
                schema
                    .fields
                    .get(field_id)
                    .ok_or_else(|| WorkspaceError::MissingField {
                        field: FieldRef::new(entity_id.clone(), field_id.clone()),
                    })?;
            let field = FieldRef::new(entity_id.clone(), field_id.clone());
            fields.insert(
                definition.key.to_string(),
                runtime_value(document, value, &field, &calculation)?,
            );
        }
        entities.insert(
            entity.key.to_string(),
            RuntimeEntity {
                schema: schema.key.to_string(),
                fields,
            },
        );
    }

    Ok(RuntimeExport {
        format_version: 2,
        document_id: document.id.to_string(),
        title: document.title.clone(),
        entities,
    })
}

/// Build a deterministic calculated view suitable for adapters.
///
/// # Errors
///
/// Returns an error if semantic addresses or formulas are invalid.
pub fn overview(document: &Document) -> Result<DocumentOverview, WorkspaceError> {
    let calculation = require_validated_calculation(document)?;
    let mut formula_count = 0;
    let mut entities = Vec::new();

    for entity in document.entities.values() {
        let schema =
            document
                .schemas
                .get(&entity.schema)
                .ok_or_else(|| WorkspaceError::MissingSchema {
                    schema: entity.schema.clone(),
                })?;
        let mut fields = Vec::new();
        for (field_id, value) in &entity.fields {
            let definition =
                schema
                    .fields
                    .get(field_id)
                    .ok_or_else(|| WorkspaceError::MissingField {
                        field: FieldRef::new(entity.id.clone(), field_id.clone()),
                    })?;
            let field_ref = FieldRef::new(entity.id.clone(), field_id.clone());
            let kind = field_kind(document, value, &definition.field_type)?;
            if kind == FieldKind::Formula {
                formula_count += 1;
            }
            let display_value = if kind == FieldKind::Formula {
                calculation
                    .value(&field_ref)
                    .map_or_else(|| "unavailable".to_owned(), format_number)
            } else {
                format_value(document, value)
            };
            fields.push(FieldOverview {
                id: field_id.clone(),
                key: definition.key.clone(),
                display_value,
                kind,
            });
        }
        fields.sort_by(|left, right| left.key.cmp(&right.key));
        entities.push(EntityOverview {
            id: entity.id.clone(),
            key: entity.key.clone(),
            label: entity_label(document, entity),
            schema: schema.key.clone(),
            fields,
        });
    }
    entities.sort_by(|left, right| left.key.cmp(&right.key));

    Ok(DocumentOverview {
        schema_count: document.schemas.len(),
        entity_count: document.entities.len(),
        formula_count,
        entities,
    })
}

fn field_kind(
    document: &Document,
    value: &Value,
    field_type: &FieldType,
) -> Result<FieldKind, WorkspaceError> {
    match value {
        Value::Formula(_) => Ok(FieldKind::Formula),
        Value::Reference(_) => {
            let FieldType::Reference { schema } = field_type else {
                return Ok(FieldKind::Input);
            };
            let target =
                document
                    .schemas
                    .get(schema)
                    .ok_or_else(|| WorkspaceError::MissingSchema {
                        schema: schema.clone(),
                    })?;
            Ok(FieldKind::Reference {
                target_schema: target.key.clone(),
            })
        }
        Value::Number(_) | Value::Text(_) | Value::Boolean(_) => Ok(FieldKind::Input),
    }
}

/// Explain one human-addressed field.
///
/// # Errors
///
/// Returns an error when lookup, projection, or calculation fails.
pub fn explain_field(
    document: &Document,
    address: &FieldAddress,
) -> Result<FieldExplanation, WorkspaceError> {
    let calculation = require_validated_calculation(document)?;
    let field = document.resolve_field(address)?;
    let value = field_value(document, &field)?;
    let display_value = calculation
        .value(&field)
        .map_or_else(|| format_value(document, value), format_number);
    let expression = match value {
        Value::Formula(expression) => {
            Some(project_expression(document, expression).map_err(|source| {
                WorkspaceError::FormulaProjection {
                    field: field.clone(),
                    source,
                }
            })?)
        }
        _ => None,
    };
    let dependencies = calculation
        .dependencies_of(&field)
        .map_or_else(Vec::new, |dependencies| {
            dependencies.iter().cloned().collect()
        });
    let index = AddressIndex::build(document)?;
    let dependency_addresses = dependencies
        .iter()
        .map(|dependency| index.field_address(document, dependency))
        .collect::<Result<Vec<_>, _>>()?;
    let mut affected_formulas = Vec::new();
    for affected in calculation.affected_by(&field) {
        if let Some(value) = calculation.value(&affected) {
            affected_formulas.push(AffectedFormula {
                address: index.field_address(document, &affected)?,
                field: affected,
                display_value: format_number(value),
            });
        }
    }

    Ok(FieldExplanation {
        field,
        address: address.clone(),
        display_value,
        expression,
        dependencies,
        dependency_addresses,
        affected_formulas,
    })
}

/// Apply a schema-typed scalar edit addressed by current human keys.
///
/// # Errors
///
/// Returns a typed lookup, parsing, validation, calculation, or diff error.
pub fn set_scalar(
    document: &Document,
    address: &FieldAddress,
    input: &str,
) -> Result<EditPreview, WorkspaceError> {
    let field = document.resolve_field(address)?;
    let entity = &document.entities[&field.entity];
    let existing = field_value(document, &field)?;
    if matches!(existing, Value::Formula(_)) {
        return Err(WorkspaceError::FormulaEdit {
            field: field.clone(),
        });
    }
    let definition = document.schemas[&entity.schema]
        .fields
        .get(&field.field)
        .ok_or_else(|| WorkspaceError::MissingField {
            field: field.clone(),
        })?;
    let value = parse_scalar(document, &field, input, &definition.field_type)?;
    let edited = field_value_candidate(document, &field, &value)?;
    finalize_edit(document, edited)
}

/// Parse, bind, and apply a formula edit addressed by current human keys.
///
/// # Errors
///
/// Returns a typed parse, binding, lookup, validation, calculation, or diff
/// error without mutating the source document.
pub fn set_formula(
    document: &Document,
    address: &FieldAddress,
    input: &str,
) -> Result<EditPreview, WorkspaceError> {
    let field = document.resolve_field(address)?;
    let expression = bind_formula_update(document, &field, input)?;
    let value = Value::Formula(expression);
    let edited = field_value_candidate(document, &field, &value)?;
    finalize_edit(document, edited)
}

/// Admit formula authoring into one complete typed bound expression without
/// applying the later candidate validation/publication gate.
pub(crate) fn bind_formula_update(
    document: &Document,
    field: &FieldRef,
    input: &str,
) -> Result<Expression, WorkspaceError> {
    let unbound = parse_expression(input).map_err(|source| WorkspaceError::InvalidFormula {
        field: field.clone(),
        source,
    })?;
    bind_formula_update_unbound(document, field, &unbound)
}

pub(crate) fn bind_formula_update_unbound(
    document: &Document,
    field: &FieldRef,
    unbound: &tachiko_formula_engine::UnboundExpression,
) -> Result<Expression, WorkspaceError> {
    formula_update_target_rule(document, field)?;

    let expression =
        bind_expression(document, unbound).map_err(|source| WorkspaceError::FormulaBinding {
            field: field.clone(),
            source: Box::new(source),
        })?;
    Ok(expression)
}

/// Duplicate an entity under a new key and generated stable identity.
///
/// Formula self-references rebase to the new stable ID; every other bound
/// relationship retains its existing stable target.
///
/// # Errors
///
/// Returns a typed key, generation, validation, calculation, or diff error.
pub fn duplicate_entity(
    document: &Document,
    source: impl AsRef<str>,
    target: impl AsRef<str>,
    generator: &mut impl IdGenerator,
) -> Result<EditPreview, WorkspaceError> {
    let source_key = EntityKey::from(source.as_ref());
    let target_key = EntityKey::from(target.as_ref());
    validate_new_entity_key(document, &target_key)?;
    let index = AddressIndex::build(document)?;
    let source_id = index
        .entity_id(&source_key)
        .map_err(|_| WorkspaceError::MissingEntity {
            entity: source_key.clone(),
        })?
        .clone();
    let target_id = next_entity_id(generator)?;
    if document.entities.contains_key(&target_id) {
        return Err(WorkspaceError::GeneratedIdCollision {
            kind: SemanticIdKind::Entity,
            id: target_id.to_string(),
        });
    }
    preflight_formula_structures(document)?;

    let mut duplicate = document.entities[&source_id].clone();
    duplicate.id = target_id.clone();
    duplicate.key = target_key;
    for value in duplicate.fields.values_mut() {
        if let Value::Formula(expression) = value {
            rewrite_expression_entity(expression, &source_id, &target_id);
        }
    }

    let mut edited = document.clone();
    edited.entities.insert(target_id, duplicate);
    finalize_edit(document, edited)
}

/// Rename an entity's mutable human key while preserving stable identity and
/// every bound relationship.
///
/// # Errors
///
/// Returns a typed key, projection, validation, calculation, or diff error.
pub fn rename_entity(
    document: &Document,
    source: impl AsRef<str>,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkspaceError> {
    let source = EntityKey::from(source.as_ref());
    let target = EntityKey::from(target.as_ref());
    if source == target {
        return Err(WorkspaceError::NoOpEntityRename { entity: source });
    }
    validate_new_entity_key(document, &target)?;
    let entity_id = AddressIndex::build(document)?
        .entity_id(&source)
        .map_err(|_| WorkspaceError::MissingEntity {
            entity: source.clone(),
        })?
        .clone();
    preflight_formula_structures(document)?;

    let mut edited = document.clone();
    edited
        .entities
        .get_mut(&entity_id)
        .ok_or(WorkspaceError::MissingEntity { entity: source })?
        .key = target;
    finalize_edit(document, edited)
}

/// Rename a schema's mutable key while preserving its stable ID.
///
/// # Errors
///
/// Returns a typed key, validation, calculation, or diff error.
pub fn rename_schema(
    document: &Document,
    source: impl AsRef<str>,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkspaceError> {
    let source = SchemaKey::from(source.as_ref());
    let target = SchemaKey::from(target.as_ref());
    if source == target {
        return Err(WorkspaceError::NoOpSchemaRename { schema: source });
    }
    validate_key(target.as_str()).map_err(|()| WorkspaceError::InvalidSchemaKey {
        schema: target.clone(),
    })?;
    let index = AddressIndex::build(document)?;
    if index.schema_id(&target).is_ok() {
        return Err(WorkspaceError::SchemaKeyAlreadyExists { schema: target });
    }
    let schema_id = index
        .schema_id(&source)
        .map_err(|_| WorkspaceError::MissingSchemaKey {
            schema: source.clone(),
        })?
        .clone();
    preflight_formula_structures(document)?;

    let mut edited = document.clone();
    edited
        .schemas
        .get_mut(&schema_id)
        .ok_or(WorkspaceError::MissingSchemaKey { schema: source })?
        .key = target;
    finalize_edit(document, edited)
}

/// Rename a field's mutable key in one schema while preserving its stable ID.
///
/// # Errors
///
/// Returns a typed key, projection, validation, calculation, or diff error.
pub fn rename_field(
    document: &Document,
    schema: impl AsRef<str>,
    source: impl AsRef<str>,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkspaceError> {
    let schema_key = SchemaKey::from(schema.as_ref());
    let source = FieldKey::from(source.as_ref());
    let target = FieldKey::from(target.as_ref());
    if source == target {
        return Err(WorkspaceError::NoOpFieldRename { field: source });
    }
    validate_key(target.as_str()).map_err(|()| WorkspaceError::InvalidFieldKey {
        schema: schema_key.clone(),
        field: target.clone(),
    })?;
    let index = AddressIndex::build(document)?;
    let schema_id = index
        .schema_id(&schema_key)
        .map_err(|_| WorkspaceError::MissingSchemaKey {
            schema: schema_key.clone(),
        })?
        .clone();
    if index.schema_field_id(&schema_id, &target).is_ok() {
        return Err(WorkspaceError::FieldKeyAlreadyExists {
            schema: schema_key.clone(),
            field: target.clone(),
        });
    }
    let field_id = index
        .schema_field_id(&schema_id, &source)
        .map_err(|_| WorkspaceError::MissingFieldKey {
            schema: schema_key.clone(),
            field: source.clone(),
        })?
        .clone();
    preflight_formula_structures(document)?;

    let mut edited = document.clone();
    let edited_schema =
        edited
            .schemas
            .get_mut(&schema_id)
            .ok_or_else(|| WorkspaceError::MissingSchemaKey {
                schema: schema_key.clone(),
            })?;
    edited_schema
        .fields
        .get_mut(&field_id)
        .ok_or(WorkspaceError::MissingFieldKey {
            schema: schema_key,
            field: source,
        })?
        .key = target;
    finalize_edit(document, edited)
}

/// Remove an entity only when no other stored or bound field targets its
/// stable ID.
///
/// # Errors
///
/// Returns a typed lookup, dependency, validation, calculation, or diff error.
pub fn remove_entity(
    document: &Document,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkspaceError> {
    let target_key = EntityKey::from(target.as_ref());
    let target_id = AddressIndex::build(document)?
        .entity_id(&target_key)
        .map_err(|_| WorkspaceError::MissingEntity {
            entity: target_key.clone(),
        })?
        .clone();
    preflight_formula_structures(document)?;

    let dependents = document
        .entities
        .iter()
        .filter(|(entity_id, _)| *entity_id != &target_id)
        .flat_map(|(entity_id, entity)| {
            entity
                .fields
                .iter()
                .filter(|(_, value)| value_references_entity(value, &target_id))
                .map(|(field_id, _)| FieldRef::new(entity_id.clone(), field_id.clone()))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if !dependents.is_empty() {
        let index = AddressIndex::build(document)?;
        let mut dependent_addresses = dependents
            .iter()
            .map(|dependent| index.field_address(document, dependent))
            .collect::<Result<Vec<_>, _>>()?;
        dependent_addresses.sort();
        return Err(WorkspaceError::EntityReferenced {
            entity: target_key,
            dependents,
            dependent_addresses,
        });
    }

    let mut edited = document.clone();
    edited.entities.remove(&target_id);
    finalize_edit(document, edited)
}

fn validate_new_entity_key(document: &Document, target: &EntityKey) -> Result<(), WorkspaceError> {
    if !is_valid_identifier(target.as_str()) {
        return Err(WorkspaceError::InvalidEntityKey {
            entity: target.clone(),
        });
    }
    let index = AddressIndex::build(document)?;
    if index.entity_id(target).is_ok() {
        return Err(WorkspaceError::EntityKeyAlreadyExists {
            entity: target.clone(),
        });
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), ()> {
    is_valid_identifier(key).then_some(()).ok_or(())
}

fn rewrite_expression_entity(expression: &mut Expression, source: &EntityId, target: &EntityId) {
    match expression {
        Expression::Number(_) => {}
        Expression::Reference(reference) => {
            if &reference.entity == source {
                reference.entity = target.clone();
            }
        }
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right }
        | Expression::Divide { left, right }
        | Expression::Minimum { left, right }
        | Expression::Maximum { left, right } => {
            rewrite_expression_entity(left, source, target);
            rewrite_expression_entity(right, source, target);
        }
    }
}

fn value_references_entity(value: &Value, target: &EntityId) -> bool {
    match value {
        Value::Reference(reference) => reference == target,
        Value::Formula(expression) => expression_references_entity(expression, target),
        Value::Number(_) | Value::Text(_) | Value::Boolean(_) => false,
    }
}

fn expression_references_entity(expression: &Expression, target: &EntityId) -> bool {
    match expression {
        Expression::Number(_) => false,
        Expression::Reference(reference) => &reference.entity == target,
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right }
        | Expression::Divide { left, right }
        | Expression::Minimum { left, right }
        | Expression::Maximum { left, right } => {
            expression_references_entity(left, target)
                || expression_references_entity(right, target)
        }
    }
}

pub(crate) fn field_value_candidate(
    document: &Document,
    field: &FieldRef,
    value: &Value,
) -> Result<Document, WorkspaceError> {
    let entity =
        document
            .entities
            .get(&field.entity)
            .ok_or_else(|| WorkspaceError::MissingEntityId {
                entity: field.entity.clone(),
            })?;
    let existing = entity
        .fields
        .get(&field.field)
        .ok_or_else(|| WorkspaceError::MissingField {
            field: field.clone(),
        })?;
    if !matches!(existing, Value::Formula(_)) && existing == value {
        return Err(WorkspaceError::NoChange {
            field: field.clone(),
        });
    }
    let definition = field_definition(document, field)?;
    field_value_input_rule(
        field,
        existing,
        semantic_value_kind(value),
        &definition.field_type,
    )?;
    preflight_formula_structures(document)?;
    if let Value::Formula(expression) = value {
        validate_expression_structure(expression).map_err(|source| {
            WorkspaceError::ExpressionComplexity {
                field: field.clone(),
                source,
            }
        })?;
    }
    if existing == value {
        return Err(WorkspaceError::NoChange {
            field: field.clone(),
        });
    }
    if let Value::Formula(expression) = value {
        project_expression(document, expression).map_err(|source| match source {
            CanonicalAuthoringProjectionError::Complexity(source) => {
                WorkspaceError::ExpressionComplexity {
                    field: field.clone(),
                    source,
                }
            }
            source @ CanonicalAuthoringProjectionError::UnresolvableBoundReferences { .. } => {
                WorkspaceError::FormulaProjection {
                    field: field.clone(),
                    source,
                }
            }
        })?;
    }

    let mut candidate = document.clone();
    candidate
        .entities
        .get_mut(&field.entity)
        .ok_or_else(|| WorkspaceError::MissingEntityId {
            entity: field.entity.clone(),
        })?
        .fields
        .insert(field.field.clone(), value.clone());
    Ok(candidate)
}

/// Classify one existing semantic value without exposing its payload.
pub(crate) fn semantic_value_kind(value: &Value) -> SemanticValueKind {
    match value {
        Value::Number(_) => SemanticValueKind::Number,
        Value::Formula(_) => SemanticValueKind::Formula,
        Value::Text(_) => SemanticValueKind::Text,
        Value::Boolean(_) => SemanticValueKind::Boolean,
        Value::Reference(_) => SemanticValueKind::Reference,
    }
}

/// Authoritative finite value-kind/type matching used by field mutations and
/// capability discovery.
pub(crate) fn value_matches_type(value_kind: SemanticValueKind, field_type: &FieldType) -> bool {
    matches!(
        (value_kind, field_type),
        (
            SemanticValueKind::Number | SemanticValueKind::Formula,
            FieldType::Number
        ) | (SemanticValueKind::Text, FieldType::Text)
            | (SemanticValueKind::Boolean, FieldType::Boolean)
            | (SemanticValueKind::Reference, FieldType::Reference { .. })
    )
}

/// Apply the shared current-formula and typed-value rules before a field value
/// candidate is constructed. Capability discovery calls this exact rule with
/// an input kind, while mutation calls it with the candidate's classified kind.
pub(crate) fn field_value_input_rule(
    field: &FieldRef,
    existing: &Value,
    input_kind: SemanticValueKind,
    field_type: &FieldType,
) -> Result<(), WorkspaceError> {
    if matches!(existing, Value::Formula(_)) && input_kind != SemanticValueKind::Formula {
        return Err(WorkspaceError::FormulaEdit {
            field: field.clone(),
        });
    }
    if !value_matches_type(input_kind, field_type) {
        return Err(WorkspaceError::TypeMismatch {
            field: field.clone(),
        });
    }
    Ok(())
}

pub(crate) fn field_definition<'document>(
    document: &'document Document,
    field: &FieldRef,
) -> Result<&'document FieldDefinition, WorkspaceError> {
    let entity =
        document
            .entities
            .get(&field.entity)
            .ok_or_else(|| WorkspaceError::MissingEntityId {
                entity: field.entity.clone(),
            })?;
    let schema =
        document
            .schemas
            .get(&entity.schema)
            .ok_or_else(|| WorkspaceError::MissingSchema {
                schema: entity.schema.clone(),
            })?;
    schema
        .fields
        .get(&field.field)
        .ok_or_else(|| WorkspaceError::MissingField {
            field: field.clone(),
        })
}

/// Authoritative target rule for every typed `FormulaUpdate` admission.
pub(crate) fn formula_update_target_rule(
    document: &Document,
    field: &FieldRef,
) -> Result<(), WorkspaceError> {
    if field_definition(document, field)?.field_type != FieldType::Number {
        return Err(WorkspaceError::NonNumericFormulaField {
            field: field.clone(),
        });
    }
    Ok(())
}

fn finalize_edit(document: &Document, edited: Document) -> Result<EditPreview, WorkspaceError> {
    require_validated_calculation_for(&edited, ValidationRole::Candidate)?;
    preflight_formula_projections(&edited)?;
    let semantic_diff = diff(document, &edited)?;
    Ok(EditPreview {
        document: edited,
        diff: semantic_diff,
    })
}

/// Build the authoritative full semantic validation report.
#[must_use]
pub fn validation_report(document: &Document) -> ValidationReport {
    semantic_validation(document).0
}

fn semantic_validation(document: &Document) -> (ValidationReport, Option<Calculation>) {
    let calculation_outcome = calculate_complete(document);
    let report = validation_report_for_calculation(document, &calculation_outcome);
    let calculation = match calculation_outcome {
        CalculationOutcome::Complete(calculation) if report.is_valid() => Some(calculation),
        CalculationOutcome::Complete(_) | CalculationOutcome::Failed(_) => None,
    };
    (report, calculation)
}

pub(crate) fn validation_report_for_calculation(
    document: &Document,
    calculation: &CalculationOutcome,
) -> ValidationReport {
    let failures = match calculation {
        CalculationOutcome::Complete(_) => None,
        CalculationOutcome::Failed(failures) => Some(failures.failures()),
    };
    validation_report_for_failures(document, failures)
}

pub(crate) fn validation_report_for_retained_calculation(
    document: &Document,
    calculation: &RetainedCalculationState,
) -> ValidationReport {
    validation_report_for_failures(
        document,
        calculation.is_failed().then_some(calculation.failures()),
    )
}

fn validation_report_for_failures(
    document: &Document,
    failures: Option<&BTreeMap<FieldRef, CalculationFailure>>,
) -> ValidationReport {
    let mut diagnostics = validate_document_core(document);
    let core_diagnostics = diagnostics.clone();
    if let Some(failures) = failures {
        diagnostics.extend(formula_diagnostics(document, failures, &core_diagnostics));
    }
    ValidationReport::new(diagnostics)
}

fn require_validated_calculation(document: &Document) -> Result<Calculation, WorkspaceError> {
    require_validated_calculation_for(document, ValidationRole::Current)
}

fn require_validated_calculation_for(
    document: &Document,
    role: ValidationRole,
) -> Result<Calculation, WorkspaceError> {
    let (report, calculation) = semantic_validation(document);
    if !report.is_valid() {
        return Err(invalid_document(report, role));
    }
    Ok(calculation.expect("a diagnostic-free formula outcome is complete"))
}

fn invalid_document(report: ValidationReport, role: ValidationRole) -> WorkspaceError {
    WorkspaceError::InvalidDocument {
        role,
        summary: format_diagnostics(report.diagnostics()),
        report,
    }
}

const MERGE_PROVIDER: DiagnosticProvider = DiagnosticProvider::new("tachiko.merge-engine");

fn unmaterialized_qualified_field_diagnostic(fact: &UnmaterializedStoredFact) -> Diagnostic {
    Diagnostic::new(
        diagnostic_codes::MERGE_UNMATERIALIZED_QUALIFIED_FIELD,
        DiagnosticSeverity::Error,
        vec![SemanticSubject::EntityField(FieldRef::new(
            fact.entity().clone(),
            fact.field().clone(),
        ))],
        MERGE_PROVIDER,
    )
    .with_related_subjects(vec![
        SemanticSubject::SchemaField {
            schema: fact.source_schema().clone(),
            field: fact.field().clone(),
        },
        SemanticSubject::SchemaField {
            schema: fact.selected_schema().clone(),
            field: fact.field().clone(),
        },
    ])
    .with_fact(DiagnosticFact::new(
        "source_schema",
        fact.source_schema().as_str(),
    ))
    .with_fact(DiagnosticFact::new(
        "selected_schema",
        fact.selected_schema().as_str(),
    ))
    .with_presentation(
        format!("entities.{}.fields.{}", fact.entity(), fact.field()),
        format!(
            "field '{}' selected under schema '{}' cannot be represented after schema '{}' was selected",
            fact.field(),
            fact.source_schema(),
            fact.selected_schema()
        ),
    )
}

const FORMULA_PROVIDER: DiagnosticProvider = DiagnosticProvider::new("tachiko.formula-engine");

fn formula_diagnostics(
    document: &Document,
    failures: &BTreeMap<FieldRef, CalculationFailure>,
    core_diagnostics: &[Diagnostic],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut surviving_failures = BTreeSet::new();
    let mut pending_failed_dependencies = Vec::new();
    let blockers = formula_prerequisite_blockers(core_diagnostics);
    let address_index = AddressIndex::build(document).ok();
    for (formula, failure) in failures {
        if is_noncanonical_cycle(formula, failure) {
            continue;
        }
        if !formula_prerequisites_available(document, formula, failure, &blockers) {
            continue;
        }
        let path = formula_path(document, address_index.as_ref(), formula);
        if let CalculationFailure::FailedDependencies { dependencies } = failure {
            pending_failed_dependencies.push((formula, path, dependencies));
            continue;
        }
        let subject = SemanticSubject::EntityField(formula.clone());
        let diagnostic = match failure {
            CalculationFailure::InvalidExpression { error } => formula_diagnostic(
                diagnostic_codes::FORMULA_STRUCTURAL,
                vec![subject],
                path,
                format!("formula '{formula}' violates the bound expression contract"),
            )
            .with_fact(DiagnosticFact::new(
                "limit",
                match error {
                    ExpressionComplexityError::NodeLimit => "node_limit",
                    ExpressionComplexityError::DepthLimit => "depth_limit",
                    ExpressionComplexityError::CanonicalLengthLimit => "canonical_length_limit",
                },
            )),
            CalculationFailure::InvalidReferences { targets } => {
                let Some(diagnostic) = project_invalid_reference_diagnostic(
                    document, formula, path, targets, &blockers,
                ) else {
                    continue;
                };
                diagnostic
            }
            CalculationFailure::Cycle { members } => formula_diagnostic(
                diagnostic_codes::FORMULA_CYCLE,
                members
                    .iter()
                    .cloned()
                    .map(SemanticSubject::EntityField)
                    .collect(),
                path,
                format!("formula dependency cycle contains {} values", members.len()),
            ),
            CalculationFailure::FailedDependencies { .. } => {
                unreachable!("failed dependencies are projected after primary failures")
            }
            CalculationFailure::DivisionByZero => formula_diagnostic(
                diagnostic_codes::FORMULA_DIVISION_BY_ZERO,
                vec![subject],
                path,
                format!("formula '{formula}' divided by zero"),
            ),
            CalculationFailure::NonFiniteResult => formula_diagnostic(
                diagnostic_codes::FORMULA_NON_FINITE_RESULT,
                vec![subject],
                path,
                format!("formula '{formula}' produced a non-finite result"),
            ),
        };
        if let CalculationFailure::Cycle { members } = failure {
            surviving_failures.extend(members.iter().cloned());
        } else {
            surviving_failures.insert(formula.clone());
        }
        diagnostics.push(diagnostic);
    }

    propagate_surviving_failed_dependencies(&pending_failed_dependencies, &mut surviving_failures);
    for (formula, path, dependencies) in pending_failed_dependencies {
        if let Some(diagnostic) =
            project_failed_dependency_diagnostic(formula, &path, dependencies, &surviving_failures)
        {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn propagate_surviving_failed_dependencies(
    pending: &[(&FieldRef, String, &BTreeSet<FieldRef>)],
    surviving_failures: &mut BTreeSet<FieldRef>,
) {
    let mut dependents_by_failure: BTreeMap<&FieldRef, Vec<&FieldRef>> = BTreeMap::new();
    for (formula, _, dependencies) in pending {
        for dependency in *dependencies {
            dependents_by_failure
                .entry(dependency)
                .or_default()
                .push(formula);
        }
    }

    let mut queue = surviving_failures.iter().cloned().collect::<VecDeque<_>>();
    while let Some(failure) = queue.pop_front() {
        let Some(dependents) = dependents_by_failure.get(&failure) else {
            continue;
        };
        for dependent in dependents {
            if surviving_failures.insert((*dependent).clone()) {
                queue.push_back((*dependent).clone());
            }
        }
    }
}

fn is_noncanonical_cycle(formula: &FieldRef, failure: &CalculationFailure) -> bool {
    matches!(failure, CalculationFailure::Cycle { members } if members.first() != Some(formula))
}

fn project_invalid_reference_diagnostic(
    document: &Document,
    formula: &FieldRef,
    path: String,
    targets: &BTreeMap<FieldRef, ReferenceFailure>,
    blockers: &FormulaPrerequisiteBlockers,
) -> Option<Diagnostic> {
    let mut missing = BTreeSet::new();
    let mut non_numeric = BTreeSet::new();
    for (target, failure) in targets {
        if !invalid_reference_prerequisites_available(document, target, *failure, blockers) {
            continue;
        }
        match failure {
            ReferenceFailure::Missing => {
                missing.insert(target.clone());
            }
            ReferenceFailure::NonNumeric => {
                non_numeric.insert(target.clone());
            }
        }
    }
    if missing.is_empty() && non_numeric.is_empty() {
        return None;
    }
    Some(invalid_reference_diagnostic(
        formula,
        SemanticSubject::EntityField(formula.clone()),
        path,
        &missing,
        &non_numeric,
    ))
}

fn project_failed_dependency_diagnostic(
    formula: &FieldRef,
    path: &str,
    dependencies: &BTreeSet<FieldRef>,
    surviving_failures: &BTreeSet<FieldRef>,
) -> Option<Diagnostic> {
    let dependencies = dependencies
        .intersection(surviving_failures)
        .cloned()
        .collect::<BTreeSet<_>>();
    if dependencies.is_empty() {
        return None;
    }
    Some(
        formula_diagnostic(
            diagnostic_codes::FORMULA_FAILED_DEPENDENCY,
            vec![SemanticSubject::EntityField(formula.clone())],
            path.to_owned(),
            format!("formula '{formula}' directly depends on failed values"),
        )
        .with_related_subjects(
            dependencies
                .into_iter()
                .map(SemanticSubject::EntityField)
                .collect(),
        ),
    )
}

fn invalid_reference_diagnostic(
    formula: &FieldRef,
    subject: SemanticSubject,
    path: String,
    missing: &BTreeSet<FieldRef>,
    non_numeric: &BTreeSet<FieldRef>,
) -> Diagnostic {
    let related = missing
        .union(non_numeric)
        .cloned()
        .map(SemanticSubject::EntityField)
        .collect();
    let mut diagnostic = formula_diagnostic(
        diagnostic_codes::FORMULA_INVALID_REFERENCES,
        vec![subject],
        path,
        format!("formula '{formula}' has invalid stable references"),
    )
    .with_related_subjects(related);
    for target in missing {
        diagnostic = diagnostic.with_fact(DiagnosticFact::new(
            "missing_target",
            field_ref_fact(target),
        ));
    }
    for target in non_numeric {
        diagnostic = diagnostic.with_fact(DiagnosticFact::new(
            "non_numeric_target",
            field_ref_fact(target),
        ));
    }
    diagnostic
}

fn formula_diagnostic(
    code: DiagnosticCode,
    subjects: Vec<SemanticSubject>,
    path: String,
    message: String,
) -> Diagnostic {
    Diagnostic::new(code, DiagnosticSeverity::Error, subjects, FORMULA_PROVIDER)
        .with_presentation(path, message)
}

fn formula_prerequisites_available(
    document: &Document,
    formula: &FieldRef,
    failure: &CalculationFailure,
    blockers: &FormulaPrerequisiteBlockers,
) -> bool {
    if !field_prerequisites_available(document, formula, &blockers.values) {
        return false;
    }
    match failure {
        CalculationFailure::InvalidExpression { .. }
        | CalculationFailure::InvalidReferences { .. }
        | CalculationFailure::FailedDependencies { .. }
        | CalculationFailure::DivisionByZero
        | CalculationFailure::NonFiniteResult => true,
        CalculationFailure::Cycle { members } => members
            .iter()
            .all(|member| field_prerequisites_available(document, member, &blockers.values)),
    }
}

fn invalid_reference_prerequisites_available(
    document: &Document,
    target: &FieldRef,
    failure: ReferenceFailure,
    blockers: &FormulaPrerequisiteBlockers,
) -> bool {
    let blocked_subjects = match failure {
        ReferenceFailure::Missing => &blockers.values,
        ReferenceFailure::NonNumeric
            if document
                .entities
                .get(&target.entity)
                .and_then(|entity| document.schemas.get(&entity.schema))
                .and_then(|schema| schema.fields.get(&target.field))
                .is_some_and(|definition| definition.field_type == FieldType::Number) =>
        {
            &blockers.values
        }
        ReferenceFailure::NonNumeric => &blockers.declarations,
    };
    field_prerequisites_available(document, target, blocked_subjects)
}

fn field_prerequisites_available(
    document: &Document,
    field: &FieldRef,
    blocked_subjects: &BTreeSet<SemanticSubject>,
) -> bool {
    let mut subjects = vec![
        SemanticSubject::Entity(field.entity.clone()),
        SemanticSubject::EntityField(field.clone()),
    ];
    if let Some(entity) = document.entities.get(&field.entity) {
        subjects.push(SemanticSubject::Schema(entity.schema.clone()));
        subjects.push(SemanticSubject::SchemaField {
            schema: entity.schema.clone(),
            field: field.field.clone(),
        });
    }
    subjects
        .iter()
        .all(|subject| !blocked_subjects.contains(subject))
}

struct FormulaPrerequisiteBlockers {
    values: BTreeSet<SemanticSubject>,
    declarations: BTreeSet<SemanticSubject>,
}

fn formula_prerequisite_blockers(core_diagnostics: &[Diagnostic]) -> FormulaPrerequisiteBlockers {
    let mut blockers = FormulaPrerequisiteBlockers {
        values: BTreeSet::new(),
        declarations: BTreeSet::new(),
    };
    for diagnostic in core_diagnostics {
        if core_diagnostic_blocks_formula_value(diagnostic.code) {
            blockers.values.extend(diagnostic.subjects.iter().cloned());
        }
        if [
            DiagnosticCode::EMPTY_STABLE_ID,
            DiagnosticCode::KEY_MISMATCH,
        ]
        .contains(&diagnostic.code)
        {
            blockers
                .declarations
                .extend(diagnostic.subjects.iter().cloned());
        } else if diagnostic.code == DiagnosticCode::MISSING_SCHEMA {
            blockers.declarations.extend(
                diagnostic
                    .subjects
                    .iter()
                    .filter(|subject| matches!(subject, SemanticSubject::Entity(_)))
                    .cloned(),
            );
        }
    }
    blockers
}

fn core_diagnostic_blocks_formula_value(code: DiagnosticCode) -> bool {
    [
        DiagnosticCode::EMPTY_STABLE_ID,
        DiagnosticCode::KEY_MISMATCH,
        DiagnosticCode::MISSING_SCHEMA,
        DiagnosticCode::MISSING_REQUIRED_FIELD,
        DiagnosticCode::UNEXPECTED_FIELD,
        DiagnosticCode::TYPE_MISMATCH,
        DiagnosticCode::MISSING_REFERENCE,
        DiagnosticCode::REFERENCE_TYPE_MISMATCH,
    ]
    .contains(&code)
}

fn formula_path(document: &Document, index: Option<&AddressIndex>, formula: &FieldRef) -> String {
    index
        .and_then(|index| index.field_address(document, formula).ok())
        .map_or_else(
            || format!("entities.{}.fields.{}", formula.entity, formula.field),
            |address| format!("formulas.{address}"),
        )
}

fn field_ref_fact(field: &FieldRef) -> String {
    format!(
        "{}:{}{}:{}",
        field.entity.as_str().len(),
        field.entity,
        field.field.as_str().len(),
        field.field
    )
}

fn preflight_formula_projections(document: &Document) -> Result<(), WorkspaceError> {
    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if let Value::Formula(expression) = value {
                let field = FieldRef::new(entity_id.clone(), field_id.clone());
                project_expression(document, expression).map_err(|source| {
                    WorkspaceError::FormulaProjection {
                        field: field.clone(),
                        source,
                    }
                })?;
            }
        }
    }
    Ok(())
}

fn preflight_formula_structures(document: &Document) -> Result<(), WorkspaceError> {
    for (entity_id, entity) in &document.entities {
        for (field_id, value) in &entity.fields {
            if let Value::Formula(expression) = value {
                validate_expression_structure(expression).map_err(|source| {
                    WorkspaceError::ExpressionComplexity {
                        field: FieldRef::new(entity_id.clone(), field_id.clone()),
                        source,
                    }
                })?;
            }
        }
    }
    Ok(())
}

fn drop_expression_iteratively(expression: Expression) {
    let mut stack = vec![expression];
    while let Some(expression) = stack.pop() {
        match expression {
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                stack.push(*right);
                stack.push(*left);
            }
            Expression::Number(_) | Expression::Reference(_) => {}
        }
    }
}

fn drop_value_iteratively(value: Value) {
    match value {
        Value::Formula(expression) => drop_expression_iteratively(expression),
        Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Reference(_) => {}
    }
}

fn format_dependent_addresses(dependents: &[FieldAddress]) -> String {
    dependents
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn field_value<'document>(
    document: &'document Document,
    field: &FieldRef,
) -> Result<&'document Value, WorkspaceError> {
    document
        .entities
        .get(&field.entity)
        .and_then(|entity| entity.fields.get(&field.field))
        .ok_or_else(|| WorkspaceError::MissingField {
            field: field.clone(),
        })
}

fn parse_scalar(
    document: &Document,
    field: &FieldRef,
    input: &str,
    field_type: &FieldType,
) -> Result<Value, WorkspaceError> {
    let invalid = |expected| WorkspaceError::InvalidValue {
        field: field.clone(),
        input: input.to_owned(),
        expected,
    };
    match field_type {
        FieldType::Number => input
            .parse::<f64>()
            .map_err(|_| invalid("number"))
            .and_then(|number| Number::new(number).map_err(|_| invalid("finite number")))
            .map(Value::Number),
        FieldType::Text => Ok(Value::Text(input.to_owned())),
        FieldType::Boolean => input
            .parse::<bool>()
            .map(Value::Boolean)
            .map_err(|_| invalid("boolean (true or false)")),
        FieldType::Reference { .. } => {
            let target = AddressIndex::build(document)?
                .entity_id(&EntityKey::from(input))
                .map_err(|_| invalid("existing entity key"))?
                .clone();
            Ok(Value::Reference(target))
        }
    }
}

fn runtime_value(
    document: &Document,
    value: &Value,
    field: &FieldRef,
    calculation: &Calculation,
) -> Result<RuntimeValue, WorkspaceError> {
    match value {
        Value::Number(number) => Ok(RuntimeValue::Number(*number)),
        Value::Text(text) => Ok(RuntimeValue::Text(text.clone())),
        Value::Boolean(boolean) => Ok(RuntimeValue::Boolean(*boolean)),
        Value::Reference(entity) => {
            let target =
                document
                    .entities
                    .get(entity)
                    .ok_or_else(|| WorkspaceError::MissingEntityId {
                        entity: entity.clone(),
                    })?;
            Ok(RuntimeValue::Reference {
                reference: target.key.to_string(),
            })
        }
        Value::Formula(_) => calculation
            .value(field)
            .map(RuntimeValue::Number)
            .ok_or_else(|| WorkspaceError::MissingCalculation {
                field: field.clone(),
            }),
    }
}

fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn next_id(
    generator: &mut impl IdGenerator,
    kind: SemanticIdKind,
) -> Result<String, WorkspaceError> {
    let id = generator.generate(kind);
    if id.is_empty() {
        Err(WorkspaceError::EmptyGeneratedId { kind })
    } else {
        Ok(id)
    }
}

fn next_document_id(generator: &mut impl IdGenerator) -> Result<DocumentId, WorkspaceError> {
    next_id(generator, SemanticIdKind::Document).map(DocumentId::from)
}

fn next_schema_id(generator: &mut impl IdGenerator) -> Result<SchemaId, WorkspaceError> {
    next_id(generator, SemanticIdKind::Schema).map(SchemaId::from)
}

fn next_field_id(generator: &mut impl IdGenerator) -> Result<FieldId, WorkspaceError> {
    next_id(generator, SemanticIdKind::Field).map(FieldId::from)
}

fn next_entity_id(generator: &mut impl IdGenerator) -> Result<EntityId, WorkspaceError> {
    next_id(generator, SemanticIdKind::Entity).map(EntityId::from)
}

fn game_balance_document(
    id: DocumentId,
    title: String,
    generator: &mut impl IdGenerator,
) -> Result<Document, WorkspaceError> {
    let schema_ids = game_balance_schema_ids(generator)?;
    let (schemas, field_ids) = game_balance_schemas(generator, &schema_ids)?;
    let entity_ids = game_balance_entity_ids(generator)?;
    let entities = game_balance_entities(&schema_ids, &field_ids, &entity_ids);

    Ok(Document {
        id,
        title,
        schemas,
        entities,
    })
}

type NamedSchemaIds = BTreeMap<&'static str, SchemaId>;
type NamedFieldIds = BTreeMap<(&'static str, &'static str), FieldId>;
type NamedEntityIds = BTreeMap<&'static str, EntityId>;

fn game_balance_schema_ids(
    generator: &mut impl IdGenerator,
) -> Result<NamedSchemaIds, WorkspaceError> {
    let mut schema_ids = BTreeMap::new();
    for key in ["characters", "economy", "items", "weapons"] {
        let id = next_schema_id(generator)?;
        if schema_ids.values().any(|existing| existing == &id) {
            return Err(WorkspaceError::GeneratedIdCollision {
                kind: SemanticIdKind::Schema,
                id: id.to_string(),
            });
        }
        schema_ids.insert(key, id);
    }
    Ok(schema_ids)
}

fn game_balance_schema_specs(
    schema_ids: &NamedSchemaIds,
) -> [(&'static str, Vec<(&'static str, FieldType)>); 4] {
    [
        (
            "characters",
            vec![
                ("level", FieldType::Number),
                ("name", FieldType::Text),
                (
                    "weapon",
                    FieldType::Reference {
                        schema: schema_ids["weapons"].clone(),
                    },
                ),
            ],
        ),
        (
            "economy",
            vec![
                ("currency", FieldType::Text),
                ("gold_per_match", FieldType::Number),
                ("matches_for_sword", FieldType::Number),
                ("upgrade_cost", FieldType::Number),
            ],
        ),
        (
            "items",
            vec![
                ("category", FieldType::Text),
                (
                    "grants_weapon",
                    FieldType::Reference {
                        schema: schema_ids["weapons"].clone(),
                    },
                ),
                ("name", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
        (
            "weapons",
            vec![
                ("attack_interval", FieldType::Number),
                ("damage", FieldType::Number),
                ("dps", FieldType::Number),
                ("name", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
    ]
}

fn game_balance_schemas(
    generator: &mut impl IdGenerator,
    schema_ids: &NamedSchemaIds,
) -> Result<(BTreeMap<SchemaId, Schema>, NamedFieldIds), WorkspaceError> {
    let mut schemas = BTreeMap::new();
    let mut field_ids = NamedFieldIds::new();
    for (schema_key, fields) in game_balance_schema_specs(schema_ids) {
        let schema_id = schema_ids[schema_key].clone();
        let mut definitions = BTreeMap::new();
        for (field_key, field_type) in fields {
            let field_id = next_field_id(generator)?;
            if definitions.contains_key(&field_id) {
                return Err(WorkspaceError::GeneratedIdCollision {
                    kind: SemanticIdKind::Field,
                    id: field_id.to_string(),
                });
            }
            field_ids.insert((schema_key, field_key), field_id.clone());
            definitions.insert(
                field_id.clone(),
                FieldDefinition {
                    id: field_id,
                    key: FieldKey::from(field_key),
                    field_type,
                    required: true,
                },
            );
        }
        schemas.insert(
            schema_id.clone(),
            Schema {
                id: schema_id,
                key: SchemaKey::from(schema_key),
                fields: definitions,
            },
        );
    }
    Ok((schemas, field_ids))
}

fn game_balance_entity_ids(
    generator: &mut impl IdGenerator,
) -> Result<NamedEntityIds, WorkspaceError> {
    let mut entity_ids = BTreeMap::new();
    for key in ["alric", "iron_sword", "shop", "tempered_blade"] {
        let id = next_entity_id(generator)?;
        if entity_ids.values().any(|existing| existing == &id) {
            return Err(WorkspaceError::GeneratedIdCollision {
                kind: SemanticIdKind::Entity,
                id: id.to_string(),
            });
        }
        entity_ids.insert(key, id);
    }
    Ok(entity_ids)
}

fn game_balance_entities(
    schema_ids: &NamedSchemaIds,
    field_ids: &NamedFieldIds,
    entity_ids: &NamedEntityIds,
) -> BTreeMap<EntityId, Entity> {
    let mut entities = BTreeMap::new();
    insert_entity(
        &mut entities,
        entity_ids,
        "alric",
        &schema_ids["characters"],
        alric_fields(field_ids, entity_ids),
    );
    insert_entity(
        &mut entities,
        entity_ids,
        "iron_sword",
        &schema_ids["weapons"],
        iron_sword_fields(field_ids, entity_ids),
    );
    insert_entity(
        &mut entities,
        entity_ids,
        "shop",
        &schema_ids["economy"],
        shop_fields(field_ids, entity_ids),
    );
    insert_entity(
        &mut entities,
        entity_ids,
        "tempered_blade",
        &schema_ids["items"],
        tempered_blade_fields(field_ids, entity_ids),
    );
    entities
}

fn alric_fields(field_ids: &NamedFieldIds, entity_ids: &NamedEntityIds) -> Vec<(FieldId, Value)> {
    vec![
        (field_ids[&("characters", "level")].clone(), number(4.0)),
        (
            field_ids[&("characters", "name")].clone(),
            Value::Text("Alric".to_owned()),
        ),
        (
            field_ids[&("characters", "weapon")].clone(),
            Value::Reference(entity_ids["iron_sword"].clone()),
        ),
    ]
}

fn iron_sword_fields(
    field_ids: &NamedFieldIds,
    entity_ids: &NamedEntityIds,
) -> Vec<(FieldId, Value)> {
    vec![
        (
            field_ids[&("weapons", "attack_interval")].clone(),
            number(0.9),
        ),
        (field_ids[&("weapons", "damage")].clone(), number(36.0)),
        (
            field_ids[&("weapons", "dps")].clone(),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Reference(FieldRef::new(
                    entity_ids["iron_sword"].clone(),
                    field_ids[&("weapons", "damage")].clone(),
                ))),
                right: Box::new(Expression::Reference(FieldRef::new(
                    entity_ids["iron_sword"].clone(),
                    field_ids[&("weapons", "attack_interval")].clone(),
                ))),
            }),
        ),
        (
            field_ids[&("weapons", "name")].clone(),
            Value::Text("Iron Sword".to_owned()),
        ),
        (field_ids[&("weapons", "price")].clone(), number(120.0)),
    ]
}

fn shop_fields(field_ids: &NamedFieldIds, entity_ids: &NamedEntityIds) -> Vec<(FieldId, Value)> {
    vec![
        (
            field_ids[&("economy", "currency")].clone(),
            Value::Text("gold".to_owned()),
        ),
        (
            field_ids[&("economy", "gold_per_match")].clone(),
            number(50.0),
        ),
        (
            field_ids[&("economy", "matches_for_sword")].clone(),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Reference(FieldRef::new(
                    entity_ids["iron_sword"].clone(),
                    field_ids[&("weapons", "price")].clone(),
                ))),
                right: Box::new(Expression::Reference(FieldRef::new(
                    entity_ids["shop"].clone(),
                    field_ids[&("economy", "gold_per_match")].clone(),
                ))),
            }),
        ),
        (
            field_ids[&("economy", "upgrade_cost")].clone(),
            Value::Formula(Expression::Reference(FieldRef::new(
                entity_ids["tempered_blade"].clone(),
                field_ids[&("items", "price")].clone(),
            ))),
        ),
    ]
}

fn tempered_blade_fields(
    field_ids: &NamedFieldIds,
    entity_ids: &NamedEntityIds,
) -> Vec<(FieldId, Value)> {
    vec![
        (
            field_ids[&("items", "category")].clone(),
            Value::Text("weapon upgrade".to_owned()),
        ),
        (
            field_ids[&("items", "grants_weapon")].clone(),
            Value::Reference(entity_ids["iron_sword"].clone()),
        ),
        (
            field_ids[&("items", "name")].clone(),
            Value::Text("Tempered Blade".to_owned()),
        ),
        (field_ids[&("items", "price")].clone(), number(200.0)),
    ]
}

fn insert_entity(
    entities: &mut BTreeMap<EntityId, Entity>,
    ids: &BTreeMap<&str, EntityId>,
    key: &'static str,
    schema: &SchemaId,
    fields: Vec<(FieldId, Value)>,
) {
    let id = ids[key].clone();
    entities.insert(
        id.clone(),
        Entity {
            id,
            key: EntityKey::from(key),
            schema: schema.clone(),
            fields: fields.into_iter().collect(),
        },
    );
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("starter constants are finite"))
}

fn entity_label(document: &Document, entity: &Entity) -> String {
    let name = document.schemas.get(&entity.schema).and_then(|schema| {
        schema
            .fields
            .values()
            .find(|definition| definition.key.as_str() == "name")
            .and_then(|definition| entity.fields.get(&definition.id))
    });
    match name {
        Some(Value::Text(name)) => name.clone(),
        _ => humanize(entity.key.as_str()),
    }
}

fn humanize(value: &str) -> String {
    value
        .split('_')
        .filter(|word| !word.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

fn format_value(document: &Document, value: &Value) -> String {
    match value {
        Value::Number(number) => format_number(*number),
        Value::Text(text) => text.clone(),
        Value::Boolean(boolean) => boolean.to_string(),
        Value::Reference(entity) => document.entities.get(entity).map_or_else(
            || format!("→ <missing:{entity}>"),
            |entity| format!("→ {}", entity.key),
        ),
        Value::Formula(_) => "formula".to_owned(),
    }
}

fn format_number(number: Number) -> String {
    let number = number.get();
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}
