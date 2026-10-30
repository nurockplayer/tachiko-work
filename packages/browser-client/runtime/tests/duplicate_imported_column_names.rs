use std::collections::BTreeMap;

use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, ImportFieldType, ImportSelection,
    TableProjection, import_workbook,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000552";

fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .unwrap()
    else {
        panic!("expected table projection");
    };
    table
}

fn labels(table: &TableProjection) -> Vec<(String, Option<bool>)> {
    table
        .columns
        .iter()
        .map(|column| (column.display_name.clone(), column.display_name_explicit))
        .collect()
}

#[test]
fn duplicate_of_imported_csv_gives_the_copy_its_header_names() {
    let csv = "note,amount,other\n,,x\npresent,1,\n";
    let workbook = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
    let (mut runtime, imported) = import_workbook(
        &workbook,
        &ImportSelection {
            column_types: vec![vec![
                ImportFieldType::Text,
                ImportFieldType::Number,
                ImportFieldType::Text,
            ]],
            extra_columns: vec![vec![]],
        },
        OCCURRENCE,
    )
    .unwrap();
    let source_key = imported.opened.table.collection.key.clone();
    let source = table(&mut runtime, &source_key);
    let source_labels = labels(&source);
    assert!(
        source
            .columns
            .iter()
            .all(|column| column.display_name_explicit.is_none())
    );
    let headers = ["Note", "Amount", "Other"];
    let column_names = source
        .columns
        .iter()
        .zip(headers)
        .map(|(column, header)| (column.id.clone(), header.to_owned()))
        .collect::<BTreeMap<_, _>>();

    let DesignerResponse::Duplicated(copy) = runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: "resident/0".to_owned(),
            collection: source_key.clone(),
            name: "Copy".to_owned(),
            column_names,
        })
        .unwrap()
    else {
        panic!("expected a duplicated collection");
    };
    let copied = table(&mut runtime, &copy.collection.key);
    assert_eq!(
        labels(&copied),
        headers
            .iter()
            .map(|header| ((*header).to_owned(), Some(true)))
            .collect::<Vec<_>>()
    );
    assert_eq!(labels(&table(&mut runtime, &source_key)), source_labels);
}
