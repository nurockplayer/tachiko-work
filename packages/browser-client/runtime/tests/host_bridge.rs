//! The native host bridge must answer exactly like the private wire path and
//! preserve the resident occurrence on every refused or failed operation.
//! The bridge is native-only, so this test is too.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tachiko_designer_runtime::{
    DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply, ImportFieldType,
    ImportSelection,
    host_bridge::{DesignerHost, MAX_HOST_PROJECT_TRANSFER_BYTES},
    interop_adapter::{ImportOptions, SourceValue, import_csv},
    open_local_document, process_wire_request,
};
use tachiko_storage::{encode_portable_package_v1, encode_roproj_v3, to_canonical_string};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Schema, SchemaId, SchemaKey, Value as CellValue,
};

const GAME_BALANCE_RO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../examples/game-balance/game-balance.ro"
));
const FIRST: &str = "00000000-0000-4000-8000-000000000501";
const SECOND: &str = "00000000-0000-4000-8000-000000000502";
const THIRD: &str = "00000000-0000-4000-8000-000000000503";

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("every bridge reply is JSON")
}

fn request(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn revision_of(reply: &Value) -> String {
    reply["response"]["payload"]["bootstrap"]["revision"]
        .as_str()
        .or_else(|| reply["response"]["payload"]["revision"].as_str())
        .or_else(|| reply["response"]["payload"]["resulting_revision"].as_str())
        .expect("reply carries a revision")
        .to_owned()
}

#[test]
fn ordinary_requests_match_the_wire_path_byte_for_byte() {
    let mut host = DesignerHost::new();
    let mut wire: Option<DesignerRuntime> = None;

    let new_tracker = request(&json!({ "type": "new_tracker", "occurrence_id": THIRD }));
    assert_eq!(
        host.request(&new_tracker),
        process_wire_request(&mut wire, &new_tracker)
    );

    let opened = host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes());
    let wire_opened = open_local_document(&mut wire, GAME_BALANCE_RO, FIRST).unwrap();
    assert_eq!(
        opened,
        serde_json::to_vec(&DesignerWireReply::Ok {
            response: DesignerResponse::Opened(Box::new(wire_opened)),
        })
        .unwrap()
    );
    let opened = json_of(&opened);
    assert_eq!(opened["status"], "ok");

    let collection = opened["response"]["payload"]["table"]["collection"]["id"]
        .as_str()
        .expect("opened document names its table")
        .to_owned();
    let query = request(&json!({ "type": "query_table", "collection": collection }));
    let table = host.request(&query);
    assert_eq!(table, process_wire_request(&mut wire, &query));
    let table = json_of(&table);
    assert_eq!(table["status"], "ok", "{table}");

    let mut edits = 0;
    for row in table["response"]["payload"]["rows"].as_array().unwrap() {
        for cell in row["fields"].as_array().unwrap() {
            let input = match cell["editable_scalar"].as_str() {
                Some("text") => json!({ "kind": "text", "value": "native bridge" }),
                Some("number") => json!({ "kind": "number", "input": "42" }),
                _ => continue,
            };
            let edit = request(&json!({
                "type": "edit_scalar",
                "expected_revision": revision_of(&opened),
                "target": cell["target"],
                "input": input,
            }));
            let published = host.request(&edit);
            assert_eq!(published, process_wire_request(&mut wire, &edit));
            assert_eq!(json_of(&published)["status"], "ok");

            // The same stale request is now refused identically and publishes nothing.
            let stale = host.request(&edit);
            assert_eq!(stale, process_wire_request(&mut wire, &edit));
            assert_eq!(json_of(&stale)["status"], "error");
            edits += 1;
            break;
        }
        if edits > 0 {
            break;
        }
    }
    assert_eq!(edits, 1, "the document exposes an editable scalar cell");

    let oversized = vec![b' '; 70_000];
    assert_eq!(
        host.request(&oversized),
        process_wire_request(&mut wire, &oversized)
    );
}

#[test]
fn export_close_and_reopen_round_trip_through_a_fresh_occurrence() {
    let mut host = DesignerHost::new();
    assert_eq!(
        json_of(&host.observe_occurrence())["error"]["code"],
        "no_project_open"
    );

    let opened = json_of(&host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()));
    assert_eq!(opened["status"], "ok", "{opened}");
    let revision = revision_of(&opened);

    let exported = host.export_project(revision.as_bytes());
    let receipt = json_of(&exported.reply);
    assert_eq!(receipt["response"]["type"], "project_exported");
    let bytes = exported.bytes.expect("a successful export carries bytes");
    assert_eq!(
        receipt["response"]["payload"]["byte_length"],
        json!(bytes.len())
    );

    let stale = host.export_project(b"resident/999");
    assert!(stale.bytes.is_none());
    assert_eq!(json_of(&stale.reply)["status"], "error");

    host.close_project();
    assert_eq!(
        json_of(&host.observe_occurrence())["error"]["code"],
        "no_project_open"
    );

    let reopened = json_of(&host.open_project(&bytes, SECOND.as_bytes()));
    assert_eq!(reopened["status"], "ok", "{reopened}");
    assert_eq!(
        reopened["response"]["payload"]["bootstrap"]["title"],
        opened["response"]["payload"]["bootstrap"]["title"]
    );
    let observed = json_of(&host.observe_occurrence());
    let scope = observed["response"]["payload"]["scope"]
        .as_str()
        .or_else(|| observed["response"]["payload"]["occurrence_scope"].as_str())
        .unwrap_or_default()
        .to_owned();
    assert!(
        observed.to_string().contains(SECOND),
        "the reopened work is a fresh occurrence: {observed} {scope}"
    );
}

#[test]
fn refused_inputs_preserve_the_resident_occurrence() {
    let mut host = DesignerHost::new();
    let opened = json_of(&host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()));
    let revision = revision_of(&opened);
    let before = host.export_project(revision.as_bytes()).bytes.unwrap();

    let invalid = json_of(&host.open_project(b"not a Tachiko project", SECOND.as_bytes()));
    assert_eq!(invalid["status"], "error");
    let invalid_utf8 = json_of(&host.open_project(&before, &[0xff, 0xfe]));
    assert_eq!(invalid_utf8["status"], "error");
    let oversized = vec![0_u8; MAX_HOST_PROJECT_TRANSFER_BYTES + 1];
    let too_large = json_of(&host.open_project(&oversized, SECOND.as_bytes()));
    assert_eq!(too_large["error"]["code"], "project_too_large");
    let too_large_sheet = host.spreadsheet(
        &request(&json!({ "type": "inspect", "format": "csv", "csv_options": { "delimiter": ",", "header": true } })),
        &oversized,
    );
    assert!(too_large_sheet.bytes.is_none());
    assert_eq!(json_of(&too_large_sheet.reply)["status"], "error");
    let malformed = host.spreadsheet(b"{\"type\":\"nope\"}", b"a,b\n1,2\n");
    assert_eq!(json_of(&malformed.reply)["status"], "error");

    assert!(
        json_of(&host.observe_occurrence())
            .to_string()
            .contains(FIRST)
    );
    assert_eq!(
        host.export_project(revision.as_bytes()).bytes.unwrap(),
        before
    );
}

#[test]
fn spreadsheet_import_installs_only_after_admission_and_exports_csv() {
    let mut host = DesignerHost::new();
    let source = b"Name,Note\nAlpha,first\nBeta,second\n";
    let options = json!({ "delimiter": ",", "header": true });

    let preview = host.spreadsheet(
        &request(&json!({ "type": "inspect", "format": "csv", "csv_options": options })),
        source,
    );
    assert_eq!(json_of(&preview.reply)["status"], "ok");
    assert_eq!(
        json_of(&host.observe_occurrence())["error"]["code"],
        "no_project_open"
    );

    let selection = serde_json::to_value(ImportSelection {
        column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Text]],
        extra_columns: vec![Vec::new()],
    })
    .unwrap();
    let import = |install: bool| {
        request(&json!({
            "type": "import",
            "format": "csv",
            "csv_options": options,
            "selection": selection,
            "occurrence_id": THIRD,
            "install": install,
        }))
    };
    let candidate = json_of(&host.spreadsheet(&import(false), source).reply);
    assert_eq!(candidate["status"], "ok", "{candidate}");
    assert_eq!(
        json_of(&host.observe_occurrence())["error"]["code"],
        "no_project_open",
        "an uninstalled candidate does not become resident"
    );

    let imported = json_of(&host.spreadsheet(&import(true), source).reply);
    assert_eq!(imported["status"], "ok");
    let payload = &imported["response"]["payload"];
    let metadata = payload["metadata"].clone();
    let collection = metadata["sheets"][0]["schema_id"].clone();
    let revision = payload["opened"]["bootstrap"]["revision"].clone();

    let export = host.spreadsheet(
        &request(&json!({
            "type": "export",
            "expected_revision": revision,
            "metadata": metadata,
            "format": "csv",
            "collection": collection,
        })),
        &[],
    );
    assert_eq!(
        json_of(&export.reply)["response"]["type"],
        "spreadsheet_exported"
    );
    let csv = String::from_utf8(export.bytes.expect("export bytes")).unwrap();
    assert!(csv.contains("Alpha") && csv.contains("second"), "{csv}");
}

fn headered_csv(headers: &[&str], values: &[&str], rows: usize) -> Vec<u8> {
    let mut lines = vec![headers.join(",")];
    lines.extend(std::iter::repeat_n(values.join(","), rows));
    lines.join("\r\n").into_bytes()
}

fn inspect_request() -> Vec<u8> {
    request(&json!({
        "type": "inspect",
        "format": "csv",
        "csv_options": { "delimiter": ",", "header": true },
    }))
}

#[test]
fn safe_64_by_2_inspection_keeps_the_ordinary_budget_and_resident() {
    let source = headered_csv(&["a", "b"], &[&"x".repeat(800), &"y".repeat(800)], 64);
    assert_eq!(source.len(), 102_595);
    let workbook = import_csv(&source, &ImportOptions::default()).unwrap();
    let preview = serde_json::to_vec(&DesignerWireReply::Ok {
        response: DesignerResponse::ImportPreview(Box::new(workbook)),
    })
    .unwrap();
    assert_eq!(preview.len(), 122_730);

    // WASM uses the ordinary complete-wire budget for this valid small rectangle.
    for resident in [false, true] {
        let mut host = DesignerHost::new();
        let revision = if resident {
            revision_of(&json_of(
                &host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()),
            ))
        } else {
            "unavailable".to_owned()
        };
        let observed = host.observe_occurrence();
        let exported = host.export_project(revision.as_bytes()).bytes;
        let inspected = host.spreadsheet(&inspect_request(), &source);
        let expected = serde_json::to_vec(&DesignerWireReply::Error {
            error: DesignerError::ProjectionTooLarge {
                actual: 122_730,
                maximum: 65_536,
            }
            .failure_projection(&revision),
        })
        .unwrap();
        assert!(
            inspected.reply == expected,
            "inspection must return the ordinary 65,536-byte refusal"
        );
        assert!(inspected.bytes.is_none());
        assert_eq!(host.observe_occurrence(), observed);
        assert_eq!(host.export_project(revision.as_bytes()).bytes, exported);
    }
}

#[test]
fn eligible_inspection_controls_keep_lexical_text_and_enlarged_replies() {
    let x = "x".repeat(800);
    let y = "y".repeat(800);
    let long_text = "x".repeat(1_200);
    let cases = [
        (vec!["a", "b"], vec![x.as_str(), y.as_str()], 65, true),
        (
            vec!["a", "b", "c"],
            vec![long_text.as_str(), "13.25", "label"],
            64,
            true,
        ),
        (
            vec!["a", "b", "c"],
            vec![long_text.as_str(), "-13.25", "label"],
            65,
            true,
        ),
        (vec!["a", "b"], vec!["small", "text"], 8, false),
    ];
    for (headers, values, rows, enlarged) in cases {
        let source = headered_csv(&headers, &values, rows);
        let workbook = import_csv(&source, &ImportOptions::default()).unwrap();
        assert_eq!(workbook.sheets[0].rows.len(), rows);
        for row in &workbook.sheets[0].rows {
            for (cell, value) in row.iter().zip(&values) {
                assert_eq!(
                    cell.value,
                    SourceValue::Text {
                        value: (*value).into()
                    }
                );
            }
        }
        let expected = serde_json::to_vec(&DesignerWireReply::Ok {
            response: DesignerResponse::ImportPreview(Box::new(workbook)),
        })
        .unwrap();
        assert_eq!(expected.len() > 65_536, enlarged);
        let mut host = DesignerHost::new();
        let inspected = host.spreadsheet(&inspect_request(), &source);
        assert!(
            inspected.reply == expected,
            "inspection must preserve the complete preview: {rows} rows, {} columns",
            headers.len()
        );
        assert!(inspected.bytes.is_none());
        assert_eq!(
            json_of(&host.observe_occurrence())["error"]["code"],
            "no_project_open"
        );
    }
}

#[test]
fn portable_bridge_preserves_v3_bytes_and_admits_constrained_packages() {
    fn fixture(hex: &str) -> Vec<u8> {
        hex.trim_end()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
    let control = fixture(include_str!(
        "../../../../docs/research/fixtures/issue-519-portable-package-v1-v3/runtime-control.hex"
    ));
    let constrained = fixture(include_str!(
        "../../../../docs/research/fixtures/issue-519-portable-package-v1-v3/golden.hex"
    ));
    let mut host = DesignerHost::new();
    let opened = json_of(&host.open_portable_ro(&control, FIRST.as_bytes()));
    assert_eq!(opened["status"], "ok", "{opened}");
    let before = host.observe_occurrence();
    let project_before = host.export_project_v3(b"resident/0").bytes.unwrap();
    let exported = host.export_portable_ro(b"resident/0");
    assert_eq!(json_of(&exported.reply)["status"], "ok");
    assert_eq!(exported.bytes.unwrap(), control);
    assert_eq!(
        json_of(&host.verify_portable_ro(&constrained))["status"],
        "ok"
    );
    // Constrained v3 documents are supported: the package replaces the resident
    // occurrence and re-exports its declarations byte for byte.
    let admitted = json_of(&host.open_portable_ro(&constrained, SECOND.as_bytes()));
    assert_eq!(admitted["status"], "ok", "{admitted}");
    assert_ne!(host.observe_occurrence(), before);
    assert_ne!(
        host.export_project_v3(b"resident/0").bytes.unwrap(),
        project_before
    );
    let reexported = host.export_portable_ro(b"resident/0");
    assert_eq!(json_of(&reexported.reply)["status"], "ok");
    assert_eq!(reexported.bytes.unwrap(), constrained);
    let mut fresh = DesignerHost::new();
    assert_eq!(
        json_of(&fresh.open_portable_ro(&control, THIRD.as_bytes()))["status"],
        "ok"
    );
    assert_eq!(
        fresh.export_project_v3(b"resident/0").bytes.unwrap(),
        project_before
    );
}

/// 64 rows x 2 Text columns of 1,200 characters: a valid document whose
/// complete ordinary reply exceeds today's 64 KiB budget, and which is not
/// text-capacity eligible (that needs 65 rows or exactly three Text columns).
fn long_text_document() -> Document {
    let mut document = Document::empty("working-size-bridge", "Working Size Bridge");
    let mut schema = Schema {
        field_order: None,
        id: SchemaId::from("grid"),
        key: SchemaKey::from("grid"),
        display_name: None,
        fields: BTreeMap::new(),
    };
    for column in 0..2 {
        let id = FieldId::from(format!("c{column}"));
        schema.fields.insert(
            id.clone(),
            FieldDefinition {
                id,
                key: FieldKey::from(format!("c{column}")),
                display_name: None,
                field_type: FieldType::Text,
                required: false,
                constraint: FieldConstraint::None,
                number_format: None,
                date_format: None,
            },
        );
    }
    document.schemas.insert(schema.id.clone(), schema);
    for row in 0..64 {
        let id = EntityId::from(format!("r{row:05}"));
        let fields = (0..2)
            .map(|column| {
                (
                    FieldId::from(format!("c{column}")),
                    CellValue::Text("x".repeat(1_200)),
                )
            })
            .collect();
        document.entities.insert(
            id.clone(),
            Entity {
                id: id.clone(),
                key: EntityKey::from(format!("k{row:05}")),
                schema: SchemaId::from("grid"),
                fields,
            },
        );
    }
    document
}

/// The three transfer shapes of one working-size-only document: the raw
/// canonical document, its v3 project bundle, and its portable package.
fn long_text_transfers() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let document = long_text_document();
    let raw = to_canonical_string(&document).unwrap().into_bytes();
    let portable = encode_portable_package_v1(&encode_roproj_v3(&document).unwrap()).unwrap();
    let mut probe = DesignerHost::new();
    let opened = json_of(&probe.open_local_document_working_size(&raw, FIRST.as_bytes()));
    assert_eq!(opened["status"], "ok", "{opened}");
    let bundle = probe
        .export_project_v3(revision_of(&opened).as_bytes())
        .bytes
        .expect("a working-size runtime exports its v3 bundle");
    (raw, bundle, portable)
}

#[test]
fn working_size_bridge_opens_what_the_ordinary_open_refuses() {
    let (raw, bundle, portable) = long_text_transfers();

    // Today's opens refuse every one of these valid bytes.
    let mut ordinary = DesignerHost::new();
    assert_eq!(
        json_of(&ordinary.open_local_document(&raw, FIRST.as_bytes()))["status"],
        "error"
    );
    assert_eq!(
        json_of(&ordinary.open_project(&bundle, FIRST.as_bytes()))["status"],
        "error"
    );
    assert_eq!(
        json_of(&ordinary.open_portable_ro(&portable, FIRST.as_bytes()))["status"],
        "error"
    );
    assert_eq!(
        json_of(&ordinary.observe_occurrence())["error"]["code"],
        "no_project_open",
        "a refused candidate never becomes resident"
    );

    // The working-size opens admit each of them and reply one window.
    let mut host = DesignerHost::new();
    for reply in [
        host.open_local_document_working_size(&raw, FIRST.as_bytes()),
        host.open_project_working_size(&bundle, SECOND.as_bytes()),
        host.open_portable_ro_working_size(&portable, THIRD.as_bytes()),
    ] {
        let reply = json_of(&reply);
        assert_eq!(reply["status"], "ok", "{reply}");
        assert_eq!(reply["response"]["type"], "opened_window");
        assert_eq!(reply["response"]["payload"]["profile"], "working_size");
        assert_eq!(reply["response"]["payload"]["window"]["total_rows"], 64);
        assert_eq!(
            reply["response"]["payload"]["window"]["rows"]
                .as_array()
                .unwrap()
                .len(),
            64
        );
        assert!(
            reply["response"]["payload"].get("table").is_none(),
            "a working-size reply never carries the whole table"
        );
    }
}

#[test]
fn working_size_refusals_preserve_the_resident_occurrence() {
    let (raw, bundle, _) = long_text_transfers();
    let mut host = DesignerHost::new();
    let opened = json_of(&host.open_local_document_working_size(&raw, FIRST.as_bytes()));
    assert_eq!(opened["status"], "ok");
    let revision = revision_of(&opened);
    let before = host.export_project(revision.as_bytes()).bytes.unwrap();
    let observed = host.observe_occurrence();

    let invalid =
        json_of(&host.open_project_working_size(b"not a Tachiko project", SECOND.as_bytes()));
    assert_eq!(invalid["status"], "error");
    let invalid_utf8 = json_of(&host.open_project_working_size(&bundle, &[0xff, 0xfe]));
    assert_eq!(invalid_utf8["error"]["code"], "invalid_occurrence");
    let oversized = vec![0_u8; MAX_HOST_PROJECT_TRANSFER_BYTES + 1];
    let too_large = json_of(&host.open_project_working_size(&oversized, SECOND.as_bytes()));
    assert_eq!(too_large["error"]["code"], "project_too_large");

    assert_eq!(host.observe_occurrence(), observed);
    assert_eq!(
        host.export_project(revision.as_bytes()).bytes.unwrap(),
        before
    );
}

#[test]
fn working_size_inspection_replies_one_window_and_keeps_the_resident() {
    let (_, bundle, _) = long_text_transfers();

    let empty = DesignerHost::new();
    let inspected = json_of(&empty.inspect_project_working_size(&bundle));
    assert_eq!(inspected["status"], "ok", "{inspected}");
    assert_eq!(inspected["response"]["type"], "opened_window");
    assert_eq!(inspected["response"]["payload"]["profile"], "working_size");
    assert_eq!(inspected["response"]["payload"]["window"]["total_rows"], 64);
    assert_eq!(
        json_of(&empty.observe_occurrence())["error"]["code"],
        "no_project_open",
        "inspection installs nothing"
    );
    let mut host = DesignerHost::new();
    let resident = json_of(&host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()));
    assert_eq!(resident["status"], "ok");
    let revision = revision_of(&resident);
    let before = host.export_project(revision.as_bytes()).bytes.unwrap();
    let observed = host.observe_occurrence();
    let inspected = json_of(&host.inspect_project_working_size(&bundle));
    assert_eq!(inspected["response"]["type"], "opened_window");
    assert_eq!(host.observe_occurrence(), observed);
    assert_eq!(
        host.export_project(revision.as_bytes()).bytes.unwrap(),
        before,
        "the resident occurrence and its profile are untouched"
    );
}

#[test]
fn a_working_size_table_saves_and_reopens_across_the_bridge() {
    let mut host = DesignerHost::new();
    let created = json_of(&host.request(&request(&json!({
        "type": "new_table",
        "occurrence_id": FIRST,
        "name": "Wire",
        "columns": [{"name": "amount", "field_type": "number", "required": false}],
        "profile": "working_size",
    }))));
    assert_eq!(created["status"], "ok", "{created}");
    assert_eq!(created["response"]["type"], "opened_window");
    let payload = &created["response"]["payload"];
    assert_eq!(payload["profile"], "working_size");
    assert_eq!(payload["window"]["total_rows"], 0);
    let collection = payload["bootstrap"]["default_collection"]
        .as_str()
        .unwrap()
        .to_owned();
    let revision = payload["bootstrap"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();

    let inserted = json_of(&host.request(&request(&json!({
        "type": "insert_row",
        "expected_revision": revision,
        "collection": collection,
        "initializers": [],
    }))));
    assert_eq!(inserted["status"], "ok", "{inserted}");
    let saved = host.export_project_v3(revision_of(&inserted).as_bytes());
    assert_eq!(json_of(&saved.reply)["status"], "ok");
    let bytes = saved.bytes.expect("one saved row exports");

    host.close_project();
    let reopened = json_of(&host.open_project_working_size(&bytes, SECOND.as_bytes()));
    assert_eq!(reopened["status"], "ok", "{reopened}");
    assert_eq!(reopened["response"]["type"], "opened_window");
    assert_eq!(reopened["response"]["payload"]["window"]["total_rows"], 1);
    assert_eq!(
        reopened["response"]["payload"]["window"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_large_paste_reaches_a_working_size_table_across_the_bridge() {
    let mut host = DesignerHost::new();
    let created = json_of(&host.request(&request(&json!({
        "type": "new_table",
        "occurrence_id": FIRST,
        "name": "Paste",
        "columns": [
            {"name": "name", "field_type": "text", "required": false},
            {"name": "note", "field_type": "text", "required": false},
        ],
        "profile": "working_size",
    }))));
    assert_eq!(created["status"], "ok", "{created}");
    let payload = &created["response"]["payload"];
    let collection = payload["bootstrap"]["default_collection"]
        .as_str()
        .unwrap()
        .to_owned();
    let revision = payload["bootstrap"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let start_field = payload["window"]["columns"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let rows = (0..3_000)
        .map(|row| json!([format!("pasted-{row:04}"), format!("note-{row:04}")]))
        .collect::<Vec<_>>();
    let paste = request(&json!({
        "type": "paste_cells",
        "expected_revision": revision,
        "collection": collection,
        "start_field": start_field,
        "rows": rows,
    }));
    assert!(
        paste.len() > 65_536,
        "paste request is only {} bytes",
        paste.len()
    );
    let pasted = json_of(&host.request(&paste));
    assert_eq!(pasted["status"], "ok", "{pasted}");
    assert_eq!(pasted["response"]["payload"]["many"], true);

    let window = json_of(&host.request(&request(&json!({
        "type": "query_table_window",
        "expected_revision": revision_of(&pasted),
        "collection": collection,
        "start": 0,
        "count": 5,
    }))));
    assert_eq!(window["status"], "ok", "{window}");
    assert_eq!(window["response"]["payload"]["total_rows"], 3_000);
    assert_eq!(
        window["response"]["payload"]["rows"][0]["fields"][0]["stored"]["value"],
        "pasted-0000"
    );
}

/// One CSV import operation with an optional profile, typed Text in every column.
fn spreadsheet_import(
    occurrence: &str,
    install: bool,
    profile: Option<&str>,
    columns: usize,
) -> Vec<u8> {
    let selection = serde_json::to_value(ImportSelection {
        column_types: vec![vec![ImportFieldType::Text; columns]],
        extra_columns: vec![Vec::new()],
    })
    .unwrap();
    let mut operation = json!({
        "type": "import",
        "format": "csv",
        "csv_options": { "delimiter": ",", "header": true },
        "selection": selection,
        "occurrence_id": occurrence,
        "install": install,
    });
    if let Some(profile) = profile {
        operation["profile"] = json!(profile);
    }
    request(&operation)
}

#[test]
fn working_size_import_installs_only_after_admission() {
    let mut host = DesignerHost::new();
    let source = b"Name,Note\nAlpha,first\nBeta,second\n";

    let candidate = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(THIRD, false, Some("working_size"), 2),
                source,
            )
            .reply,
    );
    assert_eq!(candidate["status"], "ok", "{candidate}");
    assert_eq!(candidate["response"]["type"], "imported_window");
    assert_eq!(
        candidate["response"]["payload"]["opened"]["profile"],
        "working_size"
    );
    assert_eq!(
        json_of(&host.observe_occurrence())["error"]["code"],
        "no_project_open",
        "an uninstalled working-size candidate does not become resident"
    );

    let installed = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(THIRD, true, Some("working_size"), 2),
                source,
            )
            .reply,
    );
    assert_eq!(installed["status"], "ok", "{installed}");
    let payload = &installed["response"]["payload"]["opened"];
    let revision = payload["bootstrap"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let collection = payload["window"]["collection"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        json_of(&host.observe_occurrence())["status"],
        "ok",
        "the installed import is resident"
    );
    let window = json_of(&host.request(&request(&json!({
        "type": "query_table_window",
        "expected_revision": revision,
        "collection": collection,
        "start": 0,
        "count": 50,
    }))));
    assert_eq!(window["status"], "ok", "{window}");
    assert_eq!(window["response"]["payload"]["total_rows"], 2);
}

#[test]
fn working_size_import_matches_the_ordinary_mapping() {
    let source = b"Name,Note\nAlpha,first\nBeta,second\n";
    let mut host = DesignerHost::new();
    let ordinary = json_of(
        &host
            .spreadsheet(&spreadsheet_import(THIRD, false, None, 2), source)
            .reply,
    );
    let working = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(THIRD, false, Some("working_size"), 2),
                source,
            )
            .reply,
    );
    assert_eq!(ordinary["status"], "ok", "{ordinary}");
    assert_eq!(working["status"], "ok", "{working}");
    let ordinary = &ordinary["response"]["payload"];
    let working = &working["response"]["payload"];
    // Working size carries metadata version 2: the ordinary version-1 sheets
    // without rows and widths.
    assert_eq!(ordinary["metadata"]["version"], 1);
    assert_eq!(working["metadata"]["version"], 2);
    let mut expected = ordinary["metadata"]["sheets"].clone();
    for sheet in expected.as_array_mut().unwrap() {
        let sheet = sheet.as_object_mut().unwrap();
        sheet.remove("rows");
        for column in sheet["columns"].as_array_mut().unwrap() {
            column.as_object_mut().unwrap().remove("width");
        }
    }
    assert_eq!(expected, working["metadata"]["sheets"]);
    assert_eq!(ordinary["ledger"], working["ledger"]);
    assert_eq!(working["opened"]["window"]["total_rows"], 2);
    assert_eq!(
        ordinary["opened"]["table"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        usize::try_from(working["opened"]["window"]["total_rows"].as_u64().unwrap()).unwrap()
    );
}

#[test]
fn working_size_import_binds_formulas() {
    let invoice = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/excel_samples/invoice.xlsx"
    ));
    let selection = serde_json::to_value(ImportSelection {
        column_types: vec![vec![
            ImportFieldType::Text,
            ImportFieldType::Number,
            ImportFieldType::Number,
            ImportFieldType::Number,
        ]],
        extra_columns: vec![Vec::new()],
    })
    .unwrap();
    let mut host = DesignerHost::new();
    let reply = json_of(
        &host
            .spreadsheet(
                &request(&json!({
                    "type": "import",
                    "format": "xlsx",
                    "csv_options": { "delimiter": ",", "header": true },
                    "selection": selection,
                    "occurrence_id": THIRD,
                    "install": false,
                    "profile": "working_size",
                })),
                invoice,
            )
            .reply,
    );
    assert_eq!(reply["status"], "ok", "{reply}");
    let rows = reply["response"]["payload"]["opened"]["window"]["rows"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 5);
    assert!(
        rows[0]["fields"][3]["formula"].is_object(),
        "the Amount column keeps a formula: {}",
        rows[0]
    );
    let qty = rows[0]["fields"][1]["stored"]["value"].as_f64().unwrap();
    let unit = rows[0]["fields"][2]["stored"]["value"].as_f64().unwrap();
    let amount = rows[0]["fields"][3]["calculated"]["value"]
        .as_f64()
        .unwrap();
    assert!(
        (amount - qty * unit).abs() < 1e-9,
        "the window shows the calculated value: {amount} != {qty} * {unit}"
    );
}

#[test]
fn working_size_import_keeps_todays_geometry_for_a_promoted_csv() {
    let source = headered_csv(&["a", "b"], &["x", "y"], 200);

    // Today's ordinary import accepts this eligible rectangle through the text
    // profile: its complete projection exceeds the ordinary reply budget.
    let mut ordinary = DesignerHost::new();
    let imported = json_of(
        &ordinary
            .spreadsheet(&spreadsheet_import(THIRD, false, None, 2), &source)
            .reply,
    );
    assert_eq!(imported["status"], "ok", "{imported}");
    assert_eq!(
        imported["response"]["payload"]["opened"]["table"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        200
    );

    let mut host = DesignerHost::new();
    let resident = json_of(&host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()));
    assert_eq!(resident["status"], "ok", "{resident}");
    let observed = host.observe_occurrence();
    let reply = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(THIRD, true, Some("working_size"), 2),
                &source,
            )
            .reply,
    );
    assert_eq!(
        reply["status"], "error",
        "the metadata profile refuses above today's geometry: {reply}"
    );
    assert_eq!(
        reply["error"]["code"], "imported_spreadsheet_export_refused",
        "the import path keeps today's 128-row metadata geometry: {reply}"
    );
    assert_eq!(
        reply["error"]["diagnostics"][0]["code"], "imported_spreadsheet_unrepresentable",
        "{reply}"
    );
    assert_eq!(
        host.observe_occurrence(),
        observed,
        "a refused working-size import installs nothing"
    );
}

#[test]
fn working_size_imported_project_saves_and_reopens() {
    let mut host = DesignerHost::new();
    let source = b"Name,Note\nAlpha,first\nBeta,second\n";
    let imported = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(THIRD, true, Some("working_size"), 2),
                source,
            )
            .reply,
    );
    assert_eq!(imported["status"], "ok", "{imported}");
    let payload = &imported["response"]["payload"];
    let metadata = payload["metadata"].clone();
    let revision = payload["opened"]["bootstrap"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let total_rows = payload["opened"]["window"]["total_rows"].clone();

    let saved = host.export_project_v3(revision.as_bytes());
    assert_eq!(json_of(&saved.reply)["status"], "ok");
    let bytes = saved
        .bytes
        .expect("the working-size import saves a v3 bundle");

    let mut reopened = DesignerHost::new();
    let opened = json_of(
        &reopened
            .spreadsheet(
                &request(&json!({
                    "type": "open_project",
                    "occurrence_id": SECOND,
                    "metadata": metadata.clone(),
                    "profile": "working_size",
                })),
                &bytes,
            )
            .reply,
    );
    assert_eq!(opened["status"], "ok", "{opened}");
    assert_eq!(opened["response"]["type"], "opened_window");
    assert_eq!(opened["response"]["payload"]["profile"], "working_size");
    assert_eq!(
        opened["response"]["payload"]["window"]["total_rows"],
        total_rows
    );

    let observed = reopened.observe_occurrence();
    let inspected = json_of(
        &reopened
            .spreadsheet(
                &request(&json!({
                    "type": "inspect_project",
                    "metadata": metadata,
                    "profile": "working_size",
                })),
                &bytes,
            )
            .reply,
    );
    assert_eq!(inspected["status"], "ok", "{inspected}");
    assert_eq!(inspected["response"]["type"], "opened_window");
    assert_eq!(
        inspected["response"]["payload"]["window"]["total_rows"],
        total_rows
    );
    assert_eq!(
        reopened.observe_occurrence(),
        observed,
        "inspection leaves the resident revision unchanged"
    );
}

#[test]
fn working_size_imported_open_refuses_foreign_metadata() {
    let mut host = DesignerHost::new();
    let imported = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(THIRD, true, Some("working_size"), 2),
                b"Name,Note\nAlpha,first\nBeta,second\n",
            )
            .reply,
    );
    assert_eq!(imported["status"], "ok", "{imported}");
    let revision = imported["response"]["payload"]["opened"]["bootstrap"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let bytes = host
        .export_project_v3(revision.as_bytes())
        .bytes
        .expect("the working-size import saves a v3 bundle");

    let foreign = json_of(
        &host
            .spreadsheet(
                &spreadsheet_import(SECOND, false, Some("working_size"), 2),
                b"Name,Note\nGamma,third\n",
            )
            .reply,
    );
    assert_eq!(foreign["status"], "ok", "{foreign}");
    let foreign_metadata = foreign["response"]["payload"]["metadata"].clone();

    host.close_project();
    let resident = json_of(&host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()));
    assert_eq!(resident["status"], "ok", "{resident}");
    let observed = host.observe_occurrence();
    let refused = json_of(
        &host
            .spreadsheet(
                &request(&json!({
                    "type": "open_project",
                    "occurrence_id": SECOND,
                    "metadata": foreign_metadata,
                    "profile": "working_size",
                })),
                &bytes,
            )
            .reply,
    );
    assert_eq!(
        refused["status"], "error",
        "the closure check refuses metadata from another import: {refused}"
    );
    assert_eq!(
        host.observe_occurrence(),
        observed,
        "a refused imported open leaves the resident unchanged"
    );
}

#[test]
fn working_size_profile_typos_are_refused_on_every_operation() {
    let mut host = DesignerHost::new();
    let resident = json_of(&host.open_local_document(GAME_BALANCE_RO, FIRST.as_bytes()));
    assert_eq!(resident["status"], "ok", "{resident}");
    let observed = host.observe_occurrence();
    let metadata = json!({ "version": 1, "sheets": [] });
    let replies = [
        host.spreadsheet(
            &spreadsheet_import(SECOND, false, Some("other"), 2),
            b"a,b\n1,2\n",
        )
        .reply,
        host.spreadsheet(
            &request(&json!({
                "type": "inspect_project",
                "metadata": metadata,
                "profile": "other",
            })),
            b"not a project",
        )
        .reply,
        host.spreadsheet(
            &request(&json!({
                "type": "open_project",
                "occurrence_id": SECOND,
                "metadata": metadata,
                "profile": "other",
            })),
            b"not a project",
        )
        .reply,
    ];
    for reply in replies {
        let reply = json_of(&reply);
        assert_eq!(reply["status"], "error", "{reply}");
        assert_eq!(reply["error"]["code"], "invalid_project", "{reply}");
        assert_eq!(host.observe_occurrence(), observed);
    }
}
