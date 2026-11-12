use std::collections::BTreeMap;

use tachiko_formula_engine::{
    CalculationOutcome, bind_expression, calculate_complete, parse_expression, project_expression,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, Expression, FieldConstraint,
    FieldDefinition, FieldId, FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey,
    Value,
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
        field_order: None,
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
fn absolute_and_directed_rounding_calculate_on_displayed_decimals() {
    for (source, expected) in [
        ("ABS(-0)", 0.0),
        ("ABS(-2.5)", 2.5),
        ("abs(-2.5)", 2.5),
        ("roundup(-1.21, 1)", -1.3),
        ("rounddown(-1.29, 1)", -1.2),
        ("ABS([row.input])", 2.675),
        ("ABS(1 - 4)", 3.0),
        ("ROUNDUP(1.1, 1)", 1.1),
        ("ROUNDDOWN(2.3, 1)", 2.3),
        ("ROUNDUP(-1.21, 1)", -1.3),
        ("ROUNDDOWN(-1.29, 1)", -1.2),
        ("ROUNDUP(5, 0)", 5.0),
        ("ROUNDUP(ABS([row.input]) * 1.1, 2)", 2.95),
    ] {
        let mut document = document();
        set_formula(&mut document, "output", source);
        assert_eq!(
            number_at(&calculate_complete(&document), "output"),
            expected,
            "{source}"
        );
    }
    let mut document = document();
    set_formula(&mut document, "output", "ABS(-0)");
    assert_eq!(
        number_at(&calculate_complete(&document), "output").to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn function_validation_and_projection() {
    for source in ["ABS()", "ABS(1, 2)", "ROUNDUP(1)", "ROUNDDOWN(1, 2, 3)"] {
        assert!(
            parse_expression(source)
                .unwrap_err()
                .message
                .contains("arity"),
            "{source}"
        );
    }
    let document = document();
    for (name, source) in [
        ("ROUNDUP", "ROUNDUP(1, [row.input])"),
        ("ROUNDUP", "ROUNDUP(1, 11)"),
        ("ROUNDDOWN", "ROUNDDOWN(1, 1.5)"),
        ("ROUNDDOWN", "ROUNDDOWN(1, -1)"),
    ] {
        let expression = parse_expression(source).unwrap();
        let error = bind_expression(&document, &owner("output"), &expression).unwrap_err();
        assert!(
            error.to_string().contains(&format!(
                "{name} digits must be an integer literal in 0..=10"
            )),
            "{error}"
        );
    }
    let error = parse_expression("UNKNOWN(1)").unwrap_err().message;
    for name in ["ABS", "ROUNDUP", "ROUNDDOWN"] {
        assert!(error.contains(name), "{error}");
    }
    for source in [
        "ABS([row.input])",
        "ROUNDUP(1.1, 1)",
        "ROUNDDOWN(-1.29, 1)",
        "ROUNDUP(ABS([row.input]) * 1.1, 2)",
    ] {
        let expression = bind_expression(
            &document,
            &owner("output"),
            &parse_expression(source).unwrap(),
        )
        .unwrap();
        let projected = project_expression(&document, &owner("output"), &expression).unwrap();
        let reopened = bind_expression(
            &document,
            &owner("output"),
            &parse_expression(&projected).unwrap(),
        )
        .unwrap();
        assert_eq!(
            project_expression(&document, &owner("output"), &reopened).unwrap(),
            projected
        );
    }
}
