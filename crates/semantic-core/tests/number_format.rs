use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, FieldConstraint, FieldDefinition, FieldType, NumberFormat, NumberFormatError,
    NumberFormatKind, Schema, validate_document,
};

fn format(decimals: u8, kind: NumberFormatKind) -> NumberFormat {
    NumberFormat {
        decimals,
        kind,
        thousands_separator: true,
    }
}

#[test]
fn validates_number_format_without_coercion() {
    for decimals in 0..=10 {
        for kind in [
            NumberFormatKind::Plain,
            NumberFormatKind::Percent,
            NumberFormatKind::Currency { code: "TWD".into() },
        ] {
            assert_eq!(
                format(decimals, kind).validate_for(&FieldType::Number),
                Ok(())
            );
        }
    }
    assert_eq!(
        format(11, NumberFormatKind::Plain).validate_for(&FieldType::Number),
        Err(NumberFormatError::DecimalsOutOfRange)
    );
    for code in ["usd", "US", "USDD", "U$D", "ＵＳＤ", "USD\n"] {
        assert_eq!(
            format(2, NumberFormatKind::Currency { code: code.into() })
                .validate_for(&FieldType::Number),
            Err(NumberFormatError::InvalidCurrencyCode)
        );
    }
    for kind in [
        FieldType::Text,
        FieldType::Boolean,
        FieldType::Date,
        FieldType::Reference { schema: "s".into() },
    ] {
        assert_eq!(
            format(0, NumberFormatKind::Plain).validate_for(&kind),
            Err(NumberFormatError::NonNumberField)
        );
    }
}

#[test]
fn absent_format_preserves_serialization_and_invalid_declarations_are_diagnosed() {
    let json = r#"{"id":"n","key":"n","field_type":{"type":"number"},"required":false,"constraint":{"type":"none"}}"#;
    let mut field: FieldDefinition = serde_json::from_str(json).unwrap();
    assert_eq!(field.number_format, None);
    assert_eq!(serde_json::to_string(&field).unwrap(), json);
    field.number_format = Some(format(11, NumberFormatKind::Plain));
    let mut document = Document::empty("d", "Formats");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            fields: BTreeMap::from([("n".into(), field)]),
        },
    );
    let diagnostics = validate_document(&document);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code.as_str(),
        "core.field_number_format_declaration"
    );
    let field = document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap();
    field.field_type = FieldType::Text;
    field.constraint = FieldConstraint::None;
    field.number_format.as_mut().unwrap().decimals = 2;
    assert_eq!(validate_document(&document).len(), 1);
}

#[test]
fn wire_declarations_are_closed_and_typed() {
    for json in [
        r#"{"decimals":-1,"kind":{"type":"plain"},"thousands_separator":false}"#,
        r#"{"decimals":1.5,"kind":{"type":"plain"},"thousands_separator":false}"#,
        r#"{"decimals":2,"kind":{"type":"unknown"},"thousands_separator":false}"#,
        r#"{"decimals":2,"kind":{"type":"plain","code":"USD"},"thousands_separator":false}"#,
        r#"{"decimals":2,"kind":{"type":"plain"},"thousands_separator":false,"extra":0}"#,
    ] {
        assert!(
            serde_json::from_str::<NumberFormat>(json).is_err(),
            "{json}"
        );
    }
}
