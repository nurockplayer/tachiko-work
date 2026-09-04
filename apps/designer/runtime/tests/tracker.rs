use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, DesignerWireReply, FieldTarget,
    ScalarEditInput, close_project, open_project, process_wire_request,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000001";
fn table(runtime: &mut DesignerRuntime) -> tachiko_designer_runtime::TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "tracker".to_owned(),
        })
        .unwrap()
    else {
        panic!("table")
    };
    table
}
fn paste(
    runtime: &mut DesignerRuntime,
    rows: Vec<Vec<String>>,
) -> Result<DesignerResponse, tachiko_designer_runtime::DesignerError> {
    let revision = table(runtime).revision;
    runtime.handle(DesignerRequest::PasteCells {
        expected_revision: revision,
        collection: "tracker".to_owned(),
        start_entity: None,
        start_field: "task".to_owned(),
        rows,
    })
}
#[test]
fn tracker_dozen_row_paste_history_and_durability_preserve_semantic_identity() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let rows = (0..40)
        .map(|index| {
            vec![
                format!("Shipment {index}"),
                index.to_string(),
                "false".to_owned(),
            ]
        })
        .collect();
    paste(&mut runtime, rows).unwrap();
    let pasted = table(&mut runtime);
    assert_eq!(pasted.rows.len(), 40);
    assert_eq!(pasted.revision, "resident/1");
    assert_eq!(
        pasted
            .columns
            .iter()
            .map(|column| column.id.as_str())
            .collect::<Vec<_>>(),
        vec!["task", "estimate", "done"]
    );
    assert_eq!(
        pasted.columns[2].dropdown_options,
        Some(vec!["true".to_owned(), "false".to_owned()])
    );
    let id = pasted.rows[3].id.clone();
    runtime
        .handle(DesignerRequest::RemoveRows {
            expected_revision: pasted.revision,
            entities: vec![id.clone()],
        })
        .unwrap();
    assert_eq!(table(&mut runtime).rows.len(), 39);
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(table(&mut runtime).rows, pasted.rows);
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: revision,
        })
        .unwrap();
    assert!(!table(&mut runtime).rows.iter().any(|row| row.id == id));
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .unwrap();
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target: FieldTarget {
                entity: id.clone(),
                field: "done".to_owned(),
            },
            input: ScalarEditInput::Boolean { value: true },
        })
        .unwrap();
    let before = table(&mut runtime);
    let export = runtime.export_project(&before.revision).unwrap();
    let mut slot = Some(runtime);
    close_project(&mut slot);
    let reopened = open_project(&mut slot, &export.bytes, OCCURRENCE).unwrap();
    assert_eq!(reopened.table.rows, before.rows);
    slot.as_mut()
        .unwrap()
        .handle(DesignerRequest::EditScalar {
            expected_revision: reopened.table.revision,
            target: FieldTarget {
                entity: id,
                field: "estimate".to_owned(),
            },
            input: ScalarEditInput::Number {
                input: "12.5".to_owned(),
            },
        })
        .unwrap();
}

#[test]
fn rejected_atomic_paste_keeps_state_revision_and_history() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    paste(
        &mut runtime,
        vec![vec![
            "Accepted".to_owned(),
            "2".to_owned(),
            "false".to_owned(),
        ]],
    )
    .unwrap();
    let before = table(&mut runtime);
    for rows in [
        vec![
            vec!["Valid".to_owned(), "3".to_owned(), "true".to_owned()],
            vec!["Invalid".to_owned(), "4".to_owned(), "yes".to_owned()],
        ],
        vec![vec![
            "Invalid".to_owned(),
            "NaN".to_owned(),
            "false".to_owned(),
        ]],
        vec![vec!["x".repeat(65536), "0".to_owned(), "false".to_owned()]],
    ] {
        assert!(paste(&mut runtime, rows).is_err());
        assert_eq!(table(&mut runtime), before);
    }
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: before.revision,
        })
        .unwrap();
    assert!(table(&mut runtime).rows.is_empty());
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(table(&mut runtime).rows, before.rows);
}

#[test]
fn new_tracker_wire_is_candidate_first_and_opens_without_fixture() {
    let mut runtime = None;
    let bytes = process_wire_request(
        &mut runtime,
        format!(r#"{{"type":"new_tracker","occurrence_id":"{OCCURRENCE}"}}"#).as_bytes(),
    );
    let reply: DesignerWireReply = serde_json::from_slice(&bytes).unwrap();
    assert!(matches!(
        reply,
        DesignerWireReply::Ok {
            response: DesignerResponse::Opened(_)
        }
    ));
    let before = table(runtime.as_mut().unwrap());
    let bytes = process_wire_request(
        &mut runtime,
        br#"{"type":"new_tracker","occurrence_id":"invalid"}"#,
    );
    assert!(matches!(
        serde_json::from_slice::<DesignerWireReply>(&bytes).unwrap(),
        DesignerWireReply::Error { .. }
    ));
    assert_eq!(table(runtime.as_mut().unwrap()), before);
}

#[test]
fn multi_cell_edit_is_atomic_and_new_history_branch_keeps_row_ids_distinct() {
    use tachiko_designer_runtime::CellEdit;
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    paste(
        &mut runtime,
        vec![vec!["Keep".to_owned(), "2".to_owned(), "false".to_owned()]],
    )
    .unwrap();
    let original = table(&mut runtime);
    let id = original.rows[0].id.clone();
    let edit = |field: &str, input| CellEdit {
        target: FieldTarget {
            entity: id.clone(),
            field: field.to_owned(),
        },
        input,
    };
    let good = edit(
        "task",
        ScalarEditInput::Text {
            value: "Changed".to_owned(),
        },
    );
    let invalid = edit(
        "estimate",
        ScalarEditInput::Number {
            input: "infinity".to_owned(),
        },
    );
    assert!(
        runtime
            .handle(DesignerRequest::EditCells {
                expected_revision: original.revision.clone(),
                edits: vec![good.clone(), invalid]
            })
            .is_err()
    );
    assert_eq!(table(&mut runtime), original);
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: original.revision,
            edits: vec![good, edit("done", ScalarEditInput::Boolean { value: true })],
        })
        .unwrap();
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(table(&mut runtime).rows, original.rows);
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::RemoveRows {
            expected_revision: revision,
            entities: vec![id.clone()],
        })
        .unwrap();
    let revision = table(&mut runtime).revision;
    runtime
        .handle(DesignerRequest::AppendRow {
            expected_revision: revision,
            collection: "tracker".to_owned(),
        })
        .unwrap();
    let latest = table(&mut runtime);
    assert_ne!(latest.rows[0].id, id);
    assert!(
        runtime
            .handle(DesignerRequest::Redo {
                expected_revision: latest.revision
            })
            .is_err()
    );
}

#[test]
fn paste_over_blank_row_skips_unchanged_cells_without_losing_atomic_changes() {
    use tachiko_designer_runtime::CellEdit;
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    runtime
        .handle(DesignerRequest::AppendRow {
            expected_revision: "resident/0".to_owned(),
            collection: "tracker".to_owned(),
        })
        .unwrap();
    let before = table(&mut runtime);
    let id = before.rows[0].id.clone();
    runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: before.revision,
            collection: "tracker".to_owned(),
            start_entity: Some(id.clone()),
            start_field: "task".to_owned(),
            rows: vec![
                vec!["Changed".to_owned(), "0".to_owned(), "false".to_owned()],
                vec!["Appended".to_owned(), "3".to_owned(), "false".to_owned()],
            ],
        })
        .unwrap();
    let pasted = table(&mut runtime);
    assert_eq!(pasted.rows.len(), 2);
    assert_eq!(pasted.rows[0].id, id);
    assert_eq!(pasted.revision, "resident/2");
    let edit = |field: &str, input| CellEdit {
        target: FieldTarget {
            entity: id.clone(),
            field: field.to_owned(),
        },
        input,
    };
    runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: pasted.revision,
            edits: vec![
                edit(
                    "task",
                    ScalarEditInput::Text {
                        value: "Changed".to_owned(),
                    },
                ),
                edit("done", ScalarEditInput::Boolean { value: true }),
            ],
        })
        .unwrap();
    let edited = table(&mut runtime);
    assert_eq!(edited.revision, "resident/3");
    assert!(
        runtime
            .handle(DesignerRequest::EditCells {
                expected_revision: edited.revision.clone(),
                edits: vec![edit("done", ScalarEditInput::Boolean { value: true })]
            })
            .is_err()
    );
    assert_eq!(table(&mut runtime), edited);
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: edited.revision,
        })
        .unwrap();
    assert_eq!(table(&mut runtime).rows, pasted.rows);
}

#[test]
fn deleted_persisted_highest_row_identity_is_not_reused_after_reopen() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    paste(
        &mut runtime,
        vec![
            vec!["Keep".to_owned(), "1".to_owned(), "false".to_owned()],
            vec!["Remove".to_owned(), "2".to_owned(), "false".to_owned()],
        ],
    )
    .unwrap();
    let saved = table(&mut runtime);
    let original_id = saved.rows[1].id.clone();
    let durable = runtime.export_project(&saved.revision).unwrap();
    let mut slot = None;
    let opened = open_project(
        &mut slot,
        &durable.bytes,
        "00000000-0000-4000-8000-000000000002",
    )
    .unwrap();
    let runtime = slot.as_mut().unwrap();
    runtime
        .handle(DesignerRequest::RemoveRows {
            expected_revision: opened.table.revision,
            entities: vec![original_id.clone()],
        })
        .unwrap();
    let removed = table(runtime);
    let durable = runtime.export_project(&removed.revision).unwrap();
    close_project(&mut slot);
    let reopened = open_project(
        &mut slot,
        &durable.bytes,
        "00000000-0000-4000-8000-000000000003",
    )
    .unwrap();
    let runtime = slot.as_mut().unwrap();
    runtime
        .handle(DesignerRequest::AppendRow {
            expected_revision: reopened.table.revision,
            collection: "tracker".to_owned(),
        })
        .unwrap();
    let appended = table(runtime);
    assert_eq!(appended.rows.len(), 2);
    assert_eq!(appended.rows[0].id, saved.rows[0].id);
    assert_ne!(appended.rows[1].id, original_id);
    assert!(
        appended.rows[1]
            .id
            .ends_with("00000000-0000-4000-8000-000000000003")
    );
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: appended.revision,
        })
        .unwrap();
    let revision = table(runtime).revision;
    runtime
        .handle(DesignerRequest::Redo {
            expected_revision: revision,
        })
        .unwrap();
    assert_eq!(table(runtime).rows, appended.rows);
}
