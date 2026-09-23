//! Steward-owned Issue #443 acceptance. Unknown private commands are explicitly
//! missing seams until implemented, not behavioral RED evidence.
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

fn fixture(count: usize) -> (Option<DesignerRuntime>, TableProjection) {
    fixture_width(count, 4)
}

fn fixture_width(count: usize, width: usize) -> (Option<DesignerRuntime>, TableProjection) {
    let columns = [
        json!({"name":"item","field_type":"text"}),
        json!({"name":"quantity","field_type":"number"}),
        json!({"name":"active","field_type":"boolean"}),
        json!({"name":"received","field_type":"date"}),
    ];
    let mut runtime = None;
    let reply = request(
        &mut runtime,
        &json!({
            "type":"new_table", "occurrence_id":"00000000-0000-4000-8000-000000000443",
            "name":"Inventory", "columns":&columns[..width],
        }),
    );
    let DesignerWireReply::Ok {
        response: DesignerResponse::Opened(opened),
    } = reply
    else {
        panic!("existing New Table fixture must open: {reply:?}");
    };
    if count > 0 {
        let rows: Vec<_> = (0..count)
            .map(|i| {
                vec![
                    format!("品物 {i}"),
                    format!("{i}"),
                    "true".into(),
                    "2024-02-29".into(),
                ]
                .into_iter()
                .take(width)
                .collect::<Vec<_>>()
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
    assert_eq!(current.rows.len(), count);
    assert_eq!(current.columns.len(), width);
    (runtime, current)
}

fn insert(before: &TableProjection) -> Value {
    json!({
        "type":"insert_row", "expected_revision":before.revision,
        "collection":before.collection.id,
        "initializers":before.columns.iter().map(|field| {
            let input = match field.field_type.as_str() {
                "text" => json!({"kind":"text","value":""}),
                "number" => json!({"kind":"number","input":"-2.5"}),
                "boolean" => json!({"kind":"boolean","value":false}),
                "date" => json!({"kind":"date","value":"2024-02-29"}),
                _ => panic!("fixture contains only native direct scalars"),
            };
            json!({"field":field.id,"input":input})
        }).collect::<Vec<_>>(),
    })
}

fn remove(before: &TableProjection, entities: &[&str]) -> Value {
    json!({"type":"remove_table_rows", "expected_revision":before.revision,
        "collection":before.collection.id, "entities":entities})
}

#[test]
fn existing_typed_inventory_fixture_reaches_the_mutation_boundary() {
    let (_, before) = fixture(3);
    assert!(before.rows.iter().all(|row| row.fields.len() == 4));
}

#[test]
fn insert_has_explicit_typed_values_and_preserves_identity_through_forward_history() {
    for count in [0, 3] {
        let (mut runtime, before) = fixture(count);
        publish(&mut runtime, &insert(&before));
        let after = table(&mut runtime, &before.collection.id);
        assert_eq!(after.columns, before.columns);
        assert_eq!(after.rows.len(), count + 1);
        assert_ne!(after.revision, before.revision);
        for row in &before.rows {
            assert_eq!(
                after.rows.iter().find(|added| added.id == row.id),
                Some(row)
            );
        }
        let added = after
            .rows
            .iter()
            .find(|row| !before.rows.iter().any(|old| old.id == row.id))
            .unwrap();
        assert_eq!(added.fields.len(), 4);
        for column in &before.columns {
            let field = added
                .fields
                .iter()
                .find(|value| value.target.field == column.id)
                .unwrap();
            let expected = match column.field_type.as_str() {
                "text" => json!({"kind":"text","value":""}),
                "number" => json!({"kind":"number","value":-2.5}),
                "boolean" => json!({"kind":"boolean","value":false}),
                "date" => json!({"kind":"date","value":"2024-02-29"}),
                _ => unreachable!(),
            };
            assert_eq!(serde_json::to_value(&field.stored).unwrap(), expected);
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
fn batch_removal_preserves_survivors_and_undo_restores_original_identities() {
    let (mut runtime, before) = fixture(3);
    let targets = [&*before.rows[0].id, &*before.rows[2].id];
    publish(&mut runtime, &remove(&before, &targets));
    let after = table(&mut runtime, &before.collection.id);
    assert_eq!(after.rows, vec![before.rows[1].clone()]);
    assert_eq!(after.columns, before.columns);
    assert_ne!(after.revision, before.revision);
    publish(
        &mut runtime,
        &json!({"type":"undo","expected_revision":after.revision}),
    );
    let undone = table(&mut runtime, &before.collection.id);
    assert_eq!(content(&undone), content(&before));
    assert_ne!(undone.revision, before.revision);
    publish(
        &mut runtime,
        &json!({"type":"redo","expected_revision":undone.revision}),
    );
    let redone = table(&mut runtime, &before.collection.id);
    assert_eq!(content(&redone), content(&after));
    publish(&mut runtime, &remove(&redone, &[&redone.rows[0].id]));
    let empty = table(&mut runtime, &before.collection.id);
    assert!(empty.rows.is_empty());
    assert_eq!(empty.columns, before.columns);
}

#[test]
fn invalid_incomplete_duplicate_stale_and_oversized_requests_preserve_state_and_history() {
    let (mut runtime, before) = fixture(3);
    let valid = insert(&before);
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
        .push(json!({"field":"unknown","input":{"kind":"text","value":""}}));
    cases.push(extra);
    for column in &before.columns {
        let index = before
            .columns
            .iter()
            .position(|field| field.id == column.id)
            .unwrap();
        let bad = match column.field_type.as_str() {
            "number" => json!({"kind":"number","input":"NaN"}),
            "date" => json!({"kind":"date","value":"2026-02-30"}),
            _ => json!({"kind":"formula","value":"1"}),
        };
        let mut invalid = valid.clone();
        invalid["initializers"][index]["input"] = bad;
        cases.push(invalid);
    }
    let mut stale = valid.clone();
    stale["expected_revision"] = json!("resident/0");
    assert_ne!(before.revision, "resident/0");
    cases.push(stale);
    let mut wrong_collection = valid;
    wrong_collection["collection"] = json!("unknown");
    cases.push(wrong_collection);
    cases.extend([
        remove(&before, &[]),
        remove(&before, &[&before.rows[0].id, &before.rows[0].id]),
        remove(&before, &[&before.rows[0].id, "unknown"]),
        remove(&before, &vec![&*before.rows[0].id; 129]),
    ]);
    let mut stale_remove = remove(&before, &[&before.rows[0].id]);
    stale_remove["expected_revision"] = json!("resident/0");
    cases.push(stale_remove);
    for invalid in cases {
        refuse_unchanged(&mut runtime, &before, &invalid);
    }
    // Paste is still the only successful mutation; rejected attempts neither
    // create undo entries nor hide it behind a manufactured history entry.
    publish(
        &mut runtime,
        &json!({"type":"undo","expected_revision":before.revision}),
    );
    let undone = table(&mut runtime, &before.collection.id);
    assert!(undone.rows.is_empty());
    assert_eq!(undone.columns, before.columns);
}

#[test]
fn row_capacity_refuses_atomically_then_allows_insert_after_removal() {
    // A one-column fixture reaches the row limit without exceeding the
    // independent 64 KiB publication-projection budget.
    let (mut runtime, full) = fixture_width(128, 1);
    refuse_unchanged(&mut runtime, &full, &insert(&full));
    publish(&mut runtime, &remove(&full, &[&full.rows[0].id]));
    let vacancy = table(&mut runtime, &full.collection.id);
    assert_eq!(vacancy.rows.len(), 127);
    publish(&mut runtime, &insert(&vacancy));
    let after = table(&mut runtime, &full.collection.id);
    assert_eq!(after.rows.len(), 128);
    assert!(!after.rows.iter().any(|row| row.id == full.rows[0].id));
}
