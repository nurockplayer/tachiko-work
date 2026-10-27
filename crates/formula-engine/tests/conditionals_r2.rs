use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{
    CalculationFailure, CalculationOutcome, FormulaBindError, UnboundExpression, bind_expression,
    calculate_complete, extract_dependencies, parse_expression, project_expression,
};
use tachiko_semantic_core::{
    ComparisonOperator, Document, DocumentId, Entity, EntityId, EntityKey, Expression,
    FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema,
    SchemaId, SchemaKey, Value,
};

fn definition(id: &str, field_type: FieldType, required: bool) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        field_type,
        required,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("test numbers are finite"))
}

fn document() -> Document {
    let mut fields = BTreeMap::new();
    let mut values = BTreeMap::new();
    for field in [
        "input",
        "then_value",
        "zero",
        "output",
        "logical",
        "equal",
        "not_equal",
        "less",
        "less_equal",
        "greater",
        "greater_equal",
        "round_2675",
        "round_half",
        "round_negative_half",
        "round_large",
        "round_ten",
        "percent",
        "blank_arithmetic",
        "blank_probe",
    ] {
        fields.insert(
            FieldId::from(field),
            definition(field, FieldType::Number, true),
        );
        values.insert(FieldId::from(field), number(0.0));
    }
    fields.insert(
        FieldId::from("blank_number"),
        definition("blank_number", FieldType::Number, false),
    );
    fields.insert(
        FieldId::from("note"),
        definition("note", FieldType::Text, false),
    );
    values.insert(FieldId::from("input"), number(2.675));
    values.insert(FieldId::from("then_value"), number(42.0));

    let schema = Schema {
        id: SchemaId::from("schema"),
        key: SchemaKey::from("row"),
        display_name: None,
        fields,
    };
    let entity = Entity {
        id: EntityId::from("row"),
        key: EntityKey::from("row"),
        schema: SchemaId::from("schema"),
        fields: values,
    };
    Document {
        id: DocumentId::from("formula-r2"),
        title: "Formula R2".to_owned(),
        schemas: BTreeMap::from([(SchemaId::from("schema"), schema)]),
        entities: BTreeMap::from([(EntityId::from("row"), entity)]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn owner(field: &str) -> FieldRef {
    FieldRef::new("row", field)
}

fn set_formula(document: &mut Document, target: &str, source: &str) -> Expression {
    let unbound = parse_expression(source).expect("formula source parses");
    let bound = bind_expression(document, &owner(target), &unbound).expect("formula binds");
    document
        .entities
        .get_mut("row")
        .expect("fixture entity exists")
        .fields
        .insert(FieldId::from(target), Value::Formula(bound.clone()));
    bound
}

fn number_at(outcome: &CalculationOutcome, target: &str) -> f64 {
    match outcome {
        CalculationOutcome::Complete(calculation) => calculation
            .value(&owner(target))
            .expect("target has a calculated Number")
            .get(),
        CalculationOutcome::Failed(failures) => {
            panic!("calculation failed: {:?}", failures.failures())
        }
    }
}

#[test]
fn comparison_operators_bind_and_evaluate_as_conditions() {
    let mut document = document();
    let formulas = [
        ("equal", "="),
        ("not_equal", "<>"),
        ("less", "<"),
        ("less_equal", "<="),
        ("greater", ">"),
        ("greater_equal", ">="),
    ];
    for (target, operator) in formulas {
        set_formula(
            &mut document,
            target,
            &format!("IF([row.input] {operator} 2.675, 1, 0)"),
        );
    }

    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("valid comparison formulas calculate");
    };
    for (target, expected) in [
        ("equal", 1.0),
        ("not_equal", 0.0),
        ("less", 0.0),
        ("less_equal", 1.0),
        ("greater", 0.0),
        ("greater_equal", 1.0),
    ] {
        assert_eq!(calculation.value(&owner(target)).unwrap().get(), expected);
    }
    for (operator, expected) in [
        ("=", ComparisonOperator::Equal),
        ("<>", ComparisonOperator::NotEqual),
        ("<", ComparisonOperator::Less),
        ("<=", ComparisonOperator::LessOrEqual),
        (">", ComparisonOperator::Greater),
        (">=", ComparisonOperator::GreaterOrEqual),
    ] {
        assert!(matches!(
            parse_expression(&format!("[row.input] {operator} 1")).unwrap(),
            UnboundExpression::Compare { operator, .. } if operator == expected
        ));
    }
}

#[test]
fn conditions_are_not_values_outside_condition_positions() {
    let document = document();
    for source in [
        "[row.input] > 0",
        "1 + ([row.input] > 0)",
        "IF(1, 2, 3)",
        "IF([row.input] > 0, [row.input] > 1, 0)",
        "AND([row.input] > 0, 1)",
        "OR([row.input] > 0, [row.input])",
        "NOT([row.input])",
        "ROUND([row.input] > 0, 0)",
        "ISBLANK(1)",
        "min(1 < 2, 3)",
        "(1 < 2) = 1",
        "1 < 2 < 3",
    ] {
        let parsed =
            parse_expression(source).expect("the invalid placement is syntactically valid");
        assert!(
            matches!(
                bind_expression(&document, &owner("output"), &parsed),
                Err(FormulaBindError::TypeMismatch { .. })
            ),
            "{source} must be a typed formula mismatch"
        );
    }
}

#[test]
fn round_requires_an_integer_literal_from_zero_through_ten() {
    let document = document();
    for source in [
        "ROUND([row.input], -1)",
        "ROUND([row.input], 1.5)",
        "ROUND([row.input], 11)",
        "ROUND([row.input], [row.zero])",
        "ROUND([row.input], 1 + 1)",
    ] {
        let parsed =
            parse_expression(source).expect("invalid precision is still syntactically valid");
        assert!(matches!(
            bind_expression(&document, &owner("output"), &parsed),
            Err(FormulaBindError::TypeMismatch { .. })
        ));
    }
}

#[test]
fn round_uses_half_away_from_zero_and_is_stable_at_supported_precisions() {
    let mut document = document();
    for (target, source) in [
        ("round_2675", "ROUND(2.675, 2)"),
        ("round_half", "ROUND(0.5, 0)"),
        ("round_negative_half", "ROUND(-0.5, 0)"),
        ("round_large", "ROUND(1e15, 0)"),
        ("round_ten", "ROUND(2.675, 10)"),
        ("percent", "50%"),
    ] {
        set_formula(&mut document, target, source);
    }
    let outcome = calculate_complete(&document);
    assert_eq!(number_at(&outcome, "round_2675"), 2.68);
    assert_eq!(number_at(&outcome, "round_half"), 1.0);
    assert_eq!(number_at(&outcome, "round_negative_half"), -1.0);
    assert_eq!(number_at(&outcome, "round_large"), 1e15);
    assert_eq!(number_at(&outcome, "round_ten"), 2.675);
    assert_eq!(number_at(&outcome, "percent"), 0.5);
}

#[test]
fn chosen_if_branch_controls_failure_while_dependencies_include_both_branches() {
    let mut document = document();
    set_formula(
        &mut document,
        "output",
        "IF([row.input] > 0, [row.then_value], 1 / [row.zero])",
    );
    let dependencies = extract_dependencies(
        &document,
        &owner("output"),
        match &document.entities["row"].fields["output"] {
            Value::Formula(expression) => expression,
            _ => unreachable!(),
        },
    );
    assert_eq!(
        dependencies,
        [owner("input"), owner("then_value"), owner("zero")]
            .into_iter()
            .collect()
    );

    let outcome = calculate_complete(&document);
    assert_eq!(number_at(&outcome, "output"), 42.0);
    assert_eq!(
        match &outcome {
            CalculationOutcome::Complete(calculation) => {
                calculation.dependencies_of(&owner("output")).unwrap()
            }
            CalculationOutcome::Failed(_) => panic!("the chosen branch succeeds"),
        },
        &dependencies
    );

    document
        .entities
        .get_mut("row")
        .unwrap()
        .fields
        .insert(FieldId::from("input"), number(-1.0));
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("the now-chosen branch divides by zero");
    };
    assert_eq!(
        failures.failures().get(&owner("output")),
        Some(&CalculationFailure::DivisionByZero)
    );
}

#[test]
fn untaken_if_branch_still_participates_in_static_cycle_detection() {
    let mut document = document();
    set_formula(
        &mut document,
        "output",
        "IF([row.input] > 0, [row.then_value], [row.output])",
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("both branches remain in the static dependency graph");
    };
    assert_eq!(
        failures.failures().get(&owner("output")),
        Some(&CalculationFailure::Cycle {
            members: BTreeSet::from([owner("output")]),
        })
    );
}

#[test]
fn and_or_not_and_isblank_work_with_optional_blank_number_and_text_references() {
    let mut document = document();
    set_formula(
        &mut document,
        "logical",
        "IF(AND([row.input] >= 0, OR([row.input] < 0, NOT([row.input] = 0))), 1, 0)",
    );
    set_formula(&mut document, "blank_arithmetic", "[row.blank_number] + 2");
    set_formula(
        &mut document,
        "blank_probe",
        "IF(ISBLANK([row.note]), 1, 0)",
    );

    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("blank optional Number and ISBLANK formulas calculate");
    };
    assert_eq!(calculation.value(&owner("logical")).unwrap().get(), 1.0);
    assert_eq!(
        calculation.value(&owner("blank_arithmetic")).unwrap().get(),
        2.0
    );
    assert_eq!(calculation.value(&owner("blank_probe")).unwrap().get(), 1.0);
    let blank_number_dependency = BTreeSet::from([owner("blank_number")]);
    assert_eq!(
        calculation.dependencies_of(&owner("blank_arithmetic")),
        Some(&blank_number_dependency)
    );
    let note_dependency = BTreeSet::from([owner("note")]);
    assert_eq!(
        calculation.dependencies_of(&owner("blank_probe")),
        Some(&note_dependency)
    );

    document
        .entities
        .get_mut("row")
        .unwrap()
        .fields
        .insert(FieldId::from("note"), Value::Text(String::new()));
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("ISBLANK treats empty Text as present");
    };
    assert_eq!(calculation.value(&owner("blank_probe")).unwrap().get(), 0.0);
}

#[test]
fn new_formula_syntax_round_trips_through_canonical_projection() {
    let document = document();
    let source = "IF(AND([row.input] >= 2.675, OR([row.input] <> 0, NOT([row.input] < 0))), ROUND([row.input] * 50%, 2), 0)";
    let bound = bind_expression(
        &document,
        &owner("output"),
        &parse_expression(source).unwrap(),
    )
    .unwrap();
    let canonical = project_expression(&document, &owner("output"), &bound).unwrap();
    assert_eq!(
        canonical,
        "IF(AND(([row.input] >= 2.675), OR(([row.input] <> 0), NOT(([row.input] < 0)))), ROUND(([row.input] * 0.5), 2), 0)"
    );
    let reopened = bind_expression(
        &document,
        &owner("output"),
        &parse_expression(&canonical).unwrap(),
    )
    .unwrap();
    assert_eq!(reopened, bound);
}

#[test]
fn arity_errors_are_located_and_comparison_precedence_is_below_arithmetic() {
    for source in [
        "IF(1=1,2)",
        "AND()",
        "OR()",
        "NOT(1=1,2=2)",
        "ROUND(2)",
        "ISBLANK()",
    ] {
        let error = parse_expression(source).unwrap_err();
        assert_eq!(error.position, 0);
        assert!(error.message.contains("arity"), "{source}: {error}");
    }
    let mut document = document();
    set_formula(
        &mut document,
        "output",
        "IF(1 + 2 * 3 >= 7, min(5, max(2, 3)), 0)",
    );
    assert_eq!(number_at(&calculate_complete(&document), "output"), 3.0);
}

#[test]
fn percent_literals_scale_the_decimal_before_binary64_conversion() {
    for (source, expected) in [
        ("50%", 0.5),
        ("1e2%", 1.0),
        ("1e310%", 1e308),
        ("1e-324%", 0.0),
    ] {
        let UnboundExpression::Number(number) = parse_expression(source).unwrap() else {
            panic!("percent must lower to Number");
        };
        assert_eq!(number.get(), expected);
    }
    assert!(parse_expression("1e999%").is_err());
    assert!(parse_expression("(50)%").is_err());
    let mut document = document();
    set_formula(&mut document, "output", "-50% + 200% * 50%");
    assert_eq!(number_at(&calculate_complete(&document), "output"), 0.5);
}

#[test]
fn round_preserves_large_inputs_at_finer_than_binary64_precision() {
    for source in [
        "ROUND(1e15, 10)",
        "ROUND(1000000000000000.5, 10)",
        "ROUND(-1000000000000000.5, 10)",
        "ROUND(450359962737049.56, 2)",
    ] {
        let mut document = document();
        let bound = set_formula(&mut document, "output", source);
        let Expression::Round { value, .. } = bound else {
            panic!("expected ROUND");
        };
        let Expression::Number(expected) = *value else {
            panic!("expected literal");
        };
        assert_eq!(
            number_at(&calculate_complete(&document), "output"),
            expected.get()
        );
    }
}

#[test]
fn untaken_failed_formula_reference_does_not_fail_the_conditional_node() {
    let mut document = document();
    set_formula(&mut document, "then_value", "1 / 0");
    let bound = set_formula(&mut document, "output", "IF(1 = 1, 7, [row.then_value])");
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("the independently failing formula still fails the atomic calculation");
    };
    assert_eq!(
        failures.failures().get(&owner("then_value")),
        Some(&CalculationFailure::DivisionByZero)
    );
    assert!(!failures.failures().contains_key(&owner("output")));
    assert_eq!(
        extract_dependencies(&document, &owner("output"), &bound),
        BTreeSet::from([owner("then_value")])
    );
}

#[test]
fn directly_supplied_condition_results_and_invalid_argument_types_fail_typed() {
    let mut document = document();
    document
        .entities
        .get_mut("row")
        .unwrap()
        .fields
        .insert("output".into(), Value::Formula(Expression::And(Vec::new())));
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a condition is not a Number value");
    };
    assert_eq!(
        failures.failures().get(&owner("output")),
        Some(&CalculationFailure::TypeMismatch)
    );
}

#[test]
fn incremental_conditionals_match_full_recomputation_across_failures_and_blanks() {
    let mut document = document();
    document
        .schemas
        .get_mut("schema")
        .unwrap()
        .fields
        .get_mut("input")
        .unwrap()
        .required = false;
    set_formula(
        &mut document,
        "output",
        "IF(ISBLANK([.input]), 1, IF([.input] > 0, ROUND([.input], 2), 1 / [.zero]))",
    );
    set_formula(&mut document, "logical", "[.output] * 2");
    let (mut retained, _) = tachiko_formula_engine::RetainedCalculationState::rebuild(&document);
    for input in [Some(0.0), Some(2.675), None, Some(-1.0), Some(4.5)] {
        let fields = &mut document.entities.get_mut("row").unwrap().fields;
        if let Some(input) = input {
            fields.insert("input".into(), number(input));
        } else {
            fields.remove("input");
        }
        let _transition = retained.update(&document, &BTreeSet::from([owner("input")]));
        assert_eq!(
            retained.outcome(),
            calculate_complete(&document),
            "input {input:?}"
        );
    }
}

#[test]
fn absent_non_number_fields_are_only_valid_as_presence_probes() {
    for field_type in [FieldType::Text, FieldType::Date, FieldType::Boolean] {
        let mut document = document();
        document
            .schemas
            .get_mut("schema")
            .unwrap()
            .fields
            .get_mut("note")
            .unwrap()
            .field_type = field_type;
        let source = parse_expression("[.note] + 1").unwrap();
        assert!(matches!(
            bind_expression(&document, &owner("output"), &source),
            Err(FormulaBindError::NonNumericTarget { .. })
        ));
        set_formula(&mut document, "output", "IF(ISBLANK([.note]), 3, 4)");
        assert_eq!(number_at(&calculate_complete(&document), "output"), 3.0);
    }
}

#[test]
fn round_decides_on_the_decimal_not_the_scaled_binary_product() {
    for (source, expected) in [
        ("ROUND(1.005, 2)", 1.01),
        ("ROUND(1.015, 2)", 1.02),
        ("ROUND(0.285, 2)", 0.29),
        ("ROUND(-1.005, 2)", -1.01),
        ("ROUND(2.675, 2)", 2.68),
        ("ROUND(9.995, 2)", 10.0),
        ("ROUND(-0.004, 2)", 0.0),
        ("ROUND(0.125, 2)", 0.13),
        ("ROUND(1.4999, 0)", 1.0),
    ] {
        let mut document = document();
        set_formula(&mut document, "output", source);
        let actual = number_at(&calculate_complete(&document), "output");
        assert_eq!(actual, expected, "{source}");
        assert!(
            !actual.is_sign_negative() || actual != 0.0,
            "{source} is not -0"
        );
    }
}

#[test]
fn a_missing_required_value_fails_only_the_selected_if_branch() {
    let mut document = document();
    // `then_value` is a required Number; remove its value.
    document
        .entities
        .get_mut("row")
        .unwrap()
        .fields
        .remove(&FieldId::from("then_value"));
    set_formula(
        &mut document,
        "output",
        "IF([row.input] < 0, [row.then_value], 42)",
    );
    assert_eq!(number_at(&calculate_complete(&document), "output"), 42.0);

    set_formula(
        &mut document,
        "output",
        "IF([row.input] > 0, [row.then_value], 42)",
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("the selected branch reads a missing required value");
    };
    assert!(failures.failures().contains_key(&owner("output")));

    // Outside a branch the missing value still fails before evaluation.
    set_formula(&mut document, "output", "[row.then_value] + 1");
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("an unconditional missing value fails");
    };
    assert!(matches!(
        failures.failures().get(&owner("output")),
        Some(CalculationFailure::InvalidReferences { .. })
    ));
}

#[test]
fn isblank_probe_does_not_make_a_branch_read_unconditional() {
    let mut document = document();
    document
        .entities
        .get_mut("row")
        .expect("fixture entity exists")
        .fields
        .remove("blank_probe");
    set_formula(
        &mut document,
        "output",
        "IF(ISBLANK([row.blank_probe]), 5, [row.blank_probe])",
    );
    match calculate_complete(&document) {
        CalculationOutcome::Complete(calculation) => assert_eq!(
            calculation.value(&owner("output")).map(Number::get),
            Some(5.0)
        ),
        CalculationOutcome::Failed(failures) => {
            assert!(!failures.failures().contains_key(&owner("output")));
        }
    }
}
