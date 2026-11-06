//! SUMIF(S), AVERAGEIF(S) and COUNTIF(S): one aggregate over the rows whose
//! columns satisfy every criterion. Criterion literals are limited to numbers
//! and blanks until stored text is allowed; text, dates and booleans are read
//! from a cell.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tachiko_formula_engine::{
    CalculationFailure, CalculationOutcome, ColumnFailure, FormulaBindError,
    RetainedCalculationState, bind_expression, calculate_complete, format_unbound_expression,
    parse_expression, project_expression,
};
use tachiko_semantic_core::{
    ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate, CriterionCell,
    CriterionTest, Date, Document, DocumentId, Entity, EntityId, EntityKey, Expression,
    FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema,
    SchemaId, SchemaKey, Value, validate_document,
};

fn field(name: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(name),
        key: FieldKey::from(name),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn text_field(name: &str) -> FieldDefinition {
    field(name, FieldType::Text)
}

fn number_field(name: &str) -> FieldDefinition {
    field(name, FieldType::Number)
}

fn boolean_field(name: &str) -> FieldDefinition {
    field(name, FieldType::Boolean)
}

fn date_field(name: &str) -> FieldDefinition {
    field(name, FieldType::Date)
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn date(text: &str) -> Value {
    Value::Date(Date::parse(text).unwrap())
}

fn column(schema: &str, field: &str) -> ColumnRef {
    ColumnRef::new(SchemaId::from(schema), FieldId::from(field))
}

fn owner(entity: &str, field: &str) -> FieldRef {
    FieldRef::new(entity, field)
}

fn schema(id: &str, fields: BTreeMap<FieldId, FieldDefinition>) -> Schema {
    Schema {
        field_order: None,
        id: SchemaId::from(id),
        key: SchemaKey::from(id),
        display_name: None,
        fields,
    }
}

fn plan_row(id: &str, fields: &[(&str, Value)]) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from("plan"),
        fields: fields
            .iter()
            .map(|(field, value)| (FieldId::from(*field), value.clone()))
            .collect(),
    }
}

/// Four plan rows and one summary row. `summary.result` holds the formula
/// under test, and the `criterion*` cells feed criteria references.
#[allow(clippy::too_many_lines)]
fn document() -> Document {
    let plan = schema(
        "plan",
        BTreeMap::from([
            (FieldId::from("item"), text_field("item")),
            (FieldId::from("category"), text_field("category")),
            (FieldId::from("reserve"), number_field("reserve")),
            (FieldId::from("active"), boolean_field("active")),
            (FieldId::from("due"), date_field("due")),
            (FieldId::from("quantity"), number_field("quantity")),
            (FieldId::from("quota"), number_field("quota")),
            (FieldId::from("computed"), number_field("computed")),
        ]),
    );
    let summary = schema(
        "summary",
        BTreeMap::from([
            (FieldId::from("result"), number_field("result")),
            (FieldId::from("criterion"), number_field("criterion")),
            (
                FieldId::from("criterion_text"),
                text_field("criterion_text"),
            ),
            (
                FieldId::from("criterion_bool"),
                boolean_field("criterion_bool"),
            ),
            (
                FieldId::from("criterion_date"),
                date_field("criterion_date"),
            ),
        ]),
    );
    let default = conditional(
        ConditionalAggregate::Count,
        vec![cell_criterion("category", "criterion_text")],
    );
    Document {
        id: DocumentId::from("conditional-aggregates"),
        title: "Conditional aggregates".to_owned(),
        schemas: BTreeMap::from([
            (SchemaId::from("plan"), plan),
            (SchemaId::from("summary"), summary),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("rent"),
                plan_row(
                    "rent",
                    &[
                        ("item", Value::Text("Rent".to_owned())),
                        ("category", Value::Text("housing".to_owned())),
                        ("reserve", number(12_000.0)),
                        ("active", Value::Boolean(true)),
                        ("due", date("2026-01-01")),
                        ("quantity", number(2.0)),
                        ("quota", number(2.0)),
                    ],
                ),
            ),
            (
                EntityId::from("ads"),
                plan_row(
                    "ads",
                    &[
                        ("item", Value::Text("Ads".to_owned())),
                        ("category", Value::Text("marketing".to_owned())),
                        ("reserve", number(500.0)),
                        ("active", Value::Boolean(false)),
                        ("due", date("2026-02-01")),
                        ("quantity", number(3.0)),
                        ("quota", number(2.0)),
                    ],
                ),
            ),
            (
                EntityId::from("tools"),
                plan_row(
                    "tools",
                    &[
                        ("item", Value::Text("Tools".to_owned())),
                        ("category", Value::Text("Housing".to_owned())),
                        ("active", Value::Boolean(true)),
                        ("due", date("2026-01-15")),
                        ("quantity", number(4.0)),
                        ("quota", number(4.0)),
                    ],
                ),
            ),
            (
                EntityId::from("blank"),
                plan_row(
                    "blank",
                    &[
                        ("item", Value::Text("Blank".to_owned())),
                        ("reserve", number(700.0)),
                        ("active", Value::Boolean(true)),
                        ("quantity", number(1.0)),
                        ("quota", number(1.0)),
                    ],
                ),
            ),
            (
                EntityId::from("summary"),
                Entity {
                    id: EntityId::from("summary"),
                    key: EntityKey::from("summary"),
                    schema: SchemaId::from("summary"),
                    fields: BTreeMap::from([
                        (FieldId::from("result"), Value::Formula(default)),
                        (FieldId::from("criterion"), number(2.0)),
                        (
                            FieldId::from("criterion_text"),
                            Value::Text("housing".to_owned()),
                        ),
                        (FieldId::from("criterion_bool"), Value::Boolean(true)),
                        (FieldId::from("criterion_date"), date("2026-01-01")),
                    ]),
                },
            ),
        ]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn conditional(aggregate: ConditionalAggregate, criteria: Vec<ColumnCriterion>) -> Expression {
    Expression::ColumnConditional {
        aggregate: Box::new(aggregate),
        criteria,
    }
}

fn criterion(value: Expression, column: ColumnRef) -> ColumnCriterion {
    ColumnCriterion {
        column,
        operator: ComparisonOperator::Equal,
        value: Some(Box::new(value)),
    }
}

/// A criterion read from one summary cell: the main usage until stored text is
/// allowed.
fn cell_criterion(plan_field: &str, summary_field: &str) -> ColumnCriterion {
    criterion(
        Expression::Reference(owner("summary", summary_field)),
        column("plan", plan_field),
    )
}

fn set_result(document: &mut Document, expression: Expression) {
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .insert(FieldId::from("result"), Value::Formula(expression));
}

fn with_result(mut document: Document, expression: Expression) -> Document {
    set_result(&mut document, expression);
    document
}

fn result(document: &Document) -> Option<Number> {
    let CalculationOutcome::Complete(calculation) = calculate_complete(document) else {
        panic!(
            "the document must calculate: {:?}",
            calculate_complete(document)
        );
    };
    calculation.value(&owner("summary", "result"))
}

#[allow(clippy::unnecessary_wraps)] // compared with `Calculation::value`
fn value(expected: f64) -> Option<Number> {
    Some(Number::new(expected).unwrap())
}

#[test]
fn criterion_text_semantics_match_excel() {
    // Whole-cell, case-insensitive text equality.
    let test = CriterionTest::parse("housing").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("Housing"))));
    assert!(!test.matches(Some(CriterionCell::Text("housing west"))));

    // Wildcards: `*` any run, `?` one character, `~` escapes.
    let test = CriterionTest::parse("f*").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("Food"))));
    assert!(test.matches(Some(CriterionCell::Text("F"))));
    assert!(!test.matches(Some(CriterionCell::Text("beef"))));
    let test = CriterionTest::parse("?b").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("Ab"))));
    assert!(!test.matches(Some(CriterionCell::Text("abba"))));
    let test = CriterionTest::parse("~*").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Text("*"))));
    assert!(!test.matches(Some(CriterionCell::Text("a"))));

    // Numbers, numeric text and relational comparison.
    let test = CriterionTest::parse("5").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Number(5.0))));
    assert!(test.matches(Some(CriterionCell::Text("5"))));
    assert!(!test.matches(Some(CriterionCell::Text("5.5"))));
    let test = CriterionTest::parse(">=5").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Number(5.0))));
    assert!(!test.matches(Some(CriterionCell::Number(4.0))));
    assert!(!test.matches(Some(CriterionCell::Text("6"))));

    // Dates compare as dates, including relationally.
    let january = Date::parse("2026-01-05").expect("valid date");
    let test = CriterionTest::parse("2026-01-05").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Date(january))));
    assert!(!test.matches(Some(CriterionCell::Date(
        Date::parse("2026-01-06").expect("valid date")
    ))));
    let test = CriterionTest::parse(">=2026-01-01").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Date(january))));

    // Yes/no cells and text spelling.
    let test = CriterionTest::parse("TRUE").expect("valid criterion");
    assert!(test.matches(Some(CriterionCell::Boolean(true))));
    assert!(!test.matches(Some(CriterionCell::Boolean(false))));

    // Blanks: `""` matches only blank, `"<>"` matches every non-blank.
    let test = CriterionTest::parse("").expect("valid criterion");
    assert!(test.matches(None));
    assert!(!test.matches(Some(CriterionCell::Text("x"))));
    let test = CriterionTest::parse("<>").expect("valid criterion");
    assert!(!test.matches(None));
    assert!(test.matches(Some(CriterionCell::Number(1.0))));
    // `<>x` includes blanks, which are not `x`.
    let test = CriterionTest::parse("<>x").expect("valid criterion");
    assert!(test.matches(None));
    assert!(!test.matches(Some(CriterionCell::Text("X"))));
}

#[test]
fn all_six_functions_aggregate_the_matching_rows() {
    let category = cell_criterion("category", "criterion_text");

    // COUNTIF/COUNTIFS: housing matches rent and tools case-insensitively.
    assert_eq!(
        result(&with_result(
            document(),
            conditional(ConditionalAggregate::Count, vec![category.clone()])
        )),
        value(2.0)
    );
    // SUMIFS: tools has no reserve, so it contributes nothing.
    assert_eq!(
        result(&with_result(
            document(),
            conditional(
                ConditionalAggregate::Sum(column("plan", "reserve")),
                vec![category.clone()]
            )
        )),
        value(12_000.0)
    );
    // AVERAGEIFS skips the blank reserve instead of treating it as zero.
    assert_eq!(
        result(&with_result(
            document(),
            conditional(
                ConditionalAggregate::Average(column("plan", "reserve")),
                vec![category]
            )
        )),
        value(12_000.0)
    );
    // Two criteria narrow the match to rent.
    assert_eq!(
        result(&with_result(
            document(),
            conditional(
                ConditionalAggregate::Sum(column("plan", "reserve")),
                vec![
                    cell_criterion("category", "criterion_text"),
                    cell_criterion("active", "criterion_bool"),
                ]
            )
        )),
        value(12_000.0)
    );
    // SUMIF and COUNTIF parse as the same node without a repeated range.
    for input in [
        "SUMIF([plan.quota:column], [summary.criterion])",
        "COUNTIF([plan.quota:column], [summary.criterion])",
    ] {
        let parsed = parse_expression(input).expect("valid formula");
        let bound = bind_expression(&document(), &owner("summary", "result"), &parsed)
            .expect("the criterion cell binds");
        assert_eq!(
            project_expression(&document(), &owner("summary", "result"), &bound).expect("projects"),
            if input.starts_with("SUMIF") {
                "SUMIFS([plan.quota:column], [plan.quota:column], [summary.criterion])"
            } else {
                "COUNTIFS([plan.quota:column], [summary.criterion])"
            }
        );
    }
    // SUMIF without a sum range sums the criterion column itself.
    let sumif =
        parse_expression("SUMIF([plan.quota:column], [summary.criterion])").expect("valid formula");
    let bound = bind_expression(&document(), &owner("summary", "result"), &sumif).expect("binds");
    assert_eq!(result(&with_result(document(), bound)), value(4.0));
}

#[test]
fn an_empty_match_sums_zero_and_averages_to_empty() {
    let none = cell_criterion("category", "criterion_text");
    let document = with_result(
        document(),
        conditional(
            ConditionalAggregate::Sum(column("plan", "reserve")),
            vec![none.clone()],
        ),
    );
    let mut document = document;
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("criterion_text"),
        Value::Text("nothing".to_owned()),
    );
    assert_eq!(result(&document), value(0.0));
    let document = with_result(
        document,
        conditional(ConditionalAggregate::Count, vec![none.clone()]),
    );
    assert_eq!(result(&document), value(0.0));
    let document = with_result(
        document,
        conditional(
            ConditionalAggregate::Average(column("plan", "reserve")),
            vec![none],
        ),
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("an empty average must not calculate");
    };
    assert_eq!(
        failures.failures().get(&owner("summary", "result")),
        Some(&CalculationFailure::EmptyAggregate)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_criterion_can_read_another_cell() {
    // A Number cell.
    let number_criterion = conditional(
        ConditionalAggregate::Count,
        vec![cell_criterion("quota", "criterion")],
    );
    assert_eq!(
        result(&with_result(document(), number_criterion)),
        value(2.0)
    );

    // A Text cell holding a plain word.
    let mut fixture = document();
    fixture
        .entities
        .get_mut("rent")
        .unwrap()
        .fields
        .insert(FieldId::from("category"), Value::Text("Food".to_owned()));
    fixture
        .entities
        .get_mut("tools")
        .unwrap()
        .fields
        .insert(FieldId::from("category"), Value::Text("food".to_owned()));
    fixture.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("criterion_text"),
        Value::Text("Food".to_owned()),
    );
    set_result(
        &mut fixture,
        conditional(
            ConditionalAggregate::Count,
            vec![cell_criterion("category", "criterion_text")],
        ),
    );
    assert_eq!(result(&fixture), value(2.0));

    // A Text cell holding a wildcard, a relational number and a boolean.
    for (text, column, expected) in [
        ("h*", "category", 2.0),
        (">1", "quota", 3.0),
        ("TRUE", "active", 3.0),
    ] {
        let mut document = document();
        document.entities.get_mut("summary").unwrap().fields.insert(
            FieldId::from("criterion_text"),
            Value::Text(text.to_owned()),
        );
        set_result(
            &mut document,
            conditional(
                ConditionalAggregate::Count,
                vec![cell_criterion(column, "criterion_text")],
            ),
        );
        assert_eq!(result(&document), value(expected), "{text}");
    }

    // A Text cell holding a date.
    let mut document = document();
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("criterion_text"),
        Value::Text("2026-01-05".to_owned()),
    );
    document
        .entities
        .get_mut("rent")
        .unwrap()
        .fields
        .insert(FieldId::from("due"), date("2026-01-05"));
    set_result(
        &mut document,
        conditional(
            ConditionalAggregate::Count,
            vec![cell_criterion("due", "criterion_text")],
        ),
    );
    assert_eq!(result(&document), value(1.0));

    // A blank criterion cell compares as zero.
    let mut document = self::document();
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .remove("criterion");
    document
        .entities
        .get_mut("blank")
        .unwrap()
        .fields
        .insert(FieldId::from("quota"), number(0.0));
    set_result(
        &mut document,
        conditional(
            ConditionalAggregate::Count,
            vec![cell_criterion("quota", "criterion")],
        ),
    );
    assert_eq!(result(&document), value(1.0));

    // An expression criterion needs no cell.
    set_result(
        &mut document,
        conditional(
            ConditionalAggregate::Count,
            vec![criterion(
                Expression::Add {
                    left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                    right: Box::new(Expression::Number(Number::new(1.0).unwrap())),
                },
                column("plan", "quota"),
            )],
        ),
    );
    assert_eq!(result(&document), value(2.0));
}

#[test]
fn a_same_row_criterion_fills_down_a_calculated_column() {
    let expression = conditional(
        ConditionalAggregate::Count,
        vec![criterion(
            Expression::SameRowReference(FieldId::from("category")),
            column("plan", "category"),
        )],
    );
    let mut document = document();
    for row in ["rent", "ads", "tools", "blank"] {
        document.entities.get_mut(row).unwrap().fields.insert(
            FieldId::from("computed"),
            Value::Formula(expression.clone()),
        );
    }
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("the calculated column must calculate");
    };
    for (row, expected) in [("rent", 2.0), ("ads", 1.0), ("tools", 2.0), ("blank", 0.0)] {
        assert_eq!(
            calculation.value(&owner(row, "computed")),
            value(expected),
            "{row}"
        );
    }
}

#[test]
fn criterion_literals_are_limited_to_numbers_and_blanks() {
    for literal in ["Food", "TRUE", ">=2026-01-01"] {
        let input = format!("COUNTIF([plan.category:column], \"{literal}\")");
        let error = parse_expression(&input).expect_err("literal must be refused");
        assert_eq!(
            error.message,
            "text, date and yes/no criteria can't be written in the formula yet; put the value in a cell and reference it",
            "{input}"
        );
        assert!(error.position > 0, "{input}");
    }
    // A malformed relational literal keeps its own diagnostic.
    let error = parse_expression("COUNTIF([plan.category:column], \">abc\")")
        .expect_err("relational text must be refused");
    assert!(
        error.message.contains("requires a number or a date"),
        "{}",
        error.message
    );
    // `:column` is only an argument suffix.
    let error = parse_expression("[plan.reserve:column]").expect_err("stray suffix");
    assert!(error.message.contains(":column"), "{}", error.message);
}

#[test]
fn invalid_columns_and_references_fail_or_refuse() {
    let mut document = document();
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("result"),
        Value::Formula(conditional(
            ConditionalAggregate::Sum(column("plan", "reserve")),
            vec![cell_criterion("category", "criterion_text")],
        )),
    );

    // A failed value cell fails the formula, matched or not.
    let mut failed = document.clone();
    failed.entities.get_mut("tools").unwrap().fields.insert(
        FieldId::from("reserve"),
        Value::Formula(Expression::Divide {
            left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
        }),
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&failed) else {
        panic!("a failed contributor must fail the formula");
    };
    assert!(matches!(
        failures.failures().get(&owner("summary", "result")),
        Some(CalculationFailure::FailedDependencies { .. })
    ));

    // A non-Number values column is refused at bind time.
    let parsed = parse_expression(
        "SUMIFS([plan.category:column], [plan.category:column], [summary.criterion_text])",
    )
    .expect("valid formula");
    assert!(matches!(
        bind_expression(&document, &owner("summary", "result"), &parsed),
        Err(FormulaBindError::NonNumericColumn { .. })
    ));

    // Different schemas are refused at bind time.
    let parsed = parse_expression(
        "SUMIFS([summary.result:column], [plan.category:column], [summary.criterion_text])",
    )
    .expect("valid formula");
    assert!(matches!(
        bind_expression(&document, &owner("summary", "result"), &parsed),
        Err(FormulaBindError::TypeMismatch { .. })
    ));

    // A directly supplied non-Number values column is a column failure.
    let mut non_numeric = document.clone();
    non_numeric
        .schemas
        .get_mut("plan")
        .unwrap()
        .fields
        .get_mut("reserve")
        .unwrap()
        .field_type = FieldType::Text;
    let CalculationOutcome::Failed(failures) = calculate_complete(&non_numeric) else {
        panic!("a non-Number values column must not sum");
    };
    assert_eq!(
        failures.failures().get(&owner("summary", "result")),
        Some(&CalculationFailure::InvalidColumn {
            columns: BTreeMap::from([(column("plan", "reserve"), ColumnFailure::NonNumeric)]),
        })
    );
}

#[test]
fn stored_shape_validation_refuses_impossible_criteria() {
    // A blank criterion admits only `=` and `<>`.
    let mut document = document();
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("result"),
        Value::Formula(conditional(
            ConditionalAggregate::Count,
            vec![ColumnCriterion {
                column: column("plan", "category"),
                operator: ComparisonOperator::Greater,
                value: None,
            }],
        )),
    );
    let diagnostics = validate_document(&document);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("blank criterion")),
        "{diagnostics:?}"
    );

    // A relational criterion requires a number literal.
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("result"),
        Value::Formula(conditional(
            ConditionalAggregate::Count,
            vec![ColumnCriterion {
                column: column("plan", "category"),
                operator: ComparisonOperator::Greater,
                value: Some(Box::new(Expression::Reference(owner(
                    "summary",
                    "criterion_text",
                )))),
            }],
        )),
    );
    let diagnostics = validate_document(&document);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires a number literal")),
        "{diagnostics:?}"
    );

    // At most 16 criteria.
    let many = (0..17)
        .map(|_| cell_criterion("category", "criterion_text"))
        .collect();
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("result"),
        Value::Formula(conditional(ConditionalAggregate::Count, many)),
    );
    let diagnostics = validate_document(&document);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("at most 16")),
        "{diagnostics:?}"
    );
}

fn row_roots(document: &Document, entity: &str) -> BTreeSet<FieldRef> {
    document.entities[entity]
        .fields
        .keys()
        .map(|field| FieldRef::new(entity, field.clone()))
        .collect()
}

#[allow(clippy::needless_pass_by_value)] // callers build the set inline
fn update(retained: &mut RetainedCalculationState, document: &Document, roots: BTreeSet<FieldRef>) {
    let _ = retained.update(document, &roots);
    assert_eq!(
        retained.outcome(),
        calculate_complete(document),
        "incremental calculation must equal a full one"
    );
}

#[allow(clippy::too_many_lines)]
#[test]
fn incremental_calculation_equals_full() {
    let mut document = document();
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("result"),
        Value::Formula(conditional(
            ConditionalAggregate::Sum(column("plan", "reserve")),
            vec![
                cell_criterion("category", "criterion_text"),
                cell_criterion("active", "criterion_bool"),
                cell_criterion("due", "criterion_date"),
            ],
        )),
    );
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);

    // Edit a Text criterion cell: housing → marketing leaves rent (12000) and
    // ads (500) matching the text, and the boolean/date criteria still exclude
    // ads (false) but admit rent.
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("criterion_text"),
        Value::Text("marketing".to_owned()),
    );
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("summary", "criterion_text")]),
    );
    let _ = &document;

    // Edit the Boolean criterion cell to false: nothing matches.
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .insert(FieldId::from("criterion_bool"), Value::Boolean(false));
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("summary", "criterion_bool")]),
    );

    // Edit the Date criterion cell.
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .insert(FieldId::from("criterion_date"), date("2026-02-01"));
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("summary", "criterion_date")]),
    );

    // Unset a criterion cell so it is blank, then set it back.
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .remove("criterion_date");
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("summary", "criterion_date")]),
    );
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .insert(FieldId::from("criterion_date"), date("2026-02-01"));
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("summary", "criterion_date")]),
    );
    document
        .entities
        .get_mut("summary")
        .unwrap()
        .fields
        .insert(FieldId::from("criterion_bool"), Value::Boolean(true));
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("summary", "criterion_bool")]),
    );

    // Edit a matching Number value.
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .insert(FieldId::from("reserve"), number(1_000.0));
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("ads", "reserve")]),
    );

    // Edit the cell the criterion reads.
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .insert(FieldId::from("due"), date("2026-02-01"));
    update(
        &mut retained,
        &document,
        BTreeSet::from([owner("ads", "due")]),
    );

    // Append a matching row.
    document.entities.insert(
        EntityId::from("food"),
        plan_row(
            "food",
            &[
                ("category", Value::Text("marketing".to_owned())),
                ("active", Value::Boolean(true)),
                ("due", date("2026-02-01")),
                ("reserve", number(250.0)),
                ("quantity", number(1.0)),
                ("quota", number(1.0)),
            ],
        ),
    );
    update(&mut retained, &document, row_roots(&document, "food"));

    // Remove it again.
    let roots = row_roots(&document, "food");
    document.entities.remove("food");
    update(&mut retained, &document, roots);

    // The final full calculation is the truth the incremental one matched.
    assert_eq!(retained.outcome(), calculate_complete(&document));
}

#[test]
fn a_conditional_aggregate_over_its_own_row_is_a_cycle() {
    let mut document = document();
    for row in ["rent", "ads", "tools", "blank"] {
        document.entities.get_mut(row).unwrap().fields.insert(
            FieldId::from("computed"),
            Value::Formula(conditional(
                ConditionalAggregate::Count,
                vec![cell_criterion("computed", "criterion")],
            )),
        );
    }
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a self-including conditional aggregate must not calculate");
    };
    assert_eq!(
        failures.failures().get(&owner("rent", "computed")),
        Some(&CalculationFailure::Cycle {
            members: Arc::new(BTreeSet::from([
                owner("ads", "computed"),
                owner("blank", "computed"),
                owner("rent", "computed"),
                owner("tools", "computed"),
            ])),
        })
    );
}

#[test]
fn conditional_spellings_are_case_insensitive_and_canonical() {
    for (input, expected) in [
        (
            "countif([plan.category:column], \"\")",
            "COUNTIFS([plan.category:column], \"\")",
        ),
        (
            "sumif([plan.quota:column], \"<>5\")",
            "SUMIFS([plan.quota:column], [plan.quota:column], \"<>5\")",
        ),
        (
            "AVERAGEIFS([plan.reserve:column], [plan.category:column], \">=1\")",
            "AVERAGEIFS([plan.reserve:column], [plan.category:column], \">=1\")",
        ),
    ] {
        let parsed = parse_expression(input).expect("valid formula");
        assert_eq!(format_unbound_expression(&parsed), expected);
        let reparsed = parse_expression(expected).expect("canonical text parses");
        assert_eq!(format_unbound_expression(&reparsed), expected);
    }
    // A text criterion that is a bare cell reference prints without quotes.
    let parsed = parse_expression("COUNTIFS([plan.category:column], [summary.criterion_text])")
        .expect("valid formula");
    assert_eq!(
        format_unbound_expression(&parsed),
        "COUNTIFS([plan.category:column], [summary.criterion_text])"
    );
}
