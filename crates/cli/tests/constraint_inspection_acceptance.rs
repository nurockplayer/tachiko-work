use std::collections::BTreeMap;
use tachiko_workspace_engine::{
    Document, FieldConstraint, FieldDefinition, FieldType, Schema, inspect_document,
};

fn document(constraint: FieldConstraint) -> Document {
    let mut document = Document::empty("doc", "Inspection acceptance");
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
                    constraint,
                },
            )]),
        },
    );
    document
}

#[test]
fn none_inspection_preserves_the_existing_four_member_field_wire() {
    let inspection = inspect_document(&document(FieldConstraint::None), "fixture");
    let value = serde_json::to_value(inspection).unwrap();
    assert_eq!(
        value["schemas"][0]["fields"][0],
        serde_json::json!({
            "id": "field", "key": "status", "field_type": {"type": "text"}, "required": false
        })
    );
}

#[test]
fn existing_inspection_wire_refuses_constraints_instead_of_dropping_or_expanding_them() {
    let constraint = FieldConstraint::TextLiteralSet {
        values: vec!["ready".to_owned()],
    };
    let source = document(constraint.clone());
    let original = source.clone();
    let inspection = inspect_document(&source, "fixture");
    assert_eq!(inspection.schemas[0].fields[0].constraint, constraint);
    assert!(serde_json::to_value(&inspection).is_err());
    assert!(serde_json::to_vec(&inspection).is_err());
    assert_eq!(source, original);
}
