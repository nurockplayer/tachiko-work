use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ColumnRef, DesignerRequest, DesignerResponse, DesignerRuntime, DuplicateCollectionProjection,
    Expression, FieldId, SchemaId, SelectionStatisticsRange, TableView, TableWindowProjection,
    blank_workbook, collection_copy_formula::CopyReferences, open_project_working_size,
};

fn source() -> DesignerRuntime {
    let (mut runtime, _) =
        blank_workbook::candidate("00000000-0000-4000-8000-000000000964").unwrap();
    runtime
        .handle(DesignerRequest::PasteCellsAt {
            expected_revision: "resident/0".into(),
            collection: "sheet".into(),
            row: 1,
            column: 1,
            rows: [
                ["Amount", "=20+2", "External"],
                ["2", "=[.column_1]+[.column_3]", "10"],
                ["3", "7", "20"],
            ]
            .into_iter()
            .map(|row| row.into_iter().map(Into::into).collect())
            .collect(),
        })
        .unwrap();
    runtime
}

fn create(runtime: &mut DesignerRuntime, has_header: bool) -> DuplicateCollectionProjection {
    let spec = &runtime.collection_specs["sheet"];
    let range = SelectionStatisticsRange {
        collection: "sheet".into(),
        first: spec.entities[0].clone(),
        last: spec.entities[2].clone(),
        fields: spec.columns[..2]
            .iter()
            .map(|field| field.id.to_string())
            .collect(),
    };
    let DesignerResponse::Duplicated(created) = runtime
        .handle(DesignerRequest::RangeToTable {
            expected_revision: "resident/1".into(),
            range,
            name: "Copied".into(),
            has_header,
        })
        .unwrap()
    else {
        panic!("created table expected");
    };
    created
}

fn table(runtime: &mut DesignerRuntime, key: &str) -> TableWindowProjection {
    let revision = runtime.current_revision().to_owned();
    runtime
        .query_table_window(&revision, key, &TableView::default(), 0, 10, true)
        .unwrap()
}

fn assert_formula_targets(runtime: &DesignerRuntime, copied: &str, row: usize) {
    let document = runtime.session.document();
    let source = &runtime.collection_specs["sheet"];
    let target = &runtime.collection_specs[copied];
    let owner = crate::FieldRef::new(target.entities[row].clone(), target.columns[1].id.clone());
    let crate::Value::Formula(expression) = &document.entities[&owner.entity].fields[&owner.field]
    else {
        panic!("copied formula expected");
    };
    assert_eq!(
        expression.unconditional_references(&owner),
        BTreeSet::from([
            crate::FieldRef::new(owner.entity.clone(), target.columns[0].id.clone()),
            crate::FieldRef::new(source.entities[1].clone(), source.columns[2].id.clone()),
        ])
    );
}

#[test]
fn range_to_table_publishes_one_undoable_and_saveable_table_for_each_header_option() {
    for has_header in [true, false] {
        let mut runtime = source();
        let before = runtime.session.document().clone();
        let created = create(&mut runtime, has_header);
        assert_eq!(created.publication.resulting_revision, "resident/2");
        let key = &created.collection.key;
        let copied = table(&mut runtime, key);
        let (headers, values) = if has_header {
            (["Amount", "22"], vec![["2", "12"], ["3", "7"]])
        } else {
            (
                ["Column A", "Column B"],
                vec![["Amount", "22"], ["2", "12"], ["3", "7"]],
            )
        };
        assert_eq!(
            copied
                .columns
                .iter()
                .map(|column| column.display_name.as_str())
                .collect::<Vec<_>>(),
            headers
        );
        assert_eq!(
            copied
                .rows
                .iter()
                .map(|row| {
                    std::array::from_fn(|index| {
                        let field = row
                            .fields
                            .iter()
                            .find(|field| field.target.field == copied.columns[index].id)
                            .unwrap();
                        if let Some(crate::StoredValueProjection::Text { value }) = &field.stored {
                            value.as_str()
                        } else {
                            field.display.as_deref().unwrap()
                        }
                    })
                })
                .collect::<Vec<[_; 2]>>(),
            values
        );
        assert_formula_targets(&runtime, key, usize::from(!has_header));

        let saved = runtime.export_project_v3("resident/2").unwrap();
        let mut reopened = None;
        open_project_working_size(
            &mut reopened,
            &saved.bytes,
            "00000000-0000-4000-8000-000000000965",
        )
        .unwrap();
        let mut reopened = reopened.unwrap();
        let reopened_table = table(&mut reopened, key);
        assert_eq!(reopened_table.collection, copied.collection);
        assert_eq!(reopened_table.columns, copied.columns);
        assert_eq!(reopened_table.rows, copied.rows);
        assert_formula_targets(&reopened, key, usize::from(!has_header));

        runtime
            .handle(DesignerRequest::Undo {
                expected_revision: "resident/2".into(),
            })
            .unwrap();
        assert_eq!(runtime.session.document(), &before);
    }
}

#[test]
fn duplicated_expressions_remap_column_aggregates() {
    let mut expression = Expression::ColumnAggregate {
        function: tachiko_workspace_engine::ColumnAggregateFunction::Average,
        column: ColumnRef::new("plan", "reserve"),
    };
    CopyReferences::whole(
        &BTreeMap::new(),
        &BTreeMap::from([(FieldId::from("reserve"), FieldId::from("reserve_copy"))]),
        &BTreeMap::from([(SchemaId::from("plan"), SchemaId::from("plan_copy_schema"))]),
    )
    .expression(&mut expression)
    .unwrap();
    assert_eq!(
        expression,
        Expression::ColumnAggregate {
            function: tachiko_workspace_engine::ColumnAggregateFunction::Average,
            column: ColumnRef::new("plan_copy_schema", "reserve_copy"),
        }
    );
}

#[test]
fn duplicated_lookups_move_own_columns_and_keep_foreign_ones() {
    let lookup = |schema: &str, key: &str, result: &str| Expression::Lookup {
        key: Box::new(Expression::SameRowReference(FieldId::from("name"))),
        key_column: ColumnRef::new(schema, key),
        result_column: ColumnRef::new(schema, result),
        if_missing: Some(Box::new(Expression::SameRowReference(FieldId::from(
            "reserve",
        )))),
    };
    let mut own = lookup("plan", "name", "reserve");
    let mut foreign = lookup("settings", "label", "rate");
    let fields = BTreeMap::from([
        (FieldId::from("name"), FieldId::from("name_copy")),
        (FieldId::from("reserve"), FieldId::from("reserve_copy")),
    ]);
    let schemas = BTreeMap::from([(SchemaId::from("plan"), SchemaId::from("plan_copy_schema"))]);
    for expression in [&mut own, &mut foreign] {
        CopyReferences::whole(&BTreeMap::new(), &fields, &schemas)
            .expression(expression)
            .expect("the duplicated lookup maps");
    }
    let copied = |schema: &str, key: &str, result: &str| Expression::Lookup {
        key: Box::new(Expression::SameRowReference(FieldId::from("name_copy"))),
        key_column: ColumnRef::new(schema, key),
        result_column: ColumnRef::new(schema, result),
        if_missing: Some(Box::new(Expression::SameRowReference(FieldId::from(
            "reserve_copy",
        )))),
    };
    assert_eq!(own, copied("plan_copy_schema", "name_copy", "reserve_copy"));
    assert_eq!(foreign, copied("settings", "label", "rate"));
}

#[test]
fn duplicated_expressions_move_own_column_totals_and_keep_foreign_ones() {
    let mut expression = Expression::Add {
        left: Box::new(Expression::ColumnTotal(ColumnRef::new("plan", "reserve"))),
        right: Box::new(Expression::ColumnTotal(ColumnRef::new("settings", "rate"))),
    };
    CopyReferences::whole(
        &BTreeMap::new(),
        &BTreeMap::from([(FieldId::from("reserve"), FieldId::from("reserve_copy"))]),
        &BTreeMap::from([(SchemaId::from("plan"), SchemaId::from("plan_copy_schema"))]),
    )
    .expression(&mut expression)
    .expect("the duplicated expression maps");
    assert_eq!(
        expression,
        Expression::Add {
            left: Box::new(Expression::ColumnTotal(ColumnRef::new(
                "plan_copy_schema",
                "reserve_copy"
            ))),
            right: Box::new(Expression::ColumnTotal(ColumnRef::new("settings", "rate"))),
        }
    );
}
