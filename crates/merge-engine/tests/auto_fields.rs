use std::collections::BTreeMap;
use tachiko_merge_engine::{
    ConflictFactV2, MergeOutcome, MergeOutcomeV2, MergeValueV2, merge, merge_v2,
};
use tachiko_semantic_core::{Document, FieldConstraint, FieldDefinition, FieldType, Schema};

fn scalar_document(kind: FieldType) -> Document {
    let mut document = Document::empty("d", "Auto merge");
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
fn auto_conflict_v2_preserves_type_while_frozen_v1_refuses_it() {
    let base = scalar_document(FieldType::Text);
    let left = scalar_document(FieldType::Auto);
    let right = scalar_document(FieldType::Number);
    assert!(matches!(
        merge(&base, &left, &right),
        MergeOutcome::UnsupportedAutoField
    ));
    let MergeOutcomeV2::Merged(candidate) = merge_v2(&base, &left, &base) else {
        panic!("one-sided Auto type change must merge");
    };
    assert_eq!(
        candidate.schemas["s"].fields["a"].field_type,
        FieldType::Auto
    );
    let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &left, &right) else {
        panic!("concurrent different type changes must conflict");
    };
    assert!(conflicts.iter().any(|conflict| matches!(
        conflict.left(),
        ConflictFactV2::Present(MergeValueV2::FieldType(FieldType::Auto))
    )));
}
