//! Compact rectangles use the same native and WASM arena statistics path.
use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_request_run,
    tachiko_designer_spreadsheet_run,
};
use crate::host_bridge::DesignerHost;
use serde_json::{Value, json};

fn payload(bytes: &[u8]) -> Value {
    let reply: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"].clone()
}

fn command(host: &mut DesignerHost, input: &Value) -> Value {
    let bytes = serde_json::to_vec(input).unwrap();
    assert!(bytes.len() < 65_536);
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&bytes));
    tachiko_designer_request_run();
    let browser = RESPONSE.with(|slot| slot.borrow().clone());
    assert_eq!(browser, host.request(&bytes));
    payload(&browser)
}

fn imported(host: &mut DesignerHost, rows: usize, working: bool) -> Value {
    let csv = format!("Left,Right\n{}", "2,4\n".repeat(rows));
    let mut input = json!({
        "type": "import", "format": "csv", "csv_options": {"delimiter": ",", "header": true},
        "selection": {"column_types": [["number", "number"]], "extra_columns": [[]]},
        "occurrence_id": "00000000-0000-4000-8000-000000000839", "install": true,
    });
    if working {
        input["profile"] = "working_size".into();
    }
    let bytes = serde_json::to_vec(&input).unwrap();
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&bytes));
    PROJECT.with(|slot| *slot.borrow_mut() = csv.as_bytes().to_vec());
    tachiko_designer_spreadsheet_run();
    let browser = RESPONSE.with(|slot| slot.borrow().clone());
    assert_eq!(browser, host.spreadsheet(&bytes, csv.as_bytes()).reply);
    payload(&browser)["opened"].clone()
}

#[test]
fn range_statistics_imported_native_and_wasm_bridge_match() {
    for (rows, working) in [(32, false), (10_000, true)] {
        tachiko_designer_project_close();
        let mut host = DesignerHost::new();
        let opened = imported(&mut host, rows, working);
        let revision = &opened["bootstrap"]["revision"];
        let collection = &opened["bootstrap"]["default_collection"];
        let window = command(
            &mut host,
            &json!({
                "type": "query_table_window", "expected_revision": revision,
                "collection": collection, "start": 0, "count": 32,
            }),
        );
        let last = command(
            &mut host,
            &json!({
                "type": "query_table_window", "expected_revision": revision,
                "collection": collection, "start": rows - 1, "count": 1,
            }),
        );
        let columns = window["columns"].as_array().unwrap();
        let targets = window["rows"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| {
                columns
                    .iter()
                    .map(move |column| json!({"entity": row["id"], "field": column["id"]}))
            })
            .collect::<Vec<_>>();
        let expected = command(
            &mut host,
            &json!({
                "type": "query_selection_statistics", "expected_revision": revision, "targets": targets,
            }),
        );
        let mut range = json!({"collection": collection,
            "first": window["rows"][31]["id"], "last": window["rows"][0]["id"],
            "fields": columns.iter().map(|column| &column["id"]).collect::<Vec<_>>()});
        let small = command(
            &mut host,
            &json!({
                "type": "query_selection_statistics", "expected_revision": revision, "range": range,
            }),
        );
        assert_eq!(small, expected);
        range["first"].clone_from(&last["rows"][0]["id"]);
        let all = command(
            &mut host,
            &json!({
                "type": "query_selection_statistics", "expected_revision": revision, "range": range,
            }),
        );
        assert_eq!(all["count"], rows * 2);
        assert_eq!(all["numerical_count"], rows * 2);
        assert_eq!(
            all["sum"].as_f64(),
            Some(f64::from(u32::try_from(rows * 6).unwrap()))
        );
        assert_eq!(all["average"], 3.0);
        tachiko_designer_project_close();
    }
}
