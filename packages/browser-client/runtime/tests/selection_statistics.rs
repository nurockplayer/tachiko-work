//! Work #811: revision-bound selection statistics over resident values.
use std::{collections::BTreeMap, io::Write, time::Instant};

use serde_json::json;

use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, DesignerWireReply,
    FieldTarget, SelectionStatisticsProjection, SelectionStatisticsRange, SortDirection, TableView,
    ViewSortKey, ViewTextOrder, process_wire_request,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, Expression, FieldConstraint, FieldDefinition, FieldId, FieldType,
    Number, Schema, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000811";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn field(id: &str) -> (FieldId, FieldDefinition) {
    (
        id.into(),
        FieldDefinition {
            id: id.into(),
            key: id.into(),
            display_name: None,
            field_type: FieldType::Number,
            required: false,
            constraint: FieldConstraint::None,
            number_format: None,
            date_format: None,
        },
    )
}

fn document(values: &[f64], columns: usize) -> Document {
    let mut document = Document::empty("selection", "Selection statistics");
    let fields = (0..columns)
        .map(|index| field(&format!("f{index:02}")))
        .collect::<BTreeMap<_, _>>();
    document.schemas.insert(
        "rows".into(),
        Schema {
            id: "rows".into(),
            key: "rows".into(),
            display_name: None,
            field_order: None,
            fields,
        },
    );
    document.entities = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let id = EntityId::from(format!("e{index:05}"));
            (
                id.clone(),
                Entity {
                    rank: None,
                    id,
                    key: format!("row{index}").into(),
                    schema: "rows".into(),
                    fields: (0..columns)
                        .map(|column| (format!("f{column:02}").into(), number(*value)))
                        .collect(),
                },
            )
        })
        .collect();
    document
}

fn runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(document(&[1e16, -1e16, 1.0], 1), OCCURRENCE).unwrap()
}

fn targets(ids: &[&str]) -> Vec<FieldTarget> {
    ids.iter()
        .map(|entity| FieldTarget {
            entity: (*entity).into(),
            field: "f00".into(),
        })
        .collect()
}

fn statistics(
    runtime: &mut DesignerRuntime,
    targets: Vec<FieldTarget>,
) -> SelectionStatisticsProjection {
    let response = runtime
        .handle(DesignerRequest::QuerySelectionStatistics {
            expected_revision: runtime.observe_occurrence().revision,
            targets: Some(targets),
            range: None,
        })
        .unwrap();
    let DesignerResponse::SelectionStatistics(stats) = response else {
        panic!("expected selection statistics, got {response:?}");
    };
    stats
}

#[test]
fn selection_statistics_wire_round_trip_and_read_preserves_revision() {
    let runtime = runtime();
    let before = runtime.observe_occurrence();
    let mut input: serde_json::Value = serde_json::from_str(
        r#"{
        "type": "query_selection_statistics", "expected_revision": "",
        "targets": [{"entity": "e00002", "field": "f00"}]
    }"#,
    )
    .unwrap();
    input["expected_revision"] = serde_json::to_value(&before.revision).unwrap();
    let request: DesignerRequest = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), input);
    let mut runtime = Some(runtime);
    let bytes = process_wire_request(&mut runtime, &serde_json::to_vec(&input).unwrap());
    let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reply["status"], "ok");
    let mut expected: serde_json::Value = serde_json::from_str(
        r#"{
        "type": "selection_statistics", "payload": {
            "revision": "", "count": 1, "numerical_count": 1,
            "sum": 1.0, "average": 1.0, "min": 1.0, "max": 1.0, "error_count": 0
        }
    }"#,
    )
    .unwrap();
    expected["payload"]["revision"] = serde_json::to_value(&before.revision).unwrap();
    assert_eq!(reply["response"], expected);
    let response: DesignerResponse = serde_json::from_value(reply["response"].clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), reply["response"]);
    assert_eq!(runtime.as_ref().unwrap().observe_occurrence(), before);
}

#[test]
fn selection_statistics_empty_wire_numbers_and_nulls() {
    let mut runtime = runtime();
    let stats = statistics(&mut runtime, vec![]);
    let json = serde_json::to_value(DesignerResponse::SelectionStatistics(stats)).unwrap();
    assert_eq!(json["payload"]["count"], 0);
    assert_eq!(json["payload"]["numerical_count"], 0);
    assert_eq!(json["payload"]["error_count"], 0);
    assert_eq!(json["payload"]["sum"], 0.0);
    for key in ["average", "min", "max"] {
        assert!(json["payload"][key].is_null());
    }
    assert_eq!(json["payload"].as_object().unwrap().len(), 8);
}

#[test]
fn selection_statistics_stale_revision_refuses_before_invalid_targets() {
    let mut runtime = runtime();
    let before = runtime.observe_occurrence();
    let result = runtime.handle(DesignerRequest::QuerySelectionStatistics {
        expected_revision: "stale".into(),
        targets: Some(targets(&["unknown"])),
        range: None,
    });
    assert!(matches!(result, Err(DesignerError::StaleQuery { .. })));
    assert_eq!(runtime.observe_occurrence(), before);
}

#[test]
fn selection_statistics_unknown_undeclared_and_duplicate_targets_refuse() {
    let mut runtime = runtime();
    let before = runtime.observe_occurrence();
    let cases = [
        (targets(&["e00000", "unknown"]), "unknown.f00"),
        (
            vec![
                FieldTarget::from("e00000.f00"),
                FieldTarget::from("e00000.undeclared"),
            ],
            "e00000.undeclared",
        ),
        (targets(&["e00000", "e00000"]), "e00000.f00"),
    ];
    for (targets, target_name) in cases {
        let result = runtime.handle(DesignerRequest::QuerySelectionStatistics {
            expected_revision: before.revision.clone(),
            targets: Some(targets),
            range: None,
        });
        let Err(DesignerError::InvalidView { message }) = result else {
            panic!("expected InvalidView for {target_name}, got {result:?}");
        };
        assert!(message.contains(target_name), "{message}");
        assert_eq!(runtime.observe_occurrence(), before);
    }
}

#[test]
fn selection_statistics_declared_absent_field_is_not_undeclared() {
    let mut doc = document(&[5.0], 2);
    doc.entities.get_mut("e00000").unwrap().fields.remove("f01");
    let mut runtime = DesignerRuntime::from_document(doc, OCCURRENCE).unwrap();
    let stats = statistics(&mut runtime, vec![FieldTarget::from("e00000.f01")]);
    assert_eq!(stats.count, 0);
    assert_eq!(stats.sum, Some(0.0));
}

#[test]
fn selection_statistics_sorted_view_and_collection_order_are_identical() {
    let mut runtime = runtime();
    let response = runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision: runtime.observe_occurrence().revision,
            collection: "rows".into(),
            view: TableView {
                sort: vec![ViewSortKey {
                    field: "f00".into(),
                    direction: SortDirection::Asc,
                    text_order: ViewTextOrder::Bytes,
                }],
                ..TableView::default()
            },
            start: 0,
            count: 3,
            display: false,
        })
        .unwrap();
    let DesignerResponse::TableWindow(window) = response else {
        panic!("expected table window, got {response:?}");
    };
    let ids = window
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["e00001", "e00002", "e00000"]);
    let sorted = statistics(&mut runtime, targets(&ids));
    let collection = statistics(&mut runtime, targets(&["e00000", "e00001", "e00002"]));
    assert_eq!(sorted, collection);
    assert_eq!(sorted.sum.map(f64::to_bits), Some(1.0_f64.to_bits()));
}

#[test]
fn selection_statistics_one_hundred_thousand_targets_reply_is_bounded() {
    let doc = document(&vec![2.0; 10_000], 10);
    let runtime = DesignerRuntime::from_document_working_size(doc, OCCURRENCE).unwrap();
    let before = runtime.observe_occurrence();
    let targets = (0..10_000)
        .flat_map(|row| {
            (0..10).map(move |column| FieldTarget {
                entity: format!("e{row:05}"),
                field: format!("f{column:02}"),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 100_000);
    let request = DesignerRequest::QuerySelectionStatistics {
        expected_revision: before.revision.clone(),
        targets: Some(targets),
        range: None,
    };
    let input = serde_json::to_vec(&request).unwrap();
    let mut runtime = Some(runtime);
    let started = Instant::now();
    let bytes = process_wire_request(&mut runtime, &input);
    writeln!(
        std::io::stderr(),
        "selection statistics 100000 targets elapsed_ms={}",
        started.elapsed().as_millis()
    )
    .unwrap();
    assert!(
        bytes.len() < 1024,
        "aggregate reply is {} bytes",
        bytes.len()
    );
    let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(reply["response"]["type"], "selection_statistics");
    let mut expected: serde_json::Value = serde_json::from_str(
        r#"{
        "revision": "", "count": 100000, "numerical_count": 100000,
        "sum": 200000.0, "average": 2.0, "min": 2.0, "max": 2.0, "error_count": 0
    }"#,
    )
    .unwrap();
    expected["revision"] = serde_json::to_value(&before.revision).unwrap();
    assert_eq!(reply["response"]["payload"], expected);
    assert_eq!(runtime.as_ref().unwrap().observe_occurrence(), before);
}

#[test]
fn selection_statistics_wire_request_limit_depends_on_session_profile() {
    let mut doc = document(&vec![2.0; 128], 32);
    // 4096 distinct declared targets, one of them holding a Number, exceed the
    // ordinary wire request limit.
    for entity in doc.entities.values_mut() {
        entity.fields.clear();
    }
    doc.entities
        .get_mut("e00000")
        .unwrap()
        .fields
        .insert("f00".into(), number(2.0));
    // The limit is checked on the raw bytes before parsing, so the ordinary
    // session needs no document these targets could name: an ordinary open
    // refuses a 128 x 32 document's worst-case projection anyway.
    let ordinary = DesignerRuntime::from_document(document(&[2.0], 1), OCCURRENCE).unwrap();
    let ordinary_before = ordinary.observe_occurrence();
    let working = DesignerRuntime::from_document_working_size(doc, OCCURRENCE).unwrap();
    let before = working.observe_occurrence();
    let targets = (0..128)
        .flat_map(|row| {
            (0..32).map(move |column| FieldTarget {
                entity: format!("e{row:05}"),
                field: format!("f{column:02}"),
            })
        })
        .collect();
    let input = serde_json::to_vec(&DesignerRequest::QuerySelectionStatistics {
        expected_revision: before.revision.clone(),
        targets: Some(targets),
        range: None,
    })
    .unwrap();
    assert!(input.len() > 65_536);
    assert!(input.len() < 64 * 1024 * 1024);
    let mut ordinary = Some(ordinary);
    let reply = process_wire_request(&mut ordinary, &input);
    let DesignerWireReply::Error { error } = serde_json::from_slice(&reply).unwrap() else {
        panic!("expected oversized request refusal");
    };
    assert_eq!(error.code, "request_too_large");
    assert_eq!(error.current_revision, ordinary_before.revision);
    assert_eq!(
        ordinary.as_ref().unwrap().observe_occurrence(),
        ordinary_before
    );

    let mut working = Some(working);
    let reply = process_wire_request(&mut working, &input);
    let reply: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(reply["response"]["type"], "selection_statistics", "{reply}");
    let mut expected: serde_json::Value = serde_json::from_str(
        r#"{
        "revision": "", "count": 1, "numerical_count": 1,
        "sum": 2.0, "average": 2.0, "min": 2.0, "max": 2.0, "error_count": 0
    }"#,
    )
    .unwrap();
    expected["revision"] = serde_json::to_value(&before.revision).unwrap();
    assert_eq!(reply["response"]["payload"], expected);
    assert_eq!(working.as_ref().unwrap().observe_occurrence(), before);
}

fn range_statistics(
    runtime: &mut DesignerRuntime,
    first: &str,
    last: &str,
    fields: &[&str],
) -> SelectionStatisticsProjection {
    let response = runtime
        .handle(DesignerRequest::QuerySelectionStatistics {
            expected_revision: runtime.observe_occurrence().revision,
            targets: None,
            range: Some(SelectionStatisticsRange {
                collection: "rows".into(),
                first: first.into(),
                last: last.into(),
                fields: fields.iter().map(|field| (*field).into()).collect(),
            }),
        })
        .unwrap();
    let DesignerResponse::SelectionStatistics(stats) = response else {
        panic!("expected selection statistics, got {response:?}");
    };
    stats
}

#[test]
fn range_stats_match_per_cell_targets() {
    let mut doc = document(&[1e16, -1e16, 1.0], 3);
    doc.schemas
        .get_mut("rows")
        .unwrap()
        .fields
        .get_mut("f01")
        .unwrap()
        .field_type = FieldType::Text;
    for (index, entity) in doc.entities.values_mut().enumerate() {
        entity.fields.insert(
            "f01".into(),
            Value::Text(if index == 0 {
                String::new()
            } else {
                "123".into()
            }),
        );
        entity.fields.insert(
            "f02".into(),
            Value::Formula(Expression::Number(Number::new(2.0).unwrap())),
        );
    }
    doc.entities.get_mut("e00001").unwrap().fields.remove("f00");
    let mut runtime = DesignerRuntime::from_document(doc, OCCURRENCE).unwrap();
    let before = runtime.observe_occurrence();
    let cells = (0..3)
        .flat_map(|row| {
            (0..3).map(move |column| FieldTarget {
                entity: format!("e{row:05}"),
                field: format!("f{column:02}"),
            })
        })
        .collect();
    let expected = statistics(&mut runtime, cells);
    for (first, last) in [("e00000", "e00002"), ("e00002", "e00000")] {
        assert_eq!(
            range_statistics(&mut runtime, first, last, &["f02", "f01", "f00"]),
            expected
        );
    }
    assert_eq!(expected.count, 7);
    assert_eq!(expected.error_count, 0);
    assert_eq!(
        range_statistics(&mut runtime, "e00001", "e00001", &["f00"]).count,
        0
    );
    assert_eq!(
        range_statistics(&mut runtime, "e00000", "e00002", &[]).count,
        0
    );
    assert_eq!(runtime.observe_occurrence(), before);
}

#[test]
fn range_stats_follow_stored_order_after_positional_insert() {
    let mut runtime =
        DesignerRuntime::from_document(document(&[10.0, 20.0, 30.0], 1), OCCURRENCE).unwrap();
    runtime
        .handle(
            serde_json::from_value(json!({
                "type": "insert_row", "expected_revision": runtime.observe_occurrence().revision,
                "collection": "rows", "position": {"before": "e00001"},
                "initializers": [{"field": "f00", "input": {"kind": "number", "input": "99"}}]
            }))
            .unwrap(),
        )
        .unwrap();
    let response = runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision: runtime.observe_occurrence().revision,
            collection: "rows".into(),
            view: TableView::default(),
            start: 0,
            count: 4,
            display: false,
        })
        .unwrap();
    let DesignerResponse::TableWindow(window) = response else {
        panic!("expected window");
    };
    assert_eq!(window.rows[0].id, "e00000");
    assert_eq!(window.rows[2].id, "e00001");
    assert_eq!(window.rows[3].id, "e00002");
    let cells = window.rows[..3]
        .iter()
        .map(|row| FieldTarget {
            entity: row.id.clone(),
            field: "f00".into(),
        })
        .collect();
    let expected = statistics(&mut runtime, cells);
    assert_eq!(expected.count, 3);
    assert_eq!(expected.sum, Some(129.0));
    assert_eq!(
        range_statistics(&mut runtime, "e00001", "e00000", &["f00"]),
        expected
    );
}

#[test]
fn range_stats_reject_both_forms_unknown_anchor_or_field_atomically() {
    let mut doc = document(&[2.0, 4.0], 1);
    let mut foreign = doc.entities["e00000"].clone();
    let mut other_schema = doc.schemas["rows"].clone();
    other_schema.id = "other".into();
    other_schema.key = "other".into();
    foreign.id = "foreign".into();
    foreign.key = "foreign".into();
    foreign.schema = "other".into();
    doc.schemas.insert(other_schema.id.clone(), other_schema);
    doc.entities.insert(foreign.id.clone(), foreign);
    let mut runtime = Some(DesignerRuntime::from_document(doc, OCCURRENCE).unwrap());
    let before = runtime.as_ref().unwrap().observe_occurrence();
    let saved = runtime
        .as_ref()
        .unwrap()
        .export_project_v3(&before.revision)
        .unwrap()
        .bytes;
    let base =
        json!({"collection": "rows", "first": "e00000", "last": "e00001", "fields": ["f00"]});
    let with = |key: &str, value: serde_json::Value| {
        let mut range = base.clone();
        range[key] = value;
        json!({"range": range})
    };
    let cases = [
        json!({"targets": [], "range": base}),
        json!({}),
        with("first", json!("unknown")),
        with("last", json!("unknown")),
        with("first", json!("foreign")),
        with("last", json!("foreign")),
        with("fields", json!(["f00", "unknown"])),
        with("fields", json!(["f00", "f00"])),
    ];
    for mut input in cases {
        input["type"] = "query_selection_statistics".into();
        input["expected_revision"] = before.revision.clone().into();
        let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
            &mut runtime,
            &serde_json::to_vec(&input).unwrap(),
        ))
        .unwrap();
        assert_eq!(reply["status"], "error", "{reply}");
        assert_eq!(reply["error"]["code"], "invalid_view", "{reply}");
        assert_eq!(reply["error"]["current_revision"], before.revision);
        assert_eq!(runtime.as_ref().unwrap().observe_occurrence(), before);
        assert_eq!(
            runtime
                .as_ref()
                .unwrap()
                .export_project_v3(&before.revision)
                .unwrap()
                .bytes,
            saved
        );
    }
    let result = runtime
        .as_mut()
        .unwrap()
        .handle(DesignerRequest::QuerySelectionStatistics {
            expected_revision: "stale".into(),
            targets: None,
            range: Some(serde_json::from_value(base).unwrap()),
        });
    assert!(matches!(result, Err(DesignerError::StaleQuery { .. })));
}

fn imported_runtime(rows: usize, working: bool) -> DesignerRuntime {
    use tachiko_designer_runtime::interop_adapter::{ImportOptions, import_csv};
    use tachiko_designer_runtime::{
        ImportFieldType, ImportSelection, import_workbook, import_workbook_working_size,
    };
    let mut source = import_csv(b"Left,Right\n2,4\n", &ImportOptions::default()).unwrap();
    source.sheets[0].rows = vec![source.sheets[0].rows[0].clone(); rows];
    let selection = ImportSelection {
        column_types: vec![vec![ImportFieldType::Number; 2]],
        extra_columns: vec![vec![]],
    };
    if working {
        import_workbook_working_size(&source, &selection, OCCURRENCE)
            .unwrap()
            .0
    } else {
        import_workbook(&source, &selection, OCCURRENCE).unwrap().0
    }
}

fn imported_window(
    runtime: &mut DesignerRuntime,
    start: usize,
    count: usize,
) -> tachiko_designer_runtime::TableWindowProjection {
    let response = runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision: runtime.observe_occurrence().revision,
            collection: "sheet_1".into(),
            view: TableView::default(),
            start,
            count,
            display: false,
        })
        .unwrap();
    let DesignerResponse::TableWindow(window) = response else {
        panic!("expected imported window");
    };
    window
}

fn wire_statistics(
    runtime: &mut Option<DesignerRuntime>,
    selection: serde_json::Value,
) -> serde_json::Value {
    let mut input = selection;
    input["type"] = "query_selection_statistics".into();
    input["expected_revision"] = runtime
        .as_ref()
        .unwrap()
        .observe_occurrence()
        .revision
        .into();
    let input = serde_json::to_vec(&input).unwrap();
    assert!(input.len() < 65_536);
    let reply: serde_json::Value =
        serde_json::from_slice(&process_wire_request(runtime, &input)).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    reply["response"]["payload"].clone()
}

#[test]
fn range_stats_large_imported_session_fits_request_cap() {
    for (rows, working) in [(32, false), (10_000, true)] {
        let mut resident = imported_runtime(rows, working);
        let before = resident.observe_occurrence();
        let window = imported_window(&mut resident, 0, 32);
        let last = imported_window(&mut resident, rows - 1, 1).rows[0]
            .id
            .clone();
        let fields = window
            .columns
            .iter()
            .map(|column| column.id.clone())
            .collect::<Vec<_>>();
        let cells = window
            .rows
            .iter()
            .flat_map(|row| {
                fields.iter().map(move |field| FieldTarget {
                    entity: row.id.clone(),
                    field: field.clone(),
                })
            })
            .collect::<Vec<_>>();
        let mut runtime = Some(resident);
        let per_cell = wire_statistics(&mut runtime, json!({"targets": cells}));
        let small_range = wire_statistics(
            &mut runtime,
            json!({"range": {
                "collection": "sheet_1", "first": window.rows[0].id,
                "last": window.rows[31].id, "fields": fields,
            }}),
        );
        assert_eq!(small_range, per_cell);
        let all = wire_statistics(
            &mut runtime,
            json!({"range": {
                "collection": "sheet_1", "first": window.rows[0].id, "last": last, "fields": fields,
            }}),
        );
        assert_eq!(all["count"], rows * 2);
        assert_eq!(all["numerical_count"], rows * 2);
        assert_eq!(
            all["sum"].as_f64(),
            Some(f64::from(u32::try_from(rows * 6).unwrap()))
        );
        assert_eq!(all["average"], 3.0);
        if !working {
            let input = serde_json::to_vec(&json!({
                "type": "query_selection_statistics", "expected_revision": before.revision,
                "targets": vec![cells[0].clone(); 2_000],
            }))
            .unwrap();
            assert!(input.len() > 65_536);
            let reply: serde_json::Value =
                serde_json::from_slice(&process_wire_request(&mut runtime, &input)).unwrap();
            assert_eq!(reply["error"]["code"], "request_too_large");
        }
        assert_eq!(runtime.as_ref().unwrap().observe_occurrence(), before);
    }
}
