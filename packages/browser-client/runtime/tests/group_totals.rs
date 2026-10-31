//! Work #613: read-only grouped totals (`query_group_totals`).
//!
//! One bank-like table answers "how much per category, and per month?". The
//! view is never sent; the request names one of three scopes and the runtime
//! decides membership. These tests pin the typed key order (blank last, empty
//! text distinct from blank), the footer rule reused from `query_column_totals`,
//! both scope complements, every refusal and both bounds.
#![allow(clippy::too_many_lines)] // One fixture and its independent oracle stay together.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    ColumnTotalsProjection, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    GroupBucket, GroupDimension, GroupKey, GroupScope, GroupTotalsProjection, GroupTotalsRow,
    process_wire_request,
};
use tachiko_workspace_engine::{
    Date, Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const GROUP: &str = "00000000-0000-4000-8000-000000000d01";
/// A Text/Number rectangle past `MAX_DATA_ROWS`, so it enters the text-capacity
/// profile and can exceed the ordinary 128-row cap.
const CATEGORIES: &str = "00000000-0000-4000-8000-000000000d02";
const LARGE: &str = "00000000-0000-4000-8000-000000000d03";

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn date(value: &str) -> Value {
    Value::Date(Date::parse(value).expect("fixture dates are canonical"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
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
    let mut document = Document::empty("group_seed", "Group seed");
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

/// One bank-like row. Every optional argument is a present cell; `None` leaves
/// the field absent, which is the only way to produce a blank grouping key or a
/// blank measure.
fn group_row(
    id: &str,
    category: Option<&str>,
    when: Option<&str>,
    amount: Option<f64>,
    paid: Option<bool>,
    fee: Option<f64>,
) -> (EntityId, Entity) {
    let mut values = Vec::new();
    if let Some(category) = category {
        values.push(("f01_category", text(category)));
    }
    if let Some(when) = when {
        values.push(("f02_when", date(when)));
    }
    if let Some(amount) = amount {
        values.push(("f03_amount", number(amount)));
    }
    if let Some(paid) = paid {
        values.push(("f04_paid", Value::Boolean(paid)));
    }
    if let Some(fee) = fee {
        values.push(("f05_fee", number(fee)));
    }
    entity(id, values)
}

/// Twenty rows: two categories differing only in case, an empty text and two
/// blank categories, three months of 2026 plus 2025-12 and one blank date,
/// negative and zero amounts, blank amounts, and every Yes/No state.
///
/// No formula column is included: a valid Designer document cannot hold a
/// failing or unavailable calculation, because every publication validates the
/// complete calculation (a division by zero is `validation_failed`, see
/// `designer_runtime.rs`) and a same-row reference to a blank optional Number
/// evaluates as 0. The null rule is therefore pinned by the shared
/// `NumberTotal` unit test in the library, not by an end-to-end fixture.
fn group_document() -> Document {
    schema_document(
        vec![
            field("f01_category", FieldType::Text, false),
            field("f02_when", FieldType::Date, false),
            field("f03_amount", FieldType::Number, false),
            field("f04_paid", FieldType::Boolean, false),
            field("f05_fee", FieldType::Number, false),
        ],
        vec![
            group_row(
                "e01",
                Some("Food"),
                Some("2026-01-05"),
                Some(10.0),
                Some(true),
                Some(1.0),
            ),
            group_row(
                "e02",
                Some("Food"),
                Some("2026-01-20"),
                Some(-2.0),
                Some(false),
                None,
            ),
            group_row(
                "e03",
                Some("Food"),
                Some("2026-02-03"),
                Some(0.0),
                None,
                Some(2.0),
            ),
            group_row(
                "e04",
                Some("Food"),
                Some("2026-03-15"),
                None,
                Some(true),
                None,
            ),
            group_row(
                "e05",
                Some("food"),
                Some("2026-02-10"),
                Some(5.0),
                Some(true),
                Some(1.5),
            ),
            group_row(
                "e06",
                Some("Rent"),
                Some("2026-01-01"),
                Some(-100.0),
                Some(true),
                None,
            ),
            group_row(
                "e07",
                Some("Rent"),
                Some("2026-03-01"),
                Some(-50.0),
                Some(false),
                Some(3.0),
            ),
            group_row(
                "e08",
                Some("Transport"),
                Some("2025-12-31"),
                Some(7.0),
                Some(false),
                Some(0.5),
            ),
            group_row(
                "e09",
                Some("Transport"),
                Some("2026-03-31"),
                Some(3.0),
                Some(true),
                None,
            ),
            group_row(
                "e10",
                Some(""),
                Some("2026-02-14"),
                Some(1.0),
                Some(false),
                Some(1.0),
            ),
            group_row("e11", None, Some("2026-01-31"), Some(2.0), Some(true), None),
            group_row("e12", None, None, None, None, None),
            group_row(
                "e13",
                Some("Food"),
                Some("2026-01-15"),
                Some(4.0),
                Some(true),
                Some(2.0),
            ),
            group_row(
                "e14",
                Some("Rent"),
                Some("2026-02-28"),
                Some(0.0),
                Some(true),
                None,
            ),
            group_row(
                "e15",
                Some("Transport"),
                Some("2026-02-05"),
                Some(-1.0),
                None,
                Some(1.0),
            ),
            group_row(
                "e16",
                Some("food"),
                Some("2025-12-05"),
                Some(2.0),
                Some(false),
                None,
            ),
            group_row(
                "e17",
                Some(""),
                Some("2026-03-20"),
                Some(-0.5),
                Some(true),
                Some(0.25),
            ),
            group_row(
                "e18",
                Some("Food"),
                Some("2026-02-20"),
                Some(6.0),
                Some(false),
                None,
            ),
            group_row(
                "e19",
                Some("Rent"),
                Some("2026-01-10"),
                Some(20.0),
                None,
                None,
            ),
            group_row(
                "e20",
                Some("Transport"),
                Some("2026-03-10"),
                Some(8.0),
                Some(true),
                Some(4.0),
            ),
        ],
    )
}

fn group_runtime() -> DesignerRuntime {
    DesignerRuntime::from_document(group_document(), GROUP)
        .expect("the Text/Date/Number fixture is admitted")
}

fn value_dimension(field: &str) -> GroupDimension {
    GroupDimension {
        field: field.to_owned(),
        bucket: GroupBucket::Value,
    }
}

fn bucket_dimension(field: &str, bucket: GroupBucket) -> GroupDimension {
    GroupDimension {
        field: field.to_owned(),
        bucket,
    }
}

fn all_rows() -> GroupScope {
    GroupScope::All
}

fn entities_scope(ids: &[&str]) -> GroupScope {
    GroupScope::Entities {
        entities: ids.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn except_scope(ids: &[&str]) -> GroupScope {
    GroupScope::Except {
        entities: ids.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn group_totals(
    runtime: &mut DesignerRuntime,
    dimensions: Vec<GroupDimension>,
    measures: &[&str],
    scope: GroupScope,
) -> Result<GroupTotalsProjection, DesignerError> {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime.handle(DesignerRequest::QueryGroupTotals {
        expected_revision,
        collection: "rows".to_owned(),
        dimensions,
        measures: measures
            .iter()
            .map(|measure| (*measure).to_owned())
            .collect(),
        scope,
    })? {
        DesignerResponse::GroupTotals(totals) => Ok(totals),
        other => panic!("expected grouped totals, got {other:?}"),
    }
}

fn key_label(key: &GroupKey) -> String {
    match key {
        GroupKey::Boolean { value } => format!("boolean:{value}"),
        GroupKey::Date { value } => format!("date:{value}"),
        GroupKey::Month { value } => format!("month:{value}"),
        GroupKey::Year { value } => format!("year:{value}"),
        GroupKey::Text { value } => format!("text:{value}"),
        GroupKey::Blank => "blank".to_owned(),
    }
}

fn group_labels(group: &GroupTotalsRow) -> Vec<String> {
    group.keys.iter().map(key_label).collect()
}

fn find_group<'a>(totals: &'a GroupTotalsProjection, labels: &[&str]) -> &'a GroupTotalsRow {
    totals
        .groups
        .iter()
        .find(|group| {
            group_labels(group)
                .iter()
                .map(String::as_str)
                .eq(labels.iter().copied())
        })
        .unwrap_or_else(|| panic!("expected a group with keys {labels:?}"))
}

fn column_totals(runtime: &mut DesignerRuntime, entities: &[&str]) -> ColumnTotalsProjection {
    let expected_revision = runtime.observe_occurrence().revision;
    match runtime
        .handle(DesignerRequest::QueryColumnTotals {
            expected_revision,
            collection: "rows".to_owned(),
            entities: entities.iter().map(|id| (*id).to_owned()).collect(),
        })
        .expect("the entity set is valid")
    {
        DesignerResponse::ColumnTotals(totals) => totals,
        other => panic!("expected column totals, got {other:?}"),
    }
}

/// Every entity id of the twenty-row fixture, in collection order.
fn all_ids() -> Vec<String> {
    (1..=20).map(|row| format!("e{row:02}")).collect()
}

#[test]
fn category_groups_are_ordered_and_reconcile_with_column_totals() {
    let mut runtime = group_runtime();
    let totals = group_totals(
        &mut runtime,
        vec![value_dimension("f01_category")],
        &["f03_amount"],
        all_rows(),
    )
    .expect("one Text dimension and one Number measure are valid");

    assert_eq!(totals.revision, runtime.observe_occurrence().revision);
    assert_eq!(totals.dimensions, vec![value_dimension("f01_category")]);
    assert_eq!(totals.measures, vec!["f03_amount".to_owned()]);
    assert_eq!(totals.rows, 20);

    // Rust `String` order is Unicode scalar order: "" sorts before every
    // non-empty key and lowercase letters sort after uppercase ones, so the
    // present keys are "", Food, Rent, Transport, food. The derived `Ord` puts
    // the Blank variant last.
    let labels = totals
        .groups
        .iter()
        .map(|group| group_labels(group).join("|"))
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "text:",
            "text:Food",
            "text:Rent",
            "text:Transport",
            "text:food",
            "blank",
        ]
    );

    assert_eq!(find_group(&totals, &["text:"]).rows, 2);
    assert_eq!(find_group(&totals, &["text:"]).totals, vec![Some(0.5)]);
    let food = find_group(&totals, &["text:Food"]);
    assert_eq!(food.rows, 6, "e04 has a blank amount and must still count");
    assert_eq!(food.totals, vec![Some(18.0)], "the blank amount is skipped");
    assert_eq!(
        find_group(&totals, &["text:Rent"]).totals,
        vec![Some(-130.0)]
    );
    assert_eq!(
        find_group(&totals, &["text:Transport"]).totals,
        vec![Some(17.0)]
    );
    assert_eq!(find_group(&totals, &["text:food"]).totals, vec![Some(7.0)]);
    let blank = find_group(&totals, &["blank"]);
    assert_eq!(blank.rows, 2);
    assert_eq!(blank.totals, vec![Some(2.0)]);

    assert_eq!(totals.totals, vec![Some(-85.5)]);
    assert_eq!(
        totals.groups.iter().map(|group| group.rows).sum::<usize>(),
        totals.rows
    );

    let all = all_ids();
    let all = all.iter().map(String::as_str).collect::<Vec<_>>();
    let reconciled = column_totals(&mut runtime, &all);
    assert_eq!(reconciled.rows, 20);
    assert_eq!(reconciled.column_totals["f03_amount"], totals.totals[0]);
}

#[test]
fn two_dimensions_group_lexicographically_and_a_blank_date_has_its_own_group() {
    let mut runtime = group_runtime();
    let totals = group_totals(
        &mut runtime,
        vec![
            value_dimension("f01_category"),
            bucket_dimension("f02_when", GroupBucket::Month),
        ],
        &["f03_amount"],
        all_rows(),
    )
    .expect("Category by Month is a valid two-dimension query");

    // First dimension first, then the month; a blank key sorts last in its
    // position, so (blank category, blank date) is the final group.
    let labels = totals
        .groups
        .iter()
        .map(|group| group_labels(group).join("|"))
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "text:|month:2026-02",
            "text:|month:2026-03",
            "text:Food|month:2026-01",
            "text:Food|month:2026-02",
            "text:Food|month:2026-03",
            "text:Rent|month:2026-01",
            "text:Rent|month:2026-02",
            "text:Rent|month:2026-03",
            "text:Transport|month:2025-12",
            "text:Transport|month:2026-02",
            "text:Transport|month:2026-03",
            "text:food|month:2025-12",
            "text:food|month:2026-02",
            "blank|month:2026-01",
            "blank|blank",
        ]
    );

    assert_eq!(
        find_group(&totals, &["text:Food", "month:2026-01"]).totals,
        vec![Some(12.0)]
    );
    // e04 is the only Food row in March and its amount is blank, so the group
    // totals an empty set (0), never a partial or null sum.
    let march = find_group(&totals, &["text:Food", "month:2026-03"]);
    assert_eq!(march.rows, 1);
    assert_eq!(march.totals, vec![Some(0.0)]);
    let blank = find_group(&totals, &["blank", "blank"]);
    assert_eq!(blank.rows, 1);
    assert_eq!(blank.totals, vec![Some(0.0)]);
    assert_eq!(
        totals.groups.iter().map(|group| group.rows).sum::<usize>(),
        totals.rows
    );
}

#[test]
fn date_buckets_and_boolean_keys_are_typed_and_ordered() {
    let mut runtime = group_runtime();

    let exact = group_totals(
        &mut runtime,
        vec![value_dimension("f02_when")],
        &["f03_amount"],
        all_rows(),
    )
    .expect("an exact-date grouping is valid");
    assert_eq!(
        exact.groups.len(),
        20,
        "every row has its own date or blank"
    );
    assert_eq!(key_label(&exact.groups[0].keys[0]), "date:2025-12-05");
    assert_eq!(
        key_label(&exact.groups[exact.groups.len() - 1].keys[0]),
        "blank"
    );
    assert_eq!(
        find_group(&exact, &["date:2026-01-01"]).totals,
        vec![Some(-100.0)]
    );
    assert!(
        exact.groups[..exact.groups.len() - 1]
            .iter()
            .all(|group| matches!(group.keys[0], GroupKey::Date { .. }))
    );

    let years = group_totals(
        &mut runtime,
        vec![bucket_dimension("f02_when", GroupBucket::Year)],
        &["f03_amount"],
        all_rows(),
    )
    .expect("a year grouping is valid");
    assert_eq!(
        years
            .groups
            .iter()
            .map(|group| key_label(&group.keys[0]))
            .collect::<Vec<_>>(),
        vec!["year:2025", "year:2026", "blank"]
    );
    assert_eq!(
        years
            .groups
            .iter()
            .map(|group| group.rows)
            .collect::<Vec<_>>(),
        vec![2, 17, 1]
    );
    assert_eq!(years.totals, vec![Some(-85.5)]);

    let paid = group_totals(
        &mut runtime,
        vec![value_dimension("f04_paid")],
        &["f03_amount"],
        all_rows(),
    )
    .expect("a Yes/No grouping is valid");
    assert_eq!(
        paid.groups
            .iter()
            .map(|group| key_label(&group.keys[0]))
            .collect::<Vec<_>>(),
        vec!["boolean:false", "boolean:true", "blank"]
    );
    assert_eq!(
        paid.groups
            .iter()
            .map(|group| group.rows)
            .collect::<Vec<_>>(),
        vec![6, 10, 4]
    );
    assert_eq!(
        paid.groups
            .iter()
            .map(|group| group.totals[0])
            .collect::<Vec<_>>(),
        vec![Some(-36.0), Some(-68.5), Some(19.0)]
    );
}

#[test]
fn entity_and_except_scopes_are_complements_of_one_projection() {
    let mut runtime = group_runtime();
    let sample = ["e02", "e07", "e12", "e17"];
    let complement = all_ids()
        .into_iter()
        .filter(|id| !sample.contains(&id.as_str()))
        .collect::<Vec<_>>();
    let complement = complement.iter().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(complement.len(), 16);

    // `Entities(S)` and `Except(complement of S)` cover the same rows, so the
    // two projections are identical.
    let members = group_totals(
        &mut runtime,
        vec![value_dimension("f01_category")],
        &["f03_amount"],
        entities_scope(&sample),
    )
    .expect("four named entities are a valid scope");
    let same_rows = group_totals(
        &mut runtime,
        vec![value_dimension("f01_category")],
        &["f03_amount"],
        except_scope(&complement),
    )
    .expect("the complement of the complement is a valid scope");
    assert_eq!(members, same_rows, "the scope is not echoed, so both agree");
    assert_eq!(members.rows, 4);
    assert_eq!(members.totals, vec![Some(-52.5)]);

    let rest = group_totals(
        &mut runtime,
        vec![value_dimension("f01_category")],
        &["f03_amount"],
        except_scope(&sample),
    )
    .expect("the complement is a valid scope");
    assert_eq!(rest.rows, 16);
    assert_eq!(rest.totals, vec![Some(-33.0)]);

    let sampled = column_totals(&mut runtime, &sample);
    assert_eq!(sampled.rows, 4);
    assert_eq!(sampled.column_totals["f03_amount"], members.totals[0]);
    let reconciled = column_totals(&mut runtime, &complement);
    assert_eq!(reconciled.column_totals["f03_amount"], rest.totals[0]);
}

#[test]
fn two_measures_keep_request_order_and_their_own_blanks() {
    let mut runtime = group_runtime();
    let totals = group_totals(
        &mut runtime,
        vec![value_dimension("f01_category")],
        &["f03_amount", "f05_fee"],
        all_rows(),
    )
    .expect("two distinct Number measures are valid");

    assert_eq!(
        totals.measures,
        vec!["f03_amount".to_owned(), "f05_fee".to_owned()]
    );
    assert_eq!(totals.totals, vec![Some(-85.5), Some(16.25)]);
    assert_eq!(
        find_group(&totals, &["text:"]).totals,
        vec![Some(0.5), Some(1.25)]
    );
    assert_eq!(
        find_group(&totals, &["text:Food"]).totals,
        vec![Some(18.0), Some(5.0)]
    );
    assert_eq!(
        find_group(&totals, &["text:Rent"]).totals,
        vec![Some(-130.0), Some(3.0)]
    );
    assert_eq!(
        find_group(&totals, &["text:Transport"]).totals,
        vec![Some(17.0), Some(5.5)]
    );
    assert_eq!(
        find_group(&totals, &["text:food"]).totals,
        vec![Some(7.0), Some(1.5)]
    );
    // No blank-category row carries a fee, so its fee total is the empty-set
    // 0 while its amount total is reported.
    assert_eq!(
        find_group(&totals, &["blank"]).totals,
        vec![Some(2.0), Some(0.0)]
    );
}

#[test]
fn invalid_group_queries_are_refused_with_typed_errors() {
    let mut runtime = group_runtime();
    let refusal = |runtime: &mut DesignerRuntime,
                   dimensions: Vec<GroupDimension>,
                   measures: &[&str],
                   scope: GroupScope| {
        group_totals(runtime, dimensions, measures, scope).expect_err("the query must be refused")
    };

    for dimensions in [
        Vec::new(),
        vec![
            value_dimension("f01_category"),
            value_dimension("f02_when"),
            value_dimension("f04_paid"),
        ],
    ] {
        assert!(matches!(
            refusal(&mut runtime, dimensions, &["f03_amount"], all_rows()),
            DesignerError::InvalidGroupQuery { .. }
        ));
    }
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![value_dimension("f03_amount")],
            &["f03_amount"],
            all_rows()
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![bucket_dimension("f01_category", GroupBucket::Month)],
            &["f03_amount"],
            all_rows()
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![
                value_dimension("f01_category"),
                value_dimension("f01_category")
            ],
            &["f03_amount"],
            all_rows()
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    for measures in [
        Vec::new(),
        vec![
            "f03_amount",
            "f05_fee",
            "f03_amount",
            "f05_fee",
            "f03_amount",
        ],
    ] {
        assert!(matches!(
            refusal(
                &mut runtime,
                vec![value_dimension("f01_category")],
                &measures,
                all_rows()
            ),
            DesignerError::InvalidGroupQuery { .. }
        ));
    }
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![value_dimension("f01_category")],
            &["f01_category"],
            all_rows()
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![value_dimension("f01_category")],
            &["f03_amount", "f03_amount"],
            all_rows()
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![value_dimension("f99_missing")],
            &["f03_amount"],
            all_rows()
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![value_dimension("f01_category")],
            &["f03_amount"],
            entities_scope(&["e99"])
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));
    assert!(matches!(
        refusal(
            &mut runtime,
            vec![value_dimension("f01_category")],
            &["f03_amount"],
            except_scope(&["e01", "e01"])
        ),
        DesignerError::InvalidGroupQuery { .. }
    ));

    let stale = runtime
        .handle(DesignerRequest::QueryGroupTotals {
            expected_revision: "resident/999".to_owned(),
            collection: "rows".to_owned(),
            dimensions: vec![value_dimension("f01_category")],
            measures: vec!["f03_amount".to_owned()],
            scope: all_rows(),
        })
        .expect_err("a stale revision is refused");
    assert!(matches!(stale, DesignerError::StaleQuery { .. }));
}

fn categories_runtime(count: usize, padding: usize) -> DesignerRuntime {
    let fields = vec![
        field("f01_category", FieldType::Text, false),
        field("f02_amount", FieldType::Number, false),
    ];
    let entities = (0..count)
        .map(|row| {
            let category = if padding == 0 {
                format!("cat{row:03}")
            } else {
                format!("cat{row:03}{}", "x".repeat(padding))
            };
            entity(
                &format!("r{row:05}"),
                vec![
                    ("f01_category", text(&category)),
                    (
                        "f02_amount",
                        number(f64::from(u32::try_from(row).expect("bounded fixture row"))),
                    ),
                ],
            )
        })
        .collect::<Vec<_>>();
    DesignerRuntime::from_document(schema_document(fields, entities), CATEGORIES)
        .expect("a Text/Number rectangle past 64 rows is admitted")
}

#[test]
fn the_group_limit_is_two_hundred_and_the_reply_stays_under_64_kib() {
    let mut exactly = categories_runtime(200, 0);
    let totals = group_totals(
        &mut exactly,
        vec![value_dimension("f01_category")],
        &["f02_amount"],
        all_rows(),
    )
    .expect("200 groups fit");
    assert_eq!(totals.groups.len(), 200);
    assert_eq!(totals.rows, 200);

    let mut over = categories_runtime(201, 0);
    match group_totals(
        &mut over,
        vec![value_dimension("f01_category")],
        &["f02_amount"],
        all_rows(),
    )
    .expect_err("201 groups do not fit")
    {
        DesignerError::GroupLimitExceeded { groups, maximum } => {
            assert_eq!(groups, 201);
            assert_eq!(maximum, 200);
        }
        other => panic!("expected GroupLimitExceeded, got {other:?}"),
    }

    // 200 groups, so the count passes, but each 403-character key pushes the
    // encoded projection past the 65,536-byte budget.
    let mut long = categories_runtime(200, 400);
    match group_totals(
        &mut long,
        vec![value_dimension("f01_category")],
        &["f02_amount"],
        all_rows(),
    )
    .expect_err("the encoded reply is too large")
    {
        DesignerError::GroupTotalsTooLarge { bytes, maximum } => {
            assert_eq!(maximum, 65_536);
            assert!(bytes > 65_536, "the refused reply measured {bytes} bytes");
        }
        other => panic!("expected GroupTotalsTooLarge, got {other:?}"),
    }
}

fn wire_reply(runtime: &mut Option<DesignerRuntime>, request: &str) -> serde_json::Value {
    let request = wire_json(request);
    serde_json::from_slice(&process_wire_request(
        runtime,
        &serde_json::to_vec(&request).expect("the request encodes"),
    ))
    .expect("the reply is JSON")
}

fn wire_json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("the wire request literal is JSON")
}

fn wire_string(text: &str) -> String {
    serde_json::to_string(text).expect("a string encodes")
}

#[test]
fn group_totals_wire_request_uses_the_documented_envelope() {
    let mut runtime = Some(group_runtime());
    let revision = runtime
        .as_ref()
        .expect("the occurrence is installed")
        .observe_occurrence()
        .revision;
    let reply = wire_reply(
        &mut runtime,
        &format!(
            r#"{{
            "type": "query_group_totals",
            "expected_revision": {revision},
            "collection": "rows",
            "dimensions": [
                {{"field": "f01_category"}},
                {{"field": "f02_when", "bucket": "month"}}
            ],
            "measures": ["f03_amount"],
            "scope": {{"kind": "except", "entities": ["e01", "e02"]}}
        }}"#,
            revision = wire_string(&revision)
        ),
    );
    assert_eq!(reply["status"], "ok");
    assert_eq!(reply["response"]["type"], "group_totals");
    let payload = &reply["response"]["payload"];
    assert_eq!(payload["rows"], 18);
    assert_eq!(payload["dimensions"][1]["bucket"], "month");
    assert_eq!(payload["measures"][0], "f03_amount");

    let groups = payload["groups"].as_array().expect("groups are an array");
    let blank_category = groups
        .iter()
        .find(|group| group["keys"][0]["kind"] == "blank")
        .expect("the blank category keeps its group");
    assert!(
        blank_category["keys"][0].get("value").is_none(),
        "a blank key carries no value"
    );
    let food_march = groups
        .iter()
        .find(|group| {
            group["keys"][0]["kind"] == "text"
                && group["keys"][0]["value"] == "Food"
                && group["keys"][1]["kind"] == "month"
                && group["keys"][1]["value"] == "2026-03"
        })
        .expect("Food by March survives the excluded rows");
    assert_eq!(food_march["rows"], 1);
    assert_eq!(food_march["totals"][0], 0.0);
}

#[test]
fn group_totals_wire_refusals_use_their_codes() {
    let mut over = Some(categories_runtime(201, 0));
    let revision = over
        .as_ref()
        .expect("the occurrence is installed")
        .observe_occurrence()
        .revision;
    let reply = wire_reply(
        &mut over,
        &format!(
            r#"{{
            "type": "query_group_totals",
            "expected_revision": {revision},
            "collection": "rows",
            "dimensions": [{{"field": "f01_category"}}],
            "measures": ["f02_amount"],
            "scope": {{"kind": "all"}}
        }}"#,
            revision = wire_string(&revision)
        ),
    );
    assert_eq!(reply["status"], "error");
    assert_eq!(reply["error"]["code"], "group_limit_exceeded");

    let mut runtime = Some(group_runtime());
    let revision = runtime
        .as_ref()
        .expect("the occurrence is installed")
        .observe_occurrence()
        .revision;
    let reply = wire_reply(
        &mut runtime,
        &format!(
            r#"{{
            "type": "query_group_totals",
            "expected_revision": {revision},
            "collection": "rows",
            "dimensions": [{{"field": "f03_amount"}}],
            "measures": ["f03_amount"],
            "scope": {{"kind": "all"}}
        }}"#,
            revision = wire_string(&revision)
        ),
    );
    assert_eq!(reply["status"], "error");
    assert_eq!(reply["error"]["code"], "invalid_group_query");
}

fn large_document() -> Document {
    const ROWS: usize = 8_406;
    let fields = vec![
        field("f01_category", FieldType::Text, false),
        field("f02_when", FieldType::Date, false),
        field("f03_amount", FieldType::Number, false),
    ];
    let entities = (0..ROWS)
        .map(|row| {
            let category = format!("cat{:02}", row % 12);
            let month = (row / 12) % 12 + 1;
            let day = row % 28 + 1;
            let when = format!("2026-{month:02}-{day:02}");
            let amount = f64::from(u32::try_from(row % 97).expect("bounded fixture row")) - 48.0;
            entity(
                &format!("r{row:05}"),
                vec![
                    ("f01_category", text(&category)),
                    ("f02_when", date(&when)),
                    ("f03_amount", number(amount)),
                ],
            )
        })
        .collect::<Vec<_>>();
    schema_document(fields, entities)
}

#[test]
fn an_8_406_row_document_groups_category_by_month() {
    let mut runtime = DesignerRuntime::from_document(large_document(), LARGE)
        .expect("the largest text-capacity rectangle is admitted");
    let totals = group_totals(
        &mut runtime,
        vec![
            value_dimension("f01_category"),
            bucket_dimension("f02_when", GroupBucket::Month),
        ],
        &["f03_amount"],
        all_rows(),
    )
    .expect("144 groups fit the bound");

    assert_eq!(totals.rows, 8_406);
    assert_eq!(totals.groups.len(), 144);
    assert!(
        totals
            .groups
            .iter()
            .all(|group| matches!(group.keys[0], GroupKey::Text { .. }))
    );
    assert_eq!(
        totals.groups.iter().map(|group| group.rows).sum::<usize>(),
        8_406
    );

    let expected = (0..8_406)
        .map(|row| f64::from(u32::try_from(row % 97).expect("bounded fixture row")) - 48.0)
        .sum::<f64>();
    assert_eq!(totals.totals, vec![Some(expected)]);

    let all = (0..8_406)
        .map(|row| format!("r{row:05}"))
        .collect::<Vec<_>>();
    let all = all.iter().map(String::as_str).collect::<Vec<_>>();
    let reconciled = column_totals(&mut runtime, &all);
    assert_eq!(reconciled.rows, 8_406);
    assert_eq!(reconciled.column_totals["f03_amount"], totals.totals[0]);
}
