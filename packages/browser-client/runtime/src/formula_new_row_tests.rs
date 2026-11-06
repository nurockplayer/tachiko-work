use super::{
    CalculationProjection, DesignerRequest, DesignerRuntime, FieldTarget, NewTableColumnInput,
    RowInitializer, ScalarEditInput,
};

fn fixture(formula: &str) -> (DesignerRuntime, String, Vec<String>) {
    let columns = ["Hours", "Rate", "Amount"].map(|name| NewTableColumnInput {
        name: name.into(),
        field_type: "number".into(),
        required: false,
    });
    let mut runtime =
        DesignerRuntime::new_table("00000000-0000-4000-8000-000000000593", "Work", &columns)
            .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let ids = runtime
        .query_table(&collection)
        .unwrap()
        .columns
        .into_iter()
        .map(|c| c.id)
        .collect::<Vec<_>>();
    insert(&mut runtime, &collection, &ids, &[(0, "2"), (1, "3")]);
    let row = runtime.query_table(&collection).unwrap().rows[0].id.clone();
    let revision = runtime.current_revision().to_owned();
    runtime
        .update_formula(
            &revision,
            &FieldTarget {
                entity: row,
                field: ids[2].clone(),
            },
            formula,
        )
        .unwrap();
    (runtime, collection, ids)
}

fn insert(
    runtime: &mut DesignerRuntime,
    collection: &str,
    ids: &[String],
    values: &[(usize, &str)],
) -> super::PublicationProjection {
    let revision = runtime.current_revision().to_owned();
    let inputs = values
        .iter()
        .map(|(i, value)| RowInitializer {
            field: ids[*i].clone(),
            input: ScalarEditInput::Number {
                input: (*value).into(),
            },
        })
        .collect::<Vec<_>>();
    runtime.insert_row(&revision, collection, &inputs).unwrap()
}

#[test]
fn inherited_formula_is_published_and_undoable() {
    let (mut runtime, collection, ids) = fixture("[.hours]*[.rate]");
    let publication = insert(&mut runtime, &collection, &ids, &[(0, "4"), (1, "5")]);
    let table = runtime.query_table(&collection).unwrap();
    let row = &table.rows[1];
    let field = row
        .fields
        .iter()
        .find(|f| f.target.field == ids[2])
        .unwrap();
    assert_eq!(field.formula.as_ref().unwrap().source, "[.hours]*[.rate]");
    assert!(
        matches!(field.calculated, Some(CalculationProjection::Value { value }) if value == 20.0)
    );
    assert!(publication.fields.contains(&field.target));
    for redo in [false, true] {
        let expected_revision = runtime.current_revision().to_owned();
        runtime
            .handle(if redo {
                DesignerRequest::Redo { expected_revision }
            } else {
                DesignerRequest::Undo { expected_revision }
            })
            .unwrap();
        assert_eq!(
            runtime.query_table(&collection).unwrap().rows.len(),
            if redo { 2 } else { 1 }
        );
    }
}

#[test]
fn typed_value_ends_qualification_and_blank_dependency_is_zero() {
    let (mut runtime, collection, ids) = fixture("[.hours]*[.rate]");
    insert(&mut runtime, &collection, &ids, &[(1, "5")]);
    let table = runtime.query_table(&collection).unwrap();
    let field = table.rows[1]
        .fields
        .iter()
        .find(|f| f.target.field == ids[2])
        .unwrap();
    assert!(
        matches!(field.calculated, Some(CalculationProjection::Value { value }) if value == 0.0)
    );
    insert(&mut runtime, &collection, &ids, &[(2, "7")]);
    insert(&mut runtime, &collection, &ids, &[]);
    assert!(
        runtime.query_table(&collection).unwrap().rows[3]
            .blank_fields
            .contains(&ids[2])
    );
}

#[test]
fn failed_inherited_formula_does_not_prevent_insertion() {
    let (mut runtime, collection, ids) = fixture("[.hours]/[.rate]");
    insert(&mut runtime, &collection, &ids, &[]);
    insert(&mut runtime, &collection, &ids, &[]);
    let table = runtime.query_table(&collection).unwrap();
    assert!(table.rows[1].blank_fields.contains(&ids[2]));
    assert!(table.rows[2].blank_fields.contains(&ids[2]));
}
