use std::collections::BTreeMap;

use tachiko_semantic_core::{
    BorderSide, CellAlignment, CellStyle, ColumnCriterion, ColumnRef, ComparisonOperator,
    ConditionalAggregate, ConditionalRule, DateFormat, EntityId, Expression, FieldId,
    FooterAggregate, MAX_EXPRESSION_DEPTH, Number, NumberFormat, NumberFormatKind,
    PresentationStore, RuleId, SchemaId, StyleId, TablePresentation,
};
use tachiko_storage::{FormatError, decode_presentation, encode_presentation, style_id};

const STYLE_FULL: &str = "4a19f7869a552af02c3dc68dc00487d4";
const STYLE_ITALIC: &str = "93c83af0f1468de5e223f716f7ad95f3";
const STYLE_FALSE: &str = "e6973aede98d95414bddea0f99614984";

fn full_style() -> CellStyle {
    CellStyle {
        bold: Some(true),
        italic: Some(false),
        fill: Some("#FFEE00".into()),
        text_color: Some("#112233".into()),
        alignment: Some(CellAlignment::Center),
        wrap: Some(true),
        borders: Some(vec![BorderSide::Top, BorderSide::Bottom]),
        number_format: Some(NumberFormat {
            decimals: 2,
            kind: NumberFormatKind::Plain,
            thousands_separator: true,
        }),
        date_format: Some(DateFormat::YmdCjk),
    }
}

fn store() -> PresentationStore {
    PresentationStore {
        styles: BTreeMap::from([
            (StyleId::from(STYLE_FULL), full_style()),
            (
                StyleId::from(STYLE_ITALIC),
                CellStyle {
                    italic: Some(true),
                    ..CellStyle::default()
                },
            ),
            (
                StyleId::from(STYLE_FALSE),
                CellStyle {
                    bold: Some(false),
                    borders: Some(Vec::new()),
                    ..CellStyle::default()
                },
            ),
        ]),
        tables: BTreeMap::from([(
            SchemaId::from("orders"),
            TablePresentation {
                title: Some("Q3 budget".into()),
                footer: BTreeMap::from([(FieldId::from("f1"), FooterAggregate::Sum)]),
                column_styles: BTreeMap::from([(FieldId::from("f2"), StyleId::from(STYLE_FULL))]),
            },
        )]),
        rules: BTreeMap::from([(
            RuleId::from("r1"),
            ConditionalRule {
                schema: SchemaId::from("orders"),
                rank: "a1".into(),
                fields: vec![FieldId::from("f1")],
                condition: Expression::Compare {
                    operator: ComparisonOperator::Less,
                    left: Box::new(Expression::SameRowReference(FieldId::from("f1"))),
                    right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
                },
                style: StyleId::from(STYLE_ITALIC),
            },
        )]),
        cells: BTreeMap::from([(
            (EntityId::from("e1"), FieldId::from("f1")),
            StyleId::from(STYLE_FALSE),
        )]),
    }
}

fn pinned() -> String {
    let lines = [
        format!(
            "{{\"kind\":\"style\",\"id\":\"{STYLE_FULL}\",\"style\":{{\"bold\":true,\"italic\":false,\"fill\":\"#FFEE00\",\"text_color\":\"#112233\",\"alignment\":\"center\",\"wrap\":true,\"borders\":[\"top\",\"bottom\"],\"number_format\":{{\"decimals\":2,\"kind\":{{\"type\":\"plain\"}},\"thousands_separator\":true}},\"date_format\":\"ymd_cjk\"}}}}"
        ),
        format!("{{\"kind\":\"style\",\"id\":\"{STYLE_ITALIC}\",\"style\":{{\"italic\":true}}}}"),
        format!(
            "{{\"kind\":\"style\",\"id\":\"{STYLE_FALSE}\",\"style\":{{\"bold\":false,\"borders\":[]}}}}"
        ),
        format!(
            "{{\"kind\":\"table\",\"schema\":\"orders\",\"title\":\"Q3 budget\",\"footer\":{{\"f1\":\"sum\"}},\"column_styles\":{{\"f2\":\"{STYLE_FULL}\"}}}}"
        ),
        format!(
            "{{\"kind\":\"rule\",\"id\":\"r1\",\"schema\":\"orders\",\"rank\":\"a1\",\"fields\":[\"f1\"],\"condition\":{{\"op\":\"compare\",\"args\":{{\"operator\":\"<\",\"left\":{{\"op\":\"same_row_reference\",\"args\":\"f1\"}},\"right\":{{\"op\":\"number\",\"args\":0}}}}}},\"style\":\"{STYLE_ITALIC}\"}}"
        ),
        format!(
            "{{\"kind\":\"cell\",\"entity\":\"e1\",\"field\":\"f1\",\"style\":\"{STYLE_FALSE}\"}}"
        ),
    ];
    lines.join("\n") + "\n"
}

fn refuse(payload: &str) {
    assert!(
        decode_presentation(payload.as_bytes()).is_err(),
        "expected a refusal for {payload:?}"
    );
}

#[test]
fn encode_decode_encode_is_byte_identical_and_pinned() {
    let store = store();
    let encoded = encode_presentation(&store).unwrap().unwrap();
    let expected = pinned();
    assert_eq!(String::from_utf8(encoded.clone()).unwrap(), expected);
    let decoded = decode_presentation(&encoded).unwrap();
    assert_eq!(decoded, store);
    assert_eq!(encode_presentation(&decoded).unwrap().unwrap(), encoded);
}

#[test]
fn an_empty_store_encodes_to_none() {
    assert_eq!(
        encode_presentation(&PresentationStore::default()).unwrap(),
        None
    );
}

#[test]
fn style_ids_are_stable_and_false_differs_from_absent() {
    assert_eq!(style_id(&full_style()).unwrap(), StyleId::from(STYLE_FULL));
    assert_eq!(
        style_id(&CellStyle {
            italic: Some(true),
            ..CellStyle::default()
        })
        .unwrap(),
        StyleId::from(STYLE_ITALIC)
    );
    assert_eq!(
        style_id(&CellStyle {
            bold: Some(false),
            borders: Some(Vec::new()),
            ..CellStyle::default()
        })
        .unwrap(),
        StyleId::from(STYLE_FALSE)
    );
    assert_ne!(
        style_id(&CellStyle {
            bold: Some(false),
            ..CellStyle::default()
        })
        .unwrap(),
        style_id(&CellStyle::default()).unwrap()
    );
}

#[test]
fn style_id_refuses_unspellable_styles_without_panicking() {
    let lowercase = CellStyle {
        fill: Some("#aabbcc".into()),
        ..CellStyle::default()
    };
    assert!(matches!(
        style_id(&lowercase),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
    let duplicate_borders = CellStyle {
        borders: Some(vec![BorderSide::Top, BorderSide::Top]),
        ..CellStyle::default()
    };
    assert!(matches!(
        style_id(&duplicate_borders),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn encoder_refuses_a_style_keyed_by_another_id() {
    let store = PresentationStore {
        styles: BTreeMap::from([(StyleId::from("s1"), full_style())]),
        ..PresentationStore::default()
    };
    assert!(matches!(
        encode_presentation(&store),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn decoder_refuses_an_empty_member() {
    assert!(matches!(
        decode_presentation(b""),
        Err(FormatError::InvalidRoProjectRepresentation { .. })
    ));
}

#[test]
fn decoder_refuses_unknown_kinds_members_and_values() {
    let expected = pinned();
    refuse(&expected.replace("\"kind\":\"cell\"", "\"kind\":\"note\""));
    refuse(&expected.replace("\"bold\":true", "\"bold\":true,\"strike\":true"));
    refuse(&expected.replace("\"date_format\":\"ymd_cjk\"", "\"date_format\":\"ymd\""));
    refuse(&expected.replace("\"f1\":\"sum\"", "\"f1\":\"total\""));
    refuse(&expected.replace(
        "\"borders\":[\"top\",\"bottom\"]",
        "\"borders\":[\"sideways\"]",
    ));
    refuse(&expected.replace("#FFEE00", "#ffee00"));
}

#[test]
fn decoder_refuses_duplicate_and_out_of_order_members() {
    let expected = pinned();
    refuse(&expected.replace("\"bold\":true", "\"bold\":true,\"bold\":false"));
    refuse(&expected.replace(
        "\"kind\":\"cell\",\"entity\":\"e1\"",
        "\"entity\":\"e1\",\"kind\":\"cell\"",
    ));
    refuse(&expected.replace(
        "\"borders\":[\"top\",\"bottom\"]",
        "\"borders\":[\"bottom\",\"top\"]",
    ));
    refuse(&expected.replace("\"fields\":[\"f1\"]", "\"fields\":[\"f2\",\"f1\"]"));
    refuse(&expected.replace("\"fields\":[\"f1\"]", "\"fields\":[\"f1\",\"f1\"]"));
    refuse(&expected.replace("\"fields\":[\"f1\"]", "\"fields\":[]"));
}

#[test]
fn decoder_refuses_out_of_order_records() {
    let expected = pinned();
    let mut lines: Vec<&str> = expected.lines().collect();
    lines.swap(0, 3);
    refuse(&(lines.join("\n") + "\n"));

    let later_cell = format!(
        "{{\"kind\":\"cell\",\"entity\":\"e1\",\"field\":\"f0\",\"style\":\"{STYLE_FALSE}\"}}\n"
    );
    refuse(&(expected.clone() + &later_cell));
}

#[test]
fn decoder_refuses_lines_that_do_not_end_canonically() {
    let expected = pinned();
    let without_lf = expected.strip_suffix('\n').unwrap();
    refuse(without_lf);
    refuse(&(expected.clone() + "\n"));
    refuse(&expected.replacen('\n', "\n\n", 1));
}

#[test]
fn decoder_refuses_a_style_id_that_does_not_match_its_content() {
    let expected = pinned();
    refuse(&expected.replace(
        "\"style\":{\"italic\":true}",
        "\"style\":{\"italic\":false}",
    ));
}

#[test]
fn decoder_refuses_a_table_with_no_content_or_an_empty_map() {
    refuse("{\"kind\":\"table\",\"schema\":\"orders\"}\n");
    refuse("{\"kind\":\"table\",\"schema\":\"orders\",\"footer\":{}}\n");
    refuse("{\"kind\":\"table\",\"schema\":\"orders\",\"column_styles\":{}}\n");
}

#[test]
fn decoder_refuses_noncanonical_spelling() {
    let expected = pinned();
    refuse(&expected.replace("\"bold\":true", "\"bold\": true"));
    refuse(&expected.replace("\"args\":0", "\"args\":0.0"));
}

fn store_with_condition(condition: Expression) -> PresentationStore {
    let mut store = store();
    store.rules.get_mut(&RuleId::from("r1")).unwrap().condition = condition;
    store
}

#[test]
fn encoder_refuses_a_rule_condition_the_reader_refuses() {
    let number = || Box::new(Expression::Number(Number::new(1.0).unwrap()));
    for condition in [
        Expression::Round {
            value: number(),
            digits: 11,
        },
        Expression::IsBlank(number()),
    ] {
        assert!(matches!(
            encode_presentation(&store_with_condition(condition)),
            Err(FormatError::InvalidRoProjectRepresentation { .. })
        ));
    }
}

#[test]
fn the_deepest_allowed_rule_condition_reads_back() {
    let column = || ColumnRef::new(SchemaId::from("orders"), FieldId::from("f1"));
    let conditional = |value: Option<Box<Expression>>| Expression::ColumnConditional {
        aggregate: Box::new(ConditionalAggregate::Count),
        criteria: vec![ColumnCriterion {
            column: column(),
            operator: ComparisonOperator::Equal,
            value,
        }],
    };
    let mut condition = conditional(None);
    for _ in 1..MAX_EXPRESSION_DEPTH {
        condition = conditional(Some(Box::new(condition)));
    }
    let store = store_with_condition(condition);
    let bytes = encode_presentation(&store).unwrap().unwrap();
    assert_eq!(decode_presentation(&bytes).unwrap(), store);
}
