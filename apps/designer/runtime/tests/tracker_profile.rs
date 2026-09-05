use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget, ScalarEditInput,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Schema,
    SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn human_tracker_key_does_not_claim_stock_profile_or_block_generic_scalar_edit() {
    let mut document = Document::empty("ordinary_project", "Ordinary tracker collection");
    let schema_id = SchemaId::from("ordinary_schema");
    let field_id = FieldId::from("description");
    document.schemas.insert(
        schema_id.clone(),
        Schema {
            id: schema_id.clone(),
            key: SchemaKey::from("tracker"),
            fields: BTreeMap::from([(
                field_id.clone(),
                FieldDefinition {
                    id: field_id.clone(),
                    key: FieldKey::from("description"),
                    field_type: FieldType::Text,
                    required: true,
                },
            )]),
        },
    );
    let entity_id = EntityId::from("ordinary_row");
    document.entities.insert(
        entity_id.clone(),
        Entity {
            id: entity_id,
            key: EntityKey::from("entry"),
            schema: schema_id,
            fields: BTreeMap::from([(field_id, Value::Text("Original".to_owned()))]),
        },
    );
    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "tracker".to_owned(),
        })
        .unwrap()
    else {
        panic!("table")
    };
    assert_eq!(table.tracker_profile, None);
    assert!(
        serde_json::to_value(&table)
            .unwrap()
            .get("tracker_profile")
            .is_none()
    );
    assert_eq!(table.columns[0].key, "description");
    assert!(
        runtime
            .handle(DesignerRequest::AppendRow {
                expected_revision: table.revision.clone(),
                collection: "tracker".to_owned()
            })
            .is_err()
    );
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: table.revision,
            target: FieldTarget {
                entity: "ordinary_row".to_owned(),
                field: "description".to_owned(),
            },
            input: ScalarEditInput::Text {
                value: "Edited".to_owned(),
            },
        })
        .unwrap()
    else {
        panic!("publication")
    };
    assert_eq!(publication.resulting_revision, "resident/1");
}

#[test]
fn stock_tracker_advertises_authoritative_profile() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "tracker".to_owned(),
        })
        .unwrap()
    else {
        panic!("table")
    };
    assert_eq!(table.tracker_profile, Some(true));
    assert_eq!(
        serde_json::to_value(&table).unwrap()["tracker_profile"],
        true
    );
    runtime
        .handle(DesignerRequest::AppendRow {
            expected_revision: table.revision,
            collection: "tracker".to_owned(),
        })
        .unwrap();
}
