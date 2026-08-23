//! Read-only semantic queries for Tachiko Work.

use tachiko_workspace_engine::{
    CalculationError, CanonicalAuthoringProjectionError, DiffError, Document, DocumentId, EntityId,
    EntityKey, Expression, ExpressionComplexityError, FieldId, FieldKey, FieldRef, FieldType,
    Number, SchemaId, SchemaKey, SemanticChange, ValidationReport, Value, WorkspaceError,
    analyze_formula as analyze_workspace_formula, compare_documents,
    validate_field_value_suggestion,
};
use thiserror::Error;

/// A deterministic projection of a document's semantic structure.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentDescription {
    pub id: DocumentId,
    pub title: String,
    pub schemas: Vec<SchemaDescription>,
    pub entities: Vec<EntityDescription>,
}

/// A schema and its sorted fields.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaDescription {
    pub id: SchemaId,
    pub key: SchemaKey,
    pub fields: Vec<FieldDescription>,
}

/// The structural constraints for one field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDescription {
    pub id: FieldId,
    pub key: FieldKey,
    pub field_type: FieldType,
    pub required: bool,
}

/// An entity and its sorted field identifiers.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityDescription {
    pub id: EntityId,
    pub key: EntityKey,
    pub schema: SchemaId,
    pub fields: Vec<FieldId>,
}

/// An evaluated formula together with its direct dependencies.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaExplanation {
    pub field: FieldRef,
    pub expression: Expression,
    pub value: Number,
    pub dependencies: Vec<FieldRef>,
}

/// A read-only semantic comparison of two document versions.
#[derive(Clone, Debug, PartialEq)]
pub struct ImpactExplanation {
    pub changes: Vec<SemanticChange>,
    pub summary: String,
}

/// A proposed field value that requires a separate approval and write path.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub field: FieldRef,
    pub value: Value,
    pub requires_approval: bool,
}

/// A proposed AI change is not safe enough to present for approval.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SuggestionError {
    #[error("entity '{entity}' does not exist")]
    MissingEntity { entity: EntityId },
    #[error("field '{field}' does not exist")]
    MissingField { field: FieldRef },
    #[error("field '{field}' is a formula; suggest a change to its inputs instead")]
    FormulaEdit { field: FieldRef },
    #[error("formula for '{field}' exceeds authoring complexity limits: {source}")]
    ExpressionComplexity {
        field: FieldRef,
        #[source]
        source: ExpressionComplexityError,
    },
    #[error("formula for '{field}' cannot be projected through current human addresses: {source}")]
    FormulaProjection {
        field: FieldRef,
        #[source]
        source: CanonicalAuthoringProjectionError,
    },
    #[error("value for '{field}' does not match its schema type")]
    TypeMismatch { field: FieldRef },
    #[error("'{field}' already has that value")]
    NoChange { field: FieldRef },
    #[error("suggestion would make the document invalid: {report:?}")]
    InvalidDocument { report: ValidationReport },
    #[error("suggestion would make calculation fail: {0}")]
    Calculation(#[from] CalculationError),
}

/// The formula cannot be explained from the supplied document.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FormulaExplanationError {
    #[error("formula field '{field}' does not exist")]
    MissingField { field: FieldRef },
    #[error("field '{field}' is not a formula")]
    NotFormula { field: FieldRef },
    #[error("formula field '{field}' has no calculated value")]
    MissingCalculation { field: FieldRef },
    #[error("document is semantically invalid: {report:?}")]
    InvalidDocument { report: ValidationReport },
    #[error(transparent)]
    Calculation(#[from] CalculationError),
}

/// A semantic comparison cannot be explained from the supplied snapshots.
#[derive(Debug, Error)]
pub enum ImpactExplanationError {
    #[error("document is semantically invalid: {report:?}")]
    InvalidDocument { report: ValidationReport },
    #[error(transparent)]
    Diff(#[from] DiffError),
}

/// Describe document schemas and entities in deterministic identifier order.
#[must_use]
pub fn describe_document(document: &Document) -> DocumentDescription {
    let schemas = document
        .schemas
        .iter()
        .map(|(id, schema)| SchemaDescription {
            id: id.clone(),
            key: schema.key.clone(),
            fields: schema
                .fields
                .iter()
                .map(|(id, definition)| FieldDescription {
                    id: id.clone(),
                    key: definition.key.clone(),
                    field_type: definition.field_type.clone(),
                    required: definition.required,
                })
                .collect(),
        })
        .collect();
    let entities = document
        .entities
        .iter()
        .map(|(id, entity)| EntityDescription {
            id: id.clone(),
            key: entity.key.clone(),
            schema: entity.schema.clone(),
            fields: entity.fields.keys().cloned().collect(),
        })
        .collect();

    DocumentDescription {
        id: document.id.clone(),
        title: document.title.clone(),
        schemas,
        entities,
    }
}

/// Evaluate a formula and list its direct dependencies in deterministic order.
///
/// # Errors
///
/// Returns an error when the field is missing or not a formula, or when formula calculation fails.
pub fn explain_formula(
    document: &Document,
    field: &FieldRef,
) -> Result<FormulaExplanation, FormulaExplanationError> {
    let analysis = analyze_workspace_formula(document, field).map_err(|error| match error {
        WorkspaceError::MissingField { field } => FormulaExplanationError::MissingField { field },
        WorkspaceError::NotFormula { field } => FormulaExplanationError::NotFormula { field },
        WorkspaceError::MissingCalculation { field } => {
            FormulaExplanationError::MissingCalculation { field }
        }
        WorkspaceError::InvalidDocument { report, .. } => {
            FormulaExplanationError::InvalidDocument { report }
        }
        WorkspaceError::Calculation(source) => FormulaExplanationError::Calculation(source),
        error => unreachable!("formula analysis returned an undocumented error: {error}"),
    })?;

    Ok(FormulaExplanation {
        field: analysis.field,
        expression: analysis.expression,
        value: analysis.value,
        dependencies: analysis.dependencies,
    })
}

/// Explain direct semantic changes and derived formula effects between documents.
///
/// # Errors
///
/// Returns an error when either document cannot be calculated for semantic comparison.
pub fn explain_impact(
    before: &Document,
    after: &Document,
) -> Result<ImpactExplanation, ImpactExplanationError> {
    let semantic_diff = compare_documents(before, after).map_err(|error| match error {
        WorkspaceError::InvalidDocument { report, .. } => {
            ImpactExplanationError::InvalidDocument { report }
        }
        WorkspaceError::Diff(source) => ImpactExplanationError::Diff(source),
        error => unreachable!("semantic comparison returned an undocumented error: {error}"),
    })?;

    Ok(ImpactExplanation {
        changes: semantic_diff.changes().to_vec(),
        summary: semantic_diff.render_text(),
    })
}

/// Create a validated, approval-required proposal without modifying the document.
///
/// # Errors
///
/// Returns an error when the field is missing or computed, the value does not
/// match its schema, or applying it would invalidate document semantics or calculation.
pub fn suggest_field_change(
    document: &Document,
    field: FieldRef,
    value: Value,
) -> Result<Suggestion, SuggestionError> {
    let validated =
        validate_field_value_suggestion(document, field, value).map_err(map_suggestion_error)?;

    Ok(Suggestion {
        field: validated.field,
        value: validated.value,
        requires_approval: true,
    })
}

fn map_suggestion_error(error: WorkspaceError) -> SuggestionError {
    match error {
        WorkspaceError::MissingEntityId { entity } => SuggestionError::MissingEntity { entity },
        WorkspaceError::MissingField { field } => SuggestionError::MissingField { field },
        WorkspaceError::FormulaEdit { field } => SuggestionError::FormulaEdit { field },
        WorkspaceError::ExpressionComplexity { field, source } => {
            SuggestionError::ExpressionComplexity { field, source }
        }
        WorkspaceError::FormulaProjection { field, source } => {
            SuggestionError::FormulaProjection { field, source }
        }
        WorkspaceError::TypeMismatch { field } => SuggestionError::TypeMismatch { field },
        WorkspaceError::NoChange { field } => SuggestionError::NoChange { field },
        WorkspaceError::InvalidDocument { report, .. } => {
            SuggestionError::InvalidDocument { report }
        }
        WorkspaceError::Calculation(source) => SuggestionError::Calculation(source),
        error => unreachable!("suggestion validation returned an undocumented error: {error}"),
    }
}
