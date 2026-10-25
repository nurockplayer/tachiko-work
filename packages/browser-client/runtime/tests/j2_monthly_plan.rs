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
    CalculationProjection, ColumnProjection, DesignerRequest, DesignerResponse, DesignerRuntime,
    DuplicateCollectionProjection, FieldProjection, FieldTarget, NewTableColumnInput,
    PasteCellInput, RowInitializer, ScalarEditInput, StoredValueProjection, TableProjection,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldRef, FieldType, Number, Schema, SchemaId, SchemaKey, Value,
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

fn table_column<'a>(table: &'a TableProjection, key: &str) -> &'a ColumnProjection {
    table
        .columns
        .iter()
        .find(|candidate| candidate.key == key)
        .unwrap_or_else(|| panic!("the plan has no '{key}' column"))
}

fn published(runtime: &mut DesignerRuntime, request: DesignerRequest) -> String {
    let DesignerResponse::Published(publication) =
        runtime.handle(request).expect("the request is accepted")
    else {
        panic!("expected a publication");
    };
    publication.resulting_revision
}

fn insert(
    runtime: &mut DesignerRuntime,
    expected: &str,
    collection: &str,
    initializers: Vec<RowInitializer>,
) -> String {
    published(
        runtime,
        DesignerRequest::InsertRow {
            expected_revision: expected.to_owned(),
            collection: collection.to_owned(),
            initializers,
        },
    )
}

fn number_input(field: &str, value: &str) -> RowInitializer {
    RowInitializer {
        field: field.to_owned(),
        input: ScalarEditInput::Number {
            input: value.to_owned(),
        },
    }
}

fn text_input(field: &str, value: &str) -> RowInitializer {
    RowInitializer {
        field: field.to_owned(),
        input: ScalarEditInput::Text {
            value: value.to_owned(),
        },
    }
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

fn projected<'a>(
    table: &'a TableProjection,
    row_key: &str,
    column_key: &str,
) -> &'a FieldProjection {
    let column_id = table_column(table, column_key).id.as_str();
    table
        .rows
        .iter()
        .find(|row| row.key == row_key)
        .unwrap_or_else(|| panic!("the plan has no '{row_key}' row"))
        .fields
        .iter()
        .find(|field| field.target.field == column_id)
        .unwrap_or_else(|| panic!("row '{row_key}' has no '{column_key}' value"))
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

const OPTIONAL_PLAN: &str = "00000000-0000-4000-8000-000000000603";

/// A native table whose formula column is optional: the Sheet shape that lost
/// all authoring as soon as one cell held a formula.
fn optional_formula_plan() -> DesignerRuntime {
    let mut runtime = DesignerRuntime::new_table(
        OPTIONAL_PLAN,
        "Plan",
        &[
            NewTableColumnInput {
                name: "Expense".to_owned(),
                field_type: "number".to_owned(),
                required: true,
            },
            NewTableColumnInput {
                name: "Reserve".to_owned(),
                field_type: "number".to_owned(),
                required: false,
            },
        ],
    )
    .expect("the optional-reserve table is admitted");
    let initial = table(&mut runtime, "plan");
    let expense = table_column(&initial, "expense").id.clone();
    let reserve = table_column(&initial, "reserve").id.clone();
    assert_eq!(
        insert(
            &mut runtime,
            "resident/0",
            "plan",
            vec![number_input(&expense, "10"), number_input(&reserve, "0")]
        ),
        "resident/1"
    );
    let plan = table(&mut runtime, "plan");
    let row = &plan.rows[0];
    update(
        &mut runtime,
        "resident/1",
        &format!("{}.{reserve}", row.id),
        &format!("[{}.expense] * 2", row.key),
    );
    runtime
}

#[test]
fn formula_cell_keeps_native_row_and_column_authoring() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);
    let plan = table(&mut runtime, "plan");
    assert_eq!(
        plan.native_table_profile,
        Some(true),
        "a formula cell must not remove the native table profile"
    );

    let item = table_column(&plan, "item").id.clone();
    let expense = table_column(&plan, "expense").id.clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    assert_eq!(
        insert(
            &mut runtime,
            "resident/1",
            "plan",
            vec![
                text_input(&item, "Travel"),
                number_input(&expense, "1000"),
                number_input(&reserve, "0"),
            ],
        ),
        "resident/2"
    );
    let plan = table(&mut runtime, "plan");
    let added = row(&plan, "Travel");
    assert!(
        added.fields.iter().all(|field| field.formula.is_none()),
        "a new row never copies formulas"
    );
    assert_eq!(
        cell(&plan, "Travel", "reserve").stored,
        Some(StoredValueProjection::Number { value: 0.0 })
    );

    let destination = format!("{}.{reserve}", added.id);
    assert_eq!(
        copy(
            &mut runtime,
            "resident/2",
            "rent.reserve",
            &[destination.as_str()],
            &["settings.rate"],
        ),
        "resident/3"
    );
    let plan = table(&mut runtime, "plan");
    assert_eq!(
        source(cell(&plan, "Travel", "reserve")),
        "([row_0001.expense] * [settings.rate])"
    );
    assert_eq!(number(cell(&plan, "Travel", "reserve")), Some(100.0));
}

#[test]
fn formula_cell_keeps_column_lifecycle_and_required_clear_refusal() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);

    // Column lifecycle stays available on the formula-bearing table.
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::AddColumn {
                expected_revision: "resident/1".to_owned(),
                collection: "plan".to_owned(),
                name: "Notes".to_owned(),
                field_type: "text".to_owned(),
                required: false,
                initializers: Vec::new(),
            },
        ),
        "resident/2"
    );
    let plan = table(&mut runtime, "plan");
    assert_eq!(table_column(&plan, "notes").display_name, "Notes");
    assert_eq!(plan.native_table_profile, Some(true));

    // Undo/redo across the column addition keeps the formula sources.
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::Undo {
                expected_revision: "resident/2".to_owned(),
            },
        ),
        "resident/3"
    );
    let undone = table(&mut runtime, "plan");
    assert_eq!(
        source(cell(&undone, "Rent", "reserve")),
        "([rent.expense] * [settings.rate])"
    );
    assert!(
        undone.columns.iter().all(|column| column.key != "notes"),
        "undo removes the authored column"
    );
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::Redo {
                expected_revision: "resident/3".to_owned(),
            },
        ),
        "resident/4"
    );
    let redone = table(&mut runtime, "plan");
    assert_eq!(table_column(&redone, "notes").display_name, "Notes");
    assert_eq!(
        source(cell(&redone, "Rent", "reserve")),
        "([rent.expense] * [settings.rate])"
    );

    // Reserve is required: clearing would leave the column blank.
    let error = runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/4".to_owned(),
            target: "rent.reserve".into(),
            input: ScalarEditInput::Clear,
        })
        .expect_err("a required formula column cannot be cleared");
    let failure = error.failure_projection("resident/4");
    assert_eq!(failure.code, "invalid_table_operation");
    assert!(
        failure
            .message
            .contains("clear requires an optional Text or Number column")
    );
    assert_eq!(runtime.observe_occurrence().revision, "resident/4");
}

#[test]
fn optional_formula_column_starts_new_rows_blank_and_clears() {
    let mut runtime = optional_formula_plan();
    let plan = table(&mut runtime, "plan");
    assert_eq!(plan.native_table_profile, Some(true));
    let first_key = plan.rows[0].key.clone();
    let first_id = plan.rows[0].id.clone();
    assert_eq!(number(projected(&plan, &first_key, "reserve")), Some(20.0));

    let expense = table_column(&plan, "expense").id.clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    assert_eq!(
        insert(
            &mut runtime,
            "resident/2",
            "plan",
            vec![number_input(&expense, "4")]
        ),
        "resident/3"
    );
    let plan = table(&mut runtime, "plan");
    let second = plan
        .rows
        .iter()
        .find(|row| row.key != first_key)
        .expect("the second row is present");
    assert!(
        second.blank_fields.contains(&reserve),
        "an optional formula column starts blank in a new row"
    );
    assert!(second.fields.iter().all(|field| field.formula.is_none()));

    // Clear removes an optional cell's formula.
    let target = format!("{first_id}.{reserve}");
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::EditScalar {
                expected_revision: "resident/3".to_owned(),
                target: target.as_str().into(),
                input: ScalarEditInput::Clear,
            },
        ),
        "resident/4"
    );
    let plan = table(&mut runtime, "plan");
    assert!(
        plan.rows
            .iter()
            .find(|row| row.key == first_key)
            .expect("the first row is present")
            .blank_fields
            .contains(&reserve)
    );
    assert_eq!(plan.native_table_profile, Some(true));

    // A cleared optional cell accepts a typed scalar again.
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::EditScalar {
                expected_revision: "resident/4".to_owned(),
                target: target.as_str().into(),
                input: ScalarEditInput::Number {
                    input: "7".to_owned(),
                },
            },
        ),
        "resident/5"
    );
    let plan = table(&mut runtime, "plan");
    let refilled = projected(&plan, &first_key, "reserve");
    assert_eq!(
        refilled.stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert!(refilled.formula.is_none());
    assert_eq!(plan.native_table_profile, Some(true));
}

#[test]
fn scalar_edit_replaces_a_live_formula() {
    let mut runtime = optional_formula_plan();
    let plan = table(&mut runtime, "plan");
    let row_key = plan.rows[0].key.clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    let target = format!("{}.{reserve}", plan.rows[0].id);
    update(
        &mut runtime,
        "resident/2",
        &target,
        &format!("[{row_key}.expense] * 3"),
    );

    // The typed scalar takes the formula's place, and the table keeps all of
    // its native authoring.
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::EditScalar {
                expected_revision: "resident/3".to_owned(),
                target: target.as_str().into(),
                input: ScalarEditInput::Number {
                    input: "5".to_owned(),
                },
            },
        ),
        "resident/4"
    );
    let plan = table(&mut runtime, "plan");
    let replaced = projected(&plan, &row_key, "reserve");
    assert_eq!(
        replaced.stored,
        Some(StoredValueProjection::Number { value: 5.0 })
    );
    assert!(replaced.formula.is_none());
    assert_eq!(plan.native_table_profile, Some(true));
}

#[test]
fn history_keeps_formula_sources_across_clear_and_scalar_replacement() {
    let mut runtime = optional_formula_plan();
    let plan = table(&mut runtime, "plan");
    let row_key = plan.rows[0].key.clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    let target = format!("{}.{reserve}", plan.rows[0].id);
    let formula = format!("([{row_key}.expense] * 2)");
    let clear = |runtime: &mut DesignerRuntime, expected: &str| {
        published(
            runtime,
            DesignerRequest::EditScalar {
                expected_revision: expected.to_owned(),
                target: target.as_str().into(),
                input: ScalarEditInput::Clear,
            },
        )
    };
    let set = |runtime: &mut DesignerRuntime, expected: &str, input: &str| {
        published(
            runtime,
            DesignerRequest::EditScalar {
                expected_revision: expected.to_owned(),
                target: target.as_str().into(),
                input: ScalarEditInput::Number {
                    input: input.to_owned(),
                },
            },
        )
    };
    let undo = |runtime: &mut DesignerRuntime, expected: &str| {
        published(
            runtime,
            DesignerRequest::Undo {
                expected_revision: expected.to_owned(),
            },
        )
    };
    let redo = |runtime: &mut DesignerRuntime, expected: &str| {
        published(
            runtime,
            DesignerRequest::Redo {
                expected_revision: expected.to_owned(),
            },
        )
    };

    assert_eq!(clear(&mut runtime, "resident/2"), "resident/3");
    // Undo restores the exact source; redo records the clear again.
    assert_eq!(undo(&mut runtime, "resident/3"), "resident/4");
    let restored = table(&mut runtime, "plan");
    assert_eq!(source(projected(&restored, &row_key, "reserve")), formula);
    assert_eq!(
        number(projected(&restored, &row_key, "reserve")),
        Some(20.0)
    );
    assert_eq!(redo(&mut runtime, "resident/4"), "resident/5");
    let cleared = table(&mut runtime, "plan");
    let cleared_row = cleared
        .rows
        .iter()
        .find(|row| row.key == row_key)
        .expect("the first row is present");
    assert!(
        cleared_row.blank_fields.contains(&reserve),
        "redo clears the cell again"
    );

    // A scalar replaces a live formula; undo restores the source.
    assert_eq!(undo(&mut runtime, "resident/5"), "resident/6");
    assert_eq!(set(&mut runtime, "resident/6", "7"), "resident/7");
    let replaced = table(&mut runtime, "plan");
    let scalar = projected(&replaced, &row_key, "reserve");
    assert_eq!(
        scalar.stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert!(scalar.formula.is_none());
    assert_eq!(undo(&mut runtime, "resident/7"), "resident/8");
    let restored = table(&mut runtime, "plan");
    assert_eq!(source(projected(&restored, &row_key, "reserve")), formula);
    assert_eq!(
        number(projected(&restored, &row_key, "reserve")),
        Some(20.0)
    );
    assert_eq!(redo(&mut runtime, "resident/8"), "resident/9");
    let redone = table(&mut runtime, "plan");
    assert_eq!(
        projected(&redone, &row_key, "reserve").stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert_eq!(redone.native_table_profile, Some(true));
}

#[test]
fn removals_refuse_while_a_formula_still_references_them() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);
    copy(
        &mut runtime,
        "resident/1",
        "rent.reserve",
        &["ads.reserve"],
        &["settings.rate"],
    );
    let plan = table(&mut runtime, "plan");
    let expense = table_column(&plan, "expense").id.clone();

    // Two retained formulas read their own row's Expense column.
    let error = runtime
        .handle(DesignerRequest::RemoveColumn {
            expected_revision: "resident/2".to_owned(),
            collection: "plan".to_owned(),
            field: expense,
        })
        .expect_err("a column a retained formula reads cannot be removed");
    let failure = error.failure_projection("resident/2");
    assert_eq!(failure.code, "formula_dependency_in_use");
    assert!(failure.message.contains("2 formula cell(s)"));
    assert!(failure.message.contains("column 'Expense'"));
    assert_eq!(failure.diagnostics[0].facts[0].name, "dependent_cells");
    assert_eq!(failure.diagnostics[0].facts[0].value, "2");
    assert_eq!(runtime.observe_occurrence().revision, "resident/2");

    // The shared Settings row is read by both formulas too.
    let settings = table(&mut runtime, "settings");
    let error = runtime
        .handle(DesignerRequest::RemoveTableRows {
            expected_revision: "resident/2".to_owned(),
            collection: "settings".to_owned(),
            entities: vec![settings.rows[0].id.clone()],
        })
        .expect_err("a row a retained formula reads cannot be removed");
    let failure = error.failure_projection("resident/2");
    assert_eq!(failure.code, "formula_dependency_in_use");
    assert!(failure.message.contains("2 formula cell(s)"));
    assert!(
        failure
            .message
            .contains("removed rows in collection 'Settings'")
    );

    // Removing the row that owns the only retained formula is allowed: its
    // dependent cell is removed together with it.
    let rent = table(&mut runtime, "plan")
        .rows
        .iter()
        .find(|row| row.key == "rent")
        .expect("the Rent row is present")
        .id
        .clone();
    assert_eq!(
        published(
            &mut runtime,
            DesignerRequest::RemoveTableRows {
                expected_revision: "resident/2".to_owned(),
                collection: "plan".to_owned(),
                entities: vec![rent],
            },
        ),
        "resident/3"
    );
    let plan = table(&mut runtime, "plan");
    assert!(plan.rows.iter().all(|row| row.key != "rent"));
    assert_eq!(
        source(cell(&plan, "Ads", "reserve")),
        "([ads.expense] * [settings.rate])"
    );
    assert_eq!(plan.native_table_profile, Some(true));
}

#[test]
fn saved_formula_table_reopens_with_authoring_intact() {
    let mut runtime = open_fixture();
    update(&mut runtime, "resident/0", "rent.reserve", RENT_RESERVE);
    let plan = table(&mut runtime, "plan");
    let expense = table_column(&plan, "expense").id.clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    insert(
        &mut runtime,
        "resident/1",
        "plan",
        vec![
            text_input(&table_column(&plan, "item").id.clone(), "Travel"),
            number_input(&expense, "1000"),
            number_input(&reserve, "0"),
        ],
    );
    let saved = runtime
        .export_project_v3("resident/2")
        .expect("a formula-bearing native table exports as v3");

    let mut reopened = None;
    tachiko_designer_runtime::open_project(&mut reopened, &saved.bytes, RESAVED)
        .expect("the saved project reopens");
    let mut reopened = reopened.expect("a successful open installs the occurrence");
    let plan = table(&mut reopened, "plan");
    assert_eq!(plan.native_table_profile, Some(true));
    assert_eq!(
        source(cell(&plan, "Rent", "reserve")),
        "([rent.expense] * [settings.rate])"
    );
    assert_eq!(number(cell(&plan, "Rent", "reserve")), Some(8_000.0));

    // The reopened table is still authorable.
    let item = table_column(&plan, "item").id.clone();
    let expense = table_column(&plan, "expense").id.clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    assert_eq!(
        insert(
            &mut reopened,
            "resident/0",
            "plan",
            vec![
                text_input(&item, "Meals"),
                number_input(&expense, "500"),
                number_input(&reserve, "0"),
            ],
        ),
        "resident/1"
    );
    let plan = table(&mut reopened, "plan");
    assert!(plan.rows.iter().any(|row| row.key == "row_0001"));
    assert_eq!(plan.native_table_profile, Some(true));
}

fn undo(runtime: &mut DesignerRuntime, expected: &str) -> String {
    published(
        runtime,
        DesignerRequest::Undo {
            expected_revision: expected.to_owned(),
        },
    )
}

#[test]
fn paste_over_a_formula_cell_uses_the_same_edit_and_history_rules() {
    let mut runtime = optional_formula_plan();
    let plan = table(&mut runtime, "plan");
    let row = plan.rows[0].clone();
    let reserve = table_column(&plan, "reserve").id.clone();
    let formula = format!("([{}.expense] * 2)", row.key);
    let paste = |runtime: &mut DesignerRuntime, expected: &str, cell: PasteCellInput| {
        published(
            runtime,
            DesignerRequest::PasteCells {
                expected_revision: expected.to_owned(),
                collection: "plan".to_owned(),
                start_entity: Some(row.id.clone()),
                start_field: reserve.clone(),
                rows: vec![vec![cell]],
            },
        )
    };

    // A pasted clear removes the formula; Undo reinstalls the exact source.
    assert_eq!(
        paste(
            &mut runtime,
            "resident/2",
            PasteCellInput::Typed(ScalarEditInput::Clear)
        ),
        "resident/3"
    );
    let cleared = table(&mut runtime, "plan");
    assert!(cleared.rows[0].blank_fields.contains(&reserve));
    assert_eq!(undo(&mut runtime, "resident/3"), "resident/4");
    let restored = table(&mut runtime, "plan");
    assert_eq!(source(projected(&restored, &row.key, "reserve")), formula);
    assert_eq!(
        number(projected(&restored, &row.key, "reserve")),
        Some(20.0)
    );

    // A pasted scalar replaces the formula; Undo brings the source back.
    assert_eq!(
        paste(
            &mut runtime,
            "resident/4",
            PasteCellInput::Text("7".to_owned())
        ),
        "resident/5"
    );
    let replaced = table(&mut runtime, "plan");
    let scalar = projected(&replaced, &row.key, "reserve");
    assert_eq!(
        scalar.stored,
        Some(StoredValueProjection::Number { value: 7.0 })
    );
    assert!(scalar.formula.is_none());
    assert_eq!(undo(&mut runtime, "resident/5"), "resident/6");
    let restored = table(&mut runtime, "plan");
    assert_eq!(source(projected(&restored, &row.key, "reserve")), formula);
}

/// Two native tables that share a field ID: `b.amount` is a formula over
/// `a.amount`. Field IDs are schema-scoped, so the removal check must not
/// mistake B's retained formula cell for a cell of the removed column.
fn shared_field_id_document() -> Document {
    let a = SchemaId::from("a");
    let b = SchemaId::from("b");
    let schema = |id: &SchemaId, fields: Vec<(FieldId, FieldDefinition)>| {
        (
            id.clone(),
            Schema {
                id: id.clone(),
                key: SchemaKey::from(id.as_str()),
                display_name: None,
                fields: fields.into_iter().collect(),
            },
        )
    };
    let mut document = Document::empty("shared_field_id", "Shared field ID");
    document.schemas = BTreeMap::from([
        schema(
            &a,
            vec![
                column("label", "Label", FieldType::Text),
                column("amount", "Amount", FieldType::Number),
            ],
        ),
        schema(&b, vec![column("amount", "Amount", FieldType::Number)]),
    ]);
    document.entities = BTreeMap::from([
        entity(
            "a_row",
            &a,
            BTreeMap::from([
                (FieldId::from("label"), Value::Text("first".to_owned())),
                (FieldId::from("amount"), count(3.0)),
            ]),
        ),
        entity(
            "b_row",
            &b,
            BTreeMap::from([(
                FieldId::from("amount"),
                Value::Formula(Expression::Reference(FieldRef::new("a_row", "amount"))),
            )]),
        ),
    ]);
    document
}

#[test]
fn column_removal_counts_dependents_in_tables_that_share_the_field_id() {
    let mut runtime = DesignerRuntime::from_document(shared_field_id_document(), OCCURRENCE)
        .expect("the shared-field-id document is admissible");
    assert_eq!(table(&mut runtime, "a").native_table_profile, Some(true));
    let error = runtime
        .handle(DesignerRequest::RemoveColumn {
            expected_revision: "resident/0".to_owned(),
            collection: "a".to_owned(),
            field: "amount".to_owned(),
        })
        .expect_err("a column another table's formula reads must stay");
    let failure = error.failure_projection("resident/0");
    assert_eq!(failure.code, "formula_dependency_in_use");
    assert_eq!(failure.diagnostics[0].facts[0].value, "1");
    assert_eq!(runtime.observe_occurrence().revision, "resident/0");
}
