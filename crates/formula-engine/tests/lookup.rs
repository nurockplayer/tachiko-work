use std::collections::BTreeSet;

use tachiko_formula_engine::{
    CalculationFailure, CalculationOutcome, RetainedCalculationState, bind_expression,
    calculate_complete, format_unbound_expression, parse_expression, project_expression,
};
use tachiko_semantic_core::{
    Date, Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldRef, FieldType,
    Number, Schema, Value, validate_document,
};

fn number(value: f64) -> Number {
    Number::new(value).unwrap()
}

fn field(id: &str, field_type: FieldType) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format: None,
    }
}

fn row(id: &str, schema: &str, values: Vec<(&str, Value)>) -> Entity {
    Entity {
        id: id.into(),
        key: id.into(),
        schema: schema.into(),
        fields: values
            .into_iter()
            .map(|(field, value)| (field.into(), value))
            .collect(),
    }
}

fn document() -> Document {
    let mut document = Document::empty("lookup", "Lookup");
    for (id, definitions) in [
        (
            "t",
            vec![
                ("k", FieldType::Text),
                ("n", FieldType::Number),
                ("d", FieldType::Date),
                ("b", FieldType::Boolean),
                ("v", FieldType::Number),
                ("computed", FieldType::Number),
            ],
        ),
        (
            "s",
            vec![
                ("text", FieldType::Text),
                ("number", FieldType::Number),
                ("date", FieldType::Date),
                ("boolean", FieldType::Boolean),
                ("out", FieldType::Number),
                ("fallback", FieldType::Number),
            ],
        ),
    ] {
        document.schemas.insert(
            id.into(),
            Schema {
                id: id.into(),
                key: id.into(),
                display_name: None,
                field_order: None,
                fields: definitions
                    .into_iter()
                    .map(|(id, kind)| (id.into(), field(id, kind)))
                    .collect(),
            },
        );
    }
    for (id, value) in [("a", 10.0), ("b", 20.0)] {
        document.entities.insert(
            id.into(),
            row(
                id,
                "t",
                vec![
                    ("k", Value::Text("Apple".into())),
                    ("n", Value::Number(number(1.0))),
                    ("d", Value::Date(Date::parse("2026-11-12").unwrap())),
                    ("b", Value::Boolean(true)),
                    ("v", Value::Number(number(value))),
                    (
                        "computed",
                        Value::Formula(Expression::Multiply {
                            left: Box::new(Expression::SameRowReference("v".into())),
                            right: Box::new(Expression::Number(number(2.0))),
                        }),
                    ),
                ],
            ),
        );
    }
    document.entities.insert(
        "s".into(),
        row(
            "s",
            "s",
            vec![
                ("text", Value::Text("apple".into())),
                ("number", Value::Number(number(1.0))),
                ("date", Value::Date(Date::parse("2026-11-12").unwrap())),
                ("boolean", Value::Boolean(true)),
                ("fallback", Value::Number(number(7.0))),
            ],
        ),
    );
    document
}

fn owner() -> FieldRef {
    FieldRef::new("s", "out")
}

fn bind(document: &Document, source: &str) -> Expression {
    bind_expression(document, &owner(), &parse_expression(source).unwrap()).unwrap()
}

fn install(document: &mut Document, source: &str) {
    let expression = bind(document, source);
    document
        .entities
        .get_mut("s")
        .unwrap()
        .fields
        .insert("out".into(), Value::Formula(expression));
}

fn result(document: &Document) -> Number {
    let diagnostics = validate_document(document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let CalculationOutcome::Complete(calculation) = calculate_complete(document) else {
        panic!("lookup must succeed")
    };
    calculation.values()[&owner()]
}

fn failure(document: &Document) -> CalculationFailure {
    let CalculationOutcome::Failed(failures) = calculate_complete(document) else {
        panic!("lookup must fail")
    };
    failures.failures()[&owner()].clone()
}

#[test]
fn text_keys_fold_case_and_duplicate_keys_choose_first_entity() {
    let mut doc = document();
    install(&mut doc, "LOOKUP([s.text], [t.k:column], [t.v:column])");
    assert_eq!(result(&doc), number(10.0));
    for text in ["*", "?", ">5", "1", "ÄPFEL"] {
        doc.entities
            .get_mut("a")
            .unwrap()
            .fields
            .insert("k".into(), Value::Text(text.into()));
        doc.entities
            .get_mut("s")
            .unwrap()
            .fields
            .insert("text".into(), Value::Text(text.to_lowercase()));
        assert_eq!(result(&doc), number(10.0));
    }
    doc.entities
        .get_mut("s")
        .unwrap()
        .fields
        .insert("text".into(), Value::Text("App*".into()));
    assert_eq!(failure(&doc), CalculationFailure::LookupMissingKey);
}

#[test]
fn typed_keys_and_number_expressions_match_without_text_coercion() {
    let mut doc = document();
    for (key, column) in [
        ("1 + 0", "n"),
        ("[s.number]", "n"),
        ("[s.date]", "d"),
        ("[s.boolean]", "b"),
    ] {
        install(
            &mut doc,
            &format!("LOOKUP({key}, [t.{column}:column], [t.v:column])"),
        );
        assert_eq!(result(&doc), number(10.0));
    }
    doc.entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("n".into(), Value::Number(number(-0.0)));
    install(&mut doc, "LOOKUP(0, [t.n:column], [t.v:column])");
    assert_eq!(result(&doc), number(10.0));
    for source in [
        "LOOKUP(1, [t.k:column], [t.v:column])",
        "LOOKUP([s.text], [t.n:column], [t.v:column])",
        "LOOKUP([s.boolean], [t.d:column], [t.v:column])",
        "LOOKUP(1, [t.n:column], [s.number:column])",
        "LOOKUP(1, [t.n:column], [t.k:column])",
        "LOOKUP(1, [t.n:column], [t.v:column], [s.text])",
    ] {
        assert!(
            bind_expression(&doc, &owner(), &parse_expression(source).unwrap()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn missing_key_and_lazy_fallback() {
    let mut doc = document();
    install(&mut doc, "LOOKUP(99, [t.n:column], [t.v:column])");
    assert_eq!(failure(&doc), CalculationFailure::LookupMissingKey);
    install(
        &mut doc,
        "IFERROR(LOOKUP(99, [t.n:column], [t.v:column]), 7)",
    );
    assert_eq!(result(&doc), number(7.0));
    install(
        &mut doc,
        "LOOKUP(99, [t.n:column], [t.v:column], [s.fallback])",
    );
    assert_eq!(result(&doc), number(7.0));
    install(&mut doc, "LOOKUP(1, [t.n:column], [t.v:column], 1 / 0)");
    assert_eq!(result(&doc), number(10.0));
    install(&mut doc, "LOOKUP(99, [t.n:column], [t.v:column], 1 / 0)");
    assert_eq!(failure(&doc), CalculationFailure::DivisionByZero);
    doc.entities.get_mut("s").unwrap().fields.remove("fallback");
    install(
        &mut doc,
        "LOOKUP(1, [t.n:column], [t.v:column], [s.fallback])",
    );
    assert_eq!(result(&doc), number(10.0));
}

#[test]
fn formula_result_and_optional_blank_result() {
    let mut doc = document();
    install(
        &mut doc,
        "LOOKUP([s.text], [t.k:column], [t.computed:column])",
    );
    assert_eq!(result(&doc), number(20.0));
    doc.entities.get_mut("a").unwrap().fields.remove("computed");
    assert_eq!(result(&doc), number(0.0));
}

#[test]
fn blank_keys_never_match_and_blank_key_values_are_unavailable() {
    let mut doc = document();
    doc.entities.get_mut("a").unwrap().fields.remove("n");
    install(&mut doc, "LOOKUP(0, [t.n:column], [t.v:column], 7)");
    assert_eq!(result(&doc), number(7.0));
    doc.entities.get_mut("s").unwrap().fields.remove("number");
    install(
        &mut doc,
        "LOOKUP([s.number], [t.n:column], [t.v:column], 7)",
    );
    assert_eq!(
        failure(&doc),
        CalculationFailure::FailedDependencies {
            dependencies: BTreeSet::from([FieldRef::new("s", "number")])
        }
    );
}

#[test]
fn failed_cells_in_either_column_fail_even_when_the_row_does_not_match() {
    for field in ["n", "v"] {
        let mut doc = document();
        doc.entities.get_mut("b").unwrap().fields.insert(
            field.into(),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Number(number(1.0))),
                right: Box::new(Expression::Number(number(0.0))),
            }),
        );
        install(&mut doc, "LOOKUP(1, [t.n:column], [t.v:column], 7)");
        assert_eq!(
            failure(&doc),
            CalculationFailure::FailedDependencies {
                dependencies: BTreeSet::from([FieldRef::new("b", field)])
            }
        );
    }
}

#[test]
fn parse_project_round_trip_and_dependencies() {
    let doc = document();
    for source in [
        "LOOKUP([s.text], [t.k:column], [t.v:column])",
        "LOOKUP([s.text], [t.k:column], [t.v:column], [s.fallback])",
    ] {
        let parsed = parse_expression(source).unwrap();
        assert_eq!(format_unbound_expression(&parsed), source);
        let bound = bind_expression(&doc, &owner(), &parsed).unwrap();
        assert_eq!(project_expression(&doc, &owner(), &bound).unwrap(), source);
        let deps = tachiko_formula_engine::extract_dependencies(&doc, &owner(), &bound);
        for reference in [
            FieldRef::new("s", "text"),
            FieldRef::new("a", "k"),
            FieldRef::new("a", "v"),
            FieldRef::new("b", "k"),
            FieldRef::new("b", "v"),
        ] {
            assert!(deps.contains(&reference));
        }
        assert_eq!(
            deps.contains(&FieldRef::new("s", "fallback")),
            source.contains("fallback")
        );
    }
}

fn update(
    retained: &mut RetainedCalculationState,
    document: &Document,
    roots: &BTreeSet<FieldRef>,
    expected: f64,
) {
    let _ = retained.update(document, roots);
    assert_eq!(
        retained.outcome(),
        calculate_complete(document),
        "incremental calculation must equal a full one"
    );
    let CalculationOutcome::Complete(calculation) = retained.outcome() else {
        panic!("lookup succeeds after edit")
    };
    assert_eq!(calculation.values()[&owner()], number(expected));
}

#[test]
fn retained_key_result_and_membership_edits_equal_full_rebuilds() {
    let mut doc = document();
    install(&mut doc, "LOOKUP([s.text], [t.k:column], [t.v:column])");
    let (mut retained, _) = RetainedCalculationState::rebuild(&doc);
    doc.entities
        .get_mut("a")
        .unwrap()
        .fields
        .insert("k".into(), Value::Text("pear".into()));
    update(
        &mut retained,
        &doc,
        &BTreeSet::from([FieldRef::new("a", "k")]),
        20.0,
    );
    doc.entities
        .get_mut("b")
        .unwrap()
        .fields
        .insert("v".into(), Value::Number(number(30.0)));
    update(
        &mut retained,
        &doc,
        &BTreeSet::from([FieldRef::new("b", "v")]),
        30.0,
    );
    // An inserted row's stable ID sorts first in the entity order used by totals.
    doc.entities.insert(
        "a0".into(),
        row(
            "a0",
            "t",
            vec![
                ("k", Value::Text("APPLE".into())),
                ("v", Value::Number(number(5.0))),
            ],
        ),
    );
    update(
        &mut retained,
        &doc,
        &BTreeSet::from([FieldRef::new("a0", "k"), FieldRef::new("a0", "v")]),
        5.0,
    );
}

#[test]
fn lookup_requires_exactly_two_column_arguments_and_at_most_one_fallback() {
    for source in [
        "LOOKUP(1, [t.n:column])",
        "LOOKUP(1, [t.n], [t.v:column])",
        "LOOKUP(1, [t.n:column], [t.v:column], 0, 1)",
        "LOOKUP(1, [t.n:column], [t.v:column],)",
        "[t.n:column]",
    ] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
}
