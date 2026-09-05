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

fn stock_document() -> Document {
    let tree = tachiko_storage::read_canonical_roproj(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../dogfood/operations-tracker.roproj"),
    )
    .unwrap();
    tachiko_storage::decode_roproj_v1(&tree).unwrap()
}

#[test]
fn admitted_stock_schema_with_formula_uses_generic_projection_without_losing_calculation() {
    use tachiko_workspace_engine::{Expression, Number};
    let mut document = stock_document();
    let entity = document.entities.values_mut().next().unwrap();
    let id = entity.id.to_string();
    entity.fields.insert(
        FieldId::from("estimate"),
        Value::Formula(Expression::Number(Number::new(42.0).unwrap())),
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
    let field = table
        .rows
        .iter()
        .find(|row| row.id == id)
        .unwrap()
        .fields
        .iter()
        .find(|field| field.target.field == "estimate")
        .unwrap();
    assert!(field.stored.is_none());
    assert!(field.formula.is_some());
    assert_eq!(
        field
            .calculated
            .as_ref()
            .and_then(tachiko_designer_runtime::CalculationProjection::number),
        Some(42.0)
    );
    assert!(
        runtime
            .handle(DesignerRequest::AppendRow {
                expected_revision: table.revision.clone(),
                collection: "tracker".to_owned()
            })
            .is_err()
    );
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: table.revision,
            target: FieldTarget {
                entity: id,
                field: "task".to_owned(),
            },
            input: ScalarEditInput::Text {
                value: "Still editable".to_owned(),
            },
        })
        .unwrap();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "tracker".to_owned(),
        })
        .unwrap()
    else {
        panic!("table")
    };
    assert_eq!(table.tracker_profile, None);
}

#[test]
fn admitted_missing_optional_tracker_value_uses_generic_profile() {
    for field_name in ["estimate", "done"] {
        let mut document = stock_document();
        document
            .schemas
            .get_mut(&SchemaId::from("tracker"))
            .unwrap()
            .fields
            .get_mut(&FieldId::from(field_name))
            .unwrap()
            .required = false;
        let entity = document.entities.values_mut().next().unwrap();
        let id = entity.id.to_string();
        entity.fields.remove(&FieldId::from(field_name));
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
            !table
                .rows
                .iter()
                .find(|row| row.id == id)
                .unwrap()
                .fields
                .iter()
                .any(|field| field.target.field == field_name)
        );
        assert!(
            runtime
                .handle(DesignerRequest::AppendRow {
                    expected_revision: table.revision,
                    collection: "tracker".to_owned()
                })
                .is_err()
        );
    }
}

#[test]
fn complete_but_optional_stock_fields_do_not_advertise_tracker_profile() {
    let mut document = stock_document();
    document
        .schemas
        .get_mut(&SchemaId::from("tracker"))
        .unwrap()
        .fields
        .get_mut(&FieldId::from("estimate"))
        .unwrap()
        .required = false;
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
    assert!(table.rows.iter().all(|row| row.fields.len() == 3));
    assert!(
        runtime
            .handle(DesignerRequest::AppendRow {
                expected_revision: table.revision,
                collection: "tracker".to_owned()
            })
            .is_err()
    );
}

#[test]
fn unchanged_scalar_wire_reply_preserves_no_change_code_and_revision() {
    use tachiko_designer_runtime::{DesignerWireReply, process_wire_request};
    let document = stock_document();
    let entity = document.entities.values().next().unwrap();
    let request = DesignerRequest::EditScalar {
        expected_revision: "resident/0".to_owned(),
        target: FieldTarget {
            entity: entity.id.to_string(),
            field: "done".to_owned(),
        },
        input: ScalarEditInput::Boolean { value: false },
    };
    let mut runtime = Some(DesignerRuntime::from_document(document, OCCURRENCE).unwrap());
    let before = runtime
        .as_ref()
        .unwrap()
        .export_project("resident/0")
        .unwrap();
    let reply = process_wire_request(&mut runtime, &serde_json::to_vec(&request).unwrap());
    let DesignerWireReply::Error { error } = serde_json::from_slice(&reply).unwrap() else {
        panic!("unchanged edit must have no publication")
    };
    assert_eq!(error.code, "no_change");
    assert_eq!(error.current_revision, "resident/0");
    assert!(error.diagnostics.is_empty());
    assert_eq!(
        runtime
            .as_ref()
            .unwrap()
            .export_project("resident/0")
            .unwrap(),
        before
    );
}
