//! #681 acceptance: a conditional aggregate reads its criterion from a Text
//! cell, follows structural edits, and refuses removal or retyping of a column
//! it reads. The fixture is reopened from persisted `.roproj/v3` bytes.
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;

use serde_json::json;
use tachiko_designer_runtime::host_bridge::{DesignerHost, HostReply};
use tachiko_designer_runtime::{
    CalculationProjection, DesignerError, DesignerRequest, DesignerResponse, DesignerRuntime,
    DuplicateCollectionProjection, FieldProjection, FieldTarget, RowInitializer, ScalarEditInput,
    StoredValueProjection, TableProjection,
};
use tachiko_workspace_engine::{
    ColumnCriterion, ColumnRef, ComparisonOperator, ConditionalAggregate, Document, Entity,
    EntityId, EntityKey, Expression, FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldRef,
    FieldType, Number, Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000681";
const REOPENED: &str = "00000000-0000-4000-8000-000000000682";
const EXPORTED: &str = "00000000-0000-4000-8000-000000000683";

const SUMMARY: &str =
    "SUMIFS([plan.quantity:column], [plan.category:column], [settings.criterion])";

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

fn number(value: f64) -> Value {
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

fn cell_criterion(plan_field: &str) -> ColumnCriterion {
    ColumnCriterion {
        column: ColumnRef::new("plan", plan_field),
        operator: ComparisonOperator::Equal,
        value: Some(Box::new(Expression::Reference(FieldRef::new(
            "settings",
            "criterion",
        )))),
    }
}

fn same_row_criterion() -> ColumnCriterion {
    ColumnCriterion {
        column: ColumnRef::new("plan", "category"),
        operator: ComparisonOperator::Equal,
        value: Some(Box::new(Expression::SameRowReference(FieldId::from(
            "category",
        )))),
    }
}

fn fixture() -> Document {
    let mut document = Document::empty("conditional_aggregates", "Conditional aggregates");
    let plan_id = SchemaId::from("plan");
    document.schemas.insert(
        plan_id.clone(),
        Schema {
            field_order: None,
            id: plan_id.clone(),
            key: SchemaKey::from("plan"),
            display_name: Some("Plan".to_owned()),
            fields: BTreeMap::from([
                column("item", "Item", FieldType::Text),
                column("category", "Category", FieldType::Text),
                column("quantity", "Quantity", FieldType::Number),
                column("matching", "Matching", FieldType::Number),
            ]),
        },
    );
    document.schemas.insert(
        SchemaId::from("settings"),
        Schema {
            field_order: None,
            id: SchemaId::from("settings"),
            key: SchemaKey::from("settings"),
            display_name: Some("Settings".to_owned()),
            fields: BTreeMap::from([column("criterion", "Criterion", FieldType::Text)]),
        },
    );
    document.schemas.insert(
        SchemaId::from("summary"),
        Schema {
            field_order: None,
            id: SchemaId::from("summary"),
            key: SchemaKey::from("summary"),
            display_name: Some("Summary".to_owned()),
            fields: BTreeMap::from([column("result", "Result", FieldType::Number)]),
        },
    );
    let matching = Expression::ColumnConditional {
        aggregate: Box::new(ConditionalAggregate::Sum(ColumnRef::new(
            "plan", "quantity",
        ))),
        criteria: vec![same_row_criterion()],
    };
    for (key, item, category, quantity) in [
        ("rent", "Rent", "housing", 2.0),
        ("ads", "Ads", "marketing", 3.0),
        ("tools", "Tools", "housing", 4.0),
    ] {
        let (id, row) = entity(
            key,
            &plan_id,
            BTreeMap::from([
                (FieldId::from("item"), Value::Text(item.to_owned())),
                (FieldId::from("category"), Value::Text(category.to_owned())),
                (FieldId::from("quantity"), number(quantity)),
                (FieldId::from("matching"), Value::Formula(matching.clone())),
            ]),
        );
        document.entities.insert(id, row);
    }
    let (id, row) = entity(
        "settings",
        &SchemaId::from("settings"),
        BTreeMap::from([(
            FieldId::from("criterion"),
            Value::Text("housing".to_owned()),
        )]),
    );
    document.entities.insert(id, row);
    let (id, row) = entity(
        "summary",
        &SchemaId::from("summary"),
        BTreeMap::from([(
            FieldId::from("result"),
            Value::Formula(Expression::ColumnConditional {
                aggregate: Box::new(ConditionalAggregate::Sum(ColumnRef::new(
                    "plan", "quantity",
                ))),
                criteria: vec![cell_criterion("category")],
            }),
        )]),
    );
    document.entities.insert(id, row);
    document
}

fn open_fixture() -> DesignerRuntime {
    let source = DesignerRuntime::from_document(fixture(), OCCURRENCE)
        .expect("the conditional aggregate fixture is admissible");
    let saved = source
        .export_project_v3("resident/0")
        .expect("the fixture exports as v3");
    let mut reopened = None;
    tachiko_designer_runtime::open_project(&mut reopened, &saved.bytes, REOPENED)
        .expect("the saved fixture reopens");
    reopened.expect("a successful open installs the occurrence")
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

fn published(runtime: &mut DesignerRuntime, request: DesignerRequest) -> String {
    let DesignerResponse::Published(publication) =
        runtime.handle(request).expect("the request is accepted")
    else {
        panic!("expected a publication");
    };
    publication.resulting_revision
}

fn column_id(table: &TableProjection, key: &str) -> String {
    table
        .columns
        .iter()
        .find(|candidate| candidate.key == key)
        .unwrap_or_else(|| panic!("the table has no '{key}' column"))
        .id
        .clone()
}

fn projected<'a>(table: &'a TableProjection, item: &str, column: &str) -> &'a FieldProjection {
    let id = column_id(table, column);
    let row = table
        .rows
        .iter()
        .find(|row| {
            row.fields.iter().any(|field| {
                matches!(&field.stored, Some(StoredValueProjection::Text { value }) if value == item)
            })
        })
        .unwrap_or_else(|| panic!("the table has no '{item}' row"));
    row.fields
        .iter()
        .find(|field| field.target.field == id)
        .unwrap_or_else(|| panic!("row '{item}' has no '{column}' value"))
}

fn number_of(field: &FieldProjection) -> Option<f64> {
    field
        .calculated
        .as_ref()
        .and_then(CalculationProjection::number)
}

fn summary_value(runtime: &mut DesignerRuntime) -> Option<f64> {
    let summary = table(runtime, "summary");
    let result = column_id(&summary, "result");
    let field = summary.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == result)
        .expect("the summary row has a result");
    number_of(field)
}

fn edit_text(runtime: &mut DesignerRuntime, expected: &str, target: &str, value: &str) -> String {
    published(
        runtime,
        DesignerRequest::EditScalar {
            expected_revision: expected.to_owned(),
            target: FieldTarget::from(target),
            input: ScalarEditInput::Text {
                value: value.to_owned(),
            },
        },
    )
}

#[test]
fn a_text_cell_drives_the_projection_and_undo_restores_it() {
    let mut runtime = open_fixture();
    assert_eq!(summary_value(&mut runtime), Some(6.0));
    let plan = table(&mut runtime, "plan");
    assert_eq!(number_of(projected(&plan, "Rent", "matching")), Some(6.0));
    assert_eq!(number_of(projected(&plan, "Ads", "matching")), Some(3.0));
    assert_eq!(number_of(projected(&plan, "Tools", "matching")), Some(6.0));

    let edited = edit_text(
        &mut runtime,
        "resident/0",
        "settings.criterion",
        "marketing",
    );
    assert_eq!(summary_value(&mut runtime), Some(3.0));
    let after_undo = published(
        &mut runtime,
        DesignerRequest::Undo {
            expected_revision: edited,
        },
    );
    assert_eq!(after_undo, "resident/2");
    assert_eq!(summary_value(&mut runtime), Some(6.0));
    let restored = table(&mut runtime, "summary");
    let result = column_id(&restored, "result");
    let field = restored.rows[0]
        .fields
        .iter()
        .find(|field| field.target.field == result)
        .unwrap();
    assert_eq!(
        field
            .formula
            .as_ref()
            .expect("the formula survives undo")
            .source,
        SUMMARY
    );
}

#[test]
fn a_criterion_column_cannot_be_removed_or_retyped() {
    let mut runtime = open_fixture();
    let plan = table(&mut runtime, "plan");
    let category = column_id(&plan, "category");

    let error = runtime
        .handle(DesignerRequest::RemoveColumn {
            expected_revision: "resident/0".to_owned(),
            collection: "plan".to_owned(),
            field: category.clone(),
        })
        .expect_err("a criterion column stays while a formula reads it");
    assert!(
        matches!(error, DesignerError::FormulaDependencyInUse { .. }),
        "{error:?}"
    );

    let error = runtime
        .handle(DesignerRequest::ChangeColumnType {
            expected_revision: "resident/0".to_owned(),
            collection: "plan".to_owned(),
            field: category,
            field_type: "number".to_owned(),
        })
        .expect_err("a criterion column keeps its type while a formula reads it");
    match error {
        DesignerError::FormulaDependencyInUse {
            dependent_cells,
            subject,
        } => {
            assert_eq!(dependent_cells, 4);
            assert!(subject.contains("Category"), "{subject}");
        }
        other => panic!("expected a dependency refusal: {other:?}"),
    }
}

#[test]
fn inserting_a_row_counts_it_in_the_projection() {
    let mut runtime = open_fixture();
    let inserted = published(
        &mut runtime,
        DesignerRequest::InsertRow {
            expected_revision: "resident/0".to_owned(),
            collection: "plan".to_owned(),
            initializers: vec![
                RowInitializer {
                    field: "item".to_owned(),
                    input: ScalarEditInput::Text {
                        value: "Food".to_owned(),
                    },
                },
                RowInitializer {
                    field: "category".to_owned(),
                    input: ScalarEditInput::Text {
                        value: "housing".to_owned(),
                    },
                },
                RowInitializer {
                    field: "quantity".to_owned(),
                    input: ScalarEditInput::Number {
                        input: "5".to_owned(),
                    },
                },
            ],
        },
    );
    assert_eq!(inserted, "resident/1");
    assert_eq!(summary_value(&mut runtime), Some(11.0));
    assert_eq!(table(&mut runtime, "plan").rows.len(), 4);
}

#[test]
fn duplicating_the_table_keeps_the_copied_formula_on_the_copy() {
    let mut runtime = open_fixture();
    let DesignerResponse::Duplicated(DuplicateCollectionProjection {
        publication,
        collection,
    }) = runtime
        .handle(DesignerRequest::DuplicateCollection {
            expected_revision: "resident/0".to_owned(),
            collection: "plan".to_owned(),
            name: "Plan copy".to_owned(),
            column_names: BTreeMap::new(),
        })
        .expect("the table duplicates")
    else {
        panic!("expected a duplicate projection");
    };
    assert_eq!(publication.resulting_revision, "resident/1");
    let copy = table(&mut runtime, &collection.key);
    assert_eq!(number_of(projected(&copy, "Rent", "matching")), Some(6.0));
    assert_eq!(number_of(projected(&copy, "Ads", "matching")), Some(3.0));
    let copied_quantity = projected(&copy, "Rent", "quantity").target.clone();
    published(
        &mut runtime,
        DesignerRequest::EditScalar {
            expected_revision: "resident/1".to_owned(),
            target: copied_quantity,
            input: ScalarEditInput::Number {
                input: "10".to_owned(),
            },
        },
    );
    let copy = table(&mut runtime, &collection.key);
    assert_eq!(number_of(projected(&copy, "Rent", "matching")), Some(14.0));
    assert_eq!(number_of(projected(&copy, "Ads", "matching")), Some(3.0));
    let original = table(&mut runtime, "plan");
    assert_eq!(
        number_of(projected(&original, "Rent", "matching")),
        Some(6.0)
    );
    assert_eq!(summary_value(&mut runtime), Some(6.0));
}

#[test]
fn xlsx_export_refuses_the_conditional_aggregate_and_keeps_the_value() {
    let runtime = DesignerRuntime::from_document(fixture(), OCCURRENCE).unwrap();
    let bytes = runtime.export_project_v3("resident/0").unwrap().bytes;
    let mut host = DesignerHost::new();
    let reply: serde_json::Value =
        serde_json::from_slice(&host.open_project(&bytes, EXPORTED.as_bytes())).unwrap();
    assert_eq!(reply["status"], "ok", "{reply}");
    let HostReply { reply, .. } = host.spreadsheet(
        &serde_json::to_vec(&json!({
            "type": "export_native_tables",
            "expected_revision": "resident/0",
            "collection": "summary",
            "format": "xlsx",
        }))
        .unwrap(),
        &[],
    );
    let reply: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    let ledger = reply["response"]["payload"]["ledger"]
        .as_array()
        .unwrap_or_else(|| panic!("the export reports a ledger: {reply}"));
    let finding = ledger
        .iter()
        .find(|finding| {
            finding["message"].as_str().is_some_and(|message| {
                message.contains("conditional aggregates are outside the export mapping")
            })
        })
        .unwrap_or_else(|| panic!("conditional aggregate export is refused: {reply}"));
    assert_eq!(finding["category"], "lossy_on_export");
}
