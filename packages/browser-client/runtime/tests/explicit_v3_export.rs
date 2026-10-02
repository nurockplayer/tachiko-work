use tachiko_designer_runtime::{DesignerRuntime, close_project, open_project};

#[test]
fn selected_export_keeps_live_origin_and_marks_only_successful_reopen() {
    let runtime = DesignerRuntime::moonfall("00000000-0000-4000-8000-000000000001").unwrap();
    let before = runtime.export_project("resident/0").unwrap();
    let selected = runtime.export_project_v3("resident/0").unwrap();
    assert_eq!(selected.revision, "resident/0");
    assert_eq!(
        runtime.export_project("resident/0").unwrap().bytes,
        before.bytes
    );
    assert_eq!(
        runtime
            .export_project_v3("resident/1")
            .unwrap_err()
            .failure_projection("resident/0")
            .code,
        "stale_revision"
    );
    let mut slot = Some(runtime);
    open_project(
        &mut slot,
        &selected.bytes,
        "00000000-0000-4000-8000-000000000002",
    )
    .unwrap();
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .export_project("resident/0")
            .unwrap_err()
            .failure_projection("resident/0")
            .code,
        "unsupported_project"
    );
    assert!(
        open_project(
            &mut slot,
            b"invalid",
            "00000000-0000-4000-8000-000000000003"
        )
        .is_err()
    );
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .export_project("resident/0")
            .unwrap_err()
            .failure_projection("resident/0")
            .code,
        "unsupported_project"
    );
    open_project(
        &mut slot,
        &before.bytes,
        "00000000-0000-4000-8000-000000000004",
    )
    .unwrap();
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .export_project("resident/0")
            .unwrap()
            .bytes,
        before.bytes
    );
    close_project(&mut slot);
    assert!(slot.is_none());
}
