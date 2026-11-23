//! Exercise the production request ABI against the native host for the blank-grid journey.
use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_portable_ro_export,
    tachiko_designer_portable_ro_open_working_size, tachiko_designer_project_close,
    tachiko_designer_project_export_v3, tachiko_designer_project_open_working_size,
    tachiko_designer_request_run,
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
    for (row, input) in [(10_001, "1"), (3, "=1+1")] {
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

fn start(request: &Value) -> (DesignerHost, Value, String) {
    tachiko_designer_project_close();
    let mut host = DesignerHost::default();
    let created = exchange(&mut host, request);
    assert_eq!(created["status"], "ok", "{created}");
    let collection = created["response"]["payload"]["bootstrap"]["default_collection"].clone();
    let revision = created["response"]["payload"]["window"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    (host, collection, revision)
}

fn seeded() -> (DesignerHost, Value, String) {
    let (mut host, collection, mut revision) = start(&json!({
        "type":"new_blank_workbook", "profile":"working_size",
        "occurrence_id":"00000000-0000-4000-8000-000000000923"
    }));
    for (row, column) in [(1, 1), (1, 2), (3, 1)] {
        write_at(&mut host, &collection, &mut revision, row, column);
    }
    (host, collection, revision)
}

fn write_at(
    host: &mut DesignerHost,
    collection: &Value,
    revision: &mut String,
    row: usize,
    column: usize,
) {
    let reply = exchange(
        host,
        &json!({"type":"write_cell_at",
        "expected_revision":revision, "collection":collection, "row":row, "column":column,
        "input":{"kind":"auto","input":"923"}}),
    );
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"]["resulting_revision"]
        .as_str()
        .unwrap()
        .clone_into(revision);
}

fn table(host: &mut DesignerHost, collection: &Value, revision: &str) -> Value {
    let reply = exchange(
        host,
        &json!({"type":"query_table_window",
        "expected_revision":revision,"collection":collection,"start":0,"count":40}),
    );
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"].clone()
}

#[test]
fn distant_write_bridge_preserves_sparse_positions_and_labels() {
    let (mut host, collection, mut revision) = seeded();
    write_at(&mut host, &collection, &mut revision, 40, 6);
    let window = table(&mut host, &collection, &revision);
    assert_eq!(window["total_rows"], 40);
    assert_eq!(window["columns"].as_array().unwrap().len(), 6);
    for (column, letter) in window["columns"].as_array().unwrap().iter().zip('A'..='F') {
        assert_eq!(column["display_name"], format!("Column {letter}"));
        assert_eq!(column["field_type"], "auto");
    }
    assert_eq!(window["rows"][39]["position"], 40);
    let written = window["rows"][39]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["target"]["field"] == window["columns"][5]["id"])
        .unwrap();
    assert_eq!(written["stored"], json!({"kind":"number","value":923.0}));
    let column_ids = window["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column["id"].clone())
        .collect::<Vec<_>>();
    for row in &window["rows"].as_array().unwrap()[3..39] {
        assert!(row["fields"].as_array().unwrap().is_empty());
        assert_eq!(row["blank_fields"], json!(column_ids));
    }
    tachiko_designer_project_close();
}

#[test]
fn distant_write_bridge_undo_redo_restores_rows_and_columns_together() {
    let (mut host, collection, mut revision) = seeded();
    let before = table(&mut host, &collection, &revision);
    write_at(&mut host, &collection, &mut revision, 40, 6);
    let after = table(&mut host, &collection, &revision);
    for (direction, expected) in [("undo", before), ("redo", after)] {
        let reply = exchange(
            &mut host,
            &json!({"type":direction,"expected_revision":revision}),
        );
        assert_eq!(reply["status"], "ok", "{reply}");
        revision = reply["response"]["payload"]["resulting_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let actual = table(&mut host, &collection, &revision);
        assert_eq!(actual["rows"], expected["rows"]);
        assert_eq!(actual["columns"], expected["columns"]);
        assert_eq!(actual["total_rows"], expected["total_rows"]);
    }
    tachiko_designer_project_close();
}

#[test]
fn distant_write_bridge_v3_and_portable_reopen_keep_positions() {
    for portable in [false, true] {
        let (mut host, collection, mut revision) = seeded();
        write_at(&mut host, &collection, &mut revision, 40, 6);
        let before = table(&mut host, &collection, &revision);
        REQUEST.with(|slot| *slot.borrow_mut() = revision.as_bytes().to_vec());
        let native = if portable {
            tachiko_designer_portable_ro_export();
            host.export_portable_ro(revision.as_bytes())
        } else {
            tachiko_designer_project_export_v3();
            host.export_project_v3(revision.as_bytes())
        };
        assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), native.reply);
        let bytes = PROJECT.with(|slot| slot.borrow().clone());
        assert_eq!(Some(&bytes), native.bytes.as_ref());
        tachiko_designer_project_close();
        let occurrence = b"00000000-0000-4000-8000-000000000924";
        REQUEST.with(|slot| *slot.borrow_mut() = occurrence.to_vec());
        PROJECT.with(|slot| slot.borrow_mut().clone_from(&bytes));
        let native = if portable {
            tachiko_designer_portable_ro_open_working_size();
            host.open_portable_ro_working_size(&bytes, occurrence)
        } else {
            tachiko_designer_project_open_working_size();
            host.open_project_working_size(&bytes, occurrence)
        };
        let reply = RESPONSE.with(|slot| slot.borrow().clone());
        assert_eq!(reply, native);
        let reply: Value = serde_json::from_slice(&reply).unwrap();
        assert_eq!(reply["status"], "ok", "{reply}");
        let after = table(&mut host, &collection, "resident/0");
        assert_eq!(after["rows"], before["rows"]);
        assert_eq!(after["columns"], before["columns"]);
        tachiko_designer_project_close();
    }
}

#[test]
fn distant_write_bridge_capacity_refusals_are_typed_and_atomic() {
    let (mut host, collection, mut revision) = seeded();
    let before = table(&mut host, &collection, &revision);
    for (row, column, limit, maximum, attempted) in [
        (10_001, 6, "table_rows", 10_000, 10_001),
        (40, 33, "table_fields", 32, 33),
    ] {
        let reply = exchange(
            &mut host,
            &json!({"type":"write_cell_at",
            "expected_revision":revision,"collection":collection,"row":row,"column":column,
            "input":{"kind":"auto","input":"923"}}),
        );
        assert_eq!(reply["status"], "error");
        assert_eq!(reply["error"]["code"], "capacity_exceeded");
        assert_eq!(reply["error"]["limit"], limit);
        assert_eq!(reply["error"]["maximum"], maximum);
        assert_eq!(reply["error"]["attempted"], attempted);
        assert_eq!(reply["error"]["current_revision"], revision);
        assert_eq!(table(&mut host, &collection, &revision), before);
    }
    write_at(&mut host, &collection, &mut revision, 10_000, 32);
    let reply = exchange(
        &mut host,
        &json!({"type":"query_table_window",
        "expected_revision":revision,"collection":collection,"start":9999,"count":1}),
    );
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(reply["response"]["payload"]["total_rows"], 10_000);
    assert_eq!(
        reply["response"]["payload"]["columns"][31]["display_name"],
        "Column AF"
    );
    let window = &reply["response"]["payload"];
    assert_eq!(window["rows"][0]["position"], 10_000);
    let written = window["rows"][0]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["target"]["field"] == window["columns"][31]["id"])
        .unwrap();
    assert_eq!(written["stored"], json!({"kind":"number","value":923.0}));
    tachiko_designer_project_close();
}

#[test]
fn distant_write_bridge_keeps_declared_type_admission_and_adjacent_writes() {
    let (mut host, collection, mut revision) = start(&json!({
        "type":"new_table", "profile":"working_size", "name":"Numbers",
        "occurrence_id":"00000000-0000-4000-8000-000000000923",
        "columns":[{"name":"Amount","field_type":"number","required":true}]
    }));
    let before = table(&mut host, &collection, &revision);
    let invalid = exchange(
        &mut host,
        &json!({"type":"write_cell_at",
        "expected_revision":revision,"collection":collection,"row":40,"column":1,
        "input":{"kind":"number","input":"invalid"}}),
    );
    assert_eq!(invalid["status"], "error");
    assert_eq!(table(&mut host, &collection, &revision), before);
    for row in [1, 2, 40] {
        let reply = exchange(
            &mut host,
            &json!({"type":"write_cell_at",
            "expected_revision":revision,"collection":collection,"row":row,"column":1,
            "input":{"kind":"number","input":"7"}}),
        );
        assert_eq!(reply["status"], "ok", "{reply}");
        revision = reply["response"]["payload"]["resulting_revision"]
            .as_str()
            .unwrap()
            .to_owned();
    }
    let after = table(&mut host, &collection, &revision);
    assert_eq!(after["columns"][0]["field_type"], "number");
    assert_eq!(
        after["rows"][39]["fields"][0]["stored"],
        json!({"kind":"number","value":7.0})
    );
    for row in &after["rows"].as_array().unwrap()[2..39] {
        assert_eq!(
            row["fields"][0]["stored"],
            json!({"kind":"number","value":0.0})
        );
    }
    tachiko_designer_project_close();
}
