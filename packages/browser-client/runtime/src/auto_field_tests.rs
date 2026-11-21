use crate::{
    AutoDateOrder, DesignerRequest, DesignerRuntime, FieldTarget, NewTableColumnInput,
    PasteCellInput, ScalarEditInput, ScalarKind, StoredValueProjection, TableView,
    open_portable_ro,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000887";
const REOPEN: &str = "00000000-0000-4000-8000-000000000888";

fn auto_table() -> (DesignerRuntime, String, String) {
    let runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "Mixed",
        &[NewTableColumnInput {
            name: "Value".into(),
            field_type: "auto".into(),
            required: false,
        }],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let field = runtime.query_table(&collection).unwrap().columns[0]
        .id
        .clone();
    (runtime, collection, field)
}

fn auto(input: &str) -> ScalarEditInput {
    ScalarEditInput::Auto {
        input: input.into(),
        date_order: None,
    }
}

fn samples(runtime: &mut DesignerRuntime, collection: &str, field: &str) {
    let mut rows: Vec<Vec<PasteCellInput>> = ["42", "00123", "2026-11-22", "TRUE", "hello"]
        .into_iter()
        .map(|input| vec![PasteCellInput::Text(input.into())])
        .collect();
    rows.push(vec![PasteCellInput::Typed(ScalarEditInput::Auto {
        input: "22/11/2026".into(),
        date_order: Some(AutoDateOrder::DayFirst),
    })]);
    let revision = runtime.current_revision().to_owned();
    runtime
        .paste_cell_inputs(&revision, collection, None, field, &rows)
        .unwrap();
}

fn tags(runtime: &DesignerRuntime, collection: &str) -> Vec<Option<StoredValueProjection>> {
    runtime
        .query_table(collection)
        .unwrap()
        .rows
        .iter()
        .map(|row| row.fields.first().and_then(|field| field.stored.clone()))
        .collect()
}

fn expected_tags() -> Vec<Option<StoredValueProjection>> {
    vec![
        Some(StoredValueProjection::Number { value: 42.0 }),
        Some(StoredValueProjection::Text {
            value: "00123".into(),
        }),
        Some(StoredValueProjection::Date {
            value: "2026-11-22".parse().unwrap(),
        }),
        Some(StoredValueProjection::Boolean { value: true }),
        Some(StoredValueProjection::Text {
            value: "hello".into(),
        }),
        Some(StoredValueProjection::Date {
            value: "2026-11-22".parse().unwrap(),
        }),
    ]
}

#[test]
fn auto_scalar_edits_publish_each_classifier_tag_in_windows() {
    let (mut runtime, collection, field) = auto_table();
    runtime
        .insert_row(None, "resident/0", &collection, &[])
        .unwrap();
    let target = FieldTarget {
        entity: runtime.query_table(&collection).unwrap().rows[0].id.clone(),
        field,
    };
    for (input, expected) in ["42", "00123", "2026-11-22", "TRUE", "hello"]
        .into_iter()
        .zip(expected_tags())
    {
        let revision = runtime.current_revision().to_owned();
        runtime
            .edit_scalar(&revision, &target, &auto(input))
            .unwrap();
        let revision = runtime.current_revision().to_owned();
        let window = runtime
            .query_table_window(&revision, &collection, &TableView::default(), 0, 10, true)
            .unwrap();
        assert_eq!(window.rows[0].fields[0].stored, expected);
        assert_eq!(
            window.rows[0].fields[0].editable_scalar,
            Some(ScalarKind::Auto)
        );
        assert_eq!(window.columns[0].field_type, "auto");
    }
}

#[test]
fn auto_paste_and_portable_reopen_keep_all_scalar_tags_and_date_display() {
    let (mut runtime, collection, field) = auto_table();
    samples(&mut runtime, &collection, &field);
    assert_eq!(tags(&runtime, &collection), expected_tags());
    let saved = runtime
        .export_portable_ro(runtime.current_revision())
        .unwrap();
    let mut reopened = None;
    open_portable_ro(&mut reopened, &saved.bytes, REOPEN).unwrap();
    let reopened_runtime = reopened.unwrap();
    assert_eq!(tags(&reopened_runtime, &collection), expected_tags());
    assert_eq!(runtime.presentation, reopened_runtime.presentation);
    assert_eq!(
        reopened_runtime
            .export_portable_ro(reopened_runtime.current_revision())
            .unwrap()
            .bytes,
        saved.bytes
    );
}

#[test]
fn auto_paste_undo_redo_restores_values_tags_and_implied_displays() {
    let (mut runtime, collection, field) = auto_table();
    samples(&mut runtime, &collection, &field);
    let presentation = runtime.presentation.clone();
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.query_table(&collection).unwrap().rows.len(), 0);
    assert!(runtime.presentation.is_empty());
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, true).unwrap();
    assert_eq!(tags(&runtime, &collection), expected_tags());
    assert_eq!(runtime.presentation, presentation);
}

#[test]
fn auto_empty_text_clear_and_undo_remain_distinct() {
    let (mut runtime, collection, field) = auto_table();
    runtime
        .insert_row(None, "resident/0", &collection, &[])
        .unwrap();
    let target = FieldTarget {
        entity: runtime.query_table(&collection).unwrap().rows[0].id.clone(),
        field,
    };
    let revision = runtime.current_revision().to_owned();
    runtime
        .edit_scalar(
            &revision,
            &target,
            &ScalarEditInput::Text {
                value: String::new(),
            },
        )
        .unwrap();
    assert_eq!(
        tags(&runtime, &collection),
        vec![Some(StoredValueProjection::Text {
            value: String::new()
        })]
    );
    let revision = runtime.current_revision().to_owned();
    runtime
        .edit_scalar(&revision, &target, &ScalarEditInput::Clear)
        .unwrap();
    assert_eq!(tags(&runtime, &collection), vec![None]);
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(
        tags(&runtime, &collection),
        vec![Some(StoredValueProjection::Text {
            value: String::new()
        })]
    );
}

#[test]
fn auto_formula_edit_and_paste_refuse_atomically_and_legacy_export_refuses() {
    let (mut runtime, collection, field) = auto_table();
    samples(&mut runtime, &collection, &field);
    let target = FieldTarget {
        entity: runtime.query_table(&collection).unwrap().rows[0].id.clone(),
        field: field.clone(),
    };
    let revision = runtime.current_revision().to_owned();
    let before = tags(&runtime, &collection);
    let refusal = runtime
        .edit_scalar(&revision, &target, &auto("=1+1"))
        .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("Auto formulas are unsupported")
    );
    let refusal = runtime
        .publish_formula_update(&revision, &target, "1+1", None)
        .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("Auto formulas are unsupported")
    );
    let refusal = runtime
        .paste_cells(
            &revision,
            &collection,
            Some(&target.entity),
            &field,
            &[vec!["9".into()], vec!["=1+1".into()]],
        )
        .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("Auto formulas are unsupported")
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(tags(&runtime, &collection), before);
    assert!(
        runtime
            .export_project(&revision)
            .unwrap_err()
            .to_string()
            .contains("Auto")
    );
}

#[test]
fn auto_field_evolution_add_remove_and_undo_preserve_the_tag() {
    let (mut runtime, collection, _) = auto_table();
    runtime
        .add_column_with_requiredness("resident/0", &collection, "Other", "auto", false, &[])
        .unwrap();
    let columns = runtime.query_table(&collection).unwrap().columns;
    assert_eq!(columns[1].field_type, "auto");
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::RemoveColumn {
            expected_revision: revision,
            collection: collection.clone(),
            field: columns[1].id.clone(),
        })
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.query_table(&collection).unwrap().columns, columns);
}

#[test]
fn auto_currency_and_full_width_display_survive_history_and_save() {
    let (mut runtime, collection, field) = auto_table();
    runtime
        .paste_cells(
            "resident/0",
            &collection,
            None,
            &field,
            &[vec!["￥１，２００".into()]],
        )
        .unwrap();
    assert_eq!(
        tags(&runtime, &collection),
        vec![Some(StoredValueProjection::Number { value: 1200.0 })]
    );
    let before = runtime.presentation.clone();
    let bytes = runtime
        .export_portable_ro(runtime.current_revision())
        .unwrap()
        .bytes;
    let mut reopened = None;
    open_portable_ro(&mut reopened, &bytes, REOPEN).unwrap();
    assert_eq!(reopened.unwrap().presentation, before);
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert!(runtime.presentation.is_empty());
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, true).unwrap();
    assert_eq!(runtime.presentation, before);
}

#[test]
fn auto_presentation_only_edit_and_history_publish_changed_cell() {
    let mut runtime = DesignerRuntime::new_working_size_table(
        OCCURRENCE,
        "Mixed",
        &[NewTableColumnInput {
            name: "Value".into(),
            field_type: "auto".into(),
            required: false,
        }],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let field = runtime.query_table(&collection).unwrap().columns[0]
        .id
        .clone();
    runtime
        .insert_row(None, "resident/0", &collection, &[])
        .unwrap();
    let target = FieldTarget {
        entity: runtime.query_table(&collection).unwrap().rows[0].id.clone(),
        field,
    };
    let revision = runtime.current_revision().to_owned();
    runtime
        .edit_scalar(&revision, &target, &auto("1200"))
        .unwrap();

    for step in 0..3 {
        let expected_revision = runtime.current_revision().to_owned();
        let request = match step {
            0 => DesignerRequest::EditScalar {
                expected_revision,
                target: target.clone(),
                input: auto("￥１，２００"),
            },
            1 => DesignerRequest::Undo { expected_revision },
            _ => DesignerRequest::Redo { expected_revision },
        };
        let crate::DesignerResponse::Published(publication) = runtime.handle(request).unwrap()
        else {
            panic!("expected publication");
        };
        assert!(!publication.many);
        assert_eq!(publication.fields, vec![target.clone()]);
        let revision = runtime.current_revision().to_owned();
        let window = runtime
            .query_table_window(&revision, &collection, &TableView::default(), 0, 10, true)
            .unwrap();
        assert_eq!(publication.changed, vec![window.rows[0].fields[0].clone()]);
        assert_eq!(publication.changed[0].target, target);
    }
}

#[test]
fn declared_columns_keep_their_input_meaning_and_saved_bytes_on_refusal() {
    let mut runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "Typed",
        &[
            NewTableColumnInput {
                name: "Text".into(),
                field_type: "text".into(),
                required: true,
            },
            NewTableColumnInput {
                name: "Number".into(),
                field_type: "number".into(),
                required: true,
            },
            NewTableColumnInput {
                name: "Date".into(),
                field_type: "date".into(),
                required: true,
            },
            NewTableColumnInput {
                name: "Boolean".into(),
                field_type: "boolean".into(),
                required: true,
            },
        ],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let columns = runtime.query_table(&collection).unwrap().columns;
    runtime
        .paste_cells(
            "resident/0",
            &collection,
            None,
            &columns[0].id,
            &[vec![
                "00123".into(),
                "00123".into(),
                "2026-11-22".into(),
                "true".into(),
            ]],
        )
        .unwrap();
    let table = runtime.query_table(&collection).unwrap();
    assert_eq!(
        table.rows[0].fields[0].stored,
        Some(StoredValueProjection::Text {
            value: "00123".into()
        })
    );
    assert_eq!(
        table.rows[0].fields[1].stored,
        Some(StoredValueProjection::Number { value: 123.0 })
    );
    let revision = runtime.current_revision().to_owned();
    let bytes = runtime.export_portable_ro(&revision).unwrap().bytes;
    let target = FieldTarget {
        entity: table.rows[0].id.clone(),
        field: columns[1].id.clone(),
    };
    assert!(
        runtime
            .edit_scalar(&revision, &target, &auto("42"))
            .is_err()
    );
    assert_eq!(runtime.export_portable_ro(&revision).unwrap().bytes, bytes);
}
