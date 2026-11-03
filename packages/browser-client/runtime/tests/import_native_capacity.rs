//! Shared import geometry, direct selection and metadata-backed save/reopen (#635).
use std::io::{Cursor, Read, Write};
use tachiko_designer_runtime::interop_adapter::{
    CellStyle, ImportOptions, SourceCell, SourceColumn, SourceLayout, SourceSheet, SourceValue,
    SourceWorkbook, export_csv, export_xlsx, import_csv, import_xlsx,
};
use tachiko_designer_runtime::{
    DesignerError, ImportColumnSpec, ImportFieldType, ImportSelection, import_workbook,
    inspect_imported_project, open_imported_project,
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
fn shared_xlsx_escape_refusal_preserves_the_old_row_threshold() {
    let pattern = "_x005F_x0041_";
    for rows in [64, 65, 100, 128] {
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
            let bytes = export_xlsx(&source).unwrap();
            if rows > 64 {
                let error = import_xlsx(&bytes).unwrap_err();
                assert!(
                    error.to_string().contains(
                        "Sheets above 64 data rows refuse SpreadsheetML _xHHHH_ escapes until decoded (#637)"
                    ),
                    "{rows} rows, {location}: {error}"
                );
            } else {
                let parsed = import_xlsx(&bytes).unwrap();
                assert_eq!(parsed.sheets, source.sheets);
            }
        }
    }
}

#[test]
fn shared_xlsx_escape_guard_is_local_to_each_large_sheet() {
    let mut source = workbook(2, 1, 100);
    source.sheets[0].rows.truncate(64);
    source.sheets[0].name = "_x005F_x0041_".into();
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

    let csv = format!("Name\n{}", "_x005F_x0041_\n".repeat(100));
    let parsed = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    assert!(!parsed.ledger.iter().any(|finding| finding.blocking));
    assert_eq!(parsed.sheets[0].rows.len(), 100);
    assert_eq!(
        export_csv(&parsed.sheets[0]).unwrap(),
        csv.replace('\n', "\r\n").as_bytes()
    );
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
            Err(DesignerError::ProjectionTooLarge { actual, maximum }) => {
                assert!(actual > maximum, "{dimension}");
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
        let book = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
        assert_eq!(book.sheets[0].rows.len(), rows);
        if rows == 128 {
            assert!(!book.ledger.iter().any(|finding| finding.blocking));
            export_csv(&book.sheets[0]).unwrap();
        } else {
            assert!(book.ledger.iter().any(|finding| finding.blocking));
            assert!(export_csv(&book.sheets[0]).is_err());
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
    let encoded = export_xlsx(&exported).unwrap();
    assert_eq!(
        import_xlsx(&encoded).unwrap().sheets[0].rows,
        source.sheets[0].rows
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
