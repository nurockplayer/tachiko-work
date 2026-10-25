//! Sheet's J2 monthly plan fixture: one `Plan` table whose reserve is a share
//! of each row's expense, priced by a single shared `Settings.Rate` row.
//!
//! The fixture is authored as a native `.roproj/v3` document and every test
//! starts from a fresh reopen of those bytes, so the assertions cover the
//! persisted formula sources rather than an in-memory authoring session.
//! Schema, field, and entity keys are canonical lowercase spellings; the
//! user-facing capitalisation lives in the display names and row items.

use std::collections::BTreeMap;

use tachiko_designer_runtime::{
    CalculationProjection, DesignerRequest, DesignerResponse, DesignerRuntime,
    DuplicateCollectionProjection, FieldProjection, FieldTarget, StoredValueProjection,
    TableProjection,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldConstraint, FieldDefinition, FieldId, FieldKey,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000600";
const REOPENED: &str = "00000000-0000-4000-8000-000000000601";
const RESAVED: &str = "00000000-0000-4000-8000-000000000602";

const RENT_RESERVE: &str = "[rent.expense] * [settings.rate]";

fn column(key: &str, name: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
    let id = FieldId::from(key);
    (
        id.clone(),
        FieldDefinition {
            id,
            key: FieldKey::from(key),
            display_name: Some(name.to_owned()),
            field_type,
            required: true,
            constraint: FieldConstraint::None,
        },
    )
}

fn count(value: f64) -> Value {
    Value::Number(Number::new(value).expect("fixture numbers are finite"))
}

fn entity(id: &str, schema: &SchemaId, fields: BTreeMap<FieldId, Value>) -> (EntityId, Entity) {
    let entity_id = EntityId::from(id);
    (
        entity_id.clone(),
        Entity {
            id: entity_id,
            key: EntityKey::from(id),
            schema: schema.clone(),
            fields,
        },
    )
}

fn plan_document() -> Document {
    let mut document = Document::empty("j2_monthly_plan", "J2 monthly plan");
    let plan_id = SchemaId::from("plan");
    document.schemas.insert(
        plan_id.clone(),
        Schema {
            id: plan_id.clone(),
            key: SchemaKey::from("plan"),
            display_name: Some("Plan".to_owned()),
            fields: BTreeMap::from([
                column("item", "Item", FieldType::Text),
                column("expense", "Expense", FieldType::Number),
                column("reserve", "Reserve", FieldType::Number),
            ]),
        },
    );
    let settings_id = SchemaId::from("settings");
    document.schemas.insert(
        settings_id.clone(),
        Schema {
            id: settings_id.clone(),
            key: SchemaKey::from("settings"),
            display_name: Some("Settings".to_owned()),
            fields: BTreeMap::from([column("rate", "Rate", FieldType::Number)]),
        },
    );
    for (key, item, expense) in [
        ("rent", "Rent", 80_000.0),
        ("ads", "Ads", 30_000.0),
        ("tools", "Tools", 10_000.0),
    ] {
        let (id, row) = entity(
            key,
            &plan_id,
            BTreeMap::from([
                (FieldId::from("item"), Value::Text(item.to_owned())),
                (FieldId::from("expense"), count(expense)),
                (FieldId::from("reserve"), count(0.0)),
            ]),
        );
        document.entities.insert(id, row);
    }
    let (id, row) = entity(
        "settings",
        &settings_id,
        BTreeMap::from([(FieldId::from("rate"), count(0.10))]),
    );
    document.entities.insert(id, row);
    let summary_id = SchemaId::from("summary");
    document.schemas.insert(
        summary_id.clone(),
        Schema {
            id: summary_id.clone(),
            key: SchemaKey::from("summary"),
            display_name: Some("Summary".to_owned()),
            fields: BTreeMap::from([column("total", "Total", FieldType::Number)]),
        },
    );
    let (id, row) = entity(
        "summary",
        &summary_id,
        BTreeMap::from([(FieldId::from("total"), count(0.0))]),
    );
    document.entities.insert(id, row);
    document
}

fn open_fixture() -> DesignerRuntime {
    let source = DesignerRuntime::from_document(plan_document(), OCCURRENCE)
        .expect("the J2 fixture document is admissible");
    let saved = source
        .export_project_v3("resident/0")
        .expect("the J2 fixture exports as v3");
    let mut reopened = None;
    tachiko_designer_runtime::open_project(&mut reopened, &saved.bytes, REOPENED)
        .expect("the saved J2 fixture reopens");
    reopened.expect("a successful open installs the occurrence")
}

fn update(runtime: &mut DesignerRuntime, expected: &str, target: &str, source: &str) -> String {
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::FormulaUpdate {
            expected_revision: expected.to_owned(),
            target: target.into(),
            source: source.to_owned(),
        })
        .expect("the formula update is accepted")
    else {
        panic!("expected a publication");
    };
    publication.resulting_revision
}

fn copy(
    runtime: &mut DesignerRuntime,
    expected: &str,
    source: &str,
    destinations: &[&str],
    fixed: &[&str],
) -> String {
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::CopyFormula {
            expected_revision: expected.to_owned(),
            source: source.into(),
            destinations: destinations.iter().map(|target| (*target).into()).collect(),
            fixed_references: fixed.iter().map(|target| (*target).into()).collect(),
            relative_rows: true,
            relative_columns: false,
        })
        .expect("the formula copy is accepted")
    else {
        panic!("expected a publication");
    };
    publication.resulting_revision
}

fn edit(runtime: &mut DesignerRuntime, expected: &str, target: FieldTarget, input: &str) -> String {
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: expected.to_owned(),
            target,
            input: tachiko_designer_runtime::ScalarEditInput::Number {
                input: input.to_owned(),
            },
        })
        .expect("the scalar edit is accepted")
    else {
        panic!("expected a publication");
    };
    publication.resulting_revision
}

fn duplicate(
    runtime: &mut DesignerRuntime,
    expected: &str,
    collection: &str,
    name: &str,
) -> DuplicateCollectionProjection {
    let DesignerResponse::Duplicated(duplicated) = runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: expected.to_owned(),
            collection: collection.to_owned(),
            name: name.to_owned(),
        })
        .expect("the collection duplicate is accepted")
    else {
        panic!("expected a duplicate projection");
    };
    duplicated
}

fn table(runtime: &mut DesignerRuntime, collection: &str) -> TableProjection {
    let DesignerResponse::Table(table) = runtime
        .handle(DesignerRequest::QueryTable {
            collection: collection.to_owned(),
        })
        .expect("the collection is queryable")
    else {
        panic!("expected a table projection");
    };
    table
}

fn row<'a>(table: &'a TableProjection, item: &str) -> &'a tachiko_designer_runtime::RowProjection {
    table
        .rows
        .iter()
        .find(|row| {
            row.fields.iter().any(|field| {
                matches!(&field.stored, Some(StoredValueProjection::Text { value }) if value == item)
            })
        })
        .unwrap_or_else(|| panic!("the plan has no '{item}' row"))
}

fn cell<'a>(table: &'a TableProjection, item: &str, column: &str) -> &'a FieldProjection {
    let column_id = table
        .columns
        .iter()
        .find(|candidate| candidate.key == column)
        .unwrap_or_else(|| panic!("the plan has no '{column}' column"))
        .id
        .as_str();
    row(table, item)
        .fields
        .iter()
        .find(|field| field.target.field == column_id)
        .unwrap_or_else(|| panic!("row '{item}' has no '{column}' value"))
}

fn number(field: &FieldProjection) -> Option<f64> {
    field
        .calculated
        .as_ref()
        .and_then(CalculationProjection::number)
}

fn source(field: &FieldProjection) -> &str {
    &field
        .formula
        .as_ref()
        .expect("the cell retains a formula source")
        .source
}

fn column_id(table: &TableProjection, column: &str) -> String {
    table
        .columns
        .iter()
        .find(|candidate| candidate.key == column)
        .unwrap_or_else(|| panic!("the table has no '{column}' column"))
        .id
        .clone()
}

fn column_total(table: &TableProjection, column: &str) -> Option<f64> {
    let id = column_id(table, column);
    *table
        .column_totals
        .get(&id)
        .unwrap_or_else(|| panic!("the table reports no total for '{column}'"))
}

fn first_row_number(table: &TableProjection, column: &str) -> Option<f64> {
    let id = column_id(table, column);
    number(
        table.rows[0]
            .fields
            .iter()
            .find(|field| field.target.field == id)
            .unwrap_or_else(|| panic!("the row has no '{column}' value")),
    )
}

fn first_row_source<'a>(table: &'a TableProjection, column: &str) -> &'a str {
    let id = column_id(table, column);
    source(
        table.rows[0]
            .fields
            .iter()
            .find(|field| field.target.field == id)
            .unwrap_or_else(|| panic!("the row has no '{column}' value")),
    )
}

#[test]
fn update_formula_multiplies_each_expense_by_the_shared_rate() {
    let mut runtime = open_fixture();
    assert_eq!(
        update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE),
        "resident/1"
    );
    let plan = table(&mut runtime, "plan");
    let reserve = cell(&plan, "Rent", "reserve");
    assert_eq!(source(reserve), "([rent.expense] * [settings.rate])");
    assert_eq!(number(reserve), Some(8_000.0));
}

#[test]
fn copy_formula_shifts_rows_and_keeps_the_rate_fixed() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);
    assert_eq!(
        copy(
            &mut runtime,
            "resident/1",
            "rent.reserve",
            &["ads.reserve", "tools.reserve"],
            &["settings.rate"]
        ),
        "resident/2"
    );
    let plan = table(&mut runtime, "plan");
    assert_eq!(
        source(cell(&plan, "Rent", "reserve")),
        "([rent.expense] * [settings.rate])"
    );
    assert_eq!(
        source(cell(&plan, "Ads", "reserve")),
        "([ads.expense] * [settings.rate])"
    );
    assert_eq!(
        source(cell(&plan, "Tools", "reserve")),
        "([tools.expense] * [settings.rate])"
    );
    assert_eq!(number(cell(&plan, "Rent", "reserve")), Some(8_000.0));
    assert_eq!(number(cell(&plan, "Ads", "reserve")), Some(3_000.0));
    assert_eq!(number(cell(&plan, "Tools", "reserve")), Some(1_000.0));
}

#[test]
fn saved_v3_formulas_reopen_and_recompute_without_stored_results() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);
    copy(
        &mut runtime,
        "resident/1",
        "rent.reserve",
        &["ads.reserve", "tools.reserve"],
        &["settings.rate"],
    );
    let saved = runtime
        .export_project_v3("resident/2")
        .expect("formula sources export as v3");

    let mut reopened = None;
    tachiko_designer_runtime::open_project(&mut reopened, &saved.bytes, RESAVED)
        .expect("the resaved project reopens");
    let mut reopened = reopened.expect("a successful open installs the occurrence");
    let plan = table(&mut reopened, "plan");
    assert_eq!(plan.revision, "resident/0");
    assert_eq!(
        source(cell(&plan, "Rent", "reserve")),
        "([rent.expense] * [settings.rate])"
    );
    assert_eq!(
        source(cell(&plan, "Ads", "reserve")),
        "([ads.expense] * [settings.rate])"
    );
    assert_eq!(
        source(cell(&plan, "Tools", "reserve")),
        "([tools.expense] * [settings.rate])"
    );
    assert_eq!(number(cell(&plan, "Rent", "reserve")), Some(8_000.0));
    assert_eq!(number(cell(&plan, "Ads", "reserve")), Some(3_000.0));

    // No persisted calculated value is trusted: a changed input recomputes.
    assert_eq!(
        edit(&mut reopened, "resident/0", "rent.expense".into(), "90000"),
        "resident/1"
    );
    let recomputed = table(&mut reopened, "plan");
    assert_eq!(number(cell(&recomputed, "Rent", "reserve")), Some(9_000.0));
    assert_eq!(number(cell(&recomputed, "Ads", "reserve")), Some(3_000.0));
    assert_eq!(number(cell(&recomputed, "Tools", "reserve")), Some(1_000.0));
}

/// The same-row shorthand: each formula means "this row's expense", so one
/// authored source can be copied and duplicated without per-row rewrites.
const SAME_ROW_RESERVE: &str = "[.expense] * [settings.rate]";
const SAME_ROW_SOURCE: &str = "([.expense] * [settings.rate])";

#[test]
fn same_row_reserve_shorthand_survives_copy_duplicate_and_v3_reopen() {
    let mut runtime = open_fixture();
    assert_eq!(
        update(&mut runtime, "resident/0", "rent.reserve", SAME_ROW_RESERVE),
        "resident/1"
    );
    let plan = table(&mut runtime, "plan");
    assert_eq!(source(cell(&plan, "Rent", "reserve")), SAME_ROW_SOURCE);
    assert_eq!(number(cell(&plan, "Rent", "reserve")), Some(8_000.0));

    // copyFormula accepts the shorthand unchanged: no per-row address rewrite.
    assert_eq!(
        copy(
            &mut runtime,
            "resident/1",
            "rent.reserve",
            &["ads.reserve", "tools.reserve"],
            &[]
        ),
        "resident/2"
    );
    let plan = table(&mut runtime, "plan");
    for (item, expected) in [("Rent", 8_000.0), ("Ads", 3_000.0), ("Tools", 1_000.0)] {
        assert_eq!(
            source(cell(&plan, item, "reserve")),
            SAME_ROW_SOURCE,
            "{item}"
        );
        assert_eq!(
            number(cell(&plan, item, "reserve")),
            Some(expected),
            "{item}"
        );
    }

    // duplicateCollection keeps the shorthand pointing at the copy's own rows.
    let duplicated = duplicate(&mut runtime, "resident/2", "plan", "Plan copy");
    let copy_key = duplicated.collection.key.clone();
    let copy_table = table(&mut runtime, &copy_key);
    for item in ["Rent", "Ads", "Tools"] {
        assert_eq!(
            source(cell(&copy_table, item, "reserve")),
            SAME_ROW_SOURCE,
            "{item}"
        );
    }
    assert_eq!(number(cell(&copy_table, "Ads", "reserve")), Some(3_000.0));

    let copy_ads_expense = cell(&copy_table, "Ads", "expense").target.clone();
    assert_eq!(
        edit(&mut runtime, "resident/3", copy_ads_expense, "35000"),
        "resident/4"
    );
    let copy_after = table(&mut runtime, &copy_key);
    assert_eq!(number(cell(&copy_after, "Ads", "reserve")), Some(3_500.0));
    let plan = table(&mut runtime, "plan");
    assert_eq!(number(cell(&plan, "Ads", "reserve")), Some(3_000.0));

    // v3 keeps the field identity only; reopening resolves it per owner row.
    let saved = runtime
        .export_project_v3("resident/4")
        .expect("same-row sources export as v3");
    let mut reopened = None;
    tachiko_designer_runtime::open_project(&mut reopened, &saved.bytes, RESAVED)
        .expect("the saved same-row project reopens");
    let mut reopened = reopened.expect("a successful open installs the occurrence");
    assert_eq!(
        edit(&mut reopened, "resident/0", "rent.expense".into(), "90000"),
        "resident/1"
    );
    let recomputed = table(&mut reopened, "plan");
    for (item, expected) in [("Rent", 9_000.0), ("Ads", 3_000.0), ("Tools", 1_000.0)] {
        assert_eq!(
            source(cell(&recomputed, item, "reserve")),
            SAME_ROW_SOURCE,
            "{item}"
        );
        assert_eq!(
            number(cell(&recomputed, item, "reserve")),
            Some(expected),
            "{item}"
        );
    }
}

#[test]
fn same_row_refusals_keep_their_typed_codes_and_change_nothing() {
    let mut runtime = open_fixture();
    for (formula, expected_code) in [
        ("[.reserve]", "formula_cycle"),
        ("[.missing]", "formula_unknown_reference"),
        ("[.item]", "formula_type_mismatch"),
    ] {
        let error = runtime
            .handle(DesignerRequest::FormulaUpdate {
                expected_revision: "resident/0".to_owned(),
                target: "rent.reserve".into(),
                source: formula.to_owned(),
            })
            .expect_err("the shorthand refusal is typed");
        let failure = error.failure_projection("resident/0");
        assert_eq!(failure.code, expected_code, "{formula}");
        assert!(!failure.message.is_empty(), "{formula}");
    }
    let plan = table(&mut runtime, "plan");
    assert_eq!(plan.revision, "resident/0");
    assert_eq!(
        cell(&plan, "Rent", "reserve").stored,
        Some(StoredValueProjection::Number { value: 0.0 })
    );
    assert_eq!(cell(&plan, "Rent", "reserve").formula, None);
}

#[test]
fn duplicate_collection_rewrites_intra_refs_and_keeps_the_shared_rate() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);
    copy(
        &mut runtime,
        "resident/1",
        "rent.reserve",
        &["ads.reserve", "tools.reserve"],
        &["settings.rate"],
    );
    let duplicated = duplicate(&mut runtime, "resident/2", "plan", "Plan copy");
    assert_eq!(duplicated.collection.display_name, "Plan copy");
    assert_eq!(duplicated.publication.resulting_revision, "resident/3");
    let copy_key = duplicated.collection.key.clone();
    assert_ne!(copy_key, "plan");

    let copy_table = table(&mut runtime, &copy_key);
    for item in ["Rent", "Ads", "Tools"] {
        assert_eq!(
            source(cell(&copy_table, item, "reserve")),
            format!(
                "([{}.expense] * [settings.rate])",
                row(&copy_table, item).key
            ),
            "the copied {item} reserve reads its copied expense through the shared rate"
        );
    }
    assert_eq!(number(cell(&copy_table, "Rent", "reserve")), Some(8_000.0));
    assert_eq!(number(cell(&copy_table, "Ads", "reserve")), Some(3_000.0));
    assert_eq!(number(cell(&copy_table, "Tools", "reserve")), Some(1_000.0));

    // `settings` is external to the copied collection: one shared rate row.
    let settings = table(&mut runtime, "settings");
    assert_eq!(settings.collection.entity_count, 1);

    // Editing a copied input recomputes the copy only.
    let copy_ads_expense = cell(&copy_table, "Ads", "expense").target.clone();
    assert_eq!(
        edit(&mut runtime, "resident/3", copy_ads_expense, "35000"),
        "resident/4"
    );
    let copy_after = table(&mut runtime, &copy_key);
    assert_eq!(number(cell(&copy_after, "Rent", "reserve")), Some(8_000.0));
    assert_eq!(number(cell(&copy_after, "Ads", "reserve")), Some(3_500.0));
    assert_eq!(number(cell(&copy_after, "Tools", "reserve")), Some(1_000.0));
    let plan = table(&mut runtime, "plan");
    assert_eq!(number(cell(&plan, "Rent", "reserve")), Some(8_000.0));
    assert_eq!(number(cell(&plan, "Ads", "reserve")), Some(3_000.0));
    assert_eq!(number(cell(&plan, "Tools", "reserve")), Some(1_000.0));

    // A second copied input proves the other copied rows keep their own refs.
    let copy_tools_expense = cell(&copy_after, "Tools", "expense").target.clone();
    assert_eq!(
        edit(&mut runtime, "resident/4", copy_tools_expense, "12000"),
        "resident/5"
    );
    let copy_tweaked = table(&mut runtime, &copy_key);
    assert_eq!(
        number(cell(&copy_tweaked, "Rent", "reserve")),
        Some(8_000.0)
    );
    assert_eq!(number(cell(&copy_tweaked, "Ads", "reserve")), Some(3_500.0));
    assert_eq!(
        number(cell(&copy_tweaked, "Tools", "reserve")),
        Some(1_200.0)
    );
    let plan = table(&mut runtime, "plan");
    assert_eq!(number(cell(&plan, "Tools", "reserve")), Some(1_000.0));
    assert_eq!(number(cell(&plan, "Ads", "reserve")), Some(3_000.0));
}

#[test]
fn column_totals_project_per_table_and_feed_a_cross_table_summary() {
    let mut runtime = open_fixture();
    assert_eq!(
        update(&mut runtime, "resident/0", "rent.reserve", SAME_ROW_RESERVE),
        "resident/1"
    );
    copy(
        &mut runtime,
        "resident/1",
        "rent.reserve",
        &["ads.reserve", "tools.reserve"],
        &[],
    );

    let plan = table(&mut runtime, "plan");
    assert_eq!(column_total(&plan, "reserve"), Some(12_000.0));
    assert_eq!(column_total(&plan, "expense"), Some(120_000.0));

    let duplicated = duplicate(&mut runtime, "resident/2", "plan", "Plan copy");
    let copy_key = duplicated.collection.key.clone();
    let copy_table = table(&mut runtime, &copy_key);
    assert_eq!(column_total(&copy_table, "reserve"), Some(12_000.0));

    // Editing the copy moves the copy's footer only.
    let copy_ads_expense = cell(&copy_table, "Ads", "expense").target.clone();
    assert_eq!(
        edit(&mut runtime, "resident/3", copy_ads_expense, "35000"),
        "resident/4"
    );
    let copy_table = table(&mut runtime, &copy_key);
    assert_eq!(column_total(&copy_table, "reserve"), Some(12_500.0));
    let plan = table(&mut runtime, "plan");
    assert_eq!(column_total(&plan, "reserve"), Some(12_000.0));

    // The summary reads both tables' totals, keyed by table key rather than by
    // any row.
    let summary_target = "summary.total";
    let summary_source = format!("[plan.reserve:total] + [{copy_key}.reserve:total]");
    assert_eq!(
        update(&mut runtime, "resident/4", summary_target, &summary_source),
        "resident/5"
    );
    let summary = table(&mut runtime, "summary");
    assert_eq!(first_row_number(&summary, "total"), Some(24_500.0));
    assert_eq!(
        first_row_source(&summary, "total"),
        format!("([plan.reserve:total] + [{copy_key}.reserve:total])")
    );

    // A display rename changes no identity: the total and its source stay.
    let copy_reserve = column_id(&copy_table, "reserve");
    let DesignerResponse::Published(publication) = runtime
        .handle(DesignerRequest::RenameColumn {
            expected_revision: "resident/5".to_owned(),
            collection: copy_key.clone(),
            field: copy_reserve,
            name: "Buffer".to_owned(),
        })
        .expect("renaming a copied column is accepted")
    else {
        panic!("expected a publication");
    };
    assert_eq!(publication.resulting_revision, "resident/6");
    let summary = table(&mut runtime, "summary");
    assert_eq!(first_row_number(&summary, "total"), Some(24_500.0));
    assert_eq!(
        first_row_source(&summary, "total"),
        format!("([plan.reserve:total] + [{copy_key}.reserve:total])")
    );

    // v3 keeps the column identities; reopening recomputes the totals.
    let saved = runtime
        .export_project_v3("resident/6")
        .expect("column totals export as v3");
    let mut reopened = None;
    tachiko_designer_runtime::open_project(&mut reopened, &saved.bytes, RESAVED)
        .expect("the saved column-total project reopens");
    let mut reopened = reopened.expect("a successful open installs the occurrence");
    let summary = table(&mut reopened, "summary");
    assert_eq!(first_row_number(&summary, "total"), Some(24_500.0));
    let plan = table(&mut reopened, "plan");
    assert_eq!(column_total(&plan, "reserve"), Some(12_000.0));
    let copy_table = table(&mut reopened, &copy_key);
    assert_eq!(column_total(&copy_table, "reserve"), Some(12_500.0));

    // No persisted total is trusted: a changed input recomputes both tables
    // and the summary.
    let copy_ads_expense = cell(&copy_table, "Ads", "expense").target.clone();
    assert_eq!(
        edit(&mut reopened, "resident/0", copy_ads_expense, "40000"),
        "resident/1"
    );
    let copy_table = table(&mut reopened, &copy_key);
    assert_eq!(column_total(&copy_table, "reserve"), Some(13_000.0));
    let summary = table(&mut reopened, "summary");
    assert_eq!(first_row_number(&summary, "total"), Some(25_000.0));
}

#[test]
fn column_total_refusals_keep_their_typed_codes_and_change_nothing() {
    let mut runtime = open_fixture();
    for (target, formula, expected_code) in [
        ("rent.reserve", "[plan.reserve:total]", "formula_cycle"),
        (
            "summary.total",
            "[plan.missing:total]",
            "formula_unknown_reference",
        ),
        (
            "summary.total",
            "[plan.item:total]",
            "formula_type_mismatch",
        ),
    ] {
        let error = runtime
            .handle(DesignerRequest::FormulaUpdate {
                expected_revision: "resident/0".to_owned(),
                target: target.into(),
                source: formula.to_owned(),
            })
            .expect_err("the column-total refusal is typed");
        let failure = error.failure_projection("resident/0");
        assert_eq!(failure.code, expected_code, "{formula}");
        assert!(!failure.message.is_empty(), "{formula}");
    }
    let plan = table(&mut runtime, "plan");
    assert_eq!(plan.revision, "resident/0");
    assert_eq!(
        cell(&plan, "Rent", "reserve").stored,
        Some(StoredValueProjection::Number { value: 0.0 })
    );
}
