use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    CellEdit, DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget, ScalarEditInput,
    open_project,
};
use tachiko_storage::{decode_roproj_v1, encode_roproj_v1, read_canonical_roproj};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Schema,
    SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000001";

fn mixed_document() -> Document {
    let tree = read_canonical_roproj(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../dogfood/operations-tracker.roproj"),
    )
    .unwrap();
    let mut document = decode_roproj_v1(&tree).unwrap();
    document.schemas.insert(
        SchemaId::from("notes"),
        Schema {
            id: SchemaId::from("notes"),
            key: SchemaKey::from("notes"),
            fields: BTreeMap::from([(
                FieldId::from("text"),
                FieldDefinition {
                    id: FieldId::from("text"),
                    key: FieldKey::from("text"),
                    field_type: FieldType::Text,
                    required: true,
                },
            )]),
        },
    );
    document.entities.insert(
        EntityId::from("note"),
        Entity {
            id: EntityId::from("note"),
            key: EntityKey::from("note"),
            schema: SchemaId::from("notes"),
            fields: BTreeMap::from([(
                FieldId::from("text"),
                Value::Text("original note".to_owned()),
            )]),
        },
    );
    document
}

fn revision(runtime: &mut DesignerRuntime, collection: &str) -> String {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .unwrap()
    else {
        panic!("table")
    };
    table.revision
}

fn edit_tracker(runtime: &mut DesignerRuntime, expected: &mut Document, text: &str) {
    let entity = expected
        .entities
        .values_mut()
        .find(|entity| entity.schema.as_str() == "tracker")
        .unwrap();
    let target = FieldTarget {
        entity: entity.id.to_string(),
        field: "task".to_owned(),
    };
    let expected_revision = revision(runtime, "tracker");
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision,
            edits: vec![CellEdit {
                target,
                input: ScalarEditInput::Text {
                    value: text.to_owned(),
                },
            }],
        })
        .unwrap();
    entity
        .fields
        .insert(FieldId::from("task"), Value::Text(text.to_owned()));
}

fn generic_request(expected_revision: String, input: ScalarEditInput) -> DesignerRequest {
    DesignerRequest::EditScalar {
        expected_revision,
        target: FieldTarget {
            entity: "note".to_owned(),
            field: "text".to_owned(),
        },
        input,
    }
}

fn edit_generic(runtime: &mut DesignerRuntime, expected: &mut Document) {
    let expected_revision = revision(runtime, "notes");
    runtime
        .handle(generic_request(
            expected_revision,
            ScalarEditInput::Text {
                value: "accepted note".to_owned(),
            },
        ))
        .unwrap();
    expected
        .entities
        .get_mut(&EntityId::from("note"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("text"),
            Value::Text("accepted note".to_owned()),
        );
}

fn assert_canonical(runtime: &mut DesignerRuntime, expected: &Document) {
    let expected_revision = revision(runtime, "tracker");
    let export = runtime.export_project(&expected_revision).unwrap();
    let tree = encode_roproj_v1(expected).unwrap();
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend_from_slice(&u32::try_from(tree.files().len()).unwrap().to_le_bytes());
    for file in tree.files() {
        bytes.extend_from_slice(&u16::try_from(file.path().len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(file.bytes().len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(file.path().as_bytes());
        bytes.extend_from_slice(file.bytes());
    }
    assert_eq!(
        export.bytes, bytes,
        "both collections must match exact canonical meaning"
    );
}

fn assert_history_unavailable(runtime: &mut DesignerRuntime, expected: &Document) {
    let expected_revision = revision(runtime, "tracker");
    assert!(
        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: expected_revision.clone()
            })
            .is_err(),
        "generic publication must invalidate tracker undo"
    );
    assert!(
        runtime
            .handle(DesignerRequest::Redo { expected_revision })
            .is_err(),
        "generic publication must invalidate tracker redo"
    );
    assert_canonical(runtime, expected);
}

#[test]
fn generic_publication_invalidates_tracker_undo_without_reverting_either_collection() {
    let mut expected = mixed_document();
    let mut runtime = DesignerRuntime::from_document(expected.clone(), OCCURRENCE).unwrap();
    edit_tracker(&mut runtime, &mut expected, "accepted tracker task");
    edit_generic(&mut runtime, &mut expected);
    assert_history_unavailable(&mut runtime, &expected);
    let expected_revision = revision(&mut runtime, "tracker");
    let export = runtime.export_project(&expected_revision).unwrap();
    let mut slot = None;
    open_project(
        &mut slot,
        &export.bytes,
        "00000000-0000-4000-8000-000000000002",
    )
    .unwrap();
    assert_canonical(slot.as_mut().unwrap(), &expected);
}

#[test]
fn intervening_generic_publication_invalidates_redo_and_starts_a_new_tracker_branch() {
    let original = mixed_document();
    let mut expected = original.clone();
    let mut runtime = DesignerRuntime::from_document(original.clone(), OCCURRENCE).unwrap();
    edit_tracker(&mut runtime, &mut expected, "undone tracker task");
    let expected_revision = revision(&mut runtime, "tracker");
    runtime
        .handle(DesignerRequest::Undo { expected_revision })
        .unwrap();
    expected = original;
    assert_canonical(&mut runtime, &expected);
    edit_generic(&mut runtime, &mut expected);
    assert_history_unavailable(&mut runtime, &expected);
    let branch_base = expected.clone();
    edit_tracker(&mut runtime, &mut expected, "new tracker branch");
    let expected_revision = revision(&mut runtime, "notes");
    runtime
        .handle(DesignerRequest::Undo { expected_revision })
        .unwrap();
    assert_canonical(&mut runtime, &branch_base);
    let expected_revision = revision(&mut runtime, "tracker");
    runtime
        .handle(DesignerRequest::Redo { expected_revision })
        .unwrap();
    assert_canonical(&mut runtime, &expected);
}

#[test]
fn rejected_and_no_change_generic_edits_and_queries_preserve_tracker_undo_and_redo() {
    let original = mixed_document();
    let mut expected = original.clone();
    let mut runtime = DesignerRuntime::from_document(original.clone(), OCCURRENCE).unwrap();
    edit_tracker(&mut runtime, &mut expected, "reversible tracker task");
    let current = revision(&mut runtime, "notes");
    for (expected_revision, input, code) in [
        (
            current.clone(),
            ScalarEditInput::Text {
                value: "original note".to_owned(),
            },
            "no_change",
        ),
        (
            current.clone(),
            ScalarEditInput::Boolean { value: true },
            "unsupported_edit",
        ),
        (
            "resident/0".to_owned(),
            ScalarEditInput::Text {
                value: "stale note".to_owned(),
            },
            "stale_revision",
        ),
    ] {
        let error = runtime
            .handle(generic_request(expected_revision, input))
            .unwrap_err();
        assert_eq!(error.failure_projection(&current).code, code);
        assert_canonical(&mut runtime, &expected);
    }
    let expected_revision = revision(&mut runtime, "tracker");
    runtime
        .handle(DesignerRequest::Undo { expected_revision })
        .unwrap();
    assert_canonical(&mut runtime, &original);
    let current = revision(&mut runtime, "notes");
    let error = runtime
        .handle(generic_request(
            current.clone(),
            ScalarEditInput::Text {
                value: "original note".to_owned(),
            },
        ))
        .unwrap_err();
    assert_eq!(error.failure_projection(&current).code, "no_change");
    let expected_revision = revision(&mut runtime, "tracker");
    runtime
        .handle(DesignerRequest::Redo { expected_revision })
        .unwrap();
    assert_canonical(&mut runtime, &expected);
}
