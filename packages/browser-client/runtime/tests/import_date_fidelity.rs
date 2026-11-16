//! B2: selected types, strict optional Dates, locale ownership and width disclosure.
use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, FieldTarget, ImportFieldType,
    ImportSelection, ScalarEditInput, import_workbook,
    interop_adapter::{
        CellStyle, FidelityCategory, ImportOptions, SourceCell, SourceColumn, SourceLayout,
        SourceSheet, SourceValue, SourceWorkbook, export_xlsx, import_xlsx, import_xlsx_with,
    },
};
use tachiko_workspace_engine::patch_lifecycle::PatchLifecycleError;

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000802";

fn date_workbook(formula: Option<&str>) -> SourceWorkbook {
    SourceWorkbook {
        ledger: vec![],
        sheets: vec![SourceSheet {
            name: "Data".into(),
            has_header: true,
            columns: ["Date", "Result"]
                .into_iter()
                .map(|name| SourceColumn {
                    name: name.into(),
                    width: None,
                    number_format: None,
                })
                .collect(),
            rows: vec![vec![
                SourceCell {
                    value: SourceValue::Date {
                        value: "2026-09-05".into(),
                    },
                    ..SourceCell::default()
                },
                SourceCell {
                    value: SourceValue::Number { value: 2026.0 },
                    formula: formula.map(str::to_owned),
                    style: CellStyle::default(),
                },
            ]],
            layout: SourceLayout::default(),
        }],
    }
}

fn selection(date_type: ImportFieldType) -> ImportSelection {
    ImportSelection {
        column_types: vec![vec![date_type, ImportFieldType::Number]],
        extra_columns: vec![vec![]],
    }
}

#[test]
fn date_operand_selected_as_text_keeps_cache_with_a_finding() {
    let source = import_xlsx(&export_xlsx(&date_workbook(Some("=YEAR(A2)"))).unwrap()).unwrap();
    let (runtime, imported) =
        import_workbook(&source, &selection(ImportFieldType::Text), OCCURRENCE).unwrap();
    let kept: Vec<_> = imported
        .ledger
        .iter()
        .filter(|finding| finding.code == "unsupported_formula_kept_value")
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].location, "Data!B2");
    assert!(kept[0].message.contains("selected import field types"));
    assert!(!imported.ledger.iter().any(|finding| finding.blocking));
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][0].value,
        SourceValue::Text {
            value: "2026-09-05".into()
        }
    );
    assert_eq!(
        exported.sheets[0].rows[0][1].value,
        SourceValue::Number { value: 2026.0 }
    );
    assert_eq!(exported.sheets[0].rows[0][1].formula, None);
}

#[test]
fn blank_date_criterion_range_binds_only_with_selected_date_type() {
    let mut source = date_workbook(Some("=COUNTIF($A$2:$A$2,C2)"));
    let criterion = source.sheets[0].rows[0][0].clone();
    source.sheets[0].columns.push(SourceColumn {
        name: "Criterion".into(),
        width: None,
        number_format: None,
    });
    source.sheets[0].rows[0][0].value = SourceValue::Empty;
    source.sheets[0].rows[0][1].value = SourceValue::Number { value: 0.0 };
    source.sheets[0].rows[0].push(criterion);
    for kind in [ImportFieldType::Date, ImportFieldType::Text] {
        let mut selected = selection(kind);
        selected.column_types[0].push(ImportFieldType::Date);
        let (runtime, imported) = import_workbook(&source, &selected, OCCURRENCE).unwrap();
        let exported = runtime
            .export_workbook("resident/0", &imported.metadata)
            .unwrap();
        assert_eq!(
            exported.sheets[0].rows[0][1].value,
            SourceValue::Number { value: 0.0 }
        );
        let date_selected = matches!(kind, ImportFieldType::Date);
        assert_eq!(
            exported.sheets[0].rows[0][1].formula.is_some(),
            date_selected
        );
        assert_eq!(
            imported.ledger.iter().any(
                |finding| finding.code == "unsupported_formula_kept_value" && !finding.blocking
            ),
            !date_selected
        );
    }
}

/// Re-inspects exported XLSX bytes with one header row.
fn reimport(bytes: &[u8]) -> SourceWorkbook {
    import_xlsx_with(
        bytes,
        &ImportOptions {
            header_rows: vec![Some(1)],
            ..ImportOptions::default()
        },
    )
    .unwrap()
}

/// Imports a blank Date criterion column with `formula`, exports it and re-inspects the XLSX.
fn blank_date_round_trip(formula: &str) -> (SourceWorkbook, ImportSelection) {
    let mut source = date_workbook(Some(formula));
    let criterion = source.sheets[0].rows[0][0].clone();
    source.sheets[0]
        .columns
        .extend(["Criterion", "Amount"].map(|name| SourceColumn {
            name: name.into(),
            width: None,
            number_format: None,
        }));
    source.sheets[0].rows[0][0].value = SourceValue::Empty;
    source.sheets[0].rows[0][1].value = SourceValue::Number { value: 0.0 };
    source.sheets[0].rows[0].extend([
        criterion,
        SourceCell {
            value: SourceValue::Number { value: 1.0 },
            ..SourceCell::default()
        },
    ]);
    let mut selected = selection(ImportFieldType::Date);
    selected.column_types[0].extend([ImportFieldType::Date, ImportFieldType::Number]);
    let (runtime, imported) = import_workbook(&source, &selected, OCCURRENCE).unwrap();
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    let inspected = reimport(&export_xlsx(&exported).unwrap());
    (inspected, selected)
}

#[test]
fn blank_date_criterion_round_trips_and_respects_selected_type() {
    for formula in [
        "=COUNTIFS($A$2:$A$2,C2)",
        "=SUMIFS($D$2:$D$2,$A$2:$A$2,C2)",
        "=IF(COUNTIFS($A$2:$A$2,C2)>0,1,0)",
    ] {
        let (inspected, mut selected) = blank_date_round_trip(formula);
        assert!(
            inspected.sheets[0].rows[0][1].formula.is_some(),
            "{formula}"
        );
        assert!(
            !inspected.ledger.iter().any(|finding| finding.blocking
                || finding.code == "unsupported_formula_kept_value"),
            "{formula}: {:?}",
            inspected.ledger
        );
        let (mut reopened, imported) = import_workbook(&inspected, &selected, OCCURRENCE).unwrap();
        assert!(
            !imported.ledger.iter().any(|finding| finding.blocking
                || finding.code == "unsupported_formula_kept_value"),
            "{formula}: {:?}",
            imported.ledger
        );
        let before = reopened
            .export_workbook("resident/0", &imported.metadata)
            .unwrap();
        assert!(before.sheets[0].rows[0][1].formula.is_some(), "{formula}");
        assert_eq!(
            before.sheets[0].rows[0][1].value,
            SourceValue::Number { value: 0.0 }
        );
        let DesignerResponse::Published(published) = reopened
            .handle(DesignerRequest::EditScalar {
                expected_revision: "resident/0".into(),
                target: FieldTarget {
                    entity: imported.metadata.sheets[0].rows[0].entity_id.clone(),
                    field: imported.metadata.sheets[0].columns[0].field_id.clone(),
                },
                input: ScalarEditInput::Date {
                    value: "2026-09-05".into(),
                },
            })
            .unwrap()
        else {
            panic!("date edit publishes");
        };
        let after = reopened
            .export_workbook(&published.resulting_revision, &imported.metadata)
            .unwrap();
        assert_eq!(
            after.sheets[0].rows[0][1].value,
            SourceValue::Number { value: 1.0 }
        );
        assert!(after.sheets[0].rows[0][1].formula.is_some(), "{formula}");

        selected.column_types[0][0] = ImportFieldType::Text;
        let (text_runtime, text_imported) =
            import_workbook(&inspected, &selected, OCCURRENCE).unwrap();
        assert!(
            text_imported.ledger.iter().any(|finding| {
                finding.code == "unsupported_formula_kept_value" && !finding.blocking
            }),
            "{formula}: {:?}",
            text_imported.ledger
        );
        let text_exported = text_runtime
            .export_workbook("resident/0", &text_imported.metadata)
            .unwrap();
        assert!(
            text_exported.sheets[0].rows[0][1].formula.is_none(),
            "{formula}"
        );
        assert_eq!(
            text_exported.sheets[0].rows[0][1].value,
            SourceValue::Number { value: 0.0 }
        );
    }
}

#[test]
fn blank_date_criterion_exception_skips_nonblank_columns() {
    // These controls must not acquire the blank-column exception.
    for cell in [
        SourceCell {
            value: SourceValue::Number { value: 7.0 },
            ..SourceCell::default()
        },
        SourceCell {
            value: SourceValue::Text {
                value: "text".into(),
            },
            ..SourceCell::default()
        },
        SourceCell {
            value: SourceValue::Text {
                value: String::new(),
            },
            ..SourceCell::default()
        },
        SourceCell {
            value: SourceValue::Empty,
            formula: Some("=1".into()),
            ..SourceCell::default()
        },
    ] {
        let mut source = date_workbook(Some("=COUNTIFS($A$2:$A$2,C2)"));
        let criterion = source.sheets[0].rows[0][0].clone();
        source.sheets[0].columns.push(SourceColumn {
            name: "Criterion".into(),
            width: None,
            number_format: None,
        });
        source.sheets[0].rows[0][0] = cell;
        source.sheets[0].rows[0][1].value = SourceValue::Number { value: 0.0 };
        source.sheets[0].rows[0].push(criterion);
        let inspected = reimport(&export_xlsx(&source).unwrap());
        assert!(
            inspected.ledger.iter().any(|finding| {
                finding.location == "Data!B2"
                    && finding.code == "unsupported_formula_kept_value"
                    && finding
                        .message
                        .contains("a date criterion only tests a date column")
            }),
            "{:?}",
            inspected.ledger
        );
        assert!(inspected.sheets[0].rows[0][1].formula.is_none());
    }
}

#[test]
fn clearing_imported_year_operand_refuses_atomically() {
    let source = import_xlsx(&export_xlsx(&date_workbook(Some("=YEAR(A2)"))).unwrap()).unwrap();
    let (mut runtime, imported) =
        import_workbook(&source, &selection(ImportFieldType::Date), OCCURRENCE).unwrap();
    let before = runtime.export_project_v3("resident/0").unwrap();
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    assert_eq!(
        exported.sheets[0].rows[0][1].formula.as_deref(),
        Some("YEAR(IF(ISBLANK(A2),NA(),A2))")
    );
    let error = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".into(),
            target: FieldTarget {
                entity: imported.metadata.sheets[0].rows[0].entity_id.clone(),
                field: imported.metadata.sheets[0].columns[0].field_id.clone(),
            },
            input: ScalarEditInput::Clear,
        })
        .unwrap_err();
    assert!(
        matches!(
            error,
            DesignerError::Lifecycle(PatchLifecycleError::ValidationFailed { .. })
        ),
        "clearing the operand must fail semantic calculation: {error}"
    );
    assert_eq!(runtime.observe_occurrence().revision, "resident/0");
    assert_eq!(runtime.export_project_v3("resident/0").unwrap(), before);
    assert_eq!(
        runtime
            .export_workbook("resident/0", &imported.metadata)
            .unwrap(),
        exported
    );
}

#[test]
fn english_month_formats_export_with_an_explicit_locale() {
    for pattern in [
        "mmm d, yyyy",
        "d mmm yyyy",
        "[$-409]mmm d, yyyy",
        "[$-409]d mmm yyyy",
    ] {
        let mut source = date_workbook(None);
        source.sheets[0].rows[0][0].style.number_format = Some(pattern.into());
        let source = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
        let (runtime, imported) =
            import_workbook(&source, &selection(ImportFieldType::Date), OCCURRENCE).unwrap();
        assert_eq!(
            imported.metadata.sheets[0].rows[0].styles[0].number_format,
            None
        );
        let exported = runtime
            .export_workbook("resident/0", &imported.metadata)
            .unwrap();
        let reopened = import_xlsx(&export_xlsx(&exported).unwrap()).unwrap();
        let expected = if pattern.contains(',') {
            "[$-409]mmm d, yyyy"
        } else {
            "[$-409]d mmm yyyy"
        };
        assert_eq!(
            reopened.sheets[0].rows[0][0].style.number_format.as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn non_english_locale_stays_cell_owned_in_uniform_and_mixed_columns() {
    for english_neighbor in [false, true] {
        let mut source = date_workbook(None);
        source.sheets[0].rows[0][0].style.number_format = Some("[$-407]mmm d, yyyy".into());
        if english_neighbor {
            let mut row = source.sheets[0].rows[0].clone();
            row[0].style.number_format = Some("[$-409]d mmm yyyy".into());
            source.sheets[0].rows.push(row);
        }
        let source = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
        let (runtime, imported) =
            import_workbook(&source, &selection(ImportFieldType::Date), OCCURRENCE).unwrap();
        assert_eq!(
            imported.metadata.sheets[0].rows[0].styles[0]
                .number_format
                .as_deref(),
            Some("[$-407]mmm d, yyyy")
        );
        assert!(
            !imported
                .ledger
                .iter()
                .any(|finding| finding.code == "date_format_not_supported")
        );
        let exported = runtime
            .export_workbook("resident/0", &imported.metadata)
            .unwrap();
        let reopened = import_xlsx(&export_xlsx(&exported).unwrap()).unwrap();
        assert_eq!(
            reopened.sheets[0].rows[0][0].style.number_format.as_deref(),
            Some("[$-407]mmm d, yyyy")
        );
        if english_neighbor {
            assert_eq!(
                reopened.sheets[0].rows[1][0].style.number_format.as_deref(),
                Some("[$-409]d mmm yyyy")
            );
        }
    }
}

#[test]
fn rounded_and_clamped_widths_have_one_workbook_finding() {
    let mut source = date_workbook(None);
    source.sheets[0].columns[0].width = Some(8.43);
    source.sheets[0].columns[1].width = Some(300.0);
    let mut second = source.sheets[0].clone();
    second.name = "Second".into();
    second.columns[0].width = Some(0.001);
    second.columns[1].width = Some(8.0 + 1.0 / 256.0);
    source.sheets.push(second);
    let mut selected = selection(ImportFieldType::Date);
    selected.column_types.push(selected.column_types[0].clone());
    selected.extra_columns.push(vec![]);
    let (runtime, imported) = import_workbook(&source, &selected, OCCURRENCE).unwrap();
    let findings: Vec<_> = imported
        .ledger
        .iter()
        .filter(|finding| finding.code == "column_width_converted")
        .collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].category, FidelityCategory::Converted);
    assert_eq!(findings[0].location, "workbook");
    assert!(findings[0].message.starts_with("3 column widths"));
    assert!(!findings[0].blocking);
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    let reopened = import_xlsx(&export_xlsx(&exported).unwrap()).unwrap();
    assert_eq!(reopened.sheets[0].columns[0].width, Some(2158.0 / 256.0));
    assert_eq!(reopened.sheets[0].columns[1].width, Some(255.0));
    assert_eq!(reopened.sheets[1].columns[0].width, Some(1.0 / 256.0));
    assert_eq!(reopened.sheets[1].columns[1].width, Some(8.0 + 1.0 / 256.0));
}

#[test]
fn exact_widths_do_not_report_a_conversion() {
    let mut source = date_workbook(None);
    source.sheets[0].columns[0].width = Some(8.0 + 1.0 / 256.0);
    source.sheets[0].columns[1].width = Some(255.0);
    let source = import_xlsx(&export_xlsx(&source).unwrap()).unwrap();
    let (runtime, imported) =
        import_workbook(&source, &selection(ImportFieldType::Date), OCCURRENCE).unwrap();
    assert!(
        !imported
            .ledger
            .iter()
            .any(|finding| finding.code == "column_width_converted")
    );
    let exported = runtime
        .export_workbook("resident/0", &imported.metadata)
        .unwrap();
    assert_eq!(exported.sheets[0].columns, source.sheets[0].columns);
}
