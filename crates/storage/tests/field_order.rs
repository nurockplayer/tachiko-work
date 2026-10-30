use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, FieldConstraint, FieldDefinition, FieldId, FieldType, Schema,
};
use tachiko_storage::{
    CanonicalRoProjectV3, FormatError, decode_roproj_v3, encode_roproj_v1, encode_roproj_v2,
    encode_roproj_v3, from_str, to_canonical_string,
};

fn field(id: &str) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type: FieldType::Number,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
    }
}

fn document(order: Option<Vec<&str>>) -> Document {
    let mut document = Document::empty("d", "Order");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            field_order: order.map(|ids| ids.into_iter().map(FieldId::from).collect()),
            fields: BTreeMap::from([
                ("a".into(), field("a")),
                ("b".into(), field("b")),
                ("c".into(), field("c")),
            ]),
        },
    );
    document
}

#[test]
fn v3_roundtrips_an_explicit_field_order() {
    let document = document(Some(vec!["c", "a", "b"]));
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
}

#[test]
fn id_order_documents_keep_their_bytes_without_a_field_order_member() {
    let document = document(None);
    let tree = encode_roproj_v3(&document).unwrap();
    for file in tree.files() {
        let text = String::from_utf8_lossy(file.bytes());
        assert!(!text.contains("field_order"), "{}", file.path());
    }
    let direct = to_canonical_string(&document).unwrap();
    assert!(!direct.contains("field_order"));
    assert_eq!(decode_roproj_v3(&tree).unwrap(), document);
}

#[test]
fn frozen_v1_and_v2_refuse_an_explicit_field_order() {
    let document = document(Some(vec!["c", "a", "b"]));
    assert!(matches!(
        encode_roproj_v1(&document),
        Err(FormatError::UnsupportedFieldOrder)
    ));
    assert!(matches!(
        encode_roproj_v2(&document),
        Err(FormatError::UnsupportedFieldOrder)
    ));
}
