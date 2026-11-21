use serde_json::json;
use tachiko_workspace_engine::{BorderSide, CellStyle, FieldId, SchemaId, TablePresentation};

use crate::cell_style_tests::{edit, fixture, history, window};
use crate::interop_adapter::{
    CellStyle as SourceStyle, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue,
    SourceWorkbook, export_xlsx, import_xlsx,
};
use crate::{DesignerRuntime, ImportFieldType, ImportSelection, SessionProfile, TableView};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000889";

#[test]
fn outline_save_reopen_export_and_undo_preserve_exact_sides_and_slice_one_styles() {
    for profile in [SessionProfile::Ordinary, SessionProfile::WorkingSize] {
        let mut runtime =
            DesignerRuntime::from_document_profile(fixture(), OCCURRENCE, profile).unwrap();
        edit(
            &mut runtime,
            &["r1", "r2", "r3"],
            &["b", "c"],
            &json!([
                {"kind":"bold", "value":true}, {"kind":"italic", "value":true},
                {"kind":"fill", "color":"#FFFF00"}
            ]),
        )
        .unwrap();
        let before = runtime.presentation.clone();
        let document = runtime.session.document().clone();
        let publication = edit(
            &mut runtime,
            &["r1", "r2", "r3"],
            &["b", "c"],
            &json!([
                {"kind":"border", "target":"outline", "value":true}
            ]),
        )
        .unwrap();
        assert!(publication.affected_calculations.is_empty());
        assert_eq!(runtime.undo.len(), 2);
        assert_eq!(runtime.session.document(), &document);
        let expected = vec![
            vec![
                vec![BorderSide::Top, BorderSide::Left],
                vec![BorderSide::Top, BorderSide::Right],
            ],
            vec![vec![BorderSide::Left], vec![BorderSide::Right]],
            vec![
                vec![BorderSide::Bottom, BorderSide::Left],
                vec![BorderSide::Right, BorderSide::Bottom],
            ],
        ];
        let shown = window(&mut runtime, &TableView::default());
        let sides: Vec<Vec<_>> = shown
            .rows
            .iter()
            .map(|row| {
                row.styles[1..]
                    .iter()
                    .map(|style| style.border_sides.clone())
                    .collect()
            })
            .collect();
        assert_eq!(sides, expected);
        let saved = runtime
            .export_portable_ro(runtime.current_revision())
            .unwrap();
        let (reopened, _) = crate::admit_portable_ro(&saved.bytes, OCCURRENCE).unwrap();
        assert_eq!(reopened.presentation, runtime.presentation);
        let workbook = reopened
            .export_native_tables_workbook(reopened.current_revision(), "rows")
            .unwrap();
        let imported = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
        assert!(!imported.ledger.iter().any(|finding| finding.blocking));
        for (row, expected_row) in imported.sheets[0].rows.iter().zip(&expected) {
            assert_eq!(row[0].style, SourceStyle::default());
            for (cell, sides) in row[1..].iter().zip(expected_row) {
                assert!(cell.style.bold && cell.style.italic && cell.style.border);
                assert!(cell.style.fill.as_deref().unwrap().ends_with("FFFF00"));
                assert_eq!(&cell.style.border_sides, sides);
            }
        }
        history(&mut runtime, false);
        assert_eq!(runtime.presentation, before);
        history(&mut runtime, true);
        assert_eq!(runtime.presentation, reopened.presentation);
    }
}

#[test]
fn per_side_all_and_clear_override_inherited_borders_without_changing_neighbours() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    let style = CellStyle {
        borders: Some(vec![BorderSide::Top, BorderSide::Right]),
        ..CellStyle::default()
    };
    let id = tachiko_storage::style_id(&style).unwrap();
    runtime.presentation.styles.insert(id.clone(), style);
    runtime.presentation.tables.insert(
        SchemaId::from("rows"),
        TablePresentation {
            column_styles: [(FieldId::from("b"), id)].into(),
            ..TablePresentation::default()
        },
    );
    for (target, value, expected) in [
        ("top", false, vec![BorderSide::Right]),
        ("left", true, vec![BorderSide::Right, BorderSide::Left]),
        (
            "bottom",
            true,
            vec![BorderSide::Right, BorderSide::Bottom, BorderSide::Left],
        ),
        ("right", false, vec![BorderSide::Bottom, BorderSide::Left]),
        (
            "all",
            true,
            vec![
                BorderSide::Top,
                BorderSide::Right,
                BorderSide::Bottom,
                BorderSide::Left,
            ],
        ),
        ("all", false, vec![]),
    ] {
        edit(
            &mut runtime,
            &["r2"],
            &["b"],
            &json!([
                {"kind":"border", "target":target, "value":value}
            ]),
        )
        .unwrap();
        let shown = window(&mut runtime, &TableView::default());
        let selected = shown.rows[1].styles.get(1).cloned().unwrap_or_default();
        assert_eq!(selected.border_sides, expected);
        assert_eq!(selected.border, !expected.is_empty());
        for index in [0, 2] {
            assert_eq!(
                shown.rows[index].styles[1].border_sides,
                [BorderSide::Top, BorderSide::Right]
            );
        }
    }
    let revision = runtime.current_revision().to_owned();
    let undo = runtime.undo.len();
    let error = edit(
        &mut runtime,
        &["r2"],
        &["b"],
        &json!([
            {"kind":"border", "target":"all", "value":false}
        ]),
    )
    .unwrap_err();
    assert_eq!(error.failure_projection(&revision).code, "no_change");
    assert_eq!(runtime.undo.len(), undo);
    history(&mut runtime, false);
    assert!(window(&mut runtime, &TableView::default()).rows[1].styles[1].border);
}

#[test]
fn outline_uses_selection_order_and_clear_preserves_unselected_sides() {
    let mut runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    edit(
        &mut runtime,
        &["r3", "r1"],
        &["c", "b"],
        &json!([
            {"kind":"border", "target":"outline", "value":true}
        ]),
    )
    .unwrap();
    let shown = window(&mut runtime, &TableView::default());
    assert_eq!(
        shown.rows[2].styles[2].border_sides,
        [BorderSide::Top, BorderSide::Left]
    );
    assert_eq!(
        shown.rows[0].styles[1].border_sides,
        [BorderSide::Right, BorderSide::Bottom]
    );
    assert!(shown.rows[1].styles.is_empty());
    edit(
        &mut runtime,
        &["r3", "r1"],
        &["c", "b"],
        &json!([
            {"kind":"border", "target":"bottom", "value":true},
            {"kind":"border", "target":"outline", "value":false}
        ]),
    )
    .unwrap();
    let shown = window(&mut runtime, &TableView::default());
    assert_eq!(shown.rows[2].styles[2].border_sides, [BorderSide::Bottom]);
    assert!(shown.rows[0].styles.is_empty());
    edit(
        &mut runtime,
        &["r2"],
        &["a"],
        &json!([
            {"kind":"border", "target":"outline", "value":true}
        ]),
    )
    .unwrap();
    assert_eq!(
        window(&mut runtime, &TableView::default()).rows[1].styles[0]
            .border_sides
            .len(),
        4
    );
}

fn source_fixture() -> SourceWorkbook {
    SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Imported".into(),
            has_header: true,
            columns: ["A", "B", "C"]
                .into_iter()
                .map(|name| SourceColumn {
                    name: name.into(),
                    width: None,
                    number_format: None,
                })
                .collect(),
            rows: (0..3)
                .map(|_| {
                    (0..3)
                        .map(|_| SourceCell {
                            value: SourceValue::Number { value: 1.0 },
                            formula: None,
                            style: SourceStyle {
                                border: true,
                                border_sides: vec![BorderSide::Left],
                                italic: true,
                                fill: Some("123ABC".into()),
                                ..SourceStyle::default()
                            },
                        })
                        .collect()
                })
                .collect(),
            layout: SourceLayout::default(),
        }],
        ledger: Vec::new(),
    }
}

#[test]
fn imported_styles_are_promoted_edited_saved_and_exported_in_both_profiles() {
    let source = import_xlsx(&export_xlsx(&source_fixture()).unwrap()).unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Number; 3]],
        extra_columns: vec![Vec::new()],
    };
    for working in [false, true] {
        let (mut runtime, metadata) = if working {
            let (runtime, opened) =
                crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
            (runtime, opened.metadata)
        } else {
            let (runtime, opened) =
                crate::import_workbook(&source, &selection, OCCURRENCE).unwrap();
            (runtime, opened.metadata)
        };
        let sheet = &metadata.sheets[0];
        let fields: Vec<_> = sheet.columns[1..]
            .iter()
            .map(|column| &column.field_id)
            .collect();
        let entities: Vec<_> = sheet.rows.iter().map(|row| &row.entity_id).collect();
        let before = runtime.presentation.clone();
        let request = serde_json::from_value(json!({
            "type":"set_cell_style", "expected_revision":runtime.current_revision(),
            "collection":sheet.schema_id, "entities":entities, "fields":fields,
            "changes":[{"kind":"border", "target":"all", "value":false},
                {"kind":"border", "target":"outline", "value":true},
                {"kind":"bold", "value":true}, {"kind":"fill", "color":"#FFFF00"}]
        }))
        .unwrap();
        runtime.handle(request).unwrap();
        assert_eq!(runtime.undo.len(), 1);
        let saved = runtime
            .export_portable_ro(runtime.current_revision())
            .unwrap();
        let (reopened, _) = crate::admit_portable_ro(&saved.bytes, OCCURRENCE).unwrap();
        let workbook = reopened
            .export_workbook(reopened.current_revision(), &metadata)
            .unwrap();
        let result = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
        let rows = &result.sheets[0].rows;
        assert_eq!(rows[1][1].style.border_sides, [BorderSide::Left]);
        assert_eq!(rows[1][2].style.border_sides, [BorderSide::Right]);
        assert_eq!(
            rows[0][1].style.border_sides,
            [BorderSide::Top, BorderSide::Left]
        );
        assert_eq!(
            rows[2][2].style.border_sides,
            [BorderSide::Right, BorderSide::Bottom]
        );
        for row in rows {
            assert_eq!(row[0].style.border_sides, [BorderSide::Left]);
            assert!(row[0].style.fill.as_deref().unwrap().ends_with("123ABC"));
            for cell in &row[1..] {
                assert!(cell.style.bold && cell.style.italic);
                assert!(cell.style.fill.as_deref().unwrap().ends_with("FFFF00"));
            }
        }
        history(&mut runtime, false);
        assert_eq!(runtime.presentation, before);
    }
}

#[test]
fn border_requires_explicit_target_and_value() {
    for change in [
        json!({"kind": "border", "target": "all"}),
        json!({"kind": "border", "value": false}),
        json!({"kind": "border", "target": "all", "value": null}),
    ] {
        assert!(serde_json::from_value::<crate::CellStyleChange>(change).is_err());
    }
}
