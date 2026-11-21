use std::collections::BTreeMap;

use serde_json::json;
use tachiko_workspace_engine::{
    CellStyle, Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldType, Number, Schema, SchemaId, SchemaKey, TablePresentation, Value,
};

use crate::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, PublicationProjection,
    SessionProfile, SortDirection, TableView, ViewSortKey, ViewTextOrder, admit_portable_ro,
    admit_portable_ro_working_size,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000815";

fn fixture() -> Document {
    let mut document = Document::empty("styles", "Cell styles");
    let fields = ["a", "b", "c"]
        .into_iter()
        .map(|name| {
            let id = FieldId::from(name);
            (
                id.clone(),
                FieldDefinition {
                    id,
                    key: FieldKey::from(name),
                    display_name: None,
                    field_type: FieldType::Number,
                    required: false,
                    constraint: FieldConstraint::None,
                    number_format: None,
                    date_format: None,
                },
            )
        })
        .collect();
    document.schemas.insert(
        SchemaId::from("rows"),
        Schema {
            id: SchemaId::from("rows"),
            key: SchemaKey::from("rows"),
            display_name: None,
            field_order: None,
            fields,
        },
    );
    for (name, value) in [("r1", 3.0), ("r2", 1.0), ("r3", 2.0)] {
        let id = EntityId::from(name);
        document.entities.insert(
            id.clone(),
            Entity {
                rank: None,
                id,
                key: EntityKey::from(name),
                schema: SchemaId::from("rows"),
                fields: BTreeMap::from([(
                    FieldId::from("a"),
                    Value::Number(Number::new(value).unwrap()),
                )]),
            },
        );
    }
    document
}

fn edit(
    runtime: &mut DesignerRuntime,
    entities: &[&str],
    fields: &[&str],
    changes: &serde_json::Value,
) -> Result<PublicationProjection, DesignerError> {
    let request = serde_json::from_value(json!({
        "type": "set_cell_style", "expected_revision": runtime.current_revision(),
        "collection": "rows", "entities": entities, "fields": fields, "changes": changes,
    }))
    .unwrap();
    match runtime.handle(request)? {
        DesignerResponse::Published(publication) => Ok(publication),
        _ => panic!("style edit must publish"),
    }
}

fn history(runtime: &mut DesignerRuntime, redo: bool) {
    let expected_revision = runtime.current_revision().to_owned();
    let request = if redo {
        DesignerRequest::Redo { expected_revision }
    } else {
        DesignerRequest::Undo { expected_revision }
    };
    runtime.handle(request).unwrap();
}

fn window(runtime: &mut DesignerRuntime, view: &TableView) -> crate::TableWindowProjection {
    let revision = runtime.current_revision().to_owned();
    runtime
        .query_table_window(&revision, "rows", view, 0, 10, true)
        .unwrap()
}

#[test]
fn rectangle_toggle_and_each_undo_are_presentation_only_in_every_profile() {
    for profile in [
        SessionProfile::Ordinary,
        SessionProfile::TextCapacity,
        SessionProfile::WorkingSize,
    ] {
        let mut runtime =
            DesignerRuntime::from_document_profile(fixture(), OCCURRENCE, profile).unwrap();
        let document = runtime.session.document().clone();
        let original = runtime.presentation.clone();
        let first = edit(
            &mut runtime,
            &["r1", "r2", "r3"],
            &["b"],
            &json!([
                {"kind": "toggle_bold"}, {"kind": "italic", "value": true},
            ]),
        )
        .unwrap();
        assert_ne!(first.base_revision, first.resulting_revision);
        assert!(first.many);
        assert_eq!(first.affected_calculations.len(), 0);
        assert_eq!(runtime.undo.len(), 1);
        let bold = runtime.presentation.clone();
        let shown = window(&mut runtime, &TableView::default());
        assert!(
            shown
                .rows
                .iter()
                .all(|row| row.styles[1].bold && row.styles[1].italic)
        );
        assert!(
            shown
                .rows
                .iter()
                .all(|row| row.blank_fields.contains(&"b".to_owned()))
        );
        edit(
            &mut runtime,
            &["r1", "r2", "r3"],
            &["b"],
            &json!([{"kind": "toggle_bold"}]),
        )
        .unwrap();
        let not_bold = runtime.presentation.clone();
        assert!(
            window(&mut runtime, &TableView::default())
                .rows
                .iter()
                .all(|row| !row.styles[1].bold)
        );
        history(&mut runtime, false);
        assert_eq!(runtime.presentation, bold);
        history(&mut runtime, false);
        assert_eq!(runtime.presentation, original);
        history(&mut runtime, true);
        history(&mut runtime, true);
        assert_eq!(runtime.presentation, not_bold);
        assert_eq!(runtime.session.document(), &document);
    }
}

#[test]
fn fill_follows_identity_through_sort_and_portable_save_reopen() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    edit(
        &mut runtime,
        &["r2"],
        &["c"],
        &json!([
            {"kind": "fill", "color": "#FFFF00"}, {"kind": "italic", "value": true},
        ]),
    )
    .unwrap();
    let view = TableView {
        sort: vec![ViewSortKey {
            field: "a".into(),
            direction: SortDirection::Asc,
            text_order: ViewTextOrder::Bytes,
        }],
        ..TableView::default()
    };
    let sorted = window(&mut runtime, &view);
    assert_eq!(sorted.rows[0].id, "r2");
    assert_eq!(sorted.rows[0].styles[2].fill.as_deref(), Some("FFFF00"));
    let export = runtime
        .export_portable_ro(runtime.current_revision())
        .unwrap();
    let (mut reopened, opened) = admit_portable_ro(&export.bytes, OCCURRENCE).unwrap();
    let row = opened.table.rows.iter().find(|row| row.id == "r2").unwrap();
    assert_eq!(row.styles[2].fill.as_deref(), Some("FFFF00"));
    let (_, working) = admit_portable_ro_working_size(&export.bytes, OCCURRENCE).unwrap();
    let row = working
        .window
        .rows
        .iter()
        .find(|row| row.id == "r2")
        .unwrap();
    assert!(row.styles[2].italic);
    assert_eq!(reopened.presentation, runtime.presentation);
    let sorted = window(&mut reopened, &view);
    assert_eq!(sorted.rows[0].id, "r2");
    assert_eq!(sorted.rows[0].styles[2].fill.as_deref(), Some("FFFF00"));
    assert!(sorted.rows[0].styles[2].italic);
    edit(
        &mut reopened,
        &["r2"],
        &["c"],
        &json!([
            {"kind": "fill", "color": null}, {"kind": "italic", "value": false},
        ]),
    )
    .unwrap();
    let cleared = window(&mut reopened, &view);
    assert!(cleared.rows[0].styles[2].fill.is_none());
    assert!(!cleared.rows[0].styles[2].italic);
}

fn column_fill(runtime: &mut DesignerRuntime) {
    let style = CellStyle {
        fill: Some("#123ABC".into()),
        bold: Some(true),
        ..CellStyle::default()
    };
    let id = tachiko_storage::style_id(&style).unwrap();
    runtime.presentation.styles.insert(id.clone(), style);
    runtime.presentation.tables.insert(
        SchemaId::from("rows"),
        TablePresentation {
            column_styles: BTreeMap::from([(FieldId::from("b"), id)]),
            ..TablePresentation::default()
        },
    );
}

#[test]
fn clearing_inherited_fill_preserves_neighbours_and_undo_restores_column_style() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    column_fill(&mut runtime);
    let original = runtime.presentation.clone();
    edit(
        &mut runtime,
        &["r2"],
        &["b"],
        &json!([{"kind": "fill", "color": null}]),
    )
    .unwrap();
    let shown = window(&mut runtime, &TableView::default());
    for row in shown.rows {
        assert_eq!(
            row.styles[1].fill.as_deref(),
            if row.id == "r2" { None } else { Some("123ABC") }
        );
        assert!(row.styles[1].bold);
    }
    history(&mut runtime, false);
    assert_eq!(runtime.presentation, original);
}

#[test]
fn stale_colour_unknown_ids_and_read_only_refuse_atomically() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    let revision = runtime.current_revision().to_owned();
    for color in ["yellow", "#ffff00", "#123", "#GGGGGG"] {
        let error = edit(
            &mut runtime,
            &["r1"],
            &["b"],
            &json!([
                {"kind": "bold", "value": true}, {"kind": "fill", "color": color},
            ]),
        )
        .unwrap_err();
        assert_eq!(
            error.failure_projection(&revision).code,
            "invalid_style_color"
        );
    }
    assert!(matches!(
        edit(
            &mut runtime,
            &["r1", "missing"],
            &["b"],
            &json!([{ "kind": "toggle_bold" }])
        ),
        Err(DesignerError::UnknownEntity { .. })
    ));
    assert!(matches!(
        edit(
            &mut runtime,
            &["r1"],
            &["b", "missing"],
            &json!([{ "kind": "toggle_bold" }])
        ),
        Err(DesignerError::UnknownViewField { .. })
    ));
    let request = serde_json::from_value(json!({
        "type": "set_cell_style", "expected_revision": "stale", "collection": "rows",
        "entities": ["r1"], "fields": ["b"], "changes": [{"kind": "toggle_bold"}],
    }))
    .unwrap();
    assert!(matches!(
        runtime.handle(request),
        Err(DesignerError::StaleQuery { .. })
    ));
    let request = serde_json::from_value(json!({
        "type": "set_cell_style", "expected_revision": revision, "collection": "missing",
        "entities": ["r1"], "fields": ["b"], "changes": [{"kind": "toggle_bold"}],
    }))
    .unwrap();
    assert!(matches!(
        runtime.handle(request),
        Err(DesignerError::MissingCollection { .. })
    ));
    runtime
        .collection_specs
        .get_mut("rows")
        .unwrap()
        .native_table_rows = false;
    let error = edit(
        &mut runtime,
        &["r1"],
        &["b"],
        &json!([{ "kind": "toggle_bold" }]),
    )
    .unwrap_err();
    assert_eq!(error.failure_projection(&revision).code, "read_only_table");
    assert_eq!(runtime.current_revision(), revision);
    assert!(runtime.presentation.is_empty());
    assert_eq!(runtime.undo.len(), 0);
    assert_eq!(runtime.redo.len(), 0);
}

#[test]
fn mixed_selection_toggle_sets_all_and_repeated_set_is_no_change() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    edit(
        &mut runtime,
        &["r1"],
        &["b"],
        &json!([{"kind": "bold", "value": true}]),
    )
    .unwrap();
    edit(
        &mut runtime,
        &["r1", "r2"],
        &["b", "c"],
        &json!([
            {"kind": "toggle_bold"}, {"kind": "toggle_italic"},
        ]),
    )
    .unwrap();
    for row in window(&mut runtime, &TableView::default())
        .rows
        .iter()
        .take(2)
    {
        assert!(
            row.styles[1..]
                .iter()
                .all(|style| style.bold && style.italic)
        );
    }
    let revision = runtime.current_revision().to_owned();
    let undo = runtime.undo.len();
    let error = edit(
        &mut runtime,
        &["r1", "r2"],
        &["b", "c"],
        &json!([
            {"kind": "bold", "value": true}, {"kind": "italic", "value": true},
        ]),
    )
    .unwrap_err();
    assert_eq!(error.failure_projection(&revision).code, "no_change");
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.undo.len(), undo);
}

#[test]
fn imported_styles_and_native_styles_use_the_same_projection() {
    use crate::interop_adapter::{
        CellStyle as SourceStyle, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue,
        SourceWorkbook,
    };
    use crate::{ImportFieldType, ImportSelection, import_workbook};
    let source = SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Imported".into(),
            has_header: true,
            columns: vec![SourceColumn {
                name: "Number".into(),
                width: None,
                number_format: None,
            }],
            rows: [1.0, 2.0]
                .into_iter()
                .map(|value| {
                    vec![SourceCell {
                        value: SourceValue::Number { value },
                        formula: None,
                        style: SourceStyle {
                            bold: true,
                            fill: Some("FFFF00".into()),
                            wrap: true,
                            border: true,
                            ..SourceStyle::default()
                        },
                    }]
                })
                .collect(),
            layout: SourceLayout::default(),
        }],
        ledger: Vec::new(),
    };
    let (mut runtime, imported) = import_workbook(
        &source,
        &ImportSelection {
            column_types: vec![vec![ImportFieldType::Number]],
            extra_columns: vec![Vec::new()],
        },
        OCCURRENCE,
    )
    .unwrap();
    let sheet = &imported.metadata.sheets[0];
    let request = serde_json::from_value(json!({
        "type": "set_cell_style", "expected_revision": runtime.current_revision(),
        "collection": sheet.schema_id, "entities": [sheet.rows[0].entity_id],
        "fields": [sheet.columns[0].field_id], "changes": [
            {"kind": "bold", "value": false}, {"kind": "italic", "value": true},
            {"kind": "fill", "color": null},
        ],
    }))
    .unwrap();
    let before = runtime.presentation.clone();
    runtime.handle(request).unwrap();
    let revision = runtime.current_revision().to_owned();
    let shown = runtime
        .query_table_window(
            &revision,
            &sheet.schema_id,
            &TableView::default(),
            0,
            10,
            false,
        )
        .unwrap();
    let first = &shown.rows[0].styles[0];
    assert!(!first.bold);
    assert!(first.italic);
    assert!(first.fill.is_none());
    assert!(first.wrap);
    assert!(first.border);
    let neighbour = &shown.rows[1].styles[0];
    assert!(neighbour.bold);
    assert_eq!(neighbour.fill.as_deref(), Some("FFFF00"));
    history(&mut runtime, false);
    assert_eq!(runtime.presentation, before);
}
