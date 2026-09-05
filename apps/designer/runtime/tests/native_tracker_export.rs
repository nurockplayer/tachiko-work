use tachiko_designer_runtime::interop_adapter::{CellStyle, SourceValue, export_csv, export_xlsx};
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, NativeTrackerExportPresentation,
    NativeTrackerExportRow,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000296";

fn presentation(runtime: &mut DesignerRuntime, reverse: bool) -> NativeTrackerExportPresentation {
    let DesignerResponse::Table(mut table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "tracker".to_owned(),
        })
        .expect("Tracker table should be queryable")
    else {
        panic!("expected Tracker table");
    };
    if reverse {
        table.rows.reverse();
    }
    NativeTrackerExportPresentation {
        version: 1,
        rows: table
            .rows
            .into_iter()
            .map(|row| NativeTrackerExportRow {
                entity_id: row.id,
                styles: vec![CellStyle::default(); 3],
            })
            .collect(),
    }
}

fn populate(runtime: &mut DesignerRuntime, count: usize) -> String {
    let mut revision = "resident/0".to_owned();
    for chunk in (1..=count)
        .map(|index| {
            vec![
                format!("item-{index:03}"),
                index.to_string(),
                (index % 2 == 0).to_string(),
            ]
        })
        .collect::<Vec<_>>()
        .chunks(64)
    {
        let DesignerResponse::Published(publication) = runtime
            .handle(DesignerRequest::PasteCells {
                expected_revision: revision,
                collection: "tracker".to_owned(),
                start_entity: None,
                start_field: "task".to_owned(),
                rows: chunk.to_vec(),
            })
            .expect("accepted Tracker row count should publish")
        else {
            panic!("expected published Tracker rows");
        };
        revision = publication.resulting_revision;
    }
    revision
}

#[test]
fn native_tracker_outbound_profile_covers_every_admitted_row_boundary() {
    for count in [0, 1, 64, 65, 128] {
        let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
        let revision = populate(&mut runtime, count);
        let mapped = presentation(&mut runtime, false);
        let workbook = runtime
            .export_native_tracker_workbook(&revision, &mapped)
            .expect("every admitted Tracker size should export");
        let sheet = &workbook.sheets[0];
        assert_eq!(sheet.rows.len(), count);
        assert_eq!(export_csv(sheet).is_ok(), count <= 64);
        assert_eq!(export_xlsx(&workbook).is_ok(), count <= 64);
    }

    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let revision = populate(&mut runtime, 128);
    let before = runtime.export_project(&revision).unwrap().bytes;
    assert!(
        runtime
            .handle(DesignerRequest::PasteCells {
                expected_revision: revision.clone(),
                collection: "tracker".to_owned(),
                start_entity: None,
                start_field: "task".to_owned(),
                rows: vec![vec![
                    "item-129".to_owned(),
                    "129".to_owned(),
                    "false".to_owned()
                ]],
            })
            .is_err()
    );
    assert_eq!(runtime.export_project(&revision).unwrap().bytes, before);
}

#[test]
fn native_tracker_exports_all_128_rows_without_import_provenance_or_mutation() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let rows: Vec<Vec<String>> = (1..=128)
        .map(|index| {
            vec![
                format!("item-{index:03}"),
                index.to_string(),
                (index % 2 == 0).to_string(),
            ]
        })
        .collect();
    let (first, second) = rows.split_at(64);
    let DesignerResponse::Published(first_publication) = runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: "resident/0".to_owned(),
            collection: "tracker".to_owned(),
            start_entity: None,
            start_field: "task".to_owned(),
            rows: first.to_vec(),
        })
        .expect("the first bounded stock Tracker batch should be admitted")
    else {
        panic!("expected published Tracker rows");
    };
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: first_publication.resulting_revision,
            collection: "tracker".to_owned(),
            start_entity: None,
            start_field: "task".to_owned(),
            rows: second.to_vec(),
        })
        .expect("the second bounded stock Tracker batch should reach 128 rows")
    else {
        panic!("expected published Tracker rows");
    };
    let before = runtime
        .export_project(&publication.resulting_revision)
        .unwrap()
        .bytes;
    let reversed = presentation(&mut runtime, true);
    let workbook = runtime
        .export_native_tracker_workbook(&publication.resulting_revision, &reversed)
        .expect("outbound 128-row Tracker profile should export");

    let sheet = &workbook.sheets[0];
    assert_eq!(sheet.name, "Tracker");
    assert!(sheet.has_header);
    assert_eq!(sheet.rows.len(), 128);
    assert_eq!(
        sheet
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["task", "estimate", "done"]
    );
    assert_eq!(
        sheet.rows[0][0].value,
        SourceValue::Text {
            value: "item-128".to_owned()
        }
    );
    assert_eq!(
        sheet.rows[127][0].value,
        SourceValue::Text {
            value: "item-001".to_owned()
        }
    );
    let estimates = sheet
        .rows
        .iter()
        .map(|row| match row[1].value {
            SourceValue::Number { value } => value,
            _ => panic!("estimate should remain a Number"),
        })
        .collect::<Vec<_>>();
    assert_eq!(estimates.iter().sum::<f64>(), 8256.0);
    assert!(sheet.rows.iter().enumerate().all(|(index, row)| matches!(row[2].value, SourceValue::Boolean { value } if value == ((128 - index) % 2 == 0))));
    assert!(
        export_csv(sheet).is_err(),
        "shared CSV output remains capped at 64 rows"
    );
    assert!(
        export_xlsx(&workbook).is_err(),
        "shared XLSX output remains capped at 64 rows"
    );
    assert_eq!(
        runtime
            .export_project(&publication.resulting_revision)
            .unwrap()
            .bytes,
        before
    );
}

#[test]
fn native_tracker_export_rejects_partial_or_stale_presentation_without_mutation() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: "resident/0".to_owned(),
            collection: "tracker".to_owned(),
            start_entity: None,
            start_field: "task".to_owned(),
            rows: vec![vec!["one".to_owned(), "1".to_owned(), "false".to_owned()]],
        })
        .unwrap()
    else {
        panic!("expected published Tracker row");
    };
    let before = runtime
        .export_project(&publication.resulting_revision)
        .unwrap()
        .bytes;
    let mut partial = presentation(&mut runtime, false);
    partial.rows.pop();
    assert!(
        runtime
            .export_native_tracker_workbook(&publication.resulting_revision, &partial)
            .is_err()
    );
    let current = presentation(&mut runtime, false);
    assert!(
        runtime
            .export_native_tracker_workbook("resident/9", &current)
            .is_err()
    );
    assert_eq!(
        runtime
            .export_project(&publication.resulting_revision)
            .unwrap()
            .bytes,
        before
    );
}

#[test]
fn native_tracker_preserves_scalar_meaning_and_refuses_formula_like_csv_text() {
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE).unwrap();
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::PasteCells {
            expected_revision: "resident/0".to_owned(),
            collection: "tracker".to_owned(),
            start_entity: None,
            start_field: "task".to_owned(),
            rows: vec![
                vec!["=literal".to_owned(), "-1.5".to_owned(), "false".to_owned()],
                vec![
                    "000123 \"林\"\tline\nnext".to_owned(),
                    "0".to_owned(),
                    "true".to_owned(),
                ],
            ],
        })
        .expect("special scalar values should be accepted")
    else {
        panic!("expected published Tracker rows");
    };
    let mapped = presentation(&mut runtime, false);
    let workbook = runtime
        .export_native_tracker_workbook(&publication.resulting_revision, &mapped)
        .expect("native Tracker XLSX mapping should preserve supported scalars");
    let sheet = &workbook.sheets[0];
    assert!(
        matches!(sheet.rows[0][0].value, SourceValue::Text { ref value } if value == "=literal")
    );
    assert!(matches!(sheet.rows[0][1].value, SourceValue::Number { value } if value == -1.5));
    assert!(matches!(sheet.rows[1][1].value, SourceValue::Number { value } if value == 0.0));
    assert!(matches!(
        sheet.rows[0][2].value,
        SourceValue::Boolean { value: false }
    ));
    assert!(matches!(
        sheet.rows[1][2].value,
        SourceValue::Boolean { value: true }
    ));
    assert!(export_csv(sheet).is_err());
    assert!(export_xlsx(&workbook).is_ok());
}
