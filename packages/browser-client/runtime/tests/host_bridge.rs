//! The native host bridge must answer exactly like the private wire path and
//! preserve the resident occurrence on every refused or failed operation.
//! The bridge is native-only, so this test is too.
#![cfg(not(target_arch = "wasm32"))]

use serde_json::{Value, json};
use tachiko_designer_runtime::{
    DesignerResponse, DesignerRuntime, DesignerWireReply, ImportFieldType, ImportSelection,
    host_bridge::{DesignerHost, MAX_HOST_PROJECT_TRANSFER_BYTES},
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
