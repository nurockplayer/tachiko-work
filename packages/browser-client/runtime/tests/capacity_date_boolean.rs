//! Work #610: Date and Yes/No columns in imports over 64 rows.
use std::fmt::Write as _;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget, ImportFieldType,
    ImportSelection, InteropMetadata, ScalarEditInput, StoredValueProjection, TableProjection,
    TableView, import_workbook,
    interop_adapter::{
        ImportOptions, SourceValue, export_csv, export_xlsx, import_csv, import_xlsx,
    },
    open_imported_project,
};
use tachiko_workspace_engine::Date;

const ORIGINAL: &str = "00000000-0000-4000-8000-000000000610";
const FRESH: &str = "00000000-0000-4000-8000-000000000611";

/// `Item,When,Amount,Paid` (or `When,Amount,Paid`) with one blank Date every
/// seventh row and one blank Yes/No every eleventh row.
fn csv_rows(rows: usize, item: bool) -> Vec<Vec<String>> {
    let header: &[&str] = if item {
        &["Item", "When", "Amount", "Paid"]
    } else {
        &["When", "Amount", "Paid"]
    };
    let mut out = Vec::with_capacity(rows + 1);
    out.push(header.iter().map(|name| (*name).to_owned()).collect());
    for index in 0..rows {
        let when = if index % 7 == 0 {
            String::new()
        } else {
            format!("2026-{:02}-{:02}", index % 12 + 1, index % 28 + 1)
        };
        let amount = format!("{index}.25");
        let paid = if index % 11 == 0 {
            String::new()
        } else {
            (index % 2 == 0).to_string()
        };
        let mut row = Vec::with_capacity(header.len());
        if item {
            row.push(format!("item-{index}"));
        }
        row.extend([when, amount, paid]);
        out.push(row);
    }
    out
}

fn render(rows: &[Vec<String>]) -> Vec<u8> {
    rows.iter()
        .map(|row| row.join(","))
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

fn csv(rows: usize, item: bool) -> Vec<u8> {
    render(&csv_rows(rows, item))
}

fn selection(item: bool) -> ImportSelection {
    let mut types = Vec::new();
    if item {
        types.push(ImportFieldType::Text);
    }
    types.extend([
        ImportFieldType::Date,
        ImportFieldType::Number,
        ImportFieldType::Boolean,
    ]);
    ImportSelection {
        column_types: vec![types],
        extra_columns: vec![Vec::new()],
    }
}

/// Read the whole capacity table through bounded windows: `queryTable` refuses
/// above 1,000 rows.
fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let revision = runtime.observe_occurrence().revision;
    let mut rows = Vec::new();
    let mut columns = None;
    let mut summary = None;
    let mut start = 0;
    loop {
        let DesignerResponse::TableWindow(window) = runtime
            .handle(DesignerRequest::QueryTableWindow {
                expected_revision: revision.clone(),
                collection: collection.into(),
                view: TableView::default(),
                start,
                count: 128,
            })
            .unwrap()
        else {
            panic!("table window response");
        };
        if columns.is_none() {
            columns = Some(window.columns);
            summary = Some(window.collection);
        }
        let received = window.rows.len();
        rows.extend(window.rows);
        start += received;
        if received == 0 || start >= window.total_rows {
            break;
        }
    }
    TableProjection {
        revision,
        tracker_profile: None,
        native_table_profile: None,
        collection: summary.expect("the first window carries the collection"),
        columns: columns.expect("the first window carries the columns"),
        rows,
        column_totals: std::collections::BTreeMap::new(),
    }
}

fn stored(
    projection: &TableProjection,
    row: usize,
    column: usize,
) -> Option<StoredValueProjection> {
    let column = &projection.columns[column].id;
    projection.rows[row]
        .fields
        .iter()
        .find(|field| &field.target.field == column)
        .and_then(|field| field.stored.clone())
}

fn target(projection: &TableProjection, row: usize, column: usize) -> FieldTarget {
    FieldTarget {
        entity: projection.rows[row].id.clone(),
        field: projection.columns[column].id.clone(),
    }
}

fn publish(runtime: &mut DesignerRuntime, request: DesignerRequest) {
    let before = runtime.observe_occurrence();
    let DesignerResponse::Published(result) = runtime.handle(request).unwrap() else {
        panic!("publication response");
    };
    assert_eq!(result.base_revision, before.revision);
    assert_ne!(result.resulting_revision, before.revision);
    assert_eq!(runtime.observe_occurrence().scope, before.scope);
    assert_eq!(
        runtime.observe_occurrence().revision,
        result.resulting_revision
    );
}

fn exports(runtime: &DesignerRuntime, metadata: &InteropMetadata) -> [Vec<u8>; 2] {
    let workbook = runtime
        .export_workbook(&runtime.observe_occurrence().revision, metadata)
        .unwrap();
    [
        export_csv(&workbook.sheets[0]).unwrap(),
        export_xlsx(&workbook).unwrap(),
    ]
}

fn project_bytes(runtime: &DesignerRuntime, carrier: &str) -> Vec<u8> {
    let revision = runtime.observe_occurrence().revision;
    match carrier {
        "opaque" => runtime.export_project(&revision).unwrap().bytes,
        "canonical" => runtime.export_canonical_tree(&revision).unwrap().bytes,
        _ => panic!("unknown project carrier {carrier}"),
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One generated rectangle and its whole typed lifecycle stay together.
fn typed_6304_by_4_complete_lifecycle() {
    let source = import_csv(&csv(6_304, true), &ImportOptions::default()).unwrap();
    let (mut runtime, imported) = import_workbook(&source, &selection(true), ORIGINAL)
        .expect("6,304 by 4 Date and Yes/No import must be admitted");
    let collection = imported.opened.bootstrap.default_collection.clone();
    {
        let opened = &imported.opened.table;
        assert_eq!(opened.rows.len(), 6_304);
        assert_eq!(opened.collection.entity_count, 6_304);
        assert_eq!(
            opened
                .columns
                .iter()
                .map(|column| column.field_type.as_str())
                .collect::<Vec<_>>(),
            ["text", "date", "number", "boolean"]
        );
        assert_eq!(stored(opened, 0, 1), None);
        assert_eq!(
            stored(opened, 1, 1),
            Some(StoredValueProjection::Date {
                value: Date::parse("2026-02-02").unwrap(),
            })
        );
        assert_eq!(
            stored(opened, 1, 3),
            Some(StoredValueProjection::Boolean { value: false })
        );
    }

    let before = table(&mut runtime, &collection);
    let when = target(&before, 0, 1);
    let paid_row_one = target(&before, 1, 3);
    let paid_row_zero = target(&before, 0, 3);
    let revision = runtime.observe_occurrence().revision;
    publish(
        &mut runtime,
        DesignerRequest::EditScalar {
            expected_revision: revision,
            target: when,
            input: ScalarEditInput::Date {
                value: "2026-12-31".into(),
            },
        },
    );
    let revision = runtime.observe_occurrence().revision;
    publish(
        &mut runtime,
        DesignerRequest::EditScalar {
            expected_revision: revision,
            target: paid_row_one,
            input: ScalarEditInput::Clear,
        },
    );
    let revision = runtime.observe_occurrence().revision;
    publish(
        &mut runtime,
        DesignerRequest::EditScalar {
            expected_revision: revision,
            target: paid_row_zero,
            input: ScalarEditInput::Boolean { value: true },
        },
    );

    let after_edits = table(&mut runtime, &collection);
    assert_eq!(
        stored(&after_edits, 0, 1),
        Some(StoredValueProjection::Date {
            value: Date::parse("2026-12-31").unwrap(),
        })
    );
    assert_eq!(stored(&after_edits, 1, 3), None);
    assert_eq!(
        stored(&after_edits, 0, 3),
        Some(StoredValueProjection::Boolean { value: true })
    );

    let revision = runtime.observe_occurrence().revision;
    publish(
        &mut runtime,
        DesignerRequest::Undo {
            expected_revision: revision,
        },
    );
    let revision = runtime.observe_occurrence().revision;
    publish(
        &mut runtime,
        DesignerRequest::Redo {
            expected_revision: revision,
        },
    );
    let after_redo = table(&mut runtime, &collection);
    assert_eq!(after_redo.rows, after_edits.rows);
    assert_eq!(after_redo.columns, after_edits.columns);

    let live = table(&mut runtime, &collection);
    let saved = project_bytes(&runtime, "opaque");
    let mut resident = None;
    let opened = open_imported_project(&mut resident, &saved, &imported.metadata, FRESH)
        .expect("a saved typed capacity project reopens");
    let reopened = table(
        resident.as_mut().unwrap(),
        &opened.bootstrap.default_collection,
    );
    assert_eq!(reopened.rows, live.rows);
    assert_eq!(reopened.columns, live.columns);

    let [csv_bytes, xlsx_bytes] = exports(&runtime, &imported.metadata);
    let text = String::from_utf8(csv_bytes).unwrap();
    assert!(text.contains("2026-12-31"));
    assert!(text.contains("true"));
    let exported = import_xlsx(&xlsx_bytes).unwrap();
    assert_eq!(exported.sheets[0].rows.len(), 6_304);
    assert_eq!(
        exported.sheets[0].rows[1][1].value,
        SourceValue::Date {
            value: "2026-02-02".into(),
        }
    );
}

#[test]
fn typed_capacity_xlsx_reimports_as_dates() {
    let source = import_csv(&csv(200, true), &ImportOptions::default()).unwrap();
    let (runtime, imported) = import_workbook(&source, &selection(true), ORIGINAL).unwrap();
    let [_, xlsx] = exports(&runtime, &imported.metadata);
    let reread = import_xlsx(&xlsx).unwrap();
    let (runtime, again) = import_workbook(&reread, &selection(true), FRESH)
        .expect("the large-import XLSX must import again with Date selected");
    assert_eq!(again.opened.table.rows.len(), 200);
    let types = |table: &TableProjection| {
        table
            .columns
            .iter()
            .map(|column| column.field_type.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(types(&again.opened.table), types(&imported.opened.table));
    let [_, second] = exports(&runtime, &again.metadata);
    assert_eq!(
        import_xlsx(&second).unwrap().sheets[0].rows,
        reread.sheets[0].rows
    );
}

#[test]
fn typed_8406_by_3_imports_and_exports() {
    let source = import_csv(&csv(8_406, false), &ImportOptions::default()).unwrap();
    let (runtime, imported) = import_workbook(&source, &selection(false), ORIGINAL)
        .expect("8,406 by 3 Date and Yes/No import must be admitted");
    assert_eq!(imported.opened.table.rows.len(), 8_406);
    let [csv_bytes, xlsx_bytes] = exports(&runtime, &imported.metadata);
    let text = String::from_utf8(csv_bytes).unwrap();
    assert_eq!(text.lines().count(), 8_407, "header and every row");
    assert!(text.contains("2026-02-02,1.25,false"));
    assert_eq!(
        import_xlsx(&xlsx_bytes).unwrap().sheets[0].rows.len(),
        8_406
    );
}

fn assert_refused(bytes: &[u8], item: bool) {
    // An oversized rectangle is refused while the source workbook is parsed;
    // a bad typed cell is refused when the selection is admitted.
    if let Ok(source) = import_csv(bytes, &ImportOptions::default()) {
        assert!(
            import_workbook(&source, &selection(item), ORIGINAL).is_err(),
            "this source and selection must be refused"
        );
    }
}

#[test]
fn typed_capacity_refusals() {
    assert_refused(&csv(8_406, true), true);

    let mut invalid_date = csv_rows(100, true);
    invalid_date[6][1] = "10/19/2026".into();
    assert_refused(&render(&invalid_date), true);

    let mut invalid_boolean = csv_rows(100, true);
    invalid_boolean[6][3] = "yes".into();
    assert_refused(&render(&invalid_boolean), true);
}

#[test]
fn text_number_capacity_unchanged() {
    let mut source = String::from("Item,Amount\n");
    for row in 0..100 {
        writeln!(source, "item-{row},{row}.25").unwrap();
    }
    let workbook = import_csv(source.as_bytes(), &ImportOptions::default()).unwrap();
    let (runtime, imported) = import_workbook(
        &workbook,
        &ImportSelection {
            column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Number]],
            extra_columns: vec![Vec::new()],
        },
        ORIGINAL,
    )
    .unwrap();
    assert_eq!(imported.opened.table.rows.len(), 100);
    let [csv_bytes, xlsx_bytes] = exports(&runtime, &imported.metadata);
    assert_eq!(import_xlsx(&xlsx_bytes).unwrap().sheets[0].rows.len(), 100);
    let text = String::from_utf8(csv_bytes).unwrap();
    assert_eq!(text.replace("\r\n", "\n"), source);
}
