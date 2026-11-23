use super::{
    CalculationProjection, DesignerRequest, DesignerRuntime, FieldTarget, NewTableColumnInput,
    RowInitializer, ScalarEditInput,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000593";

fn number_column(name: &str) -> NewTableColumnInput {
    NewTableColumnInput {
        name: name.into(),
        field_type: "number".into(),
        required: false,
    }
}

/// Amounts 3200 and 9800 with a blank Tax column.
fn fixture() -> (DesignerRuntime, String, String, String, Vec<String>) {
    let mut runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "Expenses",
        &[number_column("Amount"), number_column("Tax")],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let columns = runtime.query_table(&collection).unwrap().columns;
    let amount = columns[0].id.clone();
    let tax = columns[1].id.clone();
    for input in ["3200", "9800"] {
        let revision = runtime.current_revision().to_owned();
        runtime
            .insert_row(
                None,
                &revision,
                &collection,
                &[RowInitializer {
                    field: amount.clone(),
                    input: ScalarEditInput::Number {
                        input: input.into(),
                    },
                }],
            )
            .unwrap();
    }
    let rows = runtime
        .query_table(&collection)
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.id)
        .collect();
    (runtime, collection, amount, tax, rows)
}

/// `None` for a blank cell, else its calculated value.
fn tax_value(runtime: &DesignerRuntime, collection: &str, tax: &str, row: usize) -> Option<f64> {
    let row = &runtime.query_table(collection).unwrap().rows[row];
    if row.blank_fields.iter().any(|field| field == tax) {
        return None;
    }
    let field = row
        .fields
        .iter()
        .find(|field| field.target.field == tax)
        .unwrap();
    match field.calculated {
        Some(CalculationProjection::Value { value, .. }) => Some(value),
        ref other => panic!("expected a calculated value, got {other:?}"),
    }
}

fn history(runtime: &mut DesignerRuntime, redo: bool) {
    let expected_revision = runtime.current_revision().to_owned();
    runtime
        .handle(if redo {
            DesignerRequest::Redo { expected_revision }
        } else {
            DesignerRequest::Undo { expected_revision }
        })
        .unwrap();
}

#[test]
fn positional_formula_clear_and_replace_restore_formula_and_earlier_undo_entries() {
    for input in [
        ScalarEditInput::Clear,
        ScalarEditInput::Number { input: "42".into() },
    ] {
        let (mut runtime, collection, _, tax, rows) = fixture();
        let before_formula = runtime.session.document().clone();
        let target = FieldTarget {
            entity: rows[0].clone(),
            field: tax.clone(),
        };
        let revision = runtime.current_revision().to_owned();
        runtime
            .update_formula(&revision, &target, "[.amount]*0.1")
            .unwrap();
        let with_formula = runtime.session.document().clone();
        let column = runtime
            .query_table(&collection)
            .unwrap()
            .columns
            .iter()
            .position(|column| column.id == tax)
            .unwrap()
            + 1;
        let revision = runtime.current_revision().to_owned();
        runtime
            .write_cell_at(&revision, &collection, 1, column, &input)
            .unwrap();
        let edited = runtime.query_table(&collection).unwrap();
        assert!(
            edited.rows[0]
                .fields
                .iter()
                .all(|field| field.formula.is_none())
        );
        history(&mut runtime, false);
        assert_eq!(runtime.session.document(), &with_formula);
        assert_eq!(tax_value(&runtime, &collection, &tax, 0), Some(320.0));
        history(&mut runtime, false);
        assert_eq!(runtime.session.document(), &before_formula);
        history(&mut runtime, false);
        assert_eq!(runtime.query_table(&collection).unwrap().rows.len(), 1);
    }
}

#[test]
fn formula_into_blank_cell_calculates_and_undoes_to_blank() {
    let (mut runtime, collection, _, tax, rows) = fixture();
    assert_eq!(tax_value(&runtime, &collection, &tax, 0), None);
    let revision = runtime.current_revision().to_owned();
    let target = FieldTarget {
        entity: rows[0].clone(),
        field: tax.clone(),
    };
    runtime
        .update_formula(&revision, &target, "[.amount]*0.1")
        .unwrap();
    assert_eq!(tax_value(&runtime, &collection, &tax, 0), Some(320.0));

    history(&mut runtime, false);
    assert_eq!(tax_value(&runtime, &collection, &tax, 0), None);
    history(&mut runtime, true);
    assert_eq!(tax_value(&runtime, &collection, &tax, 0), Some(320.0));
}

#[test]
fn formula_copy_into_blank_cells_calculates_and_undoes_to_blank() {
    let (mut runtime, collection, _, tax, rows) = fixture();
    let source = FieldTarget {
        entity: rows[0].clone(),
        field: tax.clone(),
    };
    let revision = runtime.current_revision().to_owned();
    runtime
        .update_formula(&revision, &source, "[.amount]*0.1")
        .unwrap();
    let revision = runtime.current_revision().to_owned();
    runtime
        .copy_formula(
            &revision,
            &source,
            &[FieldTarget {
                entity: rows[1].clone(),
                field: tax.clone(),
            }],
            &[],
            true,
            false,
        )
        .unwrap();
    assert_eq!(tax_value(&runtime, &collection, &tax, 1), Some(980.0));

    history(&mut runtime, false);
    assert_eq!(tax_value(&runtime, &collection, &tax, 1), None);
    assert_eq!(tax_value(&runtime, &collection, &tax, 0), Some(320.0));
    history(&mut runtime, true);
    assert_eq!(tax_value(&runtime, &collection, &tax, 1), Some(980.0));
}
