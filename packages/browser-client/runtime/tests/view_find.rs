//! #686 PR 3: Find across a view.
//!
//! A cell matches when its display text, lowercased, contains the query
//! lowercased, with neither side trimmed; matches run row by row and then in
//! the given column order, wrapping at either end. The reply carries the
//! total match count and the found cell's view row and ordinal.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime, FindDirection,
    FindInViewProjection, FindPosition, FoundCell, SortDirection, TableView, ViewFilter,
    ViewSortKey, ViewTextOrder,
};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldType, Number, NumberFormat, NumberFormatKind, Schema, SchemaId,
    SchemaKey, Value,
};

const MIXED: &str = "00000000-0000-4000-8000-000000000c01";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn date(value: &str) -> Value {
    Value::Date(Date::parse(value).expect("fixture dates are canonical"))
}

fn usd() -> NumberFormat {
    NumberFormat {
        decimals: 2,
        kind: NumberFormatKind::Currency {
            code: "USD".to_owned(),
        },
        thousands_separator: true,
    }
}

fn field(
    id: &str,
    field_type: FieldType,
    number_format: Option<NumberFormat>,
) -> (FieldId, FieldDefinition) {
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
            number_format,
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

/// The formula doubles its same-row price, so a calculated Number is searched
/// by the text of its formatted result.
fn double_price() -> Value {
    Value::Formula(Expression::Multiply {
        left: Box::new(Expression::SameRowReference(FieldId::from("f02_price"))),
        right: Box::new(Expression::Number(Number::new(2.0).expect("2 is finite"))),
    })
}

fn find_document() -> Document {
    let mut document = Document::empty("find_seed", "Find seed");
    let schema = SchemaId::from("rows");
    document.schemas.insert(
        schema.clone(),
        Schema {
            field_order: None,
            id: schema.clone(),
            key: SchemaKey::from("rows"),
            display_name: None,
            fields: vec![
                field("f01_name", FieldType::Text, None),
                field("f02_price", FieldType::Number, Some(usd())),
                field("f03_due", FieldType::Date, None),
                field("f04_done", FieldType::Boolean, None),
                field("f05_double", FieldType::Number, Some(usd())),
                field(
                    "f06_link",
                    FieldType::Reference {
                        schema: SchemaId::from("rows"),
                    },
                    None,
                ),
            ]
            .into_iter()
            .collect(),
        },
    );
    document.entities = vec![
        entity(
            "e01",
            vec![
                ("f01_name", text("Item 10")),
                ("f02_price", number(1234.5)),
                ("f03_due", date("2026-11-05")),
                ("f04_done", Value::Boolean(true)),
                ("f05_double", double_price()),
                ("f06_link", Value::Reference(EntityId::from("e02"))),
            ],
        ),
        entity(
            "e02",
            vec![
                ("f01_name", text("Item 2")),
                ("f02_price", number(10.0)),
                ("f03_due", date("2026-01-02")),
                ("f04_done", Value::Boolean(false)),
                ("f05_double", double_price()),
            ],
        ),
        // e03 has an empty optional Text (shown as two quotation marks), a
        // present Reference and no price, date or formula.
        entity(
            "e03",
            vec![
                ("f01_name", text("")),
                ("f06_link", Value::Reference(EntityId::from("e01"))),
            ],
        ),
        entity(
            "e04",
            vec![
                ("f01_name", text("Widget")),
                ("f02_price", number(1234.501)),
                ("f03_due", date("2026-12-31")),
                ("f04_done", Value::Boolean(true)),
            ],
        ),
    ]
    .into_iter()
    .collect();
    document
}

fn find_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(find_document(), MIXED)
        .expect("the find fixture is admitted by the ordinary profile")
}

fn find(
    runtime: &mut DesignerRuntime,
    view: TableView,
    query: &str,
    fields: &[&str],
    from: Option<FindPosition>,
    direction: FindDirection,
) -> Result<FindInViewProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::FindInView {
        expected_revision,
        collection: "rows".to_owned(),
        view,
        query: query.to_owned(),
        fields: fields.iter().map(|field| (*field).to_owned()).collect(),
        from,
        direction,
    })? {
        DesignerResponse::FoundInView(projection) => Ok(projection),
        other => panic!("expected a find reply, got {other:?}"),
    }
}

fn found(entity: &str, field: &str, row: usize, ordinal: usize) -> FoundCell {
    FoundCell {
        entity: entity.to_owned(),
        field: field.to_owned(),
        row,
        // This fixture numbers its entity ids by stored position: e01 is 1.
        position: entity[1..]
            .parse()
            .expect("fixture entity ids are numbered"),
        ordinal,
    }
}

fn position(row: usize, field: &str) -> FindPosition {
    FindPosition {
        row,
        field: field.to_owned(),
    }
}

fn ids(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|id| (*id).to_owned()).collect()
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

#[test]
fn formatted_number_date_and_text_match_their_display_text() {
    let mut runtime = find_runtime();
    let price = find(
        &mut runtime,
        TableView::default(),
        "1,234",
        &["f02_price"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(price.total, 2);
    assert_eq!(price.found, Some(found("e01", "f02_price", 0, 0)));

    let due = find(
        &mut runtime,
        TableView::default(),
        "2026-",
        &["f03_due"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(due.total, 3);
    assert_eq!(due.found, Some(found("e01", "f03_due", 0, 0)));

    let name = find(
        &mut runtime,
        TableView::default(),
        "iTeM",
        &["f01_name"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(name.total, 2);
    assert_eq!(name.found, Some(found("e01", "f01_name", 0, 0)));
}

#[test]
fn matches_run_row_then_field_and_steps_wrap() {
    let mut runtime = find_runtime();
    let next = find(
        &mut runtime,
        TableView::default(),
        "1",
        &["f01_name", "f02_price"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(next.total, 4);
    assert_eq!(next.found, Some(found("e01", "f01_name", 0, 0)));

    let last = find(
        &mut runtime,
        TableView::default(),
        "1",
        &["f01_name", "f02_price"],
        None,
        FindDirection::Previous,
    )
    .expect("the query is valid");
    assert_eq!(last.found, Some(found("e04", "f02_price", 3, 3)));

    let wrapped = find(
        &mut runtime,
        TableView::default(),
        "1",
        &["f01_name", "f02_price"],
        Some(position(3, "f02_price")),
        FindDirection::Next,
    )
    .expect("the position is valid");
    assert_eq!(wrapped.found, Some(found("e01", "f01_name", 0, 0)));

    let back_wrapped = find(
        &mut runtime,
        TableView::default(),
        "1",
        &["f01_name", "f02_price"],
        Some(position(0, "f01_name")),
        FindDirection::Previous,
    )
    .expect("the position is valid");
    assert_eq!(back_wrapped.found, Some(found("e04", "f02_price", 3, 3)));

    // A stale position between two matches steps to the following one.
    let stale_next = find(
        &mut runtime,
        TableView::default(),
        "1",
        &["f01_name", "f02_price"],
        Some(position(2, "f01_name")),
        FindDirection::Next,
    )
    .expect("the position is valid");
    assert_eq!(stale_next.found, Some(found("e04", "f02_price", 3, 3)));

    let stale_previous = find(
        &mut runtime,
        TableView::default(),
        "1",
        &["f01_name", "f02_price"],
        Some(position(2, "f01_name")),
        FindDirection::Previous,
    )
    .expect("the position is valid");
    assert_eq!(stale_previous.found, Some(found("e02", "f02_price", 1, 2)));
}

#[test]
fn sorted_and_filtered_views_are_walked_in_view_order() {
    let mut runtime = find_runtime();
    let sorted = TableView {
        sort: vec![ViewSortKey {
            field: "f02_price".to_owned(),
            direction: SortDirection::Asc,
            text_order: ViewTextOrder::Bytes,
        }],
        ..TableView::default()
    };
    assert_eq!(
        order(&mut runtime, sorted.clone()),
        ids(&["e02", "e01", "e04", "e03"])
    );
    let result = find(
        &mut runtime,
        sorted,
        "1,234",
        &["f02_price"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(result.total, 2);
    assert_eq!(result.found, Some(found("e01", "f02_price", 1, 0)));

    let top = find(
        &mut runtime,
        TableView {
            sort: vec![ViewSortKey {
                field: "f02_price".to_owned(),
                direction: SortDirection::Asc,
                text_order: ViewTextOrder::Bytes,
            }],
            ..TableView::default()
        },
        "1,234",
        &["f02_price"],
        Some(position(1, "f02_price")),
        FindDirection::Next,
    )
    .expect("the position is valid");
    assert_eq!(top.found, Some(found("e04", "f02_price", 2, 1)));

    let filtered = TableView {
        filters: vec![ViewFilter::DisplayIn {
            field: "f02_price".to_owned(),
            values: vec!["$1,234.50".to_owned()],
        }],
        ..TableView::default()
    };
    assert_eq!(order(&mut runtime, filtered.clone()), ids(&["e01", "e04"]));
    let result = find(
        &mut runtime,
        filtered,
        "1,234",
        &["f02_price"],
        None,
        FindDirection::Previous,
    )
    .expect("the query is valid");
    assert_eq!(result.total, 2);
    assert_eq!(result.found, Some(found("e04", "f02_price", 1, 1)));
}

#[test]
fn only_listed_fields_are_searched() {
    let mut runtime = find_runtime();
    let result = find(
        &mut runtime,
        TableView::default(),
        "1,234",
        &["f01_name"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(result.total, 0);
    assert_eq!(result.found, None);

    let result = find(
        &mut runtime,
        TableView::default(),
        "item",
        &["f02_price"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(result.total, 0);
    assert_eq!(result.found, None);
}

#[test]
fn blank_reference_and_blank_cells_match_nothing() {
    let mut runtime = find_runtime();
    for query in ["", "   "] {
        let result = find(
            &mut runtime,
            TableView::default(),
            query,
            &["f01_name", "f02_price", "f03_due"],
            None,
            FindDirection::Next,
        )
        .expect("a blank query is a valid request");
        assert_eq!(result.total, 0);
        assert_eq!(result.found, None);
    }

    // A Reference cell shows no text.
    let reference = find(
        &mut runtime,
        TableView::default(),
        "e0",
        &["f06_link"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(reference.total, 0);
    assert_eq!(reference.found, None);

    // The empty optional Text shows as two quotation marks, so it matches a
    // quote while a blank cell does not.
    let quote = find(
        &mut runtime,
        TableView::default(),
        "\"",
        &["f01_name", "f03_due"],
        None,
        FindDirection::Next,
    )
    .expect("the query is valid");
    assert_eq!(quote.total, 1);
    assert_eq!(quote.found, Some(found("e03", "f01_name", 2, 0)));
}

#[test]
fn invalid_requests_refuse() {
    let mut runtime = find_runtime();
    for fields in [
        Vec::new(),
        vec!["f01_name", "f01_name"],
        vec!["f09_missing"],
    ] {
        let error = find(
            &mut runtime,
            TableView::default(),
            "item",
            &fields,
            None,
            FindDirection::Next,
        )
        .expect_err("the fields are invalid");
        assert!(matches!(error, DesignerError::InvalidView { .. }));
    }

    let past_end = find(
        &mut runtime,
        TableView::default(),
        "item",
        &["f01_name"],
        Some(position(4, "f01_name")),
        FindDirection::Next,
    )
    .expect_err("the row is past the view");
    assert!(matches!(past_end, DesignerError::InvalidView { .. }));

    let unsearched = find(
        &mut runtime,
        TableView::default(),
        "item",
        &["f01_name"],
        Some(position(0, "f02_price")),
        FindDirection::Next,
    )
    .expect_err("the field is not searched");
    assert!(matches!(unsearched, DesignerError::InvalidView { .. }));

    let stale = runtime
        .handle(DesignerRequest::FindInView {
            expected_revision: "stale".to_owned(),
            collection: "rows".to_owned(),
            view: TableView::default(),
            query: "item".to_owned(),
            fields: vec!["f01_name".to_owned()],
            from: None,
            direction: FindDirection::Next,
        })
        .expect_err("the revision is stale");
    assert!(matches!(stale, DesignerError::StaleQuery { .. }));
}
