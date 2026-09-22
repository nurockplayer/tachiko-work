use tachiko_semantic_core::{FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldType};

#[test]
fn field_constraint_serde_rejects_missing_null_unknown_and_extra_members() {
    let original = serde_json::to_value(FieldDefinition {
        id: FieldId::from("f"),
        key: FieldKey::from("f"),
        field_type: FieldType::Text,
        required: false,
        constraint: FieldConstraint::None,
    })
    .expect("field definition serializes");

    for replacement in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!({"type": "unknown"})),
        Some(serde_json::json!({"type": "none", "extra": true})),
    ] {
        let mut value = original.clone();
        if let Some(replacement) = replacement {
            value["constraint"] = replacement;
        } else {
            value
                .as_object_mut()
                .expect("serialized field definition is an object")
                .remove("constraint");
        }
        assert!(serde_json::from_value::<FieldDefinition>(value).is_err());
    }
}
