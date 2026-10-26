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
            number_format: None,
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
