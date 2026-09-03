use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, EntityId, FieldDefinition, FieldId, FieldRef, FieldType, Schema, SchemaId,
    Value,
};
use tachiko_workspace_engine::{
    FieldAddress, RuntimeValue, WorkspaceError, runtime_export, set_scalar, validate,
    validate_field_value_suggestion,
};
use tachiko_workspace_engine::{
    patch_lifecycle::DocumentScopeId, resident_session::ResidentWorkspaceSession,
};

fn date_document(value: &str) -> Document {
    let field_id = FieldId::from("published");
    Document {
        id: "dates".into(),
        title: "Dates".to_owned(),
        schemas: BTreeMap::from([(
            SchemaId::from("event"),
            Schema {
                id: SchemaId::from("event"),
                key: "event".into(),
                fields: BTreeMap::from([(
                    field_id.clone(),
                    FieldDefinition {
                        id: field_id,
                        key: "published".into(),
                        field_type: FieldType::Date,
                        required: true,
                    },
                )]),
            },
        )]),
        entities: BTreeMap::from([(
            EntityId::from("launch"),
            Entity {
                id: "launch".into(),
                key: "launch".into(),
                schema: "event".into(),
                fields: BTreeMap::from([("published".into(), Value::Date(value.parse().unwrap()))]),
            },
        )]),
    }
}

#[test]
fn date_flows_through_validation_edit_suggestion_runtime_and_resident_queries() {
    let document = date_document("2024-02-29");
    validate(&document).unwrap();

    let exported = runtime_export(&document).unwrap();
    assert_eq!(
        exported.entities["launch"].fields["published"],
        RuntimeValue::Date("2024-02-29".parse().unwrap())
    );

    let suggestion = validate_field_value_suggestion(
        &document,
        FieldRef::new("launch", "published"),
        Value::Date("2025-01-01".parse().unwrap()),
    )
    .unwrap();
    assert_eq!(suggestion.value, Value::Date("2025-01-01".parse().unwrap()));

    let edited = set_scalar(
        &document,
        &FieldAddress::new("launch", "published"),
        "2025-01-01",
    )
    .unwrap();
    assert_eq!(
        edited.document.entities["launch"].fields["published"],
        Value::Date("2025-01-01".parse().unwrap())
    );
    assert!(matches!(
        set_scalar(
            &document,
            &FieldAddress::new("launch", "published"),
            "1900-02-29"
        ),
        Err(WorkspaceError::InvalidValue {
            expected: "date (YYYY-MM-DD)",
            ..
        })
    ));

    let session =
        ResidentWorkspaceSession::new(DocumentScopeId::from("dates-occurrence"), document);
    let projection = session
        .query_fields(&[FieldRef::new("launch", "published")])
        .unwrap();
    assert_eq!(
        projection.value()[0].stored_value,
        Some(Value::Date("2024-02-29".parse().unwrap()))
    );
}
