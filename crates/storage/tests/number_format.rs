use std::collections::BTreeMap;
use tachiko_semantic_core::{
    Document, FieldConstraint, FieldDefinition, FieldType, NumberFormat, NumberFormatKind, Schema,
};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, ROPROJ_V3_PATHS, decode_roproj_v3, encode_roproj_v1,
    encode_roproj_v2, encode_roproj_v3, from_str, to_canonical_string,
};

fn document() -> Document {
    let mut document = Document::empty("d", "Formats");
    document.schemas.insert(
        "s".into(),
        Schema {
            field_order: None,
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            fields: BTreeMap::from([(
                "n".into(),
                FieldDefinition {
                    id: "n".into(),
                    key: "n".into(),
                    display_name: None,
                    field_type: FieldType::Number,
                    required: false,
                    constraint: FieldConstraint::None,
                    number_format: Some(NumberFormat {
                        decimals: 2,
                        kind: NumberFormatKind::Currency { code: "USD".into() },
                        thousands_separator: true,
                    }),
                    date_format: None,
                },
            )]),
        },
    );
    document
}

#[test]
fn v3_and_direct_v2_roundtrip_all_number_format_kinds() {
    for kind in [
        NumberFormatKind::Plain,
        NumberFormatKind::Percent,
        NumberFormatKind::Currency { code: "JPY".into() },
    ] {
        let mut document = document();
        document
            .schemas
            .get_mut("s")
            .unwrap()
            .fields
            .get_mut("n")
            .unwrap()
            .number_format
            .as_mut()
            .unwrap()
            .kind = kind;
        let tree = encode_roproj_v3(&document).unwrap();
        assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
        let readback = CanonicalRoProjectV3::try_from_files(
            tree.files()
                .iter()
                .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
                .collect(),
        )
        .unwrap();
        assert_eq!(readback, tree);
        let direct = to_canonical_string(&document).unwrap();
        assert_eq!(from_str(&direct).unwrap(), document);
        assert!(matches!(
            encode_roproj_v1(&document),
            Err(FormatError::UnsupportedNumberFormat)
        ));
        assert!(matches!(
            encode_roproj_v2(&document),
            Err(FormatError::UnsupportedNumberFormat)
        ));
    }
}

#[test]
fn absent_format_keeps_existing_direct_and_v3_bytes_exact() {
    let golden = include_str!("fixtures/direct-ro-v2-numeric-golden.ro");
    let document = from_str(golden).unwrap();
    assert!(!document.has_number_formats());
    assert_eq!(to_canonical_string(&document).unwrap(), golden);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/research/fixtures/issue-519-portable-package-v1-v3/input.roproj");
    let tree = CanonicalRoProjectV3::try_from_files(
        ROPROJ_V3_PATHS
            .into_iter()
            .map(|path| (path.to_owned(), std::fs::read(root.join(path)).unwrap()))
            .collect(),
    )
    .unwrap();
    let document = decode_roproj_v3(&tree).unwrap();
    assert!(!document.has_number_formats());
    assert!(!document.has_date_formats());
    assert!(!std::str::from_utf8(tree.file("schemas.json").unwrap()).unwrap().contains("date_format"));
    assert_eq!(encode_roproj_v3(&document).unwrap(), tree);
}

#[test]
fn storage_refuses_malformed_and_non_number_declarations() {
    let mut document = document();
    let field = document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap();
    field.number_format.as_mut().unwrap().decimals = 11;
    assert!(matches!(
        encode_roproj_v3(&document),
        Err(FormatError::InvalidDocument { .. })
    ));
    let field = document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap();
    field.number_format.as_mut().unwrap().decimals = 2;
    field.field_type = FieldType::Text;
    assert!(matches!(
        to_canonical_string(&document),
        Err(FormatError::InvalidDocument { .. })
    ));
    field_reset(&mut document);
    let valid = to_canonical_string(&document).unwrap();
    for invalid in ["null", "{}", "{\"decimals\":-1}"] {
        let mut json: serde_json::Value = serde_json::from_str(&valid).unwrap();
        json["schemas"]["s"]["fields"]["n"]["number_format"] =
            serde_json::from_str(invalid).unwrap();
        assert!(from_str(&serde_json::to_string(&json).unwrap()).is_err());
    }
}

fn field_reset(document: &mut Document) {
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("n")
        .unwrap()
        .field_type = FieldType::Number;
}

#[test]
fn stored_kind_rejects_members_outside_its_variant() {
    let valid = to_canonical_string(&document()).unwrap();
    for kind in [
        r#"{"type":"plain","code":"USD"}"#,
        r#"{"type":"percent","code":"USD"}"#,
    ] {
        let mut json: serde_json::Value = serde_json::from_str(&valid).unwrap();
        json["schemas"]["s"]["fields"]["n"]["number_format"]["kind"] =
            serde_json::from_str(kind).unwrap();
        assert!(
            from_str(&serde_json::to_string(&json).unwrap()).is_err(),
            "{kind}"
        );
    }
}

#[test]
fn date_formats_roundtrip_and_frozen_writers_refuse_them() {
    use tachiko_semantic_core::DateFormat;
    let mut doc = document();
    let field = doc.schemas.get_mut("s").unwrap().fields.get_mut("n").unwrap();
    field.field_type = FieldType::Date;
    field.number_format = None;
    let absent = encode_roproj_v3(&doc).unwrap();
    assert!(!std::str::from_utf8(absent.file("schemas.json").unwrap()).unwrap().contains("date_format"));
    assert_eq!(encode_roproj_v3(&decode_roproj_v3(&absent).unwrap()).unwrap(), absent);
    for (format, wire) in [
        (DateFormat::Iso, "iso"), (DateFormat::Mdy, "mdy"), (DateFormat::Dmy, "dmy"),
        (DateFormat::YmdSlash, "ymd_slash"), (DateFormat::YmdCjk, "ymd_cjk"),
        (DateFormat::MonthDayYear, "month_day_year"), (DateFormat::DayMonthYear, "day_month_year"),
    ] {
        doc.schemas.get_mut("s").unwrap().fields.get_mut("n").unwrap().date_format = Some(format);
        let tree = encode_roproj_v3(&doc).unwrap();
        assert!(std::str::from_utf8(tree.file("schemas.json").unwrap()).unwrap().contains(&format!("\"date_format\": \"{wire}\"")));
        let decoded = decode_roproj_v3(&tree).unwrap();
        assert_eq!(decoded, doc);
        assert_eq!(encode_roproj_v3(&decoded).unwrap(), tree);
        assert!(matches!(to_canonical_string(&doc), Err(FormatError::UnsupportedDateFormat)));
        assert!(matches!(encode_roproj_v1(&doc), Err(FormatError::UnsupportedDateFormat)));
        assert!(matches!(encode_roproj_v2(&doc), Err(FormatError::UnsupportedDateFormat)));
    }
    for invalid in ["null", "\"ISO\"", "\"yyyy\""] {
        let files = absent.files().iter().map(|file| {
            let bytes = if file.path() == "schemas.json" {
                let text = std::str::from_utf8(file.bytes()).unwrap();
                text.replacen("\"constraint\": {", &format!("\"date_format\": {invalid},\n      \"constraint\": {{"), 1).into_bytes()
            } else { file.bytes().to_vec() };
            (file.path().to_owned(), bytes)
        }).collect();
        assert!(CanonicalRoProjectV3::try_from_files(files).is_err(), "{invalid}");
    }
}
