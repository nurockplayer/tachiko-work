use super::{
    CalculationProjection, CapacityLimit, ColumnRef, DesignerError, DesignerRequest,
    DesignerRuntime, Document, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldTarget, FieldType, NewTableColumnInput, Number, PasteCellInput,
    RowInitializer, ScalarEditInput, Schema, SchemaId, SchemaKey, StoredValueProjection, Value,
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
    let before = runtime.current_revision().to_owned();
    let publication = insert(&mut runtime, &collection, &ids, &[(0, "4"), (1, "5")]);
    assert_eq!(publication.base_revision, before);
    assert_eq!(publication.resulting_revision, next_revision(&before));
    let table = runtime.query_table(&collection).unwrap();
    let row = &table.rows[1];
    let field = row
        .fields
        .iter()
        .find(|f| f.target.field == ids[2])
        .unwrap();
    assert_eq!(
        field.formula.as_ref().unwrap().source,
        "([.hours] * [.rate])"
    );
    assert!(
        matches!(field.calculated, Some(CalculationProjection::Value { value }) if value == 20.0)
    );
    assert!(publication.fields.contains(&field.target));
    for redo in [false, true] {
        let expected_revision = runtime.current_revision().to_owned();
        let next = next_revision(&expected_revision);
        runtime
            .handle(if redo {
                DesignerRequest::Redo { expected_revision }
            } else {
                DesignerRequest::Undo { expected_revision }
            })
            .unwrap();
        assert_eq!(runtime.current_revision(), next);
        assert_eq!(
            runtime.query_table(&collection).unwrap().rows.len(),
            if redo { 2 } else { 1 }
        );
    }
    let table = runtime.query_table(&collection).unwrap();
    assert_eq!(
        table.rows[1]
            .fields
            .iter()
            .find(|f| f.target.field == ids[2])
            .unwrap()
            .formula
            .as_ref()
            .unwrap()
            .source,
        "([.hours] * [.rate])"
    );
}

fn next_revision(revision: &str) -> String {
    let n: u32 = revision.strip_prefix("resident/").unwrap().parse().unwrap();
    format!("resident/{}", n + 1)
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
    let table = runtime.query_table(&collection).unwrap();
    let typed = table.rows[2]
        .fields
        .iter()
        .find(|f| f.target.field == ids[2])
        .unwrap();
    assert_eq!(
        typed.stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert!(typed.formula.is_none());
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

fn document_fixture(
    formulas: Vec<Option<Expression>>,
    required: bool,
) -> (DesignerRuntime, String, Vec<String>) {
    let mut document = Document::empty("rows", "Rows");
    let schema = SchemaId::from("items");
    let fields = ["hours", "rate", "amount"]
        .into_iter()
        .map(|name| {
            let id = FieldId::from(name);
            (
                id.clone(),
                FieldDefinition {
                    id,
                    key: FieldKey::from(name),
                    display_name: None,
                    number_format: None,
                    field_type: FieldType::Number,
                    required: required && name == "amount",
                    constraint: tachiko_workspace_engine::FieldConstraint::None,
                },
            )
        })
        .collect();
    document.schemas.insert(
        schema.clone(),
        Schema {
            id: schema.clone(),
            key: SchemaKey::from("items"),
            display_name: None,
            field_order: Some(vec![
                FieldId::from("hours"),
                FieldId::from("rate"),
                FieldId::from("amount"),
            ]),
            fields,
        },
    );
    for (index, formula) in formulas.into_iter().enumerate() {
        let id = EntityId::from(format!("r{}", index + 1));
        let mut fields = [
            (
                FieldId::from("hours"),
                Value::Number(Number::new(2.0).unwrap()),
            ),
            (
                FieldId::from("rate"),
                Value::Number(Number::new(3.0).unwrap()),
            ),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
        if let Some(formula) = formula {
            fields.insert(FieldId::from("amount"), Value::Formula(formula));
        }
        document.entities.insert(
            id.clone(),
            Entity {
                key: EntityKey::from(id.to_string()),
                id,
                schema: schema.clone(),
                fields,
            },
        );
    }
    let runtime =
        DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000593").unwrap();
    (
        runtime,
        "items".into(),
        vec!["hours".into(), "rate".into(), "amount".into()],
    )
}

fn literal(value: f64) -> Expression {
    Expression::Number(Number::new(value).unwrap())
}
fn relative(name: &str) -> Expression {
    Expression::SameRowReference(FieldId::from(name))
}
fn absolute(row: &str, field: &str) -> Expression {
    Expression::Reference(FieldRef::new(EntityId::from(row), FieldId::from(field)))
}
fn amount_formula(runtime: &DesignerRuntime, collection: &str, row: usize) -> Option<String> {
    runtime.query_table(collection).unwrap().rows[row]
        .fields
        .iter()
        .find(|field| field.target.field == "amount")
        .and_then(|field| field.formula.as_ref())
        .map(|formula| formula.source.clone())
}

#[test]
fn differing_formula_and_blank_row_end_qualification() {
    let (mut runtime, collection, ids) =
        document_fixture(vec![Some(literal(1.0)), Some(literal(2.0))], false);
    let before = runtime.query_table(&collection).unwrap().rows;
    insert(&mut runtime, &collection, &ids, &[]);
    let after = runtime.query_table(&collection).unwrap().rows;
    assert_eq!(&after[1..], &before[..]);
    assert!(after[0].blank_fields.contains(&ids[2]));
    let (mut runtime, collection, ids) = document_fixture(vec![Some(literal(1.0)), None], false);
    insert(&mut runtime, &collection, &ids, &[]);
    assert!(
        runtime.query_table(&collection).unwrap().rows[0]
            .blank_fields
            .contains(&ids[2])
    );
}

#[test]
fn empty_and_absolute_reference_qualification() {
    let (mut runtime, collection, ids) = document_fixture(vec![], false);
    insert(&mut runtime, &collection, &ids, &[]);
    assert!(
        runtime.query_table(&collection).unwrap().rows[0]
            .blank_fields
            .contains(&ids[2])
    );
    let (mut runtime, collection, ids) = document_fixture(
        vec![Some(absolute("r1", "hours")), Some(absolute("r1", "hours"))],
        false,
    );
    insert(&mut runtime, &collection, &ids, &[]);
    assert!(
        runtime.query_table(&collection).unwrap().rows[0]
            .blank_fields
            .contains(&ids[2])
    );
    let (mut runtime, collection, ids) = document_fixture(
        vec![Some(absolute("r1", "hours")), Some(absolute("r2", "hours"))],
        false,
    );
    insert(&mut runtime, &collection, &ids, &[]);
    assert_eq!(
        amount_formula(&runtime, &collection, 0).as_deref(),
        Some("[.hours]")
    );
}

#[test]
fn totals_and_literals_qualify() {
    for formula in [
        literal(5.0),
        Expression::ColumnTotal(ColumnRef::new(
            SchemaId::from("items"),
            FieldId::from("hours"),
        )),
    ] {
        let (mut runtime, collection, ids) = document_fixture(vec![Some(formula)], false);
        insert(&mut runtime, &collection, &ids, &[]);
        assert!(amount_formula(&runtime, &collection, 0).is_some());
    }
}

#[test]
fn required_formula_fills_or_restores_default_when_calculation_fails() {
    let division = Expression::Divide {
        left: Box::new(relative("hours")),
        right: Box::new(relative("rate")),
    };
    let (mut runtime, collection, ids) = document_fixture(vec![Some(division)], true);
    insert(&mut runtime, &collection, &ids, &[(0, "4"), (1, "2")]);
    assert!(amount_formula(&runtime, &collection, 0).is_some());
    insert(&mut runtime, &collection, &ids, &[]);
    let table = runtime.query_table(&collection).unwrap();
    let field = table
        .rows
        .iter()
        .find(|row| row.key == "row_0002")
        .unwrap()
        .fields
        .iter()
        .find(|field| field.target.field == ids[2])
        .unwrap();
    assert_eq!(
        field.stored,
        Some(StoredValueProjection::Number { value: 0.0 })
    );
    assert!(field.formula.is_none());
}

#[test]
fn paste_append_fills_blank_cells_and_preserves_typed_values_in_one_undo() {
    let (mut runtime, collection, ids) = fixture("[.hours]*[.rate]");
    let revision = runtime.current_revision().to_owned();
    let rows = vec![
        vec![
            "2".into(),
            "3".into(),
            PasteCellInput::Typed(ScalarEditInput::Clear),
        ],
        vec!["4".into(), "5".into(), "9".into()],
        vec![
            "6".into(),
            "7".into(),
            PasteCellInput::Typed(ScalarEditInput::Clear),
        ],
    ];
    let publication = runtime
        .paste_cell_inputs(&revision, &collection, None, &ids[0], &rows)
        .unwrap();
    assert_eq!(publication.resulting_revision, next_revision(&revision));
    let table = runtime.query_table(&collection).unwrap();
    assert!(
        table.rows[1]
            .fields
            .iter()
            .find(|f| f.target.field == ids[2])
            .unwrap()
            .formula
            .is_some()
    );
    let typed = table.rows[2]
        .fields
        .iter()
        .find(|f| f.target.field == ids[2])
        .unwrap();
    assert_eq!(
        typed.stored,
        Some(StoredValueProjection::Number { value: 9.0 })
    );
    assert!(typed.formula.is_none());
    assert!(
        table.rows[3]
            .fields
            .iter()
            .find(|f| f.target.field == ids[2])
            .unwrap()
            .formula
            .is_some()
    );
    runtime
        .handle(DesignerRequest::Undo {
            expected_revision: publication.resulting_revision,
        })
        .unwrap();
    assert_eq!(runtime.query_table(&collection).unwrap().rows.len(), 1);
}

#[test]
fn paste_overwrite_existing_formula_disables_append_fill() {
    let (mut runtime, collection, ids) = fixture("[.hours]*[.rate]");
    let first = runtime.query_table(&collection).unwrap().rows[0].id.clone();
    let revision = runtime.current_revision().to_owned();
    let rows = vec![
        vec![PasteCellInput::from("7")],
        vec![PasteCellInput::Typed(ScalarEditInput::Clear)],
        vec![PasteCellInput::Typed(ScalarEditInput::Clear)],
    ];
    runtime
        .paste_cell_inputs(&revision, &collection, Some(&first), &ids[2], &rows)
        .unwrap();
    let table = runtime.query_table(&collection).unwrap();
    assert!(table.rows[1].blank_fields.contains(&ids[2]));
    assert!(table.rows[2].blank_fields.contains(&ids[2]));
}

#[test]
fn formula_capacity_refuses_insert_without_publication() {
    let (seed, collection, ids) =
        document_fixture((0..86).map(|_| Some(literal(1.0))).collect(), false);
    let mut document = seed.session.export_snapshot().document().clone();
    for (index, row) in document.entities.values_mut().enumerate() {
        row.fields
            .insert(FieldId::from("hours"), Value::Formula(literal(1.0)));
        if index < 84 {
            row.fields
                .insert(FieldId::from("rate"), Value::Formula(literal(1.0)));
        }
    }
    let mut runtime =
        DesignerRuntime::from_document(document, "00000000-0000-4000-8000-000000000593").unwrap();
    let revision = runtime.current_revision().to_owned();
    let error = runtime.insert_row(&revision, &collection, &[]).unwrap_err();
    assert!(
        matches!(error, DesignerError::CapacityExceeded(error) if error.limit == CapacityLimit::Formulas)
    );
    assert_eq!(runtime.current_revision(), revision);
    assert_eq!(runtime.query_table(&collection).unwrap().rows.len(), 86);
    assert_eq!(ids[2], "amount");
}
