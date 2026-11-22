use std::collections::BTreeMap;
use tachiko_semantic_core::{
    Date, Document, Entity, FieldConstraint, FieldDefinition, FieldType, Number, Schema, Value,
};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, decode_portable_package_v1, decode_roproj_v3,
    encode_portable_package_v1, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3,
    to_canonical_string,
};

fn auto_document() -> Document {
    let mut document = Document::empty("auto-doc", "Auto");
    document.schemas.insert(
        "s".into(),
        Schema {
            headerless: false,
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
    for (index, value) in [
        Some(Value::Number(Number::new(42.0).unwrap())),
        Some(Value::Text("00123".into())),
        Some(Value::Date(Date::parse("2026-11-22").unwrap())),
        Some(Value::Boolean(true)),
        Some(Value::Text("hello".into())),
        Some(Value::Text(String::new())),
        None,
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("r{index}");
        document.entities.insert(
            id.clone().into(),
            Entity {
                id: id.clone().into(),
                key: id.into(),
                schema: "s".into(),
                rank: None,
                fields: value
                    .map(|value| BTreeMap::from([("a".into(), value)]))
                    .unwrap_or_default(),
            },
        );
    }
    document
}

#[test]
fn auto_v3_and_portable_roundtrip_keep_tags_and_canonical_bytes() {
    let document = auto_document();
    let tree = encode_roproj_v3(&document).unwrap();
    let schema = std::str::from_utf8(tree.file("schemas.json").unwrap()).unwrap();
    assert!(schema.contains("\"type\": \"auto\""));
    assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
    let bytes = encode_portable_package_v1(&tree).unwrap();
    let reopened = decode_portable_package_v1(&bytes).unwrap();
    assert_eq!(decode_roproj_v3(reopened.tree()).unwrap(), document);
    assert_eq!(encode_portable_package_v1(reopened.tree()).unwrap(), bytes);
}

#[test]
fn auto_frozen_writers_refuse_without_stripping() {
    let document = auto_document();
    assert!(matches!(
        to_canonical_string(&document),
        Err(FormatError::UnsupportedAutoField)
    ));
    assert!(matches!(
        encode_roproj_v1(&document),
        Err(FormatError::UnsupportedAutoField)
    ));
    assert!(matches!(
        encode_roproj_v2(&document),
        Err(FormatError::UnsupportedAutoField)
    ));
    assert!(document.has_auto_fields());
}

#[test]
fn auto_v3_reader_refuses_malformed_pairings_and_unknown_members() {
    let tree = encode_roproj_v3(&auto_document()).unwrap();
    for (old, new) in [
        ("\"required\": false", "\"required\": true"),
        (
            "\"type\": \"auto\"",
            "\"type\": \"auto\",\n        \"schema\": \"s\"",
        ),
    ] {
        let files = tree
            .files()
            .iter()
            .map(|file| {
                let bytes = if file.path() == "schemas.json" {
                    std::str::from_utf8(file.bytes())
                        .unwrap()
                        .replace(old, new)
                        .into_bytes()
                } else {
                    file.bytes().to_vec()
                };
                (file.path().to_owned(), bytes)
            })
            .collect();
        assert!(CanonicalRoProjectV3::try_from_files(files).is_err());
    }
}
