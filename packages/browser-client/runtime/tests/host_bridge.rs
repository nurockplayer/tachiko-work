//! The native host bridge must answer exactly like the private wire path and
//! preserve the resident occurrence on every refused or failed operation.
//! The bridge is native-only, so this test is too.
#![cfg(not(target_arch = "wasm32"))]

use serde_json::{Value, json};
use tachiko_designer_runtime::{
    DesignerError, DesignerResponse, DesignerRuntime, DesignerWireReply, ImportFieldType,
    ImportSelection,
    host_bridge::{DesignerHost, MAX_HOST_PROJECT_TRANSFER_BYTES},
    interop_adapter::{ImportOptions, SourceValue, import_csv},
    open_local_document, process_wire_request,
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
