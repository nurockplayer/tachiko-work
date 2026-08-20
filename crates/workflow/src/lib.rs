//! User-facing workflows built on Tachiko Work's semantic engine.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_diff_engine::{DiffError, SemanticDiff, diff};
use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Diagnostic, Document, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value, is_valid_identifier, validate_document,
};
use thiserror::Error;

/// A useful starting point for a newly-created semantic document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StarterTemplate {
    /// A small, connected game-balance model with derived values.
    GameBalance,
    /// A blank document for users who already know the file format.
    Empty,
}

/// A compact, deterministic view of a semantic document.
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
    pub label: String,
    pub schema: SchemaId,
    pub fields: Vec<FieldOverview>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldOverview {
    pub id: FieldId,
    pub display_value: String,
    pub kind: FieldKind,
}

/// How a field participates in the semantic model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    /// A stored scalar input.
    Input,
    /// A typed relationship to an entity in the named schema.
    Reference { target_schema: SchemaId },
    /// A calculated numeric expression.
    Formula,
}

/// A field's current meaning and calculation relationships.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldExplanation {
    pub field: FieldRef,
    pub display_value: String,
    pub expression: Option<String>,
    pub dependencies: Vec<FieldRef>,
    pub affected_formulas: Vec<AffectedFormula>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AffectedFormula {
    pub field: FieldRef,
    pub display_value: String,
}

/// A validated document edit together with its semantic consequences.
#[derive(Clone, Debug, PartialEq)]
pub struct EditPreview {
    pub document: Document,
    pub diff: SemanticDiff,
}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("entity '{entity}' does not exist")]
    MissingEntity { entity: EntityId },
    #[error("entity id '{entity}' is not a valid semantic identifier")]
    InvalidEntityIdentifier { entity: EntityId },
    #[error("entity '{entity}' already exists")]
    EntityAlreadyExists { entity: EntityId },
    #[error("cannot rename entity '{entity}' to itself")]
    NoOpEntityRename { entity: EntityId },
    #[error(
        "cannot remove entity '{entity}' because it is referenced by {}",
        format_dependent_fields(.dependents)
    )]
    EntityReferenced {
        entity: EntityId,
        dependents: Vec<FieldRef>,
    },
    #[error("field '{field}' does not exist")]
    MissingField { field: FieldRef },
    #[error("schema '{schema}' does not exist")]
    MissingSchema { schema: SchemaId },
    #[error("field '{field}' is a formula; edit its inputs instead")]
    FormulaEdit { field: FieldRef },
    #[error("'{input}' is not a valid {expected} value for '{field}'")]
    InvalidValue {
        field: FieldRef,
        input: String,
        expected: &'static str,
    },
    #[error("edit would make the document invalid: {summary}")]
    InvalidDocument {
        summary: String,
        diagnostics: Vec<Diagnostic>,
    },
    #[error("'{field}' already has that value")]
    NoChange { field: FieldRef },
    #[error("could not calculate document: {0}")]
    Calculation(#[from] CalculationError),
    #[error("could not compare edited document: {0}")]
    Diff(#[from] DiffError),
}

/// Create a deterministic document from a first-party template.
#[must_use]
pub fn create_document(
    template: StarterTemplate,
    id: impl Into<tachiko_semantic_core::DocumentId>,
    title: impl Into<String>,
) -> Document {
    let id = id.into();
    let title = title.into();
    match template {
        StarterTemplate::GameBalance => game_balance_document(id, title),
        StarterTemplate::Empty => Document::empty(id, title),
    }
}

/// Build a stable, calculated view suitable for a CLI or future UI.
///
/// # Errors
///
/// Returns an error if any formula cannot be calculated.
pub fn overview(document: &Document) -> Result<DocumentOverview, WorkflowError> {
    let calculation = calculate(document)?;
    let mut formula_count = 0;
    let entities = document
        .entities
        .values()
        .map(|entity| {
            let fields = entity
                .fields
                .iter()
                .map(|(field_id, value)| {
                    let field_ref = FieldRef::new(entity.id.clone(), field_id.clone());
                    let kind = field_kind(document, entity, field_id, value)?;
                    if kind == FieldKind::Formula {
                        formula_count += 1;
                    }
                    let display_value = if kind == FieldKind::Formula {
                        calculation
                            .value(&field_ref)
                            .map_or_else(|| "unavailable".to_owned(), format_number)
                    } else {
                        format_value(value)
                    };
                    Ok(FieldOverview {
                        id: field_id.clone(),
                        display_value,
                        kind,
                    })
                })
                .collect::<Result<Vec<_>, WorkflowError>>()?;
            Ok(EntityOverview {
                id: entity.id.clone(),
                label: entity_label(entity),
                schema: entity.schema.clone(),
                fields,
            })
        })
        .collect::<Result<Vec<_>, WorkflowError>>()?;

    Ok(DocumentOverview {
        schema_count: document.schemas.len(),
        entity_count: document.entities.len(),
        formula_count,
        entities,
    })
}

fn field_kind(
    document: &Document,
    entity: &Entity,
    field_id: &FieldId,
    value: &Value,
) -> Result<FieldKind, WorkflowError> {
    match value {
        Value::Formula(_) => Ok(FieldKind::Formula),
        Value::Reference(_) => {
            let schema = document.schemas.get(&entity.schema).ok_or_else(|| {
                WorkflowError::MissingSchema {
                    schema: entity.schema.clone(),
                }
            })?;
            let definition =
                schema
                    .fields
                    .get(field_id)
                    .ok_or_else(|| WorkflowError::MissingField {
                        field: FieldRef::new(entity.id.clone(), field_id.clone()),
                    })?;
            let FieldType::Reference { schema } = &definition.field_type else {
                return Ok(FieldKind::Input);
            };
            Ok(FieldKind::Reference {
                target_schema: schema.clone(),
            })
        }
        Value::Number(_) | Value::Text(_) | Value::Boolean(_) => Ok(FieldKind::Input),
    }
}

/// Explain a field's value, formula, dependencies, and downstream effects.
///
/// # Errors
///
/// Returns an error when the field is missing or the document cannot be calculated.
pub fn explain_field(
    document: &Document,
    field: &FieldRef,
) -> Result<FieldExplanation, WorkflowError> {
    let value = field_value(document, field)?;
    let calculation = calculate(document)?;
    let display_value = calculation
        .value(field)
        .map_or_else(|| format_value(value), format_number);
    let expression = match value {
        Value::Formula(expression) => Some(format_expression(expression)),
        _ => None,
    };
    let dependencies = calculation
        .dependencies_of(field)
        .map_or_else(Vec::new, |dependencies| {
            dependencies.iter().cloned().collect()
        });
    let affected_formulas = calculation
        .affected_by(field)
        .into_iter()
        .filter_map(|affected| {
            calculation.value(&affected).map(|value| AffectedFormula {
                field: affected,
                display_value: format_number(value),
            })
        })
        .collect();

    Ok(FieldExplanation {
        field: field.clone(),
        display_value,
        expression,
        dependencies,
        affected_formulas,
    })
}

/// Parse and apply a schema-typed scalar edit without mutating the source document.
///
/// The result is validated, calculated, and semantically compared before it is returned.
/// Formula fields must be changed through their inputs.
///
/// # Errors
///
/// Returns an error for missing fields, formula targets, invalid scalar input, validation
/// failures, no-op edits, calculation failures, or comparison failures.
pub fn set_scalar(
    document: &Document,
    field: &FieldRef,
    input: &str,
) -> Result<EditPreview, WorkflowError> {
    let entity =
        document
            .entities
            .get(&field.entity)
            .ok_or_else(|| WorkflowError::MissingEntity {
                entity: field.entity.clone(),
            })?;
    let existing = entity
        .fields
        .get(&field.field)
        .ok_or_else(|| WorkflowError::MissingField {
            field: field.clone(),
        })?;
    if matches!(existing, Value::Formula(_)) {
        return Err(WorkflowError::FormulaEdit {
            field: field.clone(),
        });
    }

    let schema =
        document
            .schemas
            .get(&entity.schema)
            .ok_or_else(|| WorkflowError::MissingSchema {
                schema: entity.schema.clone(),
            })?;
    let definition =
        schema
            .fields
            .get(&field.field)
            .ok_or_else(|| WorkflowError::MissingField {
                field: field.clone(),
            })?;
    let value = parse_scalar(field, input, &definition.field_type)?;
    if existing == &value {
        return Err(WorkflowError::NoChange {
            field: field.clone(),
        });
    }

    let mut edited = document.clone();
    let Some(edited_entity) = edited.entities.get_mut(&field.entity) else {
        return Err(WorkflowError::MissingEntity {
            entity: field.entity.clone(),
        });
    };
    edited_entity.fields.insert(field.field.clone(), value);

    finalize_edit(document, edited)
}

/// Duplicate an entity into a new semantic identity without mutating the source.
///
/// Formula references owned by the copied entity that point back to the source
/// are recursively rebased to the target. Stored references and references to
/// other entities retain their existing meaning.
///
/// # Errors
///
/// Returns an error when the source is absent, the target identifier is invalid
/// or occupied, or the candidate fails validation, calculation, or semantic diff.
pub fn duplicate_entity(
    document: &Document,
    source: impl AsRef<str>,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkflowError> {
    let source = EntityId::from(source.as_ref());
    let target = EntityId::from(target.as_ref());
    let source_entity =
        document
            .entities
            .get(&source)
            .ok_or_else(|| WorkflowError::MissingEntity {
                entity: source.clone(),
            })?;
    require_available_target(document, &target)?;

    let mut duplicate = source_entity.clone();
    duplicate.id = target.clone();
    for value in duplicate.fields.values_mut() {
        if let Value::Formula(expression) = value {
            rewrite_expression_entity(expression, &source, &target);
        }
    }

    let mut edited = document.clone();
    edited.entities.insert(target, duplicate);
    finalize_edit(document, edited)
}

/// Rename an entity and every typed relationship that points to it.
///
/// # Errors
///
/// Returns an error for a no-op, absent source, invalid or occupied target, or
/// when the rewritten candidate fails validation, calculation, or semantic diff.
pub fn rename_entity(
    document: &Document,
    source: impl AsRef<str>,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkflowError> {
    let source = EntityId::from(source.as_ref());
    let target = EntityId::from(target.as_ref());
    if source == target {
        return Err(WorkflowError::NoOpEntityRename { entity: source });
    }
    if !document.entities.contains_key(&source) {
        return Err(WorkflowError::MissingEntity { entity: source });
    }
    require_available_target(document, &target)?;

    let mut edited = document.clone();
    let Some(mut renamed) = edited.entities.remove(&source) else {
        return Err(WorkflowError::MissingEntity { entity: source });
    };
    renamed.id = target.clone();
    edited.entities.insert(target.clone(), renamed);

    for entity in edited.entities.values_mut() {
        for value in entity.fields.values_mut() {
            match value {
                Value::Reference(reference) if reference == &source => {
                    *reference = target.clone();
                }
                Value::Formula(expression) => {
                    rewrite_expression_entity(expression, &source, &target);
                }
                Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Reference(_) => {}
            }
        }
    }

    finalize_edit(document, edited)
}

/// Remove an entity when no field owned by another entity refers to it.
///
/// # Errors
///
/// Returns an error when the entity is absent, when sorted dependent field paths
/// block removal, or when the candidate fails validation, calculation, or diff.
pub fn remove_entity(
    document: &Document,
    target: impl AsRef<str>,
) -> Result<EditPreview, WorkflowError> {
    let target = EntityId::from(target.as_ref());
    if !document.entities.contains_key(&target) {
        return Err(WorkflowError::MissingEntity { entity: target });
    }

    let dependents = document
        .entities
        .iter()
        .filter(|(entity_id, _)| *entity_id != &target)
        .flat_map(|(entity_id, entity)| {
            entity
                .fields
                .iter()
                .filter(|(_, value)| value_references_entity(value, &target))
                .map(|(field_id, _)| FieldRef::new(entity_id.clone(), field_id.clone()))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if !dependents.is_empty() {
        return Err(WorkflowError::EntityReferenced {
            entity: target,
            dependents,
        });
    }

    let mut edited = document.clone();
    edited.entities.remove(&target);
    finalize_edit(document, edited)
}

fn require_available_target(document: &Document, target: &EntityId) -> Result<(), WorkflowError> {
    if !is_valid_identifier(target.as_str()) {
        return Err(WorkflowError::InvalidEntityIdentifier {
            entity: target.clone(),
        });
    }
    if document.entities.contains_key(target) {
        return Err(WorkflowError::EntityAlreadyExists {
            entity: target.clone(),
        });
    }
    Ok(())
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

fn finalize_edit(document: &Document, edited: Document) -> Result<EditPreview, WorkflowError> {
    let diagnostics = validate_document(&edited);
    if !diagnostics.is_empty() {
        let summary = format_diagnostics(&diagnostics);
        return Err(WorkflowError::InvalidDocument {
            summary,
            diagnostics,
        });
    }
    calculate(&edited)?;
    let semantic_diff = diff(document, &edited)?;

    Ok(EditPreview {
        document: edited,
        diff: semantic_diff,
    })
}

fn format_dependent_fields(dependents: &[FieldRef]) -> String {
    dependents
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn field_value<'document>(
    document: &'document Document,
    field: &FieldRef,
) -> Result<&'document Value, WorkflowError> {
    let entity =
        document
            .entities
            .get(&field.entity)
            .ok_or_else(|| WorkflowError::MissingEntity {
                entity: field.entity.clone(),
            })?;
    entity
        .fields
        .get(&field.field)
        .ok_or_else(|| WorkflowError::MissingField {
            field: field.clone(),
        })
}

fn parse_scalar(
    field: &FieldRef,
    input: &str,
    field_type: &FieldType,
) -> Result<Value, WorkflowError> {
    let invalid = |expected| WorkflowError::InvalidValue {
        field: field.clone(),
        input: input.to_owned(),
        expected,
    };
    match field_type {
        FieldType::Number => {
            let value = input.parse::<f64>().map_err(|_| invalid("number"))?;
            if !value.is_finite() {
                return Err(invalid("finite number"));
            }
            Ok(Value::Number(value))
        }
        FieldType::Text => Ok(Value::Text(input.to_owned())),
        FieldType::Boolean => input
            .parse::<bool>()
            .map(Value::Boolean)
            .map_err(|_| invalid("boolean (true or false)")),
        FieldType::Reference { .. } => Ok(Value::Reference(input.into())),
    }
}

fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn game_balance_document(id: tachiko_semantic_core::DocumentId, title: String) -> Document {
    Document {
        id,
        title,
        schemas: game_balance_schemas(),
        entities: game_balance_entities(),
    }
}

fn game_balance_schemas() -> BTreeMap<SchemaId, Schema> {
    BTreeMap::from([
        schema(
            "characters",
            [
                ("level", FieldType::Number),
                ("name", FieldType::Text),
                (
                    "weapon",
                    FieldType::Reference {
                        schema: "weapons".into(),
                    },
                ),
            ],
        ),
        schema(
            "economy",
            [
                ("currency", FieldType::Text),
                ("gold_per_match", FieldType::Number),
                ("matches_for_sword", FieldType::Number),
                ("upgrade_cost", FieldType::Number),
            ],
        ),
        schema(
            "items",
            [
                ("category", FieldType::Text),
                (
                    "grants_weapon",
                    FieldType::Reference {
                        schema: "weapons".into(),
                    },
                ),
                ("name", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
        schema(
            "weapons",
            [
                ("attack_interval", FieldType::Number),
                ("damage", FieldType::Number),
                ("dps", FieldType::Number),
                ("name", FieldType::Text),
                ("price", FieldType::Number),
            ],
        ),
    ])
}

fn game_balance_entities() -> BTreeMap<EntityId, Entity> {
    BTreeMap::from([
        entity(
            "alric",
            "characters",
            [
                ("level", Value::Number(4.0)),
                ("name", Value::Text("Alric".to_owned())),
                ("weapon", Value::Reference("iron_sword".into())),
            ],
        ),
        entity(
            "iron_sword",
            "weapons",
            [
                ("attack_interval", Value::Number(0.9)),
                ("damage", Value::Number(36.0)),
                (
                    "dps",
                    Value::Formula(Expression::Divide {
                        left: Box::new(reference("iron_sword", "damage")),
                        right: Box::new(reference("iron_sword", "attack_interval")),
                    }),
                ),
                ("name", Value::Text("Iron Sword".to_owned())),
                ("price", Value::Number(120.0)),
            ],
        ),
        entity(
            "shop",
            "economy",
            [
                ("currency", Value::Text("gold".to_owned())),
                ("gold_per_match", Value::Number(50.0)),
                (
                    "matches_for_sword",
                    Value::Formula(Expression::Divide {
                        left: Box::new(reference("iron_sword", "price")),
                        right: Box::new(reference("shop", "gold_per_match")),
                    }),
                ),
                (
                    "upgrade_cost",
                    Value::Formula(reference("tempered_blade", "price")),
                ),
            ],
        ),
        entity(
            "tempered_blade",
            "items",
            [
                ("category", Value::Text("weapon upgrade".to_owned())),
                ("grants_weapon", Value::Reference("iron_sword".into())),
                ("name", Value::Text("Tempered Blade".to_owned())),
                ("price", Value::Number(200.0)),
            ],
        ),
    ])
}

fn schema<const N: usize>(id: &str, fields: [(&str, FieldType); N]) -> (SchemaId, Schema) {
    let id = SchemaId::from(id);
    let fields = fields
        .into_iter()
        .map(|(id, field_type)| {
            (
                FieldId::from(id),
                FieldDefinition {
                    field_type,
                    required: true,
                },
            )
        })
        .collect();
    (id.clone(), Schema { id, fields })
}

fn entity<const N: usize>(
    id: &str,
    schema: &str,
    fields: [(&str, Value); N],
) -> (EntityId, Entity) {
    let id = EntityId::from(id);
    let fields = fields
        .into_iter()
        .map(|(id, value)| (FieldId::from(id), value))
        .collect();
    (
        id.clone(),
        Entity {
            id,
            schema: schema.into(),
            fields,
        },
    )
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn entity_label(entity: &Entity) -> String {
    match entity.fields.get("name") {
        Some(Value::Text(name)) => name.clone(),
        _ => humanize(entity.id.as_str()),
    }
}

fn humanize(value: &str) -> String {
    let mut words = value.split('_').filter(|word| !word.is_empty());
    let Some(first) = words.next() else {
        return value.to_owned();
    };
    let mut result = capitalize(first);
    for word in words {
        result.push(' ');
        result.push_str(word);
    }
    result
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Number(number) => format_number(*number),
        Value::Text(text) => text.clone(),
        Value::Boolean(boolean) => boolean.to_string(),
        Value::Reference(entity) => format!("→ {entity}"),
        Value::Formula(_) => "formula".to_owned(),
    }
}

fn format_expression(expression: &Expression) -> String {
    match expression {
        Expression::Number(number) => format_number(*number),
        Expression::Reference(reference) => reference.to_string(),
        Expression::Add { left, right } => format_binary(left, "+", right),
        Expression::Subtract { left, right } => format_binary(left, "-", right),
        Expression::Multiply { left, right } => format_binary(left, "*", right),
        Expression::Divide { left, right } => format_binary(left, "/", right),
        Expression::Minimum { left, right } => format!(
            "min({}, {})",
            format_expression(left),
            format_expression(right)
        ),
        Expression::Maximum { left, right } => format!(
            "max({}, {})",
            format_expression(left),
            format_expression(right)
        ),
    }
}

fn format_binary(left: &Expression, operator: &str, right: &Expression) -> String {
    format!(
        "({} {operator} {})",
        format_expression(left),
        format_expression(right)
    )
}

fn format_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}
