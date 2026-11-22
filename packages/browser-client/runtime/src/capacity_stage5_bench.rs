//! Stage 5c generated-XLSX qualification. Run from the repository root:
//! Native: `cargo test --manifest-path packages/browser-client/runtime/Cargo.toml --locked --release --lib capacity_stage5_qualification -- --ignored --nocapture`
//! WASM (after native): `cargo build --manifest-path packages/browser-client/runtime/Cargo.toml --locked --release --target wasm32-unknown-unknown && node packages/browser-client/runtime/tests/capacity_stage5_wire.mjs packages/browser-client/runtime/target/wasm32-unknown-unknown/release/tachiko_designer_runtime.wasm packages/browser-client/runtime/target/678-stage5-5c`

use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};

use crate::host_bridge::DesignerHost;
use crate::interop_adapter::{
    CellStyle, OutputProfile, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue,
    SourceWorkbook, export_xlsx_for_profile,
};

const REPLY_LIMIT: usize = 16 * 1024 * 1024;
const PREVIEW_LIMIT: usize = 48 * 1024 * 1024;

fn generated_xlsx(rows: usize) -> Vec<u8> {
    let data = (0..rows)
        .map(|row| {
            let value = f64::from(u32::try_from(row + 1).unwrap());
            let mut cells = vec![SourceCell::default(); 30];
            for (column, value) in [
                (0, SourceValue::Number { value }),
                (1, SourceValue::Number { value: 2.0 }),
                (
                    2,
                    SourceValue::Text {
                        value: format!("Row {}", row + 1),
                    },
                ),
                (
                    3,
                    SourceValue::Date {
                        value: "2026-11-22".into(),
                    },
                ),
                (
                    4,
                    SourceValue::Boolean {
                        value: row % 2 == 0,
                    },
                ),
                (
                    5,
                    SourceValue::Text {
                        value: "Stage5".into(),
                    },
                ),
                (6, SourceValue::Number { value: 3.0 }),
                (7, SourceValue::Number { value: value + 2.0 }),
                (8, SourceValue::Number { value: value * 2.0 }),
                (29, SourceValue::Number { value: 4.0 }),
            ] {
                cells[column].value = value;
            }
            cells[3].style = CellStyle {
                number_format: Some("yyyy-mm-dd".into()),
                ..CellStyle::default()
            };
            cells[7].formula = Some(format!("A{}+B{}", row + 2, row + 2));
            cells[8].formula = Some(format!("A{}*2", row + 2));
            cells
        })
        .collect();
    export_xlsx_for_profile(
        &SourceWorkbook {
            sheets: vec![SourceSheet {
                name: "Stage5".into(),
                has_header: true,
                columns: (0..30)
                    .map(|column| SourceColumn {
                        name: format!("Column{column}"),
                        width: None,
                        number_format: None,
                    })
                    .collect(),
                rows: data,
                layout: SourceLayout::default(),
            }],
            ledger: vec![],
        },
        OutputProfile::WorkingSize,
    )
    .unwrap()
}

fn run(host: &mut DesignerHost, request: &Value, source: &[u8]) -> (Value, f64, usize, usize) {
    let request = serde_json::to_vec(request).unwrap();
    let start = Instant::now();
    let reply = host.spreadsheet(&request, source);
    let elapsed = start.elapsed().as_secs_f64() * 1_000.0;
    let value: Value = serde_json::from_slice(&reply.reply).unwrap();
    assert_eq!(value["status"], "ok", "{value}");
    let length = reply.reply.len();
    let exported = reply.bytes.map_or(0, |bytes| {
        assert!(bytes.starts_with(b"PK"));
        bytes.len()
    });
    (
        value["response"]["payload"].clone(),
        elapsed,
        length,
        exported,
    )
}

fn summary(samples: &[Value], key: &str) -> Value {
    let mut times: Vec<_> = samples
        .iter()
        .map(|sample| sample[key].as_f64().unwrap())
        .collect();
    times.sort_by(f64::total_cmp);
    json!({"n": times.len(), "p50_ms": times[(times.len() - 1) / 2], "max_ms": times.last().unwrap()})
}

fn qualify(rows: usize) -> (Vec<u8>, Value) {
    assert!((1..=10_000).contains(&rows));
    let source = generated_xlsx(rows);
    let types: Vec<_> = (0..30)
        .map(|column| match column {
            2 | 5 => "text",
            3 => "date",
            4 => "boolean",
            _ => "number",
        })
        .collect();
    let inspect = json!({"type": "inspect", "format": "xlsx", "profile": "working_size",
        "csv_options": {"delimiter": ",", "header": true}});
    let import = json!({"type": "import", "format": "xlsx", "profile": "working_size",
        "csv_options": {"delimiter": ",", "header": true},
        "selection": {"column_types": [types], "extra_columns": [[]]},
        "occurrence_id": "00000000-0000-4000-8000-000000000678", "install": true});
    let mut host = DesignerHost::new();
    let mut samples = vec![];
    for sample in 1..=3 {
        let (preview, inspect_ms, inspect_bytes, _) = run(&mut host, &inspect, &source);
        assert_eq!(preview["sheets"][0]["rows"].as_array().unwrap().len(), rows);
        assert_eq!(
            preview["sheets"][0]["columns"].as_array().unwrap().len(),
            30
        );
        assert!(
            !preview["ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["blocking"] == true)
        );
        drop(preview);
        let (imported, import_ms, import_bytes, _) = run(&mut host, &import, &source);
        assert_eq!(imported["opened"]["window"]["total_rows"], rows);
        assert_eq!(
            imported["opened"]["window"]["columns"]
                .as_array()
                .unwrap()
                .len(),
            30
        );
        let export = json!({"type": "export", "format": "xlsx",
            "expected_revision": imported["opened"]["bootstrap"]["revision"],
            "collection": imported["opened"]["bootstrap"]["default_collection"],
            "metadata": imported["metadata"]});
        let (exported, export_ms, export_bytes, xlsx_bytes) = run(&mut host, &export, &[]);
        assert_eq!(exported["byte_length"], xlsx_bytes);
        let record = json!({"sample": sample, "inspect_ms": inspect_ms, "import_ms": import_ms,
            "open_ms": inspect_ms + import_ms, "export_ms": export_ms,
            "reply_bytes": {"inspect": inspect_bytes, "import": import_bytes, "export": export_bytes},
            "xlsx_bytes": xlsx_bytes});
        println!("native {record}");
        samples.push(record);
    }
    let report = json!({"target": "native", "rows": rows, "columns": 30,
        "populated_cells": rows * 10, "source_bytes": source.len(),
        "inspect_request": inspect, "import_request": import,
        "summary": {"inspect_ms": summary(&samples, "inspect_ms"),
            "import_ms": summary(&samples, "import_ms"), "open_ms": summary(&samples, "open_ms"),
            "export_ms": summary(&samples, "export_ms")},
        "samples": samples});
    println!("native summary {}", report["summary"]);
    (source, report)
}

fn assert_reply_budgets(report: &Value) {
    for sample in report["samples"].as_array().unwrap() {
        assert!(sample["reply_bytes"]["inspect"].as_u64().unwrap() <= PREVIEW_LIMIT as u64);
        for step in ["import", "export"] {
            assert!(sample["reply_bytes"][step].as_u64().unwrap() <= REPLY_LIMIT as u64);
        }
    }
}

#[test]
fn capacity_stage5_small() {
    let (_, report) = qualify(200);
    assert_reply_budgets(&report);
}

#[test]
#[ignore = "Stage 5c: explicit release-only measurement; never run 10k rows on the Owner's Mac"]
fn capacity_stage5_qualification() {
    assert!(!cfg!(debug_assertions), "run with --release");
    let rows = std::env::var("STAGE5_ROWS").map_or(10_000, |value| value.parse().unwrap());
    let directory = std::env::var_os("STAGE5_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/678-stage5-5c"),
        PathBuf::from,
    );
    let (source, report) = qualify(rows);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("source.xlsx"), source).unwrap();
    std::fs::write(
        directory.join("native.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_reply_budgets(&report);
    for step in ["open_ms", "export_ms"] {
        assert!(
            report["summary"][step]["max_ms"].as_f64().unwrap() <= 3_000.0,
            "native {step} exceeds 3 s; see native.json"
        );
    }
}
