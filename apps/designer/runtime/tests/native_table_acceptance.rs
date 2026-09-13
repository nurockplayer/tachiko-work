use serde_json::{Value, json};
use tachiko_designer_runtime::{
    DesignerResponse, DesignerRuntime, DesignerWireReply, StoredValueProjection, TableProjection,
    process_wire_request,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000315";
const SECOND_OCCURRENCE: &str = "00000000-0000-4000-8000-000000000316";

fn inventory_columns() -> Vec<Value> {
    vec![
        json!({"name": "item", "field_type": "text"}),
        json!({"name": "quantity", "field_type": "number"}),
        json!({"name": "active", "field_type": "boolean"}),
        json!({"name": "received", "field_type": "date"}),
    ]
}

fn new_inventory(occurrence_id: &str) -> Value {
    json!({
        "type": "new_table",
        "occurrence_id": occurrence_id,
        "name": "Inventory",
        "columns": inventory_columns(),
    })
}

fn request(runtime: &mut Option<DesignerRuntime>, request: &Value) -> DesignerWireReply {
    serde_json::from_slice(&process_wire_request(
        runtime,
        request.to_string().as_bytes(),
    ))
    .expect("the private runtime wire reply must remain JSON")
}

fn opened(reply: DesignerWireReply) -> tachiko_designer_runtime::OpenedProjection {
    let DesignerWireReply::Ok {
        response: DesignerResponse::Opened(opened),
    } = reply
    else {
        panic!("New Table must admit the Inventory candidate: {reply:?}");
    };
    *opened
}

fn published(reply: DesignerWireReply) -> tachiko_designer_runtime::PublicationProjection {
    let DesignerWireReply::Ok {
        response: DesignerResponse::Published(publication),
    } = reply
    else {
        panic!("the admitted Inventory operation must publish: {reply:?}");
    };
    publication
}

fn table(
    runtime: &mut Option<DesignerRuntime>,
    collection: &str,
) -> tachiko_designer_runtime::TableProjection {
    let DesignerWireReply::Ok {
        response: DesignerResponse::Table(table),
    } = request(
        runtime,
        &json!({"type": "query_table", "collection": collection}),
    )
    else {
        panic!("the admitted Inventory table must remain queryable");
    };
    table
}

fn project_bytes(runtime: &mut Option<DesignerRuntime>, expected_revision: &str) -> Vec<u8> {
    runtime
        .as_ref()
        .expect("the tested occurrence is open")
        .export_project(expected_revision)
        .expect("the current occurrence must export canonically")
        .bytes
}

fn stored_values(table: &TableProjection) -> Vec<Vec<Option<StoredValueProjection>>> {
    table
        .rows
        .iter()
        .map(|row| {
            row.fields
                .iter()
                .map(|field| field.stored.clone())
                .collect()
        })
        .collect()
}

fn expected_inventory_values() -> Vec<Vec<Option<StoredValueProjection>>> {
    vec![
        vec![
            Some(StoredValueProjection::Text {
                value: "0012".into(),
            }),
            Some(StoredValueProjection::Number { value: 3.0 }),
            Some(StoredValueProjection::Boolean { value: true }),
            Some(StoredValueProjection::Date {
                value: "2024-02-29".parse().expect("fixed Gregorian fixture"),
            }),
        ],
        vec![
            Some(StoredValueProjection::Text {
                value: "ノート".into(),
            }),
            Some(StoredValueProjection::Number { value: 0.0 }),
            Some(StoredValueProjection::Boolean { value: false }),
            Some(StoredValueProjection::Date {
                value: "2026-09-13".parse().expect("fixed Gregorian fixture"),
            }),
        ],
        vec![
            Some(StoredValueProjection::Text {
                value: "紙".into()
            }),
            Some(StoredValueProjection::Number { value: -2.0 }),
            Some(StoredValueProjection::Boolean { value: true }),
            Some(StoredValueProjection::Date {
                value: "2026-01-01".parse().expect("fixed Gregorian fixture"),
            }),
        ],
    ]
}

fn assert_opaque_table_and_column_ids(table: &TableProjection) {
    assert_ne!(
        table.collection.id, table.collection.key,
        "table identity must not use the user-facing table label",
    );
    assert!(
        table.columns.iter().all(|column| column.id != column.key),
        "field identity must not use a user-facing column label",
    );
}

fn assert_fresh_creation_generates_distinct_ids(first: &TableProjection, second: &TableProjection) {
    assert_eq!(first.collection.key, second.collection.key);
    assert_ne!(first.collection.id, second.collection.id);
    assert_eq!(
        first
            .columns
            .iter()
            .map(|column| &column.key)
            .collect::<Vec<_>>(),
        second
            .columns
            .iter()
            .map(|column| &column.key)
            .collect::<Vec<_>>(),
    );
    assert!(
        first
            .columns
            .iter()
            .zip(&second.columns)
            .all(|(first, second)| first.id != second.id),
        "each independently admitted candidate must receive new semantic field IDs",
    );
}

#[test]
fn inventory_creation_paste_and_reopen_preserve_typed_values_order_and_semantic_ids() {
    let mut runtime = None;
    let mut separately_created_runtime = None;
    let separately_created = opened(request(
        &mut separately_created_runtime,
        &new_inventory(SECOND_OCCURRENCE),
    ));
    let opened = opened(request(&mut runtime, &new_inventory(OCCURRENCE)));
    assert_eq!(opened.bootstrap.title, "Inventory");
    assert_eq!(opened.table.columns.len(), 4);
    assert_opaque_table_and_column_ids(&opened.table);
    assert_fresh_creation_generates_distinct_ids(&opened.table, &separately_created.table);
    assert_eq!(
        opened
            .table
            .columns
            .iter()
            .map(|column| (column.key.as_str(), column.field_type.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("item", "text"),
            ("quantity", "number"),
            ("active", "boolean"),
            ("received", "date"),
        ]
    );
    assert!(
        opened
            .table
            .columns
            .iter()
            .all(|column| !column.id.is_empty())
    );
    assert_eq!(
        opened
            .table
            .columns
            .iter()
            .map(|column| &column.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4,
        "column identity must not derive from a duplicate user-facing label",
    );

    let rows = vec![
        vec!["0012", "3", "true", "2024-02-29"],
        vec!["ノート", "0", "false", "2026-09-13"],
        vec!["紙", "-2", "true", "2026-01-01"],
    ];
    let publication = published(request(
        &mut runtime,
        &json!({
            "type": "paste_cells",
            "expected_revision": opened.table.revision,
            "collection": opened.table.collection.id,
            "start_entity": null,
            "start_field": opened.table.columns[0].id,
            "rows": rows,
        }),
    ));
    let before_close = table(&mut runtime, &opened.table.collection.id);
    assert_eq!(before_close.revision, publication.resulting_revision);
    assert_eq!(before_close.rows.len(), 3);
    assert_eq!(stored_values(&before_close), expected_inventory_values());
    assert!(before_close.rows.iter().all(|row| !row.id.is_empty()));
    assert_eq!(
        before_close
            .rows
            .iter()
            .map(|row| &row.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3,
        "row identity must remain distinct independently of row order",
    );

    let bytes = project_bytes(&mut runtime, &before_close.revision);
    tachiko_designer_runtime::close_project(&mut runtime);
    let reopened = tachiko_designer_runtime::open_project(&mut runtime, &bytes, SECOND_OCCURRENCE)
        .expect("saved Inventory must reopen through the actual project boundary");
    assert_eq!(reopened.table.collection, before_close.collection);
    assert_eq!(reopened.table.columns, before_close.columns);
    assert_eq!(reopened.table.rows, before_close.rows);
    assert_eq!(reopened.table.native_table_profile, Some(true));

    published(request(
        &mut runtime,
        &json!({
            "type": "edit_scalar",
            "expected_revision": reopened.table.revision,
            "target": before_close.rows[0].fields[0].target,
            "input": {"kind": "text", "value": "0013"},
        }),
    ));
    let continued = table(&mut runtime, &opened.table.collection.id);
    assert_eq!(continued.rows[0].id, before_close.rows[0].id);
    assert_eq!(
        continued.rows[0].fields[0].target,
        before_close.rows[0].fields[0].target
    );

}

#[test]
fn native_table_profile_survives_appends_after_fresh_reopen() {
    let mut runtime = None;
    let opened = opened(request(&mut runtime, &new_inventory(OCCURRENCE)));
    let bytes = project_bytes(&mut runtime, &opened.table.revision);
    tachiko_designer_runtime::close_project(&mut runtime);
    let reopened = tachiko_designer_runtime::open_project(&mut runtime, &bytes, SECOND_OCCURRENCE)
        .expect("saved Inventory must reopen as a fresh occurrence");
    assert_eq!(reopened.table.native_table_profile, Some(true));

    let first_append = published(request(
        &mut runtime,
        &json!({
            "type": "paste_cells",
            "expected_revision": reopened.table.revision,
            "collection": reopened.table.collection.id,
            "start_entity": null,
            "start_field": reopened.table.columns[0].id,
            "rows": [["continued", "4", "false", "2026-02-01"]],
        }),
    ));
    let after_first_append = table(&mut runtime, &opened.table.collection.id);
    assert_eq!(after_first_append.revision, first_append.resulting_revision);
    assert_eq!(after_first_append.native_table_profile, Some(true));

    let second_append = published(request(
        &mut runtime,
        &json!({
            "type": "paste_cells",
            "expected_revision": after_first_append.revision,
            "collection": after_first_append.collection.id,
            "start_entity": null,
            "start_field": after_first_append.columns[0].id,
            "rows": [["still-native", "5", "true", "2026-02-02"]],
        }),
    ));
    let after_second_append = table(&mut runtime, &opened.table.collection.id);
    assert_eq!(after_second_append.revision, second_append.resulting_revision);
    assert_eq!(after_second_append.native_table_profile, Some(true));
    assert_eq!(after_second_append.rows.len(), 2);
}

#[test]
fn invalid_or_over_capacity_creation_does_not_replace_or_dirty_the_current_occurrence() {
    let mut runtime = Some(DesignerRuntime::tracker(OCCURRENCE).expect("stock resident"));
    let resident = table(&mut runtime, "tracker");
    let before = project_bytes(&mut runtime, &resident.revision);
    let invalid_candidates = [
        json!({
            "type": "new_table",
            "occurrence_id": OCCURRENCE,
            "name": "",
            "columns": inventory_columns(),
        }),
        json!({
            "type": "new_table",
            "occurrence_id": OCCURRENCE,
            "name": "Inventory",
            "columns": [{"name": "", "field_type": "text"}],
        }),
        json!({
            "type": "new_table",
            "occurrence_id": OCCURRENCE,
            "name": "Inventory",
            "columns": [
                {"name": "item", "field_type": "text"},
                {"name": "item", "field_type": "number"},
            ],
        }),
        json!({
            "type": "new_table",
            "occurrence_id": OCCURRENCE,
            "name": "Inventory",
            "columns": [{"name": "owner", "field_type": "reference"}],
        }),
        json!({
            "type": "new_table",
            "occurrence_id": OCCURRENCE,
            "name": "Inventory",
            "columns": (0..33)
                .map(|index| json!({"name": format!("column_{index}"), "field_type": "text"}))
                .collect::<Vec<_>>(),
        }),
    ];

    for candidate in invalid_candidates {
        let DesignerWireReply::Error { error } = request(&mut runtime, &candidate) else {
            panic!(
                "invalid New Table candidate must reject before replacing the resident occurrence"
            );
        };
        assert_ne!(
            error.code, "invalid_request",
            "the typed candidate must be admitted far enough to report its actual rejection",
        );
        assert_eq!(table(&mut runtime, "tracker"), resident);
        assert_eq!(project_bytes(&mut runtime, &resident.revision), before);
    }
}

#[test]
fn typed_invalid_paste_and_stale_first_edit_preserve_the_new_inventory_occurrence() {
    let mut runtime = None;
    let opened = opened(request(&mut runtime, &new_inventory(OCCURRENCE)));
    let initial = table(&mut runtime, &opened.table.collection.id);
    let first = published(request(
        &mut runtime,
        &json!({
            "type": "paste_cells",
            "expected_revision": initial.revision,
            "collection": initial.collection.id,
            "start_entity": null,
            "start_field": initial.columns[0].id,
            "rows": [["0012", "3", "true", "2024-02-29"]],
        }),
    ));
    let admitted = table(&mut runtime, &initial.collection.id);
    assert_eq!(admitted.revision, first.resulting_revision);
    let before_invalid = project_bytes(&mut runtime, &admitted.revision);

    for rows in [
        json!([["0012", "NaN", "true", "2024-02-29"]]),
        json!([["0012", "3", "true", "2026-02-30"]]),
        json!([["0012", "3", "yes", "2024-02-29"]]),
        json!([
            ["valid", "3", "true", "2024-02-29"],
            ["invalid", "NaN", "false", "2026-01-01"],
        ]),
    ] {
        let DesignerWireReply::Error { error } = request(
            &mut runtime,
            &json!({
                "type": "paste_cells",
                "expected_revision": admitted.revision,
                "collection": admitted.collection.id,
                "start_entity": admitted.rows[0].id,
                "start_field": admitted.columns[0].id,
                "rows": rows,
            }),
        ) else {
            panic!("invalid typed Inventory paste must reject atomically");
        };
        assert_ne!(error.code, "invalid_request");
        assert_eq!(
            project_bytes(&mut runtime, &admitted.revision),
            before_invalid
        );
    }

    let second = published(request(
        &mut runtime,
        &json!({
            "type": "edit_scalar",
            "expected_revision": admitted.revision,
            "target": admitted.rows[0].fields[0].target,
            "input": {"kind": "text", "value": "0013"},
        }),
    ));
    let before_stale = project_bytes(&mut runtime, &second.resulting_revision);
    let DesignerWireReply::Error { error } = request(
        &mut runtime,
        &json!({
            "type": "edit_scalar",
            "expected_revision": admitted.revision,
            "target": admitted.rows[0].fields[0].target,
            "input": {"kind": "text", "value": "must not publish"},
        }),
    ) else {
        panic!("a stale first Inventory edit must reject");
    };
    assert_eq!(error.code, "stale_revision");
    assert_eq!(
        project_bytes(&mut runtime, &second.resulting_revision),
        before_stale
    );
}
