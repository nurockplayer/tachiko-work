use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{
    CalculationFailure, CalculationOutcome, RetainedCalculationState, bind_expression,
    calculate_complete, parse_expression, project_expression,
};
use tachiko_semantic_core::{
    Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

fn definition(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: FieldId::from(id),
        key: FieldKey::from(id),
        display_name: None,
        field_type,
        required: true,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format: None,
    }
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn document() -> Document {
    let fields = ["a", "b", "flag", "z", "blank", "output"]
        .into_iter()
        .map(|id| (FieldId::from(id), definition(id, FieldType::Number)))
        .chain([(FieldId::from("note"), definition("note", FieldType::Text))])
        .collect();
    let mut document = Document::empty("iferror", "IFERROR");
    document.schemas.insert(
        SchemaId::from("r"),
        Schema {
            field_order: None,
            id: SchemaId::from("r"),
            key: SchemaKey::from("r"),
            display_name: None,
            fields,
        },
    );
    document.schemas.insert(
        SchemaId::from("t"),
        Schema {
            field_order: None,
            id: SchemaId::from("t"),
            key: SchemaKey::from("t"),
            display_name: None,
            fields: BTreeMap::from([(FieldId::from("x"), definition("x", FieldType::Number))]),
        },
    );
    document.entities.insert(
        EntityId::from("r"),
        Entity {
            id: EntityId::from("r"),
            key: EntityKey::from("r"),
            schema: SchemaId::from("r"),
            fields: BTreeMap::from([
                (FieldId::from("a"), number(8.0)),
                (FieldId::from("b"), number(2.0)),
                (FieldId::from("flag"), number(1.0)),
                (FieldId::from("z"), number(0.0)),
                (FieldId::from("blank"), number(0.0)),
                (FieldId::from("output"), number(0.0)),
                (
                    FieldId::from("note"),
                    Value::Text(">invalid criterion".to_owned()),
                ),
            ]),
        },
    );
    document
}

fn owner() -> FieldRef {
    FieldRef::new("r", "output")
}

fn set_formula(document: &mut Document, source: &str) -> Expression {
    let expression =
        bind_expression(document, &owner(), &parse_expression(source).unwrap()).unwrap();
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .insert(FieldId::from("output"), Value::Formula(expression.clone()));
    expression
}

fn value(outcome: &CalculationOutcome) -> f64 {
    let CalculationOutcome::Complete(calculation) = outcome else {
        panic!("expected successful IFERROR: {outcome:?}");
    };
    calculation.value(&owner()).unwrap().get()
}

#[test]
fn own_runtime_failures_are_caught_and_fallback_is_lazy() {
    for (source, expected) in [
        ("IFERROR([r.a] / [r.z], 0)", 0.0),
        ("IFERROR([r.a] / [r.b], 1 / 0)", 4.0),
        ("iferror(1, 1 / 0)", 1.0),
        ("IFERROR([t.x:average], -1)", -1.0),
        ("IFERROR(1e308 * 1e308, 7)", 7.0),
        ("IFERROR(COUNTIF([t.x:column], [r.note]), 8)", 8.0),
        ("IFERROR(IFERROR(1 / 0, 2 / 0), 5)", 5.0),
        ("IF([r.flag] > 0, IFERROR(1 / [r.z], 9), 0)", 9.0),
        ("IFERROR(1, [r.blank])", 1.0),
        ("IFERROR(IF([r.flag] > 0, [r.blank], 2), 9)", 9.0),
    ] {
        let mut document = document();
        if source.contains("[r.blank]") {
            document
                .entities
                .get_mut("r")
                .unwrap()
                .fields
                .remove("blank");
        }
        set_formula(&mut document, source);
        assert_eq!(value(&calculate_complete(&document)), expected, "{source}");
    }
}

#[test]
fn fallback_failure_propagates() {
    let mut document = document();
    set_formula(&mut document, "IFERROR(1 / 0, 2 / 0)");
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("fallback must fail");
    };
    assert_eq!(
        failures.failures()[&owner()],
        CalculationFailure::DivisionByZero
    );
}

#[test]
fn binding_structure_and_projection_include_both_operands() {
    let document = document();
    for source in [
        "IFERROR([r.missing], 0)",
        "IFERROR(1, [r.missing])",
        "IFERROR([r.note], 0)",
        "IFERROR(1, [r.note])",
        "IFERROR(1 > 0, 0)",
        "IFERROR(1, 1 > 0)",
    ] {
        assert!(bind_expression(&document, &owner(), &parse_expression(source).unwrap()).is_err());
    }
    for source in ["IFERROR(1)", "IFERROR()", "IFERROR(1, 2, 3)"] {
        assert!(
            parse_expression(source)
                .unwrap_err()
                .message
                .contains("arity")
        );
    }
    assert!(
        parse_expression("UNKNOWN(1)")
            .unwrap_err()
            .message
            .contains("IFERROR")
    );
    let expression = bind_expression(
        &document,
        &owner(),
        &parse_expression("iferror([r.a]/[r.b], [r.z])").unwrap(),
    )
    .unwrap();
    let projected = project_expression(&document, &owner(), &expression).unwrap();
    assert!(projected.starts_with("IFERROR("), "{projected}");
    assert_eq!(
        bind_expression(&document, &owner(), &parse_expression(&projected).unwrap()).unwrap(),
        expression
    );
    assert_eq!(expression.children().len(), 2);
    assert_eq!(
        expression.unconditional_references(&owner()),
        BTreeSet::from([FieldRef::new("r", "a"), FieldRef::new("r", "b")])
    );
}

#[test]
fn upstream_failures_and_unconditional_required_blanks_are_not_caught() {
    let mut document = document();
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .remove("blank");
    set_formula(&mut document, "IFERROR([r.blank], 0)");
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("the unconditional blank precheck must fail");
    };
    assert!(matches!(
        failures.failures()[&owner()],
        CalculationFailure::InvalidReferences { .. }
    ));

    let upstream = bind_expression(
        &document,
        &FieldRef::new("r", "a"),
        &parse_expression("1 / 0").unwrap(),
    )
    .unwrap();
    document
        .entities
        .get_mut("r")
        .unwrap()
        .fields
        .insert(FieldId::from("a"), Value::Formula(upstream));
    for source in [
        "IFERROR([r.a], 0)",
        "IFERROR([r.a:total], 0)",
        "IFERROR(COUNTIF([r.a:column], 0), 0)",
    ] {
        set_formula(&mut document, source);
        let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
            panic!("upstream failure precheck must fail");
        };
        assert!(
            matches!(
                failures.failures()[&owner()],
                CalculationFailure::FailedDependencies { .. }
            ),
            "{source}"
        );
    }
    set_formula(&mut document, "IFERROR(1, [r.a])");
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("the independent upstream failure remains");
    };
    assert!(!failures.failures().contains_key(&owner()));
    assert_eq!(
        failures.failures()[&FieldRef::new("r", "a")],
        CalculationFailure::DivisionByZero
    );
}

#[test]
fn retained_edits_switch_between_value_and_fallback_like_a_full_rebuild() {
    let mut document = document();
    set_formula(&mut document, "IFERROR([r.a] / [r.b], 0)");
    let (mut retained, _) = RetainedCalculationState::rebuild(&document);
    for denominator in [0.0, 2.0] {
        document
            .entities
            .get_mut("r")
            .unwrap()
            .fields
            .insert(FieldId::from("b"), number(denominator));
        let _ = retained.update(&document, &BTreeSet::from([FieldRef::new("r", "b")]));
        let full = calculate_complete(&document);
        assert_eq!(value(&retained.outcome()), value(&full));
        assert_eq!(value(&full), if denominator == 0.0 { 0.0 } else { 4.0 });
        let CalculationOutcome::Complete(calculation) = full else {
            unreachable!()
        };
        assert_eq!(retained.dependencies(), calculation.dependencies());
        assert!(calculation.dependencies()[&owner()].contains(&FieldRef::new("r", "b")));
    }
}

#[test]
fn unused_fallback_keeps_dependencies_and_cycle_checks() {
    let mut document = document();
    set_formula(&mut document, "IFERROR(1, [r.b])");
    let CalculationOutcome::Complete(calculation) = calculate_complete(&document) else {
        panic!("unused numeric fallback succeeds");
    };
    assert!(calculation.dependencies()[&owner()].contains(&FieldRef::new("r", "b")));
    set_formula(&mut document, "IFERROR(1, [r.output])");
    let CalculationOutcome::Failed(failures) = calculate_complete(&document) else {
        panic!("unused fallback must retain its cycle");
    };
    assert!(matches!(
        failures.failures()[&owner()],
        CalculationFailure::Cycle { .. }
    ));
}
