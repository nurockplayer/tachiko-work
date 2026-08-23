mod common;

use std::{env, mem, process::Command};

use common::{OneIdGenerator, game_balance_document};
use tachiko_diff_engine::SemanticChange;
use tachiko_formula_engine::calculate;
use tachiko_semantic_core::{
    EntityId, EntityKey, Expression, FieldAddress, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, Number, Value, validate_document,
};
use tachiko_workspace_engine::{
    WorkspaceError, duplicate_entity, remove_entity, rename_entity, rename_field, rename_schema,
    set_scalar, validate_field_value_suggestion,
};

fn numeric(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn expected(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn every_expression_shape(self_entity: &str) -> Expression {
    Expression::Maximum {
        left: Box::new(Expression::Minimum {
            left: Box::new(Expression::Add {
                left: Box::new(reference(self_entity, "damage")),
                right: Box::new(numeric(1.0)),
            }),
            right: Box::new(Expression::Subtract {
                left: Box::new(Expression::Multiply {
                    left: Box::new(Expression::Divide {
                        left: Box::new(reference(self_entity, "damage")),
                        right: Box::new(reference(self_entity, "attack_interval")),
                    }),
                    right: Box::new(numeric(2.0)),
                }),
                right: Box::new(numeric(3.0)),
            }),
        }),
        right: Box::new(reference("shop", "gold_per_match")),
    }
}

fn deeply_nested_expression(depth: usize) -> Expression {
    let mut expression = numeric(1.0);
    for _ in 0..depth {
        expression = Expression::Add {
            left: Box::new(expression),
            right: Box::new(numeric(1.0)),
        };
    }
    expression
}

fn lifecycle_document() -> tachiko_semantic_core::Document {
    let mut document = game_balance_document("game", "Game");
    let weapons = document.schemas.get_mut("weapons").unwrap();
    weapons.fields.insert(
        FieldId::from("all_ops"),
        FieldDefinition {
            id: FieldId::from("all_ops"),
            key: FieldKey::from("all_ops"),
            field_type: FieldType::Number,
            required: true,
        },
    );
    weapons.fields.insert(
        FieldId::from("peer"),
        FieldDefinition {
            id: FieldId::from("peer"),
            key: FieldKey::from("peer"),
            field_type: FieldType::Reference {
                schema: "weapons".into(),
            },
            required: true,
        },
    );
    let sword = document.entities.get_mut("iron_sword").unwrap();
    sword.fields.insert(
        FieldId::from("all_ops"),
        Value::Formula(every_expression_shape("iron_sword")),
    );
    sword.fields.insert(
        FieldId::from("peer"),
        Value::Reference(EntityId::from("iron_sword")),
    );
    document
}

#[test]
fn duplicate_rebases_only_copied_self_formulas_and_returns_a_valid_preview() {
    let document = lifecycle_document();
    let mut generator = OneIdGenerator::new("steel_sword");

    let preview = duplicate_entity(&document, "iron_sword", "steel_sword", &mut generator)
        .expect("valid entity duplication should succeed");

    assert!(validate_document(&preview.document).is_empty());
    let duplicate = &preview.document.entities["steel_sword"];
    assert_eq!(duplicate.id, EntityId::from("steel_sword"));
    assert_eq!(
        duplicate.fields["all_ops"],
        Value::Formula(every_expression_shape("steel_sword")),
        "every recursively nested self formula reference should be rebased"
    );
    assert_eq!(
        duplicate.fields["peer"],
        Value::Reference(EntityId::from("iron_sword")),
        "stored relationships are copied without rebasing"
    );
    assert_eq!(
        preview.document.entities["iron_sword"].fields["all_ops"],
        Value::Formula(every_expression_shape("iron_sword")),
        "the source document entity must remain unchanged"
    );
    assert_eq!(
        preview.document.entities["alric"].fields["weapon"],
        Value::Reference(EntityId::from("iron_sword")),
        "relationships owned by other entities must remain unchanged"
    );
    let calculation = calculate(&preview.document).expect("duplicate should calculate");
    assert_eq!(
        calculation.value(&FieldRef::new("steel_sword", "all_ops")),
        Some(expected(50.0))
    );
    assert!(preview.diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::EntityAdded { entity } if entity.as_str() == "steel_sword"
    )));
    assert!(!document.entities.contains_key("steel_sword"));
}

#[test]
fn duplicate_reports_missing_invalid_and_occupied_entities_explicitly() {
    let document = lifecycle_document();

    let mut missing_generator = OneIdGenerator::new("unused");
    assert!(matches!(
        duplicate_entity(&document, "missing", "steel_sword", &mut missing_generator),
        Err(WorkspaceError::MissingEntity { entity }) if entity.as_str() == "missing"
    ));
    let mut invalid_generator = OneIdGenerator::new("unused");
    assert!(matches!(
        duplicate_entity(&document, "iron_sword", "Bad.Id", &mut invalid_generator),
        Err(WorkspaceError::InvalidEntityKey { entity }) if entity.as_str() == "Bad.Id"
    ));
    let mut occupied_generator = OneIdGenerator::new("unused");
    assert!(matches!(
        duplicate_entity(&document, "iron_sword", "shop", &mut occupied_generator),
        Err(WorkspaceError::EntityKeyAlreadyExists { entity }) if entity.as_str() == "shop"
    ));
}

#[test]
fn rename_changes_only_the_human_key_and_preserves_all_stable_references() {
    let document = lifecycle_document();

    let preview = rename_entity(&document, "iron_sword", "moonblade")
        .expect("valid entity rename should succeed");

    assert!(validate_document(&preview.document).is_empty());
    assert!(preview.document.entities.contains_key("iron_sword"));
    assert_eq!(
        preview.document.entities["iron_sword"].key,
        EntityKey::from("moonblade")
    );
    assert_eq!(
        preview.document.entities["iron_sword"].fields["all_ops"],
        Value::Formula(every_expression_shape("iron_sword"))
    );
    assert_eq!(
        preview.document.entities["iron_sword"].fields["peer"],
        Value::Reference(EntityId::from("iron_sword"))
    );
    assert_eq!(
        preview.document.entities["alric"].fields["weapon"],
        Value::Reference(EntityId::from("iron_sword"))
    );
    assert_eq!(
        preview.document.entities["tempered_blade"].fields["grants_weapon"],
        Value::Reference(EntityId::from("iron_sword"))
    );
    assert_eq!(
        preview.document.entities["shop"].fields["matches_for_sword"],
        Value::Formula(Expression::Divide {
            left: Box::new(reference("iron_sword", "price")),
            right: Box::new(reference("shop", "gold_per_match")),
        })
    );
    let calculation = calculate(&preview.document).expect("renamed document should calculate");
    assert_eq!(
        calculation.value(&FieldRef::new("iron_sword", "dps")),
        Some(expected(40.0))
    );
    assert_eq!(
        calculation.value(&FieldRef::new("shop", "matches_for_sword")),
        Some(expected(2.4))
    );
    assert_eq!(
        document.entities["alric"].fields["weapon"],
        Value::Reference(EntityId::from("iron_sword")),
        "rename must not mutate its source"
    );
}

#[test]
fn rename_rejects_noop_before_occupancy_and_reports_other_preconditions() {
    let document = lifecycle_document();

    assert!(matches!(
        rename_entity(&document, "iron_sword", "iron_sword"),
        Err(WorkspaceError::NoOpEntityRename { entity }) if entity.as_str() == "iron_sword"
    ));
    assert!(matches!(
        rename_entity(&document, "missing", "target"),
        Err(WorkspaceError::MissingEntity { entity }) if entity.as_str() == "missing"
    ));
    assert!(matches!(
        rename_entity(&document, "iron_sword", "_target"),
        Err(WorkspaceError::InvalidEntityKey { entity }) if entity.as_str() == "_target"
    ));
    assert!(matches!(
        rename_entity(&document, "iron_sword", "shop"),
        Err(WorkspaceError::EntityKeyAlreadyExists { entity }) if entity.as_str() == "shop"
    ));
}

#[test]
fn remove_reports_one_sorted_path_per_dependent_field_across_all_expression_shapes() {
    let mut document = lifecycle_document();
    document.schemas.get_mut("economy").unwrap().fields.insert(
        FieldId::from("all_ops"),
        FieldDefinition {
            id: FieldId::from("all_ops"),
            key: FieldKey::from("all_ops"),
            field_type: FieldType::Number,
            required: true,
        },
    );
    document.entities.get_mut("shop").unwrap().fields.insert(
        FieldId::from("all_ops"),
        Value::Formula(every_expression_shape("iron_sword")),
    );

    let error = remove_entity(&document, "iron_sword")
        .expect_err("referenced entities must not be removed");
    let WorkspaceError::EntityReferenced {
        entity, dependents, ..
    } = &error
    else {
        panic!("expected a typed referenced-entity error, got {error:?}");
    };

    assert_eq!(entity.as_str(), "iron_sword");
    assert_eq!(
        dependents,
        &[
            FieldRef::new("alric", "weapon"),
            FieldRef::new("shop", "all_ops"),
            FieldRef::new("shop", "matches_for_sword"),
            FieldRef::new("tempered_blade", "grants_weapon"),
        ],
        "dependent fields must be unique and sorted"
    );
    let message = error.to_string();
    assert!(message.contains("iron_sword"));
    for dependent in dependents {
        assert!(
            message.contains(&dependent.to_string()),
            "error should render dependent path {dependent}"
        );
    }
}

#[test]
fn remove_ignores_owned_self_references_and_returns_a_valid_diff() {
    let mut document = lifecycle_document();
    document.entities.remove("alric");
    document.entities.remove("tempered_blade");
    let shop = document.entities.get_mut("shop").unwrap();
    shop.fields
        .insert(FieldId::from("matches_for_sword"), number(2.4));
    shop.fields
        .insert(FieldId::from("upgrade_cost"), number(200.0));

    let preview = remove_entity(&document, "iron_sword")
        .expect("self formula and stored references disappear with their owner");

    assert!(validate_document(&preview.document).is_empty());
    assert!(!preview.document.entities.contains_key("iron_sword"));
    assert!(document.entities.contains_key("iron_sword"));
    assert!(preview.diff.changes().iter().any(|change| matches!(
        change,
        SemanticChange::EntityRemoved { entity } if entity.as_str() == "iron_sword"
    )));
    calculate(&preview.document).expect("removed document should calculate");
}

#[test]
fn remove_requires_a_present_entity() {
    let document = lifecycle_document();

    assert!(matches!(
        remove_entity(&document, "missing"),
        Err(WorkspaceError::MissingEntity { entity }) if entity.as_str() == "missing"
    ));
}

#[test]
fn lifecycle_finalizer_surfaces_validation_calculation_and_diff_failures() {
    let mut invalid_document = game_balance_document("game", "Game");
    invalid_document.title = " ".to_owned();
    let mut invalid_generator = OneIdGenerator::new("steel_sword");
    assert!(matches!(
        duplicate_entity(
            &invalid_document,
            "iron_sword",
            "steel_sword",
            &mut invalid_generator
        ),
        Err(WorkspaceError::InvalidDocument { .. })
    ));

    let mut uncalculable = game_balance_document("game", "Game");
    uncalculable
        .entities
        .get_mut("iron_sword")
        .unwrap()
        .fields
        .insert(FieldId::from("attack_interval"), number(0.0));
    let mut uncalculable_generator = OneIdGenerator::new("steel_sword");
    assert!(matches!(
        duplicate_entity(
            &uncalculable,
            "iron_sword",
            "steel_sword",
            &mut uncalculable_generator
        ),
        Err(WorkspaceError::Calculation(_))
    ));

    uncalculable.entities.remove("alric");
    uncalculable.entities.remove("shop");
    uncalculable.entities.remove("tempered_blade");
    assert!(matches!(
        remove_entity(&uncalculable, "iron_sword"),
        Err(WorkspaceError::Diff(_))
    ));
}

#[test]
fn over_limit_typed_formulas_are_rejected_before_recursive_workspace_work() {
    const CHILD_ENV: &str = "TACHIKO_OVER_LIMIT_WORKSPACE_CHILD";

    if env::var_os(CHILD_ENV).is_some() {
        let proposal_document = lifecycle_document();
        assert!(matches!(
            validate_field_value_suggestion(
                &proposal_document,
                FieldRef::new("iron_sword", "all_ops"),
                Value::Formula(deeply_nested_expression(50_000)),
            ),
            Err(WorkspaceError::ExpressionComplexity { field, .. })
                if field == FieldRef::new("iron_sword", "all_ops")
        ));
        let missing = FieldRef::new("iron_sword", "missing");
        assert!(matches!(
            validate_field_value_suggestion(
                &proposal_document,
                missing.clone(),
                Value::Formula(deeply_nested_expression(50_000)),
            ),
            Err(WorkspaceError::MissingField { field }) if field == missing
        ));

        let mut document = lifecycle_document();
        document
            .entities
            .get_mut("iron_sword")
            .unwrap()
            .fields
            .insert(
                FieldId::from("all_ops"),
                Value::Formula(deeply_nested_expression(50_000)),
            );

        let mut generator = OneIdGenerator::new("steel_sword");
        for result in [
            duplicate_entity(&document, "iron_sword", "steel_sword", &mut generator),
            rename_entity(&document, "iron_sword", "steel_sword"),
            rename_schema(&document, "weapons", "equipment"),
            rename_field(&document, "weapons", "damage", "power"),
            remove_entity(&document, "shop"),
            set_scalar(&document, &FieldAddress::new("iron_sword", "damage"), "41"),
        ] {
            assert!(matches!(
                result,
                Err(WorkspaceError::ExpressionComplexity { field, .. })
                    if field == FieldRef::new("iron_sword", "all_ops")
            ));
        }

        // Recursive drop is deliberately outside the contract being tested.
        mem::forget(document);
        return;
    }

    let status = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            "over_limit_typed_formulas_are_rejected_before_recursive_workspace_work",
        ])
        .env(CHILD_ENV, "1")
        .status()
        .unwrap();

    assert!(
        status.success(),
        "workspace mutations must reject hostile typed formulas without aborting the process: {status}"
    );
}
