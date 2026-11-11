//! ADR-0048 §5: the stored row position on a window, locate and Find.
//!
//! A view reorders and hides rows, so the displayed index is not the number a
//! positional reference names. Every row of a window carries `position`, its
//! one-based stored position, and locate and Find report that same number.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    ComparisonOperator, DesignerRequest, DesignerResponse, DesignerRuntime, FindDirection,
    SortDirection, TableView, ViewFilter, ViewSortKey, ViewTextOrder,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const POSITION_SEED: &str = "00000000-0000-4000-8000-000000000c02";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn field(id: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
    let id = FieldId::from(id);
    let key = id
        .as_str()
        .split_once('_')
        .map_or_else(|| id.as_str().to_owned(), |(_, key)| key.to_owned());
    (
        id.clone(),
        FieldDefinition {
            id,
            key: FieldKey::from(key.as_str()),
            display_name: None,
            field_type,
            required: false,
            constraint: FieldConstraint::None,
            number_format: None,
            date_format: None,
        },
    )
}

fn entity(id: &str, values: Vec<(&str, Value)>) -> (EntityId, Entity) {
    (
        EntityId::from(id),
        Entity {
            id: EntityId::from(id),
            key: EntityKey::from(id),
            schema: SchemaId::from("rows"),
            fields: values
                .into_iter()
                .map(|(field, value)| (FieldId::from(field), value))
                .collect::<BTreeMap<_, _>>(),
        },
    )
}

/// Rows in stored order: e01, e02, e03, e04, with an ascending price so a
/// descending sort reverses them exactly.
fn position_document() -> Document {
    let mut document = Document::empty("position_seed", "Position seed");
    let schema = SchemaId::from("rows");
    document.schemas.insert(
        schema.clone(),
        Schema {
            field_order: None,
            id: schema.clone(),
            key: SchemaKey::from("rows"),
            display_name: None,
            fields: vec![
                field("f01_name", FieldType::Text),
                field("f02_price", FieldType::Number),
            ]
            .into_iter()
            .collect(),
        },
    );
    document.entities = vec![
        entity(
            "e01",
            vec![("f01_name", text("Alpha")), ("f02_price", number(10.0))],
        ),
        entity(
            "e02",
            vec![("f01_name", text("Bravo")), ("f02_price", number(20.0))],
        ),
        entity(
            "e03",
            vec![("f01_name", text("Charlie")), ("f02_price", number(30.0))],
        ),
        entity(
            "e04",
            vec![("f01_name", text("Delta")), ("f02_price", number(40.0))],
        ),
    ]
    .into_iter()
    .collect();
    document
}

fn position_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(position_document(), POSITION_SEED)
        .expect("the fixture is admitted by the ordinary profile")
}

/// The window's rows as `(entity id, stored position)`.
fn shown(runtime: &mut DesignerRuntime, view: TableView) -> Vec<(String, usize)> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime
        .handle(DesignerRequest::QueryTableWindow {
            expected_revision,
            collection: "rows".to_owned(),
            view,
            start: 0,
            count: 256,
            display: false,
        })
        .expect("the view is queryable")
    {
        DesignerResponse::TableWindow(window) => window
            .rows
            .into_iter()
            .map(|row| {
                (
                    row.id,
                    row.position.expect("a window row carries its position"),
                )
            })
            .collect(),
        other => panic!("expected a table window, got {other:?}"),
    }
}

fn descending() -> TableView {
    TableView {
        sort: vec![ViewSortKey {
            field: "f02_price".to_owned(),
            direction: SortDirection::Desc,
            text_order: ViewTextOrder::Bytes,
        }],
        ..TableView::default()
    }
}

fn pairs(expected: &[(&str, usize)]) -> Vec<(String, usize)> {
    expected
        .iter()
        .map(|(id, position)| ((*id).to_owned(), *position))
        .collect()
}

fn position_in(plain: &[(String, usize)], entity: &str) -> usize {
    plain
        .iter()
        .find(|(id, _)| id == entity)
        .map(|(_, position)| *position)
        .expect("the entity is in the plain window")
}

#[test]
fn a_plain_window_numbers_rows_from_one() {
    let mut runtime = position_runtime();
    assert_eq!(
        shown(&mut runtime, TableView::default()),
        pairs(&[("e01", 1), ("e02", 2), ("e03", 3), ("e04", 4)])
    );
}

#[test]
fn a_descending_sort_reports_stored_positions_in_reverse() {
    let mut runtime = position_runtime();
    let plain = shown(&mut runtime, TableView::default());
    let sorted = shown(&mut runtime, descending());
    assert_eq!(
        sorted,
        pairs(&[("e04", 4), ("e03", 3), ("e02", 2), ("e01", 1)])
    );
    for (id, position) in &sorted {
        assert_eq!(*position, position_in(&plain, id));
    }
}

#[test]
fn a_filter_leaves_gaps_and_keeps_each_stored_position() {
    let mut runtime = position_runtime();
    let plain = shown(&mut runtime, TableView::default());
    let filtered = shown(
        &mut runtime,
        TableView {
            filters: vec![ViewFilter::NumberCompare {
                field: "f02_price".to_owned(),
                op: ComparisonOperator::Gte,
                value: 20.0,
            }],
            ..TableView::default()
        },
    );
    assert_eq!(filtered, pairs(&[("e02", 2), ("e03", 3), ("e04", 4)]));
    for (id, position) in &filtered {
        assert_eq!(*position, position_in(&plain, id));
    }
    assert!(
        filtered.iter().all(|(_, position)| *position != 1),
        "the filtered-out first row leaves a gap, not a renumbering"
    );
}

#[test]
fn locate_and_find_report_the_plain_window_position() {
    let mut runtime = position_runtime();
    let plain = shown(&mut runtime, TableView::default());
    let expected_revision = runtime.observe_occurrence().revision;
    let located = match runtime
        .handle(DesignerRequest::LocateInView {
            expected_revision,
            collection: "rows".to_owned(),
            view: descending(),
            entity: "e03".to_owned(),
        })
        .expect("the view is queryable")
    {
        DesignerResponse::LocatedInView(located) => located,
        other => panic!("expected a located-in-view projection, got {other:?}"),
    };
    assert_eq!(located.index, Some(1), "the sorted view moves e03 to row 1");
    assert_eq!(located.position, Some(position_in(&plain, "e03")));

    let expected_revision = runtime.observe_occurrence().revision;
    let found = match runtime
        .handle(DesignerRequest::FindInView {
            expected_revision,
            collection: "rows".to_owned(),
            view: descending(),
            query: "charlie".to_owned(),
            fields: vec!["f01_name".to_owned()],
            from: None,
            direction: FindDirection::Next,
        })
        .expect("the view is searchable")
    {
        DesignerResponse::FoundInView(found) => found,
        other => panic!("expected a find reply, got {other:?}"),
    };
    let found = found.found.expect("Charlie matches");
    assert_eq!(found.entity, "e03");
    assert_eq!(found.row, 1, "the sorted view moves e03 to row 1");
    assert_eq!(found.position, position_in(&plain, "e03"));
}

#[test]
fn a_query_table_reply_has_no_position_key() {
    let mut runtime = position_runtime();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "rows".to_owned(),
        })
        .expect("the small table is below the compatibility cap")
    else {
        panic!("expected a complete table");
    };
    let encoded = serde_json::to_string(&table).expect("the table encodes");
    assert!(
        !encoded.contains("\"position\""),
        "only a window carries the key"
    );
}
