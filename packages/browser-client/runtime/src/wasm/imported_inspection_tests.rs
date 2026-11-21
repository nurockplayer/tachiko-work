use serde_json::{Value, json};

use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_request_run,
    tachiko_designer_spreadsheet_run,
};
use crate::imported_lifecycle_tests::{fixture, selection};
use crate::interop_adapter::{export_xlsx, import_xlsx};

fn reply_payload() -> Value {
    let reply: Value = RESPONSE.with(|slot| serde_json::from_slice(&slot.borrow()).unwrap());
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"].clone()
}

fn command(input: &Value) -> Value {
    REQUEST.with(|slot| *slot.borrow_mut() = serde_json::to_vec(input).unwrap());
    tachiko_designer_request_run();
    reply_payload()
}

fn spreadsheet(input: &Value, bytes: Vec<u8>) -> Value {
    REQUEST.with(|slot| *slot.borrow_mut() = serde_json::to_vec(input).unwrap());
    PROJECT.with(|slot| *slot.borrow_mut() = bytes);
    tachiko_designer_spreadsheet_run();
    reply_payload()
}

#[test]
fn wasm_inspection_recovers_rename_and_undo_mappings_for_export() {
    for working in [false, true] {
        tachiko_designer_project_close();
        let source = export_xlsx(&fixture(false)).unwrap();
        let mut input = json!({
            "type": "import", "format": "xlsx", "csv_options": {"delimiter": ",", "header": true},
            "selection": selection(), "install": true,
            "occurrence_id": "00000000-0000-4000-8000-000000000870",
        });
        if working {
            input["profile"] = json!("working_size");
        }
        spreadsheet(&input, source);
        assert_eq!(
            command(&json!({"type": "inspect_project"})),
            json!({"revision": "resident/0"})
        );
        let mut published = command(&json!({
            "type": "rename_table", "expected_revision": "resident/0",
            "collection": "sheet_1", "name": "Renamed Input",
        }));
        for name in ["Renamed Input", "Input"] {
            if name == "Input" {
                published = command(&json!({"type": "undo", "expected_revision": "resident/1"}));
            }
            // The caller recovers without supplying the lifecycle reply's mapping.
            let inspected = command(&json!({"type": "inspect_project"}));
            assert_eq!(inspected["import_metadata"], published["import_metadata"]);
            let exported = spreadsheet(
                &json!({
                    "type": "export", "expected_revision": inspected["revision"],
                    "metadata": inspected["import_metadata"], "format": "xlsx", "collection": "sheet_1",
                }),
                Vec::new(),
            );
            let bytes = PROJECT.with(|slot| slot.borrow().clone());
            assert_eq!(exported["byte_length"], bytes.len());
            let workbook = import_xlsx(&bytes).unwrap();
            assert_eq!(workbook.sheets.len(), 2);
            assert_eq!(workbook.sheets[0].name, name);
        }
        tachiko_designer_project_close();
    }
}
