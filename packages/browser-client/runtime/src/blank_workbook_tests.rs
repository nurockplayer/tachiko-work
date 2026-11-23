use crate::{
    DesignerRequest, DesignerResponse, DesignerRuntime, ScalarEditInput, ScalarKind,
    StoredValueProjection, TableView, blank_workbook, open_portable_ro_working_size,
    open_project_working_size,
};

fn blank() -> DesignerRuntime {
    blank_workbook::candidate("00000000-0000-4000-8000-000000000899")
        .unwrap()
        .0
}

fn write(runtime: &mut DesignerRuntime, row: usize, column: usize, text: &str) {
    let revision = runtime.current_revision().to_owned();
    let response = runtime
        .handle(DesignerRequest::WriteCellAt {
            expected_revision: revision.clone(),
            collection: "sheet".into(),
            row,
            column,
            input: ScalarEditInput::Auto {
                input: text.into(),
                date_order: None,
            },
        })
        .unwrap();
    let DesignerResponse::Published(publication) = response else {
        panic!("published expected")
    };
    assert_eq!(publication.base_revision, revision);
    assert_ne!(publication.resulting_revision, revision);
}

fn window(runtime: &mut DesignerRuntime) -> crate::TableWindowProjection {
    let revision = runtime.current_revision().to_owned();
    runtime
        .query_table_window(&revision, "sheet", &TableView::default(), 0, 10, true)
        .unwrap()
}

fn filled() -> DesignerRuntime {
    let mut runtime = blank();
    for (row, column, input) in [
        (1, 1, "Item"),
        (1, 2, "Price"),
        (2, 1, "Tea"),
        (2, 2, "120"),
    ] {
        write(&mut runtime, row, column, input);
    }
    runtime
}

#[test]
fn delete_then_expand_preserves_surviving_names_and_generates_unique_labels() {
    for (count, deleted, row, column, names) in [
        (2, 0, 1, 2, vec!["Column B", "Column B (2)"]),
        (2, 1, 1, 2, vec!["Column A", "Column B"]),
        (
            3,
            0,
            40,
            6,
            vec![
                "Column B",
                "Column C",
                "Column C (2)",
                "Column D",
                "Column E",
                "Column F",
            ],
        ),
        (
            3,
            1,
            40,
            6,
            vec![
                "Column A",
                "Column C",
                "Column C (2)",
                "Column D",
                "Column E",
                "Column F",
            ],
        ),
        (
            3,
            2,
            40,
            6,
            vec![
                "Column A", "Column B", "Column C", "Column D", "Column E", "Column F",
            ],
        ),
    ] {
        let mut runtime = blank();
        write(&mut runtime, 1, count, "survivor");
        let before = window(&mut runtime);
        runtime
            .remove_column(&before.revision, "sheet", &before.columns[deleted].id)
            .unwrap();
        let retained = window(&mut runtime).columns;
        write(&mut runtime, row, column, "919");
        let after = window(&mut runtime);
        assert_eq!(&after.columns[..retained.len()], retained.as_slice());
        assert_eq!(
            after
                .columns
                .iter()
                .map(|c| c.display_name.as_str())
                .collect::<Vec<_>>(),
            names
        );
        assert_eq!(after.total_rows, row);
        let written = &runtime.session.document().entities
            [&runtime.collection_specs["sheet"].entities[row - 1]];
        assert_eq!(
            written.fields[after.columns[column - 1].id.as_str()],
            crate::Value::Number(919.0.try_into().unwrap())
        );
    }
}

#[test]
fn blank_workbook_clear_unallocated_cells_is_no_change_and_write_still_undoes() {
    let mut runtime = blank();
    let blank_document = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    let error = runtime
        .handle(DesignerRequest::WriteCellAt {
            expected_revision: revision.clone(),
            collection: "sheet".into(),
            row: 1,
            column: 1,
            input: ScalarEditInput::Clear,
        })
        .unwrap_err();
    assert_eq!(error.generic_failure().0, "no_change");
    assert_eq!(runtime.session.document(), &blank_document);
    assert_eq!(runtime.current_revision(), revision);
    assert!(runtime.undo.is_empty());
    assert!(runtime.redo.is_empty());
    let table = window(&mut runtime);
    assert_eq!(table.total_rows, 0);
    assert_eq!(table.rows.len(), 0);
    assert_eq!(table.columns.len(), 0);

    write(&mut runtime, 1, 1, "Tea");
    let written = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    for (row, column) in [(1, 2), (2, 1), (2, 2)] {
        let error = runtime
            .write_cell_at(&revision, "sheet", row, column, &ScalarEditInput::Clear)
            .unwrap_err();
        assert_eq!(error.generic_failure().0, "no_change");
        assert_eq!(runtime.session.document(), &written);
        assert_eq!(runtime.current_revision(), revision);
        assert_eq!(runtime.undo.len(), 1);
    }
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.session.document(), &blank_document);
    assert_eq!(window(&mut runtime).columns.len(), 0);
    assert_eq!(window(&mut runtime).rows.len(), 0);
}

#[test]
fn required_number_clear_unallocated_cell_does_not_create_a_record() {
    let mut runtime = DesignerRuntime::new_working_size_table(
        "00000000-0000-4000-8000-000000000899",
        "Numbers",
        &[crate::NewTableColumnInput {
            name: "Amount".into(),
            field_type: "number".into(),
            required: true,
        }],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let document = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    let error = runtime
        .write_cell_at(&revision, &collection, 1, 1, &ScalarEditInput::Clear)
        .unwrap_err();
    assert_eq!(error.generic_failure().0, "no_change");
    assert_eq!(runtime.session.document(), &document);
    assert!(runtime.session.document().entities.is_empty());
    assert_eq!(runtime.current_revision(), revision);
    assert!(runtime.undo.is_empty());
    assert!(runtime.redo.is_empty());

    runtime
        .write_cell_at(
            &revision,
            &collection,
            1,
            1,
            &ScalarEditInput::Number { input: "7".into() },
        )
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    let table = runtime
        .query_table_window(&revision, &collection, &TableView::default(), 0, 10, true)
        .unwrap();
    assert_eq!(table.total_rows, 1);
    assert_eq!(
        table.rows[0].fields[0].stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert_eq!(runtime.undo.len(), 1);
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.session.document(), &document);
}

#[test]
fn blank_workbook_first_adjacent_writes_are_tagged_headerless_and_undo_exactly() {
    let mut runtime = blank();
    let opened = runtime.opened_window().unwrap();
    assert!(opened.window.collection.headerless);
    assert_eq!(opened.window.collection.data_start_row, 1);
    assert_eq!(opened.window.columns, []);
    assert_eq!(opened.window.rows, []);
    let mut states = vec![runtime.session.document().clone()];
    for (row, column, input) in [
        (1, 1, "Item"),
        (1, 2, "Price"),
        (2, 1, "Tea"),
        (2, 2, "120"),
    ] {
        write(&mut runtime, row, column, input);
        states.push(runtime.session.document().clone());
    }
    let table = window(&mut runtime);
    assert_eq!(
        table
            .columns
            .iter()
            .map(|column| column.display_name.as_str())
            .collect::<Vec<_>>(),
        ["Column A", "Column B"]
    );
    assert!(
        table
            .columns
            .iter()
            .all(|column| column.field_type == "auto")
    );
    assert_eq!(
        table.rows[0].fields[0].stored,
        Some(StoredValueProjection::Text {
            value: "Item".into()
        })
    );
    assert_eq!(
        table.rows[0].fields[1].stored,
        Some(StoredValueProjection::Text {
            value: "Price".into()
        })
    );
    assert_eq!(
        table.rows[1].fields[0].stored,
        Some(StoredValueProjection::Text {
            value: "Tea".into()
        })
    );
    assert_eq!(
        table.rows[1].fields[1].stored,
        Some(StoredValueProjection::Number { value: 120.0 })
    );
    assert!(
        table
            .rows
            .iter()
            .flat_map(|row| &row.fields)
            .all(|field| field.editable_scalar == Some(ScalarKind::Auto))
    );
    assert_eq!(runtime.undo.len(), 4);
    for state in states[..4].iter().rev() {
        let revision = runtime.current_revision().to_owned();
        runtime.history_edit(&revision, false).unwrap();
        assert_eq!(runtime.session.document(), state);
    }
    for state in &states[1..] {
        let revision = runtime.current_revision().to_owned();
        runtime.history_edit(&revision, true).unwrap();
        assert_eq!(runtime.session.document(), state);
    }
    assert_eq!(window(&mut runtime).rows, table.rows);
}

#[test]
fn blank_workbook_positional_append_stays_below_surviving_row_after_reopen() {
    let mut runtime = blank();
    write(&mut runtime, 1, 1, "Deleted");
    write(&mut runtime, 2, 1, "Survivor");
    let first_row = window(&mut runtime).rows[0].id.clone();
    let revision = runtime.current_revision().to_owned();
    runtime
        .remove_table_rows(&revision, "sheet", &[first_row])
        .unwrap();
    let bytes = runtime
        .export_project_v3(runtime.current_revision())
        .unwrap()
        .bytes;
    let mut reopened = None;
    open_project_working_size(
        &mut reopened,
        &bytes,
        "00000000-0000-4000-8000-000000000900",
    )
    .unwrap();
    let mut runtime = reopened.unwrap();
    write(&mut runtime, 2, 1, "Appended");
    let table = window(&mut runtime);
    assert_eq!(table.total_rows, 2);
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| row.fields[0].stored.clone())
            .collect::<Vec<_>>(),
        vec![
            Some(StoredValueProjection::Text {
                value: "Survivor".into()
            }),
            Some(StoredValueProjection::Text {
                value: "Appended".into()
            }),
        ]
    );
}

#[test]
fn blank_workbook_v3_and_portable_reopen_preserve_schema_labels_and_cells() {
    for mut runtime in [blank(), filled()] {
        let table = window(&mut runtime);
        let document = runtime.session.document().clone();
        let revision = runtime.current_revision().to_owned();
        let v3 = runtime.export_project_v3(&revision).unwrap();
        let portable = runtime.export_portable_ro(&revision).unwrap();
        for (portable, bytes) in [(false, v3.bytes), (true, portable.bytes)] {
            let mut reopened = None;
            let occurrence = "00000000-0000-4000-8000-000000000900";
            if portable {
                open_portable_ro_working_size(&mut reopened, &bytes, occurrence).unwrap();
            } else {
                open_project_working_size(&mut reopened, &bytes, occurrence).unwrap();
            }
            let mut reopened = reopened.unwrap();
            assert_eq!(reopened.session.document(), &document);
            let after = window(&mut reopened);
            assert_eq!(after.collection, table.collection);
            assert_eq!(after.columns, table.columns);
            assert_eq!(after.rows, table.rows);
        }
    }
}

#[test]
fn blank_workbook_zero_stale_and_formula_writes_refuse_without_publication() {
    let mut runtime = filled();
    write(&mut runtime, 3, 1, "Coffee");
    let before = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    let history = runtime.undo.len();
    for (row, column, input) in [(0, 1, "1"), (1, 0, "1"), (4, 3, "=1+1")] {
        let error = runtime
            .write_cell_at(
                &revision,
                "sheet",
                row,
                column,
                &ScalarEditInput::Auto {
                    input: input.into(),
                    date_order: None,
                },
            )
            .unwrap_err();
        if input.starts_with('=') {
            assert!(error.to_string().contains("Auto formulas are unsupported"));
        }
        assert_eq!(runtime.session.document(), &before);
        assert_eq!(runtime.current_revision(), revision);
        assert_eq!(runtime.undo.len(), history);
    }
    assert!(
        runtime
            .write_cell_at(
                "resident/0",
                "sheet",
                1,
                1,
                &ScalarEditInput::Auto {
                    input: "stale".into(),
                    date_order: None
                }
            )
            .is_err()
    );
    assert_eq!(runtime.session.document(), &before);
}

#[test]
fn blank_workbook_generated_columns_and_rows_are_one_atomic_formatted_input() {
    let mut runtime = blank();
    write(&mut runtime, 1, 1, "$1,200.00");
    let presentation = runtime.presentation.clone();
    assert_eq!(
        window(&mut runtime).rows[0].fields[0].display.as_deref(),
        Some("$1,200.00")
    );
    runtime.history_edit("resident/1", false).unwrap();
    assert_eq!(window(&mut runtime).columns, []);
    assert_eq!(runtime.presentation.cells.len(), 0);
    runtime.history_edit("resident/2", true).unwrap();
    assert_eq!(runtime.presentation, presentation);
    write(&mut runtime, 2, 2, "00123");
    assert_eq!(
        window(&mut runtime).rows[1].fields[0].stored,
        Some(StoredValueProjection::Text {
            value: "00123".into()
        })
    );
    // Generated labels continue past Z within the existing 32-field profile.
    for column in 3..=27 {
        write(&mut runtime, 1, column, "x");
    }
    assert_eq!(window(&mut runtime).columns[26].display_name, "Column AA");
}

#[test]
fn blank_workbook_written_cells_and_headerless_state_survive_delta_and_merge() {
    use tachiko_workspace_engine::{
        CanonicalDirectFact, WorkspaceMergeOutcomeV2, canonical_delta, merge_documents_v2,
    };
    let base = blank().session.document().clone();
    let written = filled().session.document().clone();
    let delta = canonical_delta("tachiko.semantic-delta/v2", &base, &written).unwrap();
    assert!(
        delta
            .facts()
            .iter()
            .any(|fact| matches!(fact, CanonicalDirectFact::EntityCreated { .. }))
    );
    let mut other = base.clone();
    other.title = "Renamed workbook".into();
    let WorkspaceMergeOutcomeV2::Merged(merged) =
        merge_documents_v2("tachiko.semantic-conflict/v2", &base, &written, &other).unwrap()
    else {
        panic!("independent writes and title must merge")
    };
    assert_eq!(merged.document.schemas, written.schemas);
    assert_eq!(merged.document.entities, written.entities);
    assert_eq!(merged.document.title, other.title);
    assert!(
        merged
            .document
            .schemas
            .values()
            .all(|schema| schema.headerless)
    );
}

fn distant() -> DesignerRuntime {
    let mut runtime = filled();
    write(&mut runtime, 3, 1, "Coffee");
    write(&mut runtime, 40, 6, "923");
    runtime
}

#[test]
fn distant_write_creates_sparse_rows_and_generated_columns() {
    let mut runtime = distant();
    let revision = runtime.current_revision().to_owned();
    let table = runtime
        .query_table_window(&revision, "sheet", &TableView::default(), 0, 40, true)
        .unwrap();
    assert_eq!(table.total_rows, 40);
    assert_eq!(table.columns.len(), 6);
    for (column, letter) in table.columns.iter().zip('A'..='F') {
        assert_eq!(column.display_name, format!("Column {letter}"));
        assert_eq!(column.field_type, "auto");
    }
    assert_eq!(table.rows[39].position, Some(40));
    assert_eq!(
        table.rows[39]
            .fields
            .iter()
            .find(|field| field.target.field == table.columns[5].id)
            .unwrap()
            .stored,
        Some(StoredValueProjection::Number { value: 923.0 })
    );
    let column_ids = table
        .columns
        .iter()
        .map(|column| column.id.clone())
        .collect::<Vec<_>>();
    assert!(
        table.rows[3..39]
            .iter()
            .all(|row| row.fields.is_empty() && row.blank_fields == column_ids)
    );
    assert!(table.rows[..3].iter().all(|row| {
        column_ids[2..]
            .iter()
            .all(|id| row.blank_fields.contains(id))
    }));
    assert_eq!(
        runtime
            .session
            .document()
            .entities
            .values()
            .map(|entity| entity.fields.len())
            .sum::<usize>(),
        6
    );
}

#[test]
fn distant_write_undo_redo_restores_the_entire_structural_edit() {
    let mut runtime = filled();
    let before = runtime.session.document().clone();
    let presentation = runtime.presentation.clone();
    let history = runtime.undo.len();
    write(&mut runtime, 40, 6, "$923.00");
    let after = runtime.session.document().clone();
    let after_presentation = runtime.presentation.clone();
    assert_eq!(runtime.undo.len(), history + 1);
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.session.document(), &before);
    assert_eq!(runtime.presentation, presentation);
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, true).unwrap();
    assert_eq!(runtime.session.document(), &after);
    assert_eq!(runtime.presentation, after_presentation);
}

#[test]
fn distant_write_v3_and_portable_reopen_preserve_positions() {
    let mut runtime = distant();
    let document = runtime.session.document().clone();
    let revision = runtime.current_revision().to_owned();
    let before = runtime
        .query_table_window(&revision, "sheet", &TableView::default(), 0, 40, true)
        .unwrap();
    let v3 = runtime.export_project_v3(&revision).unwrap();
    let portable = runtime.export_portable_ro(&revision).unwrap();
    for (portable, bytes) in [(false, v3.bytes), (true, portable.bytes)] {
        let mut reopened = None;
        let occurrence = "00000000-0000-4000-8000-000000000923";
        if portable {
            open_portable_ro_working_size(&mut reopened, &bytes, occurrence).unwrap();
        } else {
            open_project_working_size(&mut reopened, &bytes, occurrence).unwrap();
        }
        let mut reopened = reopened.unwrap();
        assert_eq!(reopened.session.document(), &document);
        let revision = reopened.current_revision().to_owned();
        let after = reopened
            .query_table_window(&revision, "sheet", &TableView::default(), 0, 40, true)
            .unwrap();
        assert_eq!(after.rows, before.rows);
        assert_eq!(after.columns, before.columns);
    }
}

#[test]
fn distant_write_capacity_refusals_are_typed_and_atomic() {
    let mut runtime = filled();
    let before = runtime.session.document().clone();
    let presentation = runtime.presentation.clone();
    let revision = runtime.current_revision().to_owned();
    let history = runtime.undo.len();
    let serial = runtime.row_serial;
    for (row, column, limit, maximum, attempted) in [
        (10_001, 6, crate::CapacityLimit::TableRows, 10_000, 10_001),
        (40, 33, crate::CapacityLimit::TableFields, 32, 33),
        (
            usize::MAX,
            1,
            crate::CapacityLimit::TableRows,
            10_000,
            usize::MAX,
        ),
    ] {
        let error = runtime
            .write_cell_at(
                &revision,
                "sheet",
                row,
                column,
                &ScalarEditInput::Auto {
                    input: "923".into(),
                    date_order: None,
                },
            )
            .unwrap_err();
        let crate::DesignerError::CapacityExceeded(capacity) = error else {
            panic!("typed capacity refusal expected")
        };
        assert_eq!(
            (capacity.limit, capacity.maximum, capacity.attempted),
            (limit, maximum, attempted)
        );
        assert_eq!(capacity.collection.as_deref(), Some("sheet"));
        assert_eq!(runtime.session.document(), &before);
        assert_eq!(runtime.presentation, presentation);
        assert_eq!(runtime.current_revision(), revision);
        assert_eq!(runtime.undo.len(), history);
        assert!(runtime.redo.is_empty());
        assert_eq!(runtime.row_serial, serial);
    }
    write(&mut runtime, 10_000, 32, "923");
    let revision = runtime.current_revision().to_owned();
    let edge = runtime
        .query_table_window(&revision, "sheet", &TableView::default(), 9_999, 1, true)
        .unwrap();
    assert_eq!(edge.total_rows, 10_000);
    assert_eq!(edge.columns[31].display_name, "Column AF");
    assert_eq!(edge.rows[0].position, Some(10_000));
    assert_eq!(
        edge.rows[0]
            .fields
            .iter()
            .find(|field| field.target.field == edge.columns[31].id)
            .unwrap()
            .stored,
        Some(StoredValueProjection::Number { value: 923.0 })
    );
}

#[test]
fn distant_write_preserves_declared_type_admission_and_adjacent_writes() {
    let mut runtime = DesignerRuntime::new_working_size_table(
        "00000000-0000-4000-8000-000000000923",
        "Numbers",
        &[crate::NewTableColumnInput {
            name: "Amount".into(),
            field_type: "number".into(),
            required: true,
        }],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let revision = runtime.current_revision().to_owned();
    let before = runtime.session.document().clone();
    assert!(
        runtime
            .write_cell_at(
                &revision,
                &collection,
                40,
                1,
                &ScalarEditInput::Number {
                    input: "invalid".into()
                }
            )
            .is_err()
    );
    assert_eq!(runtime.session.document(), &before);
    assert_eq!(runtime.current_revision(), revision);
    for row in [1, 2, 40] {
        let revision = runtime.current_revision().to_owned();
        runtime
            .write_cell_at(
                &revision,
                &collection,
                row,
                1,
                &ScalarEditInput::Number { input: "7".into() },
            )
            .unwrap();
    }
    let revision = runtime.current_revision().to_owned();
    let table = runtime
        .query_table_window(&revision, &collection, &TableView::default(), 0, 40, true)
        .unwrap();
    assert_eq!(table.columns[0].field_type, "number");
    assert_eq!(
        table.rows[39].fields[0].stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert!(
        table.rows[2..39]
            .iter()
            .all(|row| row.fields[0].stored == Some(StoredValueProjection::Number { value: 0.0 }))
    );
}
