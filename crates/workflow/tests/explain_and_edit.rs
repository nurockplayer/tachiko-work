use tachiko_formula_engine::calculate;
use tachiko_semantic_core::{
    EntityId, Expression, FieldDefinition, FieldId, FieldRef, FieldType, Value,
};
use tachiko_workflow::{
    StarterTemplate, WorkflowError, create_document, explain_field, set_formula, set_scalar,
};

#[test]
fn formula_explanation_connects_expression_dependencies_and_result() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");
    let field = FieldRef::new("iron_sword", "dps");

    let explanation = explain_field(&document, &field).expect("DPS formula should explain");

    assert_eq!(explanation.display_value, "40");
    assert_eq!(
        explanation.expression.as_deref(),
        Some("([iron_sword.damage] / [iron_sword.attack_interval])")
    );
    assert_eq!(
        explanation.dependencies,
        [
            FieldRef::new("iron_sword", "attack_interval"),
            FieldRef::new("iron_sword", "damage"),
        ]
    );
    assert!(explanation.affected_formulas.is_empty());
}

#[test]
fn formula_edit_maps_authoring_syntax_to_a_valid_immutable_preview() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");
    let field = FieldRef::new("iron_sword", "dps");
    let input = "min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)";

    let preview = set_formula(&document, &field, input).expect("formula edit should be valid");

    assert_eq!(
        preview.document.entities["iron_sword"].fields["dps"],
        Value::Formula(Expression::Minimum {
            left: Box::new(Expression::Number(60.0)),
            right: Box::new(Expression::Add {
                left: Box::new(Expression::Divide {
                    left: Box::new(Expression::Reference(
                        FieldRef::new("iron_sword", "damage",)
                    )),
                    right: Box::new(Expression::Reference(FieldRef::new(
                        "iron_sword",
                        "attack_interval",
                    ))),
                }),
                right: Box::new(Expression::Number(5.0)),
            }),
        })
    );
    assert_eq!(
        calculate(&preview.document).unwrap().value(&field),
        Some(45.0)
    );
    assert_eq!(
        calculate(&document).unwrap().value(&field),
        Some(40.0),
        "formula editing must not mutate the source"
    );
    let rendered = preview.diff.render_text();
    assert!(rendered.contains(
        "dps: ([iron_sword.damage] / [iron_sword.attack_interval]) -> min(60, (([iron_sword.damage] / [iron_sword.attack_interval]) + 5))"
    ));
    assert!(rendered.contains("affected dps: 40 -> 45"));
}

#[test]
fn formula_edit_can_turn_a_stored_number_into_a_computed_field() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");
    let field = FieldRef::new("iron_sword", "damage");

    let preview = set_formula(&document, &field, "[iron_sword.price] / 3")
        .expect("a numeric input may become a formula");

    assert!(matches!(
        preview.document.entities["iron_sword"].fields["damage"],
        Value::Formula(_)
    ));
    let calculation = calculate(&preview.document).unwrap();
    assert_eq!(calculation.value(&field), Some(40.0));
    assert_eq!(
        calculation.value(&FieldRef::new("iron_sword", "dps")),
        Some(40.0 / 0.9)
    );
}

#[test]
fn formula_edit_refuses_invalid_targets_syntax_semantics_and_calculation() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");

    let wrong_type = set_formula(&document, &FieldRef::new("iron_sword", "name"), "1 + 2")
        .expect_err("text fields cannot become formulas");
    assert!(matches!(
        wrong_type,
        WorkflowError::NonNumericFormulaField { .. }
    ));

    let invalid = set_formula(&document, &FieldRef::new("iron_sword", "dps"), "min(1,")
        .expect_err("invalid syntax must fail");
    assert!(matches!(invalid, WorkflowError::InvalidFormula { .. }));

    let no_change = set_formula(
        &document,
        &FieldRef::new("iron_sword", "dps"),
        "[iron_sword.damage] / [iron_sword.attack_interval]",
    )
    .expect_err("the same formula must be refused");
    assert!(matches!(no_change, WorkflowError::NoChange { .. }));

    let missing_field = set_formula(&document, &FieldRef::new("iron_sword", "missing"), "1")
        .expect_err("missing fields must fail");
    assert!(matches!(missing_field, WorkflowError::MissingField { .. }));

    let missing_reference = set_formula(
        &document,
        &FieldRef::new("iron_sword", "dps"),
        "[missing.damage]",
    )
    .expect_err("missing references must fail validation");
    assert!(matches!(
        missing_reference,
        WorkflowError::InvalidDocument { .. }
    ));

    let cycle = set_formula(
        &document,
        &FieldRef::new("iron_sword", "dps"),
        "[iron_sword.dps] + 1",
    )
    .expect_err("cycles must fail calculation");
    assert!(matches!(cycle, WorkflowError::Calculation(_)));

    let division_by_zero = set_formula(&document, &FieldRef::new("iron_sword", "dps"), "1 / 0")
        .expect_err("division by zero must fail calculation");
    assert!(matches!(division_by_zero, WorkflowError::Calculation(_)));
}

#[test]
fn input_explanation_shows_downstream_formula_impact() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");
    let field = FieldRef::new("iron_sword", "damage");

    let explanation = explain_field(&document, &field).expect("damage should explain");

    assert_eq!(explanation.display_value, "36");
    assert!(explanation.expression.is_none());
    assert!(explanation.dependencies.is_empty());
    assert_eq!(explanation.affected_formulas.len(), 1);
    assert_eq!(
        explanation.affected_formulas[0].field,
        FieldRef::new("iron_sword", "dps")
    );
    assert_eq!(explanation.affected_formulas[0].display_value, "40");
}

#[test]
fn typed_edit_returns_valid_document_and_semantic_impact() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");
    let field = FieldRef::new("iron_sword", "damage");

    let preview = set_scalar(&document, &field, "45").expect("damage edit should be valid");

    assert_eq!(
        preview.document.entities["iron_sword"].fields["damage"],
        Value::Number(45.0)
    );
    assert_eq!(
        document.entities["iron_sword"].fields["damage"],
        Value::Number(36.0),
        "editing must not mutate the source document"
    );
    assert_eq!(
        preview.diff.render_text(),
        "Weapons Iron Sword\ndamage: 36 -> 45\naffected dps: 40 -> 50\n"
    );
}

#[test]
fn edit_parses_each_scalar_type_from_the_schema() {
    let mut document = create_document(StarterTemplate::GameBalance, "game", "Game");
    document
        .schemas
        .get_mut("characters")
        .unwrap()
        .fields
        .insert(
            FieldId::from("enabled"),
            FieldDefinition {
                field_type: FieldType::Boolean,
                required: true,
            },
        );
    document
        .entities
        .get_mut("alric")
        .unwrap()
        .fields
        .insert(FieldId::from("enabled"), Value::Boolean(true));
    let mut spare_weapon = document.entities["iron_sword"].clone();
    spare_weapon.id = EntityId::from("spare_sword");
    document
        .entities
        .insert(EntityId::from("spare_sword"), spare_weapon);

    let text = set_scalar(&document, &FieldRef::new("iron_sword", "name"), "Longsword")
        .expect("text should parse");
    assert_eq!(
        text.document.entities["iron_sword"].fields["name"],
        Value::Text("Longsword".to_owned())
    );

    let boolean = set_scalar(&document, &FieldRef::new("alric", "enabled"), "false")
        .expect("boolean should parse");
    assert_eq!(
        boolean.document.entities["alric"].fields["enabled"],
        Value::Boolean(false)
    );

    let reference = set_scalar(&document, &FieldRef::new("alric", "weapon"), "spare_sword")
        .expect("typed reference should parse");
    assert_eq!(
        reference.document.entities["alric"].fields["weapon"],
        Value::Reference(EntityId::from("spare_sword"))
    );

    let no_change = set_scalar(&document, &FieldRef::new("alric", "weapon"), "iron_sword")
        .expect_err("unchanged references should be refused");
    assert!(matches!(no_change, WorkflowError::NoChange { .. }));
}

#[test]
fn edit_refuses_formula_invalid_values_and_broken_references() {
    let mut document = create_document(StarterTemplate::GameBalance, "game", "Game");
    document
        .schemas
        .get_mut("characters")
        .unwrap()
        .fields
        .insert(
            FieldId::from("enabled"),
            FieldDefinition {
                field_type: FieldType::Boolean,
                required: true,
            },
        );
    document
        .entities
        .get_mut("alric")
        .unwrap()
        .fields
        .insert(FieldId::from("enabled"), Value::Boolean(true));

    let formula = set_scalar(&document, &FieldRef::new("iron_sword", "dps"), "50")
        .expect_err("formulas should not be overwritten by scalar editing");
    assert!(matches!(formula, WorkflowError::FormulaEdit { .. }));

    let invalid_number = set_scalar(&document, &FieldRef::new("iron_sword", "damage"), "many")
        .expect_err("invalid number should be refused");
    assert!(matches!(invalid_number, WorkflowError::InvalidValue { .. }));

    let invalid_boolean = set_scalar(&document, &FieldRef::new("alric", "enabled"), "yes")
        .expect_err("invalid boolean should be refused");
    assert!(matches!(
        invalid_boolean,
        WorkflowError::InvalidValue { .. }
    ));

    let missing = set_scalar(&document, &FieldRef::new("iron_sword", "missing"), "1")
        .expect_err("missing fields should be refused");
    assert!(matches!(missing, WorkflowError::MissingField { .. }));

    let broken_reference = set_scalar(
        &document,
        &FieldRef::new("alric", "weapon"),
        "missing_weapon",
    )
    .expect_err("broken reference should be refused");
    assert!(matches!(
        broken_reference,
        WorkflowError::InvalidDocument { .. }
    ));

    let calculation_failure = set_scalar(
        &document,
        &FieldRef::new("iron_sword", "attack_interval"),
        "0",
    )
    .expect_err("edits that break formulas should be refused");
    assert!(matches!(calculation_failure, WorkflowError::Calculation(_)));
}

#[test]
fn explanation_reports_missing_fields_without_panicking() {
    let document = create_document(StarterTemplate::GameBalance, "game", "Game");
    let error = explain_field(&document, &FieldRef::new("iron_sword", "missing"))
        .expect_err("missing field should fail");
    assert!(matches!(error, WorkflowError::MissingField { .. }));
}
