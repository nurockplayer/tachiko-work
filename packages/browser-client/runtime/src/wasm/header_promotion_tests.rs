//! Each acceptance case uses both production byte bridges and compares replies.
use serde_json::{Value, json};

use super::{
    PROJECT, REQUEST, RESPONSE, RUNTIME, tachiko_designer_portable_ro_export,
    tachiko_designer_portable_ro_open_working_size, tachiko_designer_project_close,
    tachiko_designer_project_export_v3, tachiko_designer_project_open_working_size,
    tachiko_designer_request_run,
};
use crate::{header_promotion_tests::typed, host_bridge::DesignerHost};

const OCCURRENCE: &[u8] = b"00000000-0000-4000-8000-000000000924";

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

fn setup() -> DesignerHost {
    tachiko_designer_project_close();
    let runtime = typed();
    let project = runtime
        .export_project_v3(runtime.current_revision())
        .unwrap();
    PROJECT.with(|slot| slot.borrow_mut().clone_from(&project.bytes));
    REQUEST.with(|slot| *slot.borrow_mut() = OCCURRENCE.to_vec());
    tachiko_designer_project_open_working_size();
    let mut host = DesignerHost::default();
    let reply = host.open_project_working_size(&project.bytes, OCCURRENCE);
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), reply);
    let reply: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    host
}

fn window(host: &mut DesignerHost, revision: &str) -> Value {
    ok(
        host,
        &json!({"type":"query_table_window", "expected_revision":revision,
        "collection":"sheet", "start":0, "count":10, "display":true}),
    )
}

fn promote(host: &mut DesignerHost, revision: &str) -> Value {
    ok(
        host,
        &json!({"type":"promote_first_row_to_headers",
        "expected_revision":revision, "collection":"sheet"}),
    )
}

#[test]
fn header_promotion_bridge_consumes_row_and_reports_blank_and_duplicate_fallbacks() {
    let mut host = setup();
    ok(
        &mut host,
        &json!({"type":"new_blank_workbook", "profile":"working_size",
        "occurrence_id":std::str::from_utf8(OCCURRENCE).unwrap()}),
    );
    let mut revision = "resident/0".to_owned();
    for (column, input) in [
        (1, ""),
        (2, "Column A"),
        (3, " Item "),
        (4, "Item"),
        (5, "Column A (2)"),
    ] {
        let publication = ok(
            &mut host,
            &json!({"type":"write_cell_at",
            "expected_revision":revision, "collection":"sheet", "row":1,
            "column":column, "input":{"kind":"text","value":input}}),
        );
        publication["resulting_revision"]
            .as_str()
            .unwrap()
            .clone_into(&mut revision);
    }
    let publication = ok(
        &mut host,
        &json!({"type":"write_cell_at",
        "expected_revision":revision, "collection":"sheet", "row":2,
        "column":1, "input":{"kind":"text","value":"Tea"}}),
    );
    publication["resulting_revision"]
        .as_str()
        .unwrap()
        .clone_into(&mut revision);
    let before = window(&mut host, &revision);
    let publication = promote(&mut host, &revision);
    let after = window(
        &mut host,
        publication["resulting_revision"].as_str().unwrap(),
    );
    assert_eq!(after["collection"]["headerless"], false);
    assert_eq!(after["collection"]["data_start_row"], 2);
    assert_eq!(after["total_rows"], 1);
    let mut remaining = before["rows"][1].clone();
    remaining["position"] = json!(1);
    assert_eq!(after["rows"][0], remaining);
    assert_eq!(after["columns"][0]["display_name"], "Column A (3)");
    assert_eq!(after["columns"][4]["display_name"], "Column A (2)");
    assert_eq!(after["columns"][2]["display_name"], "Item");
    assert_eq!(after["columns"][3]["display_name"], "Column D");
    assert_eq!(
        publication["header_fallbacks"],
        json!([
            {"field":after["columns"][0]["id"], "name":"Column A (3)", "reason":"blank"},
            {"field":after["columns"][3]["id"], "name":"Column D", "reason":"duplicate"}
        ])
    );
    tachiko_designer_project_close();
}

#[test]
fn header_promotion_bridge_preserves_declared_and_auto_types_and_tagged_values() {
    let mut host = setup();
    let before = window(&mut host, "resident/0");
    let publication = promote(&mut host, "resident/0");
    let after = window(
        &mut host,
        publication["resulting_revision"].as_str().unwrap(),
    );
    for (old, new) in before["columns"]
        .as_array()
        .unwrap()
        .iter()
        .zip(after["columns"].as_array().unwrap())
    {
        assert_eq!(old["field_type"], new["field_type"]);
        assert_eq!(old["key"], new["key"]);
        assert_eq!(old["id"], new["id"]);
    }
    assert_eq!(after["columns"][4]["field_type"], "auto");
    assert_eq!(
        after["rows"][0]["fields"][4]["stored"],
        json!({"kind":"text","value":"00123"})
    );
    let mut remaining = before["rows"][1].clone();
    remaining["position"] = json!(1);
    assert_eq!(after["rows"][0], remaining);
    tachiko_designer_project_close();
}

#[test]
fn header_promotion_bridge_refuses_already_headed_and_empty_without_changing_revision() {
    let mut host = setup();
    let publication = promote(&mut host, "resident/0");
    let revision = publication["resulting_revision"].as_str().unwrap();
    let before = window(&mut host, revision);
    let refused = exchange(
        &mut host,
        &json!({"type":"promote_first_row_to_headers",
        "expected_revision":revision,"collection":"sheet"}),
    );
    assert_eq!(refused["status"], "error");
    assert_eq!(refused["error"]["code"], "table_already_has_headers");
    assert_eq!(window(&mut host, revision), before);
    ok(
        &mut host,
        &json!({"type":"new_blank_workbook", "profile":"working_size",
        "occurrence_id":std::str::from_utf8(OCCURRENCE).unwrap()}),
    );
    let refused = exchange(
        &mut host,
        &json!({"type":"promote_first_row_to_headers",
        "expected_revision":"resident/0","collection":"sheet"}),
    );
    assert_eq!(refused["status"], "error");
    assert_eq!(refused["error"]["code"], "no_header_row");
    assert_eq!(window(&mut host, "resident/0")["total_rows"], 0);
    tachiko_designer_project_close();
}

fn save_reopen(host: &mut DesignerHost, revision: &str, portable: bool, before: &Value) {
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
    assert_eq!(bytes, native.bytes.unwrap());
    REQUEST.with(|slot| *slot.borrow_mut() = OCCURRENCE.to_vec());
    let native = if portable {
        tachiko_designer_portable_ro_open_working_size();
        host.open_portable_ro_working_size(&bytes, OCCURRENCE)
    } else {
        tachiko_designer_project_open_working_size();
        host.open_project_working_size(&bytes, OCCURRENCE)
    };
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), native);
    let after = window(host, "resident/0");
    for key in ["collection", "columns", "rows"] {
        assert_eq!(after[key], before[key]);
    }
}

#[test]
fn header_promotion_bridge_one_undo_redo_and_both_saved_carriers_preserve_state() {
    let mut host = setup();
    let before = window(&mut host, "resident/0");
    let publication = promote(&mut host, "resident/0");
    let promoted = window(
        &mut host,
        publication["resulting_revision"].as_str().unwrap(),
    );
    let undo = ok(
        &mut host,
        &json!({"type":"undo","expected_revision":publication["resulting_revision"]}),
    );
    let restored = window(&mut host, undo["resulting_revision"].as_str().unwrap());
    for key in ["collection", "columns", "rows"] {
        assert_eq!(restored[key], before[key]);
    }
    let redo = ok(
        &mut host,
        &json!({"type":"redo","expected_revision":undo["resulting_revision"]}),
    );
    save_reopen(
        &mut host,
        redo["resulting_revision"].as_str().unwrap(),
        false,
        &promoted,
    );
    save_reopen(&mut host, "resident/0", true, &promoted);
    tachiko_designer_project_close();
}

#[test]
fn header_promotion_bridge_keeps_direct_and_same_row_formula_bindings() {
    let mut host = setup();
    let before = window(&mut host, "resident/0");
    let publication = promote(&mut host, "resident/0");
    let after = window(
        &mut host,
        publication["resulting_revision"].as_str().unwrap(),
    );
    for column in [5, 6] {
        let prior = &before["rows"][1]["fields"][column];
        let current = &after["rows"][0]["fields"][column];
        assert_eq!(current["target"], prior["target"]);
        assert_eq!(current["formula"], prior["formula"]);
        assert_eq!(current["calculated"]["value"], 120.0);
    }
    tachiko_designer_project_close();
}

fn assert_bridge_refusal(native: crate::DesignerRuntime, wasm: crate::DesignerRuntime, code: &str) {
    // Inject retained failure states and a sidecar larger than the spreadsheet
    // request arena, then exercise the production ordinary-request byte paths.
    let document = native.session.document().clone();
    let imported = native
        .imported
        .as_ref()
        .map(|state| (state.metadata.clone(), state.lifecycle_changed));
    let assert_unchanged = |runtime: &crate::DesignerRuntime| {
        assert_eq!(runtime.current_revision(), "resident/0");
        assert_eq!(runtime.session.document(), &document);
        assert_eq!(
            runtime
                .imported
                .as_ref()
                .map(|state| (&state.metadata, state.lifecycle_changed)),
            imported
                .as_ref()
                .map(|(metadata, changed)| (metadata, *changed)),
        );
        assert!(runtime.undo.is_empty());
        assert!(runtime.redo.is_empty());
    };
    let request = serde_json::to_vec(&json!({
        "type":"promote_first_row_to_headers", "expected_revision":"resident/0",
        "collection":native.default_collection,
    }))
    .unwrap();
    let mut native = Some(native);
    RUNTIME.with(|slot| *slot.borrow_mut() = Some(wasm));
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&request));
    tachiko_designer_request_run();
    let reply = crate::process_wire_request(&mut native, &request);
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), reply);
    let reply: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(reply["status"], "error");
    assert_eq!(reply["error"]["code"], code);
    assert_eq!(reply["error"]["current_revision"], "resident/0");
    assert_unchanged(native.as_ref().unwrap());
    RUNTIME.with(|slot| assert_unchanged(slot.borrow().as_ref().unwrap()));
    tachiko_designer_project_close();
}

#[test]
fn header_promotion_bridge_refuses_failed_and_unavailable_formulas_atomically() {
    use crate::header_promotion_tests::formula_header;
    for failed in [true, false] {
        assert_bridge_refusal(
            formula_header(failed),
            formula_header(failed),
            "header_formula_unavailable",
        );
    }
}

#[test]
fn header_promotion_bridge_refuses_oversized_fallback_reply_atomically() {
    use crate::header_promotion_tests::near_limit_imported_header;
    assert_bridge_refusal(
        near_limit_imported_header(),
        near_limit_imported_header(),
        "query_too_large",
    );
}
