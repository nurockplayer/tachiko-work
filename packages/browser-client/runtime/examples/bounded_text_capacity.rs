//! Steward acceptance for Work #489 / Sheet #150; invoke with a generated fixture
//! directory and a new capture directory. This is not a product command or SDK.
use std::{collections::BTreeMap, env, fs, path::Path, time::Instant};

use serde::Deserialize;
use serde_json::json;
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, ImportFieldType, ImportSelection,
    InteropMetadata, ScalarEditInput, StoredValueProjection, TableProjection, import_workbook,
    inspect_imported_project,
    interop_adapter::{
        ImportOptions, SourceWorkbook, export_csv, export_xlsx, import_csv, import_xlsx,
    },
    open_imported_project, open_project, process_wire_request,
};

const ROWS: usize = 8_406;
const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000001";
const REOPENED: &str = "00000000-0000-4000-8000-000000000002";

#[derive(Deserialize)]
struct Manifest {
    headers: Vec<String>,
    types: Vec<String>,
    rows: Vec<Vec<String>>,
    edits: Vec<(usize, usize, String)>,
}

#[derive(Default)]
struct Timings(BTreeMap<String, Vec<f64>>);

impl Timings {
    fn measure<T>(&mut self, name: &str, action: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = action();
        self.0
            .entry(name.into())
            .or_default()
            .push(start.elapsed().as_secs_f64() * 1000.0);
        result
    }

    fn save(&self, path: &Path) {
        fs::write(path, serde_json::to_vec_pretty(&self.0).unwrap()).unwrap();
        for (name, samples) in &self.0 {
            let mut ordered = samples.clone();
            ordered.sort_by(f64::total_cmp);
            let max = ordered[ordered.len() - 1];
            let p95 = ordered[(ordered.len() * 95).div_ceil(100) - 1];
            if ["edit", "undo", "redo"].contains(&name.as_str()) {
                assert!(
                    p95 <= 500.0 && max <= 1000.0,
                    "{name}: p95={p95} max={max} ms"
                );
            } else {
                assert!(
                    max <= 10_000.0,
                    "{name}: {max} ms exceeds 10s reference budget"
                );
            }
        }
    }
}

fn saved_bytes(runtime: &DesignerRuntime, carrier: &str) -> Vec<u8> {
    let revision = runtime.observe_occurrence().revision;
    match carrier {
        "opaque" => runtime.export_project(&revision).unwrap().bytes,
        "canonical" => runtime.export_canonical_tree(&revision).unwrap().bytes,
        _ => panic!("unknown saved carrier"),
    }
}

fn read_manifest(fixtures: &Path) -> Manifest {
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(fixtures.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.rows.len(), ROWS);
    assert_eq!(
        manifest.headers,
        ["account_id", "profile_url", "unix_timestamp"]
    );
    assert_eq!(manifest.types, ["text", "text", "text"]);
    manifest
}

fn inspect(bytes: &[u8], format: &str) -> SourceWorkbook {
    match format {
        "csv" => import_csv(bytes, &ImportOptions::default()).unwrap(),
        "xlsx" => import_xlsx(bytes).unwrap(),
        _ => panic!("unknown format"),
    }
}

fn selection() -> ImportSelection {
    ImportSelection {
        column_types: vec![vec![ImportFieldType::Text; 3]],
        extra_columns: vec![vec![]],
    }
}

fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let DesignerResponse::Table(result) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.into(),
        })
        .unwrap()
    else {
        panic!("table response")
    };
    result
}

fn verify(table: &TableProjection, expected: &[Vec<String>], metadata: &InteropMetadata) {
    assert_eq!(table.rows.len(), expected.len());
    assert_eq!(table.collection.entity_count, expected.len());
    assert_eq!(table.columns.len(), 3);
    assert_eq!(metadata.sheets.len(), 1);
    let sheet = &metadata.sheets[0];
    assert_eq!(sheet.schema_id, table.collection.id);
    assert_eq!(sheet.rows.len(), expected.len());
    for (c, column) in table.columns.iter().enumerate() {
        assert_eq!(column.field_type, "text");
        assert_eq!(column.id, sheet.columns[c].field_id);
        assert_eq!(
            sheet.columns[c].name,
            ["account_id", "profile_url", "unix_timestamp"][c]
        );
    }
    for (r, (row, expected)) in table.rows.iter().zip(expected).enumerate() {
        assert_eq!(
            row.id, sheet.rows[r].entity_id,
            "stable row identity/order {r}"
        );
        assert_eq!(row.fields.len(), 3);
        for (c, (cell, value)) in row.fields.iter().zip(expected).enumerate() {
            assert_eq!(cell.target.entity, row.id);
            assert_eq!(cell.target.field, table.columns[c].id);
            assert_eq!(
                cell.stored,
                Some(StoredValueProjection::Text {
                    value: value.clone()
                }),
                "cell {r},{c}"
            );
            assert!(cell.formula.is_none(), "no formula admission");
            assert_eq!(cell.diagnostics.len(), 0);
        }
    }
}

fn publish(
    runtime: &mut DesignerRuntime,
    request: DesignerRequest,
    target: &tachiko_designer_runtime::FieldTarget,
    expected: &str,
) -> String {
    let before = runtime.observe_occurrence();
    let DesignerResponse::Published(result) = runtime.handle(request).unwrap() else {
        panic!("publication response")
    };
    assert_eq!(result.base_revision, before.revision);
    assert_ne!(result.resulting_revision, before.revision);
    assert_eq!(runtime.observe_occurrence().scope, before.scope);
    assert_eq!(
        runtime.observe_occurrence().revision,
        result.resulting_revision
    );
    let prior: u64 = before
        .revision
        .strip_prefix("resident/")
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(result.resulting_revision, format!("resident/{}", prior + 1));
    assert_eq!(result.fields.as_slice(), std::slice::from_ref(target));
    let DesignerResponse::Fields(fields) = runtime
        .handle(DesignerRequest::QueryFields {
            expected_revision: result.resulting_revision.clone(),
            fields: result.fields.clone(),
        })
        .unwrap()
    else {
        panic!("selective field response")
    };
    assert_eq!(fields.revision, result.resulting_revision);
    assert_eq!(fields.fields.len(), result.fields.len());
    assert_eq!(fields.fields[0].target, *target);
    assert_eq!(
        fields.fields[0].stored,
        Some(StoredValueProjection::Text {
            value: expected.into()
        })
    );
    assert!(fields.fields[0].formula.is_none());
    assert_eq!(fields.fields[0].diagnostics.len(), 0);
    result.resulting_revision
}

fn export_artifacts(
    runtime: &DesignerRuntime,
    metadata: &InteropMetadata,
    out: &Path,
    prefix: &str,
    times: &mut Timings,
) {
    let revision = runtime.observe_occurrence().revision;
    let csv = times.measure("export_csv", || {
        let workbook = runtime.export_workbook(&revision, metadata).unwrap();
        export_csv(&workbook.sheets[0]).unwrap()
    });
    let xlsx = times.measure("export_xlsx", || {
        let workbook = runtime.export_workbook(&revision, metadata).unwrap();
        export_xlsx(&workbook).unwrap()
    });
    fs::write(out.join(format!("{prefix}-export.csv")), csv).unwrap();
    fs::write(out.join(format!("{prefix}-export.xlsx")), xlsx).unwrap();
}

fn history(
    runtime: &mut DesignerRuntime,
    collection: &str,
    manifest: &mut Manifest,
    metadata: &InteropMetadata,
    times: &mut Timings,
) {
    for (r, c, value) in &manifest.edits {
        let before = table(runtime, collection);
        let target = before.rows[*r].fields[*c].target.clone();
        let revision = times.measure("edit", || {
            publish(
                runtime,
                DesignerRequest::EditScalar {
                    expected_revision: before.revision,
                    target: target.clone(),
                    input: ScalarEditInput::Text {
                        value: value.clone(),
                    },
                },
                &target,
                value,
            )
        });
        let undo_revision = times.measure("undo", || {
            publish(
                runtime,
                DesignerRequest::Undo {
                    expected_revision: revision,
                },
                &target,
                &manifest.rows[*r][*c],
            )
        });
        verify(&table(runtime, collection), &manifest.rows, metadata);
        times.measure("redo", || {
            publish(
                runtime,
                DesignerRequest::Redo {
                    expected_revision: undo_revision,
                },
                &target,
                value,
            )
        });
        manifest.rows[*r][*c].clone_from(value);
        verify(&table(runtime, collection), &manifest.rows, metadata);
    }
    // Two complete undo horizons of scalar edits. Intermediate unique values prevent
    // no-op publication from disguising retained-history growth.
    let target = table(runtime, collection).rows[0].fields[0].target.clone();
    for index in 0..128 {
        let value = if index == 127 {
            manifest.rows[0][0].clone()
        } else {
            format!("history-{index:03}")
        };
        let expected_revision = runtime.observe_occurrence().revision;
        times.measure("edit", || {
            publish(
                runtime,
                DesignerRequest::EditScalar {
                    expected_revision,
                    target: target.clone(),
                    input: ScalarEditInput::Text {
                        value: value.clone(),
                    },
                },
                &target,
                &value,
            )
        });
    }
    verify(&table(runtime, collection), &manifest.rows, metadata);
}

fn rejection_preserves_resident(
    runtime: DesignerRuntime,
    collection: &str,
    metadata: &InteropMetadata,
    manifest: &Manifest,
    saved: &[u8],
    carrier: &str,
) {
    let mut resident = Some(runtime);
    let initial = resident.as_ref().unwrap().observe_occurrence();
    let workbook = resident
        .as_ref()
        .unwrap()
        .export_workbook(&initial.revision, metadata)
        .unwrap();
    let csv = export_csv(&workbook.sheets[0]).unwrap();
    let xlsx = export_xlsx(&workbook).unwrap();
    for invalid in [
        b"malformed project".to_vec(),
        saved[..saved.len() - 1].to_vec(),
        [saved, b"trailing"].concat(),
    ] {
        let before = resident.as_ref().unwrap().observe_occurrence();
        assert!(open_project(&mut resident, &invalid, REOPENED).is_err());
        assert_eq!(resident.as_ref().unwrap().observe_occurrence(), before);
        assert_eq!(
            saved_bytes(resident.as_ref().unwrap(), carrier),
            saved
        );
        verify(
            &table(resident.as_mut().unwrap(), collection),
            &manifest.rows,
            metadata,
        );
        let runtime = resident.as_mut().unwrap();
        let workbook = runtime.export_workbook(&before.revision, metadata).unwrap();
        assert_eq!(export_csv(&workbook.sheets[0]).unwrap(), csv);
        assert_eq!(export_xlsx(&workbook).unwrap(), xlsx);
        let target = table(runtime, collection).rows[0].fields[0].target.clone();
        let undone = publish(
            runtime,
            DesignerRequest::Undo {
                expected_revision: before.revision,
            },
            &target,
            "history-126",
        );
        let mut previous = manifest.rows.clone();
        previous[0][0] = "history-126".into();
        verify(&table(runtime, collection), &previous, metadata);
        publish(
            runtime,
            DesignerRequest::Redo {
                expected_revision: undone,
            },
            &target,
            &manifest.rows[0][0],
        );
        verify(&table(runtime, collection), &manifest.rows, metadata);
    }
    let before = resident.as_ref().unwrap().observe_occurrence();
    let reply: serde_json::Value =
        serde_json::from_slice(&process_wire_request(&mut resident, &vec![b' '; 65_537])).unwrap();
    assert_eq!(reply["status"], "error");
    assert_eq!(reply["error"]["code"], "request_too_large");
    assert_eq!(resident.as_ref().unwrap().observe_occurrence(), before);
    assert_eq!(
        saved_bytes(resident.as_ref().unwrap(), carrier),
        saved
    );
}

fn reopen(fixtures: &Path, capture: &Path, format: &str, carrier: &str) {
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(capture.join("native-import-complete.json")).unwrap())
            .unwrap();
    let old_pid = original["pid"].as_u64().unwrap();
    assert_ne!(old_pid, u64::from(std::process::id()));
    assert!(
        !Path::new(&format!("/proc/{old_pid}")).exists(),
        "original native process must have terminated"
    );
    let mut manifest = read_manifest(fixtures);
    for (r, c, value) in &manifest.edits {
        manifest.rows[*r][*c].clone_from(value);
    }
    let bytes = fs::read(capture.join(format!("{format}-saved.project"))).unwrap();
    let metadata: InteropMetadata =
        serde_json::from_slice(&fs::read(capture.join(format!("{format}-metadata.json"))).unwrap())
            .unwrap();
    let collection = fs::read_to_string(capture.join(format!("{format}-collection.txt"))).unwrap();
    let mut times = Timings::default();
    let mut resident = None;
    assert_eq!(original["carrier"], carrier);
    let mut scopes = std::collections::BTreeSet::new();
    scopes.insert(original["scopes"][format].as_str().unwrap().to_owned());
    for cycle in 0..10 {
        let occurrence = format!("00000000-0000-4000-8000-{:012}", cycle + 2);
        let opened = times.measure("reopen", || {
            open_imported_project(&mut resident, &bytes, &metadata, &occurrence).unwrap()
        });
        verify(&opened.table, &manifest.rows, &metadata);
        let runtime = resident.as_mut().unwrap();
        assert!(scopes.insert(runtime.observe_occurrence().scope));
        assert_eq!(runtime.observe_occurrence().revision, "resident/0");
        verify(&table(runtime, &collection), &manifest.rows, &metadata);
        assert_eq!(
            times.measure("resave", || saved_bytes(runtime, carrier)),
            bytes
        );
        if cycle == 9 {
            export_artifacts(
                runtime,
                &metadata,
                capture,
                &format!("{format}-reopened"),
                &mut times,
            );
        }
        tachiko_designer_runtime::close_project(&mut resident);
        assert!(resident.is_none());
    }
    times.save(&capture.join(format!("{format}-reopen-timings.json")));
    fs::write(
        capture.join(format!("{format}-fresh-process.json")),
        serde_json::to_vec(&json!({"pid": std::process::id(), "cycles": 10, "cells": ROWS * 3, "carrier": carrier, "saved_open_route": "metadata-aware", "capacity_performance_qualification": true, "producer_qualification": false}))
            .unwrap(),
    )
    .unwrap();
}

fn run(fixtures: &Path, capture: &Path, carrier: &str) {
    fs::create_dir(capture).expect("capture directory must be new");
    let mut scopes = BTreeMap::new();
    for format in ["csv", "xlsx"] {
        let mut times = Timings::default();
        for count in [8, 64, 65, 128, 129, 1024, 1025] {
            let source = fs::read(fixtures.join(format!("source-{count}.{format}"))).unwrap();
            let workbook = inspect(&source, format);
            let (_, imported) = import_workbook(&workbook, &selection(), OCCURRENCE).unwrap();
            let manifest = read_manifest(fixtures);
            verify(
                &imported.opened.table,
                &manifest.rows[..count],
                &imported.metadata,
            );
        }
        let mut manifest = read_manifest(fixtures);
        let source = fs::read(fixtures.join(format!("source-{ROWS}.{format}"))).unwrap();
        let (mut runtime, imported) = times.measure("import", || {
            let workbook = inspect(&source, format);
            assert_eq!(workbook.ledger.iter().filter(|f| f.blocking).count(), 0);
            import_workbook(&workbook, &selection(), OCCURRENCE).unwrap()
        });
        verify(&imported.opened.table, &manifest.rows, &imported.metadata);
        let collection = &imported.opened.bootstrap.default_collection;
        history(
            &mut runtime,
            collection,
            &mut manifest,
            &imported.metadata,
            &mut times,
        );
        let bytes = times.measure("save", || saved_bytes(&runtime, carrier));
        let inspected = times.measure("inspect_saved", || {
            inspect_imported_project(&bytes, &imported.metadata).unwrap()
        });
        verify(&inspected.table, &manifest.rows, &imported.metadata);
        fs::write(capture.join(format!("{format}-saved.project")), &bytes).unwrap();
        fs::write(
            capture.join(format!("{format}-metadata.json")),
            serde_json::to_vec(&imported.metadata).unwrap(),
        )
        .unwrap();
        fs::write(capture.join(format!("{format}-collection.txt")), collection).unwrap();
        export_artifacts(&runtime, &imported.metadata, capture, format, &mut times);
        scopes.insert(format, runtime.observe_occurrence().scope);
        rejection_preserves_resident(runtime, collection, &imported.metadata, &manifest, &bytes, carrier);
        times.save(&capture.join(format!("{format}-timings.json")));
    }
    for format in ["csv", "xlsx"] {
        for file in [
            format!("source-8407.{format}"),
            format!("malformed.{format}"),
        ] {
            let bytes = fs::read(fixtures.join(file)).unwrap();
            let result = if format == "csv" {
                import_csv(&bytes, &ImportOptions::default())
            } else {
                import_xlsx(&bytes)
            };
            assert!(
                result.is_err(),
                "over-limit/malformed {format} must fail closed"
            );
        }
    }
    assert!(import_csv(&[0xff], &ImportOptions::default()).is_err());
    assert!(import_csv(&vec![b'a'; 2 * 1024 * 1024 + 1], &ImportOptions::default()).is_err());
    fs::write(capture.join("native-import-complete.json"), serde_json::to_vec(&json!({"pid": std::process::id(), "formats": ["csv", "xlsx"], "cells": ROWS * 3, "history_edits_each": 128, "carrier": carrier, "scopes": scopes, "capacity_performance_qualification": true, "producer_qualification": false})).unwrap()).unwrap();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    assert!(
        (args[1] == "run" && args.len() == 5) || (args[1] == "reopen" && args.len() == 6),
        "run FIXTURES NEW_CAPTURE opaque|canonical; reopen FIXTURES CAPTURE csv|xlsx opaque|canonical"
    );
    match args[1].as_str() {
        "run" => run(Path::new(&args[2]), Path::new(&args[3]), &args[4]),
        "reopen" => reopen(Path::new(&args[2]), Path::new(&args[3]), &args[4], &args[5]),
        _ => panic!("unknown command"),
    }
}
