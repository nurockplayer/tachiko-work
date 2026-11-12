//! #678: the direct column-totals pass against the projection path.
//!
//! `totals_for_entities` now folds the stored and calculated cells straight
//! from the resident session. This module keeps the former
//! `query_entities` → `query_fields` → `project_field` path as the oracle and
//! compares the two on documents covering every classification rule.

use std::collections::BTreeMap;

use super::{
    ColumnTotalsProjection, DesignerRequest, DesignerResponse, DesignerRuntime, Document, Entity,
    EntityId, EntityKey, Expression, FieldDefinition, FieldId, FieldKey, FieldType, Number, Schema,
    SchemaId, SchemaKey, TableView, Value, ViewFilter, totals_cache_counters,
};
use tachiko_workspace_engine::FieldConstraint;

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000678";

fn number(value: f64) -> Number {
    Number::new(value).expect("fixture numbers are finite")
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

fn stored(value: f64) -> Value {
    Value::Number(number(value))
}

fn same_row(field: &str) -> Expression {
    Expression::SameRowReference(FieldId::from(field))
}

fn multiply(left: Expression, right: f64) -> Expression {
    Expression::Multiply {
        left: Box::new(left),
        right: Box::new(Expression::Number(number(right))),
    }
}

/// A valid fixture: blank cells, `-0` beside `0`, a tiny and a huge value in
/// an order that changes the float sum, stored Numbers, and calculated cells.
pub(super) fn valid_document() -> Document {
    let mut document = Document::empty("totals_direct_seed", "Totals direct seed");
    let schema = SchemaId::from("rows");
    document.schemas.insert(
        schema.clone(),
        Schema {
            field_order: None,
            id: schema,
            key: SchemaKey::from("rows"),
            display_name: None,
            fields: vec![
                field("f01_amount", FieldType::Number),
                field("f02_tiny", FieldType::Number),
                field("f03_calc", FieldType::Number),
                field("f04_other", FieldType::Number),
                field("f05_text", FieldType::Text),
            ]
            .into_iter()
            .collect(),
        },
    );
    document.entities = vec![
        entity(
            "e01",
            vec![
                ("f01_amount", stored(1e16)),
                ("f02_tiny", stored(-0.0)),
                (
                    "f03_calc",
                    Value::Formula(multiply(same_row("f01_amount"), 0.5)),
                ),
                ("f05_text", Value::Text("alpha".to_owned())),
            ],
        ),
        entity(
            "e02",
            vec![
                ("f01_amount", stored(-1e16)),
                ("f02_tiny", stored(0.0)),
                (
                    "f03_calc",
                    Value::Formula(Expression::Add {
                        left: Box::new(same_row("f02_tiny")),
                        right: Box::new(Expression::Number(number(1.0))),
                    }),
                ),
                ("f05_text", Value::Text("beta".to_owned())),
            ],
        ),
        entity(
            "e03",
            vec![
                ("f01_amount", stored(1.0)),
                ("f02_tiny", stored(5e-324)),
                ("f04_other", stored(7.0)),
                ("f05_text", Value::Text("gamma".to_owned())),
            ],
        ),
        entity(
            "e04",
            vec![
                ("f01_amount", stored(-0.0)),
                ("f05_text", Value::Text("delta".to_owned())),
            ],
        ),
        // e05 has no amount, so it is blank in that column.
        entity(
            "e05",
            vec![
                ("f02_tiny", stored(1e-300)),
                ("f05_text", Value::Text("epsilon".to_owned())),
            ],
        ),
        entity(
            "e06",
            vec![
                ("f01_amount", stored(2.5)),
                (
                    "f04_other",
                    Value::Formula(multiply(same_row("f01_amount"), 4.0)),
                ),
                ("f05_text", Value::Text("zeta".to_owned())),
            ],
        ),
    ]
    .into_iter()
    .collect();
    document
}

/// A fixture a live document cannot hold: a stored Text in a Number column, a
/// formula that fails, and a valid formula whose outcome is unavailable while
/// the calculation is failed.
fn invalid_document() -> Document {
    let mut document = valid_document();
    document
        .entities
        .get_mut(&EntityId::from("e04"))
        .expect("e04 exists")
        .fields
        .insert(FieldId::from("f01_amount"), Value::Text("2".to_owned()));
    document
        .entities
        .get_mut(&EntityId::from("e05"))
        .expect("e05 exists")
        .fields
        .insert(
            FieldId::from("f03_calc"),
            Value::Formula(Expression::Divide {
                left: Box::new(Expression::Number(number(1.0))),
                right: Box::new(Expression::Number(number(0.0))),
            }),
        );
    document
}

/// Open the valid fixture, then install `document` without the open-time
/// validation so the differential can reach cells a live document cannot. The
/// two fixtures share their schema, entity ids and columns, so the runtime's
/// collection specs and profiles stay valid.
fn runtime_for(document: Document) -> DesignerRuntime {
    let mut runtime = DesignerRuntime::from_document(valid_document(), OCCURRENCE)
        .expect("the valid fixture is admitted");
    runtime.session = tachiko_workspace_engine::resident_session::ResidentWorkspaceSession::new(
        runtime.document_scope.clone(),
        document,
    );
    runtime
}

fn bits(totals: &BTreeMap<String, Option<f64>>) -> BTreeMap<String, Option<u64>> {
    totals
        .iter()
        .map(|(field, total)| (field.clone(), total.map(f64::to_bits)))
        .collect()
}

fn reference_totals(
    runtime: &DesignerRuntime,
    members: &[EntityId],
) -> BTreeMap<String, Option<f64>> {
    let collection = runtime.default_collection.clone();
    let spec = runtime
        .collection_spec(&collection)
        .expect("the fixture collection exists");
    runtime
        .reference_totals_for_entities(spec, members)
        .expect("the projection path totals the members")
}

fn new_totals(runtime: &DesignerRuntime, members: &[EntityId]) -> BTreeMap<String, Option<f64>> {
    let collection = runtime.default_collection.clone();
    let spec = runtime
        .collection_spec(&collection)
        .expect("the fixture collection exists");
    runtime
        .totals_for_entities(spec, members)
        .expect("the direct pass totals the members")
}

/// The named entities in collection order, as the request path sums them.
fn collection_order(runtime: &DesignerRuntime, requested: &[&str]) -> Vec<EntityId> {
    let collection = runtime.default_collection.clone();
    runtime
        .collection_spec(&collection)
        .expect("the fixture collection exists")
        .entities
        .iter()
        .filter(|id| requested.iter().any(|name| *name == id.as_str()))
        .cloned()
        .collect()
}

fn all_entities(runtime: &DesignerRuntime) -> Vec<EntityId> {
    let collection = runtime.default_collection.clone();
    runtime
        .collection_spec(&collection)
        .expect("the fixture collection exists")
        .entities
        .clone()
}

fn named_totals(runtime: &mut DesignerRuntime, entities: &[&str]) -> ColumnTotalsProjection {
    let expected_revision = runtime.current_revision().to_owned();
    let collection = runtime.default_collection.clone();
    match runtime.handle(DesignerRequest::QueryColumnTotals {
        expected_revision,
        collection,
        entities: entities.iter().map(|id| (*id).to_owned()).collect(),
    }) {
        Ok(DesignerResponse::ColumnTotals(totals)) => totals,
        other => panic!("expected column totals, got {other:?}"),
    }
}

fn view_totals(runtime: &mut DesignerRuntime, view: TableView) -> ColumnTotalsProjection {
    let expected_revision = runtime.current_revision().to_owned();
    let collection = runtime.default_collection.clone();
    match runtime.handle(DesignerRequest::QueryViewTotals {
        expected_revision,
        collection,
        view,
    }) {
        Ok(DesignerResponse::ColumnTotals(totals)) => totals,
        other => panic!("expected column totals, got {other:?}"),
    }
}

pub(super) fn window_totals(runtime: &mut DesignerRuntime) -> BTreeMap<String, Option<f64>> {
    let expected_revision = runtime.current_revision().to_owned();
    let collection = runtime.default_collection.clone();
    match runtime.handle(DesignerRequest::QueryTableWindow {
        expected_revision,
        collection,
        view: TableView::default(),
        start: 0,
        count: 0,
        display: false,
    }) {
        Ok(DesignerResponse::TableWindow(window)) => window.column_totals,
        other => panic!("expected a table window, got {other:?}"),
    }
}

fn alpha_only() -> TableView {
    TableView {
        filters: vec![ViewFilter::TextContains {
            field: "f05_text".to_owned(),
            value: "alpha".to_owned(),
        }],
        ..TableView::default()
    }
}

fn shows_nothing() -> TableView {
    TableView {
        filters: vec![ViewFilter::TextEquals {
            field: "f05_text".to_owned(),
            value: "nobody".to_owned(),
        }],
        ..TableView::default()
    }
}

#[test]
fn the_direct_pass_matches_the_projection_path() {
    for document in [valid_document(), invalid_document()] {
        let runtime = runtime_for(document);
        let every = all_entities(&runtime);
        let subset = collection_order(&runtime, &["e03", "e01", "e06"]);
        let cases = [&every[..], &subset[..], &[]];
        for members in cases {
            assert_eq!(
                bits(&new_totals(&runtime, members)),
                bits(&reference_totals(&runtime, members)),
                "members {members:?}"
            );
        }
    }
}

/// Pin what each fixture cell is, so the differential above cannot go quiet by
/// losing a failed, unavailable or non-Number contributor.
#[test]
fn the_fixtures_cover_every_cell_classification() {
    let valid = runtime_for(valid_document());
    let totals = new_totals(&valid, &all_entities(&valid));
    assert_eq!(
        totals
            .get("f01_amount")
            .copied()
            .flatten()
            .map(f64::to_bits),
        Some(3.5_f64.to_bits()),
        "collection order adds 1e16, -1e16, 1, -0 and 2.5 to exactly 3.5"
    );
    assert!(
        totals.get("f02_tiny").copied().flatten().is_some(),
        "the tiny and -0 values add to a finite total"
    );
    assert!(
        totals.get("f03_calc").copied().flatten().is_some(),
        "calculated values add"
    );
    assert_eq!(
        totals.get("f04_other").copied().flatten().map(f64::to_bits),
        Some(17.0_f64.to_bits()),
        "stored and calculated values add in the same column"
    );

    let invalid = runtime_for(invalid_document());
    let totals = new_totals(&invalid, &all_entities(&invalid));
    assert_eq!(
        totals.get("f01_amount").copied().flatten(),
        None,
        "a stored Text in a Number column nulls it"
    );
    assert_eq!(
        totals.get("f03_calc").copied().flatten(),
        None,
        "a failed and an unavailable calculation null the column"
    );
    assert_eq!(
        totals.get("f04_other").copied().flatten(),
        None,
        "an unavailable calculation nulls the column"
    );
}

#[test]
fn every_request_path_matches_the_projection_path() {
    for document in [valid_document(), invalid_document()] {
        let mut runtime = runtime_for(document);
        let every = all_entities(&runtime);

        assert_eq!(
            bits(&window_totals(&mut runtime)),
            bits(&reference_totals(&runtime, &every)),
            "the window footer totals the whole collection"
        );

        let subset = collection_order(&runtime, &["e03", "e01", "e06"]);
        assert_eq!(
            bits(&named_totals(&mut runtime, &["e03", "e01", "e06"]).column_totals),
            bits(&reference_totals(&runtime, &subset)),
            "a named subset totals only its members, in collection order"
        );

        let names = every
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect::<Vec<_>>();
        let named_every = names.iter().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(
            bits(&named_totals(&mut runtime, &named_every).column_totals),
            bits(&reference_totals(&runtime, &every)),
            "naming every entity equals the window footer"
        );

        assert_eq!(
            bits(&view_totals(&mut runtime, TableView::default()).column_totals),
            bits(&reference_totals(&runtime, &every)),
            "an unfiltered view totals the whole collection"
        );

        let alpha = collection_order(&runtime, &["e01"]);
        let filtered = view_totals(&mut runtime, alpha_only());
        assert_eq!(filtered.rows, alpha.len());
        assert_eq!(
            bits(&filtered.column_totals),
            bits(&reference_totals(&runtime, &alpha)),
            "a filtered view totals the rows it shows"
        );

        let empty = named_totals(&mut runtime, &[]);
        assert_eq!(empty.rows, 0);
        assert_eq!(
            bits(&empty.column_totals),
            bits(&reference_totals(&runtime, &[])),
            "an empty member set totals zero"
        );
        let nothing = view_totals(&mut runtime, shows_nothing());
        assert_eq!(nothing.rows, 0);
        assert_eq!(
            bits(&nothing.column_totals),
            bits(&reference_totals(&runtime, &[])),
            "a view that shows nothing totals zero"
        );
    }
}

#[test]
fn a_full_collection_query_totals_fills_then_hits_the_footer_cache() {
    let mut runtime = runtime_for(valid_document());
    let every = all_entities(&runtime);
    let names = every
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();
    let named_every = names.iter().map(String::as_str).collect::<Vec<_>>();

    let fills = totals_cache_counters::fills();
    let hits = totals_cache_counters::hits();

    named_totals(&mut runtime, &["e01"]);
    assert_eq!(
        totals_cache_counters::fills(),
        fills,
        "a subset never fills the footer cache"
    );
    assert_eq!(
        totals_cache_counters::hits(),
        hits,
        "a subset never reads the footer cache"
    );

    named_totals(&mut runtime, &named_every);
    assert_eq!(
        totals_cache_counters::fills(),
        fills + 1,
        "naming every entity fills the footer cache"
    );
    assert_eq!(
        totals_cache_counters::hits(),
        hits,
        "the first full naming misses"
    );

    named_totals(&mut runtime, &named_every);
    assert_eq!(
        totals_cache_counters::hits(),
        hits + 1,
        "the next full naming reads the cache"
    );

    let view = view_totals(&mut runtime, TableView::default());
    assert_eq!(view.rows, every.len());
    assert_eq!(
        totals_cache_counters::hits(),
        hits + 2,
        "an unfiltered view totals reads the same cache"
    );
    assert_eq!(totals_cache_counters::fills(), fills + 1);
}

#[test]
fn an_unfiltered_view_totals_fills_then_hits_the_footer_cache() {
    let mut runtime = runtime_for(valid_document());
    let every = all_entities(&runtime);

    let fills = totals_cache_counters::fills();
    let hits = totals_cache_counters::hits();

    let first = view_totals(&mut runtime, TableView::default());
    assert_eq!(first.rows, every.len());
    assert_eq!(
        totals_cache_counters::fills(),
        fills + 1,
        "an unfiltered view totals fills the footer cache"
    );
    assert_eq!(totals_cache_counters::hits(), hits);

    view_totals(&mut runtime, TableView::default());
    assert_eq!(
        totals_cache_counters::hits(),
        hits + 1,
        "the next unfiltered view totals reads the cache"
    );

    let filtered = view_totals(&mut runtime, alpha_only());
    assert_eq!(filtered.rows, 1);
    assert_eq!(
        totals_cache_counters::hits(),
        hits + 1,
        "a filtered view never reads the whole-collection cache"
    );
    assert_eq!(totals_cache_counters::fills(), fills + 1);
}
