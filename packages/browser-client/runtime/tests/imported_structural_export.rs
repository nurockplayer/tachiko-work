//! #557: real imports, native structural commands, metadata export, history and carriers.
#![cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};
use std::io::Read;
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

fn explicit_display_name_journey(source: &str) {
    let mut j = Journey::default();
    j.import(source);
    let original = j.table();
    let field = original["columns"][0]["id"].clone();
    let key = original["columns"][0]["key"].as_str().unwrap();
    assert!(
        original["columns"][0]
            .get("display_name_explicit")
            .is_none()
    );
    j.reopen(false);
    assert!(
        j.table()["columns"][0]
            .get("display_name_explicit")
            .is_none()
    );

    for name in ["Current note", key] {
        j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":field,"name":name}));
        let renamed = j.table();
        assert_eq!(renamed["columns"][0]["display_name"], name);
        assert_eq!(renamed["columns"][0]["display_name_explicit"], true);
        let mut errors = Vec::new();
        j.exports(
            "explicit",
            &[name, "Amount"],
            &[
                vec![json!("first"), json!(1.0)],
                vec![json!("second"), json!(2.0)],
            ],
            &mut errors,
        );
        assert!(errors.is_empty(), "{errors:?}");
        j.mutate(json!({"type":"undo"}));
        let undone = j.table();
        assert!(undone["columns"][0].get("display_name_explicit").is_none());
        assert_eq!(undone["columns"][0]["display_name"], key);
    }
    j.reopen(false);
    assert!(
        j.table()["columns"][0]
            .get("display_name_explicit")
            .is_none()
    );
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":field,"name":key}));
    j.mutate(json!({"type":"undo"}));
    j.mutate(json!({"type":"redo"}));
    for portable in [false, true] {
        j.reopen(portable);
        let reopened = j.table();
        assert_eq!(reopened["columns"][0]["display_name_explicit"], true);
        assert_eq!(reopened["columns"][0]["display_name"], key);
    }
    j.capture(&format!("explicit-name-{source}"));
}

#[test]
fn csv_explicit_display_name_presence_survives_history_and_reopen() {
    explicit_display_name_journey("csv");
}

#[test]
fn xlsx_explicit_display_name_presence_survives_history_and_reopen() {
    explicit_display_name_journey("xlsx");
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
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "error");
    assert!(bytes.is_none());
    assert!(
        reply["error"]["diagnostics"][0]["path"]
            .as_str()
            .unwrap()
            .contains(note.as_str().unwrap())
    );
    assert_eq!(j.current_export("csv").0["status"], "ok");
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
            number_format: None,
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

impl Journey {
    fn current_export(&mut self, format: &str) -> (Value, Option<Vec<u8>>) {
        let revision = self.table()["revision"].clone();
        self.sheet(
            &json!({"type":"export","expected_revision":revision,"metadata":self.metadata,
            "format":format,"collection":self.metadata["sheets"][0]["schema_id"]}),
            &[],
        )
    }
}

fn compatibility_cells(source: &str) {
    let mut j = Journey::default();
    let mut errors = Vec::new();
    // At the boundary, the emoji uses two UTF-16 units and four UTF-8 bytes.
    for (label, text, refusal) in [
        (
            "escape",
            "literal_x0041_value".into(),
            Some("escape_sequence"),
        ),
        (
            "escape-cr",
            "literal_x000D_value".into(),
            Some("escape_sequence"),
        ),
        ("near-miss", "literal_x00D_value".into(), None),
        ("ascii-exact", "a".repeat(32_767), None),
        ("ascii-over", "a".repeat(32_768), Some("text_limit")),
        ("utf16-exact", format!("{}😀", "a".repeat(32_765)), None),
        (
            "utf16-over",
            format!("{}😀", "a".repeat(32_766)),
            Some("text_limit"),
        ),
        ("lf-exact", "a\n".repeat(253), None),
        ("lf-over", "a\n".repeat(254), Some("line_feeds")),
        (
            "xml-illegal",
            "literal\u{fffe}value".into(),
            Some("xml_character"),
        ),
    ] {
        j.import(source);
        let table = j.table();
        let note = table["columns"][0]["id"].clone();
        let rows: Vec<_> = table["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].clone())
            .collect();
        j.mutate(json!({"type":"remove_table_rows","collection":"sheet_1","entities":rows}));
        j.mutate(json!({"type":"insert_row","collection":"sheet_1","initializers":[{"field":note,"input":{"kind":"text","value":text}}]}));
        let before = j.table();
        for format in ["csv", "xlsx"] {
            let (reply, bytes) = j.current_export(format);
            let expected = if format == "xlsx" {
                refusal
            } else if label == "xml-illegal" {
                Some("xml_character")
            } else {
                None
            };
            if let Some(code) = expected {
                if reply["status"] != "error" {
                    errors.push(format!(
                        "{source}/{label}/{format}: expected {code}, got success"
                    ));
                    continue;
                }
                assert!(bytes.is_none(), "no partial file");
                let diagnostic = &reply["error"]["diagnostics"][0];
                if reply["error"]["code"] != "imported_spreadsheet_export_refused"
                    || !diagnostic["code"].as_str().unwrap().ends_with(code)
                    || !diagnostic["path"].as_str().unwrap().contains("/row ")
                    || !diagnostic["path"].as_str().unwrap().contains("/column 1")
                {
                    errors.push(format!(
                        "{source}/{label}/{format}: unlocated or wrong refusal {reply}"
                    ));
                }
            } else {
                assert_eq!(reply["status"], "ok", "{source}/{label}/{format}: {reply}");
                let bytes = bytes.unwrap();
                let workbook = if format == "csv" {
                    import_csv(&bytes, &ImportOptions::default()).unwrap()
                } else {
                    import_xlsx(&bytes).unwrap()
                };
                assert_eq!(
                    workbook.sheets[0].rows[0][0].value,
                    SourceValue::Text {
                        value: text.clone()
                    }
                );
            }
            assert_eq!(
                j.table(),
                before,
                "export cannot mutate current content or revision"
            );
        }
        j.mutate(json!({"type":"undo"}));
    }
    j.capture(&format!("cell-compatibility-{source}"));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn csv_import_every_current_cell_has_format_specific_compatibility() {
    compatibility_cells("csv");
}
#[test]
fn xlsx_import_every_current_cell_has_format_specific_compatibility() {
    compatibility_cells("xlsx");
}

#[test]
fn foreign_current_fields_entities_and_schema_mappings_are_not_tombstones() {
    let mut source = import_csv(
        b"Note,Amount\nfirst,1\nsecond,2\n",
        &ImportOptions::default(),
    )
    .unwrap();
    let mut other = source.sheets[0].clone();
    other.name = "Other".into();
    source.sheets.push(other);
    let bytes = export_xlsx(&source).unwrap();
    let mut j = Journey::default();
    let (reply, _) = j.sheet(&json!({"type":"import","format":"xlsx","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text","number"],["text","number"]],"extra_columns":[[],[]]},"occurrence_id":ORIGINAL,"install":true}), &bytes);
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = reply["response"]["payload"]["metadata"].clone();
    let table = j.table();
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":table["columns"][0]["id"],"name":"Renamed"}));
    let original = j.metadata.clone();
    let mut errors = Vec::new();
    for kind in ["field", "entity", "schema"] {
        j.metadata = original.clone();
        match kind {
            "field" => {
                j.metadata["sheets"][0]["columns"][0]["field_id"] =
                    original["sheets"][1]["columns"][0]["field_id"].clone();
            }
            "entity" => {
                j.metadata["sheets"][0]["rows"][0]["entity_id"] =
                    original["sheets"][1]["rows"][0]["entity_id"].clone();
            }
            "schema" => {
                j.metadata["sheets"][0]["schema_id"] = original["sheets"][1]["schema_id"].clone();
                j.metadata["sheets"][1]["schema_id"] = original["sheets"][0]["schema_id"].clone();
            }
            _ => unreachable!(),
        }
        for format in ["csv", "xlsx"] {
            let (reply, bytes) = j.current_export(format);
            if reply["status"] == "error" {
                assert!(bytes.is_none());
                assert_eq!(
                    reply["error"]["code"],
                    "imported_spreadsheet_export_refused"
                );
                let path = reply["error"]["diagnostics"][0]["path"].as_str().unwrap();
                assert!(path.contains("collection "), "{reply}");
                if kind != "schema" {
                    let id = if kind == "field" {
                        &original["sheets"][1]["columns"][0]["field_id"]
                    } else {
                        &original["sheets"][1]["rows"][0]["entity_id"]
                    };
                    assert!(path.contains(id.as_str().unwrap()), "{reply}");
                }
            } else {
                errors.push(format!("foreign {kind}/{format}: succeeded"));
            }
        }
    }
    // A genuinely absent historical field ID is still a tombstone, not foreign.
    j.metadata = original.clone();
    j.metadata["sheets"][0]["columns"][0]["field_id"] = json!("absent-historical-field");
    for format in ["csv", "xlsx"] {
        let (reply, bytes) = j.current_export(format);
        assert_eq!(reply["status"], "ok", "{reply}");
        assert!(bytes.is_some());
    }
    j.capture("foreign-current");
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn header_mapping_discloses_requested_spelling_even_when_output_equals_history() {
    let mut j = Journey::default();
    let (reply, _) = j.sheet(&json!({"type":"import","format":"csv","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text","number"]],"extra_columns":[[]]},"occurrence_id":ORIGINAL,"install":true}), b"Amount,amount (2)\nfirst,1\nsecond,2\n");
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = reply["response"]["payload"]["metadata"].clone();
    let field = j.table()["columns"][1]["id"].clone();
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":field,"name":"amount"}));
    let mut errors = Vec::new();
    for format in ["csv", "xlsx"] {
        let (reply, bytes) = j.current_export(format);
        assert_eq!(reply["status"], "ok", "{reply}");
        let bytes = bytes.unwrap();
        let book = if format == "csv" {
            import_csv(&bytes, &ImportOptions::default()).unwrap()
        } else {
            import_xlsx(&bytes).unwrap()
        };
        assert_eq!(book.sheets[0].columns[1].name, "amount (2)");
        let finding = reply["response"]["payload"]["ledger"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| {
                f["code"] == "imported_current_header"
                    && f["location"]
                        .as_str()
                        .unwrap()
                        .contains(field.as_str().unwrap())
            });
        if let Some(finding) = finding {
            assert!(
                finding["message"]
                    .as_str()
                    .unwrap()
                    .contains("\"amount\" exports as \"amount (2)\"")
            );
        } else {
            errors.push(format!(
                "{format}: missing mapping finding for requested amount -> written amount (2)"
            ));
        }
    }
    j.capture("header-history-collision");
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn untouched_imported_text_is_checked_before_xlsx_encoding() {
    let mut j = Journey::default();
    let mut errors = Vec::new();
    for text in ["literal_x0041_value".to_owned(), "a".repeat(32_768)] {
        let source = format!("Note,Amount\n{text},1\n");
        let (reply, _) = j.sheet(&json!({"type":"import","format":"csv","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text","number"]],"extra_columns":[[]]},"occurrence_id":ORIGINAL,"install":true}), source.as_bytes());
        assert_eq!(reply["status"], "ok", "{reply}");
        j.metadata = reply["response"]["payload"]["metadata"].clone();
        let (csv, bytes) = j.current_export("csv");
        assert_eq!(csv["status"], "ok", "{csv}");
        assert!(bytes.is_some());
        let (xlsx, bytes) = j.current_export("xlsx");
        if xlsx["status"] == "error" {
            assert!(bytes.is_none());
        } else {
            errors.push("untouched unrepresentable Text exported as XLSX".to_owned());
        }
        let before = j.table();
        for portable in [false, true] {
            j.reopen(portable);
            let reopened = j.table();
            assert_eq!(reopened["columns"], before["columns"]);
            assert_eq!(reopened["rows"], before["rows"]);
            assert_eq!(j.current_export("csv").0["status"], "ok");
            let (reply, bytes) = j.current_export("xlsx");
            assert_eq!(reply["status"], "error", "{reply}");
            assert!(bytes.is_none());
        }
    }
    j.capture("untouched-compatibility");
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn imported_numbers_and_dates_follow_the_shared_cell_policy_without_changing_csv() {
    let mut j = Journey::default();
    j.import("csv");
    let table = j.table();
    let target = json!({"entity":table["rows"][0]["id"],"field":table["columns"][1]["id"]});
    for input in ["5e-324", "0.30000000000000004"] {
        j.mutate(
            json!({"type":"edit_scalar","target":target,"input":{"kind":"number","input":input}}),
        );
        let (csv, bytes) = j.current_export("csv");
        assert_eq!(csv["status"], "ok", "{csv}");
        assert!(bytes.is_some());
        let (xlsx, bytes) = j.current_export("xlsx");
        if input == "5e-324" {
            assert_eq!(xlsx["status"], "error", "{xlsx}");
            assert!(bytes.is_none());
            assert_eq!(
                xlsx["error"]["diagnostics"][0]["code"],
                "native_tables_xlsx_subnormal_number"
            );
            assert!(
                xlsx["error"]["diagnostics"][0]["path"]
                    .as_str()
                    .unwrap()
                    .contains("/row 2/column 2")
            );
        } else {
            assert_eq!(xlsx["status"], "ok", "{xlsx}");
            let findings = xlsx["response"]["payload"]["ledger"].as_array().unwrap();
            for code in [
                "native_tables_xlsx_precision_loss",
                "native_tables_xlsx_precision_count",
            ] {
                assert_eq!(
                    findings
                        .iter()
                        .filter(|finding| finding["code"] == code && finding["blocking"] == false)
                        .count(),
                    1
                );
            }
            let book = import_xlsx(&bytes.unwrap()).unwrap();
            assert_eq!(
                book.sheets[0].rows[0][1].value,
                SourceValue::Number {
                    value: 0.300_000_000_000_000_04
                }
            );
        }
    }
    for date in ["1899-12-31", "1900-01-01", "9999-12-31"] {
        let source = format!("Date\n{date}\n");
        let (reply, _) = j.sheet(&json!({"type":"import","format":"csv","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["date"]],"extra_columns":[[]]},"occurrence_id":ORIGINAL,"install":true}), source.as_bytes());
        assert_eq!(reply["status"], "ok", "{reply}");
        j.metadata = reply["response"]["payload"]["metadata"].clone();
        assert_eq!(j.current_export("csv").0["status"], "ok");
        let (reply, bytes) = j.current_export("xlsx");
        if date == "1899-12-31" {
            assert_eq!(reply["status"], "error", "{reply}");
            assert!(bytes.is_none());
            assert_eq!(
                reply["error"]["diagnostics"][0]["code"],
                "native_tables_xlsx_date_range"
            );
        } else {
            assert_eq!(reply["status"], "ok", "{reply}");
            assert_eq!(
                import_xlsx(&bytes.unwrap()).unwrap().sheets[0].rows[0][0].value,
                SourceValue::Date { value: date.into() }
            );
        }
    }
    j.capture("scalar-compatibility");
}

fn formula_journey(formula: &str, data_name: &str) -> Journey {
    formula_journey_rows(formula, data_name, 1)
}

fn formula_journey_rows(formula: &str, data_name: &str, rows: usize) -> Journey {
    use tachiko_designer_runtime::interop_adapter::{SourceCell, SourceColumn, SourceSheet};
    let mut source = import_csv(b"Note,Amount\nfirst,1\n", &ImportOptions::default()).unwrap();
    source.sheets[0].name = data_name.into();
    source.sheets[0].rows[0][1].value = SourceValue::Number { value: 1.0 };
    source.sheets.push(SourceSheet {
        name: "Summary".into(),
        has_header: true,
        columns: vec![SourceColumn {
            name: "Result".into(),
            width: None,
            number_format: None,
        }],
        rows: vec![
            vec![SourceCell {
                value: SourceValue::Number { value: 0.0 },
                formula: Some(formula.into()),
                ..SourceCell::default()
            }];
            rows
        ],
    });
    let bytes = export_xlsx(&source).unwrap();
    let mut j = Journey::default();
    let (reply, _) = j.sheet(&json!({"type":"import","format":"xlsx","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["text","number"],["number"]],"extra_columns":[[],[]]},"occurrence_id":ORIGINAL,"install":true}), &bytes);
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = reply["response"]["payload"]["metadata"].clone();
    assert_eq!(j.table()["native_table_profile"], true);
    j.mutate(json!({"type":"add_column","collection":"sheet_1","name":"Extra","field_type":"text","required":false}));
    j
}

#[test]
fn imported_formula_literals_and_cached_results_share_the_number_policy() {
    let mut all = Journey::default();
    let mut errors = Vec::new();
    for (formula, refusal, precision) in [
        ("5e-324/5e-324", true, false),
        ("(-5e-324)/(-5e-324)", true, false),
        ("(0.30000000000000004-0.3)*0", false, true),
        ("2.2250738585072014e-308*0", false, true),
        ("-0+0", false, false),
        ("MAX('Data'!$B$2,2)", false, false),
    ] {
        let mut j = formula_journey(formula, "Data");
        assert_eq!(j.current_export("csv").0["status"], "ok");
        let (reply, bytes) = j.current_export("xlsx");
        if refusal {
            if reply["status"] == "error" {
                assert!(bytes.is_none());
                assert_eq!(
                    reply["error"]["diagnostics"][0]["code"],
                    "native_tables_xlsx_subnormal_number"
                );
                assert!(
                    reply["error"]["diagnostics"][0]["path"]
                        .as_str()
                        .unwrap()
                        .contains("/row 2/column 1/formula")
                );
            } else {
                errors.push(format!("{formula}: unchecked formula literal exported"));
            }
        } else {
            assert_eq!(reply["status"], "ok", "{formula}: {reply}");
            let book = import_xlsx(&bytes.unwrap()).unwrap();
            assert_eq!(
                book.sheets[1].rows[0][0].value,
                SourceValue::Number {
                    value: if formula.starts_with("MAX") { 2.0 } else { 0.0 }
                }
            );
            let findings = reply["response"]["payload"]["ledger"].as_array().unwrap();
            if precision
                && !findings.iter().any(|f| {
                    f["code"] == "native_tables_xlsx_precision_loss"
                        && f["location"].as_str().unwrap().contains("/formula")
                })
            {
                errors.push(format!("{formula}: no literal precision disclosure"));
            }
            if formula == "-0+0" {
                assert_eq!(book.sheets[1].rows[0][0].formula.as_deref(), Some("(0+0)"));
            }
        }
        all.records.extend(j.records);
    }
    assert!(errors.is_empty(), "{errors:?}");
    all.capture("formula-output-policy");
}

#[test]
fn imported_csv_checks_only_selected_emitted_values_and_headers() {
    let mut j = formula_journey("'Data'!$B$2+1", "Data");
    // Unselected XML-illegal Text must not govern the selected Summary CSV.
    let table = j.table();
    j.mutate(json!({"type":"edit_scalar","target":{"entity":table["rows"][0]["id"],"field":table["columns"][0]["id"]},"input":{"kind":"text","value":"other\u{fffe}sheet"}}));
    let revision = j.table()["revision"].clone();
    let (reply, bytes) = j.sheet(&json!({"type":"export","expected_revision":revision,"metadata":j.metadata,"format":"csv","collection":j.metadata["sheets"][1]["schema_id"]}), &[]);
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(
        String::from_utf8(bytes.unwrap()).unwrap(),
        "Result\r\n2\r\n"
    );
    assert_eq!(j.current_export("xlsx").0["status"], "error");
    // Selected formula results are scalar-only; excluded styles/names/widths
    // and even a formula with XLSX-incompatible literals cannot block CSV.
    let mut other = formula_journey("5e-324/5e-324", "Data");
    other.metadata["sheets"][1]["name"] = json!("_x0041_");
    other.metadata["sheets"][1]["columns"][0]["width"] = json!(256);
    other.metadata["sheets"][1]["rows"][0]["styles"][0]["number_format"] = json!("invalid format");
    let revision = other.table()["revision"].clone();
    let (reply, bytes) = other.sheet(&json!({"type":"export","expected_revision":revision,"metadata":other.metadata,"format":"csv","collection":other.metadata["sheets"][1]["schema_id"]}), &[]);
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(
        String::from_utf8(bytes.unwrap()).unwrap(),
        "Result\r\n1\r\n"
    );
    j.records.extend(other.records);
    // Literal escape headers are ordinary CSV text, subject to CSV's own policy.
    j.import("csv");
    let field = j.table()["columns"][0]["id"].clone();
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":field,"name":"_x0041_"}));
    let (reply, bytes) = j.current_export("csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert!(
        String::from_utf8(bytes.unwrap())
            .unwrap()
            .starts_with("_x0041_,Amount")
    );
    assert_headerless_csv_output_scope(&mut j);
    j.capture("csv-output-scope");
}

fn assert_headerless_csv_output_scope(j: &mut Journey) {
    // Headerless CSV does not emit even the selected collection's names.
    let (reply, _) = j.sheet(&json!({"type":"import","format":"csv","csv_options":{"delimiter":",","header":false},"selection":{"column_types":[["text","number"]],"extra_columns":[[]]},"occurrence_id":ORIGINAL,"install":true}), b"first,1\n");
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = reply["response"]["payload"]["metadata"].clone();
    let field = j.table()["columns"][0]["id"].clone();
    j.mutate(
        json!({"type":"rename_column","collection":"sheet_1","field":field,"name":"=NotEmitted"}),
    );
    let (reply, bytes) = j.current_export("csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(String::from_utf8(bytes.unwrap()).unwrap(), "first,1\r\n");
    assert!(
        !reply["response"]["payload"]["ledger"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "imported_current_header")
    );
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(
        import_xlsx(&bytes.unwrap()).unwrap().sheets[0].columns[0].name,
        "=NotEmitted"
    );
}

fn imported_presentation_cases() -> Vec<(&'static str, Value, &'static str, &'static str)> {
    vec![
        ("sheet", json!("_x0041_"), "escape_sequence", "/sheet-name"),
        ("sheet", json!("😀".repeat(16)), "name", "/sheet-name"),
        ("sheet", json!("History"), "name", "/sheet-name"),
        (
            "number_format",
            json!("0\"_x0041_\""),
            "escape_sequence",
            "/number-format",
        ),
        (
            "number_format",
            json!("0\"bad\u{fffe}\""),
            "xml_character",
            "/number-format",
        ),
        (
            "number_format",
            json!("not a number format"),
            "style",
            "/style",
        ),
        ("alignment", json!("diagonal"), "style", "/style"),
        ("fill", json!("red"), "style", "/style"),
        ("width", json!(256), "width", "/column 2/width"),
        ("width", json!(0), "width", "/column 2/width"),
        ("width", json!(23.1), "width", "/column 2/width"),
        ("width", json!(5e-324), "width", "/column 2/width"),
        (
            "number_format",
            json!(format!("0\"{}\"", "x".repeat(252))),
            "format_limit",
            "/number-format",
        ),
        (
            "number_format",
            json!("[<5e-324]0"),
            "subnormal_number",
            "/number-format/condition 1",
        ),
        (
            "number_format",
            json!("[<1e-999]0"),
            "subnormal_number",
            "/number-format/condition 1",
        ),
    ]
}

fn assert_supported_imported_presentation(j: &mut Journey) {
    // Attribute whitespace must survive XML normalization, alongside all
    // supported style members and a fractional width.
    j.import("xlsx");
    let style = json!({"number_format":"0\"a\tb\nc\rd&<>\"", "bold":true,"fill":"123456","wrap":true,"border":true,"alignment":"right"});
    j.metadata["sheets"][0]["rows"][0]["styles"][1] = style.clone();
    j.metadata["sheets"][0]["columns"][1]["width"] = json!(23.25);
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let bytes = bytes.unwrap();
    let book = import_xlsx(&bytes).unwrap();
    let mut expected_style = style;
    expected_style["fill"] = json!("FF123456");
    assert_eq!(
        serde_json::to_value(&book.sheets[0].rows[0][1].style).unwrap(),
        expected_style
    );
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("xl/styles.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(
        xml.contains("a&#9;b&#10;c&#13;d&amp;&lt;&gt;"),
        "attribute whitespace must use XML character references: {xml}"
    );
    assert_eq!(book.sheets[0].columns[1].width, Some(23.25));
    for (format, precision) in [
        (format!("0\"{}\"", "x".repeat(251)), false),
        ("[<0.30000000000000004]0".into(), true),
        ("[<0.300000000000000000001]0".into(), true),
        ("[<0]0;[>=0]0".into(), false),
    ] {
        j.metadata["sheets"][0]["rows"][0]["styles"][1]["number_format"] = json!(format);
        let (reply, bytes) = j.current_export("xlsx");
        assert_eq!(reply["status"], "ok", "{reply}");
        let book = import_xlsx(&bytes.unwrap()).unwrap();
        assert_eq!(
            book.sheets[0].rows[0][1].style.number_format.as_deref(),
            Some(format.as_str())
        );
        assert_eq!(
            reply["response"]["payload"]["ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["code"] == "native_tables_xlsx_precision_loss"
                    && f["location"]
                        .as_str()
                        .unwrap()
                        .ends_with("/style/number-format")),
            precision
        );
    }
}

#[test]
fn imported_xlsx_presentation_surfaces_have_located_shared_policy_checks() {
    let mut j = Journey::default();
    let mut errors = Vec::new();
    for (key, value, code, path) in imported_presentation_cases() {
        j.import("csv");
        match key {
            "sheet" => j.metadata["sheets"][0]["name"] = value,
            "width" => j.metadata["sheets"][0]["columns"][1]["width"] = value,
            _ => j.metadata["sheets"][0]["rows"][0]["styles"][1][key] = value,
        }
        let (reply, bytes) = j.current_export("xlsx");
        let diagnostic = &reply["error"]["diagnostics"][0];
        if reply["status"] != "error"
            || !diagnostic["code"]
                .as_str()
                .is_some_and(|c| c.ends_with(code))
            || !diagnostic["path"]
                .as_str()
                .is_some_and(|p| p.ends_with(path))
        {
            errors.push(format!("{key}/{code}/{path}: {reply}"));
        } else {
            assert!(bytes.is_none());
        }
        if j.current_export("csv").0["status"] != "ok" {
            errors.push(format!("{key}: excluded presentation governs CSV"));
        }
    }
    assert_supported_imported_presentation(&mut j);
    assert!(errors.is_empty(), "{errors:?}");
    j.capture("presentation-output-policy");
}

#[test]
fn imported_style_inventory_refuses_before_excel_format_resource_limits() {
    use std::collections::BTreeMap;
    use tachiko_designer_runtime::DesignerRuntime;
    use tachiko_workspace_engine::{
        Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
        FieldType, Schema, SchemaId, SchemaKey, Value as Stored,
    };
    let mut document = Document::empty("style-inventory", "Style inventory");
    let mut sheets = Vec::new();
    for n in 1..=4 {
        let schema = SchemaId::from(format!("s{n}"));
        let field = FieldId::from(format!("f{n}"));
        document.schemas.insert(
            schema.clone(),
            Schema {
                field_order: None,
                id: schema.clone(),
                key: SchemaKey::from(format!("sheet_{n}")),
                display_name: None,
                fields: BTreeMap::from([(
                    field.clone(),
                    FieldDefinition {
                        id: field.clone(),
                        key: FieldKey::from("value"),
                        display_name: None,
                        number_format: None,
                        field_type: FieldType::Text,
                        required: false,
                        constraint: FieldConstraint::None,
                    },
                )]),
            },
        );
        let mut rows = Vec::new();
        for r in 0..51 {
            let entity = EntityId::from(format!("s{n}r{r}"));
            document.entities.insert(
                entity.clone(),
                Entity {
                    id: entity.clone(),
                    key: EntityKey::from(format!("s{n}r{r}")),
                    schema: schema.clone(),
                    fields: BTreeMap::from([(field.clone(), Stored::Text("v".into()))]),
                },
            );
            let mut style = serde_json::to_value(
                tachiko_designer_runtime::interop_adapter::CellStyle::default(),
            )
            .unwrap();
            style["fill"] = json!(format!("{:06X}", (n - 1) * 51 + r));
            rows.push(json!({"entity_id":entity.to_string(),"styles":[style]}));
        }
        sheets.push(json!({"schema_id":schema.to_string(),"name":format!("Sheet {n}"),"has_header":true,"columns":[{"field_id":field.to_string(),"name":"Value","width":null}],"rows":rows}));
    }
    // Saved admission avoids the import-preview receipt's independent size cap.
    // Every output element is still sent through the real host/WASM file route.
    let runtime = DesignerRuntime::from_document(document, ORIGINAL).unwrap();
    let project = runtime.export_project_v3("resident/0").unwrap().bytes;
    let mut j = Journey::default();
    let reply = HostReply {
        reply: j.host.open_project(&project, ORIGINAL.as_bytes()),
        bytes: None,
    };
    let (reply, _) = j.record("project_open", ORIGINAL.as_bytes(), &project, reply);
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = json!({"version":1,"sheets":sheets});
    assert_eq!(j.current_export("csv").0["status"], "ok");
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(
        reply["status"], "error",
        "unchecked style inventory: {reply}"
    );
    assert!(bytes.is_none());
    assert_eq!(
        reply["error"]["diagnostics"][0]["code"],
        "native_tables_xlsx_style_limit"
    );
    let original = j.metadata.clone();
    let default_style =
        serde_json::to_value(tachiko_designer_runtime::interop_adapter::CellStyle::default())
            .unwrap();
    for (index, row) in j.metadata["sheets"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .flat_map(|s| s["rows"].as_array_mut().unwrap())
        .enumerate()
    {
        if index >= 199 {
            row["styles"][0] = default_style.clone();
        }
    }
    // 199 distinct presented cells plus the generated default = 200 styles.
    assert_eq!(j.current_export("xlsx").0["status"], "ok");
    j.metadata["sheets"][3]["rows"][46]["styles"][0] =
        original["sheets"][3]["rows"][46]["styles"][0].clone();
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "error");
    assert!(bytes.is_none());
    assert_eq!(
        reply["error"]["diagnostics"][0]["code"],
        "native_tables_xlsx_style_limit"
    );
    j.capture("style-inventory-policy");
}

#[test]
fn imported_reference_names_headers_and_csv_quoting_preserve_spelling() {
    let name = "O'Brien 5e-324 & <Data>";
    let formula = "MAX('O''Brien 5e-324 & <Data>'!$B$2,2)";
    let mut j = formula_journey(formula, name);
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let book = import_xlsx(&bytes.unwrap()).unwrap();
    assert_eq!(book.sheets[0].name, name);
    assert_eq!(book.sheets[1].rows[0][0].formula.as_deref(), Some(formula));
    assert_eq!(
        book.sheets[1].rows[0][0].value,
        SourceValue::Number { value: 2.0 }
    );
    let table = j.table();
    let header = "Quoted, \"&<>\" column";
    j.mutate(json!({"type":"rename_column","collection":"sheet_1","field":table["columns"][0]["id"],"name":header}));
    for format in ["csv", "xlsx"] {
        let (reply, bytes) = j.current_export(format);
        assert_eq!(reply["status"], "ok", "{reply}");
        let bytes = bytes.unwrap();
        let book = if format == "csv" {
            import_csv(&bytes, &ImportOptions::default()).unwrap()
        } else {
            import_xlsx(&bytes).unwrap()
        };
        assert_eq!(book.sheets[0].columns[0].name, header);
        assert!(
            reply["response"]["payload"]["ledger"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["code"] == "imported_current_header")
        );
    }
    // The same fixed-point caseless worksheet inventory governs references
    // and workbook names; it is irrelevant to selected-sheet CSV bytes.
    j.metadata["sheets"][0]["name"] = json!("ß");
    j.metadata["sheets"][1]["name"] = json!("SS");
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "error", "{reply}");
    assert!(bytes.is_none());
    assert_eq!(
        reply["error"]["diagnostics"][0]["code"],
        "native_tables_xlsx_name"
    );
    assert_eq!(j.current_export("csv").0["status"], "ok");
    // Imported header spellings may contain line endings even though native
    // rename admission forbids them. They still reach the output policy.
    j.import("csv");
    let header = "Quoted, \"&<>\"\r\ncolumn";
    j.metadata["sheets"][0]["columns"][0]["name"] = json!(header);
    for format in ["csv", "xlsx"] {
        let (reply, bytes) = j.current_export(format);
        assert_eq!(reply["status"], "ok", "{reply}");
        let bytes = bytes.unwrap();
        let book = if format == "csv" {
            import_csv(&bytes, &ImportOptions::default()).unwrap()
        } else {
            import_xlsx(&bytes).unwrap()
        };
        assert_eq!(book.sheets[0].columns[0].name, header);
    }
    j.metadata["sheets"][0]["columns"][0]["name"] = json!(format!("{}z", "a\n".repeat(254)));
    assert_eq!(j.current_export("csv").0["status"], "ok");
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "error", "{reply}");
    assert!(bytes.is_none());
    assert_eq!(
        reply["error"]["diagnostics"][0]["code"],
        "native_tables_xlsx_line_feeds"
    );
    j.capture("references-headers-policy");
}

#[test]
fn imported_formula_precision_disclosures_share_the_bounded_location_budget() {
    let mut j = formula_journey_rows("0.30000000000000004*0", "Data", 32);
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let ledger = reply["response"]["payload"]["ledger"].as_array().unwrap();
    let locations: Vec<_> = ledger
        .iter()
        .filter(|f| f["code"] == "native_tables_xlsx_precision_loss")
        .collect();
    assert_eq!(locations.len(), 20);
    assert!(
        locations
            .iter()
            .all(|f| f["location"].as_str().unwrap().ends_with("/formula"))
    );
    let counts: Vec<_> = ledger
        .iter()
        .filter(|f| f["code"] == "native_tables_xlsx_precision_count")
        .collect();
    assert_eq!(counts.len(), 1);
    let message = counts[0]["message"].as_str().unwrap();
    assert!(
        message.starts_with("32 Number cells, formulas or number formats"),
        "{message}"
    );
    assert!(
        message.contains("12 additional locations omitted"),
        "{message}"
    );
    let book = import_xlsx(&bytes.unwrap()).unwrap();
    assert!(
        book.sheets[1]
            .rows
            .iter()
            .all(|r| r[0].value == SourceValue::Number { value: 0.0 }
                && r[0].formula.as_deref() == Some("(0.30000000000000004*0)"))
    );
    j.capture("precision-budget-policy");
}

#[test]
fn imported_formula_grammar_and_nonfinite_numbers_cannot_reach_file_output() {
    use tachiko_designer_runtime::interop_adapter::{SourceCell, SourceColumn, SourceSheet};
    let mut j = Journey::default();
    j.import("csv");
    let long_formula = format!("\"{}\"", "x".repeat(32_768));
    for formula in [
        "1e309",
        "NaN",
        "INF",
        "TRUE()",
        "FALSE()",
        "\"_x0041_\"",
        "\"bad\u{fffe}\"",
        "\"long string\"",
        &long_formula,
    ] {
        let source = tachiko_designer_runtime::interop_adapter::SourceWorkbook {
            sheets: vec![SourceSheet {
                name: "Input".into(),
                has_header: true,
                columns: vec![SourceColumn {
                    name: "Result".into(),
                    width: None,
                    number_format: None,
                }],
                rows: vec![vec![SourceCell {
                    value: SourceValue::Number { value: 1.0 },
                    formula: Some(formula.into()),
                    ..SourceCell::default()
                }]],
            }],
            ledger: vec![],
        };
        // XML-illegal formula text is rejected even before import encoding.
        if formula.contains('\u{fffe}') {
            assert!(export_xlsx(&source).is_err());
            continue;
        }
        let before = j.table();
        let (reply, bytes) = j.sheet(&json!({"type":"import","format":"xlsx","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["number"]],"extra_columns":[[]]},"occurrence_id":FRESH,"install":true}), &export_xlsx(&source).unwrap());
        assert_eq!(
            reply["status"], "error",
            "unsupported formula {formula}: {reply}"
        );
        assert!(bytes.is_none());
        assert_eq!(
            j.table(),
            before,
            "rejected input cannot replace the occurrence"
        );
    }
    let table = j.table();
    for input in ["NaN", "inf", "-inf", "1e309"] {
        let request = serde_json::to_vec(&json!({"type":"edit_scalar","expected_revision":table["revision"],"target":{"entity":table["rows"][0]["id"],"field":table["columns"][1]["id"]},"input":{"kind":"number","input":input}})).unwrap();
        let reply = HostReply {
            reply: j.host.request(&request),
            bytes: None,
        };
        assert_eq!(
            j.record("request_run", &request, &[], reply).0["status"],
            "error"
        );
        assert_eq!(j.table(), table);
    }
    j.capture("closed-grammar-policy");
}

#[test]
fn imported_boolean_values_use_typed_xlsx_and_lexical_csv() {
    let mut j = Journey::default();
    let mut source = import_csv(b"Flag\ntrue\nfalse\n", &ImportOptions::default()).unwrap();
    source.sheets[0].rows[0][0].value = SourceValue::Boolean { value: true };
    source.sheets[0].rows[1][0].value = SourceValue::Boolean { value: false };
    let bytes = export_xlsx(&source).unwrap();
    let (reply, _) = j.sheet(&json!({"type":"import","format":"xlsx","csv_options":{"delimiter":",","header":true},"selection":{"column_types":[["boolean"]],"extra_columns":[[]]},"occurrence_id":ORIGINAL,"install":true}), &bytes);
    assert_eq!(reply["status"], "ok", "{reply}");
    j.metadata = reply["response"]["payload"]["metadata"].clone();
    let (reply, bytes) = j.current_export("xlsx");
    assert_eq!(reply["status"], "ok", "{reply}");
    let book = import_xlsx(&bytes.unwrap()).unwrap();
    assert_eq!(
        book.sheets[0].rows[0][0].value,
        SourceValue::Boolean { value: true }
    );
    assert_eq!(
        book.sheets[0].rows[1][0].value,
        SourceValue::Boolean { value: false }
    );
    let (reply, bytes) = j.current_export("csv");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(
        String::from_utf8(bytes.unwrap()).unwrap(),
        "Flag\r\ntrue\r\nfalse\r\n"
    );
    j.capture("boolean-output-policy");
}
