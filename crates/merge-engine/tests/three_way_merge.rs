use std::collections::BTreeMap;

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_merge_engine::{MergeError, MergeOutcome, MergeSide, MergeValue, merge};
use tachiko_semantic_core::{
    DiagnosticCode, Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId,
    FieldRef, FieldType, Schema, SchemaId, Value,
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

fn optional_number_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Number,
        required: false,
    }
}

fn text_field() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Text,
        required: false,
    }
}

fn with_bonus(mut document: Document) -> Document {
    document.schemas.get_mut("weapon").unwrap().fields.insert(
        FieldId::from("bonus"),
        FieldDefinition {
            field_type: FieldType::Number,
            required: false,
        },
    );
    document
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("bonus"), Value::Number(4.0));
    document
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
        Some(MergeValue::FieldValue(Value::Number(36.0)))
    );
    assert_eq!(
        conflicts[0].ours,
        Some(MergeValue::FieldValue(Value::Number(45.0)))
    );
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::FieldValue(Value::Number(50.0)))
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
        .insert(FieldId::from("bonus"), Value::Number(8.0));

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("a deletion and an edit to the same field should conflict");
    };

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "entities.iron_sword.fields.bonus");
    assert_eq!(
        conflicts[0].base,
        Some(MergeValue::FieldValue(Value::Number(4.0)))
    );
    assert_eq!(conflicts[0].ours, None);
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::FieldValue(Value::Number(8.0)))
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
        .insert(FieldId::from("weight"), optional_number_field());
    let mut theirs = base.clone();
    theirs
        .schemas
        .get_mut("weapon")
        .unwrap()
        .fields
        .insert(FieldId::from("weight"), text_field());

    let MergeOutcome::Conflicted(conflicts) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("different concurrent additions should conflict");
    };

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].path, "schemas.weapon.fields.weight");
    assert_eq!(conflicts[0].base, None);
    assert_eq!(
        conflicts[0].ours,
        Some(MergeValue::FieldDefinition(optional_number_field()))
    );
    assert_eq!(
        conflicts[0].theirs,
        Some(MergeValue::FieldDefinition(text_field()))
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
        .insert(FieldId::from("attack_interval"), Value::Number(0.0));

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
        .insert(FieldId::from("attack_interval"), Value::Number(0.0));

    let error = merge(&base, &ours, &theirs).unwrap_err();

    assert!(matches!(
        error,
        MergeError::MergedCalculation(CalculationError::DivisionByZero { .. })
    ));
}
