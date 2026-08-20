//! Meaningful document change analysis for Tachiko Work.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef, Schema,
    SchemaId, Value,
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub enum SemanticChange {
    DocumentIdChanged {
        before: DocumentId,
        after: DocumentId,
    },
    DocumentTitleChanged {
        before: String,
        after: String,
    },
    SchemaAdded {
        schema: SchemaId,
        definition: Schema,
    },
    SchemaRemoved {
        schema: SchemaId,
        definition: Schema,
    },
    SchemaFieldAdded {
        schema: SchemaId,
        field: FieldId,
        definition: FieldDefinition,
    },
    SchemaFieldRemoved {
        schema: SchemaId,
        field: FieldId,
        definition: FieldDefinition,
    },
    SchemaFieldChanged {
        schema: SchemaId,
        field: FieldId,
        before: FieldDefinition,
        after: FieldDefinition,
    },
    EntityAdded {
        entity: EntityId,
    },
    EntityRemoved {
        entity: EntityId,
    },
    EntitySchemaChanged {
        entity: EntityId,
        before: SchemaId,
        after: SchemaId,
    },
    FieldAdded {
        field: FieldRef,
        value: Value,
    },
    FieldRemoved {
        field: FieldRef,
        value: Value,
    },
    FieldChanged {
        field: FieldRef,
        before: Value,
        after: Value,
    },
    FormulaImpact {
        field: FieldRef,
        before: f64,
        after: f64,
        causes: Vec<FieldRef>,
    },
}

impl SemanticChange {
    fn is_document_change(&self) -> bool {
        matches!(
            self,
            Self::DocumentIdChanged { .. } | Self::DocumentTitleChanged { .. }
        )
    }

    fn schema(&self) -> Option<&SchemaId> {
        match self {
            Self::SchemaAdded { schema, .. }
            | Self::SchemaRemoved { schema, .. }
            | Self::SchemaFieldAdded { schema, .. }
            | Self::SchemaFieldRemoved { schema, .. }
            | Self::SchemaFieldChanged { schema, .. } => Some(schema),
            _ => None,
        }
    }

    fn entity(&self) -> Option<&EntityId> {
        match self {
            Self::EntityAdded { entity }
            | Self::EntityRemoved { entity }
            | Self::EntitySchemaChanged { entity, .. } => Some(entity),
            Self::FieldAdded { field, .. }
            | Self::FieldRemoved { field, .. }
            | Self::FieldChanged { field, .. }
            | Self::FormulaImpact { field, .. } => Some(&field.entity),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SemanticDiff {
    changes: Vec<SemanticChange>,
    descriptors: BTreeMap<EntityId, EntityDescriptor>,
}

impl SemanticDiff {
    #[must_use]
    pub fn changes(&self) -> &[SemanticChange] {
        &self.changes
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        if self.changes.is_empty() {
            return "No semantic changes.\n".to_owned();
        }

        let mut sections = Vec::new();
        let document_changes: Vec<_> = self
            .changes
            .iter()
            .filter(|change| change.is_document_change())
            .collect();
        if !document_changes.is_empty() {
            let mut lines = vec!["Document".to_owned()];
            lines.extend(document_changes.into_iter().map(render_change));
            sections.push(lines.join("\n"));
        }

        let mut grouped_schemas: BTreeMap<&SchemaId, Vec<&SemanticChange>> = BTreeMap::new();
        for change in &self.changes {
            if let Some(schema) = change.schema() {
                grouped_schemas.entry(schema).or_default().push(change);
            }
        }
        for (schema, changes) in grouped_schemas {
            let mut lines = vec![format!("Schema {schema}")];
            lines.extend(changes.into_iter().map(render_change));
            sections.push(lines.join("\n"));
        }

        let mut grouped: BTreeMap<&EntityId, Vec<&SemanticChange>> = BTreeMap::new();
        for change in &self.changes {
            if let Some(entity) = change.entity() {
                grouped.entry(entity).or_default().push(change);
            }
        }

        for (entity_id, changes) in grouped {
            let descriptor = &self.descriptors[entity_id];
            let mut lines = vec![descriptor.heading()];
            lines.extend(changes.into_iter().map(render_change));
            sections.push(lines.join("\n"));
        }

        format!("{}\n", sections.join("\n\n"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EntityDescriptor {
    schema: SchemaId,
    label: String,
}

impl EntityDescriptor {
    fn heading(&self) -> String {
        format!("{} {}", humanize(self.schema.as_str()), self.label)
    }
}

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("could not calculate the original document: {0}")]
    BeforeCalculation(#[source] CalculationError),
    #[error("could not calculate the changed document: {0}")]
    AfterCalculation(#[source] CalculationError),
}

/// Compare two documents and calculate the semantic impact of their changes.
///
/// # Errors
///
/// Returns [`DiffError`] when either document cannot be calculated.
pub fn diff(before: &Document, after: &Document) -> Result<SemanticDiff, DiffError> {
    let before_calculation = calculate(before).map_err(DiffError::BeforeCalculation)?;
    let after_calculation = calculate(after).map_err(DiffError::AfterCalculation)?;
    let mut changes = Vec::new();
    let mut changed_fields = BTreeSet::new();
    let mut descriptors = BTreeMap::new();

    compare_document(before, after, &mut changes);
    compare_schemas(before, after, &mut changes);

    let entity_ids: BTreeSet<_> = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .cloned()
        .collect();

    for entity_id in entity_ids {
        match (
            before.entities.get(&entity_id),
            after.entities.get(&entity_id),
        ) {
            (None, Some(entity)) => {
                descriptors.insert(entity_id.clone(), descriptor(entity));
                changes.push(SemanticChange::EntityAdded { entity: entity_id });
            }
            (Some(entity), None) => {
                descriptors.insert(entity_id.clone(), descriptor(entity));
                changes.push(SemanticChange::EntityRemoved { entity: entity_id });
            }
            (Some(before_entity), Some(after_entity)) => {
                descriptors.insert(entity_id.clone(), descriptor(after_entity));
                compare_entity(
                    &entity_id,
                    before_entity,
                    after_entity,
                    &mut changes,
                    &mut changed_fields,
                );
            }
            (None, None) => unreachable!("entity id came from the document key union"),
        }
    }

    let formula_fields = formula_fields(before)
        .union(&formula_fields(after))
        .cloned()
        .collect::<BTreeSet<_>>();

    for formula in formula_fields {
        let (Some(before_value), Some(after_value)) = (
            before_calculation.value(&formula),
            after_calculation.value(&formula),
        ) else {
            continue;
        };
        if before_value.total_cmp(&after_value).is_eq() {
            continue;
        }

        let causes = changed_fields
            .iter()
            .filter(|changed| {
                *changed == &formula
                    || before_calculation.affected_by(changed).contains(&formula)
                    || after_calculation.affected_by(changed).contains(&formula)
            })
            .cloned()
            .collect();
        changes.push(SemanticChange::FormulaImpact {
            field: formula,
            before: before_value,
            after: after_value,
            causes,
        });
    }

    Ok(SemanticDiff {
        changes,
        descriptors,
    })
}

fn compare_document(before: &Document, after: &Document, changes: &mut Vec<SemanticChange>) {
    if before.id != after.id {
        changes.push(SemanticChange::DocumentIdChanged {
            before: before.id.clone(),
            after: after.id.clone(),
        });
    }
    if before.title != after.title {
        changes.push(SemanticChange::DocumentTitleChanged {
            before: before.title.clone(),
            after: after.title.clone(),
        });
    }
}

fn compare_schemas(before: &Document, after: &Document, changes: &mut Vec<SemanticChange>) {
    let schema_ids: BTreeSet<_> = before
        .schemas
        .keys()
        .chain(after.schemas.keys())
        .cloned()
        .collect();
    for schema_id in schema_ids {
        match (
            before.schemas.get(&schema_id),
            after.schemas.get(&schema_id),
        ) {
            (None, Some(schema)) => changes.push(SemanticChange::SchemaAdded {
                schema: schema_id,
                definition: schema.clone(),
            }),
            (Some(schema), None) => changes.push(SemanticChange::SchemaRemoved {
                schema: schema_id,
                definition: schema.clone(),
            }),
            (Some(before_schema), Some(after_schema)) => {
                compare_schema(&schema_id, before_schema, after_schema, changes);
            }
            (None, None) => unreachable!("schema id came from the document key union"),
        }
    }
}

fn compare_schema(
    schema_id: &SchemaId,
    before: &Schema,
    after: &Schema,
    changes: &mut Vec<SemanticChange>,
) {
    let field_ids: BTreeSet<_> = before
        .fields
        .keys()
        .chain(after.fields.keys())
        .cloned()
        .collect();
    for field_id in field_ids {
        match (before.fields.get(&field_id), after.fields.get(&field_id)) {
            (None, Some(definition)) => changes.push(SemanticChange::SchemaFieldAdded {
                schema: schema_id.clone(),
                field: field_id,
                definition: definition.clone(),
            }),
            (Some(definition), None) => changes.push(SemanticChange::SchemaFieldRemoved {
                schema: schema_id.clone(),
                field: field_id,
                definition: definition.clone(),
            }),
            (Some(before_definition), Some(after_definition))
                if before_definition != after_definition =>
            {
                changes.push(SemanticChange::SchemaFieldChanged {
                    schema: schema_id.clone(),
                    field: field_id,
                    before: before_definition.clone(),
                    after: after_definition.clone(),
                });
            }
            (Some(_), Some(_)) => {}
            (None, None) => unreachable!("field id came from the schema field key union"),
        }
    }
}

fn compare_entity(
    entity_id: &EntityId,
    before: &Entity,
    after: &Entity,
    changes: &mut Vec<SemanticChange>,
    changed_fields: &mut BTreeSet<FieldRef>,
) {
    if before.schema != after.schema {
        changes.push(SemanticChange::EntitySchemaChanged {
            entity: entity_id.clone(),
            before: before.schema.clone(),
            after: after.schema.clone(),
        });
    }

    let field_ids: BTreeSet<_> = before
        .fields
        .keys()
        .chain(after.fields.keys())
        .cloned()
        .collect();
    for field_id in field_ids {
        let field = FieldRef {
            entity: entity_id.clone(),
            field: field_id.clone(),
        };
        match (before.fields.get(&field_id), after.fields.get(&field_id)) {
            (None, Some(value)) => {
                changed_fields.insert(field.clone());
                changes.push(SemanticChange::FieldAdded {
                    field,
                    value: value.clone(),
                });
            }
            (Some(value), None) => {
                changed_fields.insert(field.clone());
                changes.push(SemanticChange::FieldRemoved {
                    field,
                    value: value.clone(),
                });
            }
            (Some(before_value), Some(after_value)) if before_value != after_value => {
                changed_fields.insert(field.clone());
                changes.push(SemanticChange::FieldChanged {
                    field,
                    before: before_value.clone(),
                    after: after_value.clone(),
                });
            }
            (Some(_), Some(_)) => {}
            (None, None) => unreachable!("field id came from the entity field key union"),
        }
    }
}

fn formula_fields(document: &Document) -> BTreeSet<FieldRef> {
    document
        .entities
        .iter()
        .flat_map(|(entity_id, entity)| {
            entity
                .fields
                .iter()
                .filter(|(_, value)| matches!(value, Value::Formula(_)))
                .map(|(field_id, _)| FieldRef {
                    entity: entity_id.clone(),
                    field: field_id.clone(),
                })
        })
        .collect()
}

fn descriptor(entity: &Entity) -> EntityDescriptor {
    let label = match entity.fields.get("name") {
        Some(Value::Text(name)) => name.clone(),
        _ => entity.id.to_string(),
    };
    EntityDescriptor {
        schema: entity.schema.clone(),
        label,
    }
}

fn render_change(change: &SemanticChange) -> String {
    match change {
        SemanticChange::DocumentIdChanged { before, after } => {
            format!("id: {before} -> {after}")
        }
        SemanticChange::DocumentTitleChanged { before, after } => {
            format!("title: {before:?} -> {after:?}")
        }
        SemanticChange::SchemaAdded { .. } => "schema added".to_owned(),
        SemanticChange::SchemaRemoved { .. } => "schema removed".to_owned(),
        SemanticChange::SchemaFieldAdded {
            field, definition, ..
        } => format!("{field} added: {}", format_definition(definition)),
        SemanticChange::SchemaFieldRemoved {
            field, definition, ..
        } => format!("{field} removed: {}", format_definition(definition)),
        SemanticChange::SchemaFieldChanged {
            field,
            before,
            after,
            ..
        } => format!(
            "{field}: {} -> {}",
            format_definition(before),
            format_definition(after)
        ),
        SemanticChange::EntityAdded { .. } => "entity added".to_owned(),
        SemanticChange::EntityRemoved { .. } => "entity removed".to_owned(),
        SemanticChange::EntitySchemaChanged { before, after, .. } => {
            format!("schema: {before} -> {after}")
        }
        SemanticChange::FieldAdded { field, value } => {
            format!("{} added: {}", field.field, format_value(value))
        }
        SemanticChange::FieldRemoved { field, value } => {
            format!("{} removed: {}", field.field, format_value(value))
        }
        SemanticChange::FieldChanged {
            field,
            before,
            after,
        } => format!(
            "{}: {} -> {}",
            field.field,
            format_value(before),
            format_value(after)
        ),
        SemanticChange::FormulaImpact {
            field,
            before,
            after,
            ..
        } => format!(
            "affected {}: {} -> {}",
            field.field,
            format_number(*before),
            format_number(*after)
        ),
    }
}

fn format_definition(definition: &FieldDefinition) -> String {
    let field_type = match &definition.field_type {
        tachiko_semantic_core::FieldType::Number => "number".to_owned(),
        tachiko_semantic_core::FieldType::Text => "text".to_owned(),
        tachiko_semantic_core::FieldType::Boolean => "boolean".to_owned(),
        tachiko_semantic_core::FieldType::Reference { schema } => {
            format!("reference({schema})")
        }
    };
    let presence = if definition.required {
        "required"
    } else {
        "optional"
    };
    format!("{field_type} ({presence})")
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Number(number) => format_number(*number),
        Value::Text(text) => format!("\"{text}\""),
        Value::Boolean(boolean) => boolean.to_string(),
        Value::Reference(entity) => format!("reference({entity})"),
        Value::Formula(expression) => format_expression(expression),
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
        Expression::Minimum { left, right } => {
            format!(
                "min({}, {})",
                format_expression(left),
                format_expression(right)
            )
        }
        Expression::Maximum { left, right } => {
            format!(
                "max({}, {})",
                format_expression(left),
                format_expression(right)
            )
        }
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
    number.to_string()
}

fn humanize(identifier: &str) -> String {
    identifier
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().chain(characters).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
