use crate::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, Expression, FieldRef,
    FieldType, HeaderFallbackReason, PublicationProjection, ScalarEditInput, StoredValueProjection,
    TableView, TableWindowProjection, Value, blank_workbook, open_portable_ro_working_size,
    open_project_working_size,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000924";

fn blank() -> DesignerRuntime {
    blank_workbook::candidate(OCCURRENCE).unwrap().0
}

fn write(runtime: &mut DesignerRuntime, row: usize, column: usize, text: &str) {
    let revision = runtime.current_revision().to_owned();
    runtime
        .write_cell_at(
            &revision,
            "sheet",
            row,
            column,
            &ScalarEditInput::Auto {
                input: text.into(),
                date_order: None,
            },
        )
        .unwrap();
}

fn window(runtime: &mut DesignerRuntime) -> TableWindowProjection {
    let revision = runtime.current_revision().to_owned();
    runtime
        .query_table_window(&revision, "sheet", &TableView::default(), 0, 10, true)
        .unwrap()
}

fn promote(runtime: &mut DesignerRuntime) -> PublicationProjection {
    let response = runtime
        .handle(DesignerRequest::PromoteFirstRowToHeaders {
            expected_revision: runtime.current_revision().to_owned(),
            collection: "sheet".into(),
        })
        .unwrap();
    let DesignerResponse::Published(publication) = response else {
        panic!("expected published")
    };
    publication
}

pub(super) fn typed() -> DesignerRuntime {
    let mut runtime = blank();
    for (row, values) in [
        (1, [" Label ", "7", "true", "2026-11-23", "Mixed", "8", "9"]),
        (2, ["Tea", "120", "false", "2026-11-24", "00123", "0", "0"]),
    ] {
        for (column, text) in values.iter().enumerate() {
            write(&mut runtime, row, column + 1, text);
        }
    }
    let spec = runtime.collection_specs["sheet"].clone();
    let mut document = runtime.session.document().clone();
    let schema = document.schemas.get_mut(spec.summary.id.as_str()).unwrap();
    for (column, kind) in spec.columns.iter().zip([
        FieldType::Text,
        FieldType::Number,
        FieldType::Boolean,
        FieldType::Date,
        FieldType::Auto,
        FieldType::Number,
        FieldType::Number,
    ]) {
        schema.fields.get_mut(&column.id).unwrap().field_type = kind;
    }
    let data = document.entities.get_mut(&spec.entities[1]).unwrap();
    data.fields.insert(
        spec.columns[5].id.clone(),
        Value::Formula(Expression::Reference(FieldRef::new(
            data.id.clone(),
            spec.columns[1].id.clone(),
        ))),
    );
    data.fields.insert(
        spec.columns[6].id.clone(),
        Value::Formula(Expression::SameRowReference(spec.columns[1].id.clone())),
    );
    DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap()
}

#[test]
fn promotion_consumes_first_stored_row_and_reports_unique_generated_fallbacks() {
    let mut runtime = blank();
    for (column, text) in ["", "Column A", " Item ", "Item", "", "item", "Column A (2)"]
        .iter()
        .enumerate()
    {
        write(&mut runtime, 1, column + 1, text);
    }
    write(&mut runtime, 2, 1, "Tea");
    let before = window(&mut runtime);
    let publication = promote(&mut runtime);
    let after = window(&mut runtime);
    assert!(!after.collection.headerless);
    assert_eq!(after.collection.data_start_row, 2);
    assert_eq!(after.total_rows, 1);
    let mut remaining = before.rows[1..].to_vec();
    remaining[0].position = Some(1);
    assert_eq!(after.rows, remaining);
    assert_eq!(
        after
            .columns
            .iter()
            .map(|c| c.display_name.as_str())
            .collect::<Vec<_>>(),
        [
            "Column A (3)",
            "Column A",
            "Item",
            "Column D",
            "Column E",
            "item",
            "Column A (2)"
        ]
    );
    assert_eq!(publication.header_fallbacks.len(), 3);
    for (fallback, (index, reason)) in publication.header_fallbacks.iter().zip([
        (0, HeaderFallbackReason::Blank),
        (3, HeaderFallbackReason::Duplicate),
        (4, HeaderFallbackReason::Blank),
    ]) {
        assert_eq!(fallback.field, after.columns[index].id);
        assert_eq!(fallback.name, after.columns[index].display_name);
        assert_eq!(fallback.reason, reason);
    }
}

#[test]
fn promotion_preserves_auto_and_all_declared_scalar_types() {
    let mut runtime = typed();
    let before = runtime.session.document().schemas.clone();
    let prior = window(&mut runtime);
    promote(&mut runtime);
    let after = window(&mut runtime);
    let mut remaining = prior.rows[1..].to_vec();
    remaining[0].position = Some(1);
    assert_eq!(after.rows, remaining);
    for (id, schema) in &before {
        for (field, definition) in &schema.fields {
            let current = &runtime.session.document().schemas[id].fields[field];
            assert_eq!(current.field_type, definition.field_type);
            assert_eq!(current.key, definition.key);
            assert_eq!(current.required, definition.required);
        }
    }
    assert_eq!(after.columns[0].display_name, "Label");
    assert_eq!(after.columns[1].display_name, "7");
    assert_eq!(after.columns[2].display_name, "true");
    assert_eq!(after.columns[3].display_name, "2026-11-23");
    assert_eq!(after.columns[4].field_type, "auto");
    assert_eq!(
        after.rows[0].fields[4].stored,
        Some(StoredValueProjection::Text {
            value: "00123".into()
        })
    );
}

#[test]
fn promotion_refuses_headers_and_empty_tables_without_publication_or_history() {
    let mut empty = blank();
    let mut headed = typed();
    promote(&mut headed);
    for (runtime, code) in [
        (&mut empty, "no_header_row"),
        (&mut headed, "table_already_has_headers"),
    ] {
        let revision = runtime.current_revision().to_owned();
        let document = runtime.session.document().clone();
        let history = (runtime.undo.len(), runtime.redo.len());
        let error = runtime
            .promote_first_row_to_headers(&revision, "sheet")
            .unwrap_err();
        assert!(matches!(
            error,
            DesignerError::NoHeaderRow | DesignerError::TableAlreadyHasHeaders
        ));
        assert_eq!(error.generic_failure().0, code);
        assert_eq!(runtime.current_revision(), revision);
        assert_eq!(runtime.session.document(), &document);
        assert_eq!((runtime.undo.len(), runtime.redo.len()), history);
    }
}

#[test]
fn promotion_one_undo_redo_and_v3_and_portable_reopen_are_exact() {
    let mut runtime = typed();
    let before = runtime.session.document().clone();
    let before_window = window(&mut runtime);
    let history = runtime.undo.len();
    promote(&mut runtime);
    let promoted = runtime.session.document().clone();
    let promoted_window = window(&mut runtime);
    assert_eq!(runtime.undo.len(), history + 1);
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.session.document(), &before);
    assert_eq!(window(&mut runtime).collection, before_window.collection);
    assert_eq!(window(&mut runtime).rows, before_window.rows);
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, true).unwrap();
    assert_eq!(runtime.session.document(), &promoted);
    let revision = runtime.current_revision().to_owned();
    let v3 = runtime.export_project_v3(&revision).unwrap();
    let portable = runtime.export_portable_ro(&revision).unwrap();
    for (portable, bytes) in [(false, v3.bytes), (true, portable.bytes)] {
        let mut reopened = None;
        if portable {
            open_portable_ro_working_size(&mut reopened, &bytes, OCCURRENCE).unwrap();
        } else {
            open_project_working_size(&mut reopened, &bytes, OCCURRENCE).unwrap();
        }
        let mut reopened = reopened.unwrap();
        assert_eq!(reopened.session.document(), &promoted);
        let after = window(&mut reopened);
        assert_eq!(after.collection, promoted_window.collection);
        assert_eq!(after.columns, promoted_window.columns);
        assert_eq!(after.rows, promoted_window.rows);
    }
}

#[test]
fn promotion_retains_field_ids_and_bound_direct_and_same_row_formulas() {
    let mut runtime = typed();
    let before = runtime.session.document().clone();
    let first = runtime.collection_specs["sheet"].entities[0].clone();
    let schema_id = runtime.collection_specs["sheet"].summary.id.clone();
    let publication = promote(&mut runtime);
    assert_eq!(publication.schemas, vec![schema_id]);
    let after = runtime.session.document();
    for (id, entity) in &before.entities {
        if *id != first {
            assert_eq!(&after.entities[id], entity);
        }
    }
    let table = window(&mut runtime);
    for column in [5, 6] {
        assert_eq!(
            table.rows[0].fields[column]
                .calculated
                .as_ref()
                .unwrap()
                .number(),
            Some(120.0)
        );
    }
}

// Failed calculations cannot pass open-time validation. Install the retained
// session directly so promotion must handle both failure and unavailability.
pub(super) fn formula_header(failed: bool) -> DesignerRuntime {
    let mut runtime = typed();
    let spec = &runtime.collection_specs["sheet"];
    let mut document = runtime.session.document().clone();
    let division = Expression::Divide {
        left: Box::new(Expression::Number(crate::Number::new(1.0).unwrap())),
        right: Box::new(Expression::Number(crate::Number::new(0.0).unwrap())),
    };
    document
        .entities
        .get_mut(&spec.entities[0])
        .unwrap()
        .fields
        .insert(
            spec.columns[5].id.clone(),
            Value::Formula(if failed {
                division.clone()
            } else {
                Expression::SameRowReference(spec.columns[1].id.clone())
            }),
        );
    document
        .entities
        .get_mut(&spec.entities[1])
        .unwrap()
        .fields
        .insert(spec.columns[6].id.clone(), Value::Formula(division));
    runtime.session = tachiko_workspace_engine::resident_session::ResidentWorkspaceSession::new(
        runtime.document_scope.clone(),
        document,
    );
    runtime
}

#[test]
fn promotion_refuses_failed_and_unavailable_formulas_atomically() {
    for failed in [true, false] {
        let mut runtime = formula_header(failed);
        let spec = &runtime.collection_specs["sheet"];
        let fields = runtime
            .session
            .query_fields(&[FieldRef::new(
                spec.entities[0].clone(),
                spec.columns[5].id.clone(),
            )])
            .unwrap();
        let outcome = &fields.value()[0].calculated_value;
        if failed {
            assert!(matches!(
                outcome,
                Some(crate::FormulaCalculationOutcome::Failure(_))
            ));
        } else {
            assert_eq!(
                outcome,
                &Some(crate::FormulaCalculationOutcome::Unavailable)
            );
        }
        let before = runtime.session.document().clone();
        let error = runtime
            .promote_first_row_to_headers("resident/0", "sheet")
            .unwrap_err();
        assert!(matches!(error, DesignerError::HeaderFormulaUnavailable));
        assert_eq!(error.generic_failure().0, "header_formula_unavailable");
        assert_eq!(runtime.current_revision(), "resident/0");
        assert_eq!(runtime.session.document(), &before);
        assert_eq!(runtime.undo.len(), 0);
        assert_eq!(runtime.redo.len(), 0);
    }
}

fn pad_header_reply(reply: &mut crate::DesignerWireReply, start: usize) {
    let mut padding = crate::MAX_TEXT_CAPACITY_PROJECTION_BYTES + 1
        - crate::serialized_projection_bytes(reply).unwrap();
    let crate::DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = reply
    else {
        unreachable!()
    };
    let metadata = publication.import_metadata.as_mut().unwrap();
    for row in &mut metadata.sheets[0].rows[start..] {
        let bytes = padding.min(crate::MAX_PROFILE_STRING_BYTES);
        row.styles[0].number_format = Some("0".repeat(bytes));
        padding -= bytes;
    }
    assert_eq!(padding, 0);
}

pub(super) fn near_limit_imported_header() -> DesignerRuntime {
    use crate::interop_adapter::{CellStyle, ImportOptions, import_csv};
    use crate::interop_document::InteropRowMetadata;

    let source = import_csv(
        b",Item,Item\nTea,1,2\n",
        &ImportOptions {
            header: false,
            ..ImportOptions::default()
        },
    )
    .unwrap();
    let selection = crate::ImportSelection {
        column_types: vec![vec![crate::ImportFieldType::Text; 3]],
        extra_columns: vec![vec![]],
    };
    let (mut runtime, _) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    let document = runtime.session.document().clone();
    let collection = runtime.default_collection.clone();
    let response = runtime
        .handle(DesignerRequest::PromoteFirstRowToHeaders {
            expected_revision: "resident/0".into(),
            collection,
        })
        .unwrap();
    let DesignerResponse::Published(mut publication) = response else {
        panic!("expected published")
    };
    let metadata = publication.import_metadata.as_mut().unwrap();
    metadata.version = 1;
    metadata.sheets[0].rows = document
        .entities
        .values()
        .map(|entity| InteropRowMetadata {
            entity_id: entity.id.to_string(),
            styles: vec![CellStyle::default(); 3],
        })
        .collect();
    let rows = &mut metadata.sheets[0].rows;
    let start = rows.len();
    for index in 0..=crate::MAX_TEXT_CAPACITY_PROJECTION_BYTES / crate::MAX_PROFILE_STRING_BYTES {
        rows.push(InteropRowMetadata {
            entity_id: format!("historical_{index}"),
            styles: vec![
                CellStyle {
                    number_format: Some(String::new()),
                    ..CellStyle::default()
                };
                3
            ],
        });
    }
    let mut reply = crate::DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    };
    pad_header_reply(&mut reply, start);
    assert!(crate::ensure_wire_reply_size(&reply, runtime.profile).is_err());
    let crate::DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = &mut reply
    else {
        unreachable!()
    };
    let metadata = publication.import_metadata.as_mut().unwrap();
    let mut original = metadata.clone();
    original.sheets[0].has_header = false;
    assert_ne!(publication.header_fallbacks.len(), 0);
    publication.header_fallbacks.clear();
    crate::ensure_wire_reply_size(&reply, runtime.profile).unwrap();
    let mut runtime = DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    crate::interop_document::validate_import_metadata_for_profile(
        runtime.session.document(),
        &original,
        runtime.profile,
    )
    .unwrap();
    runtime.attach_imported_metadata(&original);
    runtime
}

#[test]
fn promotion_refuses_reply_overflow_from_header_fallbacks_atomically() {
    let mut runtime = near_limit_imported_header();
    let before = runtime.session.document().clone();
    let imported = runtime.imported.clone().unwrap();
    let collection = runtime.default_collection.clone();
    assert!(matches!(
        runtime.promote_first_row_to_headers("resident/0", &collection),
        Err(DesignerError::ProjectionTooLarge { .. })
    ));
    assert_eq!(runtime.current_revision(), "resident/0");
    assert_eq!(runtime.session.document(), &before);
    let after = runtime.imported.as_ref().unwrap();
    assert_eq!(after.metadata, imported.metadata);
    assert_eq!(after.lifecycle_changed, imported.lifecycle_changed);
    assert_eq!(runtime.undo.len(), 0);
    assert_eq!(runtime.redo.len(), 0);
}
