use std::io::{Cursor, Read};
use tachiko_designer_runtime::interop_adapter::{
    CellStyle, FidelityCategory, FidelityFinding, SourceValue, export_csv, export_xlsx,
};
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, KeyedGroupedSumDefinitionInput,
    NativeBudgetExportCollection, NativeBudgetExportPresentation, NativeBudgetExportRow,
    NativeBudgetExportView,
};
use zip::ZipArchive;

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000297";

fn collection_mapping(
    runtime: &mut DesignerRuntime,
    collection: &str,
) -> NativeBudgetExportCollection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .expect("Budget collection should be queryable")
    else {
        panic!("expected Budget table");
    };
    NativeBudgetExportCollection {
        collection_id: table.collection.id,
        rows: table
            .rows
            .into_iter()
            .map(|row| NativeBudgetExportRow {
                entity_id: row.id,
                styles: vec![CellStyle::default(); table.columns.len()],
            })
            .collect(),
    }
}

fn presentation(
    runtime: &mut DesignerRuntime,
    views: Vec<NativeBudgetExportView>,
) -> NativeBudgetExportPresentation {
    NativeBudgetExportPresentation {
        active_view: views
            .first()
            .map(|view| view.id.clone())
            .expect("fixture has an active view"),
        version: 1,
        views,
        collections: vec![
            collection_mapping(runtime, "budget_items"),
            collection_mapping(runtime, "budget_summary"),
        ],
    }
}

fn worksheet_xml(bytes: &[u8], path: &str) -> String {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("XLSX should be a ZIP archive");
    let mut xml = String::new();
    archive
        .by_name(path)
        .expect("export should include the referenced worksheet")
        .read_to_string(&mut xml)
        .expect("worksheet XML should be UTF-8");
    xml
}

#[test]
fn native_budget_exports_disclose_saved_summary_once_without_changing_bytes_or_old_ledgers() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mapped = presentation(
        &mut runtime,
        vec![NativeBudgetExportView {
            id: "items".into(),
            name: "Budget Items".into(),
            collection_id: "budget_items".into(),
        }],
    );
    let csv_before = runtime
        .export_native_budget_csv_workbook("resident/0", &mapped)
        .unwrap();
    let xlsx_before = runtime
        .export_native_budget_workbook("resident/0", &mapped)
        .unwrap();
    assert_eq!(csv_before.ledger, vec![FidelityFinding {
        category: FidelityCategory::Converted,
        code: "native_budget_active_csv_values".into(),
        location: "Budget Items".into(),
        message: "CSV is rebuilt from the active Budget collection, entity, and field identities with authoritative calculated scalar values; inactive collections and worksheet names are not admitted.".into(),
        blocking: false,
    }]);
    let warning = FidelityFinding {
        category: FidelityCategory::LossyOnExport,
        code: "saved_keyed_grouped_sum_definitions_not_exported".into(),
        location: "workbook".into(),
        message: "Saved grouped-summary definitions are not included in this spreadsheet export. Keep a Tachiko saved copy to preserve their bindings and recomputation.".into(),
        blocking: false,
    };
    assert!(
        !xlsx_before
            .ledger
            .iter()
            .any(|finding| finding.code == warning.code)
    );
    let response = runtime
        .handle(DesignerRequest::CreateKeyedGroupedSum {
            expected_revision: "resident/0".into(),
            definition: KeyedGroupedSumDefinitionInput {
                id: "budget-summary".into(),
                orders_schema: "budget_items".into(),
                order_lookup_key_field: "name".into(),
                order_quantity_field: "planned".into(),
                products_schema: "budget_items".into(),
                product_key_field: "name".into(),
                product_category_field: "name".into(),
                product_price_field: "actual".into(),
            },
        })
        .unwrap();
    let DesignerResponse::KeyedGroupedSumPublished(published) = response else {
        panic!("expected a saved keyed grouped-sum definition");
    };
    assert!(published.result.diagnostics.is_empty());
    let revision = runtime.observe_occurrence().revision;
    for _ in 0..2 {
        let mut csv = runtime
            .export_native_budget_csv_workbook(&revision, &mapped)
            .unwrap();
        let mut xlsx = runtime
            .export_native_budget_workbook(&revision, &mapped)
            .unwrap();
        for workbook in [&mut csv, &mut xlsx] {
            let warnings = workbook
                .ledger
                .iter()
                .filter(|finding| finding.code == warning.code)
                .collect::<Vec<_>>();
            assert_eq!(
                warnings,
                vec![&warning],
                "each format must disclose exactly once"
            );
            assert_eq!(workbook.ledger.last(), Some(&warning));
            workbook.ledger.pop();
        }
        assert_eq!(
            export_csv(&csv.sheets[0]).unwrap(),
            export_csv(&csv_before.sheets[0]).unwrap()
        );
        assert_eq!(
            export_xlsx(&xlsx).unwrap(),
            export_xlsx(&xlsx_before).unwrap()
        );
        assert_eq!(
            csv, csv_before,
            "all prior CSV values/styles/findings stay unchanged"
        );
        assert_eq!(
            xlsx, xlsx_before,
            "all prior XLSX values/styles/findings stay unchanged"
        );
    }
    assert_eq!(runtime.observe_occurrence().revision, revision);
}

#[test]
fn native_budget_xlsx_omits_empty_cols_and_preserves_formulas_and_typed_dates() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mapped = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "items".into(),
                name: "Budget Items".into(),
                collection_id: "budget_items".into(),
            },
            NativeBudgetExportView {
                id: "summary".into(),
                name: "Budget Summary".into(),
                collection_id: "budget_summary".into(),
            },
        ],
    );
    let workbook = runtime
        .export_native_budget_workbook("resident/0", &mapped)
        .expect("native Budget workbook should be exportable");
    let bytes = export_xlsx(&workbook).expect("native Budget workbook should be valid XLSX");
    let items = worksheet_xml(&bytes, "xl/worksheets/sheet1.xml");
    let summary = worksheet_xml(&bytes, "xl/worksheets/sheet2.xml");

    assert!(!items.contains("<cols"));
    assert!(!summary.contains("<cols"));
    assert!(items.contains(
        "<c r=\"E2\" s=\"0\" t=\"n\"><f>(&apos;Budget Items&apos;!$A$2-&apos;Budget Items&apos;!$D$2)</f><v>0</v></c>"
    ));
    assert!(items.contains("<c r=\"B2\" s=\"1\" t=\"d\"><v>2026-09-01</v></c>"));
    assert!(summary.contains("<c r=\"C2\" s=\"1\" t=\"d\"><v>2026-09-01</v></c>"));
    assert!(summary.contains(
        "<c r=\"D2\" s=\"0\" t=\"n\"><f>(&apos;Budget Items&apos;!$D$2+&apos;Budget Items&apos;!$D$3)</f><v>1380</v></c>"
    ));
}

#[test]
fn native_budget_csv_ignores_inactive_excel_invalid_view_name_but_xlsx_refuses() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mapped = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "summary".into(),
                name: "Budget Summary".into(),
                collection_id: "budget_summary".into(),
            },
            NativeBudgetExportView {
                id: "items".into(),
                name: "x".repeat(32),
                collection_id: "budget_items".into(),
            },
        ],
    );

    let workbook = runtime
        .export_native_budget_csv_workbook("resident/0", &mapped)
        .expect("active Budget CSV must not admit inactive worksheet names");
    let csv = export_csv(&workbook.sheets[0]).expect("active Budget CSV should be writable");
    let csv = String::from_utf8(csv).unwrap();
    assert!(csv.contains("1360"), "{csv}");
    assert!(
        runtime
            .export_native_budget_workbook("resident/0", &mapped)
            .is_err()
    );
}

#[test]
fn native_budget_csv_ignores_inactive_mapping_but_xlsx_refuses() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mut mapped = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "summary".into(),
                name: "Budget Summary".into(),
                collection_id: "budget_summary".into(),
            },
            NativeBudgetExportView {
                id: "items".into(),
                name: "Budget Items".into(),
                collection_id: "budget_items".into(),
            },
        ],
    );
    mapped.collections[0].rows[0].entity_id = "foreign".into();

    let workbook = runtime
        .export_native_budget_csv_workbook("resident/0", &mapped)
        .expect("active Budget CSV must not validate inactive mappings");
    let csv = export_csv(&workbook.sheets[0]).expect("active Budget CSV should be writable");
    let csv = String::from_utf8(csv).unwrap();
    assert!(csv.contains("1360"), "{csv}");
    assert!(
        runtime
            .export_native_budget_workbook("resident/0", &mapped)
            .is_err()
    );
}

#[test]
fn native_budget_csv_preserves_active_admission_and_stale_revision_refusals() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mapped = presentation(
        &mut runtime,
        vec![NativeBudgetExportView {
            id: "summary".into(),
            name: "Budget Summary".into(),
            collection_id: "budget_summary".into(),
        }],
    );

    assert!(
        runtime
            .export_native_budget_csv_workbook("resident/1", &mapped)
            .is_err()
    );

    let mut missing_active = mapped.clone();
    missing_active.active_view = "missing".into();
    assert!(
        runtime
            .export_native_budget_csv_workbook("resident/0", &missing_active)
            .is_err()
    );

    let mut missing_mapping = mapped;
    missing_mapping.collections[1].collection_id = "budget_items".into();
    assert!(
        runtime
            .export_native_budget_csv_workbook("resident/0", &missing_mapping)
            .is_err()
    );
}

#[test]
fn native_budget_maps_formula_dependencies_once_and_keeps_aliases_presentation_only() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mapped = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "items-primary".into(),
                name: "Budget Items".into(),
                collection_id: "budget_items".into(),
            },
            NativeBudgetExportView {
                id: "items-alias".into(),
                name: "Planning desk".into(),
                collection_id: "budget_items".into(),
            },
            NativeBudgetExportView {
                id: "summary-primary".into(),
                name: "Budget Summary".into(),
                collection_id: "budget_summary".into(),
            },
        ],
    );
    let workbook = runtime
        .export_native_budget_workbook("resident/0", &mapped)
        .expect("native Budget should export through the shared writer");

    assert_eq!(
        workbook.sheets.len(),
        2,
        "aliases must not create worksheets"
    );
    assert_eq!(
        workbook
            .sheets
            .iter()
            .map(|sheet| sheet.name.as_str())
            .collect::<Vec<_>>(),
        ["Budget Items", "Budget Summary"]
    );
    assert_eq!(
        workbook
            .sheets
            .iter()
            .filter(|sheet| sheet.name == "Budget Items")
            .count(),
        1,
        "the formula dependency source collection must map once"
    );
    assert_eq!(
        workbook
            .sheets
            .iter()
            .filter(|sheet| sheet.name == "Budget Summary")
            .count(),
        1
    );

    let summary = workbook
        .sheets
        .iter()
        .find(|sheet| sheet.name == "Budget Summary")
        .unwrap();
    let planned_total = summary
        .columns
        .iter()
        .position(|column| column.name == "planned_total")
        .unwrap();
    let formula = summary
        .rows
        .iter()
        .filter_map(|row| row.get(planned_total))
        .find_map(|cell| cell.formula.as_deref())
        .unwrap();
    assert!(formula.contains("'Budget Items'!"));
    assert_eq!(formula.matches("'Budget Items'!").count(), 2);
    assert!(matches!(
        summary.rows[0][planned_total].value,
        SourceValue::Number { value } if (value - 1380.0).abs() < f64::EPSILON
    ));
    export_xlsx(&workbook).expect("native Budget workbook remains in shared writer bounds");
}

#[test]
fn native_budget_includes_formula_source_collections_without_requiring_alias_views() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mapped = presentation(
        &mut runtime,
        vec![NativeBudgetExportView {
            id: "items".into(),
            name: "Budget Items".into(),
            collection_id: "budget_items".into(),
        }],
    );
    let workbook = runtime
        .export_native_budget_workbook("resident/0", &mapped)
        .expect("formula source collections are not dependent on alias tabs");

    assert_eq!(workbook.sheets.len(), 2);
    assert_eq!(
        workbook
            .sheets
            .iter()
            .map(|sheet| sheet.name.as_str())
            .collect::<Vec<_>>(),
        ["Budget Items", "budget_summary"]
    );
    assert_eq!(
        workbook
            .sheets
            .iter()
            .filter(|sheet| sheet.name == "budget_summary")
            .count(),
        1,
        "the summary formula source collection still maps exactly once"
    );
}

#[test]
fn native_budget_rejects_excel_invalid_edge_apostrophes_in_view_names() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    for name in ["'Budget Items", "Budget Items'"] {
        let mapped = presentation(
            &mut runtime,
            vec![
                NativeBudgetExportView {
                    id: "items".into(),
                    name: name.into(),
                    collection_id: "budget_items".into(),
                },
                NativeBudgetExportView {
                    id: "summary".into(),
                    name: "Budget Summary".into(),
                    collection_id: "budget_summary".into(),
                },
            ],
        );
        assert!(
            runtime
                .export_native_budget_workbook("resident/0", &mapped)
                .is_err(),
            "edge apostrophe should not produce an Excel-invalid worksheet: {name:?}"
        );
    }
}

#[test]
fn native_budget_rejects_worksheet_collisions_and_invalid_identity_mappings_atomically() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let before = runtime.export_project("resident/0").unwrap().bytes;
    let collision = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "items".into(),
                name: "Budget".into(),
                collection_id: "budget_items".into(),
            },
            NativeBudgetExportView {
                id: "summary".into(),
                name: "budget".into(),
                collection_id: "budget_summary".into(),
            },
        ],
    );
    assert!(
        runtime
            .export_native_budget_workbook("resident/0", &collision)
            .is_err()
    );

    let mut duplicate_row = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "items".into(),
                name: "Budget Items".into(),
                collection_id: "budget_items".into(),
            },
            NativeBudgetExportView {
                id: "summary".into(),
                name: "Budget Summary".into(),
                collection_id: "budget_summary".into(),
            },
        ],
    );
    duplicate_row.collections[0].rows[1].entity_id =
        duplicate_row.collections[0].rows[0].entity_id.clone();
    assert!(
        runtime
            .export_native_budget_workbook("resident/0", &duplicate_row)
            .is_err()
    );
    assert_eq!(runtime.export_project("resident/0").unwrap().bytes, before);
}

#[test]
fn native_budget_rejects_missing_source_collection_and_invalid_view_identity() {
    let mut runtime = DesignerRuntime::budget(OCCURRENCE).unwrap();
    let mut missing_collection = presentation(
        &mut runtime,
        vec![NativeBudgetExportView {
            id: "items".into(),
            name: "Budget Items".into(),
            collection_id: "budget_items".into(),
        }],
    );
    missing_collection.collections.pop();
    assert!(
        runtime
            .export_native_budget_workbook("resident/0", &missing_collection)
            .is_err()
    );

    let mut invalid_view = presentation(
        &mut runtime,
        vec![
            NativeBudgetExportView {
                id: "items".into(),
                name: "Budget Items".into(),
                collection_id: "budget_items".into(),
            },
            NativeBudgetExportView {
                id: "summary".into(),
                name: "Budget Summary".into(),
                collection_id: "missing\0collection".into(),
            },
        ],
    );
    invalid_view.active_view = "items".into();
    assert!(
        runtime
            .export_native_budget_workbook("resident/0", &invalid_view)
            .is_err()
    );
}
