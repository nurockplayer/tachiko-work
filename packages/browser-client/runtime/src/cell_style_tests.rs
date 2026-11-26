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

pub(crate) fn fixture() -> Document {
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
            headerless: false,
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

pub(crate) fn edit(
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

pub(crate) fn history(runtime: &mut DesignerRuntime, redo: bool) {
    let expected_revision = runtime.current_revision().to_owned();
    let request = if redo {
        DesignerRequest::Redo { expected_revision }
    } else {
        DesignerRequest::Undo { expected_revision }
    };
    runtime.handle(request).unwrap();
}

pub(crate) fn window(
    runtime: &mut DesignerRuntime,
    view: &TableView,
) -> crate::TableWindowProjection {
    let revision = runtime.current_revision().to_owned();
    runtime
        .query_table_window(&revision, "rows", view, 0, 10, true)
        .unwrap()
}

fn range_number_format_request(revision: &str, rows: usize, fields: &[&str]) -> serde_json::Value {
    json!({
        "type": "set_cell_style_range", "expected_revision": revision,
        "collection": "rows", "row": 1, "row_count": rows, "fields": fields,
        "changes": [{"kind": "number_format", "format": {
            "kind": {"type": "percent"}, "decimals": 1, "thousands_separator": false,
        }}],
    })
}

#[test]
fn range_number_format_uses_stored_identities_and_one_undo() {
    let mut document = fixture();
    document.entities.get_mut("r3").unwrap().rank =
        Some(tachiko_workspace_engine::RowRank::new("A").unwrap());
    document
        .schemas
        .get_mut("rows")
        .unwrap()
        .fields
        .get_mut("a")
        .unwrap()
        .number_format = crate::interop_number_format::from_excel("0.00");
    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    edit(
        &mut runtime,
        &["r3"],
        &["a"],
        &json!([{"kind": "bold", "value": true}]),
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
    let before = window(&mut runtime, &view);
    let request = range_number_format_request(runtime.current_revision(), 2, &["a", "b"]);
    runtime
        .handle(serde_json::from_value(request).unwrap())
        .unwrap();
    assert_eq!(runtime.undo.len(), 2);
    let after = window(&mut runtime, &view);
    assert_eq!(after.rows[0].id, "r2");
    assert_eq!(after.rows[0].fields[0].display.as_deref(), Some("1.00"));
    assert_eq!(after.rows[0], before.rows[0]);
    for row in &after.rows[1..] {
        let display = if row.id == "r3" { "200.0%" } else { "300.0%" };
        assert_eq!(row.fields[0].display.as_deref(), Some(display));
        assert_eq!(row.styles[0].bold, row.id == "r3");
        assert_eq!(row.styles[1].number_format.as_deref(), Some("0.0%"));
        assert_eq!(row.styles[2].number_format, None);
    }
    history(&mut runtime, false);
    assert_eq!(window(&mut runtime, &view).rows, before.rows);
    history(&mut runtime, true);
    assert_eq!(window(&mut runtime, &view).rows, after.rows);
}

#[test]
fn range_number_format_survives_project_and_xlsx_round_trips() {
    use crate::interop_adapter::{export_xlsx, import_xlsx};
    use crate::{ImportFieldType, ImportSelection, import_workbook};

    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    let request = range_number_format_request(runtime.current_revision(), 2, &["a", "b"]);
    runtime
        .handle(serde_json::from_value(request).unwrap())
        .unwrap();
    let saved = runtime
        .export_portable_ro(runtime.current_revision())
        .unwrap();
    let (reopened, _) = admit_portable_ro(&saved.bytes, OCCURRENCE).unwrap();
    assert_eq!(reopened.presentation, runtime.presentation);

    let workbook = reopened
        .export_native_tables_workbook(reopened.current_revision(), "rows")
        .unwrap();
    let source = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Number; 3]],
        extra_columns: vec![vec![]],
    };
    let (mut imported, projection) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let revision = imported.current_revision().to_owned();
    let table = imported
        .query_table_window(
            &revision,
            &projection.opened.table.collection.key,
            &TableView::default(),
            0,
            3,
            true,
        )
        .unwrap();
    for (row, display) in table.rows[..2].iter().zip(["300.0%", "100.0%"]) {
        assert_eq!(row.fields[0].display.as_deref(), Some(display));
        assert_eq!(row.styles[1].number_format.as_deref(), Some("0.0%"));
    }
    assert!(table.rows[2].styles.is_empty());
    assert_eq!(table.rows[2].fields[0].display.as_deref(), Some("2"));
}

#[test]
fn range_number_format_applies_a1_z2000_in_one_step() {
    const ROWS: usize = 2_000;
    let mut document = fixture();
    let schema = document.schemas.get_mut("rows").unwrap();
    let template = schema.fields["a"].clone();
    for column in b'd'..=b'z' {
        let name = char::from(column).to_string();
        let mut field = template.clone();
        field.id = name.as_str().into();
        field.key = name.as_str().into();
        schema.fields.insert(field.id.clone(), field);
    }
    let fields: Vec<_> = schema.fields.keys().map(ToString::to_string).collect();
    let field_ids: Vec<_> = fields.iter().map(String::as_str).collect();
    let template = document.entities["r1"].clone();
    document.entities.clear();
    for row in 1..=ROWS {
        let name = format!("r{row:04}");
        let mut entity = template.clone();
        entity.id = name.as_str().into();
        entity.key = name.as_str().into();
        document.entities.insert(entity.id.clone(), entity);
    }
    let runtime = DesignerRuntime::from_document_working_size(document, OCCURRENCE).unwrap();
    let request = range_number_format_request(runtime.current_revision(), ROWS, &field_ids);
    let mut runtime = Some(runtime);
    let reply: serde_json::Value = serde_json::from_slice(&crate::process_wire_request(
        &mut runtime,
        &serde_json::to_vec(&request).unwrap(),
    ))
    .unwrap();
    assert_eq!(reply["response"]["type"], "published", "{reply}");
    let mut runtime = runtime.unwrap();
    assert_eq!(runtime.presentation.cells.len(), 26 * ROWS);
    assert_eq!(runtime.presentation.styles.len(), 1);
    assert_eq!(runtime.undo.len(), 1);
    history(&mut runtime, false);
    assert!(runtime.presentation.is_empty());
    assert!(runtime.undo.is_empty());
}

#[test]
fn cell_style_range_formats_20_000_cells_inside_text_capacity_wire_limit() {
    const ROWS: usize = 5_000;
    let mut document = fixture();
    let schema = document.schemas.get_mut("rows").unwrap();
    let mut fourth = schema.fields["c"].clone();
    fourth.id = "d".into();
    fourth.key = "d".into();
    schema.fields.insert(fourth.id.clone(), fourth);
    let mut template = document.entities["r1"].clone();
    for field in ["b", "c", "d"] {
        template
            .fields
            .insert(field.into(), Value::Number(Number::new(1.0).unwrap()));
    }
    let entities: Vec<_> = (0..ROWS).map(|i| format!("record-{i:05}")).collect();
    document.entities.clear();
    for id in &entities {
        let mut entity = template.clone();
        entity.id = id.as_str().into();
        entity.key = id.as_str().into();
        document.entities.insert(entity.id.clone(), entity);
    }
    let mut runtime = Some(DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap());
    let limit = crate::wire_request_limit(runtime.as_ref());
    assert_eq!(
        runtime.as_ref().unwrap().profile,
        SessionProfile::TextCapacity
    );
    assert_eq!(limit, 65_536);

    let mut request = json!({
        "type": "set_cell_style", "expected_revision": "resident/0", "collection": "rows",
        "entities": entities, "fields": ["a", "b", "c", "d"],
        "changes": [{"kind": "toggle_bold"}],
    });
    let oversized = serde_json::to_vec(&request).unwrap();
    assert!(oversized.len() > limit);
    let reply: serde_json::Value =
        serde_json::from_slice(&crate::process_wire_request(&mut runtime, &oversized)).unwrap();
    assert_eq!(reply["error"]["code"], "request_too_large");

    request.as_object_mut().unwrap().remove("entities");
    request["type"] = json!("set_cell_style_range");
    request["row"] = json!(1);
    request["row_count"] = json!(ROWS);
    let compact = serde_json::to_vec(&request).unwrap();
    assert!(compact.len() < limit);
    let reply: serde_json::Value =
        serde_json::from_slice(&crate::process_wire_request(&mut runtime, &compact)).unwrap();
    assert_eq!(reply["response"]["type"], "published", "{reply}");
    let mut runtime = runtime.unwrap();
    assert_eq!(runtime.undo.len(), 1);
    let styled = runtime.presentation.clone();
    assert_eq!(styled.cells.len(), ROWS * 4);
    assert!(
        styled
            .cells
            .values()
            .all(|id| styled.styles[id].bold == Some(true))
    );
    history(&mut runtime, false);
    assert!(runtime.presentation.is_empty());
    assert!(runtime.undo.is_empty());
    history(&mut runtime, true);
    assert_eq!(runtime.presentation, styled);
    assert_eq!(runtime.session.document(), &document);
}

#[test]
fn cell_style_range_uses_stored_rows_and_restores_mixed_sparse_styles() {
    let mut document = fixture();
    document.entities.get_mut("r3").unwrap().rank =
        Some(tachiko_workspace_engine::RowRank::new("A").unwrap());
    let mut runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
    edit(
        &mut runtime,
        &["r3"],
        &["c"],
        &json!([{"kind": "bold", "value": true}, {"kind": "italic", "value": true}]),
    )
    .unwrap();
    let before = runtime.query_table("rows").unwrap();
    let mut request = json!({
        "type": "set_cell_style_range", "expected_revision": runtime.current_revision(),
        "collection": "rows", "row": 1, "row_count": 2, "fields": ["c", "b"],
        "changes": [{"kind": "toggle_bold"}],
    });
    let publication = runtime
        .handle(serde_json::from_value(request.clone()).unwrap())
        .unwrap();
    assert!(matches!(publication, DesignerResponse::Published(_)));
    assert_eq!(runtime.undo.len(), 2);
    let after = runtime.query_table("rows").unwrap();
    for row in &after.rows {
        if row.id == "r2" {
            assert_eq!(row.styles, []);
        } else {
            assert!(!row.styles[0].bold);
            assert!(row.styles[1].bold && row.styles[2].bold);
            assert_eq!(row.styles[2].italic, row.id == "r3");
        }
    }
    let revision = runtime.current_revision().to_owned();
    request["expected_revision"] = json!(revision);
    request["row_count"] = json!(4);
    assert!(matches!(
        runtime.handle(serde_json::from_value(request).unwrap()),
        Err(DesignerError::InvalidTableOperation { .. })
    ));
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.undo.len(), 2);
    history(&mut runtime, false);
    assert_eq!(runtime.query_table("rows").unwrap().rows, before.rows);
    history(&mut runtime, true);
    assert_eq!(runtime.query_table("rows").unwrap().rows, after.rows);
    assert_eq!(runtime.session.document(), &document);
}

#[test]
fn rectangle_toggle_and_each_undo_are_presentation_only_in_every_profile() {
    for profile in [
        SessionProfile::Ordinary,
        SessionProfile::TextCapacity,
        SessionProfile::WorkingSize,
    ] {
        let mut document = fixture();
        if profile == SessionProfile::TextCapacity {
            for field in document
                .schemas
                .get_mut(&SchemaId::from("rows"))
                .unwrap()
                .fields
                .values_mut()
            {
                field.field_type = FieldType::Text;
            }
            for entity in document.entities.values_mut() {
                entity
                    .fields
                    .insert(FieldId::from("a"), Value::Text(entity.id.to_string()));
            }
        }
        let mut runtime =
            DesignerRuntime::from_document_profile(document, OCCURRENCE, profile).unwrap();
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
    assert_eq!(cleared.rows[0].id, "r2");
    assert_eq!(cleared.rows[0].styles.len(), 0);
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
    let saved = runtime.export_project_v3(&revision).unwrap();
    let (_, opened) = crate::admit_imported_project(
        &saved.bytes,
        OCCURRENCE,
        saved.import_metadata.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(opened.table.rows[0].styles, shown.rows[0].styles);
    assert_eq!(opened.table.rows[1].styles, shown.rows[1].styles);
    history(&mut runtime, false);
    assert_eq!(runtime.presentation, before);
}

#[test]
fn cell_style_fill_requires_explicit_color() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    edit(
        &mut runtime,
        &["r1"],
        &["b"],
        &json!([{"kind":"fill","color":"#FFFF00"}]),
    )
    .unwrap();
    let revision = runtime.current_revision().to_owned();
    let request = json!({
        "type": "set_cell_style", "expected_revision": revision,
        "collection": "rows", "entities": ["r1"], "fields": ["b"],
        "changes": [{"kind":"fill"}],
    });
    assert!(serde_json::from_value::<DesignerRequest>(request).is_err());
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(
        window(&mut runtime, &TableView::default()).rows[0].styles[1]
            .fill
            .as_deref(),
        Some("FFFF00")
    );
    edit(
        &mut runtime,
        &["r1"],
        &["b"],
        &json!([{"kind":"fill","color":null}]),
    )
    .unwrap();
}

#[test]
fn cell_style_rejects_projection_over_budget_before_publication() {
    let mut document = fixture();
    document.entities.clear();
    for index in 0..100 {
        let name = format!("r{index:03}");
        let id = EntityId::from(name.as_str());
        document.entities.insert(
            id.clone(),
            Entity {
                rank: None,
                id,
                key: EntityKey::from(name.as_str()),
                schema: SchemaId::from("rows"),
                fields: BTreeMap::from([(
                    FieldId::from("a"),
                    Value::Number(Number::new(1.0).unwrap()),
                )]),
            },
        );
    }
    document
        .schemas
        .get_mut(&SchemaId::from("rows"))
        .unwrap()
        .fields
        .get_mut(&FieldId::from("c"))
        .unwrap()
        .field_type = FieldType::Text;
    // Find a genuinely admitted table whose projected styles exceed the budget.
    let mut runtime = (0..=512)
        .step_by(16)
        .find_map(|length| {
            let mut candidate = document.clone();
            for entity in candidate.entities.values_mut() {
                entity
                    .fields
                    .insert(FieldId::from("c"), Value::Text("x".repeat(length)));
            }
            let runtime = DesignerRuntime::from_document(candidate, OCCURRENCE).ok()?;
            let mut opened = crate::OpenedProjection {
                bootstrap: runtime.bootstrap_projection(),
                table: runtime.query_table("rows").unwrap(),
            };
            for row in &mut opened.table.rows {
                row.styles = vec![crate::interop_adapter::CellStyle::default(); 3];
                row.styles[1].bold = true;
            }
            (crate::serialized_projection_bytes(&opened).unwrap() > crate::MAX_PROJECTION_BYTES)
                .then_some(runtime)
        })
        .expect("fixture must fit before styling and exceed the projection budget after styling");
    let revision = runtime.current_revision().to_owned();
    let names: Vec<String> = (0..100).map(|index| format!("r{index:03}")).collect();
    let entities: Vec<&str> = names.iter().map(String::as_str).collect();
    let result = edit(
        &mut runtime,
        &entities,
        &["b"],
        &json!([{"kind":"bold","value":true}]),
    );
    assert!(
        matches!(result, Err(DesignerError::ProjectionTooLarge { .. })),
        "unexpected style edit result: {result:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.undo.len(), 0);
    let result = edit(
        &mut runtime,
        &entities,
        &["b"],
        &json!([{"kind":"border","target":"all","value":true}]),
    );
    assert!(matches!(
        result,
        Err(DesignerError::ProjectionTooLarge { .. })
    ));
    assert_eq!(runtime.current_revision(), revision);
    assert!(runtime.presentation.is_empty());
    assert_eq!(runtime.undo.len(), 0);
    let result = runtime.set_column_wrap(&revision, "rows", "b", Some(true));
    assert!(
        matches!(result, Err(DesignerError::ProjectionTooLarge { .. })),
        "unexpected column style result: {result:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.undo.len(), 0);
    let exported = runtime.export_project_v3(&revision).unwrap();
    crate::admit_project(&exported.bytes, OCCURRENCE).unwrap();
    runtime.export_portable_ro(&revision).unwrap();
}

fn blank_style_document(rows: usize) -> Document {
    let mut document = fixture();
    let schema = document.schemas.get_mut(&SchemaId::from("rows")).unwrap();
    let field = schema.fields[&FieldId::from("a")].clone();
    schema.fields = "abcdefghijklmnop"
        .chars()
        .map(|name| {
            let name = name.to_string();
            let mut definition = field.clone();
            definition.id = FieldId::from(name.clone());
            definition.key = FieldKey::from(name);
            if definition.id == FieldId::from("c") {
                definition.field_type = FieldType::Text;
            }
            (definition.id.clone(), definition)
        })
        .collect();
    document.entities = (0..rows)
        .map(|index| {
            let name = format!("r{index:03}");
            let id = EntityId::from(name.clone());
            (
                id.clone(),
                Entity {
                    rank: None,
                    id,
                    key: EntityKey::from(name),
                    schema: SchemaId::from("rows"),
                    fields: BTreeMap::new(),
                },
            )
        })
        .collect();
    document
}

#[test]
fn cell_style_blank_rectangle_refuses_oversized_style_arrays() {
    let mut runtime = DesignerRuntime::from_document(blank_style_document(48), OCCURRENCE).unwrap();
    let revision = runtime.current_revision().to_owned();
    let names = runtime
        .query_table("rows")
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    let entities = names.iter().map(String::as_str).collect::<Vec<_>>();
    let error = edit(
        &mut runtime,
        &entities,
        &["b"],
        &json!([{"kind":"bold","value":true}]),
    )
    .unwrap_err();
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { .. }),
        "{error:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.undo.len(), 0);
    assert_eq!(runtime.redo.len(), 0);
    assert_eq!(
        runtime.presentation,
        tachiko_workspace_engine::PresentationStore::default()
    );
}

fn near_limit_styled_runtime() -> DesignerRuntime {
    let runtime = (1..=48)
        .rev()
        .find_map(|rows| {
            let mut runtime =
                DesignerRuntime::from_document(blank_style_document(rows), OCCURRENCE).unwrap();
            let revision = runtime.current_revision().to_owned();
            match runtime.set_column_wrap(&revision, "rows", "b", Some(true)) {
                Ok(_) => {}
                Err(DesignerError::ProjectionTooLarge { .. }) => return None,
                Err(error) => panic!("unexpected fixture failure: {error:?}"),
            }
            let table = runtime.query_table("rows").ok()?;
            let opened = crate::OpenedProjection {
                bootstrap: runtime.bootstrap_projection(),
                table,
            };
            crate::ensure_opened_wire_projection_size(&opened, runtime.profile).ok()?;
            assert!(crate::serialized_projection_bytes(&opened).unwrap() > 60_000);
            Some(runtime)
        })
        .expect("a styled table must fit just below the complete projection limit");
    assert_eq!(runtime.profile, SessionProfile::Ordinary);
    runtime
}

#[test]
fn cell_style_semantic_edits_retain_styles_during_budget_preflight() {
    let mut runtime = near_limit_styled_runtime();
    let request = |revision: &str, value: String| DesignerRequest::EditCells {
        expected_revision: revision.to_owned(),
        edits: vec![crate::CellEdit {
            target: crate::FieldTarget {
                entity: "r000".into(),
                field: "c".into(),
            },
            input: crate::ScalarEditInput::Text { value },
        }],
    };
    runtime
        .handle(request(runtime.current_revision(), "x".into()))
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    let undo = runtime.undo.len();
    let redo = runtime.redo.len();
    let table = runtime.query_table("rows").unwrap();
    let saved = runtime.export_project_v3(&revision).unwrap();
    let (_, mut opened) = crate::admit_project(&saved.bytes, OCCURRENCE).unwrap();
    // Reopen starts a new session revision; compare the persisted table content.
    opened.table.revision.clone_from(&table.revision);
    assert_eq!(opened.table, table);
    let portable = runtime.export_portable_ro(&revision).unwrap();
    let (_, mut opened) = admit_portable_ro(&portable.bytes, OCCURRENCE).unwrap();
    opened.table.revision.clone_from(&table.revision);
    assert_eq!(opened.table, table);
    let error = runtime
        .handle(request(&revision, "x".repeat(4096)))
        .unwrap_err();
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { .. }),
        "{error:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.undo.len(), undo);
    assert_eq!(runtime.redo.len(), redo);
    assert_eq!(runtime.query_table("rows").unwrap(), table);
    assert_eq!(
        runtime.export_project_v3(&revision).unwrap().bytes,
        saved.bytes
    );
}

#[test]
fn cell_style_formula_preflight_keeps_resident_styles() {
    let mut runtime = near_limit_styled_runtime();
    let target = crate::FieldTarget {
        entity: "r000".into(),
        field: "a".into(),
    };
    let revision = runtime.current_revision().to_owned();
    runtime.update_formula(&revision, &target, "1").unwrap();
    let padding = |revision: &str, value: String| DesignerRequest::EditCells {
        expected_revision: revision.to_owned(),
        edits: vec![crate::CellEdit {
            target: crate::FieldTarget {
                entity: "r000".into(),
                field: "c".into(),
            },
            input: crate::ScalarEditInput::Text { value },
        }],
    };
    runtime
        .handle(padding(runtime.current_revision(), String::new()))
        .unwrap();
    let opened = crate::OpenedProjection {
        bootstrap: runtime.bootstrap_projection(),
        table: runtime.query_table("rows").unwrap(),
    };
    let bytes = crate::serialized_projection_bytes(&opened).unwrap();
    runtime
        .handle(padding(
            runtime.current_revision(),
            "x".repeat(crate::MAX_PROJECTION_BYTES - bytes - 128),
        ))
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime.update_formula(&revision, &target, "2").unwrap();
    let revision = runtime.current_revision().to_owned();
    let table = runtime.query_table("rows").unwrap();
    let saved = runtime.export_project_v3(&revision).unwrap();
    let presentation = runtime.presentation.clone();
    let undo = runtime.undo.len();
    let redo = runtime.redo.len();
    let longer = vec!["1234567890"; 32].join("+");
    // The same formula fits without styles, so this exercises styled projection admission.
    let mut unstyled =
        DesignerRuntime::from_document(runtime.session.document().clone(), OCCURRENCE).unwrap();
    let unstyled_revision = unstyled.current_revision().to_owned();
    unstyled
        .update_formula(&unstyled_revision, &target, &longer)
        .unwrap();
    let error = runtime
        .update_formula(&revision, &target, &longer)
        .unwrap_err();
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { .. }),
        "{error:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.query_table("rows").unwrap(), table);
    assert_eq!(runtime.presentation, presentation);
    assert_eq!(runtime.undo.len(), undo);
    assert_eq!(runtime.redo.len(), redo);
    assert_eq!(
        runtime.export_project_v3(&revision).unwrap().bytes,
        saved.bytes
    );
    crate::admit_project(&saved.bytes, OCCURRENCE).unwrap();
}

#[test]
fn auto_percentage_preflight_measures_new_styles_on_an_unstyled_table() {
    let mut document = blank_style_document(48);
    document
        .schemas
        .get_mut(&SchemaId::from("rows"))
        .unwrap()
        .fields
        .get_mut(&FieldId::from("a"))
        .unwrap()
        .field_type = FieldType::Auto;
    let mut runtime = DesignerRuntime::from_document(document.clone(), OCCURRENCE).unwrap();
    assert!(runtime.presentation.is_empty());
    let edits = document
        .entities
        .keys()
        .map(|entity| crate::CellEdit {
            target: crate::FieldTarget {
                entity: entity.to_string(),
                field: "a".into(),
            },
            input: crate::ScalarEditInput::Auto {
                input: "25%".into(),
                date_order: None,
            },
        })
        .collect::<Vec<_>>();
    // The numeric values alone fit; the percentage styles expand every wide row.
    let mut values_only = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    values_only
        .handle(DesignerRequest::EditCells {
            expected_revision: values_only.current_revision().to_owned(),
            edits: edits
                .iter()
                .map(|edit| crate::CellEdit {
                    target: edit.target.clone(),
                    input: crate::ScalarEditInput::Number {
                        input: "0.25".into(),
                    },
                })
                .collect(),
        })
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    let table = runtime.query_table("rows").unwrap();
    let saved = runtime.export_project_v3(&revision).unwrap();
    let error = runtime
        .handle(DesignerRequest::EditCells {
            expected_revision: revision.clone(),
            edits,
        })
        .unwrap_err();
    assert!(
        matches!(error, DesignerError::ProjectionTooLarge { .. }),
        "{error:?}"
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.query_table("rows").unwrap(), table);
    assert!(runtime.presentation.is_empty());
    assert!(runtime.undo.is_empty());
    assert!(runtime.redo.is_empty());
    assert_eq!(
        runtime.export_project_v3(&revision).unwrap().bytes,
        saved.bytes
    );
}

#[test]
fn imported_text_capacity_style_edits_refuse_before_publication() {
    for rows in [2, crate::interop_adapter::MAX_DATA_ROWS + 1] {
        let csv = format!("Value\n{}", "x\n".repeat(rows));
        let source = crate::interop_adapter::import_csv(
            csv.as_bytes(),
            &crate::interop_adapter::ImportOptions::default(),
        )
        .unwrap();
        let (mut runtime, imported) = crate::import_workbook(
            &source,
            &crate::ImportSelection {
                column_types: vec![vec![crate::ImportFieldType::Text]],
                extra_columns: vec![Vec::new()],
            },
            OCCURRENCE,
        )
        .unwrap();
        let capacity = rows > crate::interop_adapter::MAX_DATA_ROWS;
        assert_eq!(
            runtime.profile,
            if capacity {
                SessionProfile::TextCapacity
            } else {
                SessionProfile::Ordinary
            }
        );
        let sheet = &imported.metadata.sheets[0];
        for change in [
            json!({"kind": "bold", "value": true}),
            json!({"kind": "fill", "color": "#FFFF00"}),
        ] {
            let revision = runtime.current_revision().to_owned();
            let table = runtime.query_table(&sheet.schema_id).unwrap();
            let saved = runtime.export_project_v3(&revision).unwrap();
            let presentation = runtime.presentation.clone();
            let undo = runtime.undo.len();
            let redo = runtime.redo.len();
            let result = runtime.handle(
                serde_json::from_value(json!({
                    "type": "set_cell_style", "expected_revision": revision,
                    "collection": sheet.schema_id, "entities": [sheet.rows[0].entity_id],
                    "fields": [sheet.columns[0].field_id], "changes": [change],
                }))
                .unwrap(),
            );
            if capacity {
                let error = result.unwrap_err();
                assert!(error.to_string().contains("capacity"), "{error:?}");
                assert_eq!(runtime.current_revision(), revision);
                assert_eq!(runtime.query_table(&sheet.schema_id).unwrap(), table);
                assert_eq!(runtime.presentation, presentation);
                assert_eq!(runtime.undo.len(), undo);
                assert_eq!(runtime.redo.len(), redo);
                assert_eq!(
                    runtime.export_project_v3(&revision).unwrap().bytes,
                    saved.bytes
                );
            } else {
                result.unwrap();
                let revision = runtime.current_revision().to_owned();
                let saved = runtime.export_project_v3(&revision).unwrap();
                let (_, opened) = crate::admit_imported_project(
                    &saved.bytes,
                    OCCURRENCE,
                    saved.import_metadata.as_ref().unwrap(),
                )
                .unwrap();
                assert_eq!(
                    opened.table.rows,
                    runtime.query_table(&sheet.schema_id).unwrap().rows
                );
            }
        }
    }
}
