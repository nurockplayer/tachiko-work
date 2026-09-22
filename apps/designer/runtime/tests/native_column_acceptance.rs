//! Steward-owned Issue #442 semantic acceptance. Before the private mutation
//! seam exists, an unknown request variant is a missing seam, not behavioral RED.
use serde_json::{Value, json};
use tachiko_designer_runtime::{
    DesignerResponse, DesignerRuntime, DesignerWireReply, TableProjection, process_wire_request,
};

fn request(runtime: &mut Option<DesignerRuntime>, input: &Value) -> DesignerWireReply {
    serde_json::from_slice(&process_wire_request(runtime, input.to_string().as_bytes()))
        .expect("private runtime reply is JSON")
}

fn publish(runtime: &mut Option<DesignerRuntime>, input: &Value) {
    let reply = request(runtime, input);
    assert!(
        matches!(
            reply,
            DesignerWireReply::Ok {
                response: DesignerResponse::Published(_)
            }
        ),
        "valid lifecycle candidate must publish: {reply:?}"
    );
}

fn table(runtime: &mut Option<DesignerRuntime>, collection: &str) -> TableProjection {
    let reply = request(
        runtime,
        &json!({"type":"query_table", "collection":collection}),
    );
    let DesignerWireReply::Ok {
        response: DesignerResponse::Table(table),
    } = reply
    else {
        panic!("fixture must remain queryable: {reply:?}");
    };
    table
}

fn fixture(column_count: usize, populated: bool) -> (Option<DesignerRuntime>, TableProjection) {
    let mut runtime = None;
    let columns: Vec<_> = (0..column_count)
        .map(|index| json!({"name":format!("text_{index}"), "field_type":"text"}))
        .collect();
    let reply = request(
        &mut runtime,
        &json!({
            "type":"new_table", "occurrence_id":"00000000-0000-4000-8000-000000000442",
            "name":"Inventory", "columns":columns,
        }),
    );
    let DesignerWireReply::Ok {
        response: DesignerResponse::Opened(opened),
    } = reply
    else {
        panic!("existing New Table fixture must open: {reply:?}");
    };
    if populated {
        let rows: Vec<Vec<_>> = (0..3)
            .map(|row| {
                (0..column_count)
                    .map(|column| format!("値 {row}/{column}"))
                    .collect()
            })
            .collect();
        publish(
            &mut runtime,
            &json!({
                "type":"paste_cells", "expected_revision":opened.table.revision,
                "collection":opened.table.collection.id, "start_entity":null,
                "start_field":opened.table.columns[0].id, "rows":rows,
            }),
        );
    }
    let current = table(&mut runtime, &opened.table.collection.id);
    assert_eq!(current.columns.len(), column_count);
    assert_eq!(current.rows.len(), if populated { 3 } else { 0 });
    (runtime, current)
}

fn add(before: &TableProjection, kind: &str, input: &Value) -> Value {
    json!({
        "type":"add_column", "expected_revision":before.revision,
        "collection":before.collection.id, "name":"added", "field_type":kind,
        "initializers":before.rows.iter().map(|row| json!({"entity":row.id,"input":input})).collect::<Vec<_>>(),
    })
}

fn content(table: &TableProjection) -> Value {
    json!({"collection":table.collection,"columns":table.columns,"rows":table.rows})
}

fn exported(runtime: Option<&DesignerRuntime>, revision: &str) -> Vec<u8> {
    runtime
        .expect("open fixture")
        .export_project(revision)
        .expect("current canonical export")
        .bytes
}

fn refuse_unchanged(
    runtime: &mut Option<DesignerRuntime>,
    before: &TableProjection,
    input: &Value,
) {
    let bytes = exported(runtime.as_ref(), &before.revision);
    let reply = request(runtime, input);
    assert!(
        matches!(reply, DesignerWireReply::Error { .. }),
        "must refuse: {reply:?}"
    );
    assert_eq!(table(runtime, &before.collection.id), *before);
    assert_eq!(exported(runtime.as_ref(), &before.revision), bytes);
}

#[test]
fn existing_inventory_fixture_reaches_the_real_mutation_boundary() {
    let (_, table) = fixture(2, true);
    assert!(table.rows.iter().all(|row| row.fields.len() == 2));
}

#[test]
fn each_direct_scalar_addition_preserves_originals_and_uses_forward_history() {
    for (kind, input, expected) in [
        (
            "text",
            json!({"kind":"text","value":""}),
            json!({"kind":"text","value":""}),
        ),
        (
            "number",
            json!({"kind":"number","input":"-2.5"}),
            json!({"kind":"number","value":-2.5}),
        ),
        (
            "boolean",
            json!({"kind":"boolean","value":true}),
            json!({"kind":"boolean","value":true}),
        ),
        (
            "date",
            json!({"kind":"date","value":"2024-02-29"}),
            json!({"kind":"date","value":"2024-02-29"}),
        ),
    ] {
        let (mut runtime, before) = fixture(2, true);
        publish(&mut runtime, &add(&before, kind, &input));
        let after = table(&mut runtime, &before.collection.id);
        assert_ne!(after.revision, before.revision);
        assert_eq!(after.columns.len(), before.columns.len() + 1);
        let added = after
            .columns
            .iter()
            .find(|field| !before.columns.iter().any(|old| old.id == field.id))
            .expect("one newly allocated field identity");
        assert_eq!(added.field_type, kind);
        assert_eq!(added.key, "added");
        for old_row in &before.rows {
            let row = after
                .rows
                .iter()
                .find(|row| row.id == old_row.id)
                .expect("same entity");
            for old_field in &old_row.fields {
                assert_eq!(
                    row.fields
                        .iter()
                        .find(|field| field.target == old_field.target),
                    Some(old_field)
                );
            }
            let new_value = row
                .fields
                .iter()
                .find(|field| field.target.field == added.id)
                .expect("explicit initializer");
            assert_eq!(serde_json::to_value(&new_value.stored).unwrap(), expected);
        }
        publish(
            &mut runtime,
            &json!({"type":"undo","expected_revision":after.revision}),
        );
        let undone = table(&mut runtime, &before.collection.id);
        assert_eq!(content(&undone), content(&before));
        assert_ne!(undone.revision, before.revision);
        assert_ne!(undone.revision, after.revision);
        publish(
            &mut runtime,
            &json!({"type":"redo","expected_revision":undone.revision}),
        );
        let redone = table(&mut runtime, &before.collection.id);
        assert_eq!(content(&redone), content(&after));
        assert_ne!(redone.revision, after.revision);
    }
}

#[test]
fn malformed_incomplete_stale_and_duplicate_additions_leave_no_history_entry() {
    let (mut runtime, before) = fixture(2, true);
    let valid = add(&before, "text", &json!({"kind":"text","value":"initial"}));
    let mut cases = Vec::new();
    let mut missing = valid.clone();
    missing["initializers"].as_array_mut().unwrap().pop();
    cases.push(missing);
    let mut duplicate = valid.clone();
    duplicate["initializers"][1] = duplicate["initializers"][0].clone();
    cases.push(duplicate);
    let mut extra = valid.clone();
    extra["initializers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"entity":"unknown-entity","input":{"kind":"text","value":"initial"}}));
    cases.push(extra);
    let mut stale = valid.clone();
    stale["expected_revision"] = json!("resident/0");
    assert_ne!(before.revision, "resident/0");
    cases.push(stale);
    for name in ["text_0", ""] {
        let mut invalid_name = valid.clone();
        invalid_name["name"] = json!(name);
        cases.push(invalid_name);
    }
    for input in [
        json!({"kind":"number","input":"3"}),
        json!({"kind":"formula","value":"1"}),
        json!({"kind":"reference","entity":"unknown"}),
    ] {
        cases.push(add(&before, "text", &input));
    }
    for (kind, input) in [
        ("number", json!({"kind":"number","input":"NaN"})),
        ("date", json!({"kind":"date","value":"2026-02-30"})),
        ("reference", json!({"kind":"text","value":"initial"})),
    ] {
        cases.push(add(&before, kind, &input));
    }
    for input in cases {
        refuse_unchanged(&mut runtime, &before, &input);
    }
    // The preceding real edit was paste. No rejected column attempt may cover
    // it with a fake history entry or consume its undo slot.
    publish(
        &mut runtime,
        &json!({"type":"undo","expected_revision":before.revision}),
    );
    let undone = table(&mut runtime, &before.collection.id);
    assert!(undone.rows.is_empty());
    assert_eq!(undone.columns, before.columns);
}

#[test]
fn rename_and_remove_preserve_stable_identity_and_forward_history() {
    let (mut runtime, before) = fixture(2, true);
    let field = &before.columns[1].id;
    for name in ["text_0", ""] {
        refuse_unchanged(
            &mut runtime,
            &before,
            &json!({
                "type":"rename_column", "expected_revision":before.revision,
                "collection":before.collection.id, "field":field, "name":name,
            }),
        );
    }
    publish(
        &mut runtime,
        &json!({
            "type":"rename_column", "expected_revision":before.revision,
            "collection":before.collection.id, "field":field, "name":"改名",
        }),
    );
    let renamed = table(&mut runtime, &before.collection.id);
    assert_eq!(renamed.rows, before.rows);
    assert_eq!(renamed.columns[1].id, *field);
    assert_eq!(renamed.columns[1].key, "改名");
    publish(
        &mut runtime,
        &json!({"type":"undo","expected_revision":renamed.revision}),
    );
    let undone = table(&mut runtime, &before.collection.id);
    assert_eq!(content(&undone), content(&before));
    assert_ne!(undone.revision, before.revision);
    publish(
        &mut runtime,
        &json!({"type":"redo","expected_revision":undone.revision}),
    );
    let restored = table(&mut runtime, &before.collection.id);
    assert_eq!(content(&restored), content(&renamed));
    assert_ne!(restored.revision, renamed.revision);

    refuse_unchanged(
        &mut runtime,
        &restored,
        &json!({
            "type":"remove_column", "expected_revision":before.revision,
            "collection":before.collection.id, "field":field,
        }),
    );
    publish(
        &mut runtime,
        &json!({
            "type":"remove_column", "expected_revision":restored.revision,
            "collection":restored.collection.id, "field":field,
        }),
    );
    let removed = table(&mut runtime, &before.collection.id);
    assert_eq!(removed.columns, before.columns[..1]);
    for row in &removed.rows {
        let original = before.rows.iter().find(|old| old.id == row.id).unwrap();
        assert_eq!(row.fields, original.fields[..1]);
    }
    publish(
        &mut runtime,
        &json!({"type":"undo","expected_revision":removed.revision}),
    );
    let recovered = table(&mut runtime, &before.collection.id);
    assert_eq!(content(&recovered), content(&renamed));
    assert_ne!(recovered.revision, restored.revision);
    publish(
        &mut runtime,
        &json!({"type":"redo","expected_revision":recovered.revision}),
    );
    let redone = table(&mut runtime, &before.collection.id);
    assert_eq!(content(&redone), content(&removed));
    assert_ne!(redone.revision, removed.revision);
}

#[test]
fn profile_limits_refuse_empty_table_and_capacity_overflow_atomically() {
    let (mut runtime, full) = fixture(32, false);
    refuse_unchanged(
        &mut runtime,
        &full,
        &add(&full, "text", &json!({"kind":"text","value":"explicit"})),
    );
    refuse_unchanged(
        &mut runtime,
        &full,
        &json!({"type":"undo","expected_revision":full.revision}),
    );
    let (mut runtime, one) = fixture(1, true);
    refuse_unchanged(
        &mut runtime,
        &one,
        &json!({
            "type":"remove_column", "expected_revision":one.revision,
            "collection":one.collection.id, "field":one.columns[0].id,
        }),
    );
}
