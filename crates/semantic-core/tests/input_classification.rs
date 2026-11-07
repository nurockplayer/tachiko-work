use tachiko_semantic_core::{
    ClassifiedDateDisplay, ClassifiedInput, Date, DateOrder, Number, NumberFormat,
    NumberFormatKind, classify_input,
};

fn number(value: f64) -> Number {
    Number::new(value).expect("test value is finite")
}

fn display(decimals: u8, kind: NumberFormatKind, thousands_separator: bool) -> NumberFormat {
    NumberFormat {
        decimals,
        kind,
        thousands_separator,
    }
}

fn date(year: u16, month: u8, day: u8) -> Date {
    Date::new(year, month, day).expect("test date is valid")
}

fn check(input: &str, order: Option<DateOrder>, expected: &ClassifiedInput) {
    assert_eq!(&classify_input(input, order), expected, "input {input:?}");
}

#[test]
fn classifies_blank_formula_boolean_and_text_rows() {
    check("", None, &ClassifiedInput::Blank);
    check("   ", None, &ClassifiedInput::Text("   ".into()));
    check("hello", None, &ClassifiedInput::Text("hello".into()));
    check(
        "=A1+1",
        None,
        &ClassifiedInput::FormulaSource("=A1+1".into()),
    );
    check("=", None, &ClassifiedInput::FormulaSource("=".into()));
    check("TRUE", None, &ClassifiedInput::Boolean(true));
    check("false", None, &ClassifiedInput::Boolean(false));
}

#[test]
fn classifies_numeric_rows() {
    for (input, value) in [
        ("0.5", 0.5),
        (".5", 0.5),
        ("5.", 5.0),
        ("+5", 5.0),
        (" 12 ", 12.0),
        ("1e3", 1_000.0),
        ("123456789012345", 123_456_789_012_345.0),
    ] {
        check(
            input,
            None,
            &ClassifiedInput::Number {
                value: number(value),
                display: None,
            },
        );
    }
    for input in [
        "00123",
        "007.5",
        "1e309",
        "1234567890123456",
        "1,00",
        "1,0000",
        "１２３ｘ",
    ] {
        check(input, None, &ClassifiedInput::Text(input.into()));
    }
}

#[test]
fn numeric_values_keep_their_bits() {
    for (input, bits) in [
        ("0.5", 0.5_f64.to_bits()),
        (".5", 0.5_f64.to_bits()),
        ("5.", 5.0_f64.to_bits()),
        ("+5", 5.0_f64.to_bits()),
        (" 12 ", 12.0_f64.to_bits()),
        ("1e3", 1_000.0_f64.to_bits()),
        ("123456789012345", 123_456_789_012_345.0_f64.to_bits()),
        ("1,000", 1_000.0_f64.to_bits()),
        ("1,234,567.5", 1_234_567.5_f64.to_bits()),
        ("$1,234.50", 1_234.5_f64.to_bits()),
        ("-$5", (-5.0_f64).to_bits()),
        ("$-5", (-5.0_f64).to_bits()),
        ("50%", 0.5_f64.to_bits()),
        ("0.07%", 0.0007_f64.to_bits()),
        ("1e310%", 1e308_f64.to_bits()),
        ("1,000%", 10.0_f64.to_bits()),
        ("１２．５％", 0.125_f64.to_bits()),
        ("$1.12345678901", 1.123_456_789_01_f64.to_bits()),
    ] {
        match classify_input(input, None) {
            ClassifiedInput::Number { value, .. } => assert_eq!(
                value.to_bits(),
                bits,
                "input {input:?} value should keep its exact bits"
            ),
            other => panic!("input {input:?} should be a Number, got {other:?}"),
        }
    }
}

#[test]
fn classifies_grouped_numbers_with_thousands_display() {
    check(
        "1,000",
        None,
        &ClassifiedInput::Number {
            value: number(1_000.0),
            display: Some(display(0, NumberFormatKind::Plain, true)),
        },
    );
    check(
        "1,234,567.5",
        None,
        &ClassifiedInput::Number {
            value: number(1_234_567.5),
            display: Some(display(1, NumberFormatKind::Plain, true)),
        },
    );
}

#[test]
fn classifies_currency_rows() {
    for (input, value, decimals, code, thousands) in [
        ("$1,234.50", 1_234.5, 2, "USD", true),
        ("-$5", -5.0, 0, "USD", false),
        ("$-5", -5.0, 0, "USD", false),
        ("NT$1,200", 1_200.0, 0, "TWD", true),
        ("¥1200", 1_200.0, 0, "JPY", true),
        ("￥１，２００", 1_200.0, 0, "JPY", true),
        ("€5", 5.0, 0, "EUR", false),
        ("£5", 5.0, 0, "GBP", false),
    ] {
        check(
            input,
            None,
            &ClassifiedInput::Number {
                value: number(value),
                display: Some(display(
                    decimals,
                    NumberFormatKind::Currency { code: code.into() },
                    thousands,
                )),
            },
        );
    }
    check(
        "$1.12345678901",
        None,
        &ClassifiedInput::Number {
            value: number(1.123_456_789_01),
            display: Some(display(
                10,
                NumberFormatKind::Currency { code: "USD".into() },
                false,
            )),
        },
    );
}

#[test]
fn classifies_percent_rows() {
    check(
        "50%",
        None,
        &ClassifiedInput::Number {
            value: number(0.5),
            display: Some(display(0, NumberFormatKind::Percent, false)),
        },
    );
    check(
        "１２．５％",
        None,
        &ClassifiedInput::Number {
            value: number(0.125),
            display: Some(display(1, NumberFormatKind::Percent, false)),
        },
    );
}

#[test]
fn classifies_dates_and_date_orders() {
    check(
        "2026-11-07",
        None,
        &ClassifiedInput::Date {
            value: date(2026, 11, 7),
            display: None,
        },
    );
    check(
        "2026/11/7",
        None,
        &ClassifiedInput::Date {
            value: date(2026, 11, 7),
            display: Some(ClassifiedDateDisplay::YmdSlash),
        },
    );
    check(
        "２０２６年１１月７日",
        None,
        &ClassifiedInput::Date {
            value: date(2026, 11, 7),
            display: Some(ClassifiedDateDisplay::YmdCjk),
        },
    );
    check(
        "2026/2/30",
        None,
        &ClassifiedInput::Text("2026/2/30".into()),
    );
    check("1/2/2026", None, &ClassifiedInput::Text("1/2/2026".into()));
    check(
        "1/2/2026",
        Some(DateOrder::MonthFirst),
        &ClassifiedInput::Date {
            value: date(2026, 1, 2),
            display: Some(ClassifiedDateDisplay::MonthFirst),
        },
    );
    check(
        "1/2/2026",
        Some(DateOrder::DayFirst),
        &ClassifiedInput::Date {
            value: date(2026, 2, 1),
            display: Some(ClassifiedDateDisplay::DayFirst),
        },
    );
}

#[test]
fn input_that_would_lose_its_meaning_stays_text() {
    for input in [
        "$--5", "$-+5", "$+-5", "1e-400", "1e-400%", "$1e-400", "1e-323%",
    ] {
        assert!(
            matches!(classify_input(input, None), ClassifiedInput::Text(text) if text == input),
            "{input} stays exact Text"
        );
    }
    assert!(matches!(
        classify_input("0e-400", None),
        ClassifiedInput::Number { .. }
    ));
}
