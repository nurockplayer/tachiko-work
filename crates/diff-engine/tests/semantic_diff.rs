use std::collections::BTreeMap;

use tachiko_diff_engine::{SemanticChange, diff};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};

fn balance_document(damage: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                id: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number_field(true)),
                    (FieldId::from("attack_interval"), number_field(true)),
                    (FieldId::from("dps"), number_field(true)),
                    (FieldId::from("burst"), number_field(true)),
                    (FieldId::from("rarity"), text_field(false)),
                    (FieldId::from("name"), text_field(true)),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("sword"),
            Entity {
                id: EntityId::from("sword"),
                schema: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), Value::Number(damage)),
                    (FieldId::from("attack_interval"), Value::Number(1.25)),
                    (
                        FieldId::from("dps"),
                        Value::Formula(Expression::Divide {
                            left: Box::new(reference("sword", "damage")),
                            right: Box::new(reference("sword", "attack_interval")),
                        }),
                    ),
                    (
                        FieldId::from("burst"),
                        Value::Formula(Expression::Multiply {
                            left: Box::new(reference("sword", "dps")),
                            right: Box::new(Expression::Number(2.0)),
                        }),
                    ),
                    (FieldId::from("name"), Value::Text("Sword".to_owned())),
                ]),
            },
        )]),
    }
}

fn number_field(required: bool) -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Number,
        required,
    }
}

fn text_field(required: bool) -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Text,
        required,
    }
}

fn boolean_field(required: bool) -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Boolean,
        required,
    }
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

#[test]
fn changed_balance_value_reports_direct_and_derived_meaning() {
    let before = balance_document(100.0);
    let after = balance_document(120.0);

    let semantic_diff = diff(&before, &after).unwrap();
    let rendered = semantic_diff.render_text();

    assert!(rendered.contains("Weapon Sword"));
    assert!(rendered.contains("damage: 100 -> 120"));
    assert!(rendered.contains("affected burst: 160 -> 192"));
    assert!(rendered.contains("affected dps: 80 -> 96"));
    assert!(semantic_diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FieldChanged { field, before, after }
            if field == &FieldRef::new("sword", "damage")
                && before == &Value::Number(100.0)
                && after == &Value::Number(120.0)
    )));
    assert!(semantic_diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::FormulaImpact { field, causes, .. }
            if field == &FieldRef::new("sword", "dps")
                && causes == &vec![FieldRef::new("sword", "damage")]
    )));
}

#[test]
fn formula_changes_render_in_canonical_copy_paste_syntax() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    after.entities.get_mut("sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Add {
            left: Box::new(Expression::Divide {
                left: Box::new(reference("sword", "damage")),
                right: Box::new(reference("sword", "attack_interval")),
            }),
            right: Box::new(Expression::Number(5.0)),
        }),
    );

    let rendered = diff(&before, &after).unwrap().render_text();

    assert!(rendered.contains(
        "dps: ([sword.damage] / [sword.attack_interval]) -> (([sword.damage] / [sword.attack_interval]) + 5)"
    ));
    assert!(rendered.contains("affected dps: 80 -> 85"));
}

#[test]
fn entity_addition_and_removal_are_semantic() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    let mut axe = after.entities.get("sword").unwrap().clone();
    axe.id = EntityId::from("axe");
    axe.fields
        .insert(FieldId::from("name"), Value::Text("Axe".to_owned()));
    axe.fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Divide {
            left: Box::new(reference("axe", "damage")),
            right: Box::new(reference("axe", "attack_interval")),
        }),
    );
    axe.fields.insert(
        FieldId::from("burst"),
        Value::Formula(Expression::Multiply {
            left: Box::new(reference("axe", "dps")),
            right: Box::new(Expression::Number(2.0)),
        }),
    );
    after.entities.remove("sword");
    after.entities.insert(EntityId::from("axe"), axe);

    let rendered = diff(&before, &after).unwrap().render_text();

    assert_eq!(
        rendered,
        "Weapon Axe\nentity added\n\nWeapon Sword\nentity removed\n"
    );
}

#[test]
fn optional_field_addition_and_removal_are_ordered() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    after
        .entities
        .get_mut("sword")
        .unwrap()
        .fields
        .insert(FieldId::from("rarity"), Value::Text("common".to_owned()));

    let added = diff(&before, &after).unwrap().render_text();
    let removed = diff(&after, &before).unwrap().render_text();

    assert!(added.contains("rarity added: \"common\""));
    assert!(removed.contains("rarity removed: \"common\""));
}

#[test]
fn document_identity_changes_are_typed_and_rendered() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    after.id = DocumentId::from("rebalanced");
    after.title = "Rebalanced".to_owned();

    let semantic_diff = diff(&before, &after).unwrap();

    assert_eq!(
        semantic_diff.changes(),
        [
            SemanticChange::DocumentIdChanged {
                before: DocumentId::from("balance"),
                after: DocumentId::from("rebalanced"),
            },
            SemanticChange::DocumentTitleChanged {
                before: "Balance".to_owned(),
                after: "Rebalanced".to_owned(),
            },
        ]
    );
    assert_eq!(
        semantic_diff.render_text(),
        "Document\nid: balance -> rebalanced\ntitle: \"Balance\" -> \"Rebalanced\"\n"
    );
}

#[test]
fn schema_addition_and_removal_preserve_typed_definitions() {
    let before = balance_document(100.0);
    let mut after = before.clone();
    let armor = Schema {
        id: SchemaId::from("armor"),
        fields: BTreeMap::from([(
            FieldId::from("defense"),
            FieldDefinition {
                field_type: FieldType::Number,
                required: false,
            },
        )]),
    };
    after.schemas.insert(SchemaId::from("armor"), armor.clone());

    let added = diff(&before, &after).unwrap();
    let removed = diff(&after, &before).unwrap();

    assert_eq!(
        added.changes(),
        [SemanticChange::SchemaAdded {
            schema: SchemaId::from("armor"),
            definition: armor.clone(),
        }]
    );
    assert_eq!(added.render_text(), "Schema armor\nschema added\n");
    assert_eq!(
        removed.changes(),
        [SemanticChange::SchemaRemoved {
            schema: SchemaId::from("armor"),
            definition: armor,
        }]
    );
    assert_eq!(removed.render_text(), "Schema armor\nschema removed\n");
}

#[test]
fn schema_field_changes_preserve_typed_definitions_and_order() {
    let mut before = balance_document(100.0);
    before.schemas.insert(
        SchemaId::from("metadata"),
        Schema {
            id: SchemaId::from("metadata"),
            fields: BTreeMap::from([
                (FieldId::from("legacy"), text_field(false)),
                (FieldId::from("mode"), text_field(false)),
            ]),
        },
    );
    let mut after = before.clone();
    let fields = &mut after.schemas.get_mut("metadata").unwrap().fields;
    fields.remove("legacy");
    fields.insert(FieldId::from("mode"), boolean_field(false));
    fields.insert(FieldId::from("weight"), number_field(false));

    let semantic_diff = diff(&before, &after).unwrap();

    assert_eq!(
        semantic_diff.changes(),
        [
            SemanticChange::SchemaFieldRemoved {
                schema: SchemaId::from("metadata"),
                field: FieldId::from("legacy"),
                definition: text_field(false),
            },
            SemanticChange::SchemaFieldChanged {
                schema: SchemaId::from("metadata"),
                field: FieldId::from("mode"),
                before: text_field(false),
                after: boolean_field(false),
            },
            SemanticChange::SchemaFieldAdded {
                schema: SchemaId::from("metadata"),
                field: FieldId::from("weight"),
                definition: number_field(false),
            },
        ]
    );
    assert_eq!(
        semantic_diff.render_text(),
        "Schema metadata\nlegacy removed: text (optional)\nmode: text (optional) -> boolean (optional)\nweight added: number (optional)\n"
    );
}

#[test]
fn unchanged_documents_have_an_explicit_empty_summary() {
    let document = balance_document(100.0);

    let semantic_diff = diff(&document, &document).unwrap();

    assert!(semantic_diff.changes().is_empty());
    assert_eq!(semantic_diff.render_text(), "No semantic changes.\n");
}
