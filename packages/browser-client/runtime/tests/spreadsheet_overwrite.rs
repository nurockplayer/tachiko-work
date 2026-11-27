use serde_json::{Value, json};
use std::io::{Cursor, Read, Write};
use tachiko_designer_runtime::host_bridge::{DesignerHost, HostReply};
use tachiko_designer_runtime::interop_adapter::{
    SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue, SourceWorkbook, export_xlsx,
    import_xlsx,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const SHEET: &str = "xl/worksheets/sheet1.xml";
const FIRST: &str = "00000000-0000-4000-8000-000000000978";
const SECOND: &str = "00000000-0000-4000-8000-000000000979";

fn source() -> Vec<u8> {
    export_xlsx(&SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Inventory".into(),
            has_header: true,
            columns: vec![SourceColumn {
                name: "Item".into(),
                width: None,
                number_format: None,
            }],
            rows: vec![vec![SourceCell {
                value: SourceValue::Text {
                    value: "before".into(),
                },
                ..SourceCell::default()
            }]],
            layout: SourceLayout::default(),
        }],
        ledger: vec![],
    })
    .unwrap()
}

fn repack(edit_sheet: impl FnOnce(String) -> String, extra: &[&str]) -> Vec<u8> {
    let original = source();
    let mut input = ZipArchive::new(Cursor::new(&original)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    let mut edit_sheet = Some(edit_sheet);
    for index in (0..input.len()).rev() {
        let mut entry = input.by_index(index).unwrap();
        let path = entry.name().to_owned();
        let mut contents = String::new();
        entry.read_to_string(&mut contents).unwrap();
        if path == SHEET {
            contents = edit_sheet.take().unwrap()(contents);
        }
        output.start_file(path, options).unwrap();
        output.write_all(contents.as_bytes()).unwrap();
    }
    for path in extra {
        output.start_file(*path, options).unwrap();
        output.write_all(b"<Properties/>").unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn send(host: &mut DesignerHost, request: &Value, bytes: &[u8]) -> HostReply {
    host.spreadsheet(&serde_json::to_vec(request).unwrap(), bytes)
}

fn ok(reply: &[u8]) -> Value {
    let reply: Value = serde_json::from_slice(reply).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"].clone()
}

fn open(host: &mut DesignerHost, bytes: &[u8], occurrence: &str, header: u32) -> Value {
    let receipt = send(
        host,
        &json!({
            "type": "import", "format": "xlsx", "install": true, "occurrence_id": occurrence,
            "csv_options": {"delimiter": ",", "header": true, "header_rows": [header]},
            "selection": {"column_types": [["text"]], "extra_columns": [[]]},
        }),
        bytes,
    );
    ok(&receipt.reply)
}

fn overwrite(host: &mut DesignerHost, expected: &Value) -> HostReply {
    send(
        host,
        &json!({"type": "export_xlsx_overwrite", "expected": expected}),
        &[],
    )
}

fn refusal(receipt: &HostReply, code: &str) -> Value {
    assert!(receipt.bytes.is_none());
    let reply: Value = serde_json::from_slice(&receipt.reply).unwrap();
    assert_eq!(reply["status"], "error", "{reply}");
    assert_eq!(reply["error"]["code"], code, "{reply}");
    reply["error"]["diagnostics"].clone()
}

fn has_reason(diagnostics: &Value, code: &str, path: &str) {
    assert!(
        diagnostics
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["code"] == code && entry["path"] == path),
        "{diagnostics}"
    );
}

#[test]
fn overwrite_exports_current_edit_and_leaves_one_undo() {
    let original = repack(
        |xml| {
            let attributes = r#"r="A2" s="0" t="inlineStr""#;
            assert!(xml.contains(attributes));
            let preserved = r#"<t xml:space="preserve">before</t>"#;
            assert!(xml.contains(preserved));
            let xml = xml
                .replace(attributes, r#"t="inlineStr" r="A2" s="0""#)
                .replace(preserved, "<t>before</t>")
                .replace(" xmlns=", " xmlns:m=")
                .replace('<', "<m:")
                .replace("<m:/", "</m:")
                .replace("><", ">\n  <");
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{xml}\n")
        },
        &[],
    );
    let mut host = DesignerHost::new();
    let imported = open(&mut host, &original, FIRST, 1);
    let occurrence = ok(&host.observe_occurrence());
    let ids = &imported["metadata"]["sheets"][0];
    let edited = host.request(&serde_json::to_vec(&json!({
        "type": "edit_cells", "expected_revision": occurrence["revision"],
        "edits": [{
            "target": {"entity": ids["rows"][0]["entity_id"], "field": ids["columns"][0]["field_id"]},
            "input": {"kind": "text", "value": "after"}
        }]
    })).unwrap());
    let revision = ok(&edited)["resulting_revision"].clone();
    assert_eq!(revision, "resident/1");
    let expected = json!({"scope": occurrence["scope"], "revision": revision});
    let exported = overwrite(&mut host, &expected);
    let receipt = ok(&exported.reply);
    let bytes = exported.bytes.unwrap();
    assert_eq!(receipt["revision"], revision);
    assert_eq!(receipt["byte_length"], bytes.len());
    let reopened = import_xlsx(&bytes).unwrap();
    assert_eq!(
        reopened.sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "after".into()
        }
    );
    assert_eq!(ok(&host.observe_occurrence()), expected);

    let undo = host.request(br#"{"type":"undo","expected_revision":"resident/1"}"#);
    assert_eq!(ok(&undo)["resulting_revision"], "resident/2");
    let expected = ok(&host.observe_occurrence());
    let exported = overwrite(&mut host, &expected);
    assert_eq!(
        import_xlsx(&exported.bytes.unwrap()).unwrap(),
        import_xlsx(&original).unwrap()
    );
}

#[test]
fn overwrite_refuses_a_dropped_cell_attribute() {
    let original = repack(
        |xml| {
            assert!(xml.contains(r#"<c r="A2" "#));
            xml.replace(r#"<c r="A2" "#, r#"<c r="A2" ph="1" "#)
        },
        &[],
    );
    let mut host = DesignerHost::new();
    open(&mut host, &original, FIRST, 1);
    let expected = ok(&host.observe_occurrence());
    let diagnostics = refusal(&overwrite(&mut host, &expected), "xlsx_overwrite_refused");
    has_reason(&diagnostics, "xlsx_source_part_changed", SHEET);
}

#[test]
fn overwrite_refuses_changed_space_handling_for_padded_text() {
    let original = repack(
        |xml| {
            let preserved = r#"<t xml:space="preserve">before</t>"#;
            assert!(xml.contains(preserved));
            xml.replace(preserved, "<t> before </t>")
        },
        &[],
    );
    let mut host = DesignerHost::new();
    open(&mut host, &original, FIRST, 1);
    let expected = ok(&host.observe_occurrence());
    let diagnostics = refusal(&overwrite(&mut host, &expected), "xlsx_overwrite_refused");
    has_reason(&diagnostics, "xlsx_source_part_changed", SHEET);
}

#[test]
fn overwrite_reports_rich_text_flattening_absent_from_the_import_ledger() {
    let original = repack(
        |xml| {
            xml.replace(
                "<t xml:space=\"preserve\">before</t>",
                "<r><rPr><b/></rPr><t>be</t></r><r><t>fore</t></r>",
            )
        },
        &[],
    );
    let plain = open(&mut DesignerHost::new(), &source(), FIRST, 1);
    let mut host = DesignerHost::new();
    let imported = open(&mut host, &original, FIRST, 1);
    assert_eq!(imported["ledger"], plain["ledger"]);
    let expected = ok(&host.observe_occurrence());
    let diagnostics = refusal(&overwrite(&mut host, &expected), "xlsx_overwrite_refused");
    has_reason(&diagnostics, "xlsx_rich_text_flattened", SHEET);
    let copy = send(
        &mut host,
        &json!({
            "type": "export", "expected_revision": expected["revision"], "metadata": imported["metadata"],
            "format": "xlsx", "collection": "sheet_1"
        }),
        &[],
    );
    assert!(copy.bytes.is_some());
}

#[test]
fn overwrite_reports_rows_removed_above_the_chosen_header() {
    let original = repack(
        |xml| {
            xml.replace("r=\"2\"", "r=\"3\"").replace("r=\"A2\"", "r=\"A3\"")
        .replace("r=\"1\"", "r=\"2\"").replace("r=\"A1\"", "r=\"A2\"")
        .replace("<sheetData>", "<sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Title</t></is></c></row>")
        },
        &[],
    );
    let mut host = DesignerHost::new();
    open(&mut host, &original, FIRST, 2);
    let expected = ok(&host.observe_occurrence());
    let diagnostics = refusal(&overwrite(&mut host, &expected), "xlsx_overwrite_refused");
    has_reason(&diagnostics, "rows_above_header_skipped", "Inventory!1:1");
}

#[test]
fn overwrite_reports_each_omitted_package_part() {
    let extra = ["docProps/core.xml", "xl/theme/theme1.xml"];
    let original = repack(std::convert::identity, &extra);
    let mut host = DesignerHost::new();
    open(&mut host, &original, FIRST, 1);
    let expected = ok(&host.observe_occurrence());
    let diagnostics = refusal(&overwrite(&mut host, &expected), "xlsx_overwrite_refused");
    for path in extra {
        has_reason(&diagnostics, "xlsx_source_part_omitted", path);
    }
}

#[test]
fn overwrite_refuses_a_stale_revision_or_replaced_occurrence_with_the_same_revision() {
    let original = source();
    let mut host = DesignerHost::new();
    open(&mut host, &original, FIRST, 1);
    let expected = ok(&host.observe_occurrence());
    refusal(
        &overwrite(
            &mut host,
            &json!({"scope": expected["scope"], "revision": "resident/9"}),
        ),
        "stale_revision",
    );
    open(&mut host, &original, SECOND, 1);
    assert_eq!(
        ok(&host.observe_occurrence())["revision"],
        expected["revision"]
    );
    let diagnostics = refusal(&overwrite(&mut host, &expected), "xlsx_overwrite_refused");
    has_reason(&diagnostics, "xlsx_source_occurrence_changed", "workbook");
}
