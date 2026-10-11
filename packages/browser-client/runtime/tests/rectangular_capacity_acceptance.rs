//! Work #507 Steward-authored acceptance. Baseline failures are not qualification.
//! The observable contract is Work #505 / merged authority PR #506.
use std::{env, fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, ImportFieldType, ImportSelection,
    InteropMetadata, OccurrenceProjection, ScalarEditInput, StoredValueProjection, TableProjection,
    import_workbook,
    interop_adapter::{
        ImportOptions, SourceCell, SourceColumn, SourceSheet, SourceValue, SourceWorkbook,
        export_csv, export_xlsx, import_csv, import_xlsx,
    },
    open_imported_project, process_wire_request,
};
use tachiko_storage::to_canonical_string;
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const ORIGINAL: &str = "00000000-0000-4000-8000-000000000507";
const FRESH: &str = "00000000-0000-4000-8000-000000000508";

#[test]
fn native_explicit_320_by_2_number_confirmation() {
    // Independent source model isolates explicit selection from CSV inspection.
    // CSV lexical fidelity and both complete source lifecycles are checked below.
    let source = SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Synthetic".into(),
            has_header: true,
            columns: ["date", "amount"]
                .map(|name| SourceColumn {
                    name: name.into(),
                    width: None,
                })
                .to_vec(),
            rows: (0..320)
                .map(|row| {
                    [format!("000-date-{row:03}"), "-13.25".into()]
                        .map(|value| SourceCell {
                            value: SourceValue::Text { value },
                            ..SourceCell::default()
                        })
                        .to_vec()
                })
                .collect(),
        }],
        ledger: vec![],
    };
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Number]],
        extra_columns: vec![vec![]],
    };
    let (_, imported) = import_workbook(&source, &selection, ORIGINAL)
        .expect("valid explicit 320 by 2 Text/Number confirmation must be admitted");
    assert_eq!(imported.opened.table.rows.len(), 320);
    assert_eq!(imported.opened.table.columns[0].field_type, "text");
    assert_eq!(imported.opened.table.columns[1].field_type, "number");
    for (row, actual) in imported.opened.table.rows.iter().enumerate() {
        assert_eq!(actual.fields.len(), 2);
        assert_eq!(
            actual.fields[0].stored,
            Some(StoredValueProjection::Text {
                value: format!("000-date-{row:03}"),
            })
        );
        assert_eq!(
            actual.fields[1].stored,
            Some(StoredValueProjection::Number { value: -13.25 })
        );
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Expected {
    values: Vec<Vec<Option<StoredValueProjection>>>,
    headers: Vec<String>,
    kinds: Vec<String>,
    rows: Vec<String>,
    columns: Vec<String>,
    metadata: InteropMetadata,
}

fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.into(),
        })
        .unwrap()
    else {
        panic!("complete table response");
    };
    table
}

fn verify(actual: &TableProjection, expected: &Expected) {
    assert_eq!(actual.rows.len(), expected.values.len());
    assert_eq!(actual.collection.entity_count, expected.values.len());
    assert_eq!(actual.columns.len(), expected.kinds.len());
    for (c, column) in actual.columns.iter().enumerate() {
        assert_eq!(column.field_type, expected.kinds[c]);
        assert_eq!(column.id, expected.columns[c]);
        assert_eq!(column.id, expected.metadata.sheets[0].columns[c].field_id);
        assert_eq!(
            expected.headers[c],
            expected.metadata.sheets[0].columns[c].name
        );
    }
    for (r, row) in actual.rows.iter().enumerate() {
        assert_eq!(row.id, expected.rows[r]);
        assert_eq!(row.id, expected.metadata.sheets[0].rows[r].entity_id);
        // The established projection omits missing cells. Verify the exact
        // ordered present subset; every absent logical position stays absent.
        let present = actual
            .columns
            .iter()
            .enumerate()
            .filter(|(c, _)| expected.values[r][*c].is_some())
            .collect::<Vec<_>>();
        assert_eq!(row.fields.len(), present.len());
        for (field, (c, column)) in row.fields.iter().zip(present) {
            assert_eq!(field.target.entity, row.id);
            assert_eq!(field.target.field, column.id);
            assert_eq!(field.stored, expected.values[r][c], "cell {r},{c}");
            assert!(field.formula.is_none());
            assert_eq!(
                field.diagnostics,
                [] as [tachiko_designer_runtime::DiagnosticProjection; 0]
            );
        }
    }
}

#[allow(clippy::too_many_lines)] // One generated rectangular source and its independent typed oracle stay together.
fn fixture(rows: usize, columns: usize, mixed: bool) -> (Vec<u8>, ImportSelection, Expected) {
    let headers: Vec<String> = match columns {
        2 => vec!["date".into(), "amount".into()],
        3 if !mixed => ["account_id", "profile_url", "unix_timestamp"]
            .map(String::from)
            .to_vec(),
        _ => (0..columns).map(|c| format!("f{c:02}")).collect(),
    };
    let types: Vec<ImportFieldType> = (0..columns)
        .map(|c| {
            if mixed && c % 2 == 1 {
                ImportFieldType::Number
            } else {
                ImportFieldType::Text
            }
        })
        .collect();
    let kinds = types
        .iter()
        .map(|t| match t {
            ImportFieldType::Number => "number".into(),
            ImportFieldType::Text => "text".into(),
            ImportFieldType::Boolean | ImportFieldType::Date => {
                unreachable!("fixture selects only Text and Number")
            }
        })
        .collect();
    let mut values = Vec::new();
    let mut source = String::new();
    for cells in std::iter::once(headers.clone()).chain((0..rows).map(|r| {
        let expected: Vec<Option<StoredValueProjection>> = types
            .iter()
            .enumerate()
            .map(|(c, t)| {
                if r > 0 && (r + c) % 113 == 0 {
                    return None;
                }
                Some(match t {
                    ImportFieldType::Number => StoredValueProjection::Number {
                        value: match r % 7 {
                            0 => 0.0,
                            1 => -13.25,
                            2 => 42.5,
                            3 => 1_000_000.0,
                            4 => 0.000_001,
                            5 => -0.000_000_1,
                            _ => f64::from(u32::try_from(r).unwrap()) + 0.25,
                        },
                    },
                    ImportFieldType::Text => StoredValueProjection::Text {
                        value: if r % 97 == 0 {
                            format!("000{r:05}_{c},雪\"quote\"\nLF\r\nCRLF")
                        } else {
                            format!("000{r:05}_{c}")
                        },
                    },
                    ImportFieldType::Boolean | ImportFieldType::Date => {
                        unreachable!("fixture selects only Text and Number")
                    }
                })
            })
            .collect();
        let lexical = expected
            .iter()
            .map(|value| match value {
                None => String::new(),
                Some(StoredValueProjection::Text { value }) => value.clone(),
                Some(StoredValueProjection::Number { value }) => value.to_string(),
                Some(
                    StoredValueProjection::Boolean { .. }
                    | StoredValueProjection::Date { .. }
                    | StoredValueProjection::Reference { .. },
                ) => unreachable!("fixture produces only Text and Number"),
            })
            .collect();
        values.push(expected);
        lexical
    })) {
        writeln!(
            source,
            "{}\r",
            cells
                .iter()
                .map(|value| format!("\"{}\"", value.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(",")
        )
        .unwrap();
    }
    // Keep the original bytes: a negative lexical Text is intentionally not
    // CSV-exportable, while explicit Number selection may safely accept it.
    let source_bytes = source.into_bytes();
    let source = import_csv(&source_bytes, &ImportOptions::default()).unwrap();
    // Capture source metadata independently only after a successful typed import.
    let selection = ImportSelection {
        column_types: vec![types],
        extra_columns: vec![vec![]],
    };
    let (_, imported) = import_workbook(&source, &selection, ORIGINAL).unwrap();
    let expected = Expected {
        values,
        headers,
        kinds,
        rows: imported
            .opened
            .table
            .rows
            .iter()
            .map(|r| r.id.clone())
            .collect(),
        columns: imported
            .opened
            .table
            .columns
            .iter()
            .map(|c| c.id.clone())
            .collect(),
        metadata: imported.metadata,
    };
    (source_bytes, selection, expected)
}

fn exports(runtime: &DesignerRuntime, metadata: &InteropMetadata) -> [Vec<u8>; 2] {
    let workbook = runtime
        .export_workbook(&runtime.observe_occurrence().revision, metadata)
        .unwrap();
    [
        export_csv(&workbook.sheets[0]).unwrap(),
        export_xlsx(&workbook).unwrap(),
    ]
}

fn project_bytes(runtime: &DesignerRuntime, carrier: &str) -> Vec<u8> {
    let revision = runtime.observe_occurrence().revision;
    match carrier {
        "opaque" => runtime.export_project(&revision).unwrap().bytes,
        "canonical" => runtime.export_canonical_tree(&revision).unwrap().bytes,
        _ => panic!("unknown project carrier {carrier}"),
    }
}

#[allow(clippy::too_many_arguments)] // Each argument is an explicit member of the retained-source snapshot.
fn assert_resident_unchanged(
    resident: &mut Option<DesignerRuntime>,
    collection: &str,
    expected: &Expected,
    occurrence: &OccurrenceProjection,
    table_before: &TableProjection,
    exports_before: &[Vec<u8>; 2],
    carrier: &str,
    project_before: &[u8],
) {
    let runtime = resident.as_mut().unwrap();
    assert_eq!(runtime.observe_occurrence(), occurrence.clone());
    assert_eq!(table(runtime, collection), table_before.clone());
    verify(&table(runtime, collection), expected);
    assert_eq!(
        exports(runtime, &expected.metadata).as_slice(),
        exports_before.as_slice()
    );
    assert_eq!(project_bytes(runtime, carrier), project_before);
}

fn publish(runtime: &mut DesignerRuntime, request: DesignerRequest) {
    let before = runtime.observe_occurrence();
    let DesignerResponse::Published(result) = runtime.handle(request).unwrap() else {
        panic!("publication response");
    };
    assert_eq!(result.base_revision, before.revision);
    assert_ne!(result.resulting_revision, before.revision);
    assert_eq!(runtime.observe_occurrence().scope, before.scope);
    assert_eq!(
        runtime.observe_occurrence().revision,
        result.resulting_revision
    );
}

fn native_required_mixed_document(rows: usize, text_width: usize) -> Document {
    let mut document = Document::empty("native", "capacity");
    let schema = Schema {
        id: SchemaId::from("s"),
        key: SchemaKey::from("grid"),
        fields: [("text", FieldType::Text), ("number", FieldType::Number)]
            .into_iter()
            .map(|(key, field_type)| {
                let id = FieldId::from(key);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(key),
                        field_type,
                        required: true,
                        constraint: FieldConstraint::None,
                    },
                )
            })
            .collect(),
    };
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..rows {
        let id = EntityId::from(format!("r{row:03}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id,
                key: EntityKey::from(format!("k{row:03}")),
                schema: SchemaId::from("s"),
                fields: [
                    (
                        FieldId::from("text"),
                        Value::Text(format!("row{row:03}-{}", "x".repeat(text_width))),
                    ),
                    (
                        FieldId::from("number"),
                        Value::Number(
                            Number::new(f64::from(u32::try_from(row).unwrap()) + 0.25).unwrap(),
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            },
        );
    }
    document
}

#[test]
fn ordinary_native_65_to_64_preserves_history() {
    let bytes = to_canonical_string(&native_required_mixed_document(65, 8)).unwrap();
    let mut resident = None;
    let opened =
        tachiko_designer_runtime::open_local_document(&mut resident, bytes.as_bytes(), FRESH)
            .expect("ordinary small mixed native source must be admitted");
    assert_eq!(opened.table.rows.len(), 65);
    let collection = opened.bootstrap.default_collection.clone();
    let opened_wire = serde_json::to_vec(&json!({
        "status": "ok",
        "response": {"type": "opened", "payload": opened}
    }))
    .unwrap();
    assert!(opened_wire.len() <= 65_536);

    let runtime = resident.as_mut().unwrap();
    let table_before = table(runtime, &collection);
    let last = table_before.rows.last().unwrap().id.clone();
    let revision = runtime.observe_occurrence().revision;
    publish(
        runtime,
        DesignerRequest::RemoveTableRows {
            expected_revision: revision,
            collection: collection.clone(),
            entities: vec![last],
        },
    );
    assert_eq!(table(runtime, &collection).rows.len(), 64);
    let revision = runtime.observe_occurrence().revision;
    publish(
        runtime,
        DesignerRequest::Undo {
            expected_revision: revision,
        },
    );
    assert_eq!(table(runtime, &collection).rows.len(), 65);
    let revision = runtime.observe_occurrence().revision;
    publish(
        runtime,
        DesignerRequest::Redo {
            expected_revision: revision,
        },
    );
    assert_eq!(table(runtime, &collection).rows.len(), 64);
}

#[test]
#[allow(clippy::too_many_lines)] // One full atomic-refusal and retained-history acceptance trace.
fn native_required_mixed_65_to_64_refusal_preserves_resident_and_history() {
    let bytes = to_canonical_string(&native_required_mixed_document(65, 1_200)).unwrap();
    let mut resident = None;
    let opened =
        tachiko_designer_runtime::open_local_document(&mut resident, bytes.as_bytes(), FRESH)
            .expect("native required mixed 65-row source must be admitted");
    assert_eq!(opened.table.rows.len(), 65);
    let collection = opened.bootstrap.default_collection.clone();
    let opened_wire = serde_json::to_vec(&json!({
        "status": "ok",
        "response": {"type": "opened", "payload": opened}
    }))
    .unwrap();
    assert!(
        opened_wire.len() > 65_536,
        "native candidate must exercise actual complete Opened capacity"
    );

    let runtime = resident.as_mut().unwrap();
    let original = table(runtime, &collection);
    let target = original.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == "text")
        .unwrap()
        .target
        .clone();
    for value in ["history-one", "history-two"] {
        let revision = runtime.observe_occurrence().revision;
        publish(
            runtime,
            DesignerRequest::EditScalar {
                expected_revision: revision,
                target: target.clone(),
                input: ScalarEditInput::Text {
                    value: value.into(),
                },
            },
        );
    }
    let revision = runtime.observe_occurrence().revision;
    publish(
        runtime,
        DesignerRequest::Undo {
            expected_revision: revision,
        },
    );
    let table_before = table(runtime, &collection);
    let occurrence_before = runtime.observe_occurrence();
    let saved_before = runtime
        .export_project(&occurrence_before.revision)
        .unwrap()
        .bytes;
    let last = table_before.rows.last().unwrap().id.clone();

    assert!(
        runtime
            .handle(DesignerRequest::RemoveTableRows {
                expected_revision: occurrence_before.revision.clone(),
                collection: collection.clone(),
                entities: vec![last],
            })
            .is_err()
    );
    assert_eq!(runtime.observe_occurrence(), occurrence_before);
    assert_eq!(table(runtime, &collection), table_before);
    assert_eq!(
        runtime
            .export_project(&runtime.observe_occurrence().revision)
            .unwrap()
            .bytes,
        saved_before
    );

    for (redo, expected_text) in [
        (true, "history-two"),
        (false, "history-one"),
        (false, "row000-"),
        (true, "history-one"),
        (true, "history-two"),
    ] {
        let revision = runtime.observe_occurrence().revision;
        let request = if redo {
            DesignerRequest::Redo {
                expected_revision: revision,
            }
        } else {
            DesignerRequest::Undo {
                expected_revision: revision,
            }
        };
        publish(runtime, request);
        assert!(matches!(
            &table(runtime, &collection).rows[0]
                .fields
                .iter()
                .find(|field| field.target.field == "text")
                .unwrap()
                .stored,
            Some(StoredValueProjection::Text { value }) if value.starts_with(expected_text)
        ));
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One complete import, refusal, carrier, process-reopen and history trace.
fn native_rectangular_capacity_complete_lifecycle() {
    if let Ok(directory) = env::var("TACHIKO_RECTANGULAR_REOPEN") {
        let directory = Path::new(&directory);
        let expected: Expected =
            serde_json::from_slice(&fs::read(directory.join("expected.json")).unwrap()).unwrap();
        let saved = fs::read(directory.join("project.bin")).unwrap();
        let carrier = fs::read_to_string(directory.join("carrier.txt")).unwrap();
        let mut resident = None;
        let opened =
            open_imported_project(&mut resident, &saved, &expected.metadata, FRESH).unwrap();
        assert_ne!(
            env::var("TACHIKO_RECTANGULAR_PARENT_PID").unwrap(),
            std::process::id().to_string()
        );
        let runtime = resident.as_mut().unwrap();
        verify(&opened.table, &expected);
        verify(
            &table(runtime, &opened.bootstrap.default_collection),
            &expected,
        );
        assert_eq!(project_bytes(runtime, carrier.trim()), saved);
        for (format, bytes) in ["csv", "xlsx"]
            .into_iter()
            .zip(exports(runtime, &expected.metadata))
        {
            assert_eq!(
                bytes,
                fs::read(directory.join(format!("export.{format}"))).unwrap()
            );
            fs::write(directory.join(format!("reopened.{format}")), bytes).unwrap();
        }
        fs::write(
            directory.join("reopen-pid.txt"),
            std::process::id().to_string(),
        )
        .unwrap();
        return;
    }
    // A passing ordinary control must precede the new-profile baseline failure.
    let (_, _, control) = fixture(8, 2, true);
    assert_eq!(control.values.len(), 8);
    let root = env::var_os("TACHIKO_RECTANGULAR_EVIDENCE")
        .map_or_else(env::temp_dir, Into::into)
        .join(format!(
            "rectangular-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&root).unwrap();
    for (rows, columns, mixed) in [(320, 2, true), (1000, 16, true), (8406, 3, false)] {
        let (csv, selection, expected_template) = fixture(rows, columns, mixed);
        for format in ["csv", "xlsx"] {
            let source_workbook = import_csv(&csv, &ImportOptions::default()).unwrap();
            if mixed && columns == 2 {
                assert_eq!(
                    source_workbook.sheets[0].rows[1][1].value,
                    SourceValue::Text {
                        value: "-13.25".into()
                    },
                    "large CSV feasibility must retain negative lexical Text"
                );
            }
            let (source_runtime, source_imported) =
                import_workbook(&source_workbook, &selection, ORIGINAL).unwrap();
            let xlsx_source = if format == "xlsx" {
                let revision = source_runtime.observe_occurrence().revision;
                let typed = source_runtime
                    .export_workbook(&revision, &source_imported.metadata)
                    .unwrap();
                let bytes = export_xlsx(&typed).unwrap();
                Some(import_xlsx(&bytes).unwrap())
            } else {
                None
            };
            let source = xlsx_source.as_ref().unwrap_or(&source_workbook);
            let (runtime, imported) = import_workbook(source, &selection, ORIGINAL).unwrap();
            // IDs belong to this actual imported source and remain frozen for
            // its entire edit/save/reopen/export lifecycle.
            let mut expected = expected_template.clone();
            expected.metadata = imported.metadata;
            expected.rows = imported
                .opened
                .table
                .rows
                .iter()
                .map(|row| row.id.clone())
                .collect();
            expected.columns = imported
                .opened
                .table
                .columns
                .iter()
                .map(|column| column.id.clone())
                .collect();
            verify(&imported.opened.table, &expected);
            let collection = imported.opened.bootstrap.default_collection;
            let mut resident = Some(runtime);
            let last_edit_before = expected.values[rows - 1][0].clone();
            for (r, c) in [(0, 0), (rows / 2, columns - 1), (rows - 1, 0)] {
                let runtime = resident.as_mut().unwrap();
                let before = table(runtime, &collection);
                let target = before.rows[r]
                    .fields
                    .iter()
                    .find(|field| field.target.field == before.columns[c].id)
                    .expect("selected populated logical cell")
                    .target
                    .clone();
                let (input, value) = if expected.kinds[c] == "number" {
                    (
                        ScalarEditInput::Number {
                            input: "700.25".into(),
                        },
                        StoredValueProjection::Number { value: 700.25 },
                    )
                } else {
                    (
                        ScalarEditInput::Text {
                            value: "000-edited-雪\"\r\n".into(),
                        },
                        StoredValueProjection::Text {
                            value: "000-edited-雪\"\r\n".into(),
                        },
                    )
                };
                publish(
                    runtime,
                    DesignerRequest::EditScalar {
                        expected_revision: before.revision,
                        target,
                        input,
                    },
                );
                let undo_revision = runtime.observe_occurrence().revision;
                publish(
                    runtime,
                    DesignerRequest::Undo {
                        expected_revision: undo_revision,
                    },
                );
                verify(&table(runtime, &collection), &expected);
                let redo_revision = runtime.observe_occurrence().revision;
                publish(
                    runtime,
                    DesignerRequest::Redo {
                        expected_revision: redo_revision,
                    },
                );
                expected.values[r][c] = Some(value);
                verify(&table(runtime, &collection), &expected);
            }
            {
                let runtime = resident.as_mut().unwrap();
                let occurrence_before = runtime.observe_occurrence();
                let table_before = table(runtime, &collection);
                let exports_before = exports(runtime, &expected.metadata);
                let project_before = project_bytes(runtime, "opaque");

                // A bounded typed candidate is prospective only; it does not
                // replace the actual resident occurrence.
                {
                    let (_prospective, preview) =
                        import_workbook(source, &selection, ORIGINAL).unwrap();
                    verify(&preview.opened.table, &expected_template);
                }
                assert_resident_unchanged(
                    &mut resident,
                    &collection,
                    &expected,
                    &occurrence_before,
                    &table_before,
                    &exports_before,
                    "opaque",
                    &project_before,
                );

                let mut text_selection = selection.clone();
                text_selection.column_types[0].fill(ImportFieldType::Text);
                if mixed {
                    // Explicitly confirming negative lexical values as Text
                    // remains refused; it cannot use Number feasibility.
                    assert!(import_workbook(&source_workbook, &text_selection, ORIGINAL).is_err());
                }
                let mut unsafe_source = source_workbook.clone();
                unsafe_source.sheets[0].rows[0][0].value = SourceValue::Text {
                    value: "=1+1".into(),
                };
                assert!(import_workbook(&unsafe_source, &text_selection, ORIGINAL).is_err());
                assert_resident_unchanged(
                    &mut resident,
                    &collection,
                    &expected,
                    &occurrence_before,
                    &table_before,
                    &exports_before,
                    "opaque",
                    &project_before,
                );

                assert!(
                    import_csv(&vec![b'x'; 2 * 1024 * 1024 + 1], &ImportOptions::default())
                        .is_err()
                );
                assert_resident_unchanged(
                    &mut resident,
                    &collection,
                    &expected,
                    &occurrence_before,
                    &table_before,
                    &exports_before,
                    "opaque",
                    &project_before,
                );

                let mut malformed = project_before.clone();
                malformed[0] ^= 0xff;
                assert!(
                    open_imported_project(&mut resident, &malformed, &expected.metadata, FRESH)
                        .is_err()
                );
                assert_resident_unchanged(
                    &mut resident,
                    &collection,
                    &expected,
                    &occurrence_before,
                    &table_before,
                    &exports_before,
                    "opaque",
                    &project_before,
                );
            }
            for carrier in ["opaque", "canonical"] {
                let directory = root.join(format!("{rows}x{columns}-{format}-{carrier}"));
                fs::create_dir(&directory).unwrap();
                let runtime = resident.as_mut().unwrap();
                let saved = project_bytes(runtime, carrier);
                fs::write(directory.join("project.bin"), &saved).unwrap();
                fs::write(directory.join("carrier.txt"), carrier).unwrap();
                fs::write(
                    directory.join("expected.json"),
                    serde_json::to_vec(&expected).unwrap(),
                )
                .unwrap();
                for (name, bytes) in ["csv", "xlsx"]
                    .into_iter()
                    .zip(exports(runtime, &expected.metadata))
                {
                    fs::write(directory.join(format!("export.{name}")), bytes).unwrap();
                }
                let before = runtime.observe_occurrence();
                let before_exports = exports(runtime, &expected.metadata);
                let before_project = project_bytes(runtime, carrier);
                let query = serde_json::to_vec(&DesignerRequest::QueryTable {
                    collection: collection.clone(),
                })
                .unwrap();
                let capacity_reply = process_wire_request(&mut resident, &query);
                assert!(
                    capacity_reply.len() > 65_536,
                    "fixture must exercise an actual capacity-native reply over 64 KiB"
                );
                let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
                    &mut resident,
                    &vec![b' '; 65_537],
                ))
                .unwrap();
                assert_eq!(reply["error"]["code"], "request_too_large");
                let retained = resident.as_mut().unwrap();
                assert_eq!(retained.observe_occurrence(), before);
                verify(&table(retained, &collection), &expected);
                assert_eq!(exports(retained, &expected.metadata), before_exports);
                assert_eq!(project_bytes(retained, carrier), before_project);
                // Fresh child has no inherited runtime, occurrence, heap, or history.
                let status = Command::new(env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "native_rectangular_capacity_complete_lifecycle",
                        "--nocapture",
                    ])
                    .env("TACHIKO_RECTANGULAR_REOPEN", &directory)
                    .env(
                        "TACHIKO_RECTANGULAR_PARENT_PID",
                        std::process::id().to_string(),
                    )
                    .status()
                    .unwrap();
                assert!(
                    status.success(),
                    "fresh native reopen failed at {}",
                    directory.display()
                );
                assert_eq!(
                    project_bytes(resident.as_ref().unwrap(), carrier),
                    before_project,
                    "carrier roundtrip must not replace or mutate the resident"
                );
            }
            let runtime = resident.as_mut().unwrap();
            let undo_revision = runtime.observe_occurrence().revision;
            publish(
                runtime,
                DesignerRequest::Undo {
                    expected_revision: undo_revision,
                },
            );
            let mut prior = expected.clone();
            prior.values[rows - 1][0] = last_edit_before;
            verify(&table(runtime, &collection), &prior);
            let redo_revision = runtime.observe_occurrence().revision;
            publish(
                runtime,
                DesignerRequest::Redo {
                    expected_revision: redo_revision,
                },
            );
            verify(&table(runtime, &collection), &expected);
        }
    }
}
