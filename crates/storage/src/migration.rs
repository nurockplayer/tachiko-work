//! Explicit representation migrations owned by the storage boundary.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES};
use uuid::Uuid;

use crate::{
    direct_ro::v2::{
        BinaryArgsV2, DocumentV2, EntityV2, ExpressionV2, FieldDefinitionV2, FieldRefV2,
        FieldTypeV2, SchemaV2, ValueV2,
    },
    legacy_direct_ro::v1::{BinaryArgsV1, DocumentV1, ExpressionV1, FieldTypeV1, ValueV1},
};

/// Fixed RFC 9562 `UUIDv5` namespace for `legacy-direct-ro/v1` identity migration.
///
/// It is itself UUIDv5(URL, `https://tachiko.work/migrations/legacy-direct-ro/v1`).
pub(crate) const LEGACY_V1_NAMESPACE: Uuid =
    Uuid::from_u128(0x7a19_9010_e2db_5f4f_a216_07dd_b708_f5ef);

#[derive(Debug)]
pub(crate) struct MigrationError(pub(crate) String);

struct IdentityMaps {
    document: String,
    schemas: BTreeMap<String, String>,
    fields: BTreeMap<(String, String), String>,
    entities: BTreeMap<String, String>,
    entity_schemas: BTreeMap<String, String>,
}

/// Migrate the complete frozen legacy v1 DTO into the storage-owned v2 DTO.
///
/// Mapping is deliberately completed before any relationship is rewritten.
pub(crate) fn legacy_v1_to_v2(document: DocumentV1) -> Result<DocumentV2, MigrationError> {
    validate_legacy_expressions(&document)?;
    let maps = build_identity_maps(&document)?;
    rewrite_document(document, &maps)
}

fn validate_legacy_expressions(document: &DocumentV1) -> Result<(), MigrationError> {
    for entity in document.entities.values() {
        for value in entity.fields.values() {
            if let ValueV1::Formula(expression) = value {
                validate_legacy_expression(expression)?;
            }
        }
    }
    Ok(())
}

fn validate_legacy_expression(expression: &ExpressionV1) -> Result<(), MigrationError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(expression, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(MigrationError(format!(
                "legacy formula expression exceeds {MAX_EXPRESSION_DEPTH}-depth limit"
            )));
        }
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return Err(MigrationError(format!(
                "legacy formula expression exceeds {MAX_EXPRESSION_NODES}-node limit"
            )));
        }
        match node {
            ExpressionV1::Add(args)
            | ExpressionV1::Subtract(args)
            | ExpressionV1::Multiply(args)
            | ExpressionV1::Divide(args)
            | ExpressionV1::Minimum(args)
            | ExpressionV1::Maximum(args) => {
                stack.push((&args.right, depth + 1));
                stack.push((&args.left, depth + 1));
            }
            ExpressionV1::Number(_) | ExpressionV1::Reference(_) => {}
        }
    }
    Ok(())
}

fn build_identity_maps(document: &DocumentV1) -> Result<IdentityMaps, MigrationError> {
    let legacy_document = document.id.0.as_str();
    let document_id = migration_id(&format!("document\0{legacy_document}"));

    let mut schemas = BTreeMap::new();
    let mut schema_ids = BTreeSet::new();
    let mut fields = BTreeMap::new();
    let mut field_ids = BTreeSet::new();
    for (schema_key, schema) in &document.schemas {
        let legacy_schema = schema_key.0.as_str();
        let stable_schema = migration_id(&format!("schema\0{legacy_document}\0{legacy_schema}"));
        ensure_unique("schema", &mut schema_ids, &stable_schema)?;
        schemas.insert(legacy_schema.to_owned(), stable_schema);

        for legacy_field in schema.fields.keys() {
            let stable_field = migration_id(&format!(
                "field\0{legacy_document}\0{legacy_schema}\0{}",
                legacy_field.0
            ));
            ensure_unique("field", &mut field_ids, &stable_field)?;
            fields.insert(
                (legacy_schema.to_owned(), legacy_field.0.clone()),
                stable_field,
            );
        }
    }

    let mut entities = BTreeMap::new();
    let mut entity_schemas = BTreeMap::new();
    let mut entity_ids = BTreeSet::new();
    for (entity_key, entity) in &document.entities {
        let legacy_entity = entity_key.0.as_str();
        let stable_entity = migration_id(&format!("entity\0{legacy_document}\0{legacy_entity}"));
        ensure_unique("entity", &mut entity_ids, &stable_entity)?;
        entities.insert(legacy_entity.to_owned(), stable_entity);
        entity_schemas.insert(legacy_entity.to_owned(), entity.schema.0.clone());
    }

    Ok(IdentityMaps {
        document: document_id,
        schemas,
        fields,
        entities,
        entity_schemas,
    })
}

fn rewrite_document(
    document: DocumentV1,
    maps: &IdentityMaps,
) -> Result<DocumentV2, MigrationError> {
    let schemas = document
        .schemas
        .into_iter()
        .map(|(legacy_schema, schema)| {
            let stable_schema_id = schema_id(maps, &legacy_schema.0)?.to_owned();
            let fields = schema
                .fields
                .into_iter()
                .map(|(legacy_field, definition)| {
                    let field_id = field_id(maps, &legacy_schema.0, &legacy_field.0)?.to_owned();
                    let field_type = match definition.field_type {
                        FieldTypeV1::Number => FieldTypeV2::Number,
                        FieldTypeV1::Text => FieldTypeV2::Text,
                        FieldTypeV1::Boolean => FieldTypeV2::Boolean,
                        FieldTypeV1::Reference { schema } => FieldTypeV2::Reference {
                            schema: schema_id(maps, &schema.0)?.to_owned(),
                        },
                    };
                    Ok((
                        field_id.clone(),
                        FieldDefinitionV2 {
                            id: field_id,
                            key: legacy_field.0,
                            field_type,
                            required: definition.required,
                        },
                    ))
                })
                .collect::<Result<_, MigrationError>>()?;

            Ok((
                stable_schema_id.clone(),
                SchemaV2 {
                    id: stable_schema_id,
                    key: legacy_schema.0,
                    fields,
                },
            ))
        })
        .collect::<Result<_, MigrationError>>()?;

    let entities = document
        .entities
        .into_iter()
        .map(|(legacy_entity, entity)| {
            let entity_id = entity_id(maps, &legacy_entity.0)?.to_owned();
            let legacy_schema = entity.schema.0;
            let fields = entity
                .fields
                .into_iter()
                .map(|(legacy_field, value)| {
                    let field_id = field_id(maps, &legacy_schema, &legacy_field.0)?.to_owned();
                    Ok((field_id, rewrite_value(value, maps)?))
                })
                .collect::<Result<_, MigrationError>>()?;

            Ok((
                entity_id.clone(),
                EntityV2 {
                    id: entity_id,
                    key: legacy_entity.0,
                    schema: schema_id(maps, &legacy_schema)?.to_owned(),
                    fields,
                },
            ))
        })
        .collect::<Result<_, MigrationError>>()?;

    let migrated = DocumentV2 {
        format_version: 2,
        id: maps.document.clone(),
        title: document.title,
        schemas,
        entities,
    };
    migrated
        .validate()
        .map_err(|error| MigrationError(format!("migrated v2 graph is invalid: {error:?}")))?;
    Ok(migrated)
}

fn rewrite_value(value: ValueV1, maps: &IdentityMaps) -> Result<ValueV2, MigrationError> {
    Ok(match value {
        ValueV1::Number(number) => ValueV2::Number(normalize_zero(number)),
        ValueV1::Text(text) => ValueV2::Text(text),
        ValueV1::Boolean(boolean) => ValueV2::Boolean(boolean),
        ValueV1::Reference(entity) => ValueV2::Reference(entity_id(maps, &entity.0)?.to_owned()),
        ValueV1::Formula(expression) => ValueV2::Formula(rewrite_expression(expression, maps)?),
    })
}

fn rewrite_expression(
    expression: ExpressionV1,
    maps: &IdentityMaps,
) -> Result<ExpressionV2, MigrationError> {
    Ok(match expression {
        ExpressionV1::Number(number) => ExpressionV2::Number(normalize_zero(number)),
        ExpressionV1::Reference(reference) => {
            let legacy_entity = reference.entity.0;
            let legacy_schema = maps.entity_schemas.get(&legacy_entity).ok_or_else(|| {
                MigrationError(format!(
                    "formula reference targets missing legacy entity '{legacy_entity}'"
                ))
            })?;
            ExpressionV2::Reference(FieldRefV2 {
                entity: entity_id(maps, &legacy_entity)?.to_owned(),
                field: field_id(maps, legacy_schema, &reference.field.0)?.to_owned(),
            })
        }
        ExpressionV1::Add(args) => ExpressionV2::Add(rewrite_binary(args, maps)?),
        ExpressionV1::Subtract(args) => ExpressionV2::Subtract(rewrite_binary(args, maps)?),
        ExpressionV1::Multiply(args) => ExpressionV2::Multiply(rewrite_binary(args, maps)?),
        ExpressionV1::Divide(args) => ExpressionV2::Divide(rewrite_binary(args, maps)?),
        ExpressionV1::Minimum(args) => ExpressionV2::Minimum(rewrite_binary(args, maps)?),
        ExpressionV1::Maximum(args) => ExpressionV2::Maximum(rewrite_binary(args, maps)?),
    })
}

fn rewrite_binary(args: BinaryArgsV1, maps: &IdentityMaps) -> Result<BinaryArgsV2, MigrationError> {
    Ok(BinaryArgsV2 {
        left: Box::new(rewrite_expression(*args.left, maps)?),
        right: Box::new(rewrite_expression(*args.right, maps)?),
    })
}

fn schema_id<'a>(maps: &'a IdentityMaps, legacy: &str) -> Result<&'a str, MigrationError> {
    maps.schemas
        .get(legacy)
        .map(String::as_str)
        .ok_or_else(|| MigrationError(format!("missing legacy schema '{legacy}'")))
}

fn field_id<'a>(
    maps: &'a IdentityMaps,
    legacy_schema: &str,
    legacy_field: &str,
) -> Result<&'a str, MigrationError> {
    maps.fields
        .get(&(legacy_schema.to_owned(), legacy_field.to_owned()))
        .map(String::as_str)
        .ok_or_else(|| {
            MigrationError(format!(
                "missing legacy field '{legacy_schema}.{legacy_field}'"
            ))
        })
}

fn entity_id<'a>(maps: &'a IdentityMaps, legacy: &str) -> Result<&'a str, MigrationError> {
    maps.entities
        .get(legacy)
        .map(String::as_str)
        .ok_or_else(|| MigrationError(format!("missing legacy entity '{legacy}'")))
}

fn ensure_unique(
    kind: &str,
    generated: &mut BTreeSet<String>,
    id: &str,
) -> Result<(), MigrationError> {
    if generated.insert(id.to_owned()) {
        Ok(())
    } else {
        Err(MigrationError(format!(
            "deterministic {kind} identity collision for '{id}'"
        )))
    }
}

fn migration_id(input: &str) -> String {
    Uuid::new_v5(&LEGACY_V1_NAMESPACE, input.as_bytes()).to_string()
}

fn normalize_zero(number: f64) -> f64 {
    if number == 0.0 { 0.0 } else { number }
}

#[cfg(test)]
mod tests {
    use super::{LEGACY_V1_NAMESPACE, migration_id};

    #[test]
    fn migration_namespace_and_canonical_input_vectors_are_frozen() {
        assert_eq!(
            LEGACY_V1_NAMESPACE.to_string(),
            "7a199010-e2db-5f4f-a216-07ddb708f5ef"
        );
        assert_eq!(
            migration_id("document\0legacy-doc"),
            "1213a728-1f70-5425-a330-20a8797f5e82"
        );
        assert_eq!(
            migration_id("schema\0legacy-doc\0source"),
            "ff71fea8-d907-5234-a6be-819f6e6fdf07"
        );
        assert_eq!(
            migration_id("field\0legacy-doc\0source\0calc"),
            "32c7bf4d-e5e4-5ea0-ab43-0d42c6878cce"
        );
        assert_eq!(
            migration_id("entity\0legacy-doc\0source-entity"),
            "1832624c-a6ad-55fb-b96a-8617af123e7f"
        );
    }
}
