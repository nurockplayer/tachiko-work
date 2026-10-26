//! `[schema.field:total]` sums one Number column over a schema's rows while
//! keeping the identity of the column rather than of the rows it happens to
//! contain.
use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{
    CalculationFailure, CalculationOutcome, CanonicalAuthoringProjectionError, ColumnFailure,
    FormulaBindError, ReferenceFailure, RetainedCalculationState, UnboundExpression,
    bind_expression, calculate_complete, extract_dependencies, format_unbound_expression,
    parse_expression, project_expression,
};
use tachiko_semantic_core::{
    ColumnAddress, ColumnRef, Document, DocumentId, Entity, EntityId, EntityKey, Expression,
    FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema,
    SchemaId, SchemaKey, Value,
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

fn owner(entity: &str, field: &str) -> FieldRef {
    FieldRef::new(entity, field)
}

fn column(schema: &str, field: &str) -> ColumnRef {
    ColumnRef::new(SchemaId::from(schema), FieldId::from(field))
}

fn total(schema: &str, field: &str) -> Expression {
    Expression::ColumnTotal(column(schema, field))
}

fn plan_row(id: &str, key: &str, reserve: Value) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(key),
        schema: SchemaId::from("plan"),
        fields: BTreeMap::from([
            (FieldId::from("item"), Value::Text(key.to_owned())),
            (FieldId::from("reserve"), reserve),
        ]),
    }
}

fn schema(id: &str, key: &str, fields: BTreeMap<FieldId, FieldDefinition>) -> Schema {
    Schema {
        id: SchemaId::from(id),
        key: SchemaKey::from(key),
        display_name: None,
        fields,
    }
}

/// Two plan rows and one summary row whose total reads both of them.
fn document() -> Document {
    Document {
        id: DocumentId::from("monthly-plan"),
        title: "Monthly plan".to_owned(),
        schemas: BTreeMap::from([
            (
                SchemaId::from("plan"),
                schema(
                    "plan",
                    "plan",
                    BTreeMap::from([
                        (FieldId::from("item"), text_field("item")),
                        (FieldId::from("reserve"), number_field("reserve")),
                    ]),
                ),
            ),
            (
                SchemaId::from("summary"),
                schema(
                    "summary",
                    "summary",
                    BTreeMap::from([(FieldId::from("total"), number_field("total"))]),
                ),
            ),
        ]),
        entities: BTreeMap::from([
            (
                EntityId::from("rent"),
                plan_row("rent", "Rent", number(12_000.0)),
            ),
            (EntityId::from("ads"), plan_row("ads", "Ads", number(500.0))),
            (
                EntityId::from("summary"),
                Entity {
                    id: EntityId::from("summary"),
                    key: EntityKey::from("summary"),
                    schema: SchemaId::from("summary"),
                    fields: BTreeMap::from([(
                        FieldId::from("total"),
                        Value::Formula(total("plan", "reserve")),
                    )]),
                },
            ),
        ]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

#[test]
fn the_column_total_parses_and_formats_as_written() {
    assert_eq!(
        parse_expression("[plan.reserve:total]").unwrap(),
        UnboundExpression::ColumnTotal(ColumnAddress::new("plan", "reserve"))
    );
    assert_eq!(
        format_unbound_expression(&parse_expression("[plan.reserve:total] + 5").unwrap()),
        "([plan.reserve:total] + 5)"
    );
}

#[test]
fn malformed_column_totals_are_refused() {
    for input in [
        "[.reserve:total]",
        "[plan:total]",
        "[plan.reserve:totals]",
        "[plan.reserve:total:total]",
        "[Plan.reserve:total]",
    ] {
        assert!(parse_expression(input).is_err(), "{input} must be refused");
    }
}

#[test]
fn binding_resolves_the_column_to_stable_ids() {
    let document = document();
    let bound = bind_expression(
        &document,
        &owner("summary", "total"),
        &parse_expression("[plan.reserve:total]").unwrap(),
    )
    .unwrap();
    assert_eq!(bound, total("plan", "reserve"));
}

#[test]
fn binding_refuses_unknown_or_non_numeric_columns() {
    let document = document();
    let unknown_schema = bind_expression(
        &document,
        &owner("summary", "total"),
        &parse_expression("[budget.reserve:total]").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(
        unknown_schema,
        FormulaBindError::Column {
            ref address,
            ..
        } if address == &ColumnAddress::new("budget", "reserve")
    ));

    let unknown_field = bind_expression(
        &document,
        &owner("summary", "total"),
        &parse_expression("[plan.missing:total]").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(
        unknown_field,
        FormulaBindError::Column {
            ref address,
            ..
        } if address == &ColumnAddress::new("plan", "missing")
    ));

    let non_numeric = bind_expression(
        &document,
        &owner("summary", "total"),
        &parse_expression("[plan.item:total]").unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        non_numeric,
        FormulaBindError::NonNumericColumn {
            address: ColumnAddress::new("plan", "item"),
        }
    );
}

#[test]
fn the_dependency_set_covers_every_row_of_the_column() {
    let document = document();
    assert_eq!(
        extract_dependencies(
            &document,
            &owner("summary", "total"),
            &total("plan", "reserve")
        ),
        BTreeSet::from([owner("rent", "reserve"), owner("ads", "reserve")])
    );
}

#[test]
fn projection_round_trips_through_the_current_column_keys() {
    let mut document = document();
    assert_eq!(
        project_expression(
            &document,
            &owner("summary", "total"),
            &total("plan", "reserve")
        )
        .unwrap(),
        "[plan.reserve:total]"
    );

    document.schemas.get_mut("plan").unwrap().key = SchemaKey::from("monthly");
    document
        .schemas
        .get_mut("plan")
        .unwrap()
        .fields
        .get_mut("reserve")
        .unwrap()
        .key = FieldKey::from("buffer");
    assert_eq!(
        project_expression(
            &document,
            &owner("summary", "total"),
            &total("plan", "reserve")
        )
        .unwrap(),
        "[monthly.buffer:total]"
    );
}

#[test]
fn projection_refuses_a_column_that_no_longer_round_trips() {
    let mut document = document();
    document
        .schemas
        .get_mut("plan")
        .unwrap()
        .fields
        .get_mut("reserve")
        .unwrap()
        .field_type = FieldType::Text;
    assert_eq!(
        project_expression(
            &document,
            &owner("summary", "total"),
            &total("plan", "reserve")
        ),
        Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundColumns {
                columns: BTreeSet::from([column("plan", "reserve")]),
            }
        )
    );

    document.schemas.remove("plan");
    assert_eq!(
        project_expression(
            &document,
            &owner("summary", "total"),
            &total("plan", "reserve")
        ),
        Err(
            CanonicalAuthoringProjectionError::UnresolvableBoundColumns {
                columns: BTreeSet::from([column("plan", "reserve")]),
            }
        )
    );
}

#[test]
fn the_total_sums_every_row_of_the_column() {
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document()) else {
        panic!("the fixture must calculate");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(12_500.0).unwrap())
    );
}

#[test]
fn an_empty_table_totals_zero() {
    let mut document = document();
    document.entities.remove("rent");
    document.entities.remove("ads");
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("an empty column totals zero");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(0.0).unwrap())
    );
}

#[test]
fn a_blank_cell_is_skipped_by_the_total() {
    let mut document = document();
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .remove("reserve");
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("a blank cell must not block the total");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(12_000.0).unwrap())
    );
}

#[test]
fn a_direct_reference_to_a_blank_cell_still_fails() {
    let mut document = document();
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .remove("reserve");
    document.entities.get_mut("summary").unwrap().fields.insert(
        FieldId::from("total"),
        Value::Formula(Expression::Add {
            left: Box::new(total("plan", "reserve")),
            right: Box::new(Expression::Reference(owner("ads", "reserve"))),
        }),
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a direct blank reference must not calculate");
    };
    assert_eq!(
        failures.failures().get(&owner("summary", "total")),
        Some(&CalculationFailure::InvalidReferences {
            targets: BTreeMap::from([(owner("ads", "reserve"), ReferenceFailure::Missing)]),
        })
    );
}

#[test]
fn filling_a_blank_cell_recomputes_the_total_incrementally() {
    let mut document = document();
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .remove("reserve");
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .insert(FieldId::from("reserve"), number(700.0));
    let _ = retained.update(&document, &BTreeSet::from([owner("ads", "reserve")]));
    let CalculationOutcome::Complete(calculation) = retained.outcome() else {
        panic!("the retained state must calculate");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(12_700.0).unwrap())
    );
}

#[test]
fn a_text_value_in_the_column_makes_the_total_unavailable() {
    let mut document = document();
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .insert(FieldId::from("reserve"), Value::Text("n/a".to_owned()));
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a non-Number value must not total");
    };
    assert_eq!(
        failures.failures().get(&owner("summary", "total")),
        Some(&CalculationFailure::InvalidReferences {
            targets: BTreeMap::from([(owner("ads", "reserve"), ReferenceFailure::NonNumeric)]),
        })
    );
}

#[test]
fn a_column_that_is_no_longer_numeric_is_a_column_failure() {
    let mut document = document();
    document
        .schemas
        .get_mut("plan")
        .unwrap()
        .fields
        .get_mut("reserve")
        .unwrap()
        .field_type = FieldType::Text;
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a non-numeric column must not total");
    };
    assert_eq!(
        failures.failures().get(&owner("summary", "total")),
        Some(&CalculationFailure::InvalidColumn {
            columns: BTreeMap::from([(column("plan", "reserve"), ColumnFailure::NonNumeric)]),
        })
    );
}

#[test]
fn a_column_total_over_its_own_row_is_a_cycle() {
    let mut document = document();
    document.entities.get_mut("rent").unwrap().fields.insert(
        FieldId::from("reserve"),
        Value::Formula(total("plan", "reserve")),
    );
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("a self-including total must not calculate");
    };
    assert_eq!(
        failures.failures().get(&owner("rent", "reserve")),
        Some(&CalculationFailure::Cycle {
            members: BTreeSet::from([owner("rent", "reserve")]),
        })
    );
}

#[test]
fn a_changed_row_recomputes_the_total_incrementally() {
    let mut document = document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    document
        .entities
        .get_mut("ads")
        .unwrap()
        .fields
        .insert(FieldId::from("reserve"), number(1_000.0));
    let _ = retained.update(&document, &BTreeSet::from([owner("ads", "reserve")]));
    let CalculationOutcome::Complete(calculation) = retained.outcome() else {
        panic!("the retained state must calculate");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(13_000.0).unwrap())
    );
}

fn row_roots(document: &Document, entity: &str) -> BTreeSet<FieldRef> {
    document.entities[entity]
        .fields
        .keys()
        .map(|field| FieldRef::new(entity, field.clone()))
        .collect()
}

#[test]
fn an_added_row_joins_the_total_incrementally() {
    let mut document = document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    document.entities.insert(
        EntityId::from("tools"),
        plan_row("tools", "Tools", number(1_000.0)),
    );
    let _ = retained.update(&document, &row_roots(&document, "tools"));
    assert_eq!(retained.outcome(), calculate_complete(&document));
    let CalculationOutcome::Complete(calculation) = retained.outcome() else {
        panic!("the retained state must calculate");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(13_500.0).unwrap())
    );
}

#[test]
fn a_removed_row_leaves_the_total_incrementally() {
    let mut document = document();
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let roots = row_roots(&document, "ads");
    document.entities.remove("ads");
    let _ = retained.update(&document, &roots);
    assert_eq!(retained.outcome(), calculate_complete(&document));
    let CalculationOutcome::Complete(calculation) = retained.outcome() else {
        panic!("the retained state must calculate");
    };
    assert_eq!(
        calculation.value(&owner("summary", "total")),
        Some(Number::new(12_000.0).unwrap())
    );
}

#[test]
fn a_row_moved_into_the_schema_joins_the_total_incrementally() {
    let mut document = document();
    document.entities.insert(
        EntityId::from("tools"),
        Entity {
            schema: SchemaId::from("summary"),
            fields: BTreeMap::new(),
            ..plan_row("tools", "Tools", number(0.0))
        },
    );
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    let tools = document.entities.get_mut("tools").unwrap();
    tools.schema = SchemaId::from("plan");
    tools.fields.insert(FieldId::from("reserve"), number(250.0));
    let _ = retained.update(&document, &row_roots(&document, "tools"));
    assert_eq!(retained.outcome(), calculate_complete(&document));
}
