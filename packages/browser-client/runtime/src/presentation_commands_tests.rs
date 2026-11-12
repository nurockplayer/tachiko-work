use super::{
    CellAlignment, CellStyle, ColumnAlignment, DesignerError, DesignerRequest, DesignerRuntime,
    PatchLifecycleError, PresentationStore, SortDirection, TableView, ViewFilter, ViewSortKey,
    ViewTextOrder, admit_project, totals_tests, view_index_counters,
};
use serde_json::{Map, Value};
use tachiko_workspace_engine::{FieldId, SchemaId, TablePresentation};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";
const FIELD: &str = "f01_amount";

fn runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(totals_tests::valid_document(), OCCURRENCE).unwrap()
}

fn window(runtime: &mut DesignerRuntime, view: &TableView) -> super::TableWindowProjection {
    let revision = runtime.current_revision().to_owned();
    let collection = runtime.default_collection.clone();
    runtime
        .query_table_window(&revision, &collection, view, 0, 10, false)
        .unwrap()
}

fn column(window: &super::TableWindowProjection) -> &super::ColumnProjection {
    window
        .columns
        .iter()
        .find(|column| column.id == FIELD)
        .unwrap()
}

#[test]
fn set_clear_project_and_history_each_command() {
    let mut runtime = runtime();
    let collection = runtime.default_collection.clone();
    let original = runtime.presentation.clone();
    let empty = window(&mut runtime, &TableView::default());
    let mut columns_revision = empty.columns_revision.clone();
    for kind in 0..3 {
        let before = runtime.presentation.clone();
        let revision = runtime.current_revision().to_owned();
        match kind {
            0 => runtime.set_column_alignment(
                &revision,
                &collection,
                FIELD,
                Some(ColumnAlignment::Center),
            ),
            1 => runtime.set_column_wrap(&revision, &collection, FIELD, Some(false)),
            _ => runtime.set_column_width(&revision, &collection, FIELD, Some(2560)),
        }
        .unwrap();
        assert_ne!(revision, runtime.current_revision());
        let after = runtime.presentation.clone();
        let revision = runtime.current_revision().to_owned();
        runtime.history_edit(&revision, false).unwrap();
        assert_ne!(revision, runtime.current_revision());
        assert_eq!(runtime.presentation, before);
        let revision = runtime.current_revision().to_owned();
        runtime.history_edit(&revision, true).unwrap();
        assert_ne!(revision, runtime.current_revision());
        assert_eq!(runtime.presentation, after);
        let projected = window(&mut runtime, &TableView::default());
        assert_ne!(projected.columns_revision, columns_revision);
        columns_revision = projected.columns_revision;
    }
    let shown = window(&mut runtime, &TableView::default());
    assert_ne!(empty.columns_revision, shown.columns_revision);
    assert_eq!(
        column(&shown).style.as_ref().unwrap().alignment,
        Some(ColumnAlignment::Center)
    );
    assert_eq!(column(&shown).style.as_ref().unwrap().wrap, Some(false));
    assert_eq!(column(&shown).width, Some(2560));
    for kind in 0..3 {
        let revision = runtime.current_revision().to_owned();
        match kind {
            0 => runtime.set_column_alignment(&revision, &collection, FIELD, None),
            1 => runtime.set_column_wrap(&revision, &collection, FIELD, None),
            _ => runtime.set_column_width(&revision, &collection, FIELD, None),
        }
        .unwrap();
        assert_ne!(revision, runtime.current_revision());
        let projected = window(&mut runtime, &TableView::default());
        assert_ne!(projected.columns_revision, columns_revision);
        columns_revision = projected.columns_revision;
    }
    let cleared = window(&mut runtime, &TableView::default());
    assert!(column(&cleared).style.is_none());
    assert!(column(&cleared).width.is_none());
    assert_eq!(cleared.columns_revision, empty.columns_revision);
    assert_eq!(runtime.presentation, original);
}

#[test]
fn preserves_imported_bold_and_reopens_all_values() {
    let mut runtime = runtime();
    let collection = runtime.default_collection.clone();
    let style = CellStyle {
        bold: Some(true),
        ..CellStyle::default()
    };
    let id = tachiko_storage::style_id(&style).unwrap();
    let mut table = TablePresentation::default();
    table.column_styles.insert(FieldId::from(FIELD), id.clone());
    let mut store = PresentationStore::default();
    store.styles.insert(id, style);
    store.tables.insert(SchemaId::from("rows"), table);
    runtime.set_presentation(store.clone()).unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_alignment(&revision, &collection, FIELD, Some(ColumnAlignment::Right))
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_alignment(&revision, &collection, FIELD, None)
        .unwrap();
    assert_eq!(runtime.presentation, store);
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_alignment(&revision, &collection, FIELD, Some(ColumnAlignment::Left))
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_wrap(&revision, &collection, FIELD, Some(false))
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_width(&revision, &collection, FIELD, Some(1234))
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    let export = runtime.export_project_v3(&revision).unwrap();
    let (mut reopened, _) = admit_project(&export.bytes, OCCURRENCE).unwrap();
    assert_eq!(reopened.presentation, runtime.presentation);
    let shown = window(&mut reopened, &TableView::default());
    assert_eq!(
        column(&shown).style.as_ref().unwrap().alignment,
        Some(ColumnAlignment::Left)
    );
    assert_eq!(column(&shown).style.as_ref().unwrap().wrap, Some(false));
    assert_eq!(column(&shown).width, Some(1234));
    let revision = reopened.current_revision().to_owned();
    reopened
        .set_column_wrap(&revision, &collection, FIELD, None)
        .unwrap();
    let table = &reopened.presentation.tables[&SchemaId::from("rows")];
    let style = &reopened.presentation.styles[&table.column_styles[&FieldId::from(FIELD)]];
    assert_eq!(style.bold, Some(true));
    assert_eq!(style.alignment, Some(CellAlignment::Left));
    assert_eq!(style.wrap, None);
}

#[test]
fn refusals_leave_revision_store_and_history_unchanged() {
    let mut runtime = runtime();
    let collection = runtime.default_collection.clone();
    let revision = runtime.current_revision().to_owned();
    let before = runtime.presentation.clone();
    let undo = runtime.undo.len();
    let redo = runtime.redo.len();
    assert!(matches!(
        runtime.set_column_wrap(&revision, &collection, FIELD, None),
        Err(DesignerError::Lifecycle(PatchLifecycleError::NoChange))
    ));
    assert!(matches!(
        runtime.set_column_alignment(&revision, &collection, FIELD, None),
        Err(DesignerError::Lifecycle(PatchLifecycleError::NoChange))
    ));
    assert!(matches!(
        runtime.set_column_width(&revision, &collection, FIELD, None),
        Err(DesignerError::Lifecycle(PatchLifecycleError::NoChange))
    ));
    assert!(
        runtime
            .set_column_wrap(&revision, "missing", FIELD, Some(false))
            .is_err()
    );
    for width in [0, 65281] {
        assert!(matches!(
            runtime.set_column_width(&revision, &collection, FIELD, Some(width)),
            Err(DesignerError::InvalidColumnWidth)
        ));
    }
    assert!(
        runtime
            .set_column_alignment(
                &revision,
                &collection,
                "missing",
                Some(ColumnAlignment::Left)
            )
            .is_err()
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.presentation, before);
    assert_eq!(runtime.undo.len(), undo);
    assert_eq!(runtime.redo.len(), redo);
}

#[test]
fn filtered_sorted_index_is_reused() {
    let mut runtime = runtime();
    let view = TableView {
        filters: vec![ViewFilter::TextContains {
            field: "f05_text".into(),
            value: "alpha".into(),
        }],
        sort: vec![ViewSortKey {
            field: FIELD.into(),
            direction: SortDirection::Desc,
            text_order: ViewTextOrder::Bytes,
        }],
        ..TableView::default()
    };
    let before = window(&mut runtime, &view);
    let builds = view_index_counters::builds();
    let revision = runtime.current_revision().to_owned();
    let collection = runtime.default_collection.clone();
    runtime
        .set_column_wrap(&revision, &collection, FIELD, Some(false))
        .unwrap();
    let after = window(&mut runtime, &view);
    assert_eq!(before.rows, after.rows);
    assert_eq!(view_index_counters::builds(), builds);
}

#[test]
fn working_size_refusal_preserves_everything() {
    use tachiko_workspace_engine::{EntityId, Value};

    let (mut document, _, _) = super::working_size_tests::crossing_documents();
    let entity = EntityId::from("r00005");
    let field = FieldId::from("c27");
    document
        .entities
        .get_mut(&entity)
        .unwrap()
        .fields
        .insert(field.clone(), Value::Text(String::new()));
    let base = super::save_size::saved_bytes_upper_bound(&document);
    let maximum = u64::try_from(super::capacity::MAX_WORKING_SAVED_BYTES).unwrap();
    let remaining = usize::try_from(maximum - base - 10).unwrap();
    let text = format!(
        "{}{}",
        "\u{1}".repeat(remaining / 6),
        "a".repeat(remaining % 6)
    );
    document
        .entities
        .get_mut(&entity)
        .unwrap()
        .fields
        .insert(field, Value::Text(text));
    let mut runtime = DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    let collection = runtime.default_collection.clone();
    let revision = runtime.current_revision().to_owned();
    let before = runtime.presentation.clone();
    let undo = runtime.undo.len();
    let redo = runtime.redo.len();
    assert!(matches!(
        runtime.set_column_width(&revision, &collection, "c27", Some(100)),
        Err(DesignerError::CapacityExceeded(_))
    ));
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.presentation, before);
    assert_eq!(runtime.undo.len(), undo);
    assert_eq!(runtime.redo.len(), redo);
}

#[test]
fn wire_commands_dispatch_and_return_column_publications() {
    let mut runtime = runtime();
    let collection = runtime.default_collection.clone();
    for (kind, property, value) in [
        (
            "set_column_alignment",
            "alignment",
            Value::String("left".to_owned()),
        ),
        ("set_column_wrap", "wrap", Value::Bool(false)),
        ("set_column_width", "width", Value::from(256_u32)),
    ] {
        let revision = runtime.current_revision().to_owned();
        let mut request = Map::new();
        request.insert("type".to_owned(), Value::String(kind.to_owned()));
        request.insert(
            "expected_revision".to_owned(),
            Value::String(revision.clone()),
        );
        request.insert("collection".to_owned(), Value::String(collection.clone()));
        request.insert("field".to_owned(), Value::String(FIELD.to_owned()));
        request.insert(property.to_owned(), value);
        let request = Value::Object(request);
        let request = serde_json::from_value(request).unwrap();
        let super::DesignerResponse::Published(publication) = runtime.handle(request).unwrap()
        else {
            panic!("expected publication");
        };
        assert_eq!(publication.base_revision, revision);
        assert_ne!(publication.resulting_revision, revision);
        assert_eq!(publication.schema_fields.len(), 1);
        assert_eq!(publication.schema_fields[0].field, FIELD);
        assert_eq!(publication.entities, [] as [String; 0]);
    }
}

#[test]
fn changing_one_column_keeps_a_style_used_by_another() {
    let mut runtime = runtime();
    let collection = runtime.default_collection.clone();
    let style = CellStyle {
        bold: Some(true),
        ..CellStyle::default()
    };
    let id = tachiko_storage::style_id(&style).unwrap();
    let mut table = TablePresentation::default();
    table.column_styles.insert(FieldId::from(FIELD), id.clone());
    table
        .column_styles
        .insert(FieldId::from("f02_tiny"), id.clone());
    let mut store = PresentationStore::default();
    store.styles.insert(id.clone(), style);
    store.tables.insert(SchemaId::from("rows"), table);
    runtime.set_presentation(store.clone()).unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_alignment(&revision, &collection, FIELD, Some(ColumnAlignment::Right))
        .unwrap();
    assert!(runtime.presentation.styles.contains_key(&id));
    let revision = runtime.current_revision().to_owned();
    runtime.history_edit(&revision, false).unwrap();
    assert_eq!(runtime.presentation, store);
}

#[test]
fn edits_after_a_styled_column_is_removed() {
    let mut runtime = runtime();
    let collection = runtime.default_collection.clone();
    let others = window(&mut runtime, &TableView::default())
        .columns
        .iter()
        .filter(|column| column.id != FIELD)
        .map(|column| column.id.clone())
        .collect::<Vec<_>>();
    let removed = others
        .into_iter()
        .find(|other| {
            let revision = runtime.current_revision().to_owned();
            runtime
                .set_column_width(&revision, &collection, other, Some(512))
                .unwrap();
            let revision = runtime.current_revision().to_owned();
            runtime
                .handle(DesignerRequest::RemoveColumn {
                    expected_revision: revision,
                    collection: collection.clone(),
                    field: other.clone(),
                })
                .is_ok()
        })
        .expect("a removable column");
    let removed = FieldId::from(removed);
    let stale = |runtime: &DesignerRuntime| {
        runtime
            .presentation
            .tables
            .values()
            .any(|table| table.column_widths.contains_key(&removed))
    };
    assert!(stale(&runtime));
    let revision = runtime.current_revision().to_owned();
    runtime
        .set_column_wrap(&revision, &collection, FIELD, Some(true))
        .unwrap();
    assert!(!stale(&runtime));
    assert_eq!(
        column(&window(&mut runtime, &TableView::default()))
            .style
            .as_ref()
            .and_then(|style| style.wrap),
        Some(true)
    );
}
