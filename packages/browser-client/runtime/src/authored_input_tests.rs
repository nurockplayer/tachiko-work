use super::{
    Date, DesignerRuntime, NewTableColumnInput, Number, RowInitializer, ScalarEditInput,
    StoredValueProjection, authored_date, authored_number,
};

#[test]
fn authored_numbers_accept_grouping_and_percent_and_refuse_bad_groups() {
    for (input, expected) in [
        ("1300", 1300.0),
        (" 1,300 ", 1300.0),
        ("1,234,567.25", 1_234_567.25),
        ("-1,300.5", -1300.5),
        ("+12", 12.0),
        ("12%", 0.12),
        ("12.5 %", 0.125),
        ("-1,000%", -10.0),
        ("1e3", 1000.0),
    ] {
        assert_eq!(
            authored_number(input),
            Some(Number::new(expected).unwrap()),
            "{input}"
        );
    }
    for input in [
        "", "%", "1,30", "12,3456", ",100", "1,,000", "1.000,5", "abc", "inf", "1,000,",
    ] {
        assert_eq!(authored_number(input), None, "{input}");
    }
}

#[test]
fn authored_numbers_accept_one_currency_marker() {
    for (input, expected) in [
        ("$1,234.50", 1234.5),
        ("¥1,200", 1200.0),
        ("NT$300", 300.0),
        ("US$ 12", 12.0),
        ("12 €", 12.0),
        ("1,234.50 €", 1234.5),
        ("USD 12.5", 12.5),
        ("12 TWD", 12.0),
        ("-$5", -5.0),
        ("$-5", -5.0),
        ("₩1,000", 1000.0),
        ("￥30", 30.0),
    ] {
        assert_eq!(
            authored_number(input),
            Some(Number::new(expected).unwrap()),
            "{input}"
        );
    }
    for input in [
        "€12,50",
        "1.234,50 €",
        "$12 USD",
        "$12%",
        "($5)",
        "$",
        "USD",
        "-$-5",
        "$$5",
        "usd 5",
        "US 5",
        "INF",
        "฿5",
    ] {
        assert_eq!(authored_number(input), None, "{input}");
    }
}

#[test]
fn authored_dates_accept_year_first_spellings_and_refuse_ambiguous_orders() {
    let october_7 = Date::parse("2026-10-07").unwrap();
    for input in [
        "2026-10-07",
        "2026-10-7",
        "2026/10/7",
        "2026/10/07",
        "2026.10.7",
        " 2026/10/7 ",
        "2026年10月7日",
    ] {
        assert_eq!(authored_date(input), Some(october_7), "{input}");
    }
    for input in [
        "10/7",
        "7/10/2026",
        "10/07/2026",
        "2026/13/1",
        "2026/2/30",
        "26/10/7",
        "2026/10",
        "2026-10-07T00:00",
        "２０２６/10/7",
        "",
    ] {
        assert_eq!(authored_date(input), None, "{input}");
    }
}

#[test]
fn row_entry_stores_canonical_values_from_authored_spellings() {
    let columns = [
        NewTableColumnInput {
            name: "Day".into(),
            field_type: "date".into(),
            required: true,
        },
        NewTableColumnInput {
            name: "Amount".into(),
            field_type: "number".into(),
            required: false,
        },
    ];
    let mut runtime =
        DesignerRuntime::new_table("00000000-0000-4000-8000-000000000596", "Expenses", &columns)
            .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let ids: Vec<String> = runtime
        .query_table(&collection)
        .unwrap()
        .columns
        .into_iter()
        .map(|column| column.id)
        .collect();
    let revision = runtime.current_revision().to_owned();
    runtime
        .insert_row(
            &revision,
            &collection,
            &[
                RowInitializer {
                    field: ids[0].clone(),
                    input: ScalarEditInput::Date {
                        value: "2026/10/7".into(),
                    },
                },
                RowInitializer {
                    field: ids[1].clone(),
                    input: ScalarEditInput::Number {
                        input: "1,300".into(),
                    },
                },
            ],
        )
        .unwrap();
    let row = runtime.query_table(&collection).unwrap().rows.remove(0);
    let stored = |id: &str| {
        row.fields
            .iter()
            .find(|field| field.target.field == id)
            .unwrap()
            .stored
            .clone()
    };
    assert_eq!(
        stored(&ids[0]),
        Some(StoredValueProjection::Date {
            value: Date::parse("2026-10-07").unwrap()
        })
    );
    assert_eq!(
        stored(&ids[1]),
        Some(StoredValueProjection::Number { value: 1300.0 })
    );
}
