//! #660 acceptance: one native column changes type as one atomic, undoable
//! operation. Every value converts with the interop rules or the whole command
//! refuses without a partial change.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, FieldConstraintInput,
    FieldConstraintProjection, FieldTarget, NewTableColumnInput, PublicationProjection,
    RowInitializer, RowProjection, ScalarEditInput, StoredValueProjection, TableProjection,
    open_project,
};
use tachiko_workspace_engine::{Date, NumberFormat, NumberFormatKind};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000660";
const FRESH_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000661";

fn column(name: &str, field_type: &str, required: bool) -> NewTableColumnInput {
    NewTableColumnInput {
        name: name.to_owned(),
        field_type: field_type.to_owned(),
        required,
    }
}

fn open(columns: &[NewTableColumnInput]) -> DesignerRuntime {
    DesignerRuntime::new_table(OCCURRENCE, "items", columns).expect("fixture opens")
}

fn table(runtime: &mut DesignerRuntime) -> TableProjection {
    match runtime
        .handle(DesignerRequest::QueryTable {
            collection: "items".to_owned(),
        })
        .expect("fixture stays queryable")
    {
        DesignerResponse::Table(table) => table,
        other => panic!("expected table: {other:?}"),
    }
}

fn publish(runtime: &mut DesignerRuntime, request: DesignerRequest) -> PublicationProjection {
    match runtime.handle(request).expect("fixture publishes") {
        DesignerResponse::Published(publication) => publication,
        other => panic!("expected publication: {other:?}"),
    }
}

fn text_initializer(field: &str, value: &str) -> RowInitializer {
    RowInitializer {
        field: field.to_owned(),
        input: ScalarEditInput::Text {
            value: value.to_owned(),
        },
    }
}

fn number_initializer(field: &str, input: &str) -> RowInitializer {
    RowInitializer {
        field: field.to_owned(),
        input: ScalarEditInput::Number {
            input: input.to_owned(),
        },
    }
}

fn insert(runtime: &mut DesignerRuntime, initializers: Vec<RowInitializer>) {
    let revision = table(runtime).revision;
    publish(
        runtime,
        DesignerRequest::InsertRow {
            expected_revision: revision,
            collection: "items".to_owned(),
            initializers,
        },
    );
}

fn undo(runtime: &mut DesignerRuntime) -> PublicationProjection {
    let revision = table(runtime).revision;
    publish(
        runtime,
        DesignerRequest::Undo {
            expected_revision: revision,
        },
    )
}

fn redo(runtime: &mut DesignerRuntime) -> PublicationProjection {
    let revision = table(runtime).revision;
    publish(
        runtime,
        DesignerRequest::Redo {
            expected_revision: revision,
        },
    )
}

fn change(
    runtime: &mut DesignerRuntime,
    field: &str,
    field_type: &str,
) -> Result<PublicationProjection, DesignerError> {
    let revision = table(runtime).revision;
    match runtime.handle(DesignerRequest::ChangeColumnType {
        expected_revision: revision,
        collection: "items".to_owned(),
        field: field.to_owned(),
        field_type: field_type.to_owned(),
    })? {
        DesignerResponse::Published(publication) => Ok(publication),
        other => panic!("expected publication: {other:?}"),
    }
}

fn ids(table: &TableProjection) -> BTreeMap<String, String> {
    table
        .columns
        .iter()
        .map(|column| (column.key.clone(), column.id.clone()))
        .collect()
}

fn column_id(table: &TableProjection, key: &str) -> String {
    ids(table)
        .remove(key)
        .unwrap_or_else(|| panic!("fixture column '{key}' exists"))
}

fn stored(row: &RowProjection, field: &str) -> Option<StoredValueProjection> {
    row.fields
        .iter()
        .find(|projection| projection.target.field == field)
        .and_then(|projection| projection.stored.clone())
}

fn text_of(row: &RowProjection, field: &str) -> String {
    match stored(row, field) {
        Some(StoredValueProjection::Text { value }) => value,
        other => panic!("expected stored Text: {other:?}"),
    }
}

fn number_of(row: &RowProjection, field: &str) -> f64 {
    match stored(row, field) {
        Some(StoredValueProjection::Number { value }) => value,
        other => panic!("expected stored Number: {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

fn revision_serial(revision: &str) -> u64 {
    revision
        .rsplit('/')
        .next()
        .expect("revision has a serial")
        .parse()
        .expect("revision serial is a number")
}

fn assert_refusal(error: &DesignerError, expected: &str) {
    match error {
        DesignerError::InvalidTableOperation { message } => assert_eq!(message, expected),
        other => panic!("expected table refusal: {other:?}"),
    }
}

#[test]
fn text_column_of_decimal_texts_converts_to_number_in_place() {
    let mut runtime = open(&[
        column("label", "text", true),
        column("amount", "text", false),
        column("note", "text", false),
    ]);
    let opened = table(&mut runtime);
    let label = column_id(&opened, "label");
    let amount = column_id(&opened, "amount");
    let note = column_id(&opened, "note");
    let key = opened.columns[1].key.clone();
    insert(
        &mut runtime,
        vec![
            text_initializer(&label, "first"),
            text_initializer(&amount, "1"),
            text_initializer(&note, "kept"),
        ],
    );
    insert(
        &mut runtime,
        vec![
            text_initializer(&label, "second"),
            text_initializer(&amount, "2.5"),
        ],
    );
    let before = table(&mut runtime);
    assert_eq!(before.columns[1].id, amount);
    assert_eq!(before.columns[1].field_type, "text");

    let publication = change(&mut runtime, &amount, "number").expect("values convert");

    assert_eq!(publication.base_revision, before.revision);
    assert_eq!(
        revision_serial(&publication.resulting_revision),
        revision_serial(&before.revision) + 1,
        "one publication advances the revision once"
    );
    let after = table(&mut runtime);
    assert_eq!(after.revision, publication.resulting_revision);
    assert_eq!(after.columns[1].id, amount, "stable field id is unchanged");
    assert_eq!(after.columns[1].key, key, "human key is unchanged");
    assert_eq!(after.columns[1].field_type, "number");
    assert_close(number_of(&after.rows[0], &amount), 1.0);
    assert_close(number_of(&after.rows[1], &amount), 2.5);
    assert_eq!(text_of(&after.rows[0], &note), "kept");
}

#[test]
fn undo_restores_values_position_and_constraint_and_redo_reapplies() {
    let mut runtime = open(&[
        column("label", "text", true),
        column("amount", "text", false),
        column("note", "text", false),
    ]);
    let opened = table(&mut runtime);
    let label = column_id(&opened, "label");
    let amount = column_id(&opened, "amount");
    let note = column_id(&opened, "note");
    insert(
        &mut runtime,
        vec![
            text_initializer(&label, "a"),
            text_initializer(&amount, "1"),
            text_initializer(&note, "x"),
        ],
    );
    insert(
        &mut runtime,
        vec![
            text_initializer(&label, "b"),
            text_initializer(&amount, "2.5"),
        ],
    );
    let revision = table(&mut runtime).revision;
    publish(
        &mut runtime,
        DesignerRequest::SetColumnConstraint {
            expected_revision: revision,
            collection: "items".to_owned(),
            field: amount.clone(),
            constraint: FieldConstraintInput::TextLiteralSet {
                values: vec!["1".to_owned(), "2.5".to_owned()],
            },
        },
    );
    let before = table(&mut runtime);
    assert_eq!(before.columns[1].id, amount);
    assert!(before.columns[1].number_format.is_none());
    assert_eq!(
        before.columns[1].constraint,
        Some(FieldConstraintProjection::TextLiteralSet {
            values: vec!["1".to_owned(), "2.5".to_owned()],
        })
    );

    change(&mut runtime, &amount, "number").expect("values convert");
    let after = table(&mut runtime);
    assert_eq!(after.columns[1].id, amount, "position is unchanged");
    assert!(after.columns[1].constraint.is_none());
    assert!(after.columns[1].number_format.is_none());
    assert_close(number_of(&after.rows[1], &amount), 2.5);

    let undone = undo(&mut runtime);
    assert_eq!(
        revision_serial(&undone.resulting_revision),
        revision_serial(&after.revision) + 1
    );
    let restored = table(&mut runtime);
    assert_eq!(restored.columns, before.columns);
    assert_eq!(restored.rows, before.rows);

    redo(&mut runtime);
    let reapplied = table(&mut runtime);
    assert_eq!(reapplied.columns, after.columns);
    assert_eq!(reapplied.rows, after.rows);
}

#[test]
fn number_column_converts_to_text_and_drops_then_restores_the_number_format() {
    let mut runtime = open(&[
        column("amount", "number", true),
        column("note", "text", false),
    ]);
    let opened = table(&mut runtime);
    let amount = column_id(&opened, "amount");
    let note = column_id(&opened, "note");
    insert(
        &mut runtime,
        vec![
            number_initializer(&amount, "3"),
            text_initializer(&note, "n"),
        ],
    );
    let revision = table(&mut runtime).revision;
    let format = NumberFormat {
        decimals: 2,
        kind: NumberFormatKind::Plain,
        thousands_separator: true,
    };
    publish(
        &mut runtime,
        DesignerRequest::SetColumnNumberFormat {
            expected_revision: revision,
            collection: "items".to_owned(),
            field: amount.clone(),
            number_format: Some(format.clone()),
        },
    );
    let before = table(&mut runtime);
    assert_eq!(before.columns[0].number_format, Some(format.clone()));

    change(&mut runtime, &amount, "text").expect("Number converts to Text");
    let after = table(&mut runtime);
    assert_eq!(after.columns[0].field_type, "text");
    assert!(after.columns[0].number_format.is_none(), "format dropped");
    assert_eq!(text_of(&after.rows[0], &amount), "3");

    undo(&mut runtime);
    let restored = table(&mut runtime);
    assert_eq!(restored.columns, before.columns);
    assert_eq!(restored.rows, before.rows);
}

#[test]
fn text_columns_convert_to_date_and_boolean() {
    let mut runtime = open(&[column("when", "text", false), column("flag", "text", false)]);
    let opened = table(&mut runtime);
    let when = column_id(&opened, "when");
    let flag = column_id(&opened, "flag");
    insert(
        &mut runtime,
        vec![
            text_initializer(&when, "2024-01-02"),
            text_initializer(&flag, "true"),
        ],
    );

    change(&mut runtime, &when, "date").expect("Text converts to Date");
    change(&mut runtime, &flag, "boolean").expect("Text converts to Boolean");

    let after = table(&mut runtime);
    assert_eq!(after.columns[0].field_type, "date");
    assert_eq!(
        stored(&after.rows[0], &when),
        Some(StoredValueProjection::Date {
            value: Date::parse("2024-01-02").expect("fixture date"),
        })
    );
    assert_eq!(
        stored(&after.rows[0], &flag),
        Some(StoredValueProjection::Boolean { value: true })
    );
}

#[test]
fn a_bad_value_refuses_the_whole_command_naming_the_row_and_value() {
    let mut runtime = open(&[
        column("label", "text", true),
        column("amount", "text", false),
    ]);
    let opened = table(&mut runtime);
    let label = column_id(&opened, "label");
    let amount = column_id(&opened, "amount");
    for (index, value) in ["1", "2", "abc"].into_iter().enumerate() {
        insert(
            &mut runtime,
            vec![
                text_initializer(&label, &format!("row {index}")),
                text_initializer(&amount, value),
            ],
        );
    }
    let before = table(&mut runtime);
    let error = change(&mut runtime, &amount, "number").expect_err("the third row does not parse");
    assert_refusal(
        &error,
        "row 3 of column 'amount' holds 'abc': conversion requires an unambiguous finite decimal Number",
    );
    assert_eq!(table(&mut runtime), before, "no cell changed");
    assert_eq!(
        text_of(&before.rows[2], &amount),
        "abc",
        "the source value is unchanged"
    );
}

#[test]
fn blanks_stay_blank_and_empty_text_refuses_in_a_required_column() {
    let mut runtime = open(&[
        column("amount", "text", false),
        column("required_text", "text", true),
    ]);
    let opened = table(&mut runtime);
    let amount = column_id(&opened, "amount");
    let required_text = column_id(&opened, "required_text");
    insert(&mut runtime, vec![text_initializer(&required_text, "1")]);
    insert(
        &mut runtime,
        vec![
            text_initializer(&amount, ""),
            text_initializer(&required_text, ""),
        ],
    );

    change(&mut runtime, &amount, "number").expect("an optional empty Text becomes blank");
    let after = table(&mut runtime);
    for row in &after.rows {
        assert!(
            row.blank_fields.iter().any(|field| field == &amount),
            "an empty Text cell becomes blank"
        );
        assert!(stored(row, &amount).is_none());
    }
    assert_eq!(after.rows[0].blank_fields.len(), 1, "absent stays absent");

    let before = table(&mut runtime);
    let error = change(&mut runtime, &required_text, "number")
        .expect_err("a required empty Text does not parse");
    assert_refusal(
        &error,
        "row 2 of column 'required_text' holds '': conversion requires an unambiguous finite decimal Number",
    );
    assert_eq!(table(&mut runtime), before);
}

#[test]
fn the_required_flag_survives_a_type_change() {
    let mut runtime = open(&[column("count", "text", true)]);
    let opened = table(&mut runtime);
    let count = column_id(&opened, "count");
    insert(&mut runtime, vec![text_initializer(&count, "1")]);
    insert(&mut runtime, vec![text_initializer(&count, "2")]);

    change(&mut runtime, &count, "number").expect("values convert");
    let after = table(&mut runtime);
    assert_eq!(after.columns[0].field_type, "number");
    assert!(after.columns[0].required);
    assert_close(number_of(&after.rows[0], &count), 1.0);
    assert_close(number_of(&after.rows[1], &count), 2.0);
}

#[test]
fn formula_cells_and_formula_dependents_refuse_without_a_change() {
    let mut runtime = open(&[
        column("amount", "number", true),
        column("other", "number", true),
    ]);
    let opened = table(&mut runtime);
    let amount = column_id(&opened, "amount");
    let other = column_id(&opened, "other");
    insert(
        &mut runtime,
        vec![
            number_initializer(&amount, "1"),
            number_initializer(&other, "2"),
        ],
    );
    let row = table(&mut runtime).rows[0].id.clone();
    let revision = table(&mut runtime).revision;
    publish(
        &mut runtime,
        DesignerRequest::FormulaUpdate {
            expected_revision: revision,
            target: FieldTarget {
                entity: row.clone(),
                field: amount.clone(),
            },
            source: format!("[{row}.{other}]"),
        },
    );
    let before = table(&mut runtime);
    let error =
        change(&mut runtime, &amount, "text").expect_err("a calculated column keeps its type");
    match error {
        DesignerError::InvalidTableOperation { message } => {
            assert!(message.contains("amount"), "{message}");
            assert!(message.contains("formula"), "{message}");
        }
        other => panic!("expected table refusal: {other:?}"),
    }
    assert_eq!(table(&mut runtime), before);

    let mut reader = open(&[
        column("source", "number", true),
        column("dependent", "number", true),
    ]);
    let opened = table(&mut reader);
    let source = column_id(&opened, "source");
    let dependent = column_id(&opened, "dependent");
    insert(
        &mut reader,
        vec![
            number_initializer(&source, "4"),
            number_initializer(&dependent, "0"),
        ],
    );
    let row = table(&mut reader).rows[0].id.clone();
    let revision = table(&mut reader).revision;
    publish(
        &mut reader,
        DesignerRequest::FormulaUpdate {
            expected_revision: revision,
            target: FieldTarget {
                entity: row.clone(),
                field: dependent.clone(),
            },
            source: format!("[{row}.{source}]"),
        },
    );
    let before = table(&mut reader);
    let error =
        change(&mut reader, &source, "text").expect_err("a read column does not change type");
    match error {
        DesignerError::FormulaDependencyInUse {
            dependent_cells,
            subject,
        } => {
            assert_eq!(dependent_cells, 1);
            assert!(subject.contains("source"), "{subject}");
        }
        other => panic!("expected dependency refusal: {other:?}"),
    }
    assert_eq!(table(&mut reader), before);
}

#[test]
fn same_type_unknown_type_and_stale_revision_refuse() {
    let mut runtime = open(&[column("amount", "text", false)]);
    let opened = table(&mut runtime);
    let amount = column_id(&opened, "amount");
    let before = table(&mut runtime);

    let error = change(&mut runtime, &amount, "text").expect_err("same type refuses");
    assert_refusal(&error, "column already has that type");
    assert_eq!(table(&mut runtime), before);

    let error = change(&mut runtime, &amount, "money").expect_err("unknown type refuses");
    assert_refusal(&error, "column type must be Text, Number, Boolean, or Date");
    assert_eq!(table(&mut runtime), before);

    let error = runtime
        .handle(DesignerRequest::ChangeColumnType {
            expected_revision: "resident/99".to_owned(),
            collection: "items".to_owned(),
            field: amount,
            field_type: "number".to_owned(),
        })
        .expect_err("a stale revision refuses");
    match error {
        DesignerError::StaleQuery { requested, current } => {
            assert_eq!(requested, "resident/99");
            assert_eq!(current, before.revision);
        }
        other => panic!("expected stale refusal: {other:?}"),
    }
    assert_eq!(table(&mut runtime), before);
}

#[test]
fn the_changed_type_round_trips_through_roproj_v3() {
    let mut runtime = open(&[
        column("label", "text", true),
        column("amount", "text", false),
    ]);
    let opened = table(&mut runtime);
    let label = column_id(&opened, "label");
    let amount = column_id(&opened, "amount");
    insert(
        &mut runtime,
        vec![
            text_initializer(&label, "first"),
            text_initializer(&amount, "1.5"),
        ],
    );
    change(&mut runtime, &amount, "number").expect("values convert");
    let before = table(&mut runtime);

    let export = runtime
        .export_project_v3(&before.revision)
        .expect("v3 export");
    let mut reopened = None;
    open_project(&mut reopened, &export.bytes, FRESH_OCCURRENCE).expect("v3 open");
    let mut reopened = reopened.expect("opened occurrence");

    let after = table(&mut reopened);
    assert_eq!(after.columns, before.columns, "the new type is saved");
    assert_eq!(after.columns[1].field_type, "number");
    assert_eq!(after.columns[1].id, amount);
    assert_eq!(after.columns[1].key, "amount");
    assert_close(number_of(&after.rows[0], &amount), 1.5);
}
