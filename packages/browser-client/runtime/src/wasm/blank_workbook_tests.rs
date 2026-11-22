//! Exercise the production request ABI against the native host for the blank-grid journey.
use super::{REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_request_run};
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

#[test]
fn blank_workbook_wasm_and_native_host_requests_preserve_the_atomic_journey() {
    tachiko_designer_project_close();
    let mut host = DesignerHost::default();
    let created = exchange(
        &mut host,
        &json!({"type":"new_blank_workbook",
        "occurrence_id":"00000000-0000-4000-8000-000000000899", "profile":"working_size"}),
    );
    assert_eq!(created["status"], "ok", "{created}");
    assert_eq!(created["response"]["type"], "opened_window");
    let collection = created["response"]["payload"]["bootstrap"]["default_collection"].clone();
    assert_eq!(
        created["response"]["payload"]["window"]["collection"]["headerless"],
        true
    );
    assert_eq!(
        created["response"]["payload"]["window"]["collection"]["data_start_row"],
        1
    );
    let mut revision = "resident/0".to_owned();
    for (row, column, input) in [
        (1, 1, "Item"),
        (1, 2, "Price"),
        (2, 1, "Tea"),
        (2, 2, "120"),
    ] {
        let reply = exchange(
            &mut host,
            &json!({"type":"write_cell_at", "expected_revision":revision,
            "collection":collection, "row":row, "column":column, "input":{"kind":"auto", "input":input}}),
        );
        assert_eq!(reply["status"], "ok", "{reply}");
        revision = reply["response"]["payload"]["resulting_revision"]
            .as_str()
            .unwrap()
            .to_owned();
    }
    let table = exchange(
        &mut host,
        &json!({"type":"query_table_window", "expected_revision":revision,
        "collection":collection, "start":0, "count":10}),
    );
    let window = &table["response"]["payload"];
    assert_eq!(
        window["rows"][1]["fields"][1]["stored"],
        json!({"kind":"number","value":120.0})
    );
    assert_eq!(window["columns"][1]["display_name"], "Column B");
    for (row, input) in [(40, "1"), (3, "=1+1")] {
        let refused = exchange(
            &mut host,
            &json!({"type":"write_cell_at", "expected_revision":revision,
            "collection":collection, "row":row, "column":2, "input":{"kind":"auto", "input":input}}),
        );
        assert_eq!(refused["status"], "error");
    }
    for direction in ["undo", "redo"] {
        let reply = exchange(
            &mut host,
            &json!({"type":direction,"expected_revision":revision}),
        );
        assert_eq!(reply["status"], "ok", "{reply}");
        revision = reply["response"]["payload"]["resulting_revision"]
            .as_str()
            .unwrap()
            .to_owned();
    }
    let restored = exchange(
        &mut host,
        &json!({"type":"query_table_window", "expected_revision":revision,
        "collection":collection, "start":0, "count":10}),
    );
    assert_eq!(restored["response"]["payload"]["rows"], window["rows"]);
    tachiko_designer_project_close();
}
