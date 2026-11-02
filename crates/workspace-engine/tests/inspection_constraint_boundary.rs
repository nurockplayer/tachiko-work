use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    Document, FieldConstraint, FieldDefinition, FieldType, Schema, inspect_document,
};

fn constrained_document() -> Document {
    let mut document = Document::empty("document", "Inspection boundary");
    document.schemas.insert(
        "schema".into(),
        Schema {
            field_order: None,
            id: "schema".into(),
            key: "items".into(),
            display_name: None,
            fields: BTreeMap::from([(
                "field".into(),
                FieldDefinition {
                    id: "field".into(),
                    key: "status".into(),
                    display_name: None,
                    number_format: None,
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

#[test]
fn an_explicit_field_order_refuses_the_frozen_json_transport_even_in_id_order() {
    let mut document = constrained_document();
    let schema = document.schemas.get_mut("schema").unwrap();
    schema
        .fields
        .values_mut()
        .for_each(|field| field.constraint = FieldConstraint::None);
    schema.field_order = Some(vec!["field".into()]);
    let inspection = inspect_document(&document, "fixture");

    assert_eq!(inspection.schemas[0].field_order, schema_order(&document));
    let error = serde_json::to_value(&inspection)
        .expect_err("the legacy transport must not silently omit a field order");
    assert!(error.to_string().contains("cannot represent a field order"));
}

fn schema_order(document: &Document) -> Option<Vec<tachiko_workspace_engine::FieldId>> {
    document.schemas["schema"].field_order.clone()
}
