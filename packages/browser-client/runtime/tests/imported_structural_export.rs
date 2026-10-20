//! #557: real imports, native structural commands, metadata export, history and carriers.
#![cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};
use tachiko_designer_runtime::host_bridge::{DesignerHost, HostReply};
use tachiko_designer_runtime::interop_adapter::{
    ImportOptions, SourceValue, export_xlsx, import_csv, import_xlsx,
};

const ORIGINAL: &str = "00000000-0000-4000-8000-000000000557";
const FRESH: &str = "00000000-0000-4000-8000-000000000558";

#[derive(Default)]
struct Journey {
    host: DesignerHost,
    records: Vec<Value>,
    metadata: Value,
}
impl Journey {
    fn record(
        &mut self,
        operation: &str,
        request: &[u8],
        project: &[u8],
        reply: HostReply,
    ) -> (Value, Option<Vec<u8>>) {
        let parsed = serde_json::from_slice(&reply.reply).unwrap();
        self.records.push(json!({"operation":operation,"request":request,"project":project,"reply":parsed,"bytes":reply.bytes}));
        (parsed, reply.bytes)
    }
    fn sheet(&mut self, request: &Value, source: &[u8]) -> (Value, Option<Vec<u8>>) {
        let request = serde_json::to_vec(request).unwrap();
        let reply = self.host.spreadsheet(&request, source);
        self.record("spreadsheet_run", &request, source, reply)
    }
    fn request(&mut self, request: &Value) -> Value {
        let bytes = serde_json::to_vec(request).unwrap();
        let reply = HostReply {
            reply: self.host.request(&bytes),
            bytes: None,
        };
        let (parsed, _) = self.record("request_run", &bytes, &[], reply);
        assert_eq!(parsed["status"], "ok", "{request}: {parsed}");
        parsed["response"]["payload"].clone()
    }
    fn table(&mut self) -> Value {
        self.request(&json!({"type":"query_table","collection":"sheet_1"}))
    }
    fn mutate(&mut self, mut request: Value) {
        request["expected_revision"] = self.table()["revision"].clone();
        self.request(&request);
    }
    fn import(&mut self, format: &str) {
        let csv = b"Note,Amount\nfirst,1\nsecond,2\n";
        let mut book = import_csv(csv, &ImportOptions::default()).unwrap();
        for (i, row) in book.sheets[0].rows.iter_mut().enumerate() {
            row[1].value = SourceValue::Number {
                value: f64::from(u32::try_from(i + 1).unwrap()),
            };
        }
        let bytes = if format == "csv" {
            csv.to_vec()
        } else {
            export_xlsx(&book).unwrap()
        };
        let (reply, _) = self.sheet(&json!({"type":"import","format":format,"csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text","number"]],"extra_columns":[[]]},"occurrence_id":ORIGINAL,"install":true}), &bytes);
        assert_eq!(reply["status"], "ok", "{reply}");
        self.metadata = reply["response"]["payload"]["metadata"].clone();
        let table = self.table();
        assert_eq!(table["native_table_profile"], true);
        assert!(
            table["columns"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["required"] == false)
        );
    }
    fn capture(&self, name: &str) {
        if let Ok(root) = std::env::var("TACHIKO_IMPORTED_STRUCTURAL_TRANSCRIPT") {
            std::fs::write(
                format!("{root}.{name}.json"),
                serde_json::to_vec(&self.records).unwrap(),
            )
            .unwrap();
        }
    }
    fn reopen(&mut self, portable: bool) {
        let revision = self.table()["revision"]
            .as_str()
            .unwrap()
            .as_bytes()
            .to_vec();
        let (operation, reply) = if portable {
            (
                "portable_ro_export",
                self.host.export_portable_ro(&revision),
            )
        } else {
            ("project_export_v3", self.host.export_project_v3(&revision))
        };
        let (parsed, bytes) = self.record(operation, &revision, &[], reply);
        assert_eq!(parsed["status"], "ok", "{parsed}");
        let bytes = bytes.unwrap();
        if portable {
            let reply = HostReply {
                reply: self.host.open_portable_ro(&bytes, FRESH.as_bytes()),
                bytes: None,
            };
            let (parsed, _) = self.record("portable_ro_open", FRESH.as_bytes(), &bytes, reply);
            assert_eq!(parsed["status"], "ok", "{parsed}");
        } else {
            // Exercise metadata-aware saved admission, not just the generic codec.
            let (parsed, _) = self.sheet(
                &json!({"type":"open_project","metadata":self.metadata,"occurrence_id":FRESH}),
                &bytes,
            );
            assert_eq!(parsed["status"], "ok", "{parsed}");
        }
    }
    fn exports(
        &mut self,
        label: &str,
        headers: &[&str],
        rows: &[Vec<Value>],
        errors: &mut Vec<String>,
    ) {
        for format in ["csv", "xlsx"] {
            let revision = self.table()["revision"].clone();
            let (reply, bytes) = self.sheet(&json!({"type":"export","expected_revision":revision,"metadata":self.metadata,"format":format,"collection":self.metadata["sheets"][0]["schema_id"]}), &[]);
            if reply["status"] != "ok" {
                assert!(bytes.is_none(), "refusal must never return a partial file");
                errors.push(format!("{label}/{format}: {reply}"));
                continue;
            }
            let bytes = bytes.unwrap();
            let book = if format == "csv" {
                import_csv(&bytes, &ImportOptions::default()).unwrap()
            } else {
                import_xlsx(&bytes).unwrap()
            };
            let sheet = &book.sheets[0];
            let actual_headers: Vec<_> = sheet.columns.iter().map(|c| c.name.as_str()).collect();
            let actual_rows: Vec<Vec<Value>> = sheet
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|c| match &c.value {
                            SourceValue::Empty => Value::Null,
                            SourceValue::Text { value } => json!(value),
                            SourceValue::Number { value } => {
                                if format == "csv" {
                                    json!(value.to_string())
                                } else {
                                    json!(value)
                                }
                            }
                            other => panic!("unexpected scalar {other:?}"),
                        })
                        .collect()
                })
                .collect();
            let expected_rows: Vec<Vec<Value>> = rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|v| {
                            if format == "csv" && v.is_number() {
                                json!(v.as_f64().unwrap().to_string())
                            } else {
                                v.clone()
                            }
                        })
                        .collect()
                })
                .collect();
            if actual_headers != headers || actual_rows != expected_rows {
                errors.push(format!("{label}/{format}: headers={actual_headers:?}, rows={actual_rows:?}; expected={headers:?}/{expected_rows:?}"));
            }
        }
    }
}

fn matrix(source: &str, operation: &str) {
    let mut j = Journey::default();
    j.import(source);
    let table = j.table();
    let note = &table["columns"][0]["id"];
    let amount = &table["columns"][1]["id"];
    let row = &table["rows"][0]["id"];
    let mut headers = vec!["Note", "Amount"];
    let original_rows = vec![
        vec![json!("first"), json!(1.0)],
        vec![json!("second"), json!(2.0)],
    ];
    let mut rows = original_rows.clone();
    let command = match operation {
        "rename" => {
            headers[0] = "Current note";
            json!({"type":"rename_column","collection":"sheet_1","field":note,"name":"Current note"})
        }
        "add_blank" | "add_fill" => {
            headers.push("Extra");
            for (i, values) in rows.iter_mut().enumerate() {
                values.push(if operation == "add_fill" && i == 0 {
                    json!("filled")
                } else {
                    Value::Null
                });
            }
            let initializers = if operation == "add_fill" {
                json!([{"entity":row,"input":{"kind":"text","value":"filled"}}])
            } else {
                json!([])
            };
            json!({"type":"add_column","collection":"sheet_1","name":"Extra","field_type":"text","required":false,"initializers":initializers})
        }
        "remove" => {
            headers.remove(1);
            for row in &mut rows {
                row.remove(1);
            }
            json!({"type":"remove_column","collection":"sheet_1","field":amount})
        }
        "insert_blank" => {
            rows.push(vec![Value::Null, Value::Null]);
            json!({"type":"insert_row","collection":"sheet_1","initializers":[]})
        }
        "insert" => {
            rows.push(vec![json!("third"), Value::Null]);
            json!({"type":"insert_row","collection":"sheet_1","initializers":[{"field":note,"input":{"kind":"text","value":"third"}}]})
        }
        "delete" => {
            rows.remove(0);
            json!({"type":"remove_table_rows","collection":"sheet_1","entities":[row]})
        }
        _ => unreachable!(),
    };
    let mut errors = Vec::new();
    j.mutate(command.clone());
    j.exports("edited", &headers, &rows, &mut errors);
    j.mutate(json!({"type":"undo"}));
    j.exports("undone", &["Note", "Amount"], &original_rows, &mut errors);
    j.mutate(json!({"type":"redo"}));
    // Reopen both the edited and undone content in each carrier.
    for portable in [false, true] {
        j.reopen(portable);
        j.exports(
            if portable { "package" } else { "v3" },
            &headers,
            &rows,
            &mut errors,
        );
    }
    // A fresh import supplies session history again for the undone carrier cases.
    j.import(source);
    let table = j.table();
    // IDs are deterministic for this occurrence, so replaying the original command is exact.
    assert_eq!(table["columns"][0]["id"], *note);
    j.mutate(command);
    j.mutate(json!({"type":"undo"}));
    for portable in [false, true] {
        j.reopen(portable);
        j.exports(
            "undone-reopened",
            &["Note", "Amount"],
            &original_rows,
            &mut errors,
        );
    }
    j.capture(&format!("{source}-{operation}"));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
#[test]
fn csv_rename() {
    matrix("csv", "rename");
}
#[test]
fn csv_add_blank() {
    matrix("csv", "add_blank");
}
#[test]
fn csv_add_fill() {
    matrix("csv", "add_fill");
}
#[test]
fn csv_remove() {
    matrix("csv", "remove");
}
#[test]
fn csv_insert() {
    matrix("csv", "insert");
}
#[test]
fn csv_insert_blank() {
    matrix("csv", "insert_blank");
}
#[test]
fn csv_delete() {
    matrix("csv", "delete");
}
#[test]
fn xlsx_rename() {
    matrix("xlsx", "rename");
}
#[test]
fn xlsx_add_blank() {
    matrix("xlsx", "add_blank");
}
#[test]
fn xlsx_add_fill() {
    matrix("xlsx", "add_fill");
}
#[test]
fn xlsx_remove() {
    matrix("xlsx", "remove");
}
#[test]
fn xlsx_insert() {
    matrix("xlsx", "insert");
}
#[test]
fn xlsx_insert_blank() {
    matrix("xlsx", "insert_blank");
}
#[test]
fn xlsx_delete() {
    matrix("xlsx", "delete");
}

impl Journey {
    fn refused_exports(&mut self, path: &str) {
        let table = self.table();
        for format in ["csv", "xlsx"] {
            let (reply, bytes) = self.sheet(&json!({"type":"export","expected_revision":table["revision"],"metadata":self.metadata,"format":format,"collection":self.metadata["sheets"][0]["schema_id"]}), &[]);
            assert_eq!(reply["status"], "error", "{reply}");
            assert!(bytes.is_none(), "no partial file");
            assert_eq!(
                reply["error"]["code"], "imported_spreadsheet_export_refused",
                "{reply}"
            );
            assert_eq!(reply["error"]["diagnostics"].as_array().unwrap().len(), 1);
            assert!(
                reply["error"]["diagnostics"][0]["path"]
                    .as_str()
                    .unwrap()
                    .contains(path),
                "{reply}"
            );
            assert_eq!(reply["error"]["current_revision"], table["revision"]);
            assert_eq!(
                self.table(),
                table,
                "refused export cannot mutate content/history"
            );
        }
    }
}

#[test]
fn unrepresentable_structures_refuse_whole_files_with_located_findings() {
    let mut j = Journey::default();
    j.import("csv");
    // The native authoring profile allows 17 columns; metadata output allows 16.
    for i in 0..15 {
        j.mutate(json!({"type":"add_column","collection":"sheet_1","name":format!("extra {i}"),"field_type":"text","required":false}));
    }
    let schema = j.metadata["sheets"][0]["schema_id"]
        .as_str()
        .unwrap()
        .to_owned();
    j.refused_exports(&schema);
    j.mutate(json!({"type":"undo"}));
    let revision = j.table()["revision"].clone();
    assert_eq!(j.sheet(&json!({"type":"export","expected_revision":revision,"metadata":j.metadata,"format":"xlsx","collection":schema}), &[]).0["status"], "ok");
    j.import("xlsx");
    let note = j.table()["columns"][0]["id"].clone();
    let rows: Vec<_> = (0..63).map(|i| vec![format!("new {i}")]).collect();
    j.mutate(json!({"type":"paste_cells","collection":"sheet_1","start_entity":null,"start_field":note,"rows":rows}));
    assert_eq!(j.table()["rows"].as_array().unwrap().len(), 65);
    j.refused_exports("workbook");
    j.mutate(json!({"type":"undo"}));
    j.import("csv");
    j.mutate(json!({"type":"duplicate_collection","collection":"sheet_1","name":"Unmapped"}));
    j.refused_exports("workbook");
    j.mutate(json!({"type":"undo"}));
    j.import("csv");
    let note = j.table()["columns"][0]["id"].clone();
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":note,"name":"_x0041_"}));
    j.refused_exports(note.as_str().unwrap());
    j.mutate(json!({"type":"undo"}));
    // A caseless collision with an original header uses the existing suffix mapping.
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":note,"name":"amount"}));
    let mut errors = Vec::new();
    j.exports(
        "mapped-collision",
        &["amount", "Amount (2)"],
        &[
            vec![json!("first"), json!(1.0)],
            vec![json!("second"), json!(2.0)],
        ],
        &mut errors,
    );
    assert!(errors.is_empty(), "{errors:?}");
    j.capture("refusals");
}

#[test]
fn structural_export_preserves_untouched_formulas_styles_and_workbook_coverage() {
    use tachiko_designer_runtime::interop_adapter::{
        CellStyle, SourceCell, SourceColumn, SourceSheet, SourceWorkbook,
    };
    let number = |n| SourceCell {
        value: SourceValue::Number { value: n },
        formula: None,
        style: CellStyle::default(),
    };
    let mut source = import_csv(
        b"Note,Amount\nfirst,1\nsecond,2\n",
        &ImportOptions::default(),
    )
    .unwrap();
    source.sheets[0].name = "Data".into();
    source.sheets[0].columns[1].width = Some(23.0);
    source.sheets[0].rows[0][1] = number(1.0);
    source.sheets[0].rows[1][1] = number(2.0);
    source.sheets[0].rows[0][1].style.bold = true;
    source.sheets.push(SourceSheet {
        name: "Summary".into(),
        has_header: true,
        columns: vec![SourceColumn {
            name: "Result".into(),
            width: None,
        }],
        rows: vec![vec![SourceCell {
            formula: Some("'Data'!$B$2+1".into()),
            ..number(2.0)
        }]],
    });
    let mut j = Journey::default();
    let source_bytes = export_xlsx(&SourceWorkbook {
        sheets: source.sheets,
        ledger: vec![],
    })
    .unwrap();
    let (reply, _) = j.sheet(&json!({"type":"import","format":"xlsx","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text","number"],["number"]],"extra_columns":[[],[]]},"occurrence_id":ORIGINAL,"install":true}), &source_bytes);
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = reply["response"]["payload"]["metadata"].clone();
    let note = j.table()["columns"][0]["id"].clone();
    j.mutate(json!({"type":"remove_column","collection":"sheet_1","field":note}));
    for portable in [None, Some(false), Some(true)] {
        if let Some(portable) = portable {
            j.reopen(portable);
        }
        let table = j.table();
        let (reply, bytes) = j.sheet(&json!({"type":"export","expected_revision":table["revision"],"metadata":j.metadata,"format":"xlsx","collection":j.metadata["sheets"][0]["schema_id"]}), &[]);
        assert_eq!(reply["status"], "ok", "{reply}");
        let book = import_xlsx(&bytes.unwrap()).unwrap();
        assert_eq!(book.sheets.len(), 2);
        assert_eq!(book.sheets[0].columns[0].name, "Amount");
        assert_eq!(book.sheets[0].columns[0].width, Some(23.0));
        assert!(book.sheets[0].rows[0][0].style.bold);
        assert!(!book.sheets[0].rows[1][0].style.bold);
        assert_eq!(
            book.sheets[1].rows[0][0].formula.as_deref(),
            Some("('Data'!$A$2+1)")
        );
        assert_eq!(
            book.sheets[1].rows[0][0].value,
            SourceValue::Number { value: 2.0 }
        );
    }
    j.capture("formulas");
}
