use std::collections::BTreeMap;

use tachiko_formula_engine::calculate;
use tachiko_merge_engine::{MergeOutcome, merge};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};

fn balance_document(damage: f64, attack_interval: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                id: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number_field()),
                    (FieldId::from("attack_interval"), number_field()),
                    (FieldId::from("dps"), number_field()),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("iron_sword"),
            Entity {
                id: EntityId::from("iron_sword"),
                schema: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), Value::Number(damage)),
                    (
                        FieldId::from("attack_interval"),
                        Value::Number(attack_interval),
                    ),
                    (
                        FieldId::from("dps"),
                        Value::Formula(Expression::Divide {
                            left: Box::new(Expression::Reference(FieldRef::new(
                                "iron_sword",
                                "damage",
                            ))),
                            right: Box::new(Expression::Reference(FieldRef::new(
                                "iron_sword",
                                "attack_interval",
                            ))),
                        }),
                    ),
                ]),
            },
        )]),
    }
}

fn number_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Number,
        required: true,
    }
}

#[test]
fn independent_fields_on_the_same_entity_merge() {
    let base = balance_document(36.0, 0.9);
    let ours = balance_document(45.0, 0.9);
    let theirs = balance_document(36.0, 0.8);

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent edits should merge");
    };
    assert_eq!(
        merged.entities["iron_sword"].fields["damage"],
        Value::Number(45.0)
    );
    assert_eq!(
        merged.entities["iron_sword"].fields["attack_interval"],
        Value::Number(0.8)
    );
    assert_eq!(
        calculate(&merged)
            .unwrap()
            .value(&FieldRef::new("iron_sword", "dps")),
        Some(56.25)
    );
}

#[test]
fn identical_two_sided_change_is_not_a_conflict() {
    let base = balance_document(36.0, 0.9);
    let ours = balance_document(45.0, 0.9);
    let theirs = ours.clone();

    assert!(matches!(
        merge(&base, &ours, &theirs).unwrap(),
        MergeOutcome::Merged(_)
    ));
}
