use serde_json::{Value, json};
use tachiko_workspace_engine::BorderSide;

use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_project_open,
    tachiko_designer_request_run, tachiko_designer_spreadsheet_run,
};
use crate::cell_style_tests::fixture;
use crate::host_bridge::DesignerHost;

#[test]
fn border_style_wasm_bridge_matches_native_projection_history_and_xlsx_bytes() {
    tachiko_designer_project_close();
    let occurrence = b"00000000-0000-4000-8000-000000000889";
    let tree = tachiko_storage::encode_roproj_v3(&fixture()).unwrap();
    let bundle =
        crate::encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes())))
            .unwrap();
    let mut host = DesignerHost::new();
    REQUEST.with(|slot| *slot.borrow_mut() = occurrence.to_vec());
    PROJECT.with(|slot| slot.borrow_mut().clone_from(&bundle));
    tachiko_designer_project_open();
    assert_eq!(
        RESPONSE.with(|slot| slot.borrow().clone()),
        host.open_project(&bundle, occurrence)
    );
    for operation in [
        json!({"type":"set_cell_style", "expected_revision":"resident/0", "collection":"rows",
            "entities":["r1", "r2", "r3"], "fields":["b", "c"], "changes":[
                {"kind":"border", "target":"outline", "value":true},
                {"kind":"bold", "value":true}, {"kind":"italic", "value":true},
                {"kind":"fill", "color":"#FFFF00"}]}),
        json!({"type":"undo", "expected_revision":"resident/1"}),
        json!({"type":"redo", "expected_revision":"resident/2"}),
        json!({"type":"query_table_window", "expected_revision":"resident/3", "collection":"rows",
            "view":{}, "start":0, "count":3}),
    ] {
        let bytes = serde_json::to_vec(&operation).unwrap();
        REQUEST.with(|slot| slot.borrow_mut().clone_from(&bytes));
        tachiko_designer_request_run();
        let browser = RESPONSE.with(|slot| slot.borrow().clone());
        assert_eq!(browser, host.request(&bytes));
        let reply: Value = serde_json::from_slice(&browser).unwrap();
        assert_eq!(reply["status"], "ok", "{reply}");
    }
    let reply: Value = RESPONSE.with(|slot| serde_json::from_slice(&slot.borrow()).unwrap());
    assert_eq!(
        reply["response"]["payload"]["rows"][1]["styles"][1]["border_sides"],
        json!(["left"])
    );
    let operation = serde_json::to_vec(&json!({"type":"export_native_tables",
        "expected_revision":"resident/3", "collection":"rows", "format":"xlsx"}))
    .unwrap();
    REQUEST.with(|slot| slot.borrow_mut().clone_from(&operation));
    PROJECT.with(|slot| slot.borrow_mut().clear());
    tachiko_designer_spreadsheet_run();
    let native = host.spreadsheet(&operation, &[]);
    assert_eq!(RESPONSE.with(|slot| slot.borrow().clone()), native.reply);
    let browser = PROJECT.with(|slot| slot.borrow().clone());
    assert_eq!(browser, native.bytes.unwrap());
    let result = crate::interop_adapter::import_xlsx(&browser).unwrap();
    let style = &result.sheets[0].rows[1][1].style;
    assert!(style.bold && style.italic && style.border);
    assert_eq!(style.border_sides, [BorderSide::Left]);
    assert!(style.fill.as_deref().unwrap().ends_with("FFFF00"));
    tachiko_designer_project_close();
}
