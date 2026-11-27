use std::collections::BTreeMap;

use tachiko_semantic_core::{
    FieldDefinition, FieldId, FieldKey, FieldType, Schema, SchemaId, SchemaKey,
};

use super::{CanonicalRoProjectV2, NumberV2, encode};

#[test]
fn storage_number_conversion_normalizes_zero_and_rejects_nonfinite() {
    assert_eq!(NumberV2(-0.0).into_semantic().unwrap().to_bits(), 0);
    assert!(NumberV2(f64::INFINITY).into_semantic().is_err());
}

#[test]
fn full_physical_family_canonicalizes_independent_v2_dtos() {
    let mut document = tachiko_semantic_core::Document::empty("doc", "title");
    document.schemas.insert(
        SchemaId::from("schema"),
        Schema {
            headerless: false,
            field_order: None,
            id: SchemaId::from("schema"),
            key: SchemaKey::from("items"),
            display_name: None,
            fields: BTreeMap::from([(
                FieldId::from("field"),
                FieldDefinition {
                    id: FieldId::from("field"),
                    key: FieldKey::from("name"),
                    display_name: None,
                    number_format: None,
                    date_format: None,
                    field_type: FieldType::Text,
                    required: false,
                    constraint: tachiko_semantic_core::FieldConstraint::None,
                },
            )]),
        },
    );
    let canonical = encode(&document).unwrap();
    let files = vec![
        (
            "definitions.json".to_owned(),
            b" [ ] \n".to_vec(),
        ),
        (
            "schemas.json".to_owned(),
            br#" [ { "fields" : [ { "required" : false, "field_type" : { "type" : "text" }, "key" : "name", "id" : "field" } ], "key" : "items", "id" : "schema" } ] "#.to_vec(),
        ),
        (
            "manifest.json".to_owned(),
            br#"{"document":{"title":"title","id":"doc"},"format_version":2,"format":"tachiko.roproj"}"#.to_vec(),
        ),
    ];
    let admitted = CanonicalRoProjectV2::try_from_files(files).unwrap();
    assert_eq!(admitted, canonical);
}

#[test]
fn physical_input_rejects_unknown_paths_after_manifest_dispatch() {
    let tree = encode(&tachiko_semantic_core::Document::empty("doc", "title")).unwrap();
    let mut files = tree
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect::<Vec<_>>();
    files.push(("entities/cache.txt".to_owned(), Vec::new()));
    assert!(CanonicalRoProjectV2::try_from_files(files).is_err());
}
