use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    Document, FieldConstraint, FieldDefinition, FieldType, Schema, inspect_document,
};

fn constrained_document() -> Document {
    let mut document = Document::empty("document", "Inspection boundary");
    document.schemas.insert(
        "schema".into(),
        Schema {
            id: "schema".into(),
            key: "items".into(),
            fields: BTreeMap::from([(
                "field".into(),
                FieldDefinition {
                    id: "field".into(),
                    key: "status".into(),
                    field_type: FieldType::Text,
                    required: false,
                    constraint: FieldConstraint::TextLiteralSet {
                        values: vec!["ready".to_owned()],
                    },
                },
            )]),
        },
    );
    document
}

#[test]
fn constrained_inspection_keeps_the_model_but_refuses_the_frozen_json_transport() {
    let inspection = inspect_document(&constrained_document(), "fixture");

    assert!(matches!(
        inspection.schemas[0].fields[0].constraint,
        FieldConstraint::TextLiteralSet { .. }
    ));
    let error = serde_json::to_value(&inspection)
        .expect_err("the legacy transport must not silently omit a constraint");
    assert!(
        error
            .to_string()
            .contains("cannot represent field constraints")
    );
}
