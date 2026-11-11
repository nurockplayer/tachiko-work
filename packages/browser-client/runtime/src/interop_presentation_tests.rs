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
        Some("m/d/yyyy")
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
