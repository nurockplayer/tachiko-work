//! The same real XLSX/CSV bytes must refuse atomically through both arenas.
use std::io::{Cursor, Read, Write};

use super::{
    PROJECT, REQUEST, RESPONSE, RUNTIME, tachiko_designer_project_close,
    tachiko_designer_spreadsheet_run,
};
use crate::host_bridge::DesignerHost;
use crate::interop_adapter::{
    ImportOptions, InteropError, OutputProfile, SourceValue, SourceWorkbook,
    export_csv_for_profile, export_xlsx, import_csv, import_xlsx, validate_output_for_profile,
};
use crate::{
    CapacityExceeded, CapacityLimit, DesignerError, ImportFieldType, ImportSelection,
    NativeBudgetExportCollection, NativeBudgetExportPresentation, NativeBudgetExportRow,
    NativeBudgetExportView, NativeTrackerExportPresentation, NativeTrackerExportRow,
    import_workbook,
};
use serde_json::Value;
use tachiko_workspace_engine::{
    Entity, EntityId, EntityKey, FieldId, Number, SchemaId, Value as StoredValue,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000638";

fn column(index: usize) -> String {
    if index < 26 {
        char::from(b'A' + u8::try_from(index).unwrap()).to_string()
    } else {
        format!("A{}", char::from(b'A' + u8::try_from(index - 26).unwrap()))
    }
}

// Start from a real exported package, then replace its geometry without asking
// the bounded exporter to generate a workbook it is required to refuse.
#[allow(clippy::format_push_string)] // Small, deliberately oversized XML test inputs.
fn source_xlsx(sheets: usize, columns: usize, rows: usize, extra: bool, formulas: bool) -> Vec<u8> {
    let book = import_csv(b"Name\nvalue\n", &ImportOptions::default()).unwrap();
    let bytes = export_xlsx(&book).unwrap();
    let mut source = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry.name().to_owned();
        if name.starts_with("xl/worksheets/") {
            continue;
        }
        let mut xml = String::new();
        entry.read_to_string(&mut xml).unwrap();
        if name == "xl/workbook.xml" {
            let start = xml.find("<sheets>").unwrap() + "<sheets>".len();
            let end = xml.find("</sheets>").unwrap();
            let mut nodes = String::new();
            for sheet in 1..=sheets {
                nodes.push_str(&format!(
                    "<sheet name=\"Sales{sheet}\" sheetId=\"{sheet}\" r:id=\"rId{sheet}\"/>"
                ));
            }
            xml.replace_range(start..end, &nodes);
        } else if name == "xl/_rels/workbook.xml.rels" {
            // Keep styles and the first worksheet relationship.
            let mut nodes = String::new();
            for sheet in 2..=sheets {
                nodes.push_str(&format!("<Relationship Id=\"sheet{sheet}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{sheet}.xml\"/>"));
            }
            xml = xml.replace("</Relationships>", &format!("{nodes}</Relationships>"));
            // Export uses rId1 for the first worksheet; new ids cannot collide
            // with its styles relationship.
        } else if name == "[Content_Types].xml" {
            let mut nodes = String::new();
            for sheet in 2..=sheets {
                nodes.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{sheet}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
            }
            xml = xml.replace("</Types>", &format!("{nodes}</Types>"));
        }
        if name == "xl/workbook.xml" {
            for sheet in 2..=sheets {
                xml = xml.replace(
                    &format!("r:id=\"rId{sheet}\""),
                    &format!("r:id=\"sheet{sheet}\""),
                );
            }
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    for sheet in 1..=sheets {
        let mut xml = String::from(
            "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><cols><col min=\"1\" max=\"1\" width=\"12\"/></cols><sheetData><row r=\"1\">",
        );
        for col in 0..columns {
            xml.push_str(&format!(
                "<c r=\"{}1\" t=\"inlineStr\"><is><t>C{col}</t></is></c>",
                column(col)
            ));
        }
        xml.push_str("</row>");
        for row in 2..=rows + 1 + usize::from(extra && sheet == 1) {
            xml.push_str(&format!("<row r=\"{row}\">"));
            for col in 0..columns {
                let formula = if formulas { "<f>1+1</f>" } else { "" };
                xml.push_str(&format!(
                    "<c r=\"{}{row}\" t=\"n\">{formula}<v>2</v></c>",
                    column(col)
                ));
            }
            xml.push_str("</row>");
        }
        xml.push_str("</sheetData></worksheet>");
        output
            .start_file(
                format!("xl/worksheets/sheet{sheet}.xml"),
                SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn assert_capacity(error: &Value, capacity: &CapacityExceeded) {
    assert_eq!(error["code"], "capacity_exceeded", "{error}");
    let expected = serde_json::to_value(capacity).unwrap();
    for field in ["limit", "attempted", "maximum", "collection"] {
        assert_eq!(error[field], expected[field], "{field}: {error}");
    }
    assert_eq!(
        error.get("collection").is_some(),
        capacity.collection.is_some()
    );
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains(&capacity.attempted.to_string()),
        "{message}"
    );
    assert!(message.contains(&capacity.maximum.to_string()), "{message}");
    assert!(
        message.contains("The workbook was not imported."),
        "{message}"
    );
    if let Some(name) = &capacity.collection {
        assert!(message.contains(name), "{message}");
    }
}

fn wire_refusal(
    source: &[u8],
    format: &str,
    expected: Option<&CapacityExceeded>,
    lower_bound: bool,
) {
    tachiko_designer_project_close();
    let mut host = DesignerHost::new();
    let new = format!(r#"{{"type":"new_tracker","occurrence_id":"{OCCURRENCE}"}}"#);
    // A refusal must preserve an existing resident document.
    let before = host.request(new.as_bytes());
    REQUEST.with(|slot| *slot.borrow_mut() = new.as_bytes().to_vec());
    super::tachiko_designer_request_run();
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), before);
    let query = br#"{"type":"query_capacity"}"#;
    let before = host.request(query);
    for operation in ["inspect", "import"] {
        let mut request: Value = serde_json::from_str(&format!(r#"{{"type":"{operation}","format":"{format}","csv_options":{{"delimiter":",","header":true}}}}"#)).unwrap();
        if operation == "import" {
            request["selection"] =
                serde_json::from_str(r#"{"column_types":[],"extra_columns":[]}"#).unwrap();
            request["occurrence_id"] = OCCURRENCE.into();
            request["install"] = true.into();
        }
        let request = serde_json::to_vec(&request).unwrap();
        REQUEST.with(|slot| slot.borrow_mut().clone_from(&request));
        PROJECT.with(|slot| *slot.borrow_mut() = source.to_vec());
        tachiko_designer_spreadsheet_run();
        let native = host.spreadsheet(&request, source);
        assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), native.reply);
        assert!(native.bytes.is_none());
        let reply: Value = serde_json::from_slice(&native.reply).unwrap();
        assert_eq!(reply["status"], "error", "{reply}");
        if let Some(capacity) = expected {
            assert_capacity(&reply["error"], capacity);
            assert_eq!(
                reply["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("at least"),
                lower_bound
            );
        } else {
            assert_eq!(reply["error"]["code"], "invalid_project", "{reply}");
        }
        assert_eq!(host.request(query), before);
        REQUEST.with(|slot| *slot.borrow_mut() = query.to_vec());
        super::tachiko_designer_request_run();
        assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), before);
    }
    tachiko_designer_project_close();
}

#[test]
fn import_capacity_errors_survive_runtime_host_and_wasm_wire() {
    for (
        sheets,
        columns,
        rows,
        extra,
        formulas,
        limit,
        attempted,
        maximum,
        collection,
        lower_bound,
    ) in [
        (
            33,
            1,
            1,
            false,
            false,
            CapacityLimit::Collections,
            33,
            32,
            None,
            false,
        ),
        (
            1,
            33,
            1,
            false,
            false,
            CapacityLimit::TableFields,
            33,
            32,
            Some("Sales1"),
            true,
        ),
        (
            1,
            1,
            129,
            false,
            false,
            CapacityLimit::TableRows,
            129,
            128,
            Some("Sales1"),
            false,
        ),
        (
            32,
            1,
            32,
            true,
            false,
            CapacityLimit::TotalEntities,
            1025,
            1024,
            None,
            false,
        ),
        (
            1,
            3,
            86,
            false,
            true,
            CapacityLimit::Formulas,
            257,
            256,
            None,
            true,
        ),
    ] {
        let bytes = source_xlsx(sheets, columns, rows, extra, formulas);
        let expected = CapacityExceeded {
            limit,
            attempted,
            maximum,
            collection: collection.map(str::to_owned),
        };
        let error = import_xlsx(&bytes).unwrap_err();
        let actual = match error {
            InteropError::Capacity(capacity) | InteropError::CapacityLowerBound(capacity) => {
                capacity
            }
            InteropError::Invalid(message) | InteropError::InvalidText(message) => {
                panic!("{message}")
            }
        };
        assert_eq!(actual, expected);
        wire_refusal(&bytes, "xlsx", Some(&expected), lower_bound);
    }
}

#[test]
fn csv_column_capacity_survives_host_and_wasm_wire() {
    let csv = format!(
        "{}\n",
        (0..33)
            .map(|index| format!("C{index}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let expected = CapacityExceeded {
        limit: CapacityLimit::TableFields,
        attempted: 33,
        maximum: 32,
        collection: Some("Imported table".into()),
    };
    assert!(matches!(
        import_csv(csv.as_bytes(), &ImportOptions::default()),
        Err(InteropError::Capacity(_))
    ));
    wire_refusal(csv.as_bytes(), "csv", Some(&expected), false);
}

#[test]
fn direct_runtime_selection_capacity_names_the_source_sheet() {
    for (sheets, columns, rows, formulas, limit, attempted, maximum, collection) in [
        (33, 1, 1, false, CapacityLimit::Collections, 33, 32, None),
        (
            1,
            33,
            1,
            false,
            CapacityLimit::TableFields,
            33,
            32,
            Some("Sales1"),
        ),
        (
            1,
            1,
            129,
            false,
            CapacityLimit::TableRows,
            129,
            128,
            Some("Sales1"),
        ),
        (
            9,
            1,
            113,
            false,
            CapacityLimit::TotalEntities,
            1025,
            1024,
            None,
        ),
        (1, 3, 86, true, CapacityLimit::Formulas, 258, 256, None),
    ] {
        let mut book: SourceWorkbook = import_xlsx(&source_xlsx(1, 1, 1, false, false)).unwrap();
        book.sheets[0].name = "Sales1".into();
        book.sheets[0].columns = vec![book.sheets[0].columns[0].clone(); columns];
        for (index, column) in book.sheets[0].columns.iter_mut().enumerate() {
            column.name = format!("C{index}");
        }
        let mut cell = book.sheets[0].rows[0][0].clone();
        if formulas {
            cell.formula = Some("1+1".into());
        }
        book.sheets[0].rows = vec![vec![cell; columns]; rows];
        book.sheets = vec![book.sheets[0].clone(); sheets];
        if limit == CapacityLimit::TotalEntities {
            let row = book.sheets[0].rows[0].clone();
            book.sheets[0].rows.extend([
                row.clone(),
                row.clone(),
                row.clone(),
                row.clone(),
                row.clone(),
                row.clone(),
                row.clone(),
                row,
            ]);
        }
        let selection = ImportSelection {
            column_types: vec![vec![ImportFieldType::Number; columns]; sheets],
            extra_columns: vec![Vec::new(); sheets],
        };
        let Err(error) = import_workbook(&book, &selection, OCCURRENCE) else {
            panic!("over-limit workbook imported");
        };
        assert!(matches!(
            error,
            DesignerError::ImportCapacityExceeded { .. }
        ));
        assert_capacity(
            &serde_json::to_value(error.failure_projection("resident/0")).unwrap(),
            &CapacityExceeded {
                limit,
                attempted,
                maximum,
                collection: collection.map(str::to_owned),
            },
        );
    }
}

#[test]
fn malformed_geometry_remains_invalid_project() {
    // These are syntax/range errors, even where the column also exceeds 32.
    for replacement in [
        ("r=\"A2\"", "r=\"AGbad\""),
        ("max=\"1\"", "max=\"16385\""),
        ("max=\"1\"", "max=\"4294967296\""),
        ("<sheetData>", "<dimension ref=\"AGbad\"/><sheetData>"),
    ] {
        let bytes = source_xlsx(1, 1, 1, false, false);
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let mut xml = String::new();
            entry.read_to_string(&mut xml).unwrap();
            if entry.name() == "xl/worksheets/sheet1.xml" {
                xml = xml.replace(replacement.0, replacement.1);
            }
            output
                .start_file(entry.name(), SimpleFileOptions::default())
                .unwrap();
            output.write_all(xml.as_bytes()).unwrap();
        }
        wire_refusal(&output.finish().unwrap().into_inner(), "xlsx", None, false);
    }
}

// Generate package XML directly so the bounded output encoder is never asked
// to create an input outside its own profile.
#[allow(clippy::format_push_string)]
fn working_xlsx(rows: usize, columns: usize, populated: usize, formulas: bool) -> Vec<u8> {
    let template = source_xlsx(1, 1, 1, false, false);
    let mut archive = ZipArchive::new(Cursor::new(template)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            let mut xml = String::from(
                "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\">",
            );
            for col in 0..columns {
                xml.push_str(&format!(
                    "<c r=\"{}1\" t=\"inlineStr\"><is><t>C{col}</t></is></c>",
                    column(col)
                ));
            }
            xml.push_str("</row>");
            let mut count = 0;
            for row in 2..=rows + 1 {
                xml.push_str(&format!("<row r=\"{row}\">"));
                for col in 0..columns {
                    if count >= populated {
                        break;
                    }
                    // Sparse success fixtures distribute four values across every row.
                    if populated == rows * 4 && col >= 4 {
                        break;
                    }
                    let address = format!("{}{row}", column(col));
                    if formulas {
                        xml.push_str(&format!("<c r=\"{address}\"><f>1+1</f><v>2</v></c>"));
                    } else if populated == rows * 4 && col == 3 {
                        xml.push_str(&format!(
                            "<c r=\"{address}\"><f>B{row}*C{row}</f><v>6</v></c>"
                        ));
                    } else {
                        let value = if col == 0 {
                            row - 1
                        } else if col == 1 {
                            2
                        } else {
                            3
                        };
                        xml.push_str(&format!("<c r=\"{address}\"><v>{value}</v></c>"));
                    }
                    count += 1;
                }
                xml.push_str("</row>");
            }
            xml.push_str("</sheetData></worksheet>");
            bytes = xml.into_bytes();
        }
        output.start_file(name, options).unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn csv_options() -> Value {
    object(vec![("delimiter", ",".into()), ("header", true.into())])
}

fn working_request(format: &str, columns: usize) -> Vec<u8> {
    let kind = if format == "csv" { "text" } else { "number" };
    let column_types = Value::Array(vec![Value::from(vec![kind; columns])]);
    let extra_columns = Value::Array(vec![Value::Array(Vec::new())]);
    serde_json::to_vec(&object(vec![
        ("type", "import".into()),
        ("format", format.into()),
        ("csv_options", csv_options()),
        (
            "selection",
            object(vec![
                ("column_types", column_types),
                ("extra_columns", extra_columns),
            ]),
        ),
        ("occurrence_id", OCCURRENCE.into()),
        ("install", true.into()),
        ("profile", "working_size".into()),
    ]))
    .unwrap()
}

fn assert_working_success(source: &[u8], format: &str) {
    let mut host = DesignerHost::new();
    let inspect = serde_json::to_vec(&object(vec![
        ("type", "inspect".into()),
        ("format", format.into()),
        ("csv_options", csv_options()),
        ("profile", "working_size".into()),
    ]))
    .unwrap();
    let inspected = host.spreadsheet(&inspect, source);
    let inspected: Value = serde_json::from_slice(&inspected.reply).unwrap();
    assert_eq!(inspected["status"], "ok", "{inspected}");
    drop(inspected);
    let request = working_request(format, 30);
    let imported = host.spreadsheet(&request, source);
    tachiko_designer_project_close();
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&request));
    PROJECT.with(|slot| *slot.borrow_mut() = source.to_vec());
    tachiko_designer_spreadsheet_run();
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), imported.reply);
    let response: Value = serde_json::from_slice(&imported.reply).unwrap();
    assert_eq!(response["status"], "ok", "{response}");
    let payload = &response["response"]["payload"];
    let window = &payload["opened"]["window"];
    assert_eq!(window["total_rows"], 10_000);
    let revision = window["revision"].as_str().unwrap();
    let collection = window["collection"]["id"].as_str().unwrap();
    let control = serde_json::to_vec(&object(vec![
        ("type", "export".into()),
        ("format", format.into()),
        ("expected_revision", revision.into()),
        ("collection", collection.into()),
        ("metadata", payload["metadata"].clone()),
    ]))
    .unwrap();
    assert!(control.len() < 4 * 1024 * 1024);
    let exported = host.spreadsheet(&control, &[]);
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&control));
    PROJECT.with(|slot| slot.borrow_mut().clear());
    tachiko_designer_spreadsheet_run();
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), exported.reply);
    let reply: Value = serde_json::from_slice(&exported.reply).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    let bytes = exported.bytes.unwrap();
    assert_eq!(PROJECT.with(|slot| slot.borrow().clone()), bytes);
    tachiko_designer_project_close();
    assert_export_round_trip(source, &bytes, &request, format);
    let mut last_id = String::new();
    let mut start = 0_usize;
    while start < 10_000 {
        let query = serde_json::to_vec(&object(vec![
            ("type", "query_table_window".into()),
            ("expected_revision", revision.into()),
            ("collection", collection.into()),
            ("start", start.into()),
            ("count", 256_u32.into()),
        ]))
        .unwrap();
        let reply: Value = serde_json::from_slice(&host.request(&query)).unwrap();
        assert_eq!(reply["status"], "ok", "{reply}");
        let rows = reply["response"]["payload"]["rows"].as_array().unwrap();
        assert_ne!(rows.len(), 0);
        for row in rows {
            let id = row["id"].as_str().unwrap();
            assert!(last_id.as_str() < id);
            id.clone_into(&mut last_id);
            if start == 0 || start == 9_999 {
                let value = &row["fields"][0]["stored"]["value"];
                if format == "csv" {
                    assert_eq!(value.as_str().unwrap(), (start + 1).to_string());
                } else {
                    assert_eq!(
                        value.as_f64().unwrap(),
                        f64::from(u32::try_from(start + 1).unwrap())
                    );
                    assert_eq!(row["fields"][3]["calculated"]["value"], 6.0);
                }
            }
            start += 1;
        }
    }
}

fn assert_export_round_trip(source: &[u8], bytes: &[u8], request: &[u8], format: &str) {
    let options = ImportOptions::default();
    let original = if format == "xlsx" {
        crate::interop_adapter::import_xlsx_profile(source, &options, true).unwrap()
    } else {
        crate::interop_adapter::import_csv_profile(source, &options, true).unwrap()
    };
    let roundtrip = if format == "xlsx" {
        crate::interop_adapter::import_xlsx_profile(bytes, &options, true).unwrap()
    } else {
        crate::interop_adapter::import_csv_profile(bytes, &options, true).unwrap()
    };
    assert_eq!(roundtrip.sheets[0].columns, original.sheets[0].columns);
    assert_eq!(
        roundtrip.sheets[0].rows.len(),
        original.sheets[0].rows.len()
    );
    for (before, after) in original.sheets[0]
        .rows
        .iter()
        .zip(&roundtrip.sheets[0].rows)
    {
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(after) {
            assert_eq!(before.value, after.value);
            assert_eq!(before.style, after.style);
            assert_eq!(before.formula.is_some(), after.formula.is_some());
        }
    }
    if format == "xlsx" {
        let mut again = DesignerHost::new();
        let imported = again.spreadsheet(request, bytes);
        let reply: Value = serde_json::from_slice(&imported.reply).unwrap();
        assert_eq!(reply["status"], "ok", "{reply}");
        let payload = &reply["response"]["payload"];
        let window = &payload["opened"]["window"];
        let control = serde_json::to_vec(&object(vec![
            ("type", "export".into()),
            ("format", format.into()),
            ("expected_revision", window["revision"].clone()),
            ("collection", window["collection"]["id"].clone()),
            ("metadata", payload["metadata"].clone()),
        ]))
        .unwrap();
        let exported = again.spreadsheet(&control, &[]);
        let reply: Value = serde_json::from_slice(&exported.reply).unwrap();
        assert_eq!(reply["status"], "ok", "{reply}");
        let fixed =
            crate::interop_adapter::import_xlsx_profile(&exported.bytes.unwrap(), &options, true)
                .unwrap();
        assert_eq!(roundtrip.sheets[0].rows.len(), fixed.sheets[0].rows.len());
        for (first, second) in roundtrip.sheets[0].rows.iter().zip(&fixed.sheets[0].rows) {
            assert_eq!(first.len(), second.len());
            for (first, second) in first.iter().zip(second) {
                assert_eq!(first.formula, second.formula);
            }
        }
    }
}

fn assert_working_refusal(source: &[u8], format: &str, limit: &str) {
    tachiko_designer_project_close();
    let mut host = DesignerHost::new();
    let new = format!(r#"{{"type":"new_tracker","occurrence_id":"{OCCURRENCE}"}}"#);
    let before = host.request(new.as_bytes());
    REQUEST.with(|slot| *slot.borrow_mut() = new.as_bytes().to_vec());
    super::tachiko_designer_request_run();
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), before);
    let resident = host.observe_occurrence();
    let query = br#"{"type":"query_capacity"}"#;
    let before_capacity = host.request(query);
    for operation in ["inspect", "import"] {
        let mut request: Value = serde_json::from_slice(&working_request(format, 30)).unwrap();
        request["type"] = operation.into();
        if operation == "inspect" {
            for field in ["selection", "occurrence_id", "install"] {
                let _ = request.as_object_mut().unwrap().remove(field);
            }
        }
        let request = serde_json::to_vec(&request).unwrap();
        REQUEST.with(|slot| slot.borrow_mut().clone_from(&request));
        PROJECT.with(|slot| *slot.borrow_mut() = source.to_vec());
        tachiko_designer_spreadsheet_run();
        let reply = host.spreadsheet(&request, source);
        assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), reply.reply);
        let value: Value = serde_json::from_slice(&reply.reply).unwrap();
        assert_eq!(value["error"]["code"], "capacity_exceeded", "{value}");
        assert_eq!(value["error"]["limit"], limit, "{value}");
        assert_eq!(host.observe_occurrence(), resident);
        REQUEST.with(|slot| *slot.borrow_mut() = query.to_vec());
        super::tachiko_designer_request_run();
        assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), before_capacity);
    }
    tachiko_designer_project_close();
}

#[test]
fn working_size_xlsx_last_excel_row_refuses_before_allocation() {
    let source = source_xlsx(1, 1, 1, false, false);
    let mut input = ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace("r=\"2\"", "r=\"1048576\"")
                .replace("A2", "A1048576")
                .into_bytes();
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    let bytes = output.finish().unwrap().into_inner();
    let error =
        crate::interop_adapter::import_xlsx_profile(&bytes, &ImportOptions::default(), true)
            .unwrap_err();
    assert_eq!(
        error,
        InteropError::CapacityLowerBound(CapacityExceeded {
            limit: CapacityLimit::TableRows,
            maximum: crate::capacity::WORKING_LIMITS.table_rows,
            attempted: 1_048_575,
            collection: Some("Sales1".into()),
        })
    );
    assert_working_refusal(&bytes, "xlsx", "table_rows");
}

#[test]
fn working_size_output_bounds_are_typed() {
    let mut book = import_xlsx(&source_xlsx(1, 1, 1, false, false)).unwrap();
    let row = book.sheets[0].rows[0].clone();
    book.sheets[0].rows = vec![row; crate::capacity::WORKING_LIMITS.table_rows + 1];
    let error = validate_output_for_profile(&book, OutputProfile::WorkingSize).unwrap_err();
    assert_eq!(
        error,
        InteropError::Capacity(CapacityExceeded {
            limit: CapacityLimit::TableRows,
            maximum: crate::capacity::WORKING_LIMITS.table_rows,
            attempted: crate::capacity::WORKING_LIMITS.table_rows + 1,
            collection: Some("Sales1".into()),
        })
    );

    book.sheets[0].rows.truncate(1);
    book.sheets[0].rows[0][0].value = SourceValue::Text {
        value: "x".repeat(crate::interop_adapter::MAX_NATIVE_TABLES_OUTPUT_BYTES),
    };
    let error = export_csv_for_profile(&book.sheets[0], OutputProfile::WorkingSize).unwrap_err();
    assert!(matches!(
        error,
        InteropError::Capacity(CapacityExceeded {
            limit: CapacityLimit::OutputBytes,
            maximum: crate::interop_adapter::MAX_NATIVE_TABLES_OUTPUT_BYTES,
            ..
        })
    ));

    book.sheets[0].rows[0][0].value = SourceValue::Number { value: 2.0 };
    let column = book.sheets[0].columns[0].clone();
    book.sheets[0].columns = vec![column; crate::capacity::WORKING_LIMITS.table_fields + 1];
    assert!(matches!(
        validate_output_for_profile(&book, OutputProfile::WorkingSize),
        Err(InteropError::Capacity(CapacityExceeded {
            limit: CapacityLimit::TableFields,
            attempted: 33,
            maximum: 32,
            ..
        }))
    ));

    book.sheets[0].columns.truncate(11);
    for (index, column) in book.sheets[0].columns.iter_mut().enumerate() {
        column.name = format!("C{index}");
    }
    let mut cell = book.sheets[0].rows[0][0].clone();
    cell.formula = Some("1+1".into());
    book.sheets[0].rows = vec![vec![cell; 11]; 9_091];
    assert!(matches!(
        validate_output_for_profile(&book, OutputProfile::WorkingSize),
        Err(InteropError::Capacity(CapacityExceeded {
            limit: CapacityLimit::Formulas,
            attempted: 100_001,
            maximum: 100_000,
            ..
        }))
    ));
}

#[test]
fn working_size_output_bytes_refusal_matches_host_and_wasm() {
    let mut document = crate::DesignerRuntime::tracker_document(OCCURRENCE);
    for index in 0..9_000 {
        let id = EntityId::from(format!("row-{index:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(id.as_str()),
                schema: SchemaId::from("tracker"),
                fields: [
                    (FieldId::from("task"), StoredValue::Text("x".repeat(1024))),
                    (
                        FieldId::from("estimate"),
                        StoredValue::Number(Number::new(1.0).unwrap()),
                    ),
                    (FieldId::from("done"), StoredValue::Boolean(false)),
                ]
                .into_iter()
                .collect(),
            },
        );
    }
    let runtime =
        crate::DesignerRuntime::from_document_working_size(document.clone(), OCCURRENCE).unwrap();
    let project = runtime.export_project_v3("resident/0").unwrap().bytes;
    let mut host = DesignerHost::new();
    let opened: Value =
        serde_json::from_slice(&host.open_project_working_size(&project, OCCURRENCE.as_bytes()))
            .unwrap();
    assert_eq!(opened["status"], "ok", "{opened}");
    RUNTIME.with(|slot| {
        *slot.borrow_mut() =
            Some(crate::DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap());
    });
    let before = host.observe_occurrence();
    let request = serde_json::to_vec(&object(vec![
        ("type", "export_native_tables".into()),
        ("expected_revision", "resident/0".into()),
        ("collection", "tracker".into()),
        ("format", "csv".into()),
    ]))
    .unwrap();
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&request));
    PROJECT.with(|slot| slot.borrow_mut().clear());
    tachiko_designer_spreadsheet_run();
    let reply = host.spreadsheet(&request, &[]);
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), reply.reply);
    let error: Value = serde_json::from_slice(&reply.reply).unwrap();
    assert_eq!(error["error"]["code"], "capacity_exceeded", "{error}");
    assert_eq!(error["error"]["limit"], "output_bytes", "{error}");
    assert_eq!(
        error["error"]["maximum"],
        crate::interop_adapter::MAX_NATIVE_TABLES_OUTPUT_BYTES
    );
    assert_eq!(error["error"]["attempted"], 9_306_020_u64);
    assert!(reply.bytes.is_none());
    assert_eq!(host.observe_occurrence(), before);
    tachiko_designer_project_close();
}

fn tracker_document_with_rows(count: usize) -> crate::Document {
    let mut document = crate::DesignerRuntime::tracker_document(OCCURRENCE);
    for index in 0..count {
        let id = EntityId::from(format!("row-{index:05}"));
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(id.as_str()),
                schema: SchemaId::from("tracker"),
                fields: [
                    (
                        FieldId::from("task"),
                        StoredValue::Text(format!("Task {index}")),
                    ),
                    (
                        FieldId::from("estimate"),
                        StoredValue::Number(Number::new(1.0).unwrap()),
                    ),
                    (FieldId::from("done"), StoredValue::Boolean(false)),
                ]
                .into_iter()
                .collect(),
            },
        );
    }
    document
}

fn assert_tracker_overflow(
    host: &mut DesignerHost,
    presentation: &NativeTrackerExportPresentation,
    count: usize,
) {
    let before = host.observe_occurrence();
    let edit = serde_json::to_vec(&object(vec![
        ("type", "paste_cells".into()),
        ("expected_revision", "resident/0".into()),
        ("collection", "tracker".into()),
        ("start_entity", Value::Null),
        ("start_field", "task".into()),
        (
            "rows",
            Value::Array(vec![Value::Array(vec![
                "Extra".into(),
                "1".into(),
                "false".into(),
            ])]),
        ),
    ]))
    .unwrap();
    let refused: Value = serde_json::from_slice(&host.request(&edit)).unwrap();
    assert_eq!(refused["error"]["limit"], "table_rows", "{refused}");
    assert_eq!(refused["error"]["attempted"], count + 1);
    assert_eq!(host.observe_occurrence(), before);
    let mut excess = presentation.clone();
    excess.rows.push(excess.rows[0].clone());
    let request = serde_json::to_vec(&object(vec![
        ("type", "export_native_tracker".into()),
        ("expected_revision", "resident/0".into()),
        ("presentation", serde_json::to_value(&excess).unwrap()),
        ("format", "xlsx".into()),
    ]))
    .unwrap();
    let reply: Value = serde_json::from_slice(&host.spreadsheet(&request, &[]).reply).unwrap();
    assert_eq!(reply["error"]["limit"], "table_rows", "{reply}");
    assert_eq!(reply["error"]["attempted"], count + 1);
    assert_eq!(host.observe_occurrence(), before);
}

fn assert_tracker_exports(count: usize) {
    let mut document = tracker_document_with_rows(count);
    document
        .entities
        .get_mut(&EntityId::from("row-00000"))
        .unwrap()
        .fields
        .insert(
            FieldId::from("estimate"),
            StoredValue::Formula(tachiko_workspace_engine::Expression::Number(
                Number::new(1.0).unwrap(),
            )),
        );
    let presentation = NativeTrackerExportPresentation {
        version: 1,
        rows: document
            .entities
            .keys()
            .map(|id| NativeTrackerExportRow {
                entity_id: id.to_string(),
                styles: vec![crate::interop_adapter::CellStyle::default(); 3],
            })
            .collect(),
    };
    let runtime = crate::DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    let project = runtime.export_project_v3("resident/0").unwrap().bytes;
    let mut host = DesignerHost::new();
    let opened: Value =
        serde_json::from_slice(&host.open_project_working_size(&project, OCCURRENCE.as_bytes()))
            .unwrap();
    assert_eq!(opened["status"], "ok", "{opened}");
    let before = host.observe_occurrence();
    for kind in ["export_native_tracker", "export_native_tables"] {
        for format in ["xlsx", "csv"] {
            let mut request = object(vec![
                ("type", kind.into()),
                ("expected_revision", "resident/0".into()),
                ("format", format.into()),
            ]);
            if kind == "export_native_tracker" {
                request["presentation"] = serde_json::to_value(&presentation).unwrap();
            } else {
                request["collection"] = "tracker".into();
            }
            let request = serde_json::to_vec(&request).unwrap();
            let reply = host.spreadsheet(&request, &[]);
            let value: Value = serde_json::from_slice(&reply.reply).unwrap();
            assert_eq!(value["status"], "ok", "{kind} {format}: {value}");
            let bytes = reply.bytes.unwrap();
            let workbook = if format == "xlsx" {
                crate::interop_adapter::import_xlsx_profile(&bytes, &ImportOptions::default(), true)
                    .unwrap()
            } else {
                crate::interop_adapter::import_csv_profile(&bytes, &ImportOptions::default(), true)
                    .unwrap()
            };
            assert_eq!(workbook.sheets[0].rows.len(), count);
            if format == "xlsx" {
                assert!(
                    workbook.sheets[0]
                        .rows
                        .iter()
                        .flatten()
                        .any(|cell| cell.formula.is_some())
                );
            }
            assert_eq!(host.observe_occurrence(), before);
        }
    }
    if count == crate::capacity::WORKING_LIMITS.table_rows {
        assert_tracker_overflow(&mut host, &presentation, count);
    }
}

#[test]
fn working_size_native_tracker_and_tables_export_at_capacity() {
    for count in [129, crate::capacity::WORKING_LIMITS.table_rows] {
        assert_tracker_exports(count);
    }
    let Err(error) = crate::DesignerRuntime::from_document_working_size(
        tracker_document_with_rows(crate::capacity::WORKING_LIMITS.table_rows + 1),
        OCCURRENCE,
    ) else {
        panic!("over-capacity document was admitted");
    };
    assert!(matches!(
        error,
        DesignerError::CapacityExceeded(CapacityExceeded {
            limit: CapacityLimit::TableRows,
            ..
        })
    ));
}

#[test]
fn working_size_budget_exports_more_than_128_rows() {
    for added in [127, crate::capacity::WORKING_LIMITS.total_entities - 3] {
        let mut document = crate::DesignerRuntime::budget_document(OCCURRENCE).unwrap();
        let template = document.entities[&EntityId::from("rent")].clone();
        for index in 0..added {
            let id = EntityId::from(format!("budget-{index:05}"));
            let mut entity = template.clone();
            entity.id = id.clone();
            entity.key = EntityKey::from(id.as_str());
            entity.fields.insert(
                FieldId::from("variance"),
                StoredValue::Number(Number::new(0.0).unwrap()),
            );
            document.entities.insert(id, entity);
        }
        let collections = ["budget_items", "budget_summary"]
            .into_iter()
            .map(|collection| NativeBudgetExportCollection {
                collection_id: collection.into(),
                rows: document
                    .entities
                    .values()
                    .filter(|entity| entity.schema.as_str() == collection)
                    .map(|entity| NativeBudgetExportRow {
                        entity_id: entity.id.to_string(),
                        styles: vec![crate::interop_adapter::CellStyle::default(); 5],
                    })
                    .collect(),
            })
            .collect();
        let presentation = NativeBudgetExportPresentation {
            version: 1,
            active_view: "items".into(),
            views: vec![NativeBudgetExportView {
                id: "items".into(),
                name: "Items".into(),
                collection_id: "budget_items".into(),
            }],
            collections,
        };
        let runtime =
            crate::DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
        let project = runtime.export_project_v3("resident/0").unwrap().bytes;
        let mut host = DesignerHost::new();
        let opened: Value = serde_json::from_slice(
            &host.open_project_working_size(&project, OCCURRENCE.as_bytes()),
        )
        .unwrap();
        assert_eq!(opened["status"], "ok", "{opened}");
        let before = host.observe_occurrence();
        for format in ["xlsx", "csv"] {
            let request = serde_json::to_vec(&object(vec![
                ("type", "export_native_budget".into()),
                ("expected_revision", "resident/0".into()),
                ("presentation", serde_json::to_value(&presentation).unwrap()),
                ("format", format.into()),
            ]))
            .unwrap();
            let reply = host.spreadsheet(&request, &[]);
            let value: Value = serde_json::from_slice(&reply.reply).unwrap();
            assert_eq!(value["status"], "ok", "{format}: {value}");
            assert_ne!(reply.bytes.unwrap(), Vec::<u8>::new());
            assert_eq!(host.observe_occurrence(), before);
        }
        if added == crate::capacity::WORKING_LIMITS.total_entities - 3 {
            let mut excess = presentation.clone();
            let extra = excess.collections[0].rows[0].clone();
            excess.collections[0].rows.push(extra);
            let request = serde_json::to_vec(&object(vec![
                ("type", "export_native_budget".into()),
                ("expected_revision", "resident/0".into()),
                ("presentation", serde_json::to_value(excess).unwrap()),
                ("format", "xlsx".into()),
            ]))
            .unwrap();
            let reply: Value =
                serde_json::from_slice(&host.spreadsheet(&request, &[]).reply).unwrap();
            assert_eq!(reply["error"]["limit"], "total_entities", "{reply}");
            assert_eq!(reply["error"]["attempted"], 10_001);
            assert_eq!(host.observe_occurrence(), before);
        }
    }
}

// Real OOXML inputs: either Text cells selected as Number, or Boolean
// constant formulas whose parser findings include one discarded title cell.
fn ledger_xlsx(rows: usize, columns: usize, title: bool) -> Vec<u8> {
    use std::fmt::Write;

    let header = if title { 2 } else { 1 };
    let mut xml = String::from(
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    if title {
        xml.push_str("<row r=\"1\"><c r=\"A1\"><f>TRUE()</f><v>1</v></c></row>");
    }
    write!(xml, "<row r=\"{header}\">").unwrap();
    for col in 0..columns {
        write!(
            xml,
            "<c r=\"{}{header}\" t=\"inlineStr\"><is><t>C{col}</t></is></c>",
            column(col)
        )
        .unwrap();
    }
    xml.push_str("</row>");
    for row in header + 1..=header + rows {
        write!(xml, "<row r=\"{row}\">").unwrap();
        for col in 0..columns {
            let address = format!("{}{row}", column(col));
            if title {
                write!(xml, "<c r=\"{address}\"><f>TRUE()</f><v>1</v></c>").unwrap();
            } else {
                write!(
                    xml,
                    "<c r=\"{address}\" t=\"inlineStr\"><is><t>123</t></is></c>"
                )
                .unwrap();
            }
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    let template = source_xlsx(1, 1, 1, false, false);
    let mut input = ZipArchive::new(Cursor::new(template)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            bytes.clear();
            bytes.extend_from_slice(xml.as_bytes());
        }
        output.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut output, &bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn assert_ledger_summary(ledger: &Value, code: &str, total: usize) {
    let findings = ledger
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["code"] == code)
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 101);
    for finding in &findings[..100] {
        assert!(finding["location"].as_str().unwrap().starts_with("Sales1!"));
        assert_eq!(finding["blocking"], false);
    }
    assert_eq!(findings[100]["location"], "Sales1");
    assert_eq!(
        findings[100]["message"],
        format!("{total} findings of this kind on Sales1; the first 100 are listed")
    );
    assert_eq!(findings[100]["blocking"], false);
}

#[test]
fn working_size_hundred_thousand_explicit_conversions_fit_complete_reply() {
    let source = ledger_xlsx(10_000, 10, false);
    let mut host = DesignerHost::new();
    let imported = host.spreadsheet(&working_request("xlsx", 10), &source);
    assert!(imported.reply.len() <= 16 * 1024 * 1024);
    let response: Value = serde_json::from_slice(&imported.reply).unwrap();
    assert_eq!(response["status"], "ok", "{response}");
    let payload = &response["response"]["payload"];
    assert_eq!(payload["opened"]["window"]["total_rows"], 10_000);
    assert_ledger_summary(&payload["ledger"], "explicit_column_type", 100_000);
}

#[test]
fn working_size_skipped_title_findings_are_excluded_before_capping() {
    let source = ledger_xlsx(150, 1, true);
    let mut request: Value = serde_json::from_slice(&working_request("xlsx", 1)).unwrap();
    request["csv_options"]["header_rows"] = Value::Array(vec![2.into()]);
    request["selection"]["column_types"] = Value::Array(vec![Value::Array(vec!["boolean".into()])]);
    let mut host = DesignerHost::new();
    let inspect = serde_json::to_vec(&object(vec![
        ("type", "inspect".into()),
        ("format", "xlsx".into()),
        ("csv_options", request["csv_options"].clone()),
        ("profile", "working_size".into()),
    ]))
    .unwrap();
    let inspected = host.spreadsheet(&inspect, &source);
    let preview: Value = serde_json::from_slice(&inspected.reply).unwrap();
    assert_eq!(preview["status"], "ok", "{preview}");
    assert_ledger_summary(
        &preview["response"]["payload"]["ledger"],
        "boolean_constant_formula",
        150,
    );
    let imported = host.spreadsheet(&serde_json::to_vec(&request).unwrap(), &source);
    let response: Value = serde_json::from_slice(&imported.reply).unwrap();
    assert_eq!(response["status"], "ok", "{response}");
    let payload = &response["response"]["payload"];
    assert_eq!(payload["opened"]["window"]["total_rows"], 150);
    assert_ledger_summary(&payload["ledger"], "boolean_constant_formula", 150);
    let ledger = payload["ledger"].as_array().unwrap();
    assert!(
        ledger
            .iter()
            .any(|finding| finding["code"] == "rows_above_header_skipped")
    );
    assert!(
        !ledger
            .iter()
            .any(|finding| finding["location"] == "Sales1!A1")
    );
}

#[test]
fn working_size_sparse_xlsx_ten_thousand_rows() {
    let source = working_xlsx(10_000, 30, 40_000, false);
    assert_working_success(&source, "xlsx");
    // Add one physical data row to the same package.
    let mut input = ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            let xml = String::from_utf8(bytes).unwrap().replace(
                "</sheetData>",
                "<row r=\"10002\"><c r=\"A10002\"><v>10001</v></c></row></sheetData>",
            );
            bytes = xml.into_bytes();
        }
        output
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert_working_refusal(&output.finish().unwrap().into_inner(), "xlsx", "table_rows");
}

fn working_xlsx_with_layout(rows: usize, header_row: usize, summary: bool) -> Vec<u8> {
    use std::fmt::Write as _;

    let mut xml = String::from(
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    if header_row > 1 {
        xml.push_str("<row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Title</t></is></c></row>");
        for row in 2..header_row {
            write!(xml, "<row r=\"{row}\"/>").unwrap();
        }
    }
    write!(xml, "<row r=\"{header_row}\"><c r=\"A{header_row}\" t=\"inlineStr\"><is><t>Name</t></is></c><c r=\"B{header_row}\" t=\"inlineStr\"><is><t>Amount</t></is></c></row>").unwrap();
    for row in header_row + 1..=header_row + rows {
        write!(
            xml,
            "<row r=\"{row}\"><c r=\"A{row}\"><v>1</v></c><c r=\"B{row}\"><v>2</v></c></row>"
        )
        .unwrap();
    }
    if summary {
        let first = header_row + 1;
        let last = header_row + rows;
        let row = last + 1;
        write!(xml, "<row r=\"{row}\"><c r=\"A{row}\" t=\"inlineStr\"><is><t>Total</t></is></c><c r=\"B{row}\"><f>SUM(B{first}:B{last})</f><v>0</v></c></row>").unwrap();
    }
    xml.push_str("</sheetData></worksheet>");
    let mut input = ZipArchive::new(Cursor::new(working_xlsx(1, 2, 2, false))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        output.start_file(&name, options).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            output.write_all(xml.as_bytes()).unwrap();
        } else {
            output.write_all(&bytes).unwrap();
        }
    }
    output.finish().unwrap().into_inner()
}

fn assert_working_layout_import(source: &[u8], options: &ImportOptions, code: &str) {
    let workbook = crate::interop_adapter::import_xlsx_profile(source, options, true).unwrap();
    assert_eq!(workbook.sheets[0].rows.len(), 10_000);
    assert!(workbook.ledger.iter().any(|finding| finding.code == code));
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Number; 2]],
        extra_columns: vec![Vec::new()],
    };
    let (_, imported) =
        crate::import_workbook_working_size(&workbook, &selection, OCCURRENCE).unwrap();
    assert_eq!(imported.opened.window.total_rows, 10_000);
    assert!(imported.ledger.iter().any(|finding| finding.code == code));
}

#[test]
fn working_size_xlsx_title_and_blank_rows_do_not_count_as_data() {
    assert_working_layout_import(
        &working_xlsx_with_layout(10_000, 3, false),
        &ImportOptions::default(),
        "rows_above_header_skipped",
    );
}

#[test]
fn working_size_xlsx_summary_row_does_not_count_as_data() {
    assert_working_layout_import(
        &working_xlsx_with_layout(10_000, 1, true),
        &ImportOptions::default(),
        "summary_row_excluded",
    );
}

#[test]
fn working_size_xlsx_header_offset_refuses_exact_data_row_count() {
    let error = crate::interop_adapter::import_xlsx_profile(
        &working_xlsx_with_layout(10_001, 3, false),
        &ImportOptions::default(),
        true,
    )
    .unwrap_err();
    assert_eq!(
        error,
        InteropError::Capacity(CapacityExceeded {
            limit: CapacityLimit::TableRows,
            maximum: 10_000,
            attempted: 10_001,
            collection: Some("Sales1".into()),
        })
    );
}

#[test]
fn working_size_xlsx_explicit_header_row_twenty_has_full_capacity() {
    let options = ImportOptions {
        header_rows: vec![Some(20)],
        ..ImportOptions::default()
    };
    assert_working_layout_import(
        &working_xlsx_with_layout(10_000, 20, false),
        &options,
        "rows_above_header_skipped",
    );
}

#[test]
fn working_size_sparse_csv_ten_thousand_rows() {
    let mut source = (0..30)
        .map(|col| format!("C{col}"))
        .collect::<Vec<_>>()
        .join(",");
    source.push('\n');
    for row in 1..=10_000 {
        source.push_str(&row.to_string());
        source.push_str(&",".repeat(29));
        source.push('\n');
    }
    assert_working_success(source.as_bytes(), "csv");
    source.push_str("10001");
    source.push_str(&",".repeat(29));
    source.push('\n');
    assert_working_refusal(source.as_bytes(), "csv", "table_rows");
}

#[test]
fn working_size_geometry_and_formula_refusals_are_typed() {
    assert_working_refusal(&source_xlsx(33, 1, 1, false, false), "xlsx", "collections");
    assert_working_refusal(
        &source_xlsx(2, 1, 5_000, true, false),
        "xlsx",
        "total_entities",
    );
    assert_working_refusal(&working_xlsx(1, 33, 1, false), "xlsx", "table_fields");
    assert_working_refusal(
        &working_xlsx(9_091, 11, 100_001, false),
        "xlsx",
        "populated_cells",
    );
    assert_working_refusal(&working_xlsx(9_091, 11, 100_001, true), "xlsx", "formulas");
}

#[test]
fn working_size_source_bytes_refusal_is_typed() {
    let source = vec![b'x'; 16 * 1024 * 1024 + 1];
    assert_working_refusal(&source, "xlsx", "source_bytes");
    assert_working_refusal(&source, "csv", "source_bytes");
}

fn package_with_extra(name: &str, bytes: &[u8]) -> Vec<u8> {
    let template = source_xlsx(1, 1, 1, false, false);
    let mut input = ZipArchive::new(Cursor::new(template)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        output.start_file(name, options).unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.start_file(name, options).unwrap();
    output.write_all(bytes).unwrap();
    output.finish().unwrap().into_inner()
}

#[test]
fn working_size_expanded_bytes_and_xml_nodes_refusals_are_typed() {
    let expanded = vec![b' '; 64 * 1024 * 1024 + 1];
    assert_working_refusal(
        &package_with_extra("xl/extra.bin", &expanded),
        "xlsx",
        "expanded_bytes",
    );
    drop(expanded);
    let mut xml = String::from("<root>");
    for _ in 0..1_500_000 {
        xml.push_str("<x/>");
    }
    xml.push_str("</root>");
    assert_working_refusal(
        &package_with_extra("xl/extra.xml", xml.as_bytes()),
        "xlsx",
        "xml_nodes",
    );
}

#[test]
fn working_size_serialized_source_refusal_is_typed() {
    let template = source_xlsx(1, 1, 1, false, false);
    let mut input = ZipArchive::new(Cursor::new(template)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            let mut xml = String::from(
                "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Name</t></is></c></row><row r=\"2\"><c r=\"A2\" t=\"inlineStr\"><is><t>",
            );
            xml.push_str(&"x".repeat(48 * 1024 * 1024 + 1));
            xml.push_str("</t></is></c></row></sheetData></worksheet>");
            bytes = xml.into_bytes();
        }
        output.start_file(name, options).unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert_working_refusal(
        &output.finish().unwrap().into_inner(),
        "xlsx",
        "serialized_source_bytes",
    );
}

// Keep the sparse 30-column shape used by assert_working_success. Cached
// totals are correct, so its existing export proof remains applicable.
fn column_totals_xlsx(rows: usize, formula_column: bool, mixed: bool) -> Vec<u8> {
    use std::fmt::Write;

    let last = rows + 1;
    let total = rows * (rows + 1) / 2;
    let mut xml = String::from(
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\">",
    );
    for col in 0..30 {
        write!(
            xml,
            "<c r=\"{}1\" t=\"inlineStr\"><is><t>C{col}</t></is></c>",
            column(col)
        )
        .unwrap();
    }
    xml.push_str("</row>");
    for row in 2..=last {
        let value = row - 1;
        write!(xml, "<row r=\"{row}\">").unwrap();
        if mixed && row == 2 {
            write!(
                xml,
                "<c r=\"A{row}\" t=\"inlineStr\"><is><t>mixed</t></is></c>"
            )
            .unwrap();
        } else {
            write!(xml, "<c r=\"A{row}\"><v>{value}</v></c>").unwrap();
        }
        if formula_column {
            write!(xml, "<c r=\"B{row}\"><v>2</v></c><c r=\"C{row}\"><f>A{row}*2</f><v>{}</v></c><c r=\"D{row}\"><f>SUM($C$2:$C${last})</f><v>{}</v></c>", value * 2, total * 2).unwrap();
        } else {
            let cached = if mixed { 123 } else { total };
            write!(xml, "<c r=\"B{row}\"><f>SUM($A$2:$A${last})</f><v>{cached}</v></c><c r=\"C{row}\"><v>3</v></c><c r=\"D{row}\"><f>2*3</f><v>6</v></c>").unwrap();
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    let template = source_xlsx(1, 1, 1, false, false);
    let mut input = ZipArchive::new(Cursor::new(template)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            bytes.clear();
            bytes.extend_from_slice(xml.as_bytes());
        }
        output.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut output, &bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn assert_imported_column_total(
    source: &[u8],
    request: &[u8],
    column: usize,
    expected: f64,
    formula: bool,
) -> Value {
    let mut host = DesignerHost::new();
    let imported = host.spreadsheet(request, source);
    let response: Value = serde_json::from_slice(&imported.reply).unwrap();
    assert_eq!(response["status"], "ok", "{response}");
    let payload = &response["response"]["payload"];
    let window = &payload["opened"]["window"];
    let count = window["total_rows"].as_u64().unwrap();
    for start in [0, count - 1] {
        let query = serde_json::to_vec(&object(vec![
            ("type", "query_table_window".into()),
            ("expected_revision", window["revision"].clone()),
            ("collection", window["collection"]["id"].clone()),
            ("start", start.into()),
            ("count", 1.into()),
        ]))
        .unwrap();
        let reply: Value = serde_json::from_slice(&host.request(&query)).unwrap();
        assert_eq!(reply["status"], "ok", "{reply}");
        let field = &reply["response"]["payload"]["rows"][0]["fields"][column];
        assert_eq!(!field["formula"].is_null(), formula, "{field}");
        let value = if formula {
            &field["calculated"]["value"]
        } else {
            &field["stored"]["value"]
        };
        assert_eq!(value.as_f64().unwrap(), expected, "{field}");
    }
    payload.clone()
}

#[test]
fn working_size_repeated_whole_column_totals_import_ten_thousand_rows() {
    let source = column_totals_xlsx(10_000, false, false);
    assert_working_success(&source, "xlsx");
    let payload =
        assert_imported_column_total(&source, &working_request("xlsx", 30), 1, 50_005_000.0, true);
    assert_eq!(payload["opened"]["window"]["total_rows"], 10_000);
}

#[test]
fn working_size_totals_over_formula_column_bind_every_row_first() {
    let source = column_totals_xlsx(10_000, true, false);
    let payload = assert_imported_column_total(
        &source,
        &working_request("xlsx", 30),
        3,
        100_010_000.0,
        true,
    );
    assert_eq!(payload["opened"]["window"]["total_rows"], 10_000);
}

#[test]
fn working_size_mixed_column_sum_keeps_cached_value_with_existing_finding() {
    let source = column_totals_xlsx(10, false, true);
    let workbook =
        crate::interop_adapter::import_xlsx_profile(&source, &ImportOptions::default(), true)
            .unwrap();
    assert!(
        workbook.sheets[0]
            .rows
            .iter()
            .all(|row| row[1].formula.is_none())
    );
    let findings = workbook
        .ledger
        .iter()
        .filter(|finding| finding.code == "unsupported_formula_kept_value")
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 10);
    assert!(findings.iter().all(|finding| !finding.blocking));
    let mut request: Value = serde_json::from_slice(&working_request("xlsx", 30)).unwrap();
    request["selection"]["column_types"][0][0] = "text".into();
    let payload = assert_imported_column_total(
        &source,
        &serde_json::to_vec(&request).unwrap(),
        1,
        123.0,
        false,
    );
    assert!(
        payload["ledger"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "unsupported_formula_kept_value")
    );
}
