//! `[.field]` keeps meaning "this row" through binding, projection, and
//! evaluation.
use std::collections::BTreeMap;

use tachiko_formula_engine::{
    CalculationFailure, CalculationOutcome, FormulaBindError, UnboundExpression, bind_expression,
    calculate_complete, extract_dependencies, format_unbound_expression, parse_expression,
    project_expression,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldAddress, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey,
    Value,
};

fn number_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        field_type: FieldType::Number,
        required: true,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn text_field(id: &str) -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Text,
        ..number_field(id)
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn reference(entity: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(entity, field))
}

fn reserve_formula() -> Expression {
    Expression::Multiply {
        left: Box::new(Expression::SameRowReference(FieldId::from("expense"))),
        right: Box::new(reference("global", "rate")),
    }
}

fn document() -> Document {
    let plan_fields = BTreeMap::from([
        (FieldId::from("item"), text_field("item")),
        (FieldId::from("expense"), number_field("expense")),
        (FieldId::from("reserve"), number_field("reserve")),
    ]);
    let settings_fields = BTreeMap::from([(FieldId::from("rate"), number_field("rate"))]);
    let plan = |id: &str, key: &str, expense: f64| Entity {
        id: EntityId::from(id),
        key: EntityKey::from(key),
        schema: SchemaId::from("plan"),
        fields: BTreeMap::from([
            (FieldId::from("item"), Value::Text(key.to_owned())),
            (FieldId::from("expense"), number(expense)),
            (FieldId::from("reserve"), Value::Formula(reserve_formula())),
        ]),
    };
    Document {
        id: DocumentId::from("monthly-plan"),
        title: "Monthly plan".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("plan"),
                Schema {
                    field_order: None,
                    id: SchemaId::from("plan"),
                    key: SchemaKey::from("plan"),
                    display_name: None,
                    fields: plan_fields,
                },
            ),
            (
                SchemaId::from("settings"),
                Schema {
                    field_order: None,
                    id: SchemaId::from("settings"),
                    key: SchemaKey::from("settings"),
                    display_name: None,
                    fields: settings_fields,
                },
            ),
        ]),
        entities: BTreeMap::from([
            (EntityId::from("rent"), plan("rent", "Rent", 80_000.0)),
            (EntityId::from("ads"), plan("ads", "Ads", 30_000.0)),
            (
                EntityId::from("global"),
                Entity {
                    id: EntityId::from("global"),
                    key: EntityKey::from("global"),
                    schema: SchemaId::from("settings"),
                    fields: BTreeMap::from([(FieldId::from("rate"), number(0.1))]),
                },
            ),
        ]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn owner(entity: &str, field: &str) -> FieldRef {
    FieldRef::new(entity, field)
}

#[test]
fn the_shorthand_parses_and_formats_as_written() {
    let parsed = parse_expression("[.expense] * [settings.rate]").unwrap();
    assert_eq!(
        parsed,
        UnboundExpression::Multiply {
            left: Box::new(UnboundExpression::SameRowReference(FieldKey::from(
                "expense"
            ))),
            right: Box::new(UnboundExpression::Reference(FieldAddress::new(
                "settings", "rate"
            ))),
        }
    );
    assert_eq!(
        format_unbound_expression(&parsed),
        "([.expense] * [settings.rate])"
    );
}

#[test]
fn malformed_shorthands_are_refused_at_their_position() {
    for input in ["[.]", "[.a.b]", "[.Expense]"] {
        assert!(parse_expression(input).is_err(), "{input} must be refused");
    }
}

#[test]
fn binding_resolves_the_field_against_the_owners_schema() {
    let document = document();
    let bound = bind_expression(
        &document,
        &owner("rent", "reserve"),
        &parse_expression("[.expense]").unwrap(),
    )
    .unwrap();
    assert_eq!(
        bound,
        Expression::SameRowReference(FieldId::from("expense"))
    );
    assert_eq!(
        extract_dependencies(&document, &owner("rent", "reserve"), &bound),
        std::collections::BTreeSet::from([owner("rent", "expense")])
    );
}

#[test]
fn binding_refuses_an_unknown_or_non_numeric_own_field() {
    let document = document();
    let unknown = bind_expression(
        &document,
        &owner("rent", "reserve"),
        &parse_expression("[.missing]").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(unknown, FormulaBindError::Address { .. }));

    let non_numeric = bind_expression(
        &document,
        &owner("rent", "reserve"),
        &parse_expression("[.item]").unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        non_numeric,
        FormulaBindError::NonNumericTarget {
            address: FieldAddress::new("Rent", "item"),
            reference: owner("rent", "item"),
        }
    );
}

#[test]
fn projection_keeps_the_shorthand_and_follows_the_owning_field_key() {
    let mut document = document();
    let mut expression = reserve_formula();
    assert_eq!(
        project_expression(&document, &owner("rent", "reserve"), &expression).unwrap(),
        "([.expense] * [global.rate])"
    );

    document
        .schemas
        .get_mut("plan")
        .unwrap()
        .fields
        .get_mut("expense")
        .unwrap()
        .key = FieldKey::from("cost");
    // The bound node still holds the field identity; projection follows the
    // current key for display.
    if let Expression::Multiply { left, .. } = &mut expression {
        assert!(matches!(**left, Expression::SameRowReference(_)));
    }
    assert_eq!(
        project_expression(&document, &owner("rent", "reserve"), &expression).unwrap(),
        "([.cost] * [global.rate])"
    );
}

#[test]
fn each_row_resolves_its_own_field() {
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document()) else {
        panic!("the fixture must calculate");
    };
    assert_eq!(
        calculation.value(&owner("rent", "reserve")),
        Some(Number::new(8_000.0).unwrap())
    );
    assert_eq!(
        calculation.value(&owner("ads", "reserve")),
        Some(Number::new(3_000.0).unwrap())
    );
}

#[test]
fn referring_to_the_own_formula_field_is_a_cycle() {
    let mut document = document();
    document.entities.get_mut("rent").unwrap().fields.insert(
        FieldId::from("reserve"),
        Value::Formula(Expression::SameRowReference(FieldId::from("reserve"))),
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a self-reference must not calculate");
    };
    assert_eq!(
        failures.failures().get(&owner("rent", "reserve")),
        Some(&CalculationFailure::Cycle {
            members: std::collections::BTreeSet::from([owner("rent", "reserve")]),
        })
    );
}
