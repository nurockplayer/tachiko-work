//! Issue #542: the public JSON batch shape reaches the existing atomic edit path.
use serde_json::{Value, json};
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, DesignerWireReply, ImportFieldType,
    ImportSelection, PublicationProjection, StoredValueProjection, TableProjection,
    import_workbook,
    interop_adapter::{ImportOptions, import_csv},
    process_wire_request,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000542";

fn request(runtime: &mut Option<DesignerRuntime>, input: &Value) -> DesignerWireReply {
    serde_json::from_slice(&process_wire_request(
        runtime,
        &serde_json::to_vec(input).unwrap(),
    ))
    .expect("wire reply")
}

fn publish(runtime: &mut Option<DesignerRuntime>, input: &Value) -> PublicationProjection {
    let reply = request(runtime, input);
    let DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = reply
    else {
        panic!("batch/history must publish: {reply:?}");
    };
    publication
}

fn table(runtime: &mut Option<DesignerRuntime>, collection: &str) -> TableProjection {
    let DesignerWireReply::Ok {
        response: DesignerResponse::Table(table),
    } = request(
        runtime,
        &json!({"type":"query_table", "collection":collection}),
    )
    else {
        panic!("fixture must remain queryable");
    };
    table
}

fn fixture(imported: bool) -> (Option<DesignerRuntime>, TableProjection) {
    if imported {
        // 65 rows exceed ordinary CSV admission (64), forcing the Text-capacity
        // path. Use the real CSV importer rather than a native document stand-in.
        let csv = format!("left,right\n{}", "old-left,10\n".repeat(65));
        let source = import_csv(csv.as_bytes(), &ImportOptions::default()).unwrap();
        let (runtime, projection) = import_workbook(
            &source,
            &ImportSelection {
                column_types: vec![vec![ImportFieldType::Text, ImportFieldType::Number]],
                extra_columns: vec![vec![]],
            },
            OCCURRENCE,
        )
        .expect("imported Text-capacity rectangle");
        assert_eq!(projection.opened.table.rows.len(), 65);
        return (Some(runtime), projection.opened.table);
    }
    let mut runtime = None;
    let DesignerWireReply::Ok {
        response: DesignerResponse::Opened(opened),
    } = request(
        &mut runtime,
        &json!({"type":"new_table", "occurrence_id":OCCURRENCE, "name":"Batch",
            "columns":[{"name":"left","field_type":"text"},
                       {"name":"right","field_type":"number"}]}),
    )
    else {
        panic!("native table must open");
    };
    publish(
        &mut runtime,
        &json!({"type":"paste_cells", "expected_revision":opened.table.revision,
            "collection":opened.table.collection.id, "start_entity":null,
            "start_field":opened.table.columns[0].id,
            "rows":[["old-left","10"],["other-left","20"]]}),
    );
    let before = table(&mut runtime, &opened.table.collection.id);
    assert_eq!(before.native_table_profile, Some(true));
    (runtime, before)
}

fn edits(before: &TableProjection, value: &str) -> Value {
    json!([
        {"target":before.rows[0].fields[0].target, "input":{"kind":"text","value":value}},
        {"target":before.rows[1].fields[1].target, "input":{"kind":"number","input":value}},
    ])
}

fn batch(before: &TableProjection, value: &str) -> Value {
    json!({"type":"edit_cells", "expected_revision":before.revision,
        "edits":edits(before, value)})
}

fn assert_one_publication(publication: &PublicationProjection, before: &TableProjection) {
    assert_eq!(publication.base_revision, before.revision);
    let revision: u64 = before
        .revision
        .strip_prefix("resident/")
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        publication.resulting_revision,
        format!("resident/{}", revision + 1)
    );
    assert_eq!(publication.fields.len(), 2);
    for target in [
        &before.rows[0].fields[0].target,
        &before.rows[1].fields[1].target,
    ] {
        assert!(publication.fields.contains(target));
    }
}

fn assert_same_values(actual: &TableProjection, expected: &TableProjection) {
    let mut expected = expected.clone();
    expected.revision.clone_from(&actual.revision);
    assert_eq!(actual, &expected);
}

fn history(
    runtime: &mut Option<DesignerRuntime>,
    before: &TableProjection,
    direction: &str,
) -> TableProjection {
    let publication = publish(
        runtime,
        &json!({"type":direction, "expected_revision":before.revision}),
    );
    assert_one_publication(&publication, before);
    table(runtime, &before.collection.id)
}

fn assert_refused(
    runtime: &mut Option<DesignerRuntime>,
    input: &Value,
    before: &TableProjection,
    code: &str,
) {
    let reply = request(runtime, input);
    let DesignerWireReply::Error { error } = reply else {
        panic!("invalid batch must be refused: {reply:?}");
    };
    assert_eq!(error.code, code);
    assert_eq!(error.current_revision, before.revision);
    assert_eq!(table(runtime, &before.collection.id), *before);
}

fn atomic_history(imported: bool) {
    let (mut runtime, before) = fixture(imported);
    let publication = publish(&mut runtime, &batch(&before, "101"));
    assert_one_publication(&publication, &before);
    let accepted = table(&mut runtime, &before.collection.id);
    let mut expected = before.clone();
    expected.rows[0].fields[0].stored = Some(StoredValueProjection::Text {
        value: "101".into(),
    });
    expected.rows[1].fields[1].stored = Some(StoredValueProjection::Number { value: 101.0 });
    // The Number column's footer follows the edited cell: the batch replaces
    // one stored value, so the total moves by the difference.
    let edited_column = expected.rows[1].fields[1].target.field.clone();
    let previous = before.rows[1].fields[1]
        .stored
        .as_ref()
        .and_then(StoredValueProjection::number)
        .expect("the fixture's second row holds a Number value");
    let total = before.column_totals[&edited_column].expect("the fixture totals its Number column");
    expected
        .column_totals
        .insert(edited_column, Some(total - previous + 101.0));
    assert_same_values(&accepted, &expected);
    let undone = history(&mut runtime, &accepted, "undo");
    assert_same_values(&undone, &before);
    let redone = history(&mut runtime, &undone, "redo");
    assert_same_values(&redone, &accepted);
    assert_refused(
        &mut runtime,
        &json!({"type":"redo", "expected_revision":redone.revision}),
        &redone,
        "invalid_tracker_operation",
    );
}

fn refusals_preserve_history(imported: bool) {
    let (mut runtime, original) = fixture(imported);
    publish(&mut runtime, &batch(&original, "101"));
    let first = table(&mut runtime, &original.collection.id);
    publish(&mut runtime, &batch(&first, "202"));
    let second = table(&mut runtime, &original.collection.id);
    let before = history(&mut runtime, &second, "undo");
    // Both directions are nonempty: refusals must neither add Undo nor clear Redo.
    let mut mixed = batch(&before, "must not leak");
    mixed["edits"][1]["input"] = json!({"kind":"number", "input":"not a number"});
    assert_refused(&mut runtime, &mixed, &before, "invalid_number");
    let mut duplicate = batch(&before, "duplicate");
    duplicate["edits"][1]["target"] = duplicate["edits"][0]["target"].clone();
    assert_refused(
        &mut runtime,
        &duplicate,
        &before,
        "invalid_tracker_operation",
    );
    assert_refused(
        &mut runtime,
        &json!({"type":"edit_cells", "expected_revision":before.revision, "edits":[]}),
        &before,
        "invalid_tracker_operation",
    );
    // Cardinality is checked independently of the stricter public byte limit.
    let oversized = json!({"type":"edit_cells", "expected_revision":before.revision,
        "edits":vec![edits(&before, "too many")[0].clone(); 1025]});
    let typed: DesignerRequest = serde_json::from_value(oversized).unwrap();
    let error = runtime.as_mut().unwrap().handle(typed).unwrap_err();
    assert_eq!(
        error.failure_projection(&before.revision).code,
        "invalid_tracker_operation"
    );
    assert!(
        error
            .to_string()
            .contains("edit range is empty or too large")
    );
    assert_eq!(table(&mut runtime, &before.collection.id), before);
    let wire_oversized = batch(&before, &"x".repeat(65_536));
    assert_refused(&mut runtime, &wire_oversized, &before, "request_too_large");
    let redone = history(&mut runtime, &before, "redo");
    assert_same_values(&redone, &second);
    let undone = history(&mut runtime, &redone, "undo");
    assert_same_values(&undone, &first);
    let original_again = history(&mut runtime, &undone, "undo");
    assert_same_values(&original_again, &original);
}

#[test]
fn native_batch_is_one_publication_and_history_unit() {
    atomic_history(false);
}

#[test]
fn imported_text_capacity_batch_is_one_publication_and_history_unit() {
    atomic_history(true);
}

#[test]
fn native_batch_refusals_preserve_values_revision_and_both_history_directions() {
    refusals_preserve_history(false);
}

#[test]
fn imported_text_capacity_batch_refusals_preserve_values_revision_and_both_history_directions() {
    refusals_preserve_history(true);
}
