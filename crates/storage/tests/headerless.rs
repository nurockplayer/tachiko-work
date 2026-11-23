use std::collections::BTreeMap;
use tachiko_semantic_core::{Document, Schema};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, decode_portable_package_v1, decode_roproj_v3,
    encode_portable_package_v1, encode_roproj_v1, encode_roproj_v2, encode_roproj_v3,
    to_canonical_string,
};

fn document(headerless: bool) -> Document {
    let mut document = Document::empty("d", "Blank");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            headerless,
            field_order: None,
            fields: BTreeMap::new(),
        },
    );
    document
}

#[test]
fn headerless_canonical_bytes_roundtrip_and_default_omission() {
    for headerless in [false, true] {
        let document = document(headerless);
        let tree = encode_roproj_v3(&document).unwrap();
        let expected = if headerless {
            "[\n  {\n    \"id\": \"s\",\n    \"key\": \"s\",\n    \"headerless\": true,\n    \"fields\": []\n  }\n]\n"
        } else {
            "[\n  {\n    \"id\": \"s\",\n    \"key\": \"s\",\n    \"fields\": []\n  }\n]\n"
        };
        assert_eq!(tree.file("schemas.json").unwrap(), expected.as_bytes());
        assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
        let package = encode_portable_package_v1(&tree).unwrap();
        assert_eq!(
            decode_roproj_v3(decode_portable_package_v1(&package).unwrap().tree()).unwrap(),
            document
        );
    }
}

#[test]
fn headerless_strict_reader_refuses_false_null_wrong_types_and_unknown_members() {
    let tree = encode_roproj_v3(&document(true)).unwrap();
    for invalid in [
        "false",
        "null",
        "1",
        "\"true\"",
        "{}",
        "true,\n    \"unknown\": true",
    ] {
        let files = tree
            .files()
            .iter()
            .map(|file| {
                let bytes = if file.path() == "schemas.json" {
                    String::from_utf8(file.bytes().to_vec())
                        .unwrap()
                        .replace(
                            "\"headerless\": true",
                            &format!("\"headerless\": {invalid}"),
                        )
                        .into_bytes()
                } else {
                    file.bytes().to_vec()
                };
                (file.path().to_owned(), bytes)
            })
            .collect();
        assert!(
            CanonicalRoProjectV3::try_from_files(files).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn headerless_frozen_writers_refuse_even_without_auto_fields_or_names() {
    let document = document(true);
    assert!(matches!(
        to_canonical_string(&document),
        Err(FormatError::UnsupportedHeaderless)
    ));
    assert!(matches!(
        encode_roproj_v1(&document),
        Err(FormatError::UnsupportedHeaderless)
    ));
    assert!(matches!(
        encode_roproj_v2(&document),
        Err(FormatError::UnsupportedHeaderless)
    ));
}
