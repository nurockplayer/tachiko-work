use std::collections::BTreeMap;

use tachiko_formula_engine::{
    CalculationOutcome, bind_expression, calculate_complete, parse_expression, project_expression,
};
use tachiko_semantic_core::{
    Date, Document, DocumentId, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
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
        date_format: None,
    }
}

fn document() -> Document {
    let mut fields = BTreeMap::new();
    let mut values = BTreeMap::new();
    for (id, date) in [
        ("d", "2026-11-12"),
        ("end", "2026-12-01"),
        ("sunday", "2026-11-15"),
        ("min", "0001-01-01"),
        ("max", "9999-12-31"),
        ("leap_start", "2024-02-28"),
        ("leap_end", "2024-03-01"),
        ("old_start", "1900-02-28"),
        ("old_end", "1900-03-01"),
    ] {
        fields.insert(FieldId::from(id), definition(id, FieldType::Date, true));
        values.insert(FieldId::from(id), Value::Date(Date::parse(date).unwrap()));
    }
    fields.insert(
        FieldId::from("opt"),
        definition("opt", FieldType::Date, false),
    );
    fields.insert(
        FieldId::from("text"),
        definition("text", FieldType::Text, false),
    );
    fields.insert(
        FieldId::from("output"),
        definition("output", FieldType::Number, true),
    );
    values.insert(
        FieldId::from("output"),
        Value::Number(Number::new(0.0).unwrap()),
    );
    let schema = Schema {
        field_order: None,
        id: SchemaId::from("schema"),
        key: SchemaKey::from("row"),
        display_name: None,
        fields,
    };
    let entity = Entity {
        id: EntityId::from("r"),
        key: EntityKey::from("r"),
        schema: SchemaId::from("schema"),
        fields: values,
    };
    Document {
        id: DocumentId::from("date-parts"),
        title: "Date parts".to_owned(),
        schemas: BTreeMap::from([(SchemaId::from("schema"), schema)]),
        entities: BTreeMap::from([(EntityId::from("r"), entity)]),
        keyed_grouped_sum_definitions: BTreeMap::new(),
    }
}

fn calculate(source: &str) -> CalculationOutcome {
    let mut document = document();
    let owner = FieldRef::new("r", "output");
    let expression =
        bind_expression(&document, &owner, &parse_expression(source).unwrap()).unwrap();
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .insert(FieldId::from("output"), Value::Formula(expression));
    calculate_complete(&document)
}

#[test]
fn date_functions_calculate_and_compose() {
    for (source, expected) in [
        ("YEAR([r.d])", 2026.0),
        ("MONTH([r.d])", 11.0),
        ("DAY([r.d])", 12.0),
        ("YEAR([r.min])", 1.0),
        ("MONTH([r.min])", 1.0),
        ("DAY([r.min])", 1.0),
        ("YEAR([r.max])", 9999.0),
        ("MONTH([r.max])", 12.0),
        ("DAY([r.max])", 31.0),
        ("WEEKDAY([r.d])", 5.0),
        ("WEEKDAY([r.d], 2)", 4.0),
        ("WEEKDAY([r.d], 3)", 3.0),
        ("WEEKDAY([r.sunday])", 1.0),
        ("WEEKDAY([r.sunday], 2)", 7.0),
        ("WEEKDAY([r.sunday], 3)", 6.0),
        ("DAYS([r.end], [r.d])", 19.0),
        ("DAYS([r.d], [r.end])", -19.0),
        ("DAYS([r.leap_end], [r.leap_start])", 2.0),
        ("DAYS([r.old_end], [r.old_start])", 1.0),
        ("DAYS([r.end], [r.d]) * 8", 152.0),
        ("IF(MONTH([r.end]) = 12, 1, 0)", 1.0),
    ] {
        let CalculationOutcome::Complete(result) = calculate(source) else {
            panic!("calculation failed: {source}");
        };
        assert_eq!(
            result.value(&FieldRef::new("r", "output")).unwrap().get(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn date_arguments_are_restricted_and_projection_round_trips() {
    let document = document();
    let owner = FieldRef::new("r", "output");
    for source in [
        "YEAR([r.output])",
        "YEAR(1+2)",
        "YEAR([r.text])",
        "WEEKDAY([r.d], 4)",
        "WEEKDAY([r.d], [r.output])",
    ] {
        let expression = parse_expression(source).unwrap();
        assert!(
            bind_expression(&document, &owner, &expression).is_err(),
            "{source}"
        );
    }
    for source in ["YEAR([r.output])", "YEAR(1+2)", "YEAR([r.text])"] {
        let error =
            bind_expression(&document, &owner, &parse_expression(source).unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("YEAR requires a Date field reference")
        );
    }
    for source in [
        "WEEKDAY([r.d], 4)",
        "WEEKDAY([r.d], [r.output])",
        "WEEKDAY([r.d], 11)",
    ] {
        let error =
            bind_expression(&document, &owner, &parse_expression(source).unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("WEEKDAY return_type must be 1, 2 or 3")
        );
    }
    let default = bind_expression(
        &document,
        &owner,
        &parse_expression("WEEKDAY([r.d], 1)").unwrap(),
    )
    .unwrap();
    assert_eq!(
        project_expression(&document, &owner, &default).unwrap(),
        "WEEKDAY([r.d])"
    );
    for source in ["DAYS([r.d])", "DAYS([r.d], [r.end], [r.min])"] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
    for source in [
        "YEAR([r.d])",
        "WEEKDAY([r.d])",
        "WEEKDAY([r.d], 2)",
        "DAYS([r.end], [r.d])",
    ] {
        let expression =
            bind_expression(&document, &owner, &parse_expression(source).unwrap()).unwrap();
        let projected = project_expression(&document, &owner, &expression).unwrap();
        assert_eq!(
            bind_expression(&document, &owner, &parse_expression(&projected).unwrap()).unwrap(),
            expression
        );
    }
}

#[test]
fn optional_blank_date_fails_and_iferror_catches_it() {
    let CalculationOutcome::Failed(failures) = calculate("YEAR([r.opt])") else {
        panic!("blank Date must fail");
    };
    assert!(matches!(
        &failures.failures()[&FieldRef::new("r", "output")],
        tachiko_formula_engine::CalculationFailure::FailedDependencies { dependencies }
            if dependencies.contains(&FieldRef::new("r", "opt"))
    ));
    let CalculationOutcome::Complete(result) = calculate("IFERROR(YEAR([r.opt]), 0)") else {
        panic!("IFERROR did not catch the missing date");
    };
    assert_eq!(
        result.value(&FieldRef::new("r", "output")).unwrap().get(),
        0.0
    );
}

#[test]
fn same_row_date_edits_recalculate_through_retained_state() {
    use std::collections::BTreeSet;
    use tachiko_formula_engine::RetainedCalculationState;

    let mut document = document();
    let owner = FieldRef::new("r", "output");
    let formula =
        bind_expression(&document, &owner, &parse_expression("YEAR([.d])").unwrap()).unwrap();
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .insert("output".into(), Value::Formula(formula));
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .insert("d".into(), Value::Date(Date::new(2027, 1, 1).unwrap()));
    let _ = retained.update(&document, &BTreeSet::from([FieldRef::new("r", "d")]));
    let full = calculate_complete(&document);
    assert_eq!(retained.outcome(), full);
    let CalculationOutcome::Complete(calculation) = full else {
        panic!("date update failed");
    };
    assert_eq!(calculation.value(&owner).unwrap().get(), 2027.0);
    assert!(calculation.dependencies()[&owner].contains(&FieldRef::new("r", "d")));
}
