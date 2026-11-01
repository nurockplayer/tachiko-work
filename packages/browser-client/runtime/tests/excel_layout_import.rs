//! Issue #617: header detection below a title row and summary-row recognition.
use std::fs;

use tachiko_designer_runtime::interop_adapter::{
    CellStyle, FidelityCategory, ImportOptions, SourceCell, SourceColumn, SourceLayout,
    SourceSheet, SourceValue, SourceWorkbook, SummaryAggregate, SummaryCell, SummaryRow,
    export_xlsx, import_csv, import_xlsx_with,
};
use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, ImportFieldType, ImportSelection,
    import_workbook,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000000";

fn number(value: f64) -> SourceCell {
    SourceCell {
        value: SourceValue::Number { value },
        formula: None,
        style: CellStyle::default(),
    }
}

fn formula(source: &str) -> SourceCell {
    SourceCell {
        value: SourceValue::Number { value: 0.0 },
        formula: Some(source.into()),
        style: CellStyle::default(),
    }
}

fn calculated(value: &CalculationProjection) -> f64 {
    match value {
        CalculationProjection::Value { value } => *value,
        other => panic!("expected a calculated value, got {other:?}"),
    }
}

fn invoice() -> Vec<u8> {
    fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/excel_samples/invoice.xlsx"
    ))
    .unwrap()
}

fn household() -> Vec<u8> {
    fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/excel_samples/household.xlsx"
    ))
    .unwrap()
}

fn contacts() -> Vec<u8> {
    fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/excel_samples/contacts.xlsx"
    ))
    .unwrap()
}

#[test]
fn invoice_header_below_title_and_total_row_are_recognised() {
    let workbook = import_xlsx_with(&invoice(), &ImportOptions::default()).unwrap();
    assert_eq!(workbook.sheets.len(), 1);
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.name, "Invoice");
    let columns: Vec<&str> = sheet
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert_eq!(columns, ["Item", "Qty", "Unit price", "Amount"]);
    assert_eq!(sheet.rows.len(), 5);
    assert_eq!(sheet.layout.rows_above_header, 2);
    assert_eq!(
        sheet.layout.summary_row,
        Some(SummaryRow {
            row: 9,
            cells: vec![SummaryCell {
                column: 3,
                aggregate: SummaryAggregate::Total,
                formula: "SUM(D4:D8)".into(),
            }],
        })
    );
    let blocking: Vec<_> = workbook
        .ledger
        .iter()
        .filter(|finding| finding.blocking)
        .collect();
    assert!(blocking.is_empty(), "{blocking:?}");
    assert!(workbook.ledger.iter().any(|finding| {
        finding.category == FidelityCategory::LossyOnExport
            && finding.code == "rows_above_header_skipped"
            && finding.location == "Invoice!1:2"
            && !finding.blocking
    }));
    assert!(workbook.ledger.iter().any(|finding| {
        finding.category == FidelityCategory::LossyOnExport
            && finding.code == "summary_row_excluded"
            && finding.location == "Invoice!9"
            && !finding.blocking
    }));
    assert!(
        !workbook
            .ledger
            .iter()
            .any(|finding| finding.location == "Invoice!D9")
    );
    assert_eq!(sheet.rows[0][3].formula.as_deref(), Some("B4*C4"));
}

#[test]
fn household_totals_are_summary_rows_not_blockers() {
    let workbook = import_xlsx_with(&household(), &ImportOptions::default()).unwrap();
    assert_eq!(workbook.sheets.len(), 3);
    for sheet in &workbook.sheets[..2] {
        assert_eq!(sheet.rows.len(), 8, "{}", sheet.name);
        assert_eq!(sheet.layout.rows_above_header, 0, "{}", sheet.name);
        assert_eq!(
            sheet.layout.summary_row,
            Some(SummaryRow {
                row: 10,
                cells: vec![SummaryCell {
                    column: 2,
                    aggregate: SummaryAggregate::Total,
                    formula: "SUM(C2:C9)".into(),
                }],
            }),
            "{}",
            sheet.name
        );
    }
    let summary = &workbook.sheets[2];
    assert_eq!(summary.name, "Summary");
    assert_eq!(summary.rows.len(), 2);
    assert_eq!(summary.layout.rows_above_header, 0);
    assert_eq!(
        summary.layout.summary_row,
        Some(SummaryRow {
            row: 4,
            cells: vec![SummaryCell {
                column: 1,
                aggregate: SummaryAggregate::Average,
                formula: "AVERAGE(B2:B3)".into(),
            }],
        })
    );
    assert!(
        !workbook
            .ledger
            .iter()
            .any(|finding| finding.code == "unsupported_formula_function")
    );
    assert!(!workbook.ledger.iter().any(|finding| finding.blocking));
}

#[test]
fn contacts_sheet_without_totals_is_unchanged() {
    let workbook = import_xlsx_with(&contacts(), &ImportOptions::default()).unwrap();
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.rows.len(), 5);
    assert_eq!(sheet.layout, SourceLayout::default());
    for location in ["Contacts!A6", "Contacts!D6"] {
        assert!(
            workbook.ledger.iter().any(|finding| {
                finding.code == "unsupported_formula_function"
                    && finding.location == location
                    && finding.blocking
            }),
            "{location}: {:?}",
            workbook.ledger
        );
    }
}

#[test]
fn explicit_header_rows_override_detection_and_reject_out_of_range() {
    let bytes = invoice();
    let explicit = ImportOptions {
        header_rows: vec![Some(1)],
        ..ImportOptions::default()
    };
    let workbook = import_xlsx_with(&bytes, &explicit).unwrap();
    assert_eq!(workbook.sheets[0].layout.rows_above_header, 0);
    assert!(workbook.ledger.iter().any(|finding| {
        finding.code == "invalid_header" && finding.location == "Invoice!B1" && finding.blocking
    }));

    let outside = ImportOptions {
        header_rows: vec![Some(99)],
        ..ImportOptions::default()
    };
    let error = import_xlsx_with(&bytes, &outside).unwrap_err();
    assert!(error.to_string().contains("outside worksheet"), "{error}");

    let extra = ImportOptions {
        header_rows: vec![None, None],
        ..ImportOptions::default()
    };
    let error = import_xlsx_with(&bytes, &extra).unwrap_err();
    assert!(error.to_string().contains("more entries"), "{error}");

    let keep = ImportOptions {
        keep_summary_rows: true,
        ..ImportOptions::default()
    };
    let kept = import_xlsx_with(&household(), &keep).unwrap();
    let jan = &kept.sheets[0];
    assert_eq!(jan.rows.len(), 9);
    assert_eq!(jan.layout.summary_row, None);
    assert!(kept.ledger.iter().any(|finding| {
        finding.code == "unsupported_formula_function"
            && finding.location == "Jan!C10"
            && finding.blocking
    }));

    let csv = ImportOptions {
        header_rows: vec![Some(1)],
        ..ImportOptions::default()
    };
    let error = import_csv(b"a,b\n1,2\n", &csv).unwrap_err();
    assert!(error.to_string().contains("XLSX only"), "{error}");
}

#[test]
fn exported_workbook_keeps_the_header_in_row_one() {
    let source = import_csv(b"Name,Amount\nAda,12\n", &ImportOptions::default()).unwrap();
    let bytes = export_xlsx(&source).unwrap();
    let reimported = import_xlsx_with(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(reimported.sheets[0].layout, SourceLayout::default());
    assert!(
        !reimported
            .ledger
            .iter()
            .any(|finding| finding.code == "rows_above_header_skipped")
    );
}

#[test]
fn invoice_formulas_bind_and_export_from_the_imported_columns() {
    let source = import_xlsx_with(&invoice(), &ImportOptions::default()).unwrap();
    let selection = ImportSelection {
        column_types: vec![vec![
            ImportFieldType::Text,
            ImportFieldType::Number,
            ImportFieldType::Number,
            ImportFieldType::Number,
        ]],
        extra_columns: vec![Vec::new()],
    };
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let amounts: Vec<f64> = imported
        .opened
        .table
        .rows
        .iter()
        .map(|row| calculated(row.fields[3].calculated.as_ref().unwrap()))
        .collect();
    assert_eq!(amounts, [35.0, 96.0, 24.0, 12.9, 30.0]);
    let exported = runtime
        .export_workbook(&imported.opened.bootstrap.revision, &imported.metadata)
        .unwrap();
    let formula = exported.sheets[0].rows[0][3]
        .formula
        .as_deref()
        .expect("the Amount formula is exported");
    for reference in ["$B$2", "$C$2"] {
        assert!(formula.contains(reference), "{reference} in {formula}");
    }
    export_xlsx(&exported).expect("the exported invoice is an XLSX workbook");
}

#[test]
fn household_summary_cells_bind_column_totals_and_export_as_sum_ranges() {
    let source = import_xlsx_with(&household(), &ImportOptions::default()).unwrap();
    let months = vec![
        ImportFieldType::Date,
        ImportFieldType::Text,
        ImportFieldType::Number,
        ImportFieldType::Boolean,
    ];
    let selection = ImportSelection {
        column_types: vec![
            months.clone(),
            months,
            vec![ImportFieldType::Text, ImportFieldType::Number],
        ],
        extra_columns: vec![Vec::new(), Vec::new(), Vec::new()],
    };
    let (mut runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let DesignerResponse::Table(summary) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "sheet_3".into(),
        })
        .unwrap()
    else {
        panic!("expected the Summary table");
    };
    assert_eq!(summary.rows.len(), 2);
    let spent: Vec<f64> = summary
        .rows
        .iter()
        .map(|row| calculated(row.fields[1].calculated.as_ref().unwrap()))
        .collect();
    assert_eq!(spent, [450.0, 450.0]);
    assert_eq!(
        summary.rows[0].fields[1].formula.as_ref().unwrap().source,
        "[sheet_1.column_3:total]"
    );
    assert_eq!(
        summary.rows[1].fields[1].formula.as_ref().unwrap().source,
        "[sheet_2.column_3:total]"
    );
    let exported = runtime
        .export_workbook(&imported.opened.bootstrap.revision, &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[2].rows[0][1].formula.as_deref(),
        Some("SUM('Jan'!$C$2:$C$9)")
    );
    assert_eq!(
        exported.sheets[2].rows[1][1].formula.as_deref(),
        Some("SUM('Feb'!$C$2:$C$9)")
    );
    export_xlsx(&exported).expect("the exported household workbook is XLSX");

    // A column minimum has no exact Excel spelling, so export refuses it.
    let summary = &imported.metadata.sheets[2];
    runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: imported.opened.bootstrap.revision.clone(),
            target: tachiko_designer_runtime::FieldTarget {
                entity: summary.rows[0].entity_id.clone(),
                field: summary.columns[1].field_id.clone(),
            },
            source: "[sheet_1.column_3:min]".into(),
        })
        .unwrap();
    assert!(
        runtime
            .export_workbook("resident/1", &imported.metadata)
            .is_err()
    );
}

#[test]
fn a_summary_row_reference_outside_its_imported_cells_fails() {
    let source = SourceWorkbook {
        sheets: vec![SourceSheet {
            name: "Totals".into(),
            has_header: true,
            columns: ["Amount", "Other"]
                .into_iter()
                .map(|name| SourceColumn {
                    name: name.into(),
                    width: None,
                    number_format: None,
                })
                .collect(),
            rows: vec![
                vec![formula("B4"), number(1.0)],
                vec![number(2.0), number(3.0)],
            ],
            layout: SourceLayout {
                rows_above_header: 0,
                summary_row: Some(SummaryRow {
                    row: 4,
                    cells: vec![SummaryCell {
                        column: 0,
                        aggregate: SummaryAggregate::Total,
                        formula: "SUM(A2:A3)".into(),
                    }],
                }),
            },
        }],
        ledger: Vec::new(),
    };
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Number, ImportFieldType::Number]],
        extra_columns: vec![Vec::new()],
    };
    let Err(error) = import_workbook(&source, &selection, OCCURRENCE) else {
        panic!("a summary cell outside the imported cells must fail");
    };
    assert!(
        error
            .to_string()
            .contains("summary row that is not imported"),
        "{error}"
    );
}

#[test]
fn import_options_wire_shape_stays_compact_for_existing_callers() {
    let serialized = serde_json::to_string(&ImportOptions::default()).unwrap();
    assert_eq!(serialized, r#"{"delimiter":",","header":true}"#);
    let parsed: ImportOptions =
        serde_json::from_str(r#"{"delimiter":",","header":true,"header_rows":[3]}"#).unwrap();
    assert_eq!(parsed.header_rows, vec![Some(3)]);
    assert!(!parsed.keep_summary_rows);
}

fn text(value: &str) -> SourceCell {
    SourceCell {
        value: SourceValue::Text {
            value: value.into(),
        },
        formula: None,
        style: CellStyle::default(),
    }
}

fn mutate(bytes: &[u8], name: &str, change: impl FnOnce(String) -> String) -> Vec<u8> {
    use std::io::{Cursor, Read, Write};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        entries.push((file.name().to_owned(), contents));
    }
    let target = entries.iter_mut().find(|(path, _)| path == name).unwrap();
    target.1 = change(target.1.clone());
    let mut result = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut result);
        for (path, contents) in entries {
            writer
                .start_file(
                    path,
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    result.into_inner()
}

#[test]
fn a_tachiko_export_of_the_household_workbook_imports_again() {
    let source = import_xlsx_with(&household(), &ImportOptions::default()).unwrap();
    let months = vec![
        ImportFieldType::Date,
        ImportFieldType::Text,
        ImportFieldType::Number,
        ImportFieldType::Boolean,
    ];
    let selection = ImportSelection {
        column_types: vec![
            months.clone(),
            months,
            vec![ImportFieldType::Text, ImportFieldType::Number],
        ],
        extra_columns: vec![Vec::new(), Vec::new(), Vec::new()],
    };
    let (runtime, imported) = import_workbook(&source, &selection, OCCURRENCE).unwrap();
    let exported = runtime
        .export_workbook(&imported.opened.bootstrap.revision, &imported.metadata)
        .unwrap();
    let bytes = export_xlsx(&exported).unwrap();
    let reimported = import_xlsx_with(&bytes, &ImportOptions::default()).unwrap();
    let blocking: Vec<_> = reimported
        .ledger
        .iter()
        .filter(|finding| finding.blocking)
        .collect();
    assert!(blocking.is_empty(), "{blocking:?}");
    let (mut runtime, _) = import_workbook(&reimported, &selection, OCCURRENCE).unwrap();
    let DesignerResponse::Table(summary) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "sheet_3".into(),
        })
        .unwrap()
    else {
        panic!("expected the Summary table");
    };
    assert_eq!(summary.rows.len(), 2);
    let spent: Vec<f64> = summary
        .rows
        .iter()
        .map(|row| calculated(row.fields[1].calculated.as_ref().unwrap()))
        .collect();
    assert_eq!(spent, [450.0, 450.0]);
    assert_eq!(
        summary.rows[0].fields[1].formula.as_ref().unwrap().source,
        "[sheet_1.column_3:total]"
    );
    assert_eq!(
        summary.rows[1].fields[1].formula.as_ref().unwrap().source,
        "[sheet_2.column_3:total]"
    );
}

#[test]
fn a_partial_aggregate_range_in_a_data_row_stays_refused() {
    // MIN/MAX over a whole column also stay refused: Excel gives 0 for an
    // all-blank range where a Tachiko column min/max fails (review of #621).
    for formula in ["SUM(Jan!C2:C5)", "MIN(Jan!C2:C9)", "MAX(Jan!C2:C9)"] {
        assert_refused_at_summary_b2(formula);
    }
}

fn assert_refused_at_summary_b2(formula: &str) {
    let bytes = mutate(&household(), "xl/worksheets/sheet3.xml", |xml| {
        xml.replace(">Jan!C10</f>", &format!(">{formula}</f>"))
    });
    let workbook = import_xlsx_with(&bytes, &ImportOptions::default()).unwrap();
    assert!(
        workbook.ledger.iter().any(|finding| {
            finding.code == "unsupported_formula_function"
                && finding.location == "Summary!B2"
                && finding.blocking
        }),
        "{:?}",
        workbook.ledger
    );
}

fn expense_book(last: &str) -> Vec<u8> {
    let sheet = SourceSheet {
        name: "Expenses".into(),
        has_header: true,
        columns: ["Description", "Amount"]
            .into_iter()
            .map(|name| SourceColumn {
                name: name.into(),
                width: None,
                number_format: None,
            })
            .collect(),
        rows: vec![
            vec![text("Paper"), number(10.0)],
            vec![text("Toner"), number(20.0)],
            vec![text(last), formula("SUM(B2:B3)")],
        ],
        layout: SourceLayout::default(),
    };
    export_xlsx(&SourceWorkbook {
        sheets: vec![sheet],
        ledger: Vec::new(),
    })
    .unwrap()
}

#[test]
fn a_final_record_is_not_taken_for_a_total_row() {
    let workbook =
        import_xlsx_with(&expense_book("Reimbursement"), &ImportOptions::default()).unwrap();
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.layout.summary_row, None);
    assert_eq!(sheet.rows.len(), 3);
    assert!(
        workbook.ledger.iter().any(|finding| {
            finding.code == "unsupported_formula_function"
                && finding.location == "Expenses!B4"
                && finding.blocking
        }),
        "{:?}",
        workbook.ledger
    );

    // Without any label the row stays data too (review of #621).
    let workbook = import_xlsx_with(&expense_book(""), &ImportOptions::default()).unwrap();
    assert_eq!(workbook.sheets[0].layout.summary_row, None);
    assert_eq!(workbook.sheets[0].rows.len(), 3);

    let workbook = import_xlsx_with(&expense_book("Total"), &ImportOptions::default()).unwrap();
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.rows.len(), 2);
    assert_eq!(
        sheet.layout.summary_row,
        Some(SummaryRow {
            row: 4,
            cells: vec![SummaryCell {
                column: 1,
                aggregate: SummaryAggregate::Total,
                formula: "SUM(B2:B3)".into(),
            }],
        })
    );
}

fn blank_row_before_total(total: &str) -> SourceWorkbook {
    let sheet = SourceSheet {
        name: "Expenses".into(),
        has_header: true,
        columns: ["Description", "Amount"]
            .into_iter()
            .map(|name| SourceColumn {
                name: name.into(),
                width: None,
                number_format: None,
            })
            .collect(),
        rows: vec![
            vec![text("Paper"), number(10.0)],
            vec![text("Toner"), number(20.0)],
            vec![SourceCell::default(), SourceCell::default()],
            vec![text("Total"), formula(total)],
        ],
        layout: SourceLayout::default(),
    };
    let bytes = export_xlsx(&SourceWorkbook {
        sheets: vec![sheet],
        ledger: Vec::new(),
    })
    .unwrap();
    import_xlsx_with(&bytes, &ImportOptions::default()).unwrap()
}

#[test]
fn blank_rows_before_a_total_row_stay_data_rows() {
    // Review of #621: a formula elsewhere may still read the blank row.
    let workbook = blank_row_before_total("SUM(B2:B4)");
    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.rows.len(), 3);
    assert_eq!(
        sheet.layout.summary_row.as_ref().map(|row| row.row),
        Some(5)
    );
    // A total that skips the blank row would mean other rows than the column
    // total it would become, so the row stays data (and the SUM is refused).
    let workbook = blank_row_before_total("SUM(B2:B3)");
    assert_eq!(workbook.sheets[0].layout.summary_row, None);
    assert_eq!(workbook.sheets[0].rows.len(), 4);
}

#[test]
fn a_range_nested_inside_min_is_checked_as_its_own_call() {
    let bytes = mutate(&household(), "xl/worksheets/sheet3.xml", |xml| {
        xml.replace(">Jan!C10</f>", ">MIN(SUM(Jan!C2:C9),1)</f>")
    });
    let workbook = import_xlsx_with(&bytes, &ImportOptions::default()).unwrap();
    let blocking: Vec<_> = workbook
        .ledger
        .iter()
        .filter(|finding| finding.blocking)
        .collect();
    assert!(blocking.is_empty(), "{blocking:?}");
}
