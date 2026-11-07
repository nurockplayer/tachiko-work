//! #686 PR 5: totals over a view (`queryViewTotals`).
//!
//! A filtered footer must total the rows a view shows without naming every
//! entity id, because 10,000 ids exceed the 64 KiB request limit. The view
//! request sums the same rows in collection order, so a sort never changes a
//! float sum that a different order would round differently.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    ColumnTotalsProjection, ComparisonOperator, DesignerError, DesignerRequest, DesignerResponse,
    DesignerRuntime, SortDirection, TableView, ViewFilter, ViewSortKey, ViewTextOrder,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const TOTALS: &str = "00000000-0000-4000-8000-000000000c05";

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

/// `e01 = 1e16`, `e02 = -1e16` and `e03 = 1.0` in collection order sum to
/// 1.0; summing them in the view's ascending order gives 0 instead, so a
/// totals reply that forgot to restore collection order would fail here.
fn totals_document() -> Document {
    let mut document = Document::empty("totals_seed", "Totals seed");
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
            vec![("f01_name", text("Alpha")), ("f02_amount", number(1e16))],
        ),
        entity(
            "e02",
            vec![("f01_name", text("Beta")), ("f02_amount", number(-1e16))],
        ),
        entity(
            "e03",
            vec![("f01_name", text("Gamma")), ("f02_amount", number(1.0))],
        ),
        entity(
            "e04",
            vec![("f01_name", text("Delta")), ("f02_amount", number(2.5))],
        ),
        // e05 has no amount, so it contributes to `rows` but not to the sum.
        entity("e05", vec![("f01_name", text("Epsilon"))]),
        entity(
            "e06",
            vec![("f01_name", text("Zeta")), ("f02_amount", number(7.0))],
        ),
    ]
    .into_iter()
    .collect();
    document
}

fn totals_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(totals_document(), TOTALS)
        .expect("the totals fixture is admitted by the ordinary profile")
}

fn filtered(filter: ViewFilter) -> TableView {
    TableView {
        filters: vec![filter],
        ..TableView::default()
    }
}

fn view_totals(
    runtime: &mut DesignerRuntime,
    view: TableView,
) -> Result<ColumnTotalsProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::QueryViewTotals {
        expected_revision,
        collection: "rows".to_owned(),
        view,
    })? {
        DesignerResponse::ColumnTotals(projection) => Ok(projection),
        other => panic!("expected column totals, got {other:?}"),
    }
}

fn named_totals(
    runtime: &mut DesignerRuntime,
    entities: &[&str],
) -> Result<ColumnTotalsProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::QueryColumnTotals {
        expected_revision,
        collection: "rows".to_owned(),
        entities: entities.iter().map(|id| (*id).to_owned()).collect(),
    })? {
        DesignerResponse::ColumnTotals(projection) => Ok(projection),
        other => panic!("expected column totals, got {other:?}"),
    }
}

fn amount_bits(projection: &ColumnTotalsProjection) -> u64 {
    projection
        .column_totals
        .get("f02_amount")
        .copied()
        .expect("the amount column has a totals entry")
        .expect("the fixture amount sum is finite")
        .to_bits()
}

#[test]
fn a_filtered_views_totals_match_naming_the_same_rows() {
    let mut runtime = totals_runtime();
    let view = filtered(ViewFilter::NumberCompare {
        field: "f02_amount".to_owned(),
        op: ComparisonOperator::Gte,
        value: 0.0,
    });
    let shown = view_totals(&mut runtime, view).expect("the filtered view is valid");
    // The named request is deliberately out of collection order; both replies
    // must sum in collection order anyway.
    let named = named_totals(&mut runtime, &["e04", "e01", "e06", "e03"])
        .expect("the same rows are valid members");
    assert_eq!(shown.rows, 4);
    assert_eq!(named.rows, shown.rows);
    assert_eq!(shown.revision, named.revision);
    assert_eq!(amount_bits(&shown), amount_bits(&named));
}

#[test]
fn a_sort_does_not_change_the_totals() {
    let mut runtime = totals_runtime();
    let view = TableView {
        sort: vec![ViewSortKey {
            field: "f02_amount".to_owned(),
            direction: SortDirection::Asc,
            text_order: ViewTextOrder::Bytes,
        }],
        filters: vec![ViewFilter::NumberCompare {
            field: "f02_amount".to_owned(),
            op: ComparisonOperator::Gte,
            value: -1e16,
        }],
        ..TableView::default()
    };
    let sorted = view_totals(&mut runtime, view).expect("the sorted view is valid");
    let named = named_totals(&mut runtime, &["e01", "e02", "e03", "e04", "e06"])
        .expect("the same rows are valid members");
    assert_eq!(sorted.rows, 5);
    assert_eq!(amount_bits(&sorted), amount_bits(&named));
    // Summing the view's own ascending order would give 0.0; collection order
    // gives 10.5 exactly.
    assert_eq!(amount_bits(&sorted), 10.5_f64.to_bits());
}

#[test]
fn an_empty_view_totals_the_same_as_no_entities() {
    let mut runtime = totals_runtime();
    let view = filtered(ViewFilter::NumberCompare {
        field: "f02_amount".to_owned(),
        op: ComparisonOperator::Gte,
        value: 1e300,
    });
    let empty = view_totals(&mut runtime, view).expect("the empty view is valid");
    let none = named_totals(&mut runtime, &[]).expect("an empty set is valid");
    assert_eq!(empty.rows, 0);
    assert_eq!(none.rows, empty.rows);
    assert_eq!(amount_bits(&empty), amount_bits(&none));
}

#[test]
fn kept_entities_count_even_when_a_filter_excludes_them() {
    let mut runtime = totals_runtime();
    let view = TableView {
        filters: vec![ViewFilter::TextContains {
            field: "f01_name".to_owned(),
            value: "alpha".to_owned(),
        }],
        keep_entities: vec!["e05".to_owned()],
        ..TableView::default()
    };
    let kept = view_totals(&mut runtime, view).expect("the kept view is valid");
    let named = named_totals(&mut runtime, &["e01", "e05"]).expect("both rows are valid members");
    assert_eq!(kept.rows, 2);
    assert_eq!(kept.rows, named.rows);
    assert_eq!(amount_bits(&kept), amount_bits(&named));
}

#[test]
fn a_stale_revision_refuses() {
    let mut runtime = totals_runtime();
    let stale = runtime.handle(DesignerRequest::QueryViewTotals {
        expected_revision: "00000000-0000-4000-8000-000000000000".to_owned(),
        collection: "rows".to_owned(),
        view: TableView::default(),
    });
    assert!(matches!(stale, Err(DesignerError::StaleQuery { .. })));
}

#[test]
fn an_unknown_view_column_refuses_like_the_window() {
    let mut runtime = totals_runtime();
    let view = filtered(ViewFilter::TextContains {
        field: "f99_missing".to_owned(),
        value: "anything".to_owned(),
    });
    let totals = view_totals(&mut runtime, view.clone());
    assert!(matches!(
        totals,
        Err(DesignerError::UnknownViewField { .. })
    ));
    let expected_revision = runtime.observe_occurrence().revision;
    let window = runtime.handle(DesignerRequest::QueryTableWindow {
        expected_revision,
        collection: "rows".to_owned(),
        view,
        start: 0,
        count: 0,
        display: false,
    });
    assert!(matches!(
        window,
        Err(DesignerError::UnknownViewField { .. })
    ));
}
