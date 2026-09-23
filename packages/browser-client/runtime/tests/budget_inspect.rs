use tachiko_designer_runtime::{DesignerRequest, DesignerRuntime, inspect_project, open_project};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000000";

#[test]
fn candidate_inspection_preserves_resident_revision_bytes_and_both_history_directions() {
    let mut resident = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    resident
        .handle(DesignerRequest::PasteCells {
            expected_revision: "resident/0".into(),
            collection: "tracker".into(),
            start_entity: None,
            start_field: "task".into(),
            rows: vec![vec![
                "Unsaved accepted work".into(),
                "3".into(),
                "false".into(),
            ]],
        })
        .unwrap();
    let before = resident.export_project("resident/1").unwrap().bytes;
    let candidate = DesignerRuntime::budget("00000000-0000-4000-8000-000000000001")
        .unwrap()
        .export_project("resident/0")
        .unwrap()
        .bytes;
    let inspected = inspect_project(&candidate).unwrap();
    assert_eq!(inspected.bootstrap.title, "Monthly Budget");
    assert_eq!(inspected.bootstrap.collections.len(), 2);
    assert_eq!(resident.export_project("resident/1").unwrap().bytes, before);
    assert!(inspect_project(b"invalid project").is_err());
    resident
        .handle(DesignerRequest::Undo {
            expected_revision: "resident/1".into(),
        })
        .unwrap();
    inspect_project(&candidate).unwrap();
    resident
        .handle(DesignerRequest::Redo {
            expected_revision: "resident/2".into(),
        })
        .unwrap();
    assert_eq!(resident.export_project("resident/3").unwrap().bytes, before);
    let mut accepted = Some(resident);
    let opened = open_project(
        &mut accepted,
        &candidate,
        "00000000-0000-4000-8000-000000000002",
    )
    .unwrap();
    assert_eq!(opened, inspected);
}
