//! Sheet 1.0 capacity: a text-capacity occurrence (up to 8,406 rows of one
//! Text/Number table) saves as `.roproj/v3` and as a portable `.ro`, reopens
//! as the same text-capacity occurrence, stays editable and re-saves
//! byte-identically. Documents beyond both profiles refuse with a typed
//! `capacity_exceeded` with the active strict-profile limit, without replacing or
//! changing the resident occurrence.

use tachiko_designer_runtime::{
    CapacityLimit, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    ScalarEditInput, TableProjection, TableView, inspect_project, open_portable_ro, open_project,
    verify_portable_ro,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Schema, SchemaId, SchemaKey, Value,
};

const SOURCE: &str = "00000000-0000-4000-8000-000000000a01";
const REOPENED: &str = "00000000-0000-4000-8000-000000000a02";
const PORTABLE: &str = "00000000-0000-4000-8000-000000000a03";
const CAPACITY_ROWS: usize = 8_406;

/// One required three-column Text table, the shape of Sheet's CSV capacity
/// journey.
fn text_table(rows: usize) -> Document {
    let mut document = Document::empty("capacity", "Capacity");
    let schema = SchemaId::from("grid");
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("grid"),
            display_name: None,
            fields: ["a", "b", "c"]
                .into_iter()
                .map(|key| {
                    let id = FieldId::from(key);
                    (
                        id.clone(),
                        FieldDefinition {
                            id,
                            key: FieldKey::from(key),
                            display_name: None,
                            field_type: FieldType::Text,
                            required: true,
                            constraint: FieldConstraint::None,
                            number_format: None,
                        },
                    )
                })
                .collect(),
        },
    );
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:05}")),
                schema: schema.clone(),
                fields: ["a", "b", "c"]
                    .into_iter()
                    .map(|key| (FieldId::from(key), Value::Text(format!("{key}{row}"))))
                    .collect(),
            },
        );
    }
    document
}

/// The same v3 framing the runtime writes, built directly from the storage
/// codec so open is exercised on bytes the runtime did not produce.
fn independently_frame_v3(document: &Document) -> Vec<u8> {
    let tree = tachiko_storage::encode_roproj_v3(document).expect("the codec encodes the tree");
    let mut bytes = b"TWDPROJ1".to_vec();
    bytes.extend(u32::try_from(tree.files().len()).unwrap().to_le_bytes());
    for file in tree.files() {
        bytes.extend(u16::try_from(file.path().len()).unwrap().to_le_bytes());
        bytes.extend(u32::try_from(file.bytes().len()).unwrap().to_le_bytes());
        bytes.extend(file.path().as_bytes());
        bytes.extend(file.bytes());
    }
    bytes
}

/// Read the whole capacity table through bounded windows: `queryTable` now
/// refuses above 1,000 rows, so paging is the only complete read.
fn table(runtime: &mut DesignerRuntime) -> TableProjection {
    let revision = runtime.observe_occurrence().revision;
    let mut rows = Vec::new();
    let mut columns = Vec::new();
    let mut collection = None;
    let mut start = 0;
    loop {
        let DesignerResponse::TableWindow(window) = runtime
            .handle(DesignerRequest::QueryTableWindow {
                expected_revision: revision.clone(),
                collection: "grid".to_owned(),
                view: TableView::default(),
                start,
                count: 256,
            })
            .expect("the capacity table is windowable")
        else {
            panic!("expected a table window");
        };
        if collection.is_none() {
            columns = window.columns;
            collection = Some(window.collection);
        }
        let received = window.rows.len();
        rows.extend(window.rows);
        start += received;
        if received == 0 || start >= window.total_rows {
            break;
        }
    }
    TableProjection {
        revision: String::from("normalized"),
        tracker_profile: None,
        native_table_profile: None,
        collection: collection.expect("the first window carries the collection"),
        columns,
        rows,
        // Windows carry no totals yet; this assembly only compares rows.
        column_totals: std::collections::BTreeMap::new(),
    }
}

fn reopen(bytes: &[u8], occurrence: &str) -> DesignerRuntime {
    let mut slot = None;
    open_project(&mut slot, bytes, occurrence).expect("the saved capacity project reopens");
    slot.expect("a successful open installs the occurrence")
}

fn edit(runtime: &mut DesignerRuntime, expected: &str, target: &str, value: &str) -> String {
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: expected.to_owned(),
            target: target.into(),
            input: ScalarEditInput::Text {
                value: value.to_owned(),
            },
        })
        .expect("a capacity cell edit publishes")
    else {
        panic!("expected a publication");
    };
    publication.resulting_revision
}

fn stored_text(runtime: &mut DesignerRuntime, row: &str, field: &str) -> Option<String> {
    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: runtime.observe_occurrence().revision,
            fields: vec![format!("{row}.{field}").as_str().into()],
        })
        .expect("one field query is within bounds")
    else {
        panic!("expected a field batch");
    };
    match &fields.fields[0].stored {
        Some(tachiko_designer_runtime::StoredValueProjection::Text { value }) => {
            Some(value.clone())
        }
        _ => None,
    }
}

#[test]
fn capacity_table_saves_as_v3_reopens_edits_and_resaves_identically() {
    let mut source = DesignerRuntime::from_document(text_table(CAPACITY_ROWS), SOURCE)
        .expect("8,406 rows of three Text columns fit the text-capacity profile");
    let saved = source
        .export_project_v3("resident/0")
        .expect("a text-capacity occurrence saves as v3");
    inspect_project(&saved.bytes).expect("the saved bytes inspect");
    assert_eq!(
        saved.bytes,
        independently_frame_v3(&text_table(CAPACITY_ROWS)),
        "the runtime writes the canonical v3 framing of the same document"
    );

    let mut reopened = reopen(&saved.bytes, REOPENED);
    assert_eq!(
        table(&mut reopened),
        table(&mut source),
        "the reopened occurrence projects like the source, paging included"
    );
    assert_eq!(
        reopened
            .export_project_v3("resident/0")
            .expect("the reopened occurrence re-saves")
            .bytes,
        saved.bytes
    );

    // Editing the last row of the reopened occurrence publishes, saves, and
    // the saved edit reopens and re-saves byte-identically.
    let last = format!("r{:05}", CAPACITY_ROWS - 1);
    assert_eq!(
        edit(&mut reopened, "resident/0", &format!("{last}.b"), "edited"),
        "resident/1"
    );
    let edited = reopened
        .export_project_v3("resident/1")
        .expect("the edited occurrence saves");
    assert_ne!(edited.bytes, saved.bytes);
    let mut again = reopen(&edited.bytes, SOURCE);
    assert_eq!(
        stored_text(&mut again, &last, "b").as_deref(),
        Some("edited")
    );
    assert_eq!(
        again
            .export_project_v3("resident/0")
            .expect("the edited reopen re-saves")
            .bytes,
        edited.bytes
    );
}

#[test]
fn capacity_table_round_trips_through_a_portable_ro() {
    let source = DesignerRuntime::from_document(text_table(CAPACITY_ROWS), SOURCE)
        .expect("the capacity document is admissible");
    let portable = source
        .export_portable_ro("resident/0")
        .expect("a text-capacity occurrence saves as portable .ro");
    verify_portable_ro(&portable.bytes).expect("the portable artifact verifies");

    let mut slot = None;
    open_portable_ro(&mut slot, &portable.bytes, PORTABLE).expect("the .ro reopens");
    let mut opened = slot.expect("a successful open installs the occurrence");
    let mut control = DesignerRuntime::from_document(text_table(CAPACITY_ROWS), SOURCE).unwrap();
    assert_eq!(table(&mut opened), table(&mut control));
    assert_eq!(
        opened
            .export_portable_ro("resident/0")
            .expect("the reopened .ro re-saves")
            .bytes,
        portable.bytes
    );
    assert_eq!(
        opened.export_project_v3("resident/0").unwrap().bytes,
        source.export_project_v3("resident/0").unwrap().bytes,
        "the .ro and the v3 project carry the same document"
    );

    let first = "r00000.a";
    assert_eq!(
        edit(&mut opened, "resident/0", first, "ro edit"),
        "resident/1"
    );
    let resaved = opened
        .export_portable_ro("resident/1")
        .expect("the edited .ro occurrence saves");
    let mut slot = None;
    open_portable_ro(&mut slot, &resaved.bytes, REOPENED).expect("the edited .ro reopens");
    let mut reopened = slot.unwrap();
    assert_eq!(
        stored_text(&mut reopened, "r00000", "a").as_deref(),
        Some("ro edit")
    );
    assert_eq!(
        reopened.export_portable_ro("resident/0").unwrap().bytes,
        resaved.bytes
    );
}

#[test]
fn one_row_over_capacity_refuses_typed_and_keeps_the_resident_occurrence() {
    let over = independently_frame_v3(&text_table(CAPACITY_ROWS + 1));
    let error = inspect_project(&over).expect_err("8,407 rows exceed both profiles");
    assert!(matches!(
        &error,
        DesignerError::CapacityExceeded(capacity)
            if capacity.limit == CapacityLimit::TableRows
                && capacity.maximum == 128
                && capacity.attempted == CAPACITY_ROWS + 1
                && capacity.collection.as_deref() == Some("grid")
    ));

    let mut resident = Some(
        DesignerRuntime::from_document(text_table(CAPACITY_ROWS), SOURCE)
            .expect("the capacity document is admissible"),
    );
    let before = resident.as_ref().unwrap().observe_occurrence();
    let saved = resident
        .as_ref()
        .unwrap()
        .export_project_v3("resident/0")
        .unwrap()
        .bytes;
    let error = open_project(&mut resident, &over, REOPENED)
        .expect_err("an over-capacity project never replaces the occurrence");
    assert!(matches!(
        error,
        DesignerError::CapacityExceeded(capacity)
            if capacity.limit == CapacityLimit::TableRows
                && capacity.maximum == 128
                && capacity.attempted == CAPACITY_ROWS + 1
                && capacity.collection.as_deref() == Some("grid")
    ));
    let retained = resident.as_ref().unwrap();
    assert_eq!(retained.observe_occurrence(), before);
    assert_eq!(
        retained.export_project_v3("resident/0").unwrap().bytes,
        saved
    );
}

#[test]
fn strict_profile_documents_keep_the_original_profile() {
    // 128 rows fit the strict profile, so the eligible shape still reopens in
    // it rather than in the text-capacity profile.
    let mut source = DesignerRuntime::from_document(text_table(128), SOURCE).unwrap();
    let saved = source.export_project_v3("resident/0").unwrap();
    let mut reopened = reopen(&saved.bytes, REOPENED);
    let strict = table(&mut reopened);
    assert_eq!(strict, table(&mut source));
    assert_eq!(
        strict.rows.len(),
        128,
        "the strict profile projects every row"
    );
    assert_eq!(
        reopened.export_project_v3("resident/0").unwrap().bytes,
        saved.bytes
    );
}
