use std::collections::BTreeMap;

use tachiko_semantic_core::{
    AddressIndex, AddressIndexError, Document, DocumentId, Entity, EntityId, EntityKey, Expression,
    FieldAddress, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema,
    SchemaId, SchemaKey, Value, validate_document,
};

fn identity_document() -> Document {
    let schema_id = SchemaId::from("opaque-schema-token");
    let damage_id = FieldId::from("opaque-damage-token");
    let dps_id = FieldId::from("opaque-dps-token");
    let entity_id = EntityId::from("opaque-entity-token");

    Document {
        id: DocumentId::from("not-a-uuid"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("weapon"),
                fields: BTreeMap::from([
                    (
                        damage_id.clone(),
                        FieldDefinition {
                            id: damage_id.clone(),
                            key: FieldKey::from("damage"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                    (
                        dps_id.clone(),
                        FieldDefinition {
                            id: dps_id.clone(),
                            key: FieldKey::from("dps"),
                            field_type: FieldType::Number,
                            required: true,
                        },
                    ),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id.clone(),
                key: EntityKey::from("iron_sword"),
                schema: schema_id,
                fields: BTreeMap::from([
                    (damage_id.clone(), Value::Number(Number::new(50.0).unwrap())),
                    (
                        dps_id,
                        Value::Formula(Expression::Reference(FieldRef::new(entity_id, damage_id))),
                    ),
                ]),
            },
        )]),
    }
}

#[test]
fn opaque_ids_and_mutable_keys_are_distinct_semantic_concepts() {
    let mut document = identity_document();
    assert!(validate_document(&document).is_empty());

    let before_entity_id = document
        .resolve_field(&FieldAddress::new("iron_sword", "dps"))
        .unwrap()
        .entity;
    let before_formula = document
        .entities
        .get(&before_entity_id)
        .unwrap()
        .fields
        .values()
        .find(|value| matches!(value, Value::Formula(_)))
        .cloned()
        .unwrap();

    document.entities.get_mut(&before_entity_id).unwrap().key = EntityKey::from("moonblade");

    let renamed = document
        .resolve_field(&FieldAddress::new("moonblade", "dps"))
        .unwrap();
    assert_eq!(renamed.entity, before_entity_id);
    assert_eq!(
        document
            .entities
            .get(&before_entity_id)
            .unwrap()
            .fields
            .values()
            .find(|value| matches!(value, Value::Formula(_)))
            .unwrap(),
        &before_formula
    );
    assert!(validate_document(&document).is_empty());
}

#[test]
fn address_index_reports_duplicate_entity_keys_in_stable_id_order() {
    let mut document = identity_document();
    let first = EntityId::from("a-stable-id");
    let second = EntityId::from("z-stable-id");
    let template = document.entities.values().next().unwrap().clone();
    document.entities.clear();
    document.entities.insert(
        second.clone(),
        Entity {
            id: second.clone(),
            ..template.clone()
        },
    );
    document.entities.insert(
        first.clone(),
        Entity {
            id: first.clone(),
            ..template
        },
    );

    let error = AddressIndex::build(&document).unwrap_err();

    assert_eq!(
        error,
        AddressIndexError::DuplicateEntityKey {
            key: EntityKey::from("iron_sword"),
            ids: vec![first, second],
        }
    );
}

#[test]
fn address_index_reports_duplicate_schema_keys_in_stable_id_order() {
    let mut document = identity_document();
    let first = SchemaId::from("a-stable-id");
    let second = SchemaId::from("z-stable-id");
    let template = document.schemas.values().next().unwrap().clone();
    document.schemas.clear();
    document.schemas.insert(
        second.clone(),
        Schema {
            id: second.clone(),
            ..template.clone()
        },
    );
    document.schemas.insert(
        first.clone(),
        Schema {
            id: first.clone(),
            ..template
        },
    );

    assert_eq!(
        AddressIndex::build(&document).unwrap_err(),
        AddressIndexError::DuplicateSchemaKey {
            key: SchemaKey::from("weapon"),
            ids: vec![first, second],
        }
    );
}

#[test]
fn address_index_reports_duplicate_field_keys_in_stable_id_order() {
    let mut document = identity_document();
    let schema = document.schemas.get_mut("opaque-schema-token").unwrap();
    let template = schema.fields.values().next().unwrap().clone();
    let first = FieldId::from("a-stable-id");
    let second = FieldId::from("z-stable-id");
    schema.fields.clear();
    schema.fields.insert(
        second.clone(),
        FieldDefinition {
            id: second.clone(),
            ..template.clone()
        },
    );
    schema.fields.insert(
        first.clone(),
        FieldDefinition {
            id: first.clone(),
            ..template
        },
    );

    assert_eq!(
        AddressIndex::build(&document).unwrap_err(),
        AddressIndexError::DuplicateFieldKey {
            schema: SchemaId::from("opaque-schema-token"),
            key: FieldKey::from("damage"),
            ids: vec![first, second],
        }
    );
}

#[test]
fn number_rejects_non_finite_values_and_normalizes_zero() {
    let positive_zero = Number::new(0.0).unwrap();
    let negative_zero = Number::new(-0.0).unwrap();

    assert_eq!(positive_zero, negative_zero);
    assert_eq!(negative_zero.to_bits(), 0);
    assert!(Number::new(f64::NAN).is_err());
    assert!(Number::new(f64::INFINITY).is_err());
    assert!(Number::new(f64::NEG_INFINITY).is_err());
}
