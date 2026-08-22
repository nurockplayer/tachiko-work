use std::collections::{BTreeMap, VecDeque};

use tachiko_formula_engine::project_expression;
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use tachiko_workflow::{
    IdGenerator, SemanticIdKind, WorkflowError, duplicate_entity, rename_entity, rename_field,
    rename_schema,
};

struct DeterministicGenerator {
    ids: VecDeque<String>,
}

impl DeterministicGenerator {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        }
    }
}

impl IdGenerator for DeterministicGenerator {
    fn generate(&mut self, _kind: SemanticIdKind) -> String {
        self.ids.pop_front().unwrap()
    }
}

fn document() -> Document {
    let schema_id = SchemaId::from("schema-stable");
    let damage_id = FieldId::from("damage-stable");
    let dps_id = FieldId::from("dps-stable");
    let source_id = EntityId::from("source-stable");
    let owner_id = EntityId::from("owner-stable");
    Document {
        id: DocumentId::from("document-stable"),
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
        entities: BTreeMap::from([
            (
                source_id.clone(),
                Entity {
                    id: source_id.clone(),
                    key: EntityKey::from("iron_sword"),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([
                        (damage_id.clone(), Value::Number(Number::new(50.0).unwrap())),
                        (dps_id.clone(), Value::Number(Number::new(50.0).unwrap())),
                    ]),
                },
            ),
            (
                owner_id.clone(),
                Entity {
                    id: owner_id,
                    key: EntityKey::from("shop"),
                    schema: schema_id,
                    fields: BTreeMap::from([
                        (damage_id, Value::Number(Number::new(1.0).unwrap())),
                        (
                            dps_id,
                            Value::Formula(Expression::Reference(FieldRef::new(
                                source_id,
                                "damage-stable",
                            ))),
                        ),
                    ]),
                },
            ),
        ]),
    }
}

#[test]
fn entity_field_and_schema_rename_preserve_ids_and_bound_meaning() {
    let original = document();
    let Value::Formula(bound) = original.entities["owner-stable"].fields["dps-stable"].clone()
    else {
        unreachable!()
    };

    let entity = rename_entity(&original, "iron_sword", "moonblade").unwrap();
    let field = rename_field(&entity.document, "weapon", "damage", "power").unwrap();
    let schema = rename_schema(&field.document, "weapon", "equipment").unwrap();

    assert_eq!(
        schema.document.entities["source-stable"].id,
        EntityId::from("source-stable")
    );
    assert_eq!(
        schema.document.schemas["schema-stable"].id,
        SchemaId::from("schema-stable")
    );
    assert_eq!(
        schema.document.schemas["schema-stable"].fields["damage-stable"].id,
        FieldId::from("damage-stable")
    );
    assert_eq!(
        schema.document.entities["owner-stable"].fields["dps-stable"],
        Value::Formula(bound.clone())
    );
    assert_eq!(
        project_expression(&schema.document, &bound).unwrap(),
        "[moonblade.power]"
    );
}

#[test]
fn duplicate_gets_a_new_generated_id_and_only_rebases_formula_self_references() {
    let source_id = EntityId::from("source-stable");
    let mut source = document();
    source.entities.get_mut(&source_id).unwrap().fields.insert(
        FieldId::from("dps-stable"),
        Value::Formula(Expression::Reference(FieldRef::new(
            source_id.clone(),
            "damage-stable",
        ))),
    );
    let mut generator = DeterministicGenerator::new(&["copy-stable"]);

    let preview = duplicate_entity(&source, "iron_sword", "moonblade", &mut generator).unwrap();

    let copy = &preview.document.entities["copy-stable"];
    assert_eq!(copy.key, EntityKey::from("moonblade"));
    assert_eq!(
        copy.fields["dps-stable"],
        Value::Formula(Expression::Reference(FieldRef::new(
            "copy-stable",
            "damage-stable"
        )))
    );
}

#[test]
fn rename_projection_accepts_4096_bytes_and_rejects_4097_atomically() {
    let source = document();
    // `[<entity>.damage]` adds nine bytes around the entity key.
    let accepted_key = "a".repeat(4_087);
    let rejected_key = "a".repeat(4_088);

    let accepted = rename_entity(&source, "iron_sword", &accepted_key).unwrap();
    let error = rename_entity(&source, "iron_sword", &rejected_key).unwrap_err();

    let Value::Formula(expression) =
        &accepted.document.entities["owner-stable"].fields["dps-stable"]
    else {
        unreachable!()
    };
    assert_eq!(
        project_expression(&accepted.document, expression)
            .unwrap()
            .len(),
        4_096
    );
    assert!(matches!(error, WorkflowError::FormulaProjection { .. }));
    assert_eq!(
        source.entities["source-stable"].key,
        EntityKey::from("iron_sword")
    );
}
