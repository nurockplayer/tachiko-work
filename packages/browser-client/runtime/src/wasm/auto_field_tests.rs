//! The production byte bridge and native host publish the same Auto scalars.
use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_portable_ro_export,
    tachiko_designer_portable_ro_open, tachiko_designer_project_close,
    tachiko_designer_project_export, tachiko_designer_request_run,
};
use crate::host_bridge::DesignerHost;
use serde_json::{Value, json};

fn exchange(host: &mut DesignerHost, request: &Value) -> Value {
    let bytes = serde_json::to_vec(request).unwrap();
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&bytes));
    tachiko_designer_request_run();
    let reply = RESPONSE.with(|slot| slot.borrow().clone());
    assert_eq!(reply, host.request(&bytes));
    serde_json::from_slice(&reply).unwrap()
}

fn ok(host: &mut DesignerHost, request: &Value) -> Value {
    let reply = exchange(host, request);
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"].clone()
}

fn save_reopen(host: &mut DesignerHost, revision: &str, collection: &Value, before: &Value) {
    REQUEST.with(|slot| *slot.borrow_mut() = revision.as_bytes().to_vec());
    tachiko_designer_portable_ro_export();
    let native = host.export_portable_ro(revision.as_bytes());
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), native.reply);
    let bytes = PROJECT.with(|slot| slot.borrow().clone());
    assert_eq!(bytes, native.bytes.unwrap());
    let occurrence = b"00000000-0000-4000-8000-000000000889";
    REQUEST.with(|slot| *slot.borrow_mut() = occurrence.to_vec());
    tachiko_designer_portable_ro_open();
    let browser = RESPONSE.with(|slot| slot.borrow().clone());
    assert_eq!(browser, host.open_portable_ro(&bytes, occurrence));
    let reopened = ok(
        host,
        &json!({"type":"query_table_window", "expected_revision":"resident/0",
        "collection":collection, "view":{}, "start":0, "count":10, "display":true}),
    );
    assert_eq!(reopened["rows"], before["rows"]);
    assert_eq!(reopened["columns"], before["columns"]);
}

fn scalar_edits(host: &mut DesignerHost, target: &Value, collection: &Value) -> String {
    let mut revision = "resident/1".to_owned();
    for (input, kind) in [
        ("00123", "text"),
        ("2026-11-22", "date"),
        ("TRUE", "boolean"),
        ("hello", "text"),
        ("42", "number"),
    ] {
        let publication = ok(
            host,
            &json!({"type":"edit_scalar", "expected_revision":revision,
            "target":target, "input":{"kind":"auto", "input":input}}),
        );
        publication["resulting_revision"]
            .as_str()
            .unwrap()
            .clone_into(&mut revision);
        let window = ok(
            host,
            &json!({"type":"query_table_window", "expected_revision":revision,
            "collection":collection, "start":0, "count":1}),
        );
        assert_eq!(window["rows"][0]["fields"][0]["stored"]["kind"], kind);
    }
    for input in [json!({"kind":"text", "value":""}), json!({"kind":"clear"})] {
        let publication = ok(
            host,
            &json!({"type":"edit_scalar", "expected_revision":revision,
            "target":target, "input":input}),
        );
        publication["resulting_revision"]
            .as_str()
            .unwrap()
            .clone_into(&mut revision);
    }
    for direction in ["undo", "undo", "undo", "redo"] {
        let publication = ok(
            host,
            &json!({"type":direction, "expected_revision":revision}),
        );
        publication["resulting_revision"]
            .as_str()
            .unwrap()
            .clone_into(&mut revision);
    }
    revision
}

#[test]
fn auto_wasm_bridge_tags_edits_history_and_portable_roundtrip() {
    tachiko_designer_project_close();
    let mut host = DesignerHost::new();
    let opened = ok(
        &mut host,
        &json!({"type":"new_table",
        "occurrence_id":"00000000-0000-4000-8000-000000000887", "name":"Mixed",
        "columns":[{"name":"Value", "field_type":"auto", "required":false}]}),
    );
    let collection = opened["table"]["collection"]["id"].clone();
    let field = opened["table"]["columns"][0]["id"].clone();
    ok(
        &mut host,
        &json!({"type":"paste_cells", "expected_revision":"resident/0", "collection":collection,
        "start_entity":null, "start_field":field,
        "rows":[["42"],["00123"],["2026-11-22"],["TRUE"],["hello"],
            [{"kind":"auto", "input":"22/11/2026", "date_order":"day_first"}]]}),
    );
    let window = ok(
        &mut host,
        &json!({"type":"query_table_window", "expected_revision":"resident/1",
        "collection":collection, "view":{}, "start":0, "count":10, "display":true}),
    );
    for (row, tag) in window["rows"]
        .as_array()
        .unwrap()
        .iter()
        .zip(["number", "text", "date", "boolean", "text", "date"])
    {
        assert_eq!(row["fields"][0]["stored"]["kind"], tag);
        assert_eq!(row["fields"][0]["editable_scalar"], "auto");
    }
    assert_eq!(window["rows"][1]["fields"][0]["stored"]["value"], "00123");
    let target = json!({"entity":window["rows"][0]["id"], "field":field});
    let revision = scalar_edits(&mut host, &target, &collection);
    REQUEST.with(|slot| *slot.borrow_mut() = revision.as_bytes().to_vec());
    tachiko_designer_project_export();
    let legacy = RESPONSE.with(|slot| slot.borrow().clone());
    assert_eq!(legacy, host.export_project(revision.as_bytes()).reply);
    let legacy_reply: Value = serde_json::from_slice(&legacy).unwrap();
    assert_eq!(legacy_reply["status"], "error");
    assert!(
        legacy_reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Auto")
    );
    save_reopen(&mut host, &revision, &collection, &window);
    tachiko_designer_project_close();
}

#[test]
fn auto_wasm_bridge_publishes_formulas_and_refuses_declared_failures() {
    tachiko_designer_project_close();
    let mut host = DesignerHost::new();
    let opened = ok(
        &mut host,
        &json!({
            "type":"new_table", "occurrence_id":"00000000-0000-4000-8000-000000000949",
            "name":"Mixed", "columns":[
                {"name":"Input", "field_type":"auto", "required":false},
                {"name":"Result", "field_type":"auto", "required":false},
                {"name":"Typed", "field_type":"number", "required":false}
            ]
        }),
    );
    let collection = &opened["table"]["collection"]["id"];
    let columns = &opened["table"]["columns"];
    let field = &columns[0]["id"];
    ok(
        &mut host,
        &json!({
            "type":"paste_cells", "expected_revision":"resident/0", "collection":collection,
            "start_entity":null, "start_field":field,
            "rows":[["00123","=[.input]+[.typed]","1"],["2026-01-01","=[.input]+1","1"],
                ["TRUE","=[.input]","1"],["tea","=[.input]","1"],["tea","=[.input]*2","1"],
                ["","=[.input]+1","1"],["0","=1/[.input]","1"]]
        }),
    );
    let query = |revision: &str| {
        json!({
            "type":"query_table_window", "expected_revision":revision, "collection":collection,
            "view":{}, "start":0, "count":10, "display":true
        })
    };
    let window = ok(&mut host, &query("resident/1"));
    for (row, value) in [
        (0, json!({"kind":"number","value":124})),
        (1, json!({"kind":"date","value":"2026-01-02"})),
        (2, json!({"kind":"boolean","value":true})),
        (3, json!({"kind":"text","value":"tea"})),
        (5, json!({"kind":"number","value":1})),
    ] {
        let result = &window["rows"][row]["fields"][1]["calculated"]["result"];
        assert_eq!(result["kind"], value["kind"]);
        if let Some(number) = value["value"].as_f64() {
            assert_eq!(result["value"].as_f64(), Some(number));
        } else {
            assert_eq!(result["value"], value["value"]);
        }
    }
    for (row, code, display) in [(4, "value", "#VALUE!"), (6, "division_by_zero", "#DIV/0!")] {
        let field = &window["rows"][row]["fields"][1];
        assert_eq!(field["calculated"]["status"], "failure");
        assert_eq!(field["calculated"]["code"], code);
        assert_eq!(field["display"], display);
    }
    let row = &window["rows"][0]["id"];
    let target = json!({"entity":row, "field":columns[2]["id"]});
    let refused = exchange(
        &mut host,
        &json!({
            "type":"formula_update", "expected_revision":"resident/1", "target":target, "source":"1/0"
        }),
    );
    assert_eq!(refused["status"], "error");
    assert_eq!(ok(&mut host, &query("resident/1")), window);
    save_reopen(&mut host, "resident/1", collection, &window);
    tachiko_designer_project_close();
}
