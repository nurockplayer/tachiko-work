//! Capacity Stage A (A-03): `query_table_window`, `locate_in_view` and the
//! 1,000-row `queryTable` refusal.
//!
//! The view is request state: sorting, filtering and kept entities never enter
//! the document, the history or the revision token. These tests pin the
//! stateless order (blanks last, entity-id tie-break), every filter kind, the
//! cache invalidation on publication, the reply byte cap, and the typed
//! `use_window_query` refusal above 1,000 rows.
#![allow(clippy::too_many_lines)] // One fixture and its independent oracle stay together.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    ComparisonOperator, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    ColumnTotalsProjection, LocateInViewResult, LocatedInViewProjection, ScalarEditInput,
    ScalarLiteral, SortDirection, TableView, TableWindowProjection, ViewFilter, ViewSortKey,
    process_wire_request,
};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition,
    FieldId, FieldKey, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const SMALL: &str = "00000000-0000-4000-8000-000000000b01";
const THOUSAND: &str = "00000000-0000-4000-8000-000000000b02";
const OVER: &str = "00000000-0000-4000-8000-000000000b03";
const WIDE: &str = "00000000-0000-4000-8000-000000000b04";
const WIDENED_SCHEMA: &str = "00000000-0000-4000-8000-000000000b05";
const REFERENCE: &str = "00000000-0000-4000-8000-000000000b06";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn date(value: &str) -> Value {
    Value::Date(Date::parse(value).expect("fixture dates are canonical"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

/// `f06_double` is a formula column so sorting and filtering can be pinned to
/// the calculated Number rather than to the stored expression. The same-row
/// reference keeps the formula valid for every fixture entity.
fn double_reference() -> Value {
    Value::Formula(Expression::Multiply {
        left: Box::new(Expression::SameRowReference(FieldId::from("f03_amount"))),
        right: Box::new(Expression::Number(Number::new(2.0).expect("2 is finite"))),
    })
}

fn field(id: &str, field_type: FieldType, required: bool) -> (FieldId, FieldDefinition) {
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
            required,
            constraint: FieldConstraint::None,
            number_format: None,
        },
    )
}

fn entity_of(id: &str, fields: BTreeMap<FieldId, Value>) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from("rows"),
        fields,
    }
}

fn entity(id: &str, values: Vec<(&str, Value)>) -> (EntityId, Entity) {
    (
        EntityId::from(id),
        entity_of(
            id,
            values
                .into_iter()
                .map(|(field, value)| (FieldId::from(field), value))
                .collect(),
        ),
    )
}

fn schema_document(
    fields: Vec<(FieldId, FieldDefinition)>,
    entities: Vec<(EntityId, Entity)>,
) -> Document {
    let mut document = Document::empty("window_seed", "Window seed");
    let schema = SchemaId::from("rows");
    document.schemas.insert(
        schema.clone(),
        Schema {
            field_order: None,
            id: schema.clone(),
            key: SchemaKey::from("rows"),
            display_name: None,
            fields: fields.into_iter().collect(),
        },
    );
    document.entities = entities.into_iter().collect();
    document
}

fn small_fields() -> Vec<(FieldId, FieldDefinition)> {
    vec![
        field("f01_name", FieldType::Text, false),
        field("f02_group", FieldType::Text, false),
        field("f03_amount", FieldType::Number, false),
        field("f04_due", FieldType::Date, false),
        field("f05_done", FieldType::Boolean, false),
        field("f06_double", FieldType::Number, false),
    ]
}

/// Twelve rows with intentional blanks (e04, e12), tied values (amount 5, 7,
/// 10 and -2), and a formula column. Entity ids sort in fixture order so the
/// tie-break is directly visible in the expected orders.
fn small_document() -> Document {
    let entities = vec![
        entity(
            "e01",
            vec![
                ("f01_name", text("Alpha")),
                ("f02_group", text("x")),
                ("f03_amount", number(10.0)),
                ("f04_due", date("2026-01-01")),
                ("f05_done", Value::Boolean(true)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e02",
            vec![
                ("f01_name", text("beta")),
                ("f02_group", text("y")),
                ("f03_amount", number(5.0)),
                ("f04_due", date("2026-03-01")),
                ("f05_done", Value::Boolean(false)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e03",
            vec![
                ("f01_name", text("Gamma")),
                ("f02_group", text("x")),
                ("f03_amount", number(-2.0)),
                ("f04_due", date("2026-02-01")),
                ("f05_done", Value::Boolean(true)),
                ("f06_double", double_reference()),
            ],
        ),
        // e04 keeps only its name: amount, formula, group, due and done are
        // all blank, so every blank rule has one row to act on.
        entity("e04", vec![("f01_name", text("delta"))]),
        entity(
            "e05",
            vec![
                ("f01_name", text("Epsilon")),
                ("f02_group", text("y")),
                ("f03_amount", number(10.0)),
                ("f04_due", date("2025-12-31")),
                ("f05_done", Value::Boolean(false)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e06",
            vec![
                ("f01_name", text("zeta")),
                ("f02_group", text("x")),
                ("f03_amount", number(100.0)),
                ("f04_due", date("2026-01-01")),
                ("f05_done", Value::Boolean(true)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e07",
            vec![
                ("f01_name", text("eta")),
                ("f02_group", text("y")),
                ("f03_amount", number(7.0)),
                ("f04_due", date("2026-02-15")),
                ("f05_done", Value::Boolean(false)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e08",
            vec![
                ("f01_name", text("Theta")),
                ("f02_group", text("x")),
                ("f03_amount", number(5.0)),
                ("f04_due", date("2026-03-01")),
                ("f05_done", Value::Boolean(true)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e09",
            vec![
                ("f01_name", text("iota")),
                ("f02_group", text("y")),
                ("f03_amount", number(-2.0)),
                ("f04_due", date("2026-02-01")),
                ("f05_done", Value::Boolean(false)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e10",
            vec![
                ("f01_name", text("Kappa")),
                ("f02_group", text("x")),
                ("f03_amount", number(7.0)),
                ("f04_due", date("2026-02-15")),
                ("f05_done", Value::Boolean(true)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e11",
            vec![
                ("f01_name", text("lambda")),
                ("f02_group", text("y")),
                ("f03_amount", number(0.0)),
                ("f04_due", date("2026-01-01")),
                ("f05_done", Value::Boolean(false)),
                ("f06_double", double_reference()),
            ],
        ),
        entity(
            "e12",
            vec![
                ("f01_name", text("Mu")),
                ("f02_group", text("x")),
                ("f04_due", date("2026-03-01")),
                ("f05_done", Value::Boolean(true)),
            ],
        ),
    ];
    schema_document(small_fields(), entities)
}

fn small_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(small_document(), SMALL)
        .expect("the mixed fixture is admitted by the ordinary profile")
}

/// Three rows over a Text label and a self-referencing column, so reference
/// presence, reference value filters and reference sorting are pinned
/// independently of the scalar fixture.
fn reference_document() -> Document {
    schema_document(
        vec![
            field("f01_label", FieldType::Text, false),
            field(
                "f02_link",
                FieldType::Reference {
                    schema: SchemaId::from("rows"),
                },
                false,
            ),
        ],
        vec![
            entity(
                "e01",
                vec![
                    ("f01_label", text("alpha")),
                    ("f02_link", Value::Reference(EntityId::from("e02"))),
                ],
            ),
            entity("e02", vec![("f01_label", text("beta"))]),
            entity(
                "e03",
                vec![
                    ("f01_label", text("gamma")),
                    ("f02_link", Value::Reference(EntityId::from("e01"))),
                ],
            ),
        ],
    )
}

/// One Text/Number rectangle of the given size, the text-capacity shape the
/// window path must stay correct at. Every row repeats the same Text value so
/// a wide fixture can reach the reply byte cap with legal cell text.
fn grid_document(rows: usize, width: usize, text_length: usize) -> Document {
    let keys = (0..width)
        .map(|column| format!("c{column:02}"))
        .collect::<Vec<_>>();
    let fields = keys
        .iter()
        .map(|key| {
            let id = FieldId::from(key.as_str());
            (
                id.clone(),
                FieldDefinition {
                    id,
                    key: FieldKey::from(key.as_str()),
                    display_name: None,
                    field_type: FieldType::Text,
                    required: true,
                    constraint: FieldConstraint::None,
                    number_format: None,
                },
            )
        })
        .collect();
    let entities = (0..rows)
        .map(|row| {
            let id = format!("r{row:05}");
            let value = format!("{row:05}_{}", "x".repeat(text_length));
            let values = keys
                .iter()
                .map(|key| (FieldId::from(key.as_str()), Value::Text(value.clone())))
                .collect::<BTreeMap<_, _>>();
            (EntityId::from(id.as_str()), entity_of(&id, values))
        })
        .collect();
    schema_document(fields, entities)
}

fn window(
    runtime: &mut DesignerRuntime,
    view: TableView,
    start: usize,
    count: usize,
) -> Result<TableWindowProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::QueryTableWindow {
        expected_revision,
        collection: "rows".to_owned(),
        view,
        start,
        count,
    })? {
        DesignerResponse::TableWindow(window) => Ok(window),
        other => panic!("expected a table window, got {other:?}"),
    }
}

fn located(
    runtime: &mut DesignerRuntime,
    view: TableView,
    entity: &str,
) -> Result<LocatedInViewProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::LocateInView {
        expected_revision,
        collection: "rows".to_owned(),
        view,
        entity: entity.to_owned(),
    })? {
        DesignerResponse::LocatedInView(located) => Ok(located),
        other => panic!("expected a located-in-view projection, got {other:?}"),
    }
}

fn ids(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|id| (*id).to_owned()).collect()
}

fn row_ids(window: &TableWindowProjection) -> Vec<String> {
    window.rows.iter().map(|row| row.id.clone()).collect()
}

fn sorted(field: &str, direction: SortDirection) -> TableView {
    TableView {
        sort: vec![ViewSortKey {
            field: field.to_owned(),
            direction,
        }],
        ..TableView::default()
    }
}

fn filtered(filter: ViewFilter) -> TableView {
    TableView {
        filters: vec![filter],
        ..TableView::default()
    }
}

fn order(runtime: &mut DesignerRuntime, view: TableView) -> Vec<String> {
    let window = window(runtime, view, 0, 256).expect("the view is queryable");
    row_ids(&window)
}

fn compare(field: &str, op: ComparisonOperator, value: f64) -> ViewFilter {
    ViewFilter::NumberCompare {
        field: field.to_owned(),
        op,
        value,
    }
}

fn date_compare(field: &str, op: ComparisonOperator, value: &str) -> ViewFilter {
    ViewFilter::DateCompare {
        field: field.to_owned(),
        op,
        value: value.to_owned(),
    }
}

#[test]
fn window_edges_clamp_and_reuse_the_complete_projection() {
    let mut runtime = small_runtime();
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "rows".to_owned(),
        })
        .expect("the small table is below the compatibility cap")
    else {
        panic!("expected a complete table");
    };

    let full = window(&mut runtime, TableView::default(), 0, 256).expect("the window is bounded");
    assert_eq!(full.total_rows, 12);
    assert_eq!(full.unfiltered_rows, 12);
    assert_eq!(full.start, 0);
    assert_eq!(full.columns, table.columns);
    assert_eq!(full.rows, table.rows);
    assert_eq!(full.collection.entity_count, 12);
    // The digest is stable for another identical query at the same revision.
    let repeated = window(&mut runtime, TableView::default(), 0, 256).expect("repeatable");
    assert_eq!(repeated.columns_revision, full.columns_revision);

    let totals = window(&mut runtime, TableView::default(), 4, 0).expect("count 0 is legal");
    assert_eq!(totals.total_rows, 12);
    assert_eq!(totals.start, 4);
    assert_eq!(totals.rows.len(), 0, "count 0 returns totals only");

    let past_end = window(&mut runtime, TableView::default(), 99, 256).expect("start clamps");
    assert_eq!(past_end.start, 12);
    assert_eq!(past_end.rows.len(), 0);

    let tail = window(&mut runtime, TableView::default(), 10, 256).expect("the tail is a window");
    assert_eq!(row_ids(&tail), ids(&["e11", "e12"]));

    let middle = window(&mut runtime, TableView::default(), 3, 4).expect("the middle is a window");
    assert_eq!(row_ids(&middle), ids(&["e04", "e05", "e06", "e07"]));
    assert_eq!(
        middle.total_rows, 12,
        "a window keeps the total, not its own count"
    );

    let too_many = window(&mut runtime, TableView::default(), 0, 257).unwrap_err();
    assert!(matches!(too_many, DesignerError::InvalidView { .. }));

    let too_many_kept = window(
        &mut runtime,
        TableView {
            keep_entities: (0..257).map(|index| format!("kept{index}")).collect(),
            ..TableView::default()
        },
        0,
        1,
    )
    .unwrap_err();
    assert!(matches!(too_many_kept, DesignerError::InvalidView { .. }));
}

#[test]
fn sorts_pin_blanks_last_ties_and_calculated_values() {
    let mut runtime = small_runtime();
    // -2, 0, 5, 7, 10, 100 with ties broken by ascending entity id and blanks
    // last in both directions.
    assert_eq!(
        order(&mut runtime, sorted("f03_amount", SortDirection::Asc)),
        ids(&[
            "e03", "e09", "e11", "e02", "e08", "e07", "e10", "e01", "e05", "e06", "e04", "e12",
        ])
    );
    assert_eq!(
        order(&mut runtime, sorted("f03_amount", SortDirection::Desc)),
        ids(&[
            "e06", "e01", "e05", "e07", "e10", "e02", "e08", "e11", "e03", "e09", "e04", "e12",
        ])
    );
    // Text order is the Unicode scalar order of the stored text.
    assert_eq!(
        order(&mut runtime, sorted("f01_name", SortDirection::Asc)),
        ids(&[
            "e01", "e05", "e03", "e10", "e12", "e08", "e02", "e04", "e07", "e09", "e11", "e06",
        ])
    );
    // A formula column sorts by its calculated Number; a failed calculation
    // (blank amount) is blank and sorts last.
    let by_amount = order(&mut runtime, sorted("f03_amount", SortDirection::Asc));
    assert_eq!(
        order(&mut runtime, sorted("f06_double", SortDirection::Asc)),
        by_amount
    );
    // Two keys: group first, then amount descending inside each group.
    let two_keys = TableView {
        sort: vec![
            ViewSortKey {
                field: "f02_group".to_owned(),
                direction: SortDirection::Asc,
            },
            ViewSortKey {
                field: "f03_amount".to_owned(),
                direction: SortDirection::Desc,
            },
        ],
        ..TableView::default()
    };
    assert_eq!(
        order(&mut runtime, two_keys),
        ids(&[
            "e06", "e01", "e10", "e08", "e03", "e12", "e05", "e07", "e02", "e11", "e09", "e04",
        ])
    );
}

#[test]
fn every_filter_kind_selects_the_documented_rows() {
    let mut runtime = small_runtime();
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::TextEquals {
                field: "f01_name".to_owned(),
                value: "BETA".to_owned(),
            })
        ),
        ids(&["e02"]),
        "text equals is case-insensitive"
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::TextContains {
                field: "f01_name".to_owned(),
                value: "TA".to_owned(),
            })
        ),
        ids(&["e02", "e04", "e06", "e07", "e08", "e09"]),
        "text contains is case-insensitive"
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f03_amount", ComparisonOperator::Eq, 10.0))
        ),
        ids(&["e01", "e05"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f03_amount", ComparisonOperator::Ne, 10.0))
        ),
        ids(&["e02", "e03", "e06", "e07", "e08", "e09", "e10", "e11"]),
        "comparisons never match a blank cell"
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f03_amount", ComparisonOperator::Lt, 5.0))
        ),
        ids(&["e03", "e09", "e11"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f03_amount", ComparisonOperator::Lte, 5.0))
        ),
        ids(&["e02", "e03", "e08", "e09", "e11"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f03_amount", ComparisonOperator::Gt, 5.0))
        ),
        ids(&["e01", "e05", "e06", "e07", "e10"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f03_amount", ComparisonOperator::Gte, 10.0))
        ),
        ids(&["e01", "e05", "e06"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(date_compare(
                "f04_due",
                ComparisonOperator::Gte,
                "2026-02-15"
            ))
        ),
        ids(&["e02", "e07", "e08", "e10", "e12"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(date_compare(
                "f04_due",
                ComparisonOperator::Lt,
                "2026-01-01"
            ))
        ),
        ids(&["e05"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(date_compare(
                "f04_due",
                ComparisonOperator::Eq,
                "2026-01-01"
            ))
        ),
        ids(&["e01", "e06", "e11"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::IsBlank {
                field: "f03_amount".to_owned(),
            })
        ),
        ids(&["e04", "e12"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::NotBlank {
                field: "f04_due".to_owned(),
            })
        ),
        ids(&[
            "e01", "e02", "e03", "e05", "e06", "e07", "e08", "e09", "e10", "e11", "e12",
        ])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::InSet {
                field: "f02_group".to_owned(),
                values: vec![ScalarLiteral::Text("x".to_owned())],
            })
        ),
        ids(&["e01", "e03", "e06", "e08", "e10", "e12"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::InSet {
                field: "f03_amount".to_owned(),
                values: vec![ScalarLiteral::Number(5.0), ScalarLiteral::Number(7.0)],
            })
        ),
        ids(&["e02", "e07", "e08", "e10"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::InSet {
                field: "f05_done".to_owned(),
                values: vec![ScalarLiteral::Boolean(false)],
            })
        ),
        ids(&["e02", "e05", "e07", "e09", "e11"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(compare("f06_double", ComparisonOperator::Gte, 20.0))
        ),
        ids(&["e01", "e05", "e06"]),
        "a formula filters by its calculated Number"
    );
    // Filters AND together.
    let both = TableView {
        filters: vec![
            ViewFilter::TextEquals {
                field: "f02_group".to_owned(),
                value: "X".to_owned(),
            },
            compare("f03_amount", ComparisonOperator::Gt, 5.0),
        ],
        ..TableView::default()
    };
    assert_eq!(order(&mut runtime, both), ids(&["e01", "e06", "e10"]));
}

#[test]
fn kept_entities_stay_visible_and_count_toward_the_total() {
    let mut runtime = small_runtime();
    let view = TableView {
        filters: vec![compare("f03_amount", ComparisonOperator::Gt, 50.0)],
        keep_entities: vec!["e04".to_owned(), "e02".to_owned(), "e06".to_owned()],
        ..TableView::default()
    };
    let kept_window = window(&mut runtime, view.clone(), 0, 256).expect("kept rows are admitted");
    assert_eq!(row_ids(&kept_window), ids(&["e02", "e04", "e06"]));
    assert_eq!(
        kept_window.total_rows, 3,
        "kept entities are part of the total"
    );
    assert_eq!(kept_window.unfiltered_rows, 12);
    // An unknown kept id is ignored rather than refused.
    let unknown_kept = TableView {
        keep_entities: vec!["nobody".to_owned()],
        ..view.clone()
    };
    let kept_unknown = window(&mut runtime, unknown_kept, 0, 256).expect("kept ids are advisory");
    assert_eq!(row_ids(&kept_unknown), ids(&["e06"]));

    let found = located(&mut runtime, view.clone(), "e04").expect("kept rows are locatable");
    assert_eq!(found.result, LocateInViewResult::Found);
    assert_eq!(found.index, Some(1));
    let filtered_out = located(&mut runtime, view, "e01").expect("locating is not an error");
    assert_eq!(filtered_out.result, LocateInViewResult::FilteredOut);
    assert_eq!(filtered_out.index, None);
}

#[test]
fn locate_in_view_reports_the_sorted_position() {
    let mut runtime = small_runtime();
    let view = sorted("f03_amount", SortDirection::Asc);
    let sorted_window = window(&mut runtime, view.clone(), 0, 256).expect("the sorted window");
    let located_row = located(&mut runtime, view, "e06").expect("the row is locatable");
    assert_eq!(located_row.result, LocateInViewResult::Found);
    assert_eq!(
        located_row.index,
        sorted_window.rows.iter().position(|row| row.id == "e06")
    );
    assert_eq!(located_row.index, Some(9));

    let unknown = located(&mut runtime, TableView::default(), "e99").unwrap_err();
    assert!(matches!(
        unknown,
        DesignerError::UnknownEntity { ref entity, .. } if entity == "e99"
    ));

    let stale = runtime.handle(DesignerRequest::LocateInView {
        expected_revision: "revision/not-current".to_owned(),
        collection: "rows".to_owned(),
        view: TableView::default(),
        entity: "e01".to_owned(),
    });
    assert!(matches!(stale, Err(DesignerError::StaleQuery { .. })));
}

#[test]
fn view_validation_fails_with_typed_errors() {
    let mut runtime = small_runtime();
    let unknown_field = window(
        &mut runtime,
        filtered(ViewFilter::IsBlank {
            field: "nope".to_owned(),
        }),
        0,
        1,
    )
    .unwrap_err();
    assert!(matches!(
        unknown_field,
        DesignerError::UnknownViewField { ref field, .. } if field == "nope"
    ));

    let unknown_sort = window(&mut runtime, sorted("nope", SortDirection::Asc), 0, 1).unwrap_err();
    assert!(matches!(
        unknown_sort,
        DesignerError::UnknownViewField { .. }
    ));

    let wrong_kind = window(
        &mut runtime,
        filtered(ViewFilter::TextEquals {
            field: "f03_amount".to_owned(),
            value: "5".to_owned(),
        }),
        0,
        1,
    )
    .unwrap_err();
    assert!(matches!(wrong_kind, DesignerError::InvalidView { .. }));

    let bad_date = window(
        &mut runtime,
        filtered(date_compare("f04_due", ComparisonOperator::Gt, "2026-1-1")),
        0,
        1,
    )
    .unwrap_err();
    assert!(matches!(bad_date, DesignerError::InvalidView { .. }));

    let bad_literal = window(
        &mut runtime,
        filtered(ViewFilter::InSet {
            field: "f03_amount".to_owned(),
            values: vec![ScalarLiteral::Text("five".to_owned())],
        }),
        0,
        1,
    )
    .unwrap_err();
    assert!(matches!(bad_literal, DesignerError::InvalidView { .. }));

    let non_finite = window(
        &mut runtime,
        filtered(compare("f03_amount", ComparisonOperator::Eq, f64::NAN)),
        0,
        1,
    )
    .unwrap_err();
    assert!(matches!(non_finite, DesignerError::InvalidView { .. }));

    let stale_view = runtime.handle(DesignerRequest::QueryTableWindow {
        expected_revision: "revision/not-current".to_owned(),
        collection: "rows".to_owned(),
        view: TableView::default(),
        start: 0,
        count: 1,
    });
    assert!(matches!(stale_view, Err(DesignerError::StaleQuery { .. })));
}

#[test]
fn reference_columns_support_presence_filters_only() {
    let mut runtime = DesignerRuntime::from_document(reference_document(), REFERENCE)
        .expect("the reference fixture is admitted by the ordinary profile");
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::IsBlank {
                field: "f02_link".to_owned(),
            })
        ),
        ids(&["e02"])
    );
    assert_eq!(
        order(
            &mut runtime,
            filtered(ViewFilter::NotBlank {
                field: "f02_link".to_owned(),
            })
        ),
        ids(&["e01", "e03"])
    );
    for filter in [
        ViewFilter::TextEquals {
            field: "f02_link".to_owned(),
            value: "e02".to_owned(),
        },
        compare("f02_link", ComparisonOperator::Eq, 1.0),
        ViewFilter::InSet {
            field: "f02_link".to_owned(),
            values: vec![ScalarLiteral::Text("e02".to_owned())],
        },
    ] {
        assert!(
            matches!(
                window(&mut runtime, filtered(filter), 0, 8),
                Err(DesignerError::InvalidView { .. })
            ),
            "a value filter on a reference column is refused"
        );
    }
    // Silent entity-id ordering would misrepresent a reference sort.
    assert!(matches!(
        window(&mut runtime, sorted("f02_link", SortDirection::Asc), 0, 8),
        Err(DesignerError::InvalidView { .. })
    ));
}

#[test]
fn query_table_refuses_above_one_thousand_rows() {
    let mut thousand = DesignerRuntime::from_document(grid_document(1_000, 3, 8), THOUSAND)
        .expect("1,000 rows are the compatibility limit and still open");
    let DesignerResponse::Table(table) = thousand
        .handle(DesignerRequest::QueryTable {
            collection: "rows".to_owned(),
        })
        .expect("1,000 rows keep the complete projection")
    else {
        panic!("expected a complete table");
    };
    assert_eq!(table.rows.len(), 1_000);

    let mut over = DesignerRuntime::from_document(grid_document(1_001, 3, 8), OVER)
        .expect("1,001 rows are a legal text-capacity rectangle");
    let refusal = over
        .handle(DesignerRequest::QueryTable {
            collection: "rows".to_owned(),
        })
        .unwrap_err();
    assert!(matches!(
        refusal,
        DesignerError::UseWindowQuery {
            rows: 1_001,
            maximum: 1_000,
            ..
        }
    ));

    let total = window(&mut over, TableView::default(), 0, 0).expect("totals are available");
    assert_eq!(total.total_rows, 1_001);
    assert_eq!(total.unfiltered_rows, 1_001);
    assert_eq!(total.rows.len(), 0);
    let first = window(&mut over, TableView::default(), 0, 256).expect("the first window");
    assert_eq!(first.rows.len(), 256);
    assert_eq!(first.rows[0].id, "r00000");
    let tail = window(&mut over, TableView::default(), 1_000, 256).expect("the last window");
    assert_eq!(tail.rows.len(), 1);
    assert_eq!(tail.start, 1_000);
    let past_end = window(&mut over, TableView::default(), 1_001, 256).expect("past the end");
    assert_eq!(past_end.rows.len(), 0);
    assert_eq!(past_end.start, 1_001);
}

#[test]
fn text_capacity_windows_use_the_text_capacity_reply_budget() {
    // 250 legal rows of 16 × 256-byte Text cells stay under the text-capacity
    // aggregate yet project beyond 1 MiB. A text-capacity occurrence already
    // admits its complete table within 16 MiB, so its windows use that budget
    // instead of the strict 1 MiB window cap.
    let mut runtime = DesignerRuntime::from_document(grid_document(250, 16, 256), WIDE)
        .expect("the wide rectangle is admitted");
    let totals = window(&mut runtime, TableView::default(), 0, 0).expect("totals need no rows");
    assert_eq!(totals.total_rows, 250);
    assert_eq!(totals.rows.len(), 0);

    let small = window(&mut runtime, TableView::default(), 0, 16).expect("16 rows fit");
    assert_eq!(small.rows.len(), 16);

    let wide = window(&mut runtime, TableView::default(), 0, 250)
        .expect("a text-capacity window above 1 MiB stays inside 16 MiB");
    assert_eq!(wide.rows.len(), 250);
    assert!(
        serde_json::to_vec(&wide).expect("the window encodes").len() > 1_048_576,
        "the fixture must exceed the strict window cap to prove the profile budget"
    );
}

/// The small fixture document plus one optional trailing column, to prove the
/// digest depends on the column definitions.
fn widened_document() -> Document {
    let mut document = small_document();
    let schema = document
        .schemas
        .get_mut(&SchemaId::from("rows"))
        .expect("the fixture has one schema");
    let (id, definition) = field("f07_extra", FieldType::Text, false);
    schema.fields.insert(id, definition);
    document
}

#[test]
fn view_index_cache_invalidates_when_a_publication_advances_the_revision() {
    let mut runtime = small_runtime();
    let view = sorted("f03_amount", SortDirection::Asc);
    let before = window(&mut runtime, view.clone(), 0, 256).expect("the sorted window");
    assert_eq!(before.rows[0].id, "e03");
    assert_eq!(before.rows[9].id, "e06");
    assert_eq!(before.rows.last().map(|row| row.id.as_str()), Some("e12"));

    let revision = runtime.observe_occurrence().revision;
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target: "e06.f03_amount".into(),
            input: ScalarEditInput::Number {
                input: "-100".to_owned(),
            },
        })
        .expect("the edit publishes")
    else {
        panic!("expected a publication");
    };
    assert_ne!(publication.resulting_revision, before.revision);

    let after = window(&mut runtime, view, 0, 256).expect("the same view after the edit");
    assert_eq!(after.revision, publication.resulting_revision);
    assert_eq!(
        after.rows[0].id, "e06",
        "the cached order must not survive an edit"
    );
    assert_eq!(after.rows[1].id, "e03");
    assert_eq!(after.rows.last().map(|row| row.id.as_str()), Some("e12"));
    assert_eq!(
        after.columns_revision, before.columns_revision,
        "an ordinary row edit never changes the column digest"
    );
    // The formula column follows the edited value.
    let formula = order(&mut runtime, sorted("f06_double", SortDirection::Asc));
    assert_eq!(formula[0], "e06");

    let mut changed = DesignerRuntime::from_document(widened_document(), WIDENED_SCHEMA)
        .expect("the same rows with one extra column are admitted");
    let changed_window = window(&mut changed, TableView::default(), 0, 1).expect("queryable");
    assert_ne!(
        changed_window.columns_revision, before.columns_revision,
        "a column definition change must move the digest"
    );
}

/// Parse literal wire text; the runtime tests avoid the `serde_json` macro so every request
/// stays inside the auditable source surface.
fn wire_json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("the wire request literal is JSON")
}

fn wire_string(text: &str) -> String {
    serde_json::to_string(text).expect("a string encodes")
}

#[test]
fn wire_requests_use_the_documented_envelope() {
    let mut runtime = Some(small_runtime());
    let revision = runtime
        .as_ref()
        .expect("the occurrence is installed")
        .observe_occurrence()
        .revision;
    let request = wire_json(&format!(
        r#"{{
        "type": "query_table_window",
        "expected_revision": {revision},
        "collection": "rows",
        "view": {{
            "sort": [{{"field": "f03_amount", "direction": "asc"}}],
            "filters": [{{"kind": "number_compare", "field": "f03_amount", "op": ">=", "value": 5.0}}],
            "keep_entities": ["e04"]
        }},
        "start": 0,
        "count": 4
    }}"#,
        revision = wire_string(&revision)
    ));
    let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
        &mut runtime,
        &serde_json::to_vec(&request).expect("the request encodes"),
    ))
    .expect("the reply is JSON");
    assert_eq!(reply["status"], "ok", "{reply}");
    assert_eq!(reply["response"]["type"], "table_window");
    assert_eq!(reply["response"]["payload"]["total_rows"], 8);
    assert_eq!(reply["response"]["payload"]["unfiltered_rows"], 12);
    assert_eq!(reply["response"]["payload"]["start"], 0);
    assert_eq!(reply["response"]["payload"]["rows"][0]["id"], "e02");
    assert_eq!(
        reply["response"]["payload"]["rows"]
            .as_array()
            .expect("rows are an array")
            .len(),
        4
    );
    assert!(
        reply["response"]["payload"]["columns_revision"]
            .as_str()
            .unwrap_or_default()
            .starts_with("columns/")
    );

    let locate = wire_json(&format!(
        r#"{{
        "type": "locate_in_view",
        "expected_revision": {revision},
        "collection": "rows",
        "view": {{"sort": [{{"field": "f03_amount", "direction": "asc"}}]}},
        "entity": "e06"
    }}"#,
        revision = wire_string(&revision)
    ));
    let locate_reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
        &mut runtime,
        &serde_json::to_vec(&locate).expect("the request encodes"),
    ))
    .expect("the reply is JSON");
    assert_eq!(locate_reply["response"]["type"], "located_in_view");
    assert_eq!(locate_reply["response"]["payload"]["result"], "found");
    assert_eq!(locate_reply["response"]["payload"]["index"], 9);

    // Every comparison operator and fractional literal survives the buffered
    // (arbitrary-precision) wire decode, not just the buffered `from_value` path.
    for op in ["=", "<>", "<", "<=", ">", ">="] {
        let filtered_request = wire_json(&format!(
            r#"{{
            "type": "query_table_window",
            "expected_revision": {revision},
            "collection": "rows",
            "view": {{
                "filters": [{{"kind": "number_compare", "field": "f03_amount", "op": "{op}", "value": 5.5}}]
            }},
            "start": 0,
            "count": 0
        }}"#,
            revision = wire_string(&revision)
        ));
        let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
            &mut runtime,
            &serde_json::to_vec(&filtered_request).expect("the request encodes"),
        ))
        .expect("the reply is JSON");
        assert_eq!(reply["status"], "ok", "operator {op}: {reply}");
        assert_eq!(reply["response"]["type"], "table_window");
    }
    let set_request = wire_json(&format!(
        r#"{{
        "type": "query_table_window",
        "expected_revision": {revision},
        "collection": "rows",
        "view": {{
            "filters": [{{"kind": "in_set", "field": "f03_amount", "values": [5.0, 7.5]}}]
        }},
        "start": 0,
        "count": 0
    }}"#,
        revision = wire_string(&revision)
    ));
    let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
        &mut runtime,
        &serde_json::to_vec(&set_request).expect("the request encodes"),
    ))
    .expect("the reply is JSON");
    assert_eq!(reply["status"], "ok", "{reply}");

    let mut over = Some(
        DesignerRuntime::from_document(grid_document(1_001, 3, 8), OVER)
            .expect("1,001 rows are admitted"),
    );
    let refusal = wire_json(r#"{"type": "query_table", "collection": "rows"}"#);
    let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
        &mut over,
        &serde_json::to_vec(&refusal).expect("the request encodes"),
    ))
    .expect("the refusal is JSON");
    assert_eq!(reply["status"], "error");
    assert_eq!(reply["error"]["code"], "use_window_query");
    assert_eq!(reply["error"]["row_count"], 1_001);
}

#[test]
fn windows_carry_the_query_table_profile_flags() {
    let mut runtime = small_runtime();
    let table = whole_table(&mut runtime);
    let totals = window(&mut runtime, TableView::default(), 0, 0).expect("count 0 is legal");
    assert_eq!(totals.tracker_profile, table.tracker_profile);
    assert_eq!(totals.native_table_profile, table.native_table_profile);

    let mut wide = DesignerRuntime::from_document(grid_document(250, 16, 256), WIDE)
        .expect("the wide rectangle is admitted");
    let wide_table = whole_table(&mut wide);
    let wide_totals = window(&mut wide, TableView::default(), 0, 0).expect("count 0 is legal");
    assert_eq!(wide_totals.tracker_profile, wide_table.tracker_profile);
    assert_eq!(
        wide_totals.native_table_profile,
        wide_table.native_table_profile
    );
}

fn whole_table(runtime: &mut DesignerRuntime) -> tachiko_designer_runtime::TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: "rows".to_owned(),
        })
        .expect("the table is at most 1,000 rows")
    else {
        panic!("expected a table projection");
    };
    table
}

/// Three native-table rows over an optional Number column holding 1, 2 and a
/// blank, so the whole-collection total is 3 and the blank is skipped.
const TOTALS: &str = "00000000-0000-4000-8000-000000000b07";

fn totals_document() -> Document {
    schema_document(
        vec![
            field("f01_label", FieldType::Text, false),
            field("f02_amount", FieldType::Number, false),
        ],
        vec![
            entity(
                "e01",
                vec![("f01_label", text("one")), ("f02_amount", number(1.0))],
            ),
            entity(
                "e02",
                vec![("f01_label", text("two")), ("f02_amount", number(2.0))],
            ),
            entity("e03", vec![("f01_label", text("three"))]),
        ],
    )
}

fn totals_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(totals_document(), TOTALS)
        .expect("a Text/Number native table is admitted")
}

fn amount_total(totals: &BTreeMap<String, Option<f64>>) -> Option<f64> {
    totals.get("f02_amount").copied().flatten()
}

#[test]
fn every_window_carries_the_query_table_column_totals() {
    let mut runtime = totals_runtime();
    let expected = whole_table(&mut runtime).column_totals;
    assert_eq!(amount_total(&expected), Some(3.0));

    for (start, count) in [(0, 1), (1, 1), (0, 0)] {
        let window = window(&mut runtime, TableView::default(), start, count)
            .expect("the window is bounded");
        assert_eq!(
            window.column_totals, expected,
            "start {start} with count {count} must carry the whole-collection totals"
        );
    }
}

#[test]
fn a_filtered_window_still_reports_the_whole_collection_totals() {
    let mut runtime = totals_runtime();
    let expected = whole_table(&mut runtime).column_totals;

    let filtered = window(
        &mut runtime,
        filtered(compare("f02_amount", ComparisonOperator::Gt, 1.5)),
        0,
        256,
    )
    .expect("the filtered view is queryable");
    assert_eq!(row_ids(&filtered), ids(&["e02"]));
    assert_eq!(filtered.total_rows, 1);
    assert_eq!(
        filtered.column_totals, expected,
        "a view filter must never narrow the footer"
    );
}

#[test]
fn the_column_totals_cache_follows_the_revision() {
    let mut runtime = totals_runtime();
    let before = window(&mut runtime, TableView::default(), 0, 1).expect("the window is bounded");
    assert_eq!(amount_total(&before.column_totals), Some(3.0));

    let revision = runtime.observe_occurrence().revision;
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target: "e02.f02_amount".into(),
            input: ScalarEditInput::Number {
                input: "10".to_owned(),
            },
        })
        .expect("the edit publishes")
    else {
        panic!("expected a publication");
    };

    let after = window(&mut runtime, TableView::default(), 0, 1).expect("the window is bounded");
    assert_eq!(after.revision, publication.resulting_revision);
    assert_eq!(
        amount_total(&after.column_totals),
        Some(11.0),
        "the cached totals must not survive an edit"
    );
}

fn column_totals(
    runtime: &mut DesignerRuntime,
    entities: &[&str],
) -> Result<ColumnTotalsProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::QueryColumnTotals {
        expected_revision,
        collection: "rows".to_owned(),
        entities: ids(entities),
    })? {
        DesignerResponse::ColumnTotals(totals) => Ok(totals),
        other => panic!("expected column totals, got {other:?}"),
    }
}

fn amount_of(totals: &ColumnTotalsProjection) -> Option<f64> {
    amount_total(&totals.column_totals)
}

/// Forty fractional Number values, so a sum that changed its addition order
/// would show up in the last bits instead of cancelling out.
fn fractional_runtime() -> DesignerRuntime {
    let entities = (0_u16..40)
        .map(|row| {
            entity(
                &format!("e{row:02}"),
                vec![
                    ("f01_label", text(&format!("row {row}"))),
                    ("f02_amount", number(0.1 * f64::from(row))),
                ],
            )
        })
        .collect();
    DesignerRuntime::from_document(
        schema_document(
            vec![
                field("f01_label", FieldType::Text, false),
                field("f02_amount", FieldType::Number, false),
            ],
            entities,
        ),
        TOTALS,
    )
    .expect("a Text/Number native table is admitted")
}

/// The totals table plus a second collection holding one foreign entity.
fn two_collection_runtime() -> DesignerRuntime {
    let mut document = totals_document();
    let other = SchemaId::from("other");
    document.schemas.insert(
        other.clone(),
        Schema {
            field_order: None,
            id: other.clone(),
            key: SchemaKey::from("other"),
            display_name: None,
            fields: [field("f02_amount", FieldType::Number, false)]
                .into_iter()
                .collect(),
        },
    );
    let (id, mut foreign) = entity("x01", vec![("f02_amount", number(5.0))]);
    foreign.schema = other;
    document.entities.insert(id, foreign);
    DesignerRuntime::from_document(document, TOTALS).expect("two collections are admitted")
}

#[test]
fn naming_every_entity_matches_the_window_footer_exactly() {
    let mut runtime = fractional_runtime();
    let every = (0..40).map(|row| format!("e{row:02}")).collect::<Vec<_>>();
    let every_refs = every.iter().map(String::as_str).collect::<Vec<_>>();
    let named = column_totals(&mut runtime, &every_refs).expect("every entity is a member");
    let footer = window(&mut runtime, TableView::default(), 0, 0)
        .expect("count 0 is legal")
        .column_totals;
    assert_eq!(named.rows, 40);
    assert_eq!(
        named.column_totals, footer,
        "the full set must match the footer bit for bit"
    );
}

#[test]
fn a_subset_sums_only_its_own_entities() {
    let mut runtime = fractional_runtime();
    let subset = column_totals(&mut runtime, &["e31", "e05", "e30"])
        .expect("the members are in the collection");
    let expected = [5_u16, 30, 31]
        .iter()
        .fold(0.0, |total, row| total + 0.1 * f64::from(*row));
    let got = amount_of(&subset).expect("the column total is present");
    assert_eq!(subset.rows, 3);
    assert!(
        (got - expected).abs() < 1e-9,
        "subset sum {got} must equal {expected}"
    );
}

#[test]
fn a_filtered_window_total_equals_the_rows_it_shows() {
    let mut runtime = totals_runtime();
    let shown = window(
        &mut runtime,
        filtered(compare("f02_amount", ComparisonOperator::Gt, 1.5)),
        0,
        256,
    )
    .expect("the filtered view is queryable");
    let shown_ids = row_ids(&shown);
    let refs = shown_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let named = column_totals(&mut runtime, &refs).expect("the shown rows are members");
    assert_eq!(shown_ids, ids(&["e02"]));
    assert_eq!(named.rows, 1);
    assert_eq!(amount_of(&named), Some(2.0));
    assert_eq!(amount_total(&shown.column_totals), Some(3.0));
}

#[test]
fn blank_cells_are_skipped_and_an_empty_set_totals_to_zero() {
    let mut runtime = totals_runtime();
    let blank_only = column_totals(&mut runtime, &["e03"]).expect("e03 is a member");
    assert_eq!(amount_of(&blank_only), Some(0.0));
    let empty = column_totals(&mut runtime, &[]).expect("an empty set is legal");
    assert_eq!(empty.rows, 0);
    assert_eq!(amount_of(&empty), Some(0.0));
}

#[test]
fn unknown_duplicate_foreign_oversized_and_stale_sets_are_refused_unchanged() {
    let mut runtime = two_collection_runtime();
    let before = runtime.observe_occurrence().revision;

    let unknown = column_totals(&mut runtime, &["e01", "e99"]).unwrap_err();
    assert!(matches!(
        unknown,
        DesignerError::InvalidView { ref message } if message.contains("e99")
    ));
    let duplicate = column_totals(&mut runtime, &["e01", "e01"]).unwrap_err();
    assert!(matches!(
        duplicate,
        DesignerError::InvalidView { ref message } if message.contains("e01")
    ));
    let foreign = column_totals(&mut runtime, &["e01", "x01"]).unwrap_err();
    assert!(matches!(
        foreign,
        DesignerError::InvalidView { ref message } if message.contains("x01")
    ));
    // One more than the 1,000-row table bound.
    let oversized = ["e01"; 1001];
    let too_many = column_totals(&mut runtime, &oversized).unwrap_err();
    assert!(matches!(too_many, DesignerError::InvalidView { .. }));
    let stale = runtime.handle(DesignerRequest::QueryColumnTotals {
        expected_revision: "revision/not-current".to_owned(),
        collection: "rows".to_owned(),
        entities: ids(&["e01"]),
    });
    assert!(matches!(stale, Err(DesignerError::StaleQuery { .. })));

    assert_eq!(
        runtime.observe_occurrence().revision,
        before,
        "a refused request must change nothing"
    );
}

#[test]
fn column_totals_follow_the_revision_after_an_edit() {
    let mut runtime = totals_runtime();
    let before = column_totals(&mut runtime, &["e01", "e02", "e03"]).expect("the set is valid");
    assert_eq!(amount_of(&before), Some(3.0));

    let revision = runtime.observe_occurrence().revision;
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: revision,
            target: "e02.f02_amount".into(),
            input: ScalarEditInput::Number {
                input: "10".to_owned(),
            },
        })
        .expect("the edit publishes")
    else {
        panic!("expected a publication");
    };

    let after = column_totals(&mut runtime, &["e01", "e02", "e03"]).expect("the set is valid");
    assert_eq!(after.revision, publication.resulting_revision);
    assert_eq!(amount_of(&after), Some(11.0));
    let without_edited_row = column_totals(&mut runtime, &["e01", "e03"]).expect("the set is valid");
    assert_eq!(amount_of(&without_edited_row), Some(1.0));
}

#[test]
fn column_totals_wire_request_uses_the_documented_envelope() {
    let mut runtime = Some(totals_runtime());
    let revision = runtime
        .as_ref()
        .expect("the occurrence is installed")
        .observe_occurrence()
        .revision;
    let request = wire_json(&format!(
        r#"{{
        "type": "query_column_totals",
        "expected_revision": {revision},
        "collection": "rows",
        "entities": ["e02", "e03"]
    }}"#,
        revision = wire_string(&revision)
    ));
    let reply: serde_json::Value = serde_json::from_slice(&process_wire_request(
        &mut runtime,
        &serde_json::to_vec(&request).expect("the request encodes"),
    ))
    .expect("the reply is JSON");
    assert_eq!(reply["response"]["type"], "column_totals");
    assert_eq!(reply["response"]["payload"]["rows"], 2);
    assert_eq!(
        reply["response"]["payload"]["column_totals"]["f02_amount"],
        2.0
    );
}
