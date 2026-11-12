use std::collections::BTreeMap;

use tachiko_workspace_engine::{
    CellAlignment, CellStyle as StoredStyle, Date, DateFormat, Document, Entity, EntityId,
    EntityKey, FieldConstraint, FieldDefinition, FieldType, PresentationStore, Schema, SchemaId,
    SchemaKey, TablePresentation, Value,
};

use crate::interop_adapter::{
    CellStyle, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue, SourceWorkbook,
    export_csv, export_xlsx, import_xlsx,
};
use crate::interop_document::ImportedOutput;
use crate::{
    DesignerRequest, DesignerResponse, DesignerRuntime, ImportFieldType, ImportSelection,
    import_workbook,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000668";

fn field(id: &str, field_type: FieldType, date_format: Option<DateFormat>) -> FieldDefinition {
    FieldDefinition {
        id: id.into(),
        key: id.into(),
        display_name: None,
        field_type,
        required: false,
        constraint: FieldConstraint::None,
        number_format: None,
        date_format,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Exercise projection and the encoded round trip together.
fn native_presentation_is_projected_and_survives_xlsx() {
    let mut document = Document::empty("presentation", "Presentation");
    let fields = [
        field("a-date", FieldType::Date, Some(DateFormat::MonthDayYear)),
        field("b-text", FieldType::Text, None),
    ];
    let schema = Schema {
        id: "table".into(),
        key: SchemaKey::from("table"),
        display_name: Some("Table".into()),
        field_order: None,
        fields: fields
            .into_iter()
            .map(|field| (field.id.clone(), field))
            .collect(),
    };
    document.schemas.insert(schema.id.clone(), schema);
    for id in ["row1", "row2"] {
        let entity = Entity {
            id: id.into(),
            key: EntityKey::from(id),
            schema: "table".into(),
            fields: BTreeMap::from([
                (
                    "a-date".into(),
                    Value::Date(Date::parse("2024-02-29").unwrap()),
                ),
                ("b-text".into(), Value::Text(id.into())),
            ]),
        };
        document.entities.insert(entity.id.clone(), entity);
    }
    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    let column = StoredStyle {
        alignment: Some(CellAlignment::Center),
        wrap: Some(true),
        ..Default::default()
    };
    let cell = StoredStyle {
        alignment: Some(CellAlignment::Right),
        ..Default::default()
    };
    let date = StoredStyle {
        date_format: Some(DateFormat::Dmy),
        ..Default::default()
    };
    let column_id = tachiko_storage::style_id(&column).unwrap();
    let cell_id = tachiko_storage::style_id(&cell).unwrap();
    let date_id = tachiko_storage::style_id(&date).unwrap();
    let mut store = PresentationStore::default();
    store.styles.extend([
        (column_id.clone(), column),
        (cell_id.clone(), cell),
        (date_id.clone(), date),
    ]);
    store.tables.insert(
        SchemaId::from("table"),
        TablePresentation {
            column_styles: BTreeMap::from([("b-text".into(), column_id)]),
            column_widths: BTreeMap::from([("a-date".into(), 2560)]),
            ..Default::default()
        },
    );
    store
        .cells
        .insert((EntityId::from("row2"), "b-text".into()), cell_id);
    store
        .cells
        .insert((EntityId::from("row2"), "a-date".into()), date_id);
    runtime.set_presentation(store).unwrap();
    let workbook = runtime
        .export_native_tables_workbook("resident/0", "table")
        .unwrap();
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.columns[0].width, Some(10.0));
    assert_eq!(
        sheet.rows[0][0].style.number_format.as_deref(),
        Some("mmm d, yyyy")
    );
    assert_eq!(
        sheet.rows[1][0].style.number_format.as_deref(),
        Some("d/m/yyyy")
    );
    assert_eq!(sheet.rows[0][1].style.alignment.as_deref(), Some("center"));
    assert_eq!(sheet.rows[1][1].style.alignment.as_deref(), Some("right"));
    assert!(sheet.rows.iter().all(|row| row[1].style.wrap));
    let reopened = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
    let sheet = &reopened.sheets[0];
    assert_eq!(sheet.columns[0].width, Some(10.0));
    for (row, pattern) in sheet.rows.iter().zip(["mmm d, yyyy", "d/m/yyyy"]) {
        assert!(matches!(&row[0].value, SourceValue::Date { .. }));
        assert_eq!(row[0].style.number_format.as_deref(), Some(pattern));
        assert!(row[1].style.wrap);
    }
    assert_eq!(sheet.rows[0][1].style.alignment.as_deref(), Some("center"));
    assert_eq!(sheet.rows[1][1].style.alignment.as_deref(), Some("right"));
    let csv = runtime
        .export_native_tables_csv_workbook("resident/0", "table")
        .unwrap();
    assert_eq!(csv.sheets[0].columns[0].width, None);
    assert!(
        csv.sheets[0]
            .rows
            .iter()
            .flatten()
            .all(|cell| cell.style == CellStyle::default())
    );
}

#[test]
#[allow(clippy::too_many_lines)] // Import, store override, clear and CSV in one journey.
fn imported_store_overrides_metadata_and_csv_ignores_presentation() {
    let source = SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Dates".into(),
            has_header: true,
            columns: vec![SourceColumn {
                name: "Date".into(),
                width: Some(15.0),
                number_format: None,
            }],
            rows: vec![vec![SourceCell {
                value: SourceValue::Date {
                    value: "2024-02-29".into(),
                },
                style: CellStyle {
                    number_format: Some("m/d/yyyy".into()),
                    ..Default::default()
                },
                formula: None,
            }]],
            layout: SourceLayout::default(),
        }],
        ledger: vec![],
    };
    let source = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    let (mut runtime, imported) = import_workbook(
        &source,
        &ImportSelection {
            column_types: vec![vec![ImportFieldType::Date]],
            extra_columns: vec![vec![]],
        },
        OCCURRENCE,
    )
    .unwrap();
    let metadata = &imported.metadata;
    let sheet = &metadata.sheets[0];
    let collection = imported.opened.bootstrap.default_collection.clone();
    let field = sheet.columns[0].field_id.clone();
    let csv_before = runtime
        .export_imported_workbook(
            "resident/0",
            metadata,
            ImportedOutput::Csv(&sheet.schema_id),
        )
        .unwrap();
    let mut store = PresentationStore::default();
    store.tables.insert(
        SchemaId::from(sheet.schema_id.clone()),
        TablePresentation {
            column_widths: BTreeMap::from([(field.clone().into(), 2560)]),
            ..Default::default()
        },
    );
    runtime.set_presentation(store).unwrap();
    let DesignerResponse::Published(set) = runtime
        .handle(DesignerRequest::SetColumnDateFormat {
            expected_revision: "resident/0".into(),
            collection: collection.clone(),
            field: field.clone(),
            date_format: Some(DateFormat::DayMonthYear),
        })
        .unwrap()
    else {
        panic!("date format publishes")
    };
    let exported = runtime
        .export_workbook(&set.resulting_revision, metadata)
        .unwrap();
    assert_eq!(exported.sheets[0].columns[0].width, Some(10.0));
    assert_eq!(
        exported.sheets[0].rows[0][0].style.number_format.as_deref(),
        Some("d mmm yyyy")
    );
    let csv_after = runtime
        .export_imported_workbook(
            &set.resulting_revision,
            metadata,
            ImportedOutput::Csv(&sheet.schema_id),
        )
        .unwrap();
    assert_eq!(
        export_csv(&csv_before.sheets[0]).unwrap(),
        export_csv(&csv_after.sheets[0]).unwrap()
    );
    assert_eq!(csv_after.sheets[0].columns[0].width, None);
    assert_eq!(csv_after.sheets[0].rows[0][0].style, CellStyle::default());
    let DesignerResponse::Published(cleared) = runtime
        .handle(DesignerRequest::SetColumnDateFormat {
            expected_revision: set.resulting_revision,
            collection,
            field,
            date_format: None,
        })
        .unwrap()
    else {
        panic!("clearing date format publishes")
    };
    let exported = runtime
        .export_workbook(&cleared.resulting_revision, metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][0].style.number_format.as_deref(),
        Some("yyyy-mm-dd")
    );
}

#[test]
fn cell_wrap_false_overrides_column_and_incoming_style() {
    let field = field("text", FieldType::Text, None);
    let column = StoredStyle {
        wrap: Some(true),
        ..Default::default()
    };
    let cell = StoredStyle {
        wrap: Some(false),
        ..Default::default()
    };
    let column_id = tachiko_storage::style_id(&column).unwrap();
    let cell_id = tachiko_storage::style_id(&cell).unwrap();
    let mut store = PresentationStore::default();
    store
        .styles
        .extend([(column_id.clone(), column), (cell_id.clone(), cell)]);
    store.tables.insert(
        "table".into(),
        TablePresentation {
            column_styles: BTreeMap::from([(field.id.clone(), column_id)]),
            ..Default::default()
        },
    );
    store
        .cells
        .insert(("row".into(), field.id.clone()), cell_id);
    let style = crate::interop_date_format::cell_style(
        &store,
        &SchemaId::from("table"),
        &field,
        &EntityId::from("row"),
        &SourceValue::Text {
            value: "text".into(),
        },
        CellStyle {
            wrap: true,
            ..Default::default()
        },
    );
    assert!(!style.wrap);
}

fn import_presentation_fixture(patterns: &[&str]) -> (SourceWorkbook, ImportSelection) {
    let rows = patterns
        .iter()
        .enumerate()
        .map(|(index, pattern)| {
            vec![
                SourceCell {
                    value: SourceValue::Date {
                        value: "2024-02-29".into(),
                    },
                    style: CellStyle {
                        number_format: Some((*pattern).into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                SourceCell {
                    value: SourceValue::Text {
                        value: format!("row {index}"),
                    },
                    style: CellStyle {
                        alignment: Some(if index == 1 { "right" } else { "center" }.into()),
                        wrap: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ]
        })
        .collect();
    (
        SourceWorkbook {
            sheets: vec![SourceSheet {
                name: "Dates".into(),
                has_header: true,
                columns: vec![
                    SourceColumn {
                        name: "Date".into(),
                        width: Some(12.5),
                        number_format: None,
                    },
                    SourceColumn {
                        name: "Text".into(),
                        width: None,
                        number_format: None,
                    },
                ],
                rows,
                layout: SourceLayout::default(),
            }],
            ledger: vec![],
        },
        ImportSelection {
            column_types: vec![vec![ImportFieldType::Date, ImportFieldType::Text]],
            extra_columns: vec![vec![]],
        },
    )
}

fn assert_promoted(runtime: &DesignerRuntime, metadata: &crate::interop_document::InteropMetadata) {
    let sheet = &metadata.sheets[0];
    let schema = SchemaId::from(sheet.schema_id.clone());
    let date = tachiko_workspace_engine::FieldId::from(sheet.columns[0].field_id.clone());
    let text = tachiko_workspace_engine::FieldId::from(sheet.columns[1].field_id.clone());
    let snapshot = runtime.session.export_snapshot();
    assert_eq!(
        snapshot.document().schemas[&schema].fields[&date].date_format,
        Some(DateFormat::MonthDayYear)
    );
    let table = &runtime.presentation.tables[&schema];
    assert_eq!(table.column_widths[&date], 3200);
    let column = &runtime.presentation.styles[&table.column_styles[&text]];
    assert_eq!(column.alignment, None);
    assert_eq!(column.wrap, Some(true));
    for (entity, alignment) in snapshot
        .document()
        .entities
        .values()
        .zip([CellAlignment::Center, CellAlignment::Right])
    {
        let key = (entity.id.clone(), text.clone());
        assert_eq!(
            runtime.presentation.styles[&runtime.presentation.cells[&key]].alignment,
            Some(alignment)
        );
    }
    assert_eq!(sheet.columns[0].width, None);
}

#[test]
fn import_presentation_round_trip_clear_and_reopen() {
    let (source, selection) = import_presentation_fixture(&["mmm d, yyyy", "mmm d, yyyy"]);
    let (mut runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    assert_promoted(&runtime, &imported.metadata);
    let workbook = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    let reread = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
    assert_eq!(reread.sheets[0].columns[0].width, Some(12.5));
    for (row, alignment) in reread.sheets[0].rows.iter().zip(["center", "right"]) {
        assert_eq!(row[0].style.number_format.as_deref(), Some("mmm d, yyyy"));
        assert_eq!(row[1].style.alignment.as_deref(), Some(alignment));
        assert!(row[1].style.wrap);
    }
    let saved = runtime
        .export_project_v3(runtime.current_revision())
        .unwrap();
    let mut reopened = None;
    crate::open_project(&mut reopened, &saved.bytes, OCCURRENCE).unwrap();
    assert_promoted(reopened.as_ref().unwrap(), &imported.metadata);
    let revision = runtime.current_revision().to_owned();
    let DesignerResponse::Published(cleared) = runtime
        .handle(DesignerRequest::SetColumnDateFormat {
            expected_revision: revision,
            collection: imported.opened.bootstrap.default_collection.clone(),
            field: imported.metadata.sheets[0].columns[0].field_id.clone(),
            date_format: None,
        })
        .unwrap()
    else {
        panic!("clear publishes")
    };
    let result = runtime
        .export_workbook(&cleared.resulting_revision, &imported.metadata)
        .unwrap();
    assert_eq!(
        result.sheets[0].rows[0][0].style.number_format.as_deref(),
        Some("yyyy-mm-dd")
    );
}

#[test]
fn a_blank_cell_with_its_own_date_format_keeps_the_column_mixed() {
    let (mut source, selection) = import_presentation_fixture(&["mmm d, yyyy", "yyyy-mm-dd"]);
    source.sheets[0].rows[1][0].value = SourceValue::Empty;
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let sheet = &imported.metadata.sheets[0];
    let schema = SchemaId::from(sheet.schema_id.clone());
    let field = tachiko_workspace_engine::FieldId::from(sheet.columns[0].field_id.clone());
    let snapshot = runtime.session.export_snapshot();
    // The blank cell's ISO format would be lost under a MonthDayYear field.
    assert_eq!(
        snapshot.document().schemas[&schema].fields[&field].date_format,
        None
    );
    let filled = (
        EntityId::from(sheet.rows[0].entity_id.clone()),
        field.clone(),
    );
    let blank = (
        EntityId::from(sheet.rows[1].entity_id.clone()),
        field.clone(),
    );
    assert_eq!(
        runtime.presentation.styles[&runtime.presentation.cells[&filled]].date_format,
        Some(DateFormat::MonthDayYear)
    );
    assert!(
        runtime
            .presentation
            .cells
            .get(&blank)
            .is_none_or(|id| runtime.presentation.styles[id].date_format.is_none())
    );
    assert_eq!(
        sheet.rows[1].styles[0].number_format.as_deref(),
        Some("yyyy-mm-dd")
    );
    let exported = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    let rows = &exported.sheets[0].rows;
    assert_eq!(
        rows[0][0].style.number_format.as_deref(),
        Some("mmm d, yyyy")
    );
    assert_eq!(
        rows[1][0].style.number_format.as_deref(),
        Some("yyyy-mm-dd")
    );
}

#[test]
fn import_mixed_and_unsupported_date_formats() {
    let (source, selection) = import_presentation_fixture(&["m/d/yyyy", "d mmm yyyy"]);
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let sheet = &imported.metadata.sheets[0];
    let schema = SchemaId::from(sheet.schema_id.clone());
    let field = tachiko_workspace_engine::FieldId::from(sheet.columns[0].field_id.clone());
    let snapshot = runtime.session.export_snapshot();
    assert_eq!(
        snapshot.document().schemas[&schema].fields[&field].date_format,
        None
    );
    for (row, format) in sheet
        .rows
        .iter()
        .zip([DateFormat::Mdy, DateFormat::DayMonthYear])
    {
        let key = (EntityId::from(row.entity_id.clone()), field.clone());
        assert_eq!(
            runtime.presentation.styles[&runtime.presentation.cells[&key]].date_format,
            Some(format)
        );
    }
    let exported = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    for (row, pattern) in exported.sheets[0]
        .rows
        .iter()
        .zip(["m/d/yyyy", "d mmm yyyy"])
    {
        assert_eq!(row[0].style.number_format.as_deref(), Some(pattern));
    }
    let (source, selection) = import_presentation_fixture(&["mm/dd/yyyy", "mm/dd/yyyy"]);
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    assert_eq!(
        imported
            .ledger
            .iter()
            .filter(|finding| finding.code == "date_format_not_supported")
            .count(),
        1
    );
    let sheet = &imported.metadata.sheets[0];
    let snapshot = runtime.session.export_snapshot();
    assert_eq!(
        snapshot.document().schemas[&SchemaId::from(sheet.schema_id.clone())].fields
            [&tachiko_workspace_engine::FieldId::from(sheet.columns[0].field_id.clone())]
            .date_format,
        None
    );
    let exported = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][0].style.number_format.as_deref(),
        Some("yyyy-mm-dd")
    );
}

#[test]
fn working_size_import_promotes_presentation() {
    let (source, selection) = import_presentation_fixture(&["mmm d, yyyy", "mmm d, yyyy"]);
    let (runtime, imported) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    assert_promoted(&runtime, &imported.metadata);
}

fn stage5_fixture() -> (SourceWorkbook, ImportSelection) {
    let numeric = crate::interop_number_format::from_excel("#,##0.00").unwrap();
    let rows = (0..3)
        .map(|index| {
            vec![
                SourceCell {
                    value: SourceValue::Text {
                        value: format!("uniform {index}"),
                    },
                    style: CellStyle {
                        bold: true,
                        fill: Some("FFFFFF00".into()),
                        border: true,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                SourceCell {
                    value: SourceValue::Text {
                        value: format!("mixed {index}"),
                    },
                    style: CellStyle {
                        bold: index == 0,
                        fill: (index == 1).then(|| "FF00B050".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                SourceCell {
                    value: SourceValue::Number { value: 1.0 },
                    style: CellStyle {
                        number_format: Some(if index == 1 { "0%" } else { "#,##0.00" }.into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ]
        })
        .collect();
    (
        SourceWorkbook {
            sheets: vec![SourceSheet {
                name: "Styles".into(),
                has_header: true,
                columns: vec![
                    SourceColumn {
                        name: "Uniform".into(),
                        width: Some(12.0),
                        number_format: None,
                    },
                    SourceColumn {
                        name: "Mixed".into(),
                        width: None,
                        number_format: None,
                    },
                    SourceColumn {
                        name: "Number".into(),
                        width: None,
                        number_format: Some(numeric),
                    },
                ],
                rows,
                layout: SourceLayout::default(),
            }],
            ledger: vec![],
        },
        ImportSelection {
            column_types: vec![vec![
                ImportFieldType::Text,
                ImportFieldType::Text,
                ImportFieldType::Number,
            ]],
            extra_columns: vec![vec![]],
        },
    )
}

#[test]
fn blank_number_cell_keeps_its_format_through_xlsx() {
    let (mut source, selection) = stage5_fixture();
    source.sheets[0].rows[1][2].value = SourceValue::Empty;
    let source = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    assert_eq!(
        source.sheets[0].columns[2].number_format,
        crate::interop_number_format::from_excel("#,##0.00")
    );
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let workbook = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    let reread = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
    assert!(matches!(
        reread.sheets[0].rows[1][2].value,
        SourceValue::Empty
    ));
    assert_eq!(
        reread.sheets[0].rows[1][2].style.number_format.as_deref(),
        Some("0%")
    );
}

fn metadata_v2_fixture() -> (SourceWorkbook, ImportSelection) {
    let (mut source, mut selection) = stage5_fixture();
    source.sheets[0].columns[1].name = "Quantity".into();
    source.sheets[0].columns.push(SourceColumn {
        name: "Amount".into(),
        width: None,
        number_format: None,
    });
    source.sheets[0].columns.push(SourceColumn {
        name: "Date".into(),
        width: Some(14.0),
        number_format: None,
    });
    for (index, row) in source.sheets[0].rows.iter_mut().enumerate() {
        row[1].value = SourceValue::Number { value: 2.0 };
        row.push(SourceCell {
            value: SourceValue::Number { value: 2.0 },
            formula: Some(format!("B{}*C{}", index + 2, index + 2)),
            ..Default::default()
        });
        row.push(SourceCell {
            value: SourceValue::Date {
                value: "2024-02-29".into(),
            },
            style: CellStyle {
                number_format: Some("mmm d, yyyy".into()),
                ..Default::default()
            },
            ..Default::default()
        });
    }
    selection.column_types[0] = vec![
        ImportFieldType::Text,
        ImportFieldType::Number,
        ImportFieldType::Number,
        ImportFieldType::Number,
        ImportFieldType::Date,
    ];
    let xlsx = export_xlsx(&source).unwrap();
    (import_xlsx(&xlsx).unwrap(), selection)
}

#[test]
fn working_metadata_v2_wire_parity_and_reopen() {
    let (source, selection) = metadata_v2_fixture();
    let (mut ordinary, v1) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let (mut working, v2) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    assert_eq!(v1.metadata.version, 1);
    assert_eq!(v2.metadata.version, 2);
    let v1_json = serde_json::to_string(&v1.metadata).unwrap();
    assert!(v1_json.contains("\"rows\""));
    assert!(v1_json.contains("\"width\""));
    let json = serde_json::to_string(&v2.metadata).unwrap();
    assert!(!json.contains("\"rows\""));
    assert!(!json.contains("\"width\""));
    let decoded: crate::InteropMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, v2.metadata);
    let first = working
        .export_workbook(working.current_revision(), &decoded)
        .unwrap();
    let legacy = working
        .export_workbook(working.current_revision(), &v1.metadata)
        .unwrap();
    assert_eq!(legacy.sheets, first.sheets);
    assert_eq!(
        first.sheets[0].rows[0][3].formula.as_deref(),
        Some("(B2*C2)")
    );
    assert!(first.sheets[0].rows[0][0].style.bold);
    assert!(first.sheets[0].rows[0][0].style.fill.is_some());
    assert_eq!(first.sheets[0].columns[0].width, Some(12.0));
    for runtime in [&mut ordinary, &mut working] {
        let schema = v1.metadata.sheets[0].schema_id.clone();
        let deleted = v1.metadata.sheets[0].rows[2].entity_id.clone();
        runtime
            .handle(DesignerRequest::RemoveTableRows {
                expected_revision: runtime.current_revision().to_owned(),
                collection: schema.clone(),
                entities: vec![deleted],
            })
            .unwrap();
        runtime
            .handle(DesignerRequest::InsertRow {
                expected_revision: runtime.current_revision().to_owned(),
                collection: schema,
                initializers: vec![],
            })
            .unwrap();
    }
    let a = ordinary
        .export_workbook(ordinary.current_revision(), &v1.metadata)
        .unwrap();
    let b = working
        .export_workbook(working.current_revision(), &decoded)
        .unwrap();
    let a = import_xlsx(&export_xlsx(&a).unwrap()).unwrap();
    let b = import_xlsx(&export_xlsx(&b).unwrap()).unwrap();
    assert_eq!(a.sheets, b.sheets);
    let saved = working
        .export_project_v3(working.current_revision())
        .unwrap();
    let (reopened, _) =
        crate::admit_imported_project_working_size(&saved.bytes, OCCURRENCE, &decoded).unwrap();
    let reopened = reopened
        .export_workbook(reopened.current_revision(), &decoded)
        .unwrap();
    assert_eq!(
        b.sheets,
        import_xlsx(&export_xlsx(&reopened).unwrap())
            .unwrap()
            .sheets
    );
}

#[test]
fn working_metadata_v2_rejects_foreign_mappings_and_profiles() {
    let (mut source, mut selection) = stage5_fixture();
    let mut other = source.sheets[0].clone();
    other.name = "Other".into();
    source.sheets.push(other);
    selection
        .column_types
        .push(selection.column_types[0].clone());
    selection.extra_columns.push(vec![]);
    let (working, imported) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    let (ordinary, ordinary_imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let error = ordinary
        .export_workbook(ordinary.current_revision(), &imported.metadata)
        .unwrap_err();
    assert!(matches!(
        error,
        crate::DesignerError::ImportedSpreadsheetExport { .. }
    ));
    let snapshot = working.session.export_snapshot();
    let error = crate::interop_document::validate_import_metadata_for_profile(
        snapshot.document(),
        &imported.metadata,
        crate::capacity::SessionProfile::TextCapacity,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        crate::DesignerError::ImportedSpreadsheetExport { .. }
    ));
    let mut foreign = imported.metadata.clone();
    foreign.sheets[0].columns[0].field_id = foreign.sheets[1].columns[0].field_id.clone();
    let error = working
        .export_workbook(working.current_revision(), &foreign)
        .unwrap_err();
    assert!(matches!(
        error,
        crate::DesignerError::ImportedSpreadsheetExport { finding }
            if finding.code == "imported_spreadsheet_unrepresentable"
                && finding.location.contains(foreign.sheets[1].columns[0].field_id.as_str())
    ));
    for (runtime, metadata) in [
        (&working, &imported.metadata),
        (&ordinary, &ordinary_imported.metadata),
    ] {
        let mut foreign = metadata.clone();
        foreign.sheets[0].schema_id = "foreign".into();
        let error = runtime
            .export_workbook(runtime.current_revision(), &foreign)
            .unwrap_err();
        assert!(matches!(
            error,
            crate::DesignerError::ImportedSpreadsheetExport { .. }
        ));
    }
    let mut json = serde_json::to_value(&imported.metadata).unwrap();
    json["sheets"][0]["rows"] = serde_json::Value::Array(Vec::new());
    assert!(serde_json::from_value::<crate::InteropMetadata>(json).is_err());
    let mut json = serde_json::to_value(&imported.metadata).unwrap();
    json["sheets"][0]["columns"][0]["width"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<crate::InteropMetadata>(json).is_err());
}

#[test]
fn working_metadata_v2_omits_deleted_column_like_v1() {
    let (source, selection) = stage5_fixture();
    let (mut ordinary, v1) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let (mut working, v2) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    let collection = v1.metadata.sheets[0].schema_id.clone();
    let field = v1.metadata.sheets[0].columns[1].field_id.clone();
    for runtime in [&mut ordinary, &mut working] {
        runtime
            .handle(DesignerRequest::RemoveColumn {
                expected_revision: runtime.current_revision().to_owned(),
                collection: collection.clone(),
                field: field.clone(),
            })
            .unwrap();
    }
    let ordinary = ordinary
        .export_workbook(ordinary.current_revision(), &v1.metadata)
        .unwrap();
    let working = working
        .export_workbook(working.current_revision(), &v2.metadata)
        .unwrap();
    assert_eq!(working.sheets, ordinary.sheets);
    assert_eq!(
        working.sheets[0]
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Uniform", "Number"]
    );
}

#[test]
fn working_metadata_v2_reports_unmapped_presentation() {
    let (mut source, selection) = stage5_fixture();
    source.sheets[0].rows[0][2].style.number_format = Some("0.00E+00".into());
    let (runtime, imported) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    assert_eq!(imported.metadata.version, 2);
    let findings: Vec<_> = imported
        .ledger
        .iter()
        .filter(|item| item.code == "presentation_not_kept")
        .collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].category,
        crate::interop_adapter::FidelityCategory::LossyOnExport
    );
    assert_eq!(findings[0].location, "Styles:column 3");
    assert!(findings[0].message.contains("0.00E+00"));
    let exported = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][2].value,
        SourceValue::Number { value: 1.0 }
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn stage5_promotion_export_reopen_and_working_size() {
    use tachiko_workspace_engine::{BorderSide, FieldId};
    let (source, selection) = stage5_fixture();
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    assert!(
        !imported
            .ledger
            .iter()
            .any(|finding| finding.code == "presentation_bound_dropped")
    );
    let sheet = &imported.metadata.sheets[0];
    let schema = SchemaId::from(sheet.schema_id.clone());
    let fields: Vec<FieldId> = sheet
        .columns
        .iter()
        .map(|column| column.field_id.clone().into())
        .collect();
    let table = &runtime.presentation.tables[&schema];
    let column = &runtime.presentation.styles[&table.column_styles[&fields[0]]];
    assert_eq!(column.bold, Some(true));
    assert_eq!(column.fill.as_deref(), Some("#FFFF00"));
    assert_eq!(
        column.borders,
        Some(vec![
            BorderSide::Top,
            BorderSide::Right,
            BorderSide::Bottom,
            BorderSide::Left
        ])
    );
    assert!(!table.column_styles.contains_key(&fields[1]));
    for row in &sheet.rows {
        assert!(!row.styles[0].bold && row.styles[0].fill.is_none() && !row.styles[0].border);
    }
    let style_at = |row: usize, column: usize| {
        &runtime.presentation.styles[&runtime.presentation.cells[&(
            EntityId::from(sheet.rows[row].entity_id.clone()),
            fields[column].clone(),
        )]]
    };
    assert_eq!(style_at(0, 1).bold, Some(true));
    assert_eq!(style_at(1, 1).fill.as_deref(), Some("#00B050"));
    assert_eq!(
        style_at(1, 2).number_format,
        crate::interop_number_format::from_excel("0%")
    );
    assert_eq!(sheet.rows[1].styles[2].number_format, None);
    let snapshot = runtime.session.export_snapshot();
    assert_eq!(
        snapshot.document().schemas[&schema].fields[&fields[2]].number_format,
        crate::interop_number_format::from_excel("#,##0.00")
    );
    let workbook = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    let reread = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
    for (row, pattern) in reread.sheets[0]
        .rows
        .iter()
        .zip(["#,##0.00", "0%", "#,##0.00"])
    {
        assert!(row[0].style.bold && row[0].style.border);
        assert!(row[0].style.fill.as_deref().unwrap().ends_with("FFFF00"));
        assert_eq!(row[2].style.number_format.as_deref(), Some(pattern));
    }
    assert!(reread.sheets[0].rows[0][1].style.bold);
    assert!(
        reread.sheets[0].rows[1][1]
            .style
            .fill
            .as_deref()
            .unwrap()
            .to_ascii_uppercase()
            .ends_with("00B050")
    );
    let saved = runtime
        .export_project_v3(runtime.current_revision())
        .unwrap();
    let mut reopened = None;
    crate::open_project(&mut reopened, &saved.bytes, OCCURRENCE).unwrap();
    assert_eq!(reopened.unwrap().presentation, runtime.presentation);
    let (working, working_imported) =
        crate::import_workbook_working_size(&source, &selection, OCCURRENCE).unwrap();
    assert_eq!(working.presentation, runtime.presentation);
    assert_eq!(working_imported.metadata.version, 2);
}

#[test]
fn stage5_false_bold_overrides_column() {
    let field = field("text", FieldType::Text, None);
    let column = StoredStyle {
        bold: Some(true),
        ..Default::default()
    };
    let cell = StoredStyle {
        bold: Some(false),
        ..Default::default()
    };
    let column_id = tachiko_storage::style_id(&column).unwrap();
    let cell_id = tachiko_storage::style_id(&cell).unwrap();
    let mut store = PresentationStore::default();
    store
        .styles
        .extend([(column_id.clone(), column), (cell_id.clone(), cell)]);
    store.tables.insert(
        "table".into(),
        TablePresentation {
            column_styles: BTreeMap::from([(field.id.clone(), column_id)]),
            ..Default::default()
        },
    );
    store
        .cells
        .insert(("row".into(), field.id.clone()), cell_id);
    let style = crate::interop_date_format::cell_style(
        &store,
        &"table".into(),
        &field,
        &"row".into(),
        &SourceValue::Text { value: "x".into() },
        CellStyle::default(),
    );
    assert!(!style.bold);
    let (source, selection) = stage5_fixture();
    let (mut runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let sheet = &imported.metadata.sheets[0];
    let mut store = runtime.presentation.clone();
    let exception = StoredStyle {
        bold: Some(false),
        ..Default::default()
    };
    let id = tachiko_storage::style_id(&exception).unwrap();
    store.styles.insert(id.clone(), exception);
    store.cells.insert(
        (
            sheet.rows[0].entity_id.clone().into(),
            sheet.columns[0].field_id.clone().into(),
        ),
        id,
    );
    runtime.set_presentation(store).unwrap();
    let workbook = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    let reread = import_xlsx(&export_xlsx(&workbook).unwrap()).unwrap();
    assert!(!reread.sheets[0].rows[0][0].style.bold);
    assert!(reread.sheets[0].rows[1][0].style.bold);
}

#[test]
fn stage5_unmapped_presentation_stays_in_metadata_and_export() {
    // The XLSX reader only yields hex `rgb` fills, and invalid fills are
    // refused by the import's export proof, so only an unmapped number
    // pattern can stay behind in metadata.
    // Column 2 keeps its `#,##0.00` field format: the cell's own pattern must
    // survive export instead of being replaced by the field default.
    let (mut source, selection) = stage5_fixture();
    source.sheets[0].rows[0][2].style.number_format = Some("0.00E+00".into());
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    assert_eq!(
        imported.metadata.sheets[0].rows[0].styles[2]
            .number_format
            .as_deref(),
        Some("0.00E+00")
    );
    let workbook = runtime
        .export_workbook(runtime.current_revision(), &imported.metadata)
        .unwrap();
    assert_eq!(
        workbook.sheets[0].rows[0][2].style.number_format.as_deref(),
        Some("0.00E+00")
    );
}

#[test]
fn stage5_palette_bound_drops_cell_and_validates() {
    let mut document = Document::empty("doc", "Doc");
    let definition = field("text", FieldType::Text, None);
    document.schemas.insert(
        "table".into(),
        Schema {
            id: "table".into(),
            key: "table".into(),
            display_name: None,
            field_order: None,
            fields: BTreeMap::from([(definition.id.clone(), definition)]),
        },
    );
    let mut pending = BTreeMap::new();
    for index in 0..4098 {
        let id = format!("row{index:04}");
        document.entities.insert(
            id.clone().into(),
            Entity {
                id: id.clone().into(),
                key: id.clone().into(),
                schema: "table".into(),
                fields: BTreeMap::new(),
            },
        );
        pending.insert(
            (id.into(), "text".into()),
            StoredStyle {
                // The last record reuses the first record's admitted style.
                fill: Some(format!("#{:06X}", index % 4097)),
                ..Default::default()
            },
        );
    }
    let mut store = PresentationStore::default();
    assert!(crate::interop_document::bound_import_presentation(
        &mut store, pending, 4098
    ));
    assert_eq!(store.styles.len(), 4096);
    assert_eq!(store.cells.len(), 4097);
    assert!(store.cells.contains_key(&("row4097".into(), "text".into())));
    tachiko_semantic_core::validate_presentation(&store, &document).unwrap();
}
