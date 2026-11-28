use super::{
    PROJECT, REQUEST, RESPONSE, tachiko_designer_project_close, tachiko_designer_spreadsheet_run,
};
use crate::host_bridge::DesignerHost;
use crate::row_rank_tests::imported_fixture;
use serde_json::json;

#[test]
fn ranked_imported_wasm_inspect_and_open_match_native_in_both_profiles() {
    let (runtime, metadata) = imported_fixture(false);
    let tree = tachiko_storage::encode_roproj_v3_with_presentation(
        runtime.session.document(),
        &runtime.presentation,
    )
    .unwrap();
    let bundle =
        crate::encode_project_bundle(tree.files().iter().map(|file| (file.path(), file.bytes())))
            .unwrap();
    for profile in [None, Some("working_size")] {
        for operation in ["inspect_project", "open_project"] {
            tachiko_designer_project_close();
            let mut input = json!({"type": operation, "profile": profile, "metadata": metadata});
            if operation == "open_project" {
                input["occurrence_id"] = json!("00000000-0000-4000-8000-000000000872");
            }
            let request = serde_json::to_vec(&input).unwrap();
            REQUEST.with(|slot| *slot.borrow_mut() = request.clone());
            PROJECT.with(|slot| *slot.borrow_mut() = bundle.clone());
            tachiko_designer_spreadsheet_run();
            let reply = RESPONSE.with(|slot| slot.borrow().clone());
            let decoded: serde_json::Value = serde_json::from_slice(&reply).unwrap();
            assert_eq!(decoded["status"], "ok", "{decoded}");
            assert_eq!(
                reply,
                DesignerHost::new().spreadsheet(&request, &bundle).reply
            );
        }
    }
    tachiko_designer_project_close();
}
