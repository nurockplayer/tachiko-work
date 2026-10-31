//! Issue #617: header detection below a title row and summary-row recognition.
use std::fs;

use tachiko_designer_runtime::interop_adapter::{
    FidelityCategory, ImportOptions, SourceLayout, SummaryAggregate, SummaryCell, SummaryRow,
    export_xlsx, import_csv, import_xlsx_with,
};

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
fn import_options_wire_shape_stays_compact_for_existing_callers() {
    let serialized = serde_json::to_string(&ImportOptions::default()).unwrap();
    assert_eq!(serialized, r#"{"delimiter":",","header":true}"#);
    let parsed: ImportOptions =
        serde_json::from_str(r#"{"delimiter":",","header":true,"header_rows":[3]}"#).unwrap();
    assert_eq!(parsed.header_rows, vec![Some(3)]);
    assert!(!parsed.keep_summary_rows);
}
