use tachiko_designer_runtime::interop_adapter::{CellStyle, SourceValue, export_xlsx};
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, NativeBudgetExportCollection,
    NativeBudgetExportPresentation, NativeBudgetExportRow, NativeBudgetExportView,
};

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

    assert_eq!(workbook.sheets.len(), 2, "aliases must not create worksheets");
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
    assert!(runtime
        .export_native_budget_workbook("resident/0", &collision)
        .is_err());

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
    assert!(runtime
        .export_native_budget_workbook("resident/0", &duplicate_row)
        .is_err());
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
    assert!(runtime
        .export_native_budget_workbook("resident/0", &missing_collection)
        .is_err());

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
    assert!(runtime
        .export_native_budget_workbook("resident/0", &invalid_view)
        .is_err());
}
