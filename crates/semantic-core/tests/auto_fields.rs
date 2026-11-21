use std::collections::BTreeMap;
use tachiko_semantic_core::{
    ClassifiedInput, Date, DateOrder, Document, Entity, Expression, FieldConstraint,
    FieldDefinition, FieldType, Number, NumberFormat, NumberFormatKind, Schema, Value,
    classify_input, validate_document_core,
};

fn auto_document(value: Option<Value>) -> Document {
    let mut document = Document::empty("auto-doc", "Auto");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            field_order: None,
            fields: BTreeMap::from([(
                "a".into(),
                FieldDefinition {
                    id: "a".into(),
                    key: "a".into(),
                    display_name: None,
                    field_type: FieldType::Auto,
                    required: false,
                    constraint: FieldConstraint::None,
                    number_format: None,
                    date_format: None,
                },
            )]),
        },
    );
    document.entities.insert(
        "r".into(),
        Entity {
            id: "r".into(),
            key: "r".into(),
            schema: "s".into(),
            rank: None,
            fields: value
                .map(|value| BTreeMap::from([("a".into(), value)]))
                .unwrap_or_default(),
        },
    );
    document
}

#[test]
fn auto_admits_classifier_scalar_tags_losslessly() {
    for (input, expected) in [
        ("42", Value::Number(Number::new(42.0).unwrap())),
        ("00123", Value::Text("00123".into())),
        (
            "2026-11-22",
            Value::Date(Date::parse("2026-11-22").unwrap()),
        ),
        ("TRUE", Value::Boolean(true)),
        ("hello", Value::Text("hello".into())),
    ] {
        let value = match classify_input(input, None) {
            ClassifiedInput::Number { value, .. } => Value::Number(value),
            ClassifiedInput::Text(value) => Value::Text(value),
            ClassifiedInput::Date { value, .. } => Value::Date(value),
            ClassifiedInput::Boolean(value) => Value::Boolean(value),
            other => panic!("unexpected input {other:?}"),
        };
        assert_eq!(value, expected);
        assert_eq!(validate_document_core(&auto_document(Some(value))), []);
    }
}

#[test]
fn auto_day_first_date_is_explicit() {
    assert!(
        matches!(classify_input("22/11/2026", Some(DateOrder::DayFirst)),
        ClassifiedInput::Date { value, .. } if value == Date::parse("2026-11-22").unwrap())
    );
    assert_eq!(
        classify_input("22/11/2026", None),
        ClassifiedInput::Text("22/11/2026".into())
    );
}

#[test]
fn auto_empty_text_and_absence_are_distinct_valid_states() {
    let text = auto_document(Some(Value::Text(String::new())));
    let blank = auto_document(None);
    assert_ne!(text, blank);
    for state in [text, blank] {
        assert_eq!(validate_document_core(&state), []);
    }
    assert_eq!(classify_input("", None), ClassifiedInput::Blank);
}

#[test]
fn auto_required_constraint_and_reference_are_refused() {
    let mut required = auto_document(None);
    required
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("a")
        .unwrap()
        .required = true;
    let mut constrained = auto_document(None);
    constrained
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("a")
        .unwrap()
        .constraint = FieldConstraint::TextLiteralSet {
        values: vec!["x".into()],
    };
    for state in [
        required,
        constrained,
        auto_document(Some(Value::Reference("r".into()))),
    ] {
        assert_ne!(validate_document_core(&state), []);
    }
}

#[test]
fn auto_formula_is_explicitly_unsupported() {
    let formula = Value::Formula(Expression::Number(Number::new(1.0).unwrap()));
    let diagnostics = validate_document_core(&auto_document(Some(formula)));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Auto formulas are unsupported"))
    );
}

#[test]
fn auto_admits_number_display_but_declared_types_stay_strict() {
    let format = NumberFormat {
        decimals: 2,
        kind: NumberFormatKind::Plain,
        thousands_separator: true,
    };
    assert!(format.validate_for(&FieldType::Auto).is_ok());
    for kind in [
        FieldType::Number,
        FieldType::Text,
        FieldType::Boolean,
        FieldType::Date,
    ] {
        let mut state = auto_document(Some(Value::Number(Number::new(42.0).unwrap())));
        state
            .schemas
            .get_mut("s")
            .unwrap()
            .fields
            .get_mut("a")
            .unwrap()
            .field_type = kind.clone();
        assert_eq!(
            validate_document_core(&state).is_empty(),
            kind == FieldType::Number
        );
    }
}
