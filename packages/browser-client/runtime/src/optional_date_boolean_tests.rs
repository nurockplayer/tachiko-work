//! Work #598: optional Date and Boolean fields in native tables admit the same
//! blank, clear, requiredness, export and import authority as optional Text and
//! Number (ADR-0045 amendment). Blank stays distinct from `false` and any date.
use super::interop_adapter::{
    CellStyle, SourceCell, SourceColumn, SourceSheet, SourceValue, SourceWorkbook,
};
use super::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget,
    ImportFieldType, ImportSelection, NewTableColumnInput, RowInitializer, ScalarEditInput,
    TableProjection, import_workbook,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000598";
const NEXT: &str = "00000000-0000-4000-8000-000000000599";

fn column(name: &str, field_type: &str, required: bool) -> NewTableColumnInput {
    NewTableColumnInput {
        name: name.into(),
        field_type: field_type.into(),
        required,
    }
}

/// A required Name plus optional Due (Date) and Done (Boolean) columns.
fn fixture() -> (DesignerRuntime, String, Vec<String>) {
    let runtime = DesignerRuntime::new_table(
        OCCURRENCE,
        "Tasks",
        &[
            column("Name", "text", true),
            column("Due", "date", false),
            column("Done", "boolean", false),
        ],
    )
    .unwrap();
    let collection = runtime.collection_specs.keys().next().unwrap().clone();
    let columns = runtime.query_table(&collection).unwrap().columns;
    let ids = columns.iter().map(|column| column.id.clone()).collect();
    (runtime, collection, ids)
}

fn insert_named_row(runtime: &mut DesignerRuntime, collection: &str, name_field: &str, name: &str) {
    let revision = runtime.current_revision().to_owned();
    runtime
        .insert_row(
            &revision,
            collection,
            &[RowInitializer {
                field: name_field.into(),
                input: ScalarEditInput::Text { value: name.into() },
            }],
        )
        .unwrap();
}

fn table(runtime: &DesignerRuntime, collection: &str) -> TableProjection {
    runtime.query_table(collection).unwrap()
}

fn row_ids(runtime: &DesignerRuntime, collection: &str) -> Vec<String> {
    table(runtime, collection)
        .rows
        .into_iter()
        .map(|row| row.id)
        .collect()
}

fn blank_fields(runtime: &DesignerRuntime, collection: &str, row: usize) -> Vec<String> {
    table(runtime, collection).rows[row].blank_fields.clone()
}

fn edit(runtime: &mut DesignerRuntime, entity: &str, field: &str, input: ScalarEditInput) {
    let expected_revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision,
            target: FieldTarget {
                entity: entity.into(),
                field: field.into(),
            },
            input,
        })
        .unwrap();
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
fn optional_date_and_boolean_blank_set_clear_and_undo_redo_restore_exact_blanks() {
    let (mut runtime, collection, ids) = fixture();
    let (name, due, done) = (&ids[0], &ids[1], &ids[2]);
    insert_named_row(&mut runtime, &collection, name, "alpha");
    let row = row_ids(&runtime, &collection)[0].clone();
    assert_eq!(
        blank_fields(&runtime, &collection, 0),
        [due.clone(), done.clone()]
    );

    edit(
        &mut runtime,
        &row,
        due,
        ScalarEditInput::Date {
            value: "2026-10-31".into(),
        },
    );
    edit(
        &mut runtime,
        &row,
        done,
        ScalarEditInput::Boolean { value: false },
    );
    // A stored `false` is present, not blank.
    assert_eq!(blank_fields(&runtime, &collection, 0), Vec::<String>::new());

    edit(&mut runtime, &row, due, ScalarEditInput::Clear);
    assert_eq!(blank_fields(&runtime, &collection, 0), vec![due.clone()]);
    edit(&mut runtime, &row, done, ScalarEditInput::Clear);
    assert_eq!(
        blank_fields(&runtime, &collection, 0),
        vec![due.clone(), done.clone()]
    );

    // Undo walks back through each edit; redo replays them exactly.
    history(&mut runtime, false);
    assert_eq!(blank_fields(&runtime, &collection, 0), vec![due.clone()]);
    history(&mut runtime, false);
    assert_eq!(blank_fields(&runtime, &collection, 0), Vec::<String>::new());
    history(&mut runtime, false);
    assert_eq!(blank_fields(&runtime, &collection, 0), vec![done.clone()]);
    history(&mut runtime, false);
    assert_eq!(
        blank_fields(&runtime, &collection, 0),
        vec![due.clone(), done.clone()]
    );
    history(&mut runtime, true);
    assert_eq!(blank_fields(&runtime, &collection, 0), vec![done.clone()]);
    history(&mut runtime, true);
    assert_eq!(blank_fields(&runtime, &collection, 0), Vec::<String>::new());
}

#[test]
fn optional_date_column_added_to_rows_is_blank_then_requiredness_follows_text_and_number() {
    let (mut runtime, collection, ids) = fixture();
    let name = ids[0].clone();
    insert_named_row(&mut runtime, &collection, &name, "alpha");
    insert_named_row(&mut runtime, &collection, &name, "beta");
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::AddColumn {
            expected_revision: revision,
            collection: collection.clone(),
            name: "Start".into(),
            field_type: "date".into(),
            required: false,
            initializers: Vec::new(),
        })
        .unwrap();
    let projection = table(&runtime, &collection);
    let start = projection.columns[3].id.clone();
    assert!(!projection.columns[3].required);
    // Rows also keep their blank Due and Done cells from the fixture.
    let blank = vec![ids[1].clone(), ids[2].clone(), start.clone()];
    assert_eq!(blank_fields(&runtime, &collection, 0), blank);
    assert_eq!(blank_fields(&runtime, &collection, 1), blank);

    let revision = runtime.current_revision().to_owned();
    let refusal = runtime
        .handle(DesignerRequest::SetColumnRequiredness {
            expected_revision: revision.clone(),
            collection: collection.clone(),
            field: start.clone(),
            required: true,
        })
        .unwrap_err();
    assert_eq!(
        refusal.failure_projection(&revision).code,
        "required_values_missing"
    );
    assert!(!table(&runtime, &collection).columns[3].required);

    let entities = row_ids(&runtime, &collection);
    edit(
        &mut runtime,
        &entities[0],
        &start,
        ScalarEditInput::Date {
            value: "2026-10-01".into(),
        },
    );
    let revision = runtime.current_revision().to_owned();
    let refusal = runtime
        .handle(DesignerRequest::SetColumnRequiredness {
            expected_revision: revision.clone(),
            collection: collection.clone(),
            field: start.clone(),
            required: true,
        })
        .unwrap_err();
    assert_eq!(
        refusal.failure_projection(&revision).code,
        "required_values_missing"
    );
    edit(
        &mut runtime,
        &entities[1],
        &start,
        ScalarEditInput::Date {
            value: "2026-10-02".into(),
        },
    );
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::SetColumnRequiredness {
            expected_revision: revision,
            collection: collection.clone(),
            field: start.clone(),
            required: true,
        })
        .unwrap();
    assert!(table(&runtime, &collection).columns[3].required);
}

#[test]
fn blank_boolean_is_not_zero_and_blank_date_is_probed_by_isblank() {
    let (mut runtime, collection, ids) = fixture();
    let (name, due, done) = (ids[0].clone(), ids[1].clone(), ids[2].clone());
    insert_named_row(&mut runtime, &collection, &name, "alpha");
    let row = row_ids(&runtime, &collection)[0].clone();
    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::AddColumn {
            expected_revision: revision,
            collection: collection.clone(),
            name: "Calc".into(),
            field_type: "number".into(),
            required: false,
            initializers: Vec::new(),
        })
        .unwrap();
    let calc = table(&runtime, &collection).columns[3].id.clone();
    let target = FieldTarget {
        entity: row.clone(),
        field: calc.clone(),
    };

    // A blank Boolean is not a Number: arithmetic on it is a type mismatch, never 0.
    let revision = runtime.current_revision().to_owned();
    let error = runtime
        .update_formula(&revision, &target, "[.done] + 1")
        .expect_err("a blank Boolean never reads as 0");
    assert_eq!(
        error.failure_projection(&revision).code,
        "formula_type_mismatch"
    );

    // ISBLANK probes presence on any type; a blank Date and a blank Boolean are blank.
    let revision = runtime.current_revision().to_owned();
    runtime
        .update_formula(&revision, &target, "IF(ISBLANK([.due]), 1, 0)")
        .unwrap();
    let field = table(&runtime, &collection).rows[0]
        .fields
        .iter()
        .find(|f| f.target.field == calc)
        .unwrap()
        .calculated
        .clone();
    assert_eq!(field, Some(CalculationProjection::Value { value: 1.0 }));
    assert_eq!(blank_fields(&runtime, &collection, 0), [due, done]);
}

fn blank_optional_workbook(columns: &[&str], rows: &[Vec<SourceValue>]) -> SourceWorkbook {
    let cell = |value: SourceValue| SourceCell {
        value,
        formula: None,
        style: CellStyle::default(),
    };
    SourceWorkbook {
        ledger: Vec::new(),
        sheets: vec![SourceSheet {
            name: "Tasks".into(),
            has_header: true,
            columns: columns
                .iter()
                .map(|name| SourceColumn {
                    name: (*name).into(),
                    width: None,
                    number_format: None,
                })
                .collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().cloned().map(cell).collect())
                .collect(),
        }],
    }
}

#[test]
fn imported_optional_date_and_boolean_columns_project_as_a_native_table() {
    let workbook = blank_optional_workbook(
        &["Name", "Due", "Done"],
        &[
            vec![
                SourceValue::Text {
                    value: "alpha".into(),
                },
                SourceValue::Date {
                    value: "2026-10-01".into(),
                },
                SourceValue::Boolean { value: true },
            ],
            vec![
                SourceValue::Text {
                    value: "beta".into(),
                },
                SourceValue::Empty,
                SourceValue::Empty,
            ],
        ],
    );
    let selection = ImportSelection {
        column_types: vec![vec![
            ImportFieldType::Text,
            ImportFieldType::Date,
            ImportFieldType::Boolean,
        ]],
        extra_columns: vec![Vec::new()],
    };
    let (mut runtime, projection) = import_workbook(&workbook, &selection, NEXT).unwrap();
    let collection = projection.opened.bootstrap.default_collection.clone();
    let imported = table(&runtime, &collection);
    assert_eq!(imported.native_table_profile, Some(true));
    assert!(!imported.columns[1].required);
    assert!(!imported.columns[2].required);
    assert_eq!(blank_fields(&runtime, &collection, 1).len(), 2);

    let revision = runtime.current_revision().to_owned();
    runtime
        .handle(DesignerRequest::InsertRow {
            expected_revision: revision,
            collection: collection.clone(),
            initializers: vec![RowInitializer {
                field: imported.columns[0].id.clone(),
                input: ScalarEditInput::Text {
                    value: "gamma".into(),
                },
            }],
        })
        .unwrap();
    assert_eq!(table(&runtime, &collection).rows.len(), 3);

    let revision = runtime.current_revision().to_owned();
    let response = runtime
        .handle(DesignerRequest::AddColumn {
            expected_revision: revision,
            collection: collection.clone(),
            name: "Note".into(),
            field_type: "text".into(),
            required: false,
            initializers: Vec::new(),
        })
        .unwrap();
    assert!(matches!(response, DesignerResponse::Published(_)));
    assert_eq!(table(&runtime, &collection).columns.len(), 4);
}

#[test]
fn native_export_writes_empty_cells_for_blank_date_and_boolean_without_refusal() {
    let (mut runtime, collection, ids) = fixture();
    let name = ids[0].clone();
    insert_named_row(&mut runtime, &collection, &name, "alpha");
    insert_named_row(&mut runtime, &collection, &name, "beta");
    let row = row_ids(&runtime, &collection)[0].clone();
    edit(
        &mut runtime,
        &row,
        &ids[1],
        ScalarEditInput::Date {
            value: "2026-10-31".into(),
        },
    );
    let revision = runtime.current_revision().to_owned();

    for workbook in [
        runtime
            .export_native_tables_workbook(&revision, &collection)
            .unwrap(),
        runtime
            .export_native_tables_csv_workbook(&revision, &collection)
            .unwrap(),
    ] {
        assert!(!workbook.ledger.iter().any(|finding| finding.blocking));
        let sheet = &workbook.sheets[0];
        let blank = sheet
            .rows
            .iter()
            .find(|row| {
                row[0].value
                    == SourceValue::Text {
                        value: "beta".into(),
                    }
            })
            .unwrap();
        assert_eq!(blank[1].value, SourceValue::Empty);
        assert_eq!(blank[2].value, SourceValue::Empty);
        let set = sheet
            .rows
            .iter()
            .find(|row| {
                row[0].value
                    == SourceValue::Text {
                        value: "alpha".into(),
                    }
            })
            .unwrap();
        assert_eq!(
            set[1].value,
            SourceValue::Date {
                value: "2026-10-31".into()
            }
        );
        assert_eq!(set[2].value, SourceValue::Empty);
    }
}
