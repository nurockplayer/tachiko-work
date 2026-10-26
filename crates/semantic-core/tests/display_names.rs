use std::collections::BTreeMap;
use tachiko_semantic_core::*;

fn document() -> Document {
    let fields = ["item", "other", "third"].map(|key| {
        let id = FieldId::from(key);
        (
            id.clone(),
            FieldDefinition {
                id,
                key: key.into(),
                display_name: None,
                number_format: None,
                field_type: FieldType::Text,
                required: false,
                constraint: FieldConstraint::None,
            },
        )
    });
    let schema = Schema {
        id: "table".into(),
        key: "table".into(),
        display_name: None,
        fields: BTreeMap::from(fields),
    };
    let mut document = Document::empty("names", "Names");
    document.schemas.insert(schema.id.clone(), schema);
    document
}

#[test]
fn frozen_spelling_and_byte_bound() {
    for scalar in [
        '\u{9}', '\u{d}', ' ', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}', '\u{200a}', '\u{2028}',
        '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
    ] {
        assert_eq!(trim_display_name(&format!("{scalar}項目{scalar}")), "項目");
    }
    assert_eq!(
        trim_display_name("\u{feff}Item\u{feff}"),
        "\u{feff}Item\u{feff}"
    );
    for scalar in (0..=31).chain(127..=159).chain([0x2028, 0x2029]) {
        assert!(
            display_name_failures(&format!("a{}b", char::from_u32(scalar).unwrap()))
                .contains(&DisplayNameFailure::ForbiddenScalar)
        );
    }
    for name in [
        "Item",
        "item",
        "Item Name",
        "項目",
        "🧾",
        "Ｉtem",
        "é",
        "e\u{301}",
        "a\u{200b}b",
        "a\u{feff}b",
        "a\u{a0}b",
    ] {
        assert!(display_name_failures(name).is_empty(), "{name:?}");
    }
    assert_eq!(display_name_failures(&"🧾".repeat(1024)), []);
    assert_eq!(
        display_name_failures(&("🧾".repeat(1024) + "a")),
        [DisplayNameFailure::TooLong]
    );
    assert_eq!(display_name_failures(""), [DisplayNameFailure::Empty]);
    assert_eq!(
        display_name_failures(" \u{3000} "),
        [DisplayNameFailure::Empty, DisplayNameFailure::Untrimmed]
    );
    assert_eq!(
        display_name_failures(&(" ".to_owned() + &"🧾".repeat(1024) + " ")),
        [DisplayNameFailure::Untrimmed]
    );
    assert_eq!(
        display_name_failures(" Item "),
        [DisplayNameFailure::Untrimmed]
    );
}

#[test]
fn empty_table_checks_exact_effective_names_and_all_collision_subjects() {
    let mut document = document();
    let table = document.schemas.get_mut(&SchemaId::from("table")).unwrap();
    table
        .fields
        .get_mut(&FieldId::from("other"))
        .unwrap()
        .display_name = Some("item".into());
    table
        .fields
        .get_mut(&FieldId::from("third"))
        .unwrap()
        .display_name = Some("item".into());
    let diagnostics = validate_document_core(&document);
    assert_eq!(diagnostics.len(), 1);
    let collision = &diagnostics[0];
    assert_eq!(collision.code, DiagnosticCode::DUPLICATE_DISPLAY_NAME);
    assert_eq!(collision.subjects.len(), 3);
    assert_eq!(collision.related_subjects, collision.subjects);
    assert_eq!(
        collision.facts,
        [DiagnosticFact::new("effective_name", "item")]
    );
    let table = document.schemas.get_mut(&SchemaId::from("table")).unwrap();
    table
        .fields
        .get_mut(&FieldId::from("other"))
        .unwrap()
        .display_name = Some("Item".into());
    table
        .fields
        .get_mut(&FieldId::from("third"))
        .unwrap()
        .display_name = Some("Ｉtem".into());
    let mut second = table.clone();
    assert_eq!(validate_document_core(&document), []);
    second.id = "second".into();
    second.key = "second".into();
    second.display_name = Some("table".into());
    document.schemas.insert(second.id.clone(), second);
    assert_eq!(
        validate_document_core(&document)[0].code,
        DiagnosticCode::DUPLICATE_DISPLAY_NAME
    );
}

#[test]
fn spelling_failures_accumulate_and_suppress_dependent_uniqueness() {
    let mut document = document();
    let table = document.schemas.get_mut(&SchemaId::from("table")).unwrap();
    for field in table.fields.values_mut() {
        field.display_name = Some("\0".repeat(4097));
    }
    let diagnostics = validate_document_core(&document);
    assert_eq!(diagnostics.len(), 6);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != DiagnosticCode::DUPLICATE_DISPLAY_NAME)
    );
}

#[test]
fn optional_semantic_names_omit_absence_and_reject_null() {
    let field = &document().schemas[&SchemaId::from("table")].fields[&FieldId::from("item")];
    let json = serde_json::to_string(field).unwrap();
    assert!(!json.contains("display_name"));
    let null = json.replacen("\"field_type\"", "\"display_name\":null,\"field_type\"", 1);
    assert!(serde_json::from_str::<FieldDefinition>(&null).is_err());
}
