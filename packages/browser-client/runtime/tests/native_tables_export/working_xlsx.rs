use std::io::Write;

use super::{
    Cursor, DesignerHost, HostReply, Json, NEXT, OCCURRENCE, export_xlsx, import_xlsx, json,
};
use tachiko_designer_runtime::interop_adapter::{
    CellStyle, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue, SourceWorkbook,
};
use zip::{ZipWriter, write::SimpleFileOptions};

const DATE_FORMAT: &str = "[$-411]yyyy-mm-dd";

fn xlsx() -> Vec<u8> {
    export_xlsx(&SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Dates".into(),
            has_header: true,
            columns: ["Date", "Note"]
                .map(|name| SourceColumn {
                    name: name.into(),
                    width: None,
                    number_format: None,
                })
                .into(),
            rows: vec![
                vec![
                    SourceCell {
                        value: SourceValue::Date {
                            value: "2024-02-29".into(),
                        },
                        style: CellStyle {
                            number_format: Some(DATE_FORMAT.into()),
                            ..CellStyle::default()
                        },
                        ..SourceCell::default()
                    },
                    SourceCell {
                        value: SourceValue::Text {
                            value: "before".into(),
                        },
                        ..SourceCell::default()
                    },
                ];
                32
            ],
            layout: SourceLayout::default(),
        }],
        ledger: vec![],
    })
    .unwrap()
}

fn send(host: &mut DesignerHost, request: &Json, bytes: &[u8]) -> HostReply {
    host.spreadsheet(&serde_json::to_vec(request).unwrap(), bytes)
}

fn ok(reply: &[u8]) -> Json {
    let reply: Json = serde_json::from_slice(reply).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"].clone()
}

fn import(host: &mut DesignerHost, bytes: &[u8]) -> Json {
    ok(&send(
        host,
        &json!({
            "type": "import", "format": "xlsx", "install": true, "occurrence_id": OCCURRENCE,
            "csv_options": {"delimiter": ",", "header": true, "header_rows": [1]},
            "selection": {"column_types": [["date", "text"]], "extra_columns": [[]]},
        }),
        bytes,
    )
    .reply)
}

fn reopen(host: &mut DesignerHost) {
    let expected = ok(&host.observe_occurrence())["payload"].clone();
    let reply = send(
        host,
        &json!({
            "type": "reopen_imported_xlsx_working_size", "expected": expected, "occurrence_id": NEXT,
        }),
        &[],
    );
    assert_eq!(ok(&reply.reply)["type"], "opened_window");
    assert_ne!(
        ok(&host.observe_occurrence())["payload"]["scope"],
        expected["scope"]
    );
}

fn overwrite(host: &mut DesignerHost) -> HostReply {
    let expected = ok(&host.observe_occurrence())["payload"].clone();
    send(
        host,
        &json!({"type": "export_xlsx_overwrite", "expected": expected}),
        &[],
    )
}

#[test]
fn working_size_xlsx_overwrite_keeps_locale_date_after_70kb_edit_batch() {
    let mut host = DesignerHost::new();
    let imported = import(&mut host, &xlsx());
    let sheet = &imported["payload"]["metadata"]["sheets"][0];
    let rows = sheet["rows"].as_array().unwrap();
    let edits: Vec<_> = rows
        .iter()
        .map(|row| {
            json!({
                "target": {"entity": row["entity_id"], "field": sheet["columns"][1]["field_id"]},
                "input": {"kind": "text", "value": ""},
            })
        })
        .collect();
    let mut request =
        json!({"type": "edit_cells", "expected_revision": "resident/0", "edits": edits});
    let padding = 70_000 - serde_json::to_vec(&request).unwrap().len();
    for (index, edit) in request["edits"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let text_bytes = padding / rows.len() + usize::from(index < padding % rows.len());
        edit["input"]["value"] = json!("x".repeat(text_bytes));
    }
    let bytes = serde_json::to_vec(&request).unwrap();
    assert_eq!(bytes.len(), 70_000);

    reopen(&mut host);
    let published = ok(&host.request(&bytes));
    assert_eq!(published["payload"]["resulting_revision"], "resident/1");
    let exported = overwrite(&mut host);
    ok(&exported.reply);
    let workbook = import_xlsx(&exported.bytes.unwrap()).unwrap();
    let exported_rows = &workbook.sheets[0].rows;
    assert_eq!(
        exported_rows[0][0].style.number_format.as_deref(),
        Some(DATE_FORMAT)
    );
    assert_eq!(
        exported_rows[0][0].value,
        SourceValue::Date {
            value: "2024-02-29".into()
        }
    );
    assert_eq!(exported_rows.len(), rows.len());
    for (index, row) in exported_rows.iter().enumerate() {
        assert_eq!(
            row[1].value,
            SourceValue::Text {
                value: request["edits"][index]["input"]["value"]
                    .as_str()
                    .unwrap()
                    .into(),
            }
        );
    }
}

#[test]
fn working_size_xlsx_overwrite_keeps_typed_refusal_for_omitted_part() {
    let path = "docProps/custom.xml";
    let mut archive = ZipWriter::new_append(Cursor::new(xlsx())).unwrap();
    archive
        .start_file(path, SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"<Properties/>").unwrap();
    let bytes = archive.finish().unwrap().into_inner();

    let mut host = DesignerHost::new();
    import(&mut host, &bytes);
    reopen(&mut host);
    let refused = overwrite(&mut host);
    assert!(refused.bytes.is_none());
    let reply: Json = serde_json::from_slice(&refused.reply).unwrap();
    assert_eq!(reply["status"], "error", "{reply}");
    assert_eq!(reply["error"]["code"], "xlsx_overwrite_refused", "{reply}");
    assert!(
        reply["error"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "xlsx_source_part_omitted" && diagnostic["path"] == path
            }),
        "{reply}"
    );
}
