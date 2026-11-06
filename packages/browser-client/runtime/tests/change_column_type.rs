//! #660 acceptance: one native column changes type as one atomic, undoable
//! operation. Every value converts with the interop rules or the whole command
//! refuses without a partial change.
#![cfg(not(target_arch = "wasm32"))]

use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, NewTableColumnInput,
    PublicationProjection, RowInitializer, RowProjection, ScalarEditInput, StoredValueProjection,
    TableProjection,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000660";

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

fn column_id(table: &TableProjection, key: &str) -> String {
    table
        .columns
        .iter()
        .find(|column| column.key == key)
        .expect("fixture column exists")
        .id
        .clone()
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

#[test]
fn text_column_of_decimal_texts_converts_to_number_in_place() {
    let mut runtime = open(&[
        column("label", "text", true),
        column("amount", "text", false),
        column("note", "text", false),
    ]);
    let opened = table(&mut runtime);
    let amount = column_id(&opened, "amount");
    let key = opened.columns[1].key.clone();
    insert(
        &mut runtime,
        vec![
            text_initializer("label", "first"),
            text_initializer("amount", "1"),
            text_initializer("note", "kept"),
        ],
    );
    insert(
        &mut runtime,
        vec![
            text_initializer("label", "second"),
            text_initializer("amount", "2.5"),
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
    assert_eq!(text_of(&after.rows[0], &column_id(&after, "note")), "kept");
}
