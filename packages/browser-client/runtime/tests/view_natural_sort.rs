//! #686 PR 4: `text_order` on a view sort key.
//!
//! Text sorts by UTF-8 bytes by default. `natural` follows Sheet's list order:
//! case-insensitive, with runs of ASCII digits compared as numbers and every
//! other character by code point. Blanks stay last in both directions and the
//! entity-ID tie-break is unchanged.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, LocateInViewResult,
    LocatedInViewProjection, SortDirection, TableView, ViewSortKey, ViewTextOrder,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const NATURAL: &str = "00000000-0000-4000-8000-000000000c04";

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

/// `Item 10`/`Item 2` prove digit runs; `apple`/`Banana`/`cherry` prove case
/// folding; three rows named `Item 2` keep a tie for the second sort key; e06
/// has no name, so it is the blank that stays last.
fn natural_document() -> Document {
    let mut document = Document::empty("natural_seed", "Natural seed");
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
                field("f02_amount", FieldType::Number),
            ]
            .into_iter()
            .collect(),
        },
    );
    document.entities = vec![
        entity(
            "e01",
            vec![("f01_name", text("Item 10")), ("f02_amount", number(100.0))],
        ),
        entity(
            "e02",
            vec![("f01_name", text("Item 2")), ("f02_amount", number(20.0))],
        ),
        entity(
            "e03",
            vec![("f01_name", text("apple")), ("f02_amount", number(3.0))],
        ),
        entity(
            "e04",
            vec![("f01_name", text("Banana")), ("f02_amount", number(4.0))],
        ),
        entity(
            "e05",
            vec![("f01_name", text("cherry")), ("f02_amount", number(5.0))],
        ),
        entity("e06", vec![("f02_amount", number(6.0))]),
        entity(
            "e07",
            vec![("f01_name", text("Item 2")), ("f02_amount", number(7.0))],
        ),
        entity(
            "e08",
            vec![("f01_name", text("Item 2")), ("f02_amount", number(2.0))],
        ),
    ]
    .into_iter()
    .collect();
    document
}

fn natural_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(natural_document(), NATURAL).expect("the fixture is admitted")
}

fn sorted(field: &str, direction: SortDirection, text_order: ViewTextOrder) -> TableView {
    TableView {
        sort: vec![ViewSortKey {
            field: field.to_owned(),
            direction,
            text_order,
        }],
        ..TableView::default()
    }
}

fn order(runtime: &mut DesignerRuntime, view: TableView) -> Vec<String> {
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
        DesignerResponse::TableWindow(window) => {
            window.rows.into_iter().map(|row| row.id).collect()
        }
        other => panic!("expected a table window, got {other:?}"),
    }
}

fn located(
    runtime: &mut DesignerRuntime,
    view: TableView,
    entity: &str,
) -> LocatedInViewProjection {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime
        .handle(DesignerRequest::LocateInView {
            expected_revision,
            collection: "rows".to_owned(),
            view,
            entity: entity.to_owned(),
        })
        .expect("the view is queryable")
    {
        DesignerResponse::LocatedInView(located) => located,
        other => panic!("expected a located-in-view projection, got {other:?}"),
    }
}

fn ids(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn natural_orders_text_case_insensitively_with_digit_runs_as_numbers() {
    let mut runtime = natural_runtime();
    assert_eq!(
        order(
            &mut runtime,
            sorted("f01_name", SortDirection::Asc, ViewTextOrder::Natural)
        ),
        // `apple`, `Banana`, `cherry` ignore case; `Item 2` precedes `Item 10`;
        // the three `Item 2` rows keep ascending entity-ID order.
        ids(&["e03", "e04", "e05", "e02", "e07", "e08", "e01", "e06"])
    );
}

#[test]
fn natural_descending_reverses_and_blanks_stay_last() {
    let mut runtime = natural_runtime();
    assert_eq!(
        order(
            &mut runtime,
            sorted("f01_name", SortDirection::Desc, ViewTextOrder::Natural)
        ),
        // Ties keep ascending entity-ID order in both directions, and the blank
        // name (e06) sorts last either way.
        ids(&["e01", "e02", "e07", "e08", "e05", "e04", "e03", "e06"])
    );
}

#[test]
fn default_and_explicit_bytes_keep_the_byte_order() {
    let mut runtime = natural_runtime();
    let bytes = ids(&["e04", "e01", "e02", "e07", "e08", "e03", "e05", "e06"]);
    assert_eq!(
        order(
            &mut runtime,
            sorted("f01_name", SortDirection::Asc, ViewTextOrder::Bytes)
        ),
        bytes
    );
    assert_eq!(
        order(
            &mut runtime,
            sorted("f01_name", SortDirection::Asc, ViewTextOrder::default())
        ),
        bytes
    );

    // A view written before `text_order` existed still decodes as `bytes` and
    // keeps its old serialized spelling, so cache keys do not change.
    let implicit: ViewSortKey = serde_json::from_str(r#"{"field":"f01_name","direction":"asc"}"#)
        .expect("a view from before the field still decodes");
    assert_eq!(implicit.text_order, ViewTextOrder::Bytes);
    assert_eq!(
        serde_json::to_string(&implicit).expect("the key encodes"),
        r#"{"field":"f01_name","direction":"asc"}"#
    );
}

#[test]
fn window_locate_and_two_keys_agree_on_natural_order() {
    let mut runtime = natural_runtime();
    let view = TableView {
        sort: vec![
            ViewSortKey {
                field: "f01_name".to_owned(),
                direction: SortDirection::Asc,
                text_order: ViewTextOrder::Natural,
            },
            ViewSortKey {
                field: "f02_amount".to_owned(),
                direction: SortDirection::Asc,
                text_order: ViewTextOrder::Bytes,
            },
        ],
        ..TableView::default()
    };
    let expected = ids(&["e03", "e04", "e05", "e08", "e07", "e02", "e01", "e06"]);
    assert_eq!(order(&mut runtime, view.clone()), expected);

    for (entity, index) in [("e03", 0), ("e08", 3), ("e01", 6), ("e06", 7)] {
        let found = located(&mut runtime, view.clone(), entity);
        assert_eq!(
            found.result,
            LocateInViewResult::Found,
            "{entity} is visible"
        );
        assert_eq!(found.index, Some(index), "{entity} sits at {index}");
    }
}

#[test]
fn the_cached_view_index_keeps_bytes_and_natural_apart() {
    let mut runtime = natural_runtime();
    let bytes = order(
        &mut runtime,
        sorted("f01_name", SortDirection::Asc, ViewTextOrder::Bytes),
    );
    let natural = order(
        &mut runtime,
        sorted("f01_name", SortDirection::Asc, ViewTextOrder::Natural),
    );
    assert_eq!(
        bytes,
        ids(&["e04", "e01", "e02", "e07", "e08", "e03", "e05", "e06"])
    );
    assert_eq!(
        natural,
        ids(&["e03", "e04", "e05", "e02", "e07", "e08", "e01", "e06"])
    );
}
