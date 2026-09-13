use tachiko_designer_runtime::{DesignerRequest, DesignerRuntime, NewTableColumnInput};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000317";

fn inventory() -> DesignerRuntime {
    DesignerRuntime::new_table(
        OCCURRENCE,
        "Inventory",
        &[
            NewTableColumnInput {
                name: "item".to_owned(),
                field_type: "text".to_owned(),
            },
            NewTableColumnInput {
                name: "quantity".to_owned(),
                field_type: "number".to_owned(),
            },
            NewTableColumnInput {
                name: "active".to_owned(),
                field_type: "boolean".to_owned(),
            },
            NewTableColumnInput {
                name: "received".to_owned(),
                field_type: "date".to_owned(),
            },
        ],
    )
    .expect("the bounded Inventory fixture must admit")
}

fn table(
    runtime: &mut DesignerRuntime,
    collection: &str,
) -> tachiko_designer_runtime::TableProjection {
    let tachiko_designer_runtime::DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .expect("the fixture table must be queryable")
    else {
        panic!("query_table must return a table projection");
    };
    table
}

#[test]
fn native_table_profile_requires_complete_typed_rows_without_publication() {
    let mut runtime = inventory();
    let initial = table(&mut runtime, "inventory");
    assert_eq!(initial.native_table_profile, Some(true));
    let before = runtime
        .export_project(&initial.revision)
        .expect("the initial Inventory must export")
        .bytes;

    let result = runtime.handle(DesignerRequest::PasteCells {
        expected_revision: initial.revision.clone(),
        collection: initial.collection.id,
        start_entity: None,
        start_field: initial.columns[0].id.clone(),
        rows: vec![vec!["only the item".to_owned()]],
    });

    assert!(
        result.is_err(),
        "partial new row must reject before publication"
    );
    assert_eq!(
        runtime
            .export_project(&initial.revision)
            .expect("rejected paste must keep the initial revision exportable")
            .bytes,
        before,
    );
    assert!(table(&mut runtime, "inventory").rows.is_empty());
}

#[test]
fn row_paste_rejects_non_native_collections_before_row_construction() {
    let mut runtime = DesignerRuntime::moonfall(OCCURRENCE).expect("Moonfall fixture must admit");
    let initial = table(&mut runtime, "weapons");
    assert_ne!(initial.native_table_profile, Some(true));
    let before = runtime
        .export_project(&initial.revision)
        .expect("Moonfall must export before the rejected request")
        .bytes;

    let result = runtime.handle(DesignerRequest::PasteCells {
        expected_revision: initial.revision.clone(),
        collection: initial.collection.id,
        start_entity: None,
        start_field: initial.columns[0].id.clone(),
        rows: vec![vec!["must not append".to_owned()]],
    });

    assert!(result.is_err(), "non-native row paste must be refused");
    assert_eq!(
        runtime
            .export_project(&initial.revision)
            .expect("rejected paste must preserve the Moonfall export")
            .bytes,
        before,
    );
}
