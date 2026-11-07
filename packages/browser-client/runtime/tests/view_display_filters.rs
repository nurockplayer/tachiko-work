//! #686 PR 2: `display_contains`, `display_in` and distinct display values.
//!
//! Display filters compare the text the grid shows instead of the stored
//! scalar, so a filter over "$1,234.50" finds both 1234.5 and 1234.501, and a
//! Reference column answers without a refusal. Distinct values come back in
//! Sheet's list order: `""` first, then natural order, bounded by `limit` and
//! the window reply byte cap.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    ComparisonOperator, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    DistinctValuesProjection, ScalarLiteral, TableView, ViewFilter,
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

/// The formula doubles its same-row price, so a display filter over a
/// calculated Number proves the formatted result is what matches.
fn double_price() -> Value {
    Value::Formula(Expression::Multiply {
        left: Box::new(Expression::SameRowReference(FieldId::from("f02_price"))),
        right: Box::new(Expression::Number(Number::new(2.0).expect("2 is finite"))),
    })
}

fn mixed_document() -> Document {
    let mut document = Document::empty("display_seed", "Display seed");
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
        // 1234.501 rounds to the same "$1,234.50" text as 1234.5.
        entity(
            "e02",
            vec![
                ("f01_name", text("Item 2")),
                ("f02_price", number(1234.501)),
                ("f03_due", date("2026-01-02")),
                ("f04_done", Value::Boolean(false)),
                ("f05_double", double_price()),
            ],
        ),
        // e03 keeps a name only: price, date and formula are blank, and its
        // reference is present but has no display text.
        entity(
            "e03",
            vec![("f06_link", Value::Reference(EntityId::from("e01")))],
        ),
        entity(
            "e04",
            vec![
                ("f01_name", text("Item 2")),
                ("f02_price", number(10.0)),
                ("f03_due", date("2026-12-31")),
                ("f04_done", Value::Boolean(true)),
            ],
        ),
    ]
    .into_iter()
    .collect();
    document
}

fn mixed_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(mixed_document(), MIXED)
        .expect("the mixed fixture is admitted by the ordinary profile")
}

fn filtered(filter: ViewFilter) -> TableView {
    TableView {
        filters: vec![filter],
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

fn distinct(
    runtime: &mut DesignerRuntime,
    view: TableView,
    field: &str,
    limit: usize,
) -> Result<DistinctValuesProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::QueryDistinctValues {
        expected_revision,
        collection: "rows".to_owned(),
        view,
        field: field.to_owned(),
        limit,
    })? {
        DesignerResponse::DistinctValues(values) => Ok(values),
        other => panic!("expected distinct values, got {other:?}"),
    }
}

fn ids(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn display_in_matches_every_value_that_renders_the_same_text() {
    let mut runtime = mixed_runtime();
    let shown = order(
        &mut runtime,
        filtered(ViewFilter::DisplayIn {
            field: "f02_price".to_owned(),
            values: vec!["$1,234.50".to_owned()],
        }),
    );
    assert_eq!(shown, ids(&["e01", "e02"]));
}

#[test]
fn display_contains_matches_numbers_booleans_and_dates() {
    let mut runtime = mixed_runtime();
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayContains {
                field: "f02_price".to_owned(),
                value: "1,234".to_owned(),
            })
        ),
        ids(&["e01", "e02"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayContains {
                field: "f04_done".to_owned(),
                value: "TRUE".to_owned(),
            })
        ),
        ids(&["e01", "e04"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayContains {
                field: "f03_due".to_owned(),
                value: "2026-11".to_owned(),
            })
        ),
        ids(&["e01"])
    );
}

#[test]
fn display_in_empty_string_finds_blank_cells() {
    let mut runtime = mixed_runtime();
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayIn {
                field: "f02_price".to_owned(),
                values: vec![String::new()],
            })
        ),
        ids(&["e03"])
    );
}

#[test]
fn display_contains_on_a_reference_column_matches_nothing_and_does_not_refuse() {
    let mut runtime = mixed_runtime();
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayContains {
                field: "f06_link".to_owned(),
                value: "e02".to_owned(),
            })
        ),
        Vec::<String>::new()
    );
    // A present reference has no display text either, so `""` matches every
    // row of this column, blank and reference alike.
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayIn {
                field: "f06_link".to_owned(),
                values: vec![String::new()],
            })
        ),
        ids(&["e01", "e02", "e03", "e04"])
    );
}

#[test]
fn a_formula_column_filters_by_its_formatted_result() {
    let mut runtime = mixed_runtime();
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::DisplayIn {
                field: "f05_double".to_owned(),
                values: vec!["$2,469.00".to_owned()],
            })
        ),
        ids(&["e01", "e02"]),
        "both calculated prices render as $2,469.00"
    );
}

#[test]
fn distinct_values_use_sheet_list_order() {
    let mut runtime = mixed_runtime();
    let values = distinct(&mut runtime, TableView::default(), "f01_name", 10)
        .expect("the name column is queryable");
    assert_eq!(values.field, "f01_name");
    assert_eq!(
        values.values,
        vec![String::new(), "Item 2".to_owned(), "Item 10".to_owned()]
    );
    assert!(!values.truncated);
}

#[test]
fn distinct_values_respect_the_views_other_filters() {
    let mut runtime = mixed_runtime();
    let view = filtered(ViewFilter::NumberCompare {
        field: "f02_price".to_owned(),
        op: ComparisonOperator::Gte,
        value: 100.0,
    });
    let values = distinct(&mut runtime, view, "f01_name", 10).expect("the filtered view is valid");
    assert_eq!(
        values.values,
        vec!["Item 2".to_owned(), "Item 10".to_owned()]
    );
}

#[test]
fn distinct_values_truncate_at_the_limit_and_flag_it() {
    let mut runtime = mixed_runtime();
    let values =
        distinct(&mut runtime, TableView::default(), "f01_name", 2).expect("limit 2 is valid");
    assert_eq!(values.values, vec![String::new(), "Item 2".to_owned()]);
    assert!(values.truncated);

    let all =
        distinct(&mut runtime, TableView::default(), "f01_name", 4).expect("limit 4 is valid");
    assert_eq!(all.values.len(), 3);
    assert!(!all.truncated);
}

#[test]
fn distinct_values_refuse_a_limit_outside_its_bounds() {
    let mut runtime = mixed_runtime();
    for limit in [0, 1_001] {
        assert!(
            matches!(
                distinct(&mut runtime, TableView::default(), "f01_name", limit),
                Err(DesignerError::InvalidView { .. })
            ),
            "limit {limit} is refused"
        );
    }
}

#[test]
fn the_existing_text_filters_keep_their_behaviour() {
    let mut runtime = mixed_runtime();
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::TextContains {
                field: "f01_name".to_owned(),
                value: "item".to_owned(),
            })
        ),
        ids(&["e01", "e02", "e04"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::InSet {
                field: "f01_name".to_owned(),
                values: vec![ScalarLiteral::Text("Item 2".to_owned())],
            })
        ),
        ids(&["e02", "e04"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::InSet {
                field: "f02_price".to_owned(),
                values: vec![ScalarLiteral::Number(1234.5)],
            })
        ),
        ids(&["e01"])
    );
}

#[test]
fn present_empty_text_stays_apart_from_a_blank_cell() {
    let mut document = mixed_document();
    let (id, row) = entity("z_empty", vec![("f01_name", text(""))]);
    document.entities.insert(id, row);
    let mut runtime = DesignerRuntime::from_document(document, MIXED).expect("fixture admits");
    let quotes = "\"\"".to_owned();
    let marked = filtered(ViewFilter::DisplayIn {
        field: "f01_name".to_owned(),
        values: vec![quotes.clone()],
    });
    assert_eq!(order(&mut runtime, marked), ids(&["z_empty"]));
    let blank = filtered(ViewFilter::DisplayIn {
        field: "f01_name".to_owned(),
        values: vec![String::new()],
    });
    assert!(!order(&mut runtime, blank).contains(&"z_empty".to_owned()));
    let values = distinct(&mut runtime, TableView::default(), "f01_name", 10)
        .expect("the distinct query is bounded");
    assert_eq!(values.values[0], "");
    assert!(values.values.contains(&quotes));
}
