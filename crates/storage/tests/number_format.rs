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
