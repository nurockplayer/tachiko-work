//! The same real XLSX/CSV bytes must refuse atomically through both arenas.
use std::io::{Cursor, Read, Write};

use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_spreadsheet_run,
};
use crate::host_bridge::DesignerHost;
use crate::interop_adapter::{
    ImportOptions, InteropError, SourceWorkbook, export_xlsx, import_csv, import_xlsx,
};
use crate::{
    CapacityExceeded, CapacityLimit, DesignerError, ImportFieldType, ImportSelection,
    import_workbook,
};
use serde_json::Value;
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

fn working_request(format: &str, columns: usize) -> Vec<u8> {
    let kind = if format == "csv" { "text" } else { "number" };
    serde_json::to_vec(&serde_json::json!({
        "type": "import", "format": format,
        "csv_options": {"delimiter": ",", "header": true},
        "selection": {"column_types": [vec![kind; columns]], "extra_columns": [[]]},
        "occurrence_id": OCCURRENCE, "install": true, "profile": "working_size"
    }))
    .unwrap()
}

fn assert_working_success(source: &[u8], format: &str) {
    let mut host = DesignerHost::new();
    let inspect = serde_json::to_vec(&serde_json::json!({
        "type": "inspect", "format": format,
        "csv_options": {"delimiter": ",", "header": true}, "profile": "working_size"
    }))
    .unwrap();
    let inspected = host.spreadsheet(&inspect, source);
    let inspected: Value = serde_json::from_slice(&inspected.reply).unwrap();
    assert_eq!(inspected["status"], "ok", "{inspected}");
    drop(inspected);
    let request = working_request(format, 30);
    let response: Value =
        serde_json::from_slice(&host.spreadsheet(&request, source).reply).unwrap();
    assert_eq!(response["status"], "ok", "{response}");
    let payload = &response["response"]["payload"];
    let window = &payload["opened"]["window"];
    assert_eq!(window["total_rows"], 10_000);
    let revision = window["revision"].as_str().unwrap();
    let collection = window["collection"]["id"].as_str().unwrap();
    let control = serde_json::to_vec(&serde_json::json!({
        "type": "export", "format": "xlsx", "expected_revision": revision,
        "collection": collection, "metadata": payload["metadata"]
    }))
    .unwrap();
    assert!(control.len() < 4 * 1024 * 1024);
    let mut last_id = String::new();
    let mut start = 0;
    while start < 10_000 {
        let query = serde_json::to_vec(&serde_json::json!({"type":"query_table_window", "expected_revision":revision, "collection":collection, "start":start, "count":256})).unwrap();
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
