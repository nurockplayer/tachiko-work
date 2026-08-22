//! Read-only semantic queries for Tachiko Work.

use tachiko_diff_engine::{DiffError, SemanticChange, diff};
use tachiko_formula_engine::{
    CalculationError, CanonicalAuthoringProjectionError, ExpressionComplexityError, calculate,
    project_expression, validate_expression_structure,
};
use tachiko_semantic_core::{
    Diagnostic, Document, DocumentId, EntityKey, Expression, FieldId, FieldKey, FieldRef,
    FieldType, Number, SchemaId, SchemaKey, Value, validate_document,
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
    pub id: tachiko_semantic_core::EntityId,
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
    MissingEntity {
        entity: tachiko_semantic_core::EntityId,
    },
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
    #[error("suggestion would make the document invalid: {diagnostics:?}")]
    InvalidDocument { diagnostics: Vec<Diagnostic> },
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
    #[error(transparent)]
    Calculation(#[from] CalculationError),
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
    let value = document
        .entities
        .get(&field.entity)
        .and_then(|entity| entity.fields.get(&field.field))
        .ok_or_else(|| FormulaExplanationError::MissingField {
            field: field.clone(),
        })?;
    let Value::Formula(expression) = value else {
        return Err(FormulaExplanationError::NotFormula {
            field: field.clone(),
        });
    };

    let calculation = calculate(document)?;
    let value =
        calculation
            .value(field)
            .ok_or_else(|| FormulaExplanationError::MissingCalculation {
                field: field.clone(),
            })?;
    let dependencies = calculation
        .dependencies_of(field)
        .map_or_else(Vec::new, |dependencies| {
            dependencies.iter().cloned().collect()
        });

    Ok(FormulaExplanation {
        field: field.clone(),
        expression: expression.clone(),
        value,
        dependencies,
    })
}

/// Explain direct semantic changes and derived formula effects between documents.
///
/// # Errors
///
/// Returns an error when either document cannot be calculated for semantic comparison.
pub fn explain_impact(before: &Document, after: &Document) -> Result<ImpactExplanation, DiffError> {
    let semantic_diff = diff(before, after)?;

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
    let entity =
        document
            .entities
            .get(&field.entity)
            .ok_or_else(|| SuggestionError::MissingEntity {
                entity: field.entity.clone(),
            })?;
    let existing =
        entity
            .fields
            .get(&field.field)
            .ok_or_else(|| SuggestionError::MissingField {
                field: field.clone(),
            })?;
    if matches!(existing, Value::Formula(_)) && !matches!(value, Value::Formula(_)) {
        return Err(SuggestionError::FormulaEdit { field });
    }
    if existing == &value {
        return Err(SuggestionError::NoChange { field });
    }
    let definition = document
        .schemas
        .get(&entity.schema)
        .and_then(|schema| schema.fields.get(&field.field))
        .ok_or_else(|| SuggestionError::MissingField {
            field: field.clone(),
        })?;
    if !value_matches_type(&value, &definition.field_type) {
        return Err(SuggestionError::TypeMismatch { field });
    }
    if let Value::Formula(expression) = &value {
        validate_expression_structure(expression).map_err(|source| {
            SuggestionError::ExpressionComplexity {
                field: field.clone(),
                source,
            }
        })?;
        project_expression(document, expression).map_err(|source| match source {
            CanonicalAuthoringProjectionError::Complexity(source) => {
                SuggestionError::ExpressionComplexity {
                    field: field.clone(),
                    source,
                }
            }
            source @ CanonicalAuthoringProjectionError::UnresolvableBoundReferences { .. } => {
                SuggestionError::FormulaProjection {
                    field: field.clone(),
                    source,
                }
            }
        })?;
    }

    let mut proposed = document.clone();
    let proposed_entity =
        proposed
            .entities
            .get_mut(&field.entity)
            .ok_or_else(|| SuggestionError::MissingEntity {
                entity: field.entity.clone(),
            })?;
    proposed_entity
        .fields
        .insert(field.field.clone(), value.clone());
    let diagnostics = validate_document(&proposed);
    if !diagnostics.is_empty() {
        return Err(SuggestionError::InvalidDocument { diagnostics });
    }
    calculate(&proposed)?;

    Ok(Suggestion {
        field,
        value,
        requires_approval: true,
    })
}

fn value_matches_type(value: &Value, field_type: &FieldType) -> bool {
    matches!(
        (value, field_type),
        (Value::Number(_) | Value::Formula(_), FieldType::Number)
            | (Value::Text(_), FieldType::Text)
            | (Value::Boolean(_), FieldType::Boolean)
            | (Value::Reference(_), FieldType::Reference { .. })
    )
}
