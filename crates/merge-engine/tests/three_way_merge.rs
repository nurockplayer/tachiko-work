use std::collections::BTreeMap;

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_merge_engine::{MergeError, MergeOutcome, MergeSide, MergeValue, merge};
use tachiko_semantic_core::{
    DiagnosticCode, Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn balance_document(damage: f64, attack_interval: f64) -> Document {
    Document {
        id: DocumentId::from("balance"),
        title: "Balance".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("weapon"),
            Schema {
                id: SchemaId::from("weapon"),
                key: SchemaKey::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number_field("damage")),
                    (
                        FieldId::from("attack_interval"),
                        number_field("attack_interval"),
                    ),
                    (FieldId::from("dps"), number_field("dps")),
                ]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("iron_sword"),
            Entity {
                id: EntityId::from("iron_sword"),
                key: "iron_sword".into(),
                schema: SchemaId::from("weapon"),
                fields: BTreeMap::from([
                    (FieldId::from("damage"), number(damage)),
                    (FieldId::from("attack_interval"), number(attack_interval)),
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

fn field(id: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        field_type,
        required,
    }
}

fn number_field(id: &str) -> FieldDefinition {
    field(id, FieldType::Number, true)
}

fn optional_number_field(id: &str) -> FieldDefinition {
    field(id, FieldType::Number, false)
}

fn text_field(id: &str) -> FieldDefinition {
    field(id, FieldType::Text, false)
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn expected(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn with_bonus(mut document: Document) -> Document {
    document
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), optional_number_field("bonus"));
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), number(4.0));
    document
}

fn with_marker_schema(mut document: Document) -> Document {
    document.schemas.insert(
        SchemaId::from("marker"),
        Schema {
            id: SchemaId::from("marker"),
            key: SchemaKey::from("marker"),
            fields: BTreeMap::new(),
        },
    );
    document
}

fn marker_entity(id: &str) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: id.into(),
        schema: SchemaId::from("marker"),
        fields: BTreeMap::new(),
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
    assert_eq!(merged.entities["iron_sword"].fields["damage"], number(45.0));
    assert_eq!(
        merged.entities["iron_sword"].fields["attack_interval"],
        number(0.8)
    );
    assert_eq!(
        calculate(&merged)
            .unwrap()
            .value(&FieldRef::new("iron_sword", "dps")),
        Some(expected(56.25))
    );
}

#[test]
fn key_rename_and_value_edit_merge_by_stable_identity() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.entities.get_mut("iron_sword").unwrap().key = "moonblade".into();
    let mut theirs = base.clone();
    theirs
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), number(45.0));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("a key rename and independent value edit should merge")
    };

    let entity = &merged.entities["iron_sword"];
    assert_eq!(entity.id, EntityId::from("iron_sword"));
    assert_eq!(entity.key.as_str(), "moonblade");
    assert_eq!(entity.fields["damage"], number(45.0));
    assert_eq!(
        entity.fields["dps"], base.entities["iron_sword"].fields["dps"],
        "bound formula identity must not be rewritten during a key merge"
    );
}

#[test]
fn divergent_key_renames_conflict_on_the_same_stable_object() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.entities.get_mut("iron_sword").unwrap().key = "moonblade".into();
    let mut theirs = base.clone();
    theirs.entities.get_mut("iron_sword").unwrap().key = "sunblade".into();

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("divergent key renames should conflict")
    };

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "entities.iron_sword.key");
    assert_eq!(
        conflicts[0].base,
        Some(MergeValue::EntityKey("iron_sword".into()))
    );
    assert_eq!(
        conflicts[0].ours,
        Some(MergeValue::EntityKey("moonblade".into()))
    );
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::EntityKey("sunblade".into()))
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

#[test]
fn independent_document_id_and_title_changes_merge() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.id = DocumentId::from("rebalanced");
    let mut theirs = base.clone();
    theirs.title = "Rebalanced".to_owned();

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent document identity changes should merge");
    };

    assert_eq!(merged.id, DocumentId::from("rebalanced"));
    assert_eq!(merged.title, "Rebalanced");
}

#[test]
fn identical_two_sided_document_identity_change_merges() {
    let base = balance_document(36.0, 0.9);
    let mut changed = base.clone();
    changed.id = DocumentId::from("rebalanced");
    changed.title = "Rebalanced".to_owned();

    let MergeOutcome::Merged(merged) = merge(&base, &changed, &changed).unwrap() else {
        panic!("identical document identity changes should merge");
    };

    assert_eq!(merged.id, DocumentId::from("rebalanced"));
    assert_eq!(merged.title, "Rebalanced");
}

#[test]
fn one_sided_schema_addition_merges() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.schemas.insert(
        SchemaId::from("armor"),
        Schema {
            id: SchemaId::from("armor"),
            key: SchemaKey::from("armor"),
            fields: BTreeMap::new(),
        },
    );

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided schema addition should merge");
    };

    assert_eq!(merged.schemas["armor"].id, SchemaId::from("armor"));
}

#[test]
fn identical_two_sided_schema_addition_merges() {
    let base = balance_document(36.0, 0.9);
    let mut changed = base.clone();
    changed.schemas.insert(
        SchemaId::from("armor"),
        Schema {
            id: SchemaId::from("armor"),
            key: SchemaKey::from("armor"),
            fields: BTreeMap::new(),
        },
    );

    let MergeOutcome::Merged(merged) = merge(&base, &changed, &changed).unwrap() else {
        panic!("an identical two-sided schema addition should merge");
    };

    assert_eq!(merged.schemas["armor"].id, SchemaId::from("armor"));
}

#[test]
fn independent_schema_additions_merge() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.schemas.insert(
        SchemaId::from("armor"),
        Schema {
            id: SchemaId::from("armor"),
            key: SchemaKey::from("armor"),
            fields: BTreeMap::new(),
        },
    );
    let mut theirs = base.clone();
    theirs.schemas.insert(
        SchemaId::from("potion"),
        Schema {
            id: SchemaId::from("potion"),
            key: SchemaKey::from("potion"),
            fields: BTreeMap::new(),
        },
    );

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent schema additions should merge");
    };

    assert!(merged.schemas.contains_key("armor"));
    assert!(merged.schemas.contains_key("potion"));
}

#[test]
fn schema_deletion_merges_when_the_other_side_is_unchanged() {
    let base = with_marker_schema(balance_document(36.0, 0.9));
    let mut ours = base.clone();
    ours.schemas.remove("marker");

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided unused-schema deletion should merge");
    };

    assert!(!merged.schemas.contains_key("marker"));
}

#[test]
fn one_sided_schema_field_addition_merges() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), optional_number_field("weight"));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided schema field addition should merge");
    };

    assert_eq!(
        merged.schemas["weapon"].fields["weight"],
        optional_number_field("weight")
    );
}

#[test]
fn identical_two_sided_schema_field_addition_merges() {
    let base = balance_document(36.0, 0.9);
    let mut changed = base.clone();
    changed
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), optional_number_field("weight"));

    let MergeOutcome::Merged(merged) = merge(&base, &changed, &changed).unwrap() else {
        panic!("an identical two-sided schema field addition should merge");
    };

    assert_eq!(
        merged.schemas["weapon"].fields["weight"],
        optional_number_field("weight")
    );
}

#[test]
fn independent_schema_field_additions_merge() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), optional_number_field("bonus"));
    let mut theirs = base.clone();
    theirs
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), optional_number_field("weight"));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent schema field additions should merge");
    };

    assert_eq!(
        merged.schemas["weapon"].fields["bonus"],
        optional_number_field("bonus")
    );
    assert_eq!(
        merged.schemas["weapon"].fields["weight"],
        optional_number_field("weight")
    );
}

#[test]
fn independent_existing_schema_field_definition_changes_merge() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("damage"), optional_number_field("damage"));
    let mut theirs = base.clone();
    theirs.schemas.get_mut("weapon").unwrap().fields.insert(
        FieldId::from("attack_interval"),
        optional_number_field("attack_interval"),
    );

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent schema field definition changes should merge");
    };

    assert!(!merged.schemas["weapon"].fields["damage"].required);
    assert!(!merged.schemas["weapon"].fields["attack_interval"].required);
}

#[test]
fn schema_field_deletion_merges_when_the_other_side_is_unchanged() {
    let mut base = balance_document(36.0, 0.9);
    base.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), optional_number_field("weight"));
    let mut ours = base.clone();
    ours.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .remove("weight");

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided unused schema field deletion should merge");
    };

    assert!(!merged.schemas["weapon"].fields.contains_key("weight"));
}

#[test]
fn one_sided_entity_addition_merges() {
    let base = with_marker_schema(balance_document(36.0, 0.9));
    let mut ours = base.clone();
    ours.entities
        .insert(EntityId::from("rare"), marker_entity("rare"));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided entity addition should merge");
    };

    assert_eq!(merged.entities["rare"], marker_entity("rare"));
}

#[test]
fn identical_two_sided_entity_addition_merges() {
    let base = with_marker_schema(balance_document(36.0, 0.9));
    let mut changed = base.clone();
    changed
        .entities
        .insert(EntityId::from("rare"), marker_entity("rare"));

    let MergeOutcome::Merged(merged) = merge(&base, &changed, &changed).unwrap() else {
        panic!("an identical two-sided entity addition should merge");
    };

    assert_eq!(merged.entities["rare"], marker_entity("rare"));
}

#[test]
fn independent_entity_additions_merge() {
    let base = with_marker_schema(balance_document(36.0, 0.9));
    let mut ours = base.clone();
    ours.entities
        .insert(EntityId::from("rare"), marker_entity("rare"));
    let mut theirs = base.clone();
    theirs
        .entities
        .insert(EntityId::from("legendary"), marker_entity("legendary"));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent entity additions should merge");
    };

    assert_eq!(merged.entities["rare"], marker_entity("rare"));
    assert_eq!(merged.entities["legendary"], marker_entity("legendary"));
}

#[test]
fn entity_deletion_merges_when_the_other_side_is_unchanged() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.entities.remove("iron_sword");

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided entity deletion should merge");
    };

    assert!(!merged.entities.contains_key("iron_sword"));
}

#[test]
fn one_sided_entity_field_addition_merges() {
    let mut base = balance_document(36.0, 0.9);
    base.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), optional_number_field("bonus"));
    let mut ours = base.clone();
    ours.entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), number(4.0));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided entity field addition should merge");
    };

    assert_eq!(merged.entities["iron_sword"].fields["bonus"], number(4.0));
}

#[test]
fn identical_two_sided_entity_field_addition_merges() {
    let mut base = balance_document(36.0, 0.9);
    base.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), optional_number_field("bonus"));
    let mut changed = base.clone();
    changed
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), number(4.0));

    let MergeOutcome::Merged(merged) = merge(&base, &changed, &changed).unwrap() else {
        panic!("an identical two-sided entity field addition should merge");
    };

    assert_eq!(merged.entities["iron_sword"].fields["bonus"], number(4.0));
}

#[test]
fn independent_entity_field_additions_merge() {
    let mut base = balance_document(36.0, 0.9);
    base.schemas.get_mut("weapon").unwrap().fields.extend([
        (FieldId::from("bonus"), optional_number_field("bonus")),
        (FieldId::from("weight"), optional_number_field("weight")),
    ]);
    let mut ours = base.clone();
    ours.entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), number(4.0));
    let mut theirs = base.clone();
    theirs
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), number(8.0));

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent entity field additions should merge");
    };

    assert_eq!(merged.entities["iron_sword"].fields["bonus"], number(4.0));
    assert_eq!(merged.entities["iron_sword"].fields["weight"], number(8.0));
}

#[test]
fn entity_field_deletion_merges_when_the_other_side_is_unchanged() {
    let base = with_bonus(balance_document(36.0, 0.9));
    let mut ours = base.clone();
    ours.entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .remove("bonus");

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided entity field deletion should merge");
    };

    assert!(!merged.entities["iron_sword"].fields.contains_key("bonus"));
}

#[test]
fn entity_schema_membership_change_merges_when_the_other_side_is_unchanged() {
    let mut base = balance_document(36.0, 0.9);
    let mut alternate = base.schemas["weapon"].clone();
    alternate.id = SchemaId::from("alternate_weapon");
    alternate.key = SchemaKey::from("alternate_weapon");
    base.schemas
        .insert(SchemaId::from("alternate_weapon"), alternate);
    let mut ours = base.clone();
    ours.entities.get_mut("iron_sword").unwrap().schema = SchemaId::from("alternate_weapon");

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &base).unwrap() else {
        panic!("a one-sided entity schema-membership change should merge");
    };

    assert_eq!(
        merged.entities["iron_sword"].schema,
        SchemaId::from("alternate_weapon")
    );
}

#[test]
fn same_field_divergence_returns_the_typed_conflict_payload() {
    let base = balance_document(36.0, 0.9);
    let ours = balance_document(45.0, 0.9);
    let theirs = balance_document(50.0, 0.9);

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("different edits to the same field should conflict");
    };

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "entities.iron_sword.fields.damage");
    assert_eq!(
        conflicts[0].base,
        Some(MergeValue::FieldValue(number(36.0)))
    );
    assert_eq!(
        conflicts[0].ours,
        Some(MergeValue::FieldValue(number(45.0)))
    );
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::FieldValue(number(50.0)))
    );
}

#[test]
fn delete_versus_modify_returns_the_optional_entry_conflict_payload() {
    let base = with_bonus(balance_document(36.0, 0.9));
    let mut ours = base.clone();
    ours.entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .remove("bonus");
    let mut theirs = base.clone();
    theirs
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), number(8.0));

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("a deletion and an edit to the same field should conflict");
    };

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "entities.iron_sword.fields.bonus");
    assert_eq!(conflicts[0].base, Some(MergeValue::FieldValue(number(4.0))));
    assert_eq!(conflicts[0].ours, None);
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::FieldValue(number(8.0)))
    );
}

#[test]
fn different_concurrent_field_additions_return_the_typed_conflict_payload() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), optional_number_field("weight"));
    let mut theirs = base.clone();
    theirs
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), text_field("weight"));

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("different concurrent additions should conflict");
    };

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "schemas.weapon.fields.weight");
    assert_eq!(conflicts[0].base, None);
    assert_eq!(
        conflicts[0].ours,
        Some(MergeValue::FieldDefinition(optional_number_field("weight")))
    );
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::FieldDefinition(text_field("weight")))
    );
}

#[test]
fn conflicts_are_returned_in_lexical_path_order() {
    let base = balance_document(36.0, 0.9);
    let ours = balance_document(45.0, 0.8);
    let theirs = balance_document(50.0, 0.7);

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("two divergent fields should conflict");
    };

    assert_eq!(
        conflicts
            .iter()
            .map(|conflict| conflict.path.as_str())
            .collect::<Vec<_>>(),
        [
            "entities.iron_sword.fields.attack_interval",
            "entities.iron_sword.fields.damage",
        ]
    );
}

#[test]
fn invalid_input_reports_the_side_and_validation_diagnostics() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.entities.get_mut("iron_sword").unwrap().fields.insert(
        FieldId::from("damage"),
        Value::Text("forty-five".to_owned()),
    );

    let error = merge(&base, &ours, &base).unwrap_err();

    let MergeError::InvalidInput { side, diagnostics } = error else {
        panic!("invalid input should report validation diagnostics");
    };
    assert_eq!(side, MergeSide::Ours);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "entities.iron_sword.fields.damage"
            && diagnostic.code == DiagnosticCode::TypeMismatch
    }));
}

#[test]
fn uncalculable_input_reports_the_side() {
    let base = balance_document(36.0, 0.9);
    let mut ours = base.clone();
    ours.entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));

    let error = merge(&base, &ours, &base).unwrap_err();

    assert!(matches!(
        error,
        MergeError::InputCalculation {
            side: MergeSide::Ours,
            source: CalculationError::DivisionByZero { .. },
        }
    ));
}

#[test]
fn combined_broken_reference_is_rejected_as_an_invalid_merged_document() {
    let base = with_bonus(balance_document(36.0, 0.9));
    let mut ours = base.clone();
    ours.entities.get_mut("iron_sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Reference(FieldRef::new("iron_sword", "bonus"))),
    );
    let mut theirs = base.clone();
    theirs
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .remove("bonus");
    theirs
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .remove("bonus");

    let error = merge(&base, &ours, &theirs).unwrap_err();

    let MergeError::InvalidMergedDocument { diagnostics } = error else {
        panic!("a broken combined reference should reject the candidate");
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "entities.iron_sword.fields.dps"
            && diagnostic.code == DiagnosticCode::MissingFormulaReference
    }));
}

#[test]
fn combined_division_by_zero_is_rejected_as_a_merged_calculation_error() {
    let mut base = balance_document(36.0, 0.9);
    base.entities.get_mut("iron_sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Divide {
            left: Box::new(Expression::Reference(FieldRef::new("iron_sword", "damage"))),
            right: Box::new(Expression::Reference(FieldRef::new("iron_sword", "damage"))),
        }),
    );
    let mut ours = base.clone();
    ours.entities.get_mut("iron_sword").unwrap().fields.insert(
        FieldId::from("dps"),
        Value::Formula(Expression::Divide {
            left: Box::new(Expression::Reference(FieldRef::new("iron_sword", "damage"))),
            right: Box::new(Expression::Reference(FieldRef::new(
                "iron_sword",
                "attack_interval",
            ))),
        }),
    );
    let mut theirs = base.clone();
    theirs
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));

    let error = merge(&base, &ours, &theirs).unwrap_err();

    assert!(matches!(
        error,
        MergeError::MergedCalculation(CalculationError::DivisionByZero { .. })
    ));
}
