//! A conditional aggregate's memo and candidate walk must give exactly the
//! results of the full per-row walk: the same values, failures and sum order.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate, Date, Document,
    DocumentId, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

use crate::{CalculationOutcome, RetainedCalculationState, calculate_complete};

use super::conditional_walks;

/// The walk a case must take.
#[derive(Clone, Copy, Debug)]
enum Walk {
    /// The first criterion named the candidate rows.
    Candidates,
    /// The first criterion named no candidate row at all.
    NoCandidates,
    /// The first criterion had no keys, or its column holds a cell that may
    /// fail, so every row was read.
    Full,
    /// A failed dependency ended the formula before any row was read.
    Skipped,
}

fn field(name: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(name),
        key: FieldKey::from(name),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format: None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("finite"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn date(value: &str) -> Value {
    Value::Date(Date::parse(value).expect("valid date"))
}

fn divide(left: f64, right: f64) -> Expression {
    Expression::Divide {
        left: Box::new(literal(left)),
        right: Box::new(literal(right)),
    }
}

fn literal(value: f64) -> Expression {
    Expression::Number(Number::new(value).expect("finite"))
}

fn add(left: f64, right: f64) -> Expression {
    Expression::Add {
        left: Box::new(literal(left)),
        right: Box::new(literal(right)),
    }
}

fn multiply(left: f64, right: f64) -> Expression {
    Expression::Multiply {
        left: Box::new(literal(left)),
        right: Box::new(literal(right)),
    }
}

fn column(name: &str) -> ColumnRef {
    ColumnRef::new(SchemaId::from("row"), FieldId::from(name))
}

fn summary_reference(name: &str) -> Expression {
    Expression::Reference(FieldRef::new("summary", name))
}

fn same_row_reference(name: &str) -> Expression {
    Expression::SameRowReference(FieldId::from(name))
}

fn criterion(column: ColumnRef, value: Expression) -> ColumnCriterion {
    ColumnCriterion {
        column,
        operator: ComparisonOperator::Equal,
        value: Some(Box::new(value)),
    }
}

fn blank_criterion(column: ColumnRef, operator: ComparisonOperator) -> ColumnCriterion {
    ColumnCriterion {
        column,
        operator,
        value: None,
    }
}

fn conditional(aggregate: ConditionalAggregate, criteria: Vec<ColumnCriterion>) -> Expression {
    Expression::ColumnConditional {
        aggregate: Box::new(aggregate),
        criteria,
    }
}

fn row_schema() -> Schema {
    Schema {
        field_order: None,
        id: SchemaId::from("row"),
        key: SchemaKey::from("row"),
        display_name: None,
        fields: BTreeMap::from([
            (
                FieldId::from("category"),
                field("category", FieldType::Text),
            ),
            (FieldId::from("amount"), field("amount", FieldType::Number)),
            (FieldId::from("other"), field("other", FieldType::Number)),
            (FieldId::from("flag"), field("flag", FieldType::Boolean)),
            (FieldId::from("due"), field("due", FieldType::Date)),
            (FieldId::from("note"), field("note", FieldType::Text)),
            (FieldId::from("tag"), field("tag", FieldType::Number)),
            (FieldId::from("check"), field("check", FieldType::Number)),
        ]),
    }
}

fn summary_schema() -> Schema {
    Schema {
        field_order: None,
        id: SchemaId::from("summary"),
        key: SchemaKey::from("summary"),
        display_name: None,
        fields: BTreeMap::from([
            (FieldId::from("result"), field("result", FieldType::Number)),
            (
                FieldId::from("other_result"),
                field("other_result", FieldType::Number),
            ),
            (
                FieldId::from("criterion_number"),
                field("criterion_number", FieldType::Number),
            ),
            (
                FieldId::from("criterion_text"),
                field("criterion_text", FieldType::Text),
            ),
            (
                FieldId::from("criterion_boolean"),
                field("criterion_boolean", FieldType::Boolean),
            ),
            (
                FieldId::from("criterion_date"),
                field("criterion_date", FieldType::Date),
            ),
        ]),
    }
}

fn row(id: &str, fields: &[(&str, Value)]) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from("row"),
        fields: fields
            .iter()
            .map(|(name, value)| (FieldId::from(*name), value.clone()))
            .collect(),
    }
}

fn document(rows: Vec<Entity>, summary_fields: &[(&str, Value)]) -> Document {
    let mut summary = Entity {
        id: EntityId::from("summary"),
        key: EntityKey::from("summary"),
        schema: SchemaId::from("summary"),
        fields: summary_fields
            .iter()
            .map(|(name, value)| (FieldId::from(*name), value.clone()))
            .collect(),
    };
    summary
        .fields
        .entry(FieldId::from("result"))
        .or_insert_with(|| number(0.0));
    summary
        .fields
        .entry(FieldId::from("other_result"))
        .or_insert_with(|| number(0.0));
    let mut entities = rows
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect::<BTreeMap<_, _>>();
    entities.insert(EntityId::from("summary"), summary);
    Document {
        id: DocumentId::from("conditional-walks"),
        title: "Conditional walks".to_owned(),
        schemas: BTreeMap::from([
            (SchemaId::from("row"), row_schema()),
            (SchemaId::from("summary"), summary_schema()),
        ]),
        entities,
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

/// Five rows covering blank cells, a formula cell in the criterion column and
/// one in the sum column, text that spells a number and a boolean, and a date.
fn base() -> Document {
    document(
        vec![
            row(
                "a",
                &[
                    ("category", text("food")),
                    ("amount", number(1.0)),
                    ("other", number(10.0)),
                    ("flag", Value::Boolean(true)),
                    ("due", date("2026-01-05")),
                    ("note", text("one")),
                    ("tag", number(1.0)),
                ],
            ),
            row(
                "b",
                &[
                    ("category", text("Food")),
                    ("amount", number(2.0)),
                    ("other", number(20.0)),
                    ("flag", Value::Boolean(false)),
                    ("due", date("2026-02-01")),
                    ("note", text("TRUE")),
                    ("tag", number(2.0)),
                ],
            ),
            row(
                "c",
                &[
                    ("category", text("drink")),
                    ("amount", number(3.0)),
                    ("other", number(30.0)),
                    ("flag", Value::Boolean(true)),
                    ("due", date("2026-01-05")),
                    ("tag", Value::Formula(add(1.0, 2.0))),
                ],
            ),
            row(
                "d",
                &[
                    ("amount", Value::Formula(multiply(2.0, 3.0))),
                    ("other", number(40.0)),
                    ("flag", Value::Boolean(false)),
                    ("note", text("true")),
                ],
            ),
            row(
                "e",
                &[
                    ("category", text("ÀB")),
                    ("amount", number(0.0)),
                    ("other", number(50.0)),
                    ("flag", Value::Boolean(true)),
                    ("due", date("2026-03-01")),
                    ("note", text("1")),
                    ("tag", number(1.5)),
                ],
            ),
        ],
        &[
            ("criterion_number", number(1.0)),
            ("criterion_text", text("food")),
            ("criterion_boolean", Value::Boolean(true)),
            ("criterion_date", date("2026-01-05")),
        ],
    )
}

fn with_result(mut document: Document, expression: Expression) -> Document {
    document
        .entities
        .get_mut("summary")
        .expect("the summary row exists")
        .fields
        .insert(FieldId::from("result"), Value::Formula(expression));
    document
}

fn set(document: &mut Document, entity: &str, name: &str, value: Value) {
    document
        .entities
        .get_mut(entity)
        .expect("the row exists")
        .fields
        .insert(FieldId::from(name), value);
}

fn result(outcome: &CalculationOutcome) -> Option<Number> {
    match outcome {
        CalculationOutcome::Complete(calculation) => {
            calculation.value(&FieldRef::new("summary", "result"))
        }
        CalculationOutcome::Failed(_) => None,
    }
}

/// The document calculated with the caches forced off.
fn forced_calculation(document: &Document) -> CalculationOutcome {
    conditional_walks::set_bypass(true);
    let outcome = calculate_complete(document);
    conditional_walks::set_bypass(false);
    outcome
}

/// The document calculates to the same outcome with and without the caches,
/// through the expected walk.
fn same_as_the_full_walk(document: &Document, walk: Walk) -> CalculationOutcome {
    conditional_walks::take();
    let calculated = calculate_complete(document);
    let (rows, candidate_walks, full_walks) = conditional_walks::take();
    match walk {
        Walk::Candidates => {
            assert!(rows > 0, "the candidate walk must read rows");
            assert!(candidate_walks > 0, "the candidate walk must run");
        }
        Walk::NoCandidates => {
            assert_eq!(rows, 0, "an empty candidate list reads no row");
            assert!(candidate_walks > 0, "the candidate walk must run");
        }
        Walk::Full => {
            assert!(rows > 0, "the full walk must read rows");
            assert!(full_walks > 0, "the full walk must run");
        }
        Walk::Skipped => assert_eq!(
            candidate_walks + full_walks,
            0,
            "a failed dependency must end the formula before any row is read"
        ),
    }
    assert_eq!(
        calculated,
        forced_calculation(document),
        "the caches must not change the outcome"
    );
    calculated
}

#[test]
fn number_text_and_mixed_criteria_match_the_full_walk() {
    // A Number criterion column with blanks and a formula cell.
    let counted = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(column("tag"), literal(1.0))],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&counted), Some(Number::new(1.0).expect("finite")));

    // A Text criterion column read from a summary cell, summed case-insensitively.
    let summed = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![criterion(
                    column("category"),
                    summary_reference("criterion_text"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&summed), Some(Number::new(3.0).expect("finite")));

    // A mixed pair of criteria over a Number and a Text column.
    let mixed = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![
                    criterion(column("tag"), literal(1.0)),
                    criterion(column("category"), summary_reference("criterion_text")),
                ],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&mixed), Some(Number::new(1.0).expect("finite")));

    // An expression criterion: 1 + 1 reads as the number 2.
    let expression = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(column("tag"), add(1.0, 1.0))],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&expression), Some(Number::new(1.0).expect("finite")));

    // Three criteria.
    let three = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![
                    criterion(column("category"), summary_reference("criterion_text")),
                    criterion(column("flag"), summary_reference("criterion_boolean")),
                    criterion(column("tag"), literal(1.0)),
                ],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&three), Some(Number::new(1.0).expect("finite")));
}

#[test]
fn a_same_row_criterion_fills_down_and_matches_the_full_walk() {
    let expression = conditional(
        ConditionalAggregate::Count,
        vec![criterion(column("tag"), same_row_reference("tag"))],
    );
    let mut document = base();
    for id in ["a", "b", "c", "d", "e"] {
        set(
            &mut document,
            id,
            "check",
            Value::Formula(expression.clone()),
        );
    }
    let outcome = same_as_the_full_walk(&document, Walk::Candidates);
    let CalculationOutcome::Complete(_) = outcome else {
        panic!("the calculated column must calculate");
    };
    // Every row's own tag matches itself, except the blank one.
    let values = match outcome {
        CalculationOutcome::Complete(calculation) => (0..5)
            .map(|index| {
                let id = ["a", "b", "c", "d", "e"][index];
                calculation.value(&FieldRef::new(id, "check"))
            })
            .collect::<Vec<_>>(),
        CalculationOutcome::Failed(_) => unreachable!(),
    };
    let expected = [1.0, 1.0, 1.0, 0.0, 1.0]
        .into_iter()
        .map(|value| Some(Number::new(value).expect("finite")))
        .collect::<Vec<_>>();
    assert_eq!(values, expected);
}

#[test]
fn failures_in_criterion_and_sum_columns_match_the_full_walk() {
    // A failed cell in the criterion column.
    let mut document = base();
    set(&mut document, "b", "tag", Value::Formula(divide(1.0, 0.0)));
    let failed_criterion = same_as_the_full_walk(
        &with_result(
            document,
            conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![criterion(column("tag"), literal(1.0))],
            ),
        ),
        Walk::Skipped,
    );
    assert!(matches!(failed_criterion, CalculationOutcome::Failed(_)));

    // A failed cell in the sum column.
    let mut document = base();
    set(
        &mut document,
        "b",
        "amount",
        Value::Formula(divide(1.0, 0.0)),
    );
    let failed_sum = same_as_the_full_walk(
        &with_result(
            document,
            conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![criterion(column("tag"), literal(1.0))],
            ),
        ),
        Walk::Skipped,
    );
    assert!(matches!(failed_sum, CalculationOutcome::Failed(_)));

    // A failed cell in a second criterion's column, on a row the first
    // criterion rejects and on a row it accepts.
    for (id, walk) in [("d", Walk::Skipped), ("a", Walk::Skipped)] {
        let mut document = base();
        set(&mut document, id, "tag", Value::Formula(divide(1.0, 0.0)));
        let failed_second = same_as_the_full_walk(
            &with_result(
                document,
                conditional(
                    ConditionalAggregate::Count,
                    vec![
                        criterion(column("category"), summary_reference("criterion_text")),
                        criterion(column("tag"), literal(1.0)),
                    ],
                ),
            ),
            walk,
        );
        assert!(matches!(failed_second, CalculationOutcome::Failed(_)));
    }
}

#[test]
fn a_reference_cell_in_a_criterion_column_matches_the_full_walk() {
    // In the first criterion's column the whole walk has to run, and the row
    // fails the formula.
    let mut document = base();
    set(
        &mut document,
        "a",
        "category",
        Value::Reference(EntityId::from("b")),
    );
    let first = same_as_the_full_walk(
        &with_result(
            document,
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(
                    column("category"),
                    summary_reference("criterion_text"),
                )],
            ),
        ),
        Walk::Full,
    );
    assert!(matches!(first, CalculationOutcome::Failed(_)));

    // In a second criterion's column, on a row the first criterion rejects,
    // the row is never read and the formula still calculates.
    let mut document = base();
    set(
        &mut document,
        "d",
        "tag",
        Value::Reference(EntityId::from("b")),
    );
    let rejected = same_as_the_full_walk(
        &with_result(
            document,
            conditional(
                ConditionalAggregate::Count,
                vec![
                    criterion(column("category"), summary_reference("criterion_text")),
                    criterion(column("tag"), literal(1.0)),
                ],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&rejected), Some(Number::new(1.0).expect("finite")));

    // On a row the first criterion accepts, the row is read and fails.
    let mut document = base();
    set(
        &mut document,
        "a",
        "tag",
        Value::Reference(EntityId::from("b")),
    );
    let accepted = same_as_the_full_walk(
        &with_result(
            document,
            conditional(
                ConditionalAggregate::Count,
                vec![
                    criterion(column("category"), summary_reference("criterion_text")),
                    criterion(column("tag"), literal(1.0)),
                ],
            ),
        ),
        Walk::Candidates,
    );
    assert!(matches!(accepted, CalculationOutcome::Failed(_)));
}

#[test]
fn every_aggregate_matches_the_full_walk() {
    let count = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(
                    column("category"),
                    summary_reference("criterion_text"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&count), Some(Number::new(2.0).expect("finite")));

    let sum = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![criterion(
                    column("category"),
                    summary_reference("criterion_text"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&sum), Some(Number::new(3.0).expect("finite")));

    let average = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Average(column("amount")),
                vec![criterion(
                    column("category"),
                    summary_reference("criterion_text"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&average), Some(Number::new(1.5).expect("finite")));

    // An average with no match is the empty aggregate failure.
    let empty = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Average(column("amount")),
                vec![criterion(column("tag"), literal(99.0))],
            ),
        ),
        Walk::NoCandidates,
    );
    let CalculationOutcome::Failed(failures) = empty else {
        panic!("an empty average must not calculate");
    };
    assert_eq!(
        failures.failures().get(&FieldRef::new("summary", "result")),
        Some(&crate::CalculationFailure::EmptyAggregate)
    );
}

#[test]
fn scanning_criteria_match_the_full_walk() {
    // A wildcard and the blank text have no key and read every row.
    for value in [text("fo*"), text("")] {
        let mut document = base();
        set(&mut document, "summary", "criterion_text", value);
        same_as_the_full_walk(
            &with_result(
                document,
                conditional(
                    ConditionalAggregate::Count,
                    vec![criterion(
                        column("category"),
                        summary_reference("criterion_text"),
                    )],
                ),
            ),
            Walk::Full,
        );
    }
    // `<>` is not `=`, so it reads every row too.
    let not_blank = with_result(
        base(),
        conditional(
            ConditionalAggregate::Count,
            vec![blank_criterion(column("tag"), ComparisonOperator::NotEqual)],
        ),
    );
    same_as_the_full_walk(&not_blank, Walk::Full);
    let relational = with_result(
        base(),
        conditional(
            ConditionalAggregate::Count,
            vec![ColumnCriterion {
                column: column("tag"),
                operator: ComparisonOperator::GreaterOrEqual,
                value: Some(Box::new(literal(1.5))),
            }],
        ),
    );
    same_as_the_full_walk(&relational, Walk::Full);

    // A boolean criterion also matches the text spelling of the value, in a
    // Boolean column and in a Text column.
    let booleans = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(
                    column("flag"),
                    summary_reference("criterion_boolean"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&booleans), Some(Number::new(3.0).expect("finite")));
    let boolean_text = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(
                    column("note"),
                    summary_reference("criterion_boolean"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(
        result(&boolean_text),
        Some(Number::new(2.0).expect("finite"))
    );

    // A date criterion reads a Date column and its text spelling.
    let dates = same_as_the_full_walk(
        &with_result(
            base(),
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(
                    column("due"),
                    summary_reference("criterion_date"),
                )],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(result(&dates), Some(Number::new(2.0).expect("finite")));
}

#[test]
fn zero_signs_and_numeric_text_match_the_full_walk() {
    let amounts = conditional(
        ConditionalAggregate::Count,
        vec![criterion(column("amount"), literal(0.0))],
    );
    let mut document = base();
    set(&mut document, "a", "amount", number(-0.0));
    set(&mut document, "b", "amount", number(0.0));
    let zeros = same_as_the_full_walk(&with_result(document, amounts), Walk::Candidates);
    assert_eq!(result(&zeros), Some(Number::new(3.0).expect("finite")));

    // Text cells that spell the same number as the operand.
    let mut document = base();
    set(&mut document, "a", "note", text("1"));
    set(&mut document, "b", "note", text("1.0"));
    set(&mut document, "c", "note", text("1e0"));
    set(&mut document, "d", "note", text("+1"));
    let numeric_text = same_as_the_full_walk(
        &with_result(
            document,
            conditional(
                ConditionalAggregate::Count,
                vec![criterion(column("note"), literal(1.0))],
            ),
        ),
        Walk::Candidates,
    );
    assert_eq!(
        result(&numeric_text),
        Some(Number::new(5.0).expect("finite"))
    );
}

#[test]
fn one_key_computes_once_and_each_aggregate_stays_its_own() {
    // The same formula on two owners has one key: the second owner reads the
    // first one's result.
    let expression = conditional(
        ConditionalAggregate::Sum(column("amount")),
        vec![criterion(
            column("category"),
            summary_reference("criterion_text"),
        )],
    );
    let mut document = with_result(base(), expression.clone());
    document
        .entities
        .get_mut("summary")
        .expect("the summary row exists")
        .fields
        .insert(FieldId::from("other_result"), Value::Formula(expression));
    conditional_walks::take();
    let _ = calculate_complete(&document);
    let (rows, candidate_walks, full_walks) = conditional_walks::take();
    assert!(rows > 0);
    assert_eq!(
        candidate_walks + full_walks,
        1,
        "two identical formulas must walk once"
    );

    // A different aggregate or a different sum column is a different key and
    // both still match the full walk.
    let other_aggregate = with_result(
        base(),
        conditional(
            ConditionalAggregate::Average(column("amount")),
            vec![criterion(
                column("category"),
                summary_reference("criterion_text"),
            )],
        ),
    );
    same_as_the_full_walk(&other_aggregate, Walk::Candidates);
    let other_column = with_result(
        base(),
        conditional(
            ConditionalAggregate::Sum(column("other")),
            vec![criterion(
                column("category"),
                summary_reference("criterion_text"),
            )],
        ),
    );
    let outcome = same_as_the_full_walk(&other_column, Walk::Candidates);
    assert_eq!(result(&outcome), Some(Number::new(30.0).expect("finite")));
}

#[test]
fn the_sum_keeps_entity_order_bit_for_bit() {
    let mut document = document(
        vec![
            row("a", &[("category", text("x")), ("amount", number(1e16))]),
            row("b", &[("category", text("x")), ("amount", number(1.0))]),
            row("c", &[("category", text("x")), ("amount", number(-1e16))]),
        ],
        &[("criterion_text", text("x"))],
    );
    document
        .entities
        .get_mut("summary")
        .expect("the summary row exists")
        .fields
        .insert(
            FieldId::from("result"),
            Value::Formula(conditional(
                ConditionalAggregate::Sum(column("amount")),
                vec![criterion(
                    column("category"),
                    summary_reference("criterion_text"),
                )],
            )),
        );
    let outcome = same_as_the_full_walk(&document, Walk::Candidates);
    let CalculationOutcome::Complete(calculation) = outcome else {
        panic!("the sum must calculate");
    };
    let total = calculation
        .value(&FieldRef::new("summary", "result"))
        .expect("the formula resolved");
    let entity_order = [1e16_f64, 1.0, -1e16]
        .into_iter()
        .fold(0.0_f64, |sum, value| sum + value);
    let other_order = [1e16_f64, -1e16, 1.0]
        .into_iter()
        .fold(0.0_f64, |sum, value| sum + value);
    assert_eq!(
        total.to_bits(),
        entity_order.to_bits(),
        "the conditional sum adds its cells in entity order"
    );
    assert_ne!(
        entity_order.to_bits(),
        other_order.to_bits(),
        "the chosen values make the sum order-dependent"
    );
}

#[test]
fn a_retained_edit_matches_the_full_walk() {
    let mut document = with_result(
        base(),
        conditional(
            ConditionalAggregate::Sum(column("amount")),
            vec![criterion(
                column("category"),
                summary_reference("criterion_text"),
            )],
        ),
    );
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);

    set(&mut document, "summary", "criterion_text", text("drink"));
    let _ = retained.update(
        &document,
        &BTreeSet::from([FieldRef::new("summary", "criterion_text")]),
    );
    assert_eq!(retained.outcome(), forced_calculation(&document));

    set(&mut document, "a", "amount", number(4.0));
    let _ = retained.update(&document, &BTreeSet::from([FieldRef::new("a", "amount")]));
    assert_eq!(retained.outcome(), forced_calculation(&document));
}

#[test]
fn one_conditional_per_row_reads_a_bounded_number_of_rows() {
    const ROWS: usize = 2_000;

    // Every row's own criterion, over one Text column and one Number column.
    for categories in [ROWS, 10] {
        let rows = (0..ROWS)
            .map(|index| {
                let value = f64::from(u32::try_from(index).expect("rows fit in u32"));
                let category = format!("g{}", index % categories);
                row(
                    &format!("r{index:04}"),
                    &[
                        ("category", text(&category)),
                        ("amount", number(value)),
                        ("check", Value::Formula(per_row_conditional())),
                    ],
                )
            })
            .collect::<Vec<_>>();
        let document = document(rows, &[]);

        conditional_walks::take();
        let outcome = calculate_complete(&document);
        let (rows_read, candidate_walks, full_walks) = conditional_walks::take();
        assert!(
            matches!(outcome, CalculationOutcome::Complete(_)),
            "the per-row conditional must calculate"
        );
        assert!(candidate_walks > 0 && full_walks == 0);
        assert!(
            rows_read <= 2 * ROWS,
            "{ROWS} formulas with {categories} categories read {rows_read} rows"
        );

        conditional_walks::set_bypass(true);
        conditional_walks::take();
        let forced = calculate_complete(&document);
        let (forced_rows, _, _) = conditional_walks::take();
        conditional_walks::set_bypass(false);
        assert!(matches!(forced, CalculationOutcome::Complete(_)));
        assert_eq!(
            forced_rows,
            ROWS * ROWS,
            "the full walk reads every row per formula"
        );
    }
}

/// `COUNTIF(row.category, this row's category)`, one per row.
fn per_row_conditional() -> Expression {
    conditional(
        ConditionalAggregate::Count,
        vec![criterion(
            column("category"),
            same_row_reference("category"),
        )],
    )
}
