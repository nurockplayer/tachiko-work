//! Semantic three-way merge for Tachiko Work documents.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Diagnostic, Document, DocumentId, Entity, EntityId, FieldDefinition, FieldId, Schema, SchemaId,
    Value, validate_document,
};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub enum MergeOutcome {
    Merged(Document),
    Conflicted(Vec<MergeConflict>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergeConflict {
    pub path: String,
    pub base: Option<MergeValue>,
    pub ours: Option<MergeValue>,
    pub theirs: Option<MergeValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MergeValue {
    DocumentId(DocumentId),
    DocumentTitle(String),
    Schema(Schema),
    FieldDefinition(FieldDefinition),
    Entity(Entity),
    EntityId(EntityId),
    SchemaId(SchemaId),
    FieldValue(Value),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeSide {
    Base,
    Ours,
    Theirs,
}

#[derive(Debug, Error)]
pub enum MergeError {
    #[error("invalid {side:?} input: {diagnostics:?}")]
    InvalidInput {
        side: MergeSide,
        diagnostics: Vec<Diagnostic>,
    },
    #[error("could not calculate {side:?} input: {source}")]
    InputCalculation {
        side: MergeSide,
        #[source]
        source: CalculationError,
    },
    #[error("invalid merged document: {diagnostics:?}")]
    InvalidMergedDocument { diagnostics: Vec<Diagnostic> },
    #[error("could not calculate merged document: {0}")]
    MergedCalculation(CalculationError),
}

/// Merge semantic changes from `ours` and `theirs` against their common `base`.
///
/// # Errors
///
/// Returns [`MergeError`] when an input or conflict-free candidate is unsafe.
pub fn merge(
    base: &Document,
    ours: &Document,
    theirs: &Document,
) -> Result<MergeOutcome, MergeError> {
    validate_and_calculate_input(MergeSide::Base, base)?;
    validate_and_calculate_input(MergeSide::Ours, ours)?;
    validate_and_calculate_input(MergeSide::Theirs, theirs)?;

    let mut conflicts = Vec::new();
    let id = merge_scalar(
        "id",
        &base.id,
        &ours.id,
        &theirs.id,
        |id| MergeValue::DocumentId(id.clone()),
        &mut conflicts,
    );
    let title = merge_scalar(
        "title",
        &base.title,
        &ours.title,
        &theirs.title,
        |title| MergeValue::DocumentTitle(title.clone()),
        &mut conflicts,
    );
    let schemas = merge_schemas(
        &base.schemas,
        &ours.schemas,
        &theirs.schemas,
        &mut conflicts,
    );
    let entities = merge_entities(
        &base.entities,
        &ours.entities,
        &theirs.entities,
        &mut conflicts,
    );

    if !conflicts.is_empty() {
        conflicts.sort_by(|left, right| left.path.cmp(&right.path));
        return Ok(MergeOutcome::Conflicted(conflicts));
    }

    let (Some(id), Some(title), Some(schemas), Some(entities)) = (id, title, schemas, entities)
    else {
        unreachable!("missing merge selection must have produced a conflict")
    };
    let candidate = Document {
        id,
        title,
        schemas,
        entities,
    };
    let diagnostics = validate_document(&candidate);
    if !diagnostics.is_empty() {
        return Err(MergeError::InvalidMergedDocument { diagnostics });
    }
    calculate(&candidate).map_err(MergeError::MergedCalculation)?;

    Ok(MergeOutcome::Merged(candidate))
}

fn validate_and_calculate_input(side: MergeSide, document: &Document) -> Result<(), MergeError> {
    let diagnostics = validate_document(document);
    if !diagnostics.is_empty() {
        return Err(MergeError::InvalidInput { side, diagnostics });
    }
    calculate(document).map_err(|source| MergeError::InputCalculation { side, source })?;
    Ok(())
}

fn merge_schemas(
    base: &BTreeMap<SchemaId, Schema>,
    ours: &BTreeMap<SchemaId, Schema>,
    theirs: &BTreeMap<SchemaId, Schema>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<SchemaId, Schema>> {
    let schema_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut schemas = BTreeMap::new();
    let mut complete = true;

    for schema_id in schema_ids {
        let path = format!("schemas.{schema_id}");
        match (
            base.get(&schema_id),
            ours.get(&schema_id),
            theirs.get(&schema_id),
        ) {
            (Some(base), Some(ours), Some(theirs)) => {
                if let Some(schema) = merge_schema(&path, base, ours, theirs, conflicts) {
                    schemas.insert(schema_id, schema);
                } else {
                    complete = false;
                }
            }
            (base, ours, theirs) => match merge_optional(
                &path,
                base,
                ours,
                theirs,
                |schema| MergeValue::Schema(schema.clone()),
                conflicts,
            ) {
                OptionalChoice::Chosen(Some(schema)) => {
                    schemas.insert(schema_id, schema);
                }
                OptionalChoice::Chosen(None) => {}
                OptionalChoice::Conflict => complete = false,
            },
        }
    }

    complete.then_some(schemas)
}

fn merge_schema(
    path: &str,
    base: &Schema,
    ours: &Schema,
    theirs: &Schema,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<Schema> {
    let id = merge_scalar(
        &format!("{path}.id"),
        &base.id,
        &ours.id,
        &theirs.id,
        |id| MergeValue::SchemaId(id.clone()),
        conflicts,
    );
    let fields = merge_schema_fields(path, &base.fields, &ours.fields, &theirs.fields, conflicts);

    Some(Schema {
        id: id?,
        fields: fields?,
    })
}

fn merge_schema_fields(
    schema_path: &str,
    base: &BTreeMap<FieldId, FieldDefinition>,
    ours: &BTreeMap<FieldId, FieldDefinition>,
    theirs: &BTreeMap<FieldId, FieldDefinition>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<FieldId, FieldDefinition>> {
    let field_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut fields = BTreeMap::new();
    let mut complete = true;

    for field_id in field_ids {
        let path = format!("{schema_path}.fields.{field_id}");
        match merge_optional(
            &path,
            base.get(&field_id),
            ours.get(&field_id),
            theirs.get(&field_id),
            |field| MergeValue::FieldDefinition(field.clone()),
            conflicts,
        ) {
            OptionalChoice::Chosen(Some(field)) => {
                fields.insert(field_id, field);
            }
            OptionalChoice::Chosen(None) => {}
            OptionalChoice::Conflict => complete = false,
        }
    }

    complete.then_some(fields)
}

fn merge_entities(
    base: &BTreeMap<EntityId, Entity>,
    ours: &BTreeMap<EntityId, Entity>,
    theirs: &BTreeMap<EntityId, Entity>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<EntityId, Entity>> {
    let entity_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut entities = BTreeMap::new();
    let mut complete = true;

    for entity_id in entity_ids {
        let path = format!("entities.{entity_id}");
        match (
            base.get(&entity_id),
            ours.get(&entity_id),
            theirs.get(&entity_id),
        ) {
            (Some(base), Some(ours), Some(theirs)) => {
                if let Some(entity) = merge_entity(&path, base, ours, theirs, conflicts) {
                    entities.insert(entity_id, entity);
                } else {
                    complete = false;
                }
            }
            (base, ours, theirs) => match merge_optional(
                &path,
                base,
                ours,
                theirs,
                |entity| MergeValue::Entity(entity.clone()),
                conflicts,
            ) {
                OptionalChoice::Chosen(Some(entity)) => {
                    entities.insert(entity_id, entity);
                }
                OptionalChoice::Chosen(None) => {}
                OptionalChoice::Conflict => complete = false,
            },
        }
    }

    complete.then_some(entities)
}

fn merge_entity(
    path: &str,
    base: &Entity,
    ours: &Entity,
    theirs: &Entity,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<Entity> {
    let id = merge_scalar(
        &format!("{path}.id"),
        &base.id,
        &ours.id,
        &theirs.id,
        |id| MergeValue::EntityId(id.clone()),
        conflicts,
    );
    let schema = merge_scalar(
        &format!("{path}.schema"),
        &base.schema,
        &ours.schema,
        &theirs.schema,
        |schema| MergeValue::SchemaId(schema.clone()),
        conflicts,
    );
    let fields = merge_entity_fields(path, &base.fields, &ours.fields, &theirs.fields, conflicts);

    Some(Entity {
        id: id?,
        schema: schema?,
        fields: fields?,
    })
}

fn merge_entity_fields(
    entity_path: &str,
    base: &BTreeMap<FieldId, Value>,
    ours: &BTreeMap<FieldId, Value>,
    theirs: &BTreeMap<FieldId, Value>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<FieldId, Value>> {
    let field_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut fields = BTreeMap::new();
    let mut complete = true;

    for field_id in field_ids {
        let path = format!("{entity_path}.fields.{field_id}");
        match merge_optional(
            &path,
            base.get(&field_id),
            ours.get(&field_id),
            theirs.get(&field_id),
            |field| MergeValue::FieldValue(field.clone()),
            conflicts,
        ) {
            OptionalChoice::Chosen(Some(field)) => {
                fields.insert(field_id, field);
            }
            OptionalChoice::Chosen(None) => {}
            OptionalChoice::Conflict => complete = false,
        }
    }

    complete.then_some(fields)
}

fn merge_scalar<T: Clone + PartialEq>(
    path: &str,
    base: &T,
    ours: &T,
    theirs: &T,
    value: impl Fn(&T) -> MergeValue,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<T> {
    let selected = choose(base, ours, theirs);
    if selected.is_none() {
        conflicts.push(MergeConflict {
            path: path.to_owned(),
            base: Some(value(base)),
            ours: Some(value(ours)),
            theirs: Some(value(theirs)),
        });
    }
    selected
}

fn merge_optional<T: Clone + PartialEq>(
    path: &str,
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
    value: impl Fn(&T) -> MergeValue,
    conflicts: &mut Vec<MergeConflict>,
) -> OptionalChoice<T> {
    let selected = choose_optional(base, ours, theirs);
    if matches!(selected, OptionalChoice::Conflict) {
        conflicts.push(MergeConflict {
            path: path.to_owned(),
            base: base.map(&value),
            ours: ours.map(&value),
            theirs: theirs.map(value),
        });
    }
    selected
}

fn choose<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> Option<T> {
    if ours == theirs {
        Some(ours.clone())
    } else if ours == base {
        Some(theirs.clone())
    } else if theirs == base {
        Some(ours.clone())
    } else {
        None
    }
}

fn choose_optional<T: Clone + PartialEq>(
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
) -> OptionalChoice<T> {
    if ours == theirs {
        OptionalChoice::Chosen(ours.cloned())
    } else if ours == base {
        OptionalChoice::Chosen(theirs.cloned())
    } else if theirs == base {
        OptionalChoice::Chosen(ours.cloned())
    } else {
        OptionalChoice::Conflict
    }
}

enum OptionalChoice<T> {
    Chosen(Option<T>),
    Conflict,
}
