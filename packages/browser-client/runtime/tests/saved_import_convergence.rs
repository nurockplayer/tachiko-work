//! Proposed supplementary regression source for Work #495; UNCOMPILED/UNRUN.
//! No prior f33 receipt supplies evidence for this file. Expected outcomes come
//! from owner contract5999679572 and adopted disposition6024421600.
use std::io::{Cursor, Read};
use tachiko_designer_runtime::interop_adapter::{ImportOptions, SourceWorkbook, import_csv};
use tachiko_designer_runtime::{
    CleanupOperation, CollectionSummary, ColumnProjection, DesignerError, DesignerRequest,
    DesignerResponse, DesignerRuntime, FieldProjection, FieldTarget, ImportFieldType,
    ImportSelection, InteropMetadata, RowProjection, ScalarEditInput, ScalarKind,
    StoredValueProjection, TableProjection, import_workbook, inspect_imported_project,
    open_imported_project, open_project,
};
use tachiko_storage::{CanonicalRoProjectV1, decode_roproj_v1, from_bytes};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Schema, SchemaId, SchemaKey, Value,
};
const ORIGINAL: &str = "00000000-0000-4000-8000-000000000161";
const FRESH: &str = "00000000-0000-4000-8000-000000000162";
const SECOND: &str = "00000000-0000-4000-8000-000000000163";

fn native_document(rows: usize) -> Document {
    let mut document = Document::empty("d", "capacity");
    let schema = Schema {
        id: SchemaId::from("s"),
        key: SchemaKey::from("grid"),
        fields: ["a", "b", "c"]
            .into_iter()
            .map(|key| {
                let id = FieldId::from(key);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(key),
                        field_type: FieldType::Text,
                        required: true,
                        constraint: FieldConstraint::None,
                    },
                )
            })
            .collect(),
    };
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("s"),
                fields: ["a", "b", "c"]
                    .into_iter()
                    .map(|key| (FieldId::from(key), Value::Text("x".into())))
                    .collect(),
            },
        );
    }
    document
}

fn imported(rows: usize) -> (DesignerRuntime, InteropMetadata) {
    // Exact independently recorded compact saved-copy counterexample5999450017.
    let mut csv = String::from("a,b,c\r\n");
    for i in 0..rows {
        let v = if i < 2 { 0 } else { i };
        csv.push_str(&format!("key{v:03},value{v:03},00{v:03}\r\n"));
    }
    let source = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text; 3]],
        extra_columns: vec![vec![]],
    };
    let (runtime, projection) = import_workbook(&source, &selection, ORIGINAL).unwrap();
    assert_eq!(projection.opened.table.rows.len(), rows);
    (runtime, projection.metadata)
}
fn bootstrap(runtime: &mut DesignerRuntime) -> tachiko_designer_runtime::BootstrapProjection {
    let DesignerResponse::Bootstrap(bootstrap) = runtime
        .handle(DesignerRequest::Bootstrap {
            occurrence_id: FRESH.into(),
        })
        .unwrap()
    else {
        panic!("expected bootstrap")
    };
    bootstrap
}
fn table(runtime: &mut DesignerRuntime) -> TableProjection {
    let collection = bootstrap(runtime).default_collection;
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable { collection })
        .unwrap()
    else {
        panic!("expected complete table")
    };
    table
}
fn opaque(runtime: &DesignerRuntime) -> Vec<u8> {
    runtime
        .export_project(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes
}
fn document(runtime: &DesignerRuntime) -> Document {
    // Independent strict storage reader, with only existing transport framing
    // decoded here. Text-only fixtures must remain v1; no permissive fallback.
    let bytes = opaque(runtime);
    if let Some(payload) = bytes.strip_prefix(b"TWDPROJ2") {
        return from_bytes(payload).unwrap();
    }
    let mut cursor = Cursor::new(&bytes);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"TWDPROJ1");
    let mut count = [0; 4];
    cursor.read_exact(&mut count).unwrap();
    let count = u32::from_le_bytes(count);
    assert!(count <= 1024);
    let mut files = Vec::new();
    for _ in 0..count {
        let mut path_len = [0; 2];
        let mut body_len = [0; 4];
        cursor.read_exact(&mut path_len).unwrap();
        cursor.read_exact(&mut body_len).unwrap();
        let mut path = vec![0; usize::from(u16::from_le_bytes(path_len))];
        let mut body = vec![0; usize::try_from(u32::from_le_bytes(body_len)).unwrap()];
        cursor.read_exact(&mut path).unwrap();
        cursor.read_exact(&mut body).unwrap();
        files.push((String::from_utf8(path).unwrap(), body));
    }
    assert_eq!(cursor.position() as usize, bytes.len());
    decode_roproj_v1(&CanonicalRoProjectV1::try_from_files(files).unwrap()).unwrap()
}
fn carriers(runtime: &DesignerRuntime) -> [Vec<u8>; 2] {
    let canonical = runtime
        .export_canonical_tree(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes;
    // These separately obtained saved carriers can encode the same Text-only
    // document identically. Actual client/file-entry routing is a separate gate.
    [canonical, opaque(runtime)]
}
fn workbook(runtime: &DesignerRuntime, metadata: &InteropMetadata) -> SourceWorkbook {
    runtime
        .export_workbook(&runtime.observe_occurrence().revision, metadata)
        .unwrap()
}
fn edit(runtime: &mut DesignerRuntime, target: &FieldTarget, text: &str) {
    let response = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: runtime.observe_occurrence().revision,
            target: target.clone(),
            input: ScalarEditInput::Text { value: text.into() },
        })
        .unwrap();
    assert!(matches!(response, DesignerResponse::Published(_)));
}
fn history(runtime: &mut DesignerRuntime, redo: bool) {
    let expected_revision = runtime.observe_occurrence().revision;
    let request = if redo {
        DesignerRequest::Redo { expected_revision }
    } else {
        DesignerRequest::Undo { expected_revision }
    };
    assert!(matches!(
        runtime.handle(request).unwrap(),
        DesignerResponse::Published(_)
    ));
}
fn seed_histories(runtime: &mut DesignerRuntime) -> Vec<Document> {
    let original = document(runtime);
    // Last row: preserve the first duplicate pair for Dedup's falsifier.
    let target = table(runtime).rows.last().unwrap().fields[0].target.clone();
    edit(runtime, &target, "history-one");
    let one = document(runtime);
    edit(runtime, &target, "history-two");
    let two = document(runtime);
    history(runtime, false);
    assert_eq!(document(runtime), one);
    vec![original, one, two]
}
fn assert_histories(runtime: &mut DesignerRuntime, states: &[Document]) {
    for (redo, index) in [(true, 2), (false, 1), (false, 0), (true, 1), (true, 2)] {
        history(runtime, redo);
        assert_eq!(document(runtime), states[index]);
    }
}

#[test]
fn new_saved_open_preserves_both_carriers_and_complete_edit_history_resave() {
    let (runtime, metadata) = imported(66);
    let original_document = document(&runtime);
    let original_workbook = workbook(&runtime, &metadata);
    for carrier in carriers(&runtime) {
        let inspection = inspect_imported_project(&carrier, &metadata).unwrap();
        assert_eq!(inspection.table.rows.len(), 66);
        let mut resident = None;
        let opened = open_imported_project(&mut resident, &carrier, &metadata, FRESH).unwrap();
        let current = resident.as_mut().unwrap();
        assert_eq!(
            opened,
            tachiko_designer_runtime::OpenedProjection {
                bootstrap: bootstrap(current),
                table: table(current),
            }
        );
        assert_eq!(document(current), original_document);
        assert_eq!(workbook(current, &metadata), original_workbook);
        let target = table(current).rows[2].fields[1].target.clone();
        edit(current, &target, "雪&<edited>");
        let changed = document(current);
        history(current, false);
        assert_eq!(document(current), original_document);
        history(current, true);
        assert_eq!(document(current), changed);
        let changed_workbook = workbook(current, &metadata);
        let saved = carriers(current);
        // Drop the complete runtime. This tests fresh occurrence reconstruction;
        // separate OS-process/Worker restart is an additional retained gate.
        drop(resident);
        for resaved in saved {
            let mut fresh = None;
            open_imported_project(&mut fresh, &resaved, &metadata, SECOND).unwrap();
            let reopened = fresh.as_ref().unwrap();
            assert_eq!(document(reopened), changed);
            assert_eq!(workbook(reopened, &metadata), changed_workbook);
            assert_eq!(metadata.sheets[0].rows.len(), 66);
        }
    }
}

#[test]
fn invalid_saved_metadata_refuses_before_replacement_and_retains_both_histories() {
    let (source, metadata) = imported(66);
    let mut invalid = Vec::new();
    let mut m = metadata.clone();
    m.version = 2;
    invalid.push(m);
    let mut m = metadata.clone();
    m.sheets[0].rows.pop();
    invalid.push(m);
    let mut m = metadata.clone();
    m.sheets[0].rows.swap(0, 1);
    invalid.push(m);
    let mut m = metadata.clone();
    m.sheets[0].columns.swap(0, 1);
    invalid.push(m);
    let mut m = metadata.clone();
    m.sheets[0].rows[0].entity_id = "absent".into();
    invalid.push(m);
    let mut m = metadata.clone();
    m.sheets[0].columns[0].field_id = "absent".into();
    invalid.push(m);
    for carrier in carriers(&source) {
        for bad in &invalid {
            let mut empty = None;
            assert!(open_imported_project(&mut empty, &carrier, bad, FRESH).is_err());
            assert!(empty.is_none());
            let (runtime, resident_metadata) = imported(8);
            let mut resident = Some(runtime);
            let current = resident.as_mut().unwrap();
            let states = seed_histories(current);
            let occurrence = current.observe_occurrence();
            let all_cells = table(current);
            let saved = carriers(current);
            let mapped = workbook(current, &resident_metadata);
            assert!(inspect_imported_project(&carrier, bad).is_err());
            assert!(open_imported_project(&mut resident, &carrier, bad, FRESH).is_err());
            let retained = resident.as_mut().unwrap();
            assert_eq!(retained.observe_occurrence(), occurrence);
            assert_eq!(table(retained), all_cells);
            assert_eq!(carriers(retained), saved);
            assert_eq!(workbook(retained, &resident_metadata), mapped);
            assert_histories(retained, &states);
        }
    }
}

fn dedup(
    runtime: &mut DesignerRuntime,
) -> Result<DesignerResponse, tachiko_designer_runtime::DesignerError> {
    let all = table(runtime);
    let expected_revision = runtime.observe_occurrence().revision;
    let response = runtime.handle(DesignerRequest::PreviewCleanup {
        expected_revision,
        operation: CleanupOperation::Deduplicate {
            entities: all.rows.iter().map(|r| r.id.clone()).collect(),
            key_fields: all.columns.iter().map(|c| c.id.clone()).collect(),
        },
    })?;
    let DesignerResponse::CleanupPreview(preview) = response else {
        panic!("expected preview")
    };
    runtime.handle(DesignerRequest::CommitCleanup {
        expected_revision: preview.revision,
        preview_id: preview.preview_id,
    })
}

#[test]
fn enlarged_import_dedup_66_to_65_and_65_to_64_refuses_atomically() {
    for rows in [66, 65] {
        let (source, metadata) = imported(rows);
        for carrier in carriers(&source) {
            let mut resident = None;
            open_imported_project(&mut resident, &carrier, &metadata, FRESH).unwrap();
            let runtime = resident.as_mut().unwrap();
            let states = seed_histories(runtime);
            let occurrence = runtime.observe_occurrence();
            let all_cells = table(runtime);
            let saved = carriers(runtime);
            let mapped = workbook(runtime, &metadata);
            assert!(
                dedup(runtime).is_err(),
                "no qualified metadata transition exists"
            );
            assert_eq!(runtime.observe_occurrence(), occurrence);
            assert_eq!(table(runtime), all_cells);
            assert_eq!(carriers(runtime), saved);
            assert_eq!(workbook(runtime, &metadata), mapped);
            assert_histories(runtime, &states);
        }
    }
}

#[test]
fn ordinary_eight_to_seven_control_remains_published_and_exportable() {
    let (mut runtime, metadata) = imported(8);
    let original = document(&runtime);
    assert!(matches!(
        dedup(&mut runtime).unwrap(),
        DesignerResponse::Published(_)
    ));
    assert_eq!(table(&mut runtime).rows.len(), 7);
    let changed = document(&runtime);
    assert_eq!(workbook(&runtime, &metadata).sheets[0].rows.len(), 7);
    for carrier in carriers(&runtime) {
        inspect_imported_project(&carrier, &metadata).unwrap();
    }
    history(&mut runtime, false);
    assert_eq!(document(&runtime), original);
    history(&mut runtime, true);
    assert_eq!(document(&runtime), changed);
}

#[test]
fn ordinary_native_open_keeps_generic_65_through_128_row_authoring() {
    // Existing generic opening remains separately callable; metadata-bearing
    // opening must not become a silent prerequisite for ordinary authoring.
    for rows in [65, 66, 128] {
        let mut source = DesignerRuntime::from_document(native_document(rows), ORIGINAL).unwrap();
        let original = document(&source);
        let saved = opaque(&source);
        let mut resident = None;
        open_project(&mut resident, &saved, FRESH).unwrap();
        let fresh = resident.as_mut().unwrap();
        assert_eq!(table(fresh).rows.len(), rows);
        assert_eq!(document(fresh), original);

        if rows <= 66 {
            let selected = v3_bytes(&source);
            let mut v3 = None;
            open_project(&mut v3, &selected, SECOND).unwrap();
            assert_eq!(table(v3.as_mut().unwrap()).rows.len(), rows);
            ordinary_export_refuses(v3.as_ref().unwrap());
            continue;
        }

        // The expected full Table projection comes directly from the unchanged
        // document and the established projection semantics, not from the
        // candidate runtime's result.
        let expected = expected_native_table(&original, &format!("{FRESH}/0"));
        let expected_bytes = serde_json::to_vec(&expected).unwrap();
        assert_eq!(expected_bytes.len(), 73_213);
        assert!(expected_bytes.len() > 65_536);
        assert_eq!(table(fresh), expected);

        // A valid storage-v3 document is independently framed from the same
        // original snapshot. Selected v3 export refuses this Table envelope.
        let before_export_occurrence = source.observe_occurrence();
        let before_export_bootstrap = bootstrap(&mut source);
        let before_export_table = table(&mut source);
        let before_export_saved = carriers(&source);
        let export_error = source
            .export_project_v3(&before_export_occurrence.revision)
            .expect_err("the original Table projection exceeds the native reply limit");
        assert!(matches!(
            export_error,
            DesignerError::UnsupportedProject { .. }
        ));
        assert_eq!(source.observe_occurrence(), before_export_occurrence);
        assert_eq!(bootstrap(&mut source), before_export_bootstrap);
        assert_eq!(table(&mut source), before_export_table);
        assert_eq!(carriers(&source), before_export_saved);

        let encoded = independently_frame_v3(&original);
        let inspect_error = tachiko_designer_runtime::inspect_project(&encoded)
            .expect_err("v3 Inspect refuses the original oversized Table projection");
        assert!(matches!(
            inspect_error,
            DesignerError::UnsupportedProject { .. }
        ));
        let mut empty = None;
        let open_error = open_project(&mut empty, &encoded, FRESH)
            .expect_err("v3 open refuses the original oversized Table projection");
        assert!(matches!(
            open_error,
            DesignerError::UnsupportedProject { .. }
        ));
        assert!(empty.is_none());

        let (mut seeded, metadata) = imported(8);
        let states = seed_histories(&mut seeded);
        let before_occurrence = seeded.observe_occurrence();
        let before_bootstrap = bootstrap(&mut seeded);
        let before_table = table(&mut seeded);
        let before_saved = carriers(&seeded);
        let before_workbook = workbook(&seeded, &metadata);
        let mut seeded = Some(seeded);
        let open_error = open_project(&mut seeded, &encoded, FRESH)
            .expect_err("v3 open refusal preserves the seeded resident");
        assert!(matches!(
            open_error,
            DesignerError::UnsupportedProject { .. }
        ));
        let retained = seeded.as_mut().unwrap();
        assert_eq!(retained.observe_occurrence(), before_occurrence);
        assert_eq!(bootstrap(retained), before_bootstrap);
        assert_eq!(table(retained), before_table);
        assert_eq!(carriers(retained), before_saved);
        assert_eq!(workbook(retained, &metadata), before_workbook);
        assert_histories(retained, &states);
    }
}

fn expected_native_table(document: &Document, revision: &str) -> TableProjection {
    let schema = document.schemas.get(&SchemaId::from("s")).unwrap();
    let entities = document
        .entities
        .values()
        .filter(|entity| entity.schema == schema.id)
        .collect::<Vec<_>>();
    TableProjection {
        revision: revision.to_owned(),
        tracker_profile: None,
        native_table_profile: Some(true),
        collection: CollectionSummary {
            id: schema.id.to_string(),
            key: schema.key.to_string(),
            entity_count: entities.len(),
        },
        columns: schema
            .fields
            .values()
            .map(|field| ColumnProjection {
                id: field.id.to_string(),
                key: field.key.to_string(),
                field_type: "text".to_owned(),
                dropdown_options: None,
            })
            .collect(),
        rows: entities
            .into_iter()
            .map(|entity| RowProjection {
                id: entity.id.to_string(),
                key: entity.key.to_string(),
                fields: schema
                    .fields
                    .values()
                    .map(|definition| {
                        let value = entity.fields.get(&definition.id).unwrap();
                        let Value::Text(text) = value else {
                            panic!("the fixed native table fixture contains only Text values")
                        };
                        FieldProjection {
                            target: FieldTarget {
                                entity: entity.id.to_string(),
                                field: definition.id.to_string(),
                            },
                            address: format!("{}.{}", entity.key, definition.key),
                            stored: Some(StoredValueProjection::Text {
                                value: text.clone(),
                            }),
                            formula: None,
                            calculated: None,
                            diagnostics: Vec::new(),
                            editable_scalar: Some(ScalarKind::Text),
                        }
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn v3_bytes(runtime: &DesignerRuntime) -> Vec<u8> {
    runtime
        .export_project_v3(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes
}
fn ordinary_export_refuses(runtime: &DesignerRuntime) {
    assert_eq!(
        runtime
            .export_project(&runtime.observe_occurrence().revision)
            .unwrap_err()
            .failure_projection(&runtime.observe_occurrence().revision)
            .code,
        "unsupported_project"
    );
}
fn independently_frame_v3(document: &Document) -> Vec<u8> {
    let tree = tachiko_storage::encode_roproj_v3(document).unwrap();
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

#[test]
fn compact_capacity_runtime_keeps_original_pure_v3_eligibility_but_not_import_closure() {
    // Read-only Astra advice: judge the snapshot's original native envelope,
    // not a blanket text_capacity ban. Imported-carrier closure is separate.
    let (mut runtime, metadata) = imported(66);
    let states = seed_histories(&mut runtime);
    let before = runtime.observe_occurrence();
    let cells = table(&mut runtime);
    let ordinary_saved = carriers(&runtime);
    let selected = v3_bytes(&runtime);
    assert_eq!(selected, independently_frame_v3(&document(&runtime)));
    assert_eq!(runtime.observe_occurrence(), before);
    assert_eq!(table(&mut runtime), cells);
    assert_eq!(carriers(&runtime), ordinary_saved); // Pure export never marks origin.
    let mut plain = None;
    open_project(&mut plain, &selected, FRESH).unwrap(); // Original native control.
    assert_eq!(table(plain.as_mut().unwrap()).rows.len(), 66);
    ordinary_export_refuses(plain.as_ref().unwrap());
    let mut empty = None;
    assert!(inspect_imported_project(&selected, &metadata).is_err());
    assert!(open_imported_project(&mut empty, &selected, &metadata, SECOND).is_err());
    assert!(empty.is_none());
    let mut resident = Some(runtime);
    assert!(open_imported_project(&mut resident, &selected, &metadata, SECOND).is_err());
    let retained = resident.as_mut().unwrap();
    assert_eq!(retained.observe_occurrence(), before);
    assert_eq!(table(retained), cells);
    assert_eq!(carriers(retained), ordinary_saved);
    assert_histories(retained, &states);
}

#[test]
fn metadata_aware_bounded_v3_open_preserves_origin_through_history_and_failed_replacement() {
    let (source, metadata) = imported(8);
    let selected = v3_bytes(&source);
    // Storage framing is validated independently before exercising the API.
    assert_eq!(selected, independently_frame_v3(&document(&source)));
    inspect_imported_project(&selected, &metadata).unwrap();
    let mut resident = None;
    open_imported_project(&mut resident, &selected, &metadata, FRESH).unwrap();
    let current = resident.as_mut().unwrap();
    ordinary_export_refuses(current);
    bootstrap(current);
    ordinary_export_refuses(current);
    let original = v3_bytes(current);
    let target = table(current).rows[2].fields[1].target.clone();
    edit(current, &target, "v3-history-one");
    ordinary_export_refuses(current);
    let one = v3_bytes(current);
    edit(current, &target, "v3-history-two");
    let two = v3_bytes(current);
    history(current, false);
    ordinary_export_refuses(current);
    assert_eq!(v3_bytes(current), one);
    let occurrence = current.observe_occurrence();
    let all_cells = table(current);
    let mapped = workbook(current, &metadata);
    let mut bad = metadata.clone();
    bad.version = 2;
    assert!(open_imported_project(&mut resident, &selected, &bad, SECOND).is_err());
    let retained = resident.as_mut().unwrap();
    assert_eq!(retained.observe_occurrence(), occurrence);
    assert_eq!(table(retained), all_cells);
    assert_eq!(workbook(retained, &metadata), mapped);
    ordinary_export_refuses(retained);
    for (redo, expected) in [
        (true, &two),
        (false, &one),
        (false, &original),
        (true, &one),
        (true, &two),
    ] {
        history(retained, redo);
        ordinary_export_refuses(retained);
        assert_eq!(v3_bytes(retained), *expected);
    }
    let reopened_bytes = v3_bytes(retained);
    drop(resident);
    let mut fresh = None;
    open_imported_project(&mut fresh, &reopened_bytes, &metadata, SECOND).unwrap();
    ordinary_export_refuses(fresh.as_ref().unwrap());
    assert_eq!(v3_bytes(fresh.as_ref().unwrap()), reopened_bytes);
}

#[test]
fn enlarged_v3_cannot_enter_through_capacity_decode_open_inspect_or_selected_export() {
    let candidate = native_document(129); // Original native per-table ceiling128.
    let source = DesignerRuntime::from_document(candidate.clone(), ORIGINAL).unwrap();
    let before = opaque(&source);
    assert!(
        source
            .export_project_v3(&source.observe_occurrence().revision)
            .is_err()
    );
    assert_eq!(opaque(&source), before);
    // Strict storage codec may encode a valid v3 tree independently of Designer
    // admission. This reaches the producer boundary instead of invalid framing.
    let encoded = independently_frame_v3(&candidate);
    assert!(tachiko_designer_runtime::inspect_project(&encoded).is_err());
    let mut empty = None;
    assert!(open_project(&mut empty, &encoded, FRESH).is_err());
    assert!(empty.is_none());
    let (mut original, metadata) = imported(8);
    let states = seed_histories(&mut original);
    let occurrence = original.observe_occurrence();
    let all_cells = table(&mut original);
    let saved = carriers(&original);
    let mapped = workbook(&original, &metadata);
    let mut resident = Some(original);
    assert!(open_project(&mut resident, &encoded, FRESH).is_err());
    let retained = resident.as_mut().unwrap();
    assert_eq!(retained.observe_occurrence(), occurrence);
    assert_eq!(table(retained), all_cells);
    assert_eq!(carriers(retained), saved);
    assert_eq!(workbook(retained, &metadata), mapped);
    assert_histories(retained, &states);
}

#[test]
fn small_row_v3_cannot_bypass_original_complete_reply_limit() {
    let mut candidate = native_document(64);
    for schema in candidate.schemas.values_mut() {
        for field in schema.fields.values_mut() {
            field.key = format!("{}{}", field.key, "f".repeat(255)).into();
        }
    }
    for entity in candidate.entities.values_mut() {
        entity.key = format!("{}{}", entity.key, "k".repeat(250)).into();
    }
    // Original profile strings stay individually bounded. Repeated addresses
    // inflate the reply independently of the small stored cells/row count.
    let source = DesignerRuntime::from_document(candidate.clone(), ORIGINAL).unwrap();
    assert!(
        serde_json::to_vec(&table(
            &mut DesignerRuntime::from_document(candidate.clone(), ORIGINAL).unwrap()
        ))
        .unwrap()
        .len()
            > 65_536
    );
    assert!(
        source
            .export_project_v3(&source.observe_occurrence().revision)
            .is_err()
    );
    let encoded = independently_frame_v3(&candidate);
    assert!(tachiko_designer_runtime::inspect_project(&encoded).is_err());
    let mut empty = None;
    assert!(open_project(&mut empty, &encoded, FRESH).is_err());
    assert!(empty.is_none());
}

#[test]
fn invalid_saved_occurrence_identity_refuses_before_replacement_and_preserves_histories() {
    let (source, metadata) = imported(66);
    for carrier in carriers(&source) {
        for invalid in [
            "",
            "not-an-occurrence",
            "00000000-0000-1000-8000-000000000164",
        ] {
            let mut empty = None;
            assert!(open_imported_project(&mut empty, &carrier, &metadata, invalid).is_err());
            assert!(empty.is_none());
            let (runtime, resident_metadata) = imported(8);
            let mut resident = Some(runtime);
            let current = resident.as_mut().unwrap();
            let states = seed_histories(current);
            let occurrence = current.observe_occurrence();
            let all_cells = table(current);
            let saved = carriers(current);
            let mapped = workbook(current, &resident_metadata);
            assert!(open_imported_project(&mut resident, &carrier, &metadata, invalid).is_err());
            let retained = resident.as_mut().unwrap();
            assert_eq!(retained.observe_occurrence(), occurrence);
            assert_eq!(table(retained), all_cells);
            assert_eq!(carriers(retained), saved);
            assert_eq!(workbook(retained, &resident_metadata), mapped);
            assert_histories(retained, &states);
        }
    }
}
