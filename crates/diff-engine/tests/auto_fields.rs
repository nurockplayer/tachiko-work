use std::collections::BTreeMap;
use tachiko_diff_engine::{
    CANONICAL_SEMANTIC_DELTA_V2, CanonicalDirectFact, DiffError, canonical_delta, diff,
};
use tachiko_semantic_core::{Document, FieldConstraint, FieldDefinition, FieldType, Schema};

fn scalar_document(kind: FieldType) -> Document {
    let mut document = Document::empty("d", "Auto delta");
    document.schemas.insert(
        "s".into(),
        Schema {
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
                    field_type: kind,
                    required: false,
                    constraint: FieldConstraint::None,
                    number_format: None,
                    date_format: None,
                },
            )]),
        },
    );
    document
}

#[test]
fn auto_delta_v2_carries_type_and_frozen_v1_refuses_it() {
    let before = scalar_document(FieldType::Text);
    let after = scalar_document(FieldType::Auto);
    let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &before, &after).unwrap();
    assert!(delta.facts().iter().any(|fact| matches!(
        fact,
        CanonicalDirectFact::SchemaFieldTypeChanged {
            before: FieldType::Text,
            after: FieldType::Auto,
            ..
        }
    )));
    assert!(matches!(
        diff(&before, &after),
        Err(DiffError::UnsupportedAutoField)
    ));
    assert!(matches!(
        diff(&after, &after),
        Err(DiffError::UnsupportedAutoField)
    ));
}
