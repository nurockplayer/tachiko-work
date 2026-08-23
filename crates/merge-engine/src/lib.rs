//! Semantic three-way merge for Tachiko Work documents.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey,
    FieldType, Schema, SchemaId, SchemaKey, Value,
};

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
    FieldId(FieldId),
    SchemaKey(SchemaKey),
    EntityKey(EntityKey),
    FieldKey(FieldKey),
    FieldType(FieldType),
    Required(bool),
    FieldValue(Value),
}

/// Merge semantic changes from `ours` and `theirs` against their common `base`.
///
/// This engine owns model-level reconciliation only. Workspace-engine applies
/// semantic validation and operation-specific gates to inputs and candidates.
#[must_use]
pub fn merge(base: &Document, ours: &Document, theirs: &Document) -> MergeOutcome {
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
        return MergeOutcome::Conflicted(conflicts);
    }

    let (Some(id), Some(title), Some(schemas), Some(entities)) = (id, title, schemas, entities)
    else {
        unreachable!("missing merge selection must have produced a conflict")
    };
    MergeOutcome::Merged(Document {
        id,
        title,
        schemas,
        entities,
    })
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
    let key = merge_scalar(
        &format!("{path}.key"),
        &base.key,
        &ours.key,
        &theirs.key,
        |key| MergeValue::SchemaKey(key.clone()),
        conflicts,
    );

    Some(Schema {
        id: id?,
        key: key?,
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
        match (
            base.get(&field_id),
            ours.get(&field_id),
            theirs.get(&field_id),
        ) {
            (Some(base), Some(ours), Some(theirs)) => {
                if let Some(field) = merge_field_definition(&path, base, ours, theirs, conflicts) {
                    fields.insert(field_id, field);
                } else {
                    complete = false;
                }
            }
            (base, ours, theirs) => match merge_optional(
                &path,
                base,
                ours,
                theirs,
                |field| MergeValue::FieldDefinition(field.clone()),
                conflicts,
            ) {
                OptionalChoice::Chosen(Some(field)) => {
                    fields.insert(field_id, field);
                }
                OptionalChoice::Chosen(None) => {}
                OptionalChoice::Conflict => complete = false,
            },
        }
    }

    complete.then_some(fields)
}

fn merge_field_definition(
    path: &str,
    base: &FieldDefinition,
    ours: &FieldDefinition,
    theirs: &FieldDefinition,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<FieldDefinition> {
    let id = merge_scalar(
        &format!("{path}.id"),
        &base.id,
        &ours.id,
        &theirs.id,
        |id| MergeValue::FieldId(id.clone()),
        conflicts,
    );
    let key = merge_scalar(
        &format!("{path}.key"),
        &base.key,
        &ours.key,
        &theirs.key,
        |key| MergeValue::FieldKey(key.clone()),
        conflicts,
    );
    let field_type = merge_scalar(
        &format!("{path}.field_type"),
        &base.field_type,
        &ours.field_type,
        &theirs.field_type,
        |field_type| MergeValue::FieldType(field_type.clone()),
        conflicts,
    );
    let required = merge_scalar(
        &format!("{path}.required"),
        &base.required,
        &ours.required,
        &theirs.required,
        |required| MergeValue::Required(*required),
        conflicts,
    );

    Some(FieldDefinition {
        id: id?,
        key: key?,
        field_type: field_type?,
        required: required?,
    })
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
    let key = merge_scalar(
        &format!("{path}.key"),
        &base.key,
        &ours.key,
        &theirs.key,
        |key| MergeValue::EntityKey(key.clone()),
        conflicts,
    );
    let fields = merge_entity_fields(path, &base.fields, &ours.fields, &theirs.fields, conflicts);

    Some(Entity {
        id: id?,
        key: key?,
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
