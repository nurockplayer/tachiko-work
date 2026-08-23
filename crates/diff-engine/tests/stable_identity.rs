use std::collections::BTreeMap;

use tachiko_diff_engine::{SemanticChange, diff};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn document() -> Document {
    let schema_id = SchemaId::from("schema-stable");
    let field_id = FieldId::from("field-stable");
    let entity_id = EntityId::from("entity-stable");
    Document {
        id: DocumentId::from("document-stable"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from("weapon"),
                fields: BTreeMap::from([(
                    field_id.clone(),
                    FieldDefinition {
                        id: field_id.clone(),
                        key: FieldKey::from("damage"),
                        field_type: FieldType::Number,
                        required: true,
                    },
                )]),
            },
        )]),
        entities: BTreeMap::from([(
            entity_id.clone(),
            Entity {
                id: entity_id,
                key: EntityKey::from("iron_sword"),
                schema: schema_id,
                fields: BTreeMap::from([(field_id, Value::Number(Number::new(50.0).unwrap()))]),
            },
        )]),
    }
}

#[test]
fn key_renames_are_continuity_changes_not_remove_and_add() {
    let before = document();
    let mut after = before.clone();
    after.schemas.get_mut("schema-stable").unwrap().key = SchemaKey::from("equipment");
    after
        .schemas
        .get_mut("schema-stable")
        .unwrap()
        .fields
        .get_mut("field-stable")
        .unwrap()
        .key = FieldKey::from("power");
    after.entities.get_mut("entity-stable").unwrap().key = EntityKey::from("moonblade");

    let result = diff(&before, &after).unwrap();

    assert_eq!(
        result.changes(),
        [
            SemanticChange::SchemaKeyChanged {
                schema: SchemaId::from("schema-stable"),
                before: SchemaKey::from("weapon"),
                after: SchemaKey::from("equipment"),
            },
            SemanticChange::FieldKeyChanged {
                schema: SchemaId::from("schema-stable"),
                field: FieldId::from("field-stable"),
                before: FieldKey::from("damage"),
                after: FieldKey::from("power"),
            },
            SemanticChange::EntityKeyChanged {
                entity: EntityId::from("entity-stable"),
                before: EntityKey::from("iron_sword"),
                after: EntityKey::from("moonblade"),
            },
        ]
    );
    assert!(!result.render_text().contains("added"));
    assert!(!result.render_text().contains("removed"));
}
