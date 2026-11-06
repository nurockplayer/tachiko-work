//! Shared import geometry, direct selection and metadata-backed save/reopen (#635).
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use tachiko_designer_runtime::interop_adapter::{
    CellStyle, ImportOptions, InteropError, SourceCell, SourceColumn, SourceLayout, SourceSheet,
    SourceValue, SourceWorkbook, export_csv, export_xlsx, import_csv, import_xlsx,
};
use tachiko_designer_runtime::{
    CapacityLimit, DesignerError, DesignerRuntime, ImportColumnSpec, ImportFieldType,
    ImportSelection, import_workbook, inspect_imported_project, open_imported_project,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const ORIGINAL: &str = "00000000-0000-4000-8000-000000000635";
const FRESH: &str = "00000000-0000-4000-8000-000000000636";

fn workbook(sheets: usize, columns: usize, rows: usize) -> SourceWorkbook {
    SourceWorkbook {
        sheets: (0..sheets)
            .map(|index| SourceSheet {
                name: format!("Sheet{}", index + 1),
                has_header: true,
                columns: (0..columns)
                    .map(|column| SourceColumn {
                        name: format!("C{column}"),
                        width: None,
                        number_format: None,
                    })
                    .collect(),
                rows: vec![
                    vec![
                        SourceCell {
                            value: SourceValue::Number { value: 1.25 },
                            style: CellStyle {
                                number_format: Some("0.00".into()),
                                ..CellStyle::default()
                            },
                            formula: None,
                        };
                        columns
                    ];
                    rows
                ],
                layout: SourceLayout::default(),
            })
            .collect(),
        ledger: Vec::new(),
    }
}

fn selection(book: &SourceWorkbook) -> ImportSelection {
    ImportSelection {
        column_types: book
            .sheets
            .iter()
            .map(|sheet| vec![ImportFieldType::Number; sheet.columns.len()])
            .collect(),
        extra_columns: vec![Vec::new(); book.sheets.len()],
    }
}

/// Alter valid package geometry without asking the bounded writer to select
/// a different profile: extend a sheet, column or row, or remove the last row.
fn boundary_xlsx(book: &SourceWorkbook, dimension: &str) -> Vec<u8> {
    let bytes = export_xlsx(book).unwrap();
    let mut source = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let mut extra_sheet = None;
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut xml = String::new();
        entry.read_to_string(&mut xml).unwrap();
        match (dimension, name.as_str()) {
            ("sheets", "xl/workbook.xml") => {
                xml = xml.replace(
                    "</sheets>",
                    "<sheet name=\"Sheet33\" sheetId=\"33\" r:id=\"rId33\"/></sheets>",
                );
            }
            ("sheets", "xl/_rels/workbook.xml.rels") => {
                xml = xml.replace("</Relationships>", "<Relationship Id=\"rId33\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet33.xml\"/></Relationships>");
            }
            ("sheets", "[Content_Types].xml") => {
                xml = xml.replace("</Types>", "<Override PartName=\"/xl/worksheets/sheet33.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>");
            }
            ("sheets", "xl/worksheets/sheet1.xml") => extra_sheet = Some(xml.clone()),
            ("columns", "xl/worksheets/sheet1.xml") => {
                xml = xml.replacen(
                    "</row>",
                    "<c r=\"AG1\" t=\"inlineStr\"><is><t>C32</t></is></c></row>",
                    1,
                );
                let end = xml.rfind("</row>").unwrap();
                xml.insert_str(end, "<c r=\"AG2\" t=\"n\"><v>1</v></c>");
            }
            ("rows" | "total", "xl/worksheets/sheet1.xml") => {
                let row = book.sheets[0].rows.len() + 2;
                xml = xml.replace(
                    "</sheetData>",
                    &format!(
                        "<row r=\"{row}\"><c r=\"A{row}\" t=\"n\"><v>1</v></c></row></sheetData>"
                    ),
                );
            }
            ("remove_last_row", "xl/worksheets/sheet1.xml") => {
                let start = xml.rfind("<row ").unwrap();
                let end = start + xml[start..].find("</row>").unwrap() + "</row>".len();
                xml.replace_range(start..end, "");
            }
            _ => {}
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    if let Some(xml) = extra_sheet {
        output
            .start_file("xl/worksheets/sheet33.xml", SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn refused_source(bytes: &[u8]) {
    if let Ok(book) = import_xlsx(bytes) {
        assert!(book.ledger.iter().any(|finding| finding.blocking));
        assert!(import_workbook(&book, &selection(&book), ORIGINAL).is_err());
    }
}

#[test]
fn xlsx_escape_text_round_trips_across_shared_and_capacity_profiles() {
    let pattern = "_x005F_x0041_";
    for rows in [10, 64, 65, 100, 128, 200] {
        for location in ["sheet", "column", "text"] {
            let mut source = workbook(1, 1, rows);
            for cell in source.sheets[0].rows.iter_mut().flatten() {
                cell.value = SourceValue::Text {
                    value: "value".into(),
                };
                cell.style = CellStyle::default();
            }
            match location {
                "sheet" => source.sheets[0].name = pattern.into(),
                "column" => source.sheets[0].columns[0].name = pattern.into(),
                "text" => {
                    source.sheets[0].rows[rows - 1][0].value = SourceValue::Text {
                        value: pattern.into(),
                    };
                }
                _ => unreachable!(),
            }
            if location == "sheet" {
                let error = export_xlsx(&source).unwrap_err().to_string();
                assert!(error.contains("worksheet"), "{error}");
                assert!(error.contains("escape"), "{error}");
                continue;
            }
            let bytes = export_xlsx(&source).unwrap();
            let parsed = import_xlsx(&bytes).unwrap();
            assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
            assert_eq!(parsed.sheets, source.sheets, "{rows} rows, {location}");
            let reopened = import_xlsx(&export_xlsx(&parsed).unwrap()).unwrap();
            assert_eq!(reopened.sheets, parsed.sheets);
        }
    }
}

#[test]
fn shared_xlsx_escape_text_round_trips_in_mixed_sheet_sizes() {
    let mut source = workbook(2, 1, 100);
    source.sheets[0].rows.truncate(64);
    source.sheets[0].columns[0].name = "_x005F_x0041_".into();
    source.sheets[0].rows[0][0] = SourceCell {
        value: SourceValue::Text {
            value: "_x005F_x0041_".into(),
        },
        ..SourceCell::default()
    };
    let parsed = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(parsed.sheets[0].name, source.sheets[0].name);
    assert_eq!(parsed.sheets[0].columns, source.sheets[0].columns);
    assert_eq!(parsed.sheets[0].rows, source.sheets[0].rows);
    source.sheets[0].name = "_x005F_x0041_".into();
    let error = export_xlsx(&source).unwrap_err().to_string();
    assert!(
        error.contains("native_tables_xlsx_escape_sequence"),
        "{error}"
    );
}

#[test]
fn shared_hundred_row_xlsx_and_csv_keep_their_admission() {
    let source = workbook(1, 1, 100);
    let parsed = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    // Non-default styles require Shared, not TextCapacity.
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(parsed.sheets[0].rows.len(), 100);
    assert!(parsed.sheets[0].columns[0].number_format.is_some());
    import_workbook(&parsed, &selection(&parsed), ORIGINAL).unwrap();

    let csv = format!("Name\n{}", "value\n".repeat(100));
    let parsed = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(parsed.sheets[0].rows.len(), 100);
    assert_eq!(
        export_csv(&parsed.sheets[0]).unwrap(),
        csv.replace('\n', "\r\n").as_bytes()
    );
}

#[test]
fn csv_escape_text_stays_literal_through_xlsx_and_project_reopen() {
    for rows in [10, 64, 65, 100, 128, 200] {
        for location in ["column", "text"] {
            let options = ImportOptions::default();
            let header = if location == "column" {
                "_x0041_"
            } else {
                "Name"
            };
            let value = if location == "text" {
                "_x0041_"
            } else {
                "value"
            };
            let csv = format!("{header}\n{}", format!("{value}\n").repeat(rows));
            let source = import_csv(csv.as_bytes(), &options).unwrap();
            let selected = ImportSelection {
                column_types: vec![vec![ImportFieldType::Text]],
                extra_columns: vec![Vec::new()],
            };
            let parsed = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
            assert_eq!(parsed.sheets, source.sheets);

            let (runtime, imported) = import_workbook(&source, &selected, ORIGINAL).unwrap();
            let saved = runtime
                .export_project_v3(&runtime.observe_occurrence().revision)
                .unwrap()
                .bytes;
            let mut resident = None;
            open_imported_project(&mut resident, &saved, &imported.metadata, FRESH).unwrap();
            assert_eq!(
                export_csv(&source.sheets[0]).unwrap(),
                csv.replace('\n', "\r\n").as_bytes()
            );
        }
    }
}

#[test]
fn shared_xlsx_and_direct_selection_geometry_boundaries() {
    for (sheets, columns, rows, dimension) in [
        (32, 1, 1, "sheets"),
        (1, 32, 1, "columns"),
        (1, 1, 128, "rows"),
    ] {
        let book = workbook(sheets, columns, rows);
        let bytes = export_xlsx(&book).unwrap();
        let parsed = import_xlsx(&bytes).unwrap();
        assert!(
            !parsed.ledger.iter().any(|finding| finding.blocking),
            "{:?}",
            parsed.ledger
        );
        assert_eq!(parsed.sheets.len(), sheets);
        assert_eq!(parsed.sheets[0].columns.len(), columns);
        assert_eq!(parsed.sheets[0].rows.len(), rows);
        match import_workbook(&parsed, &selection(&parsed), ORIGINAL) {
            Ok(_) => {}
            // Geometry does not waive the existing complete-publication cap.
            Err(DesignerError::ImportCapacityExceeded { capacity, .. }) => {
                assert_eq!(
                    capacity.limit,
                    CapacityLimit::ProjectionBytes,
                    "{dimension}"
                );
                assert!(capacity.attempted > capacity.maximum, "{dimension}");
            }
            Err(error) => panic!("{dimension}: {error}"),
        }
        refused_source(&boundary_xlsx(&book, dimension));
        let oversized = match dimension {
            "sheets" => workbook(33, 1, 1),
            "columns" => workbook(1, 33, 1),
            "rows" => workbook(1, 1, 129),
            _ => unreachable!(),
        };
        assert!(export_xlsx(&oversized).is_err());
        assert!(import_workbook(&oversized, &selection(&oversized), ORIGINAL).is_err());
    }
}

#[test]
fn shared_total_rows_are_simultaneous_with_per_sheet_bounds() {
    // 32 rows on each of 32 sheets; +1 remains below the per-sheet ceiling.
    let mut book = workbook(32, 1, 32);
    let bytes = export_xlsx(&book).unwrap();
    let parsed = import_xlsx(&bytes).unwrap();
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(
        parsed
            .sheets
            .iter()
            .map(|sheet| sheet.rows.len())
            .sum::<usize>(),
        1024
    );
    refused_source(&boundary_xlsx(&book, "total"));
    let row = book.sheets[0].rows[0].clone();
    book.sheets[0].rows.push(row);
    assert!(export_xlsx(&book).is_err());
    let error = import_workbook(&book, &selection(&book), ORIGINAL)
        .err()
        .unwrap();
    assert!(error.to_string().contains("1025"), "{error}");
}

#[test]
fn wide_csv_import_export_and_direct_selection() {
    for columns in [20, 32, 33] {
        let csv = format!(
            "{}\n{}\n",
            (0..columns)
                .map(|n| format!("C{n}"))
                .collect::<Vec<_>>()
                .join(","),
            vec!["value"; columns].join(",")
        );
        if columns == 33 {
            assert!(import_csv(csv.as_bytes(), &ImportOptions::default()).is_err());
            continue;
        }
        let book = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
        assert_eq!(book.sheets[0].columns.len(), columns);
        assert!(!book.ledger.iter().any(|finding| finding.blocking));
        let selected = ImportSelection {
            column_types: vec![vec![ImportFieldType::Text; columns]],
            extra_columns: vec![Vec::new()],
        };
        let (runtime, imported) = import_workbook(&book, &selected, ORIGINAL).unwrap();
        let exported = runtime
            .export_workbook(&runtime.observe_occurrence().revision, &imported.metadata)
            .unwrap();
        assert_eq!(
            export_csv(&exported.sheets[0]).unwrap(),
            csv.replace('\n', "\r\n").as_bytes()
        );
    }
}

#[test]
fn styled_shared_rows_import_export_save_inspect_and_reopen() {
    let rows = 65;
    let source = workbook(1, 1, rows);
    let parsed = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    let (runtime, imported) = import_workbook(&parsed, &selection(&parsed), ORIGINAL).unwrap();
    let exported = runtime
        .export_workbook(&runtime.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    // Uniform formats become semantic column presentation on import.
    assert_eq!(exported.sheets[0].columns, parsed.sheets[0].columns);
    assert!(exported.sheets[0].columns[0].number_format.is_some());
    assert_eq!(exported.sheets[0].rows.len(), rows);
    assert!(
        exported.sheets[0]
            .rows
            .iter()
            .all(|row| row[0].value == SourceValue::Number { value: 1.25 })
    );
    let encoded = export_xlsx(&exported).unwrap();
    assert_eq!(
        import_xlsx(&encoded).unwrap().sheets[0].rows,
        parsed.sheets[0].rows
    );
    let saved = runtime
        .export_project_v3(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes;
    inspect_imported_project(&saved, &imported.metadata).unwrap();
    let mut resident = None;
    open_imported_project(&mut resident, &saved, &imported.metadata, FRESH).unwrap();
    let reopened = resident.unwrap();
    let after = reopened
        .export_workbook(&reopened.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    assert_eq!(after.sheets, exported.sheets);
    assert_eq!(export_xlsx(&after).unwrap(), encoded);
}

#[test]
fn wide_csv_shared_row_boundary_and_plus_one() {
    let header = (0..20)
        .map(|n| format!("C{n}"))
        .collect::<Vec<_>>()
        .join(",");
    let row = ["value"; 20].join(",") + "\n";
    for rows in [128, 129] {
        let csv = format!("{header}\n{}", row.repeat(rows));
        let parsed = import_csv(csv.as_bytes(), &ImportOptions::default());
        if rows == 128 {
            let book = parsed.unwrap();
            assert_eq!(book.sheets[0].rows.len(), rows);
            assert!(!book.ledger.iter().any(|finding| finding.blocking));
            export_csv(&book.sheets[0]).unwrap();
        } else {
            let InteropError::Capacity(capacity) = parsed.unwrap_err() else {
                panic!("expected a typed row-capacity refusal");
            };
            assert_eq!(capacity.limit, CapacityLimit::TableRows);
            assert_eq!(capacity.collection.as_deref(), Some("Imported table"));
            assert_eq!(capacity.attempted, 129);
            assert_eq!(capacity.maximum, 128);
        }
    }
}

#[test]
fn shared_csv_extra_column_import_export_save_and_reopen() {
    for rows in [65, 128] {
        let csv = format!("a,b\n{}", "one,two\n".repeat(rows));
        let source = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
        assert!(!source.ledger.iter().any(|finding| finding.blocking));
        let selected = ImportSelection {
            column_types: vec![vec![ImportFieldType::Text; 2]],
            extra_columns: vec![vec![ImportColumnSpec {
                name: "Optional".into(),
                field_type: ImportFieldType::Text,
            }]],
        };
        let (runtime, imported) = import_workbook(&source, &selected, ORIGINAL).unwrap();
        let exported = runtime
            .export_workbook(&runtime.observe_occurrence().revision, &imported.metadata)
            .unwrap();
        assert_eq!(exported.sheets[0].columns.len(), 3);
        assert_eq!(exported.sheets[0].rows.len(), rows);
        assert!(exported.sheets[0].rows.iter().all(|row| {
            row[0].value
                == SourceValue::Text {
                    value: "one".into(),
                }
                && row[1].value
                    == SourceValue::Text {
                        value: "two".into(),
                    }
                && row[2].value == SourceValue::Empty
        }));
        assert_eq!(
            export_csv(&exported.sheets[0]).unwrap(),
            format!("a,b,Optional\r\n{}", "one,two,\"\"\r\n".repeat(rows)).as_bytes()
        );
        let xlsx = export_xlsx(&exported).unwrap();
        let parsed = import_xlsx(&xlsx).unwrap();
        assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
        assert_eq!(parsed.sheets[0].rows, exported.sheets[0].rows);
        let saved = runtime
            .export_project_v3(&runtime.observe_occurrence().revision)
            .unwrap()
            .bytes;
        inspect_imported_project(&saved, &imported.metadata).unwrap();
        let mut resident = None;
        open_imported_project(&mut resident, &saved, &imported.metadata, FRESH).unwrap();
        let reopened = resident.unwrap();
        let after = reopened
            .export_workbook(&reopened.observe_occurrence().revision, &imported.metadata)
            .unwrap();
        assert_eq!(after.sheets, exported.sheets);
        assert_eq!(export_xlsx(&after).unwrap(), xlsx);
    }
}

#[test]
fn text_capacity_csv_still_refuses_extra_columns() {
    let csv = format!("a,b\n{}", "one,two\n".repeat(129));
    let source = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    let mut selected = ImportSelection {
        column_types: vec![vec![ImportFieldType::Text; 2]],
        extra_columns: vec![Vec::new()],
    };
    import_workbook(&source, &selected, ORIGINAL).unwrap();
    selected.extra_columns[0].push(ImportColumnSpec {
        name: "Optional".into(),
        field_type: ImportFieldType::Text,
    });
    let error = import_workbook(&source, &selected, ORIGINAL).err().unwrap();
    assert!(error.to_string().contains("no extra columns"), "{error}");
}

/// One Number column and one same-row formula column, so each data row adds
/// exactly one source formula.
fn formula_workbook(sheets: usize, rows: usize) -> SourceWorkbook {
    SourceWorkbook {
        sheets: (0..sheets)
            .map(|index| SourceSheet {
                name: format!("Sheet{}", index + 1),
                has_header: true,
                columns: vec![
                    SourceColumn {
                        name: "Value".into(),
                        width: None,
                        number_format: None,
                    },
                    SourceColumn {
                        name: "Double".into(),
                        width: None,
                        number_format: None,
                    },
                ],
                rows: (0..rows)
                    .map(|row| {
                        vec![
                            SourceCell {
                                value: SourceValue::Number { value: 1.25 },
                                ..SourceCell::default()
                            },
                            SourceCell {
                                value: SourceValue::Number { value: 2.5 },
                                formula: Some(format!("A{}*2", row + 2)),
                                ..SourceCell::default()
                            },
                        ]
                    })
                    .collect(),
                layout: SourceLayout::default(),
            })
            .collect(),
        ledger: Vec::new(),
    }
}

fn assert_formula_column(book: &SourceWorkbook, rows: usize) {
    for (sheet_index, sheet) in book.sheets.iter().enumerate() {
        assert_eq!(sheet.rows.len(), rows, "sheet {}", sheet_index + 1);
        for (row, cells) in sheet.rows.iter().enumerate() {
            assert_eq!(cells[0].value, SourceValue::Number { value: 1.25 });
            assert_eq!(cells[0].formula, None);
            assert_eq!(
                cells[1].formula.as_deref(),
                Some(format!("(A{}*2)", row + 2).as_str()),
                "sheet {} row {}",
                sheet_index + 1,
                row + 1
            );
            assert_eq!(cells[1].value, SourceValue::Number { value: 2.5 });
        }
    }
}

fn formula_round_trip(source: &SourceWorkbook) {
    let parsed = import_xlsx(&export_xlsx(source).unwrap()).unwrap();
    assert_eq!(parsed.sheets, source.sheets);
    let (runtime, imported) = import_workbook(&parsed, &selection(&parsed), ORIGINAL).unwrap();
    let exported = runtime
        .export_workbook(&runtime.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    assert_formula_column(&exported, source.sheets[0].rows.len());
    assert_eq!(exported.sheets.len(), source.sheets.len());
    let saved = runtime
        .export_project_v3(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes;
    inspect_imported_project(&saved, &imported.metadata).unwrap();
    let mut resident = None;
    open_imported_project(&mut resident, &saved, &imported.metadata, FRESH).unwrap();
    let reopened = resident.unwrap();
    let after = reopened
        .export_workbook(&reopened.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    assert_formula_column(&after, source.sheets[0].rows.len());
    assert_eq!(after.sheets.len(), source.sheets.len());
    assert_eq!(after.sheets, exported.sheets);
    assert_eq!(
        export_xlsx(&after).unwrap(),
        export_xlsx(&exported).unwrap()
    );
}

#[test]
fn xlsx_formulas_round_trip_above_the_old_bound() {
    // A single sheet admits at most 66 of these formulas; the ordinary
    // 64 KiB opened projection refuses the next row before the 256-formula
    // bound is reachable. Two sheets carry 100 formulas with 50 rows each.
    for (sheets, rows) in [(1, 33), (1, 66), (2, 50)] {
        formula_round_trip(&formula_workbook(sheets, rows));
    }
}

#[test]
fn xlsx_formula_growth_stops_at_the_ordinary_projection_gate() {
    for (sheets, rows) in [(1, 67), (2, 52)] {
        let source = formula_workbook(sheets, rows);
        let parsed = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
        assert_eq!(parsed.sheets, source.sheets);
        let Err(error) = import_workbook(&parsed, &selection(&parsed), ORIGINAL) else {
            panic!("{sheets}x{rows} formula workbook imported");
        };
        let DesignerError::ImportCapacityExceeded { capacity, at_least } = &error else {
            panic!("expected the import capacity refusal, got {error}");
        };
        assert!(!at_least);
        assert_eq!(capacity.limit, CapacityLimit::ProjectionBytes);
        // MAX_PROJECTION_BYTES, the ordinary-profile complete opened projection.
        assert_eq!(capacity.maximum, 65_536);
        assert!(capacity.attempted > capacity.maximum);
        assert!(capacity.collection.is_none());
        assert_eq!(
            error.to_string(),
            format!(
                "Workbook is too large to open: it needs {} bytes and the limit is 65536. The workbook was not imported.",
                capacity.attempted
            )
        );
        // The wire shape the kit's `isCapacityFailure` accepts.
        let failure = serde_json::to_value(error.failure_projection("unavailable")).unwrap();
        assert_eq!(failure["code"], "capacity_exceeded", "{failure}");
        assert_eq!(failure["limit"], "projection_bytes", "{failure}");
        assert_eq!(failure["maximum"], 65_536, "{failure}");
        assert_eq!(
            failure["attempted"],
            serde_json::json!(capacity.attempted),
            "{failure}"
        );
        assert!(failure.get("collection").is_none(), "{failure}");
    }
}

/// The canonical shape behind the 2×52 import: two tables whose Number and
/// formula columns overflow the complete post-edit refresh. Import cannot
/// expose its intermediate document, so this builds the same canonical
/// geometry directly.
fn oversized_post_edit_document() -> Document {
    let mut document = Document::empty("refresh_overflow", "Oversized post-edit refresh");
    for sheet in 0..2 {
        let schema_id = SchemaId::from(format!("sheet_{sheet}"));
        let value = FieldId::from(format!("value_column_{sheet:05}"));
        let formula = FieldId::from(format!("double_column_{sheet:04}"));
        document.schemas.insert(
            schema_id.clone(),
            Schema {
                id: schema_id.clone(),
                key: SchemaKey::from(format!("sheet_{sheet}")),
                display_name: None,
                fields: BTreeMap::from([
                    (
                        value.clone(),
                        FieldDefinition {
                            id: value.clone(),
                            key: FieldKey::from("value"),
                            display_name: None,
                            field_type: FieldType::Number,
                            required: false,
                            constraint: FieldConstraint::None,
                            number_format: None,
                        },
                    ),
                    (
                        formula.clone(),
                        FieldDefinition {
                            id: formula.clone(),
                            key: FieldKey::from("double"),
                            display_name: None,
                            field_type: FieldType::Number,
                            required: false,
                            constraint: FieldConstraint::None,
                            number_format: None,
                        },
                    ),
                ]),
                field_order: None,
            },
        );
        for row in 0..100 {
            let entity_id = EntityId::from(format!("row_entity_{sheet:05}_{row:05}"));
            document.entities.insert(
                entity_id.clone(),
                Entity {
                    id: entity_id.clone(),
                    key: EntityKey::from(format!("row_{sheet}_{row}")),
                    schema: schema_id.clone(),
                    fields: BTreeMap::from([
                        (
                            value.clone(),
                            Value::Number(Number::new(1.25).expect("finite fixture number")),
                        ),
                        (
                            formula.clone(),
                            Value::Formula(Expression::Reference(FieldRef::new(
                                entity_id,
                                value.clone(),
                            ))),
                        ),
                    ]),
                },
            );
        }
    }
    document
}

#[test]
fn non_import_open_keeps_todays_post_edit_refresh_refusal() {
    let error = DesignerRuntime::from_document(oversized_post_edit_document(), ORIGINAL)
        .err()
        .expect("the document must exceed the post-edit refresh bound");
    let DesignerError::PostEditRefreshTooLarge { actual, maximum } = &error else {
        panic!("expected the post-edit refresh refusal, got {error}");
    };
    assert_eq!(*maximum, 65_536);
    assert!(*actual > *maximum);
    // The Display text and wire code non-import callers see are unchanged.
    assert_eq!(
        error.to_string(),
        format!(
            "this canonical project is outside the bounded Designer profile: the worst-case post-edit refresh is not bounded: projection is {actual} bytes; the bounded maximum is 65536"
        )
    );
    let failure = serde_json::to_value(error.failure_projection("unavailable")).unwrap();
    assert_eq!(failure["code"], "unsupported_project", "{failure}");
    assert_eq!(failure["message"], error.to_string(), "{failure}");
    assert!(failure.get("limit").is_none(), "{failure}");
}

fn date_workbook(rows: usize, columns: usize) -> SourceWorkbook {
    let mut source = workbook(1, columns, rows);
    for cell in source.sheets[0].rows.iter_mut().flatten() {
        cell.value = SourceValue::Date {
            value: "2026-11-04".into(),
        };
        cell.style.number_format = Some("yyyy-mm-dd".into());
    }
    source
}

#[cfg(not(target_arch = "wasm32"))]
fn host_xlsx_export(source: &[u8], selected: &ImportSelection) -> Vec<u8> {
    let mut host = tachiko_designer_runtime::host_bridge::DesignerHost::new();
    let mut request: serde_json::Value = serde_json::from_str(
        r#"{"type":"import","format":"xlsx","csv_options":{"delimiter":",","header":true},"selection":null,"occurrence_id":"00000000-0000-4000-8000-000000000635","install":true}"#,
    ).unwrap();
    request["selection"] = serde_json::to_value(selected).unwrap();
    let reply = host.spreadsheet(&serde_json::to_vec(&request).unwrap(), source);
    let reply: serde_json::Value = serde_json::from_slice(&reply.reply).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    let payload = &reply["response"]["payload"];
    let mut request: serde_json::Value = serde_json::from_str(
        r#"{"type":"export","format":"xlsx","expected_revision":"resident/0","metadata":null,"collection":"sheet_1"}"#,
    ).unwrap();
    request["metadata"] = payload["metadata"].clone();
    let reply = host.spreadsheet(&serde_json::to_vec(&request).unwrap(), &[]);
    let response: serde_json::Value = serde_json::from_slice(&reply.reply).unwrap();
    assert_eq!(response["status"], "ok", "{response}");
    reply.bytes.unwrap()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn shared_shaped_dates_promoted_to_capacity_import_save_and_reopen() {
    // Retain the plain capacity package's presentation while bringing its
    // geometry down to Shared, as in an externally supplied plain workbook.
    let mut source = date_workbook(129, 16);
    let bytes = boundary_xlsx(&source, "remove_last_row");
    source.sheets[0].rows.pop();
    let parsed = import_xlsx(&bytes).unwrap();
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(parsed.sheets[0].rows, source.sheets[0].rows);
    let selected = ImportSelection {
        column_types: vec![vec![ImportFieldType::Date; 16]],
        extra_columns: vec![Vec::new()],
    };
    // Repeated imported field identities exceed the ordinary profile-string
    // budget even though the source fits Shared geometry and has no Text.
    let (runtime, imported) = import_workbook(&parsed, &selected, ORIGINAL).unwrap();
    assert!(
        imported.metadata.sheets[0]
            .rows
            .iter()
            .flat_map(|row| &row.styles)
            .all(|style| *style == CellStyle::default())
    );
    let exported = runtime
        .export_workbook(&runtime.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    assert_eq!(exported.sheets[0].rows.len(), 128);
    assert_eq!(exported.sheets[0].columns.len(), 16);
    let encoded = host_xlsx_export(&bytes, &selected);
    assert_eq!(
        import_xlsx(&encoded).unwrap().sheets[0].rows,
        source.sheets[0].rows
    );
    let reparsed = import_xlsx(&encoded).unwrap();
    assert!(!reparsed.ledger.iter().any(|finding| finding.blocking
        || matches!(finding.code.as_str(), "style_profile" | "bounded_workbook")));
    let (runtime, imported) = import_workbook(&reparsed, &selected, ORIGINAL).unwrap();
    let saved = runtime
        .export_project_v3(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes;
    inspect_imported_project(&saved, &imported.metadata).unwrap();
    let mut resident = None;
    open_imported_project(&mut resident, &saved, &imported.metadata, FRESH).unwrap();
    let reopened = resident.unwrap();
    let after = reopened
        .export_workbook(&reopened.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    assert_eq!(after.sheets, exported.sheets);
    assert_eq!(host_xlsx_export(&encoded, &selected), encoded);
}

#[test]
fn small_shared_dates_keep_their_nondefault_style() {
    let source = date_workbook(2, 2);
    let parsed = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    let selected = ImportSelection {
        column_types: vec![vec![ImportFieldType::Date; 2]],
        extra_columns: vec![Vec::new()],
    };
    let (runtime, imported) = import_workbook(&parsed, &selected, ORIGINAL).unwrap();
    assert!(
        imported.metadata.sheets[0]
            .rows
            .iter()
            .flat_map(|row| &row.styles)
            .all(|style| *style == source.sheets[0].rows[0][0].style)
    );
    let saved = runtime
        .export_project_v3(&runtime.observe_occurrence().revision)
        .unwrap()
        .bytes;
    let mut resident = None;
    open_imported_project(&mut resident, &saved, &imported.metadata, FRESH).unwrap();
    let reopened = resident.unwrap();
    let after = reopened
        .export_workbook(&reopened.observe_occurrence().revision, &imported.metadata)
        .unwrap();
    assert_eq!(after.sheets[0].rows, source.sheets[0].rows);
}
