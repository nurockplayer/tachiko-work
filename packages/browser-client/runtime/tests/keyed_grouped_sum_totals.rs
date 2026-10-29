//! J5: the grouped-sum projection carries an authoritative overall total.
//!
//! The fixture is the J4 Catalog/Sales workbook: Sales rows reference Catalog
//! keys, and the definition prices each sale from its catalog entry.
use std::collections::BTreeMap;
use tachiko_designer_runtime::{
    DesignerRequest, DesignerResponse, DesignerRuntime, KeyedGroupedSumDefinitionInput,
    KeyedGroupedSumProjection, ScalarEditInput,
};
use tachiko_workspace_engine::{
    Document, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey, FieldType, Number,
    Schema, SchemaId, SchemaKey, Value,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000075";
const DEFINITION: &str = "sales-summary";

fn field_type(name: &str) -> FieldType {
    match name {
        "quantity" | "price" => FieldType::Number,
        _ => FieldType::Text,
    }
}

fn schema(name: &str, fields: &[&str]) -> Schema {
    let id = SchemaId::from(name);
    Schema {
        field_order: None,
        id: id.clone(),
        key: SchemaKey::from(name),
        display_name: None,
        fields: fields
            .iter()
            .map(|field| {
                let id = FieldId::from(*field);
                (
                    id.clone(),
                    FieldDefinition {
                        id,
                        key: FieldKey::from(*field),
                        display_name: None,
                        number_format: None,
                        field_type: field_type(field),
                        required: true,
                        constraint: tachiko_workspace_engine::FieldConstraint::None,
                    },
                )
            })
            .collect(),
    }
}

fn entity(schema: &str, id: &str, values: &[(&str, Value)]) -> Entity {
    Entity {
        id: EntityId::from(id),
        key: EntityKey::from(id),
        schema: SchemaId::from(schema),
        fields: values
            .iter()
            .map(|(field, value)| (FieldId::from(*field), value.clone()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn number(value: f64) -> Value {
    Value::Number(Number::new(value).unwrap())
}

fn document(pen_price: f64) -> Document {
    let mut document = Document::empty(OCCURRENCE, "J4 Catalog/Sales");
    document.schemas.insert(
        SchemaId::from("sales"),
        schema("sales", &["key", "quantity"]),
    );
    document.schemas.insert(
        SchemaId::from("catalog"),
        schema("catalog", &["key", "category", "price"]),
    );
    for (id, key, quantity) in [
        ("sale_1", "PEN", 3.0),
        ("sale_2", "NOTE", 2.0),
        ("sale_3", "PEN", 1.0),
    ] {
        document.entities.insert(
            EntityId::from(id),
            entity(
                "sales",
                id,
                &[("key", text(key)), ("quantity", number(quantity))],
            ),
        );
    }
    for (id, key, category, price) in [
        ("product_1", "PEN", "Stationery", pen_price),
        ("product_2", "NOTE", "Paper", 500.0),
    ] {
        document.entities.insert(
            EntityId::from(id),
            entity(
                "catalog",
                id,
                &[
                    ("key", text(key)),
                    ("category", text(category)),
                    ("price", number(price)),
                ],
            ),
        );
    }
    document
}

fn fixture(pen_price: f64) -> DesignerRuntime {
    DesignerRuntime::from_document(document(pen_price), OCCURRENCE).unwrap()
}

fn definition() -> KeyedGroupedSumDefinitionInput {
    KeyedGroupedSumDefinitionInput {
        id: DEFINITION.to_owned(),
        orders_schema: "sales".to_owned(),
        order_lookup_key_field: "key".to_owned(),
        order_quantity_field: "quantity".to_owned(),
        products_schema: "catalog".to_owned(),
        product_key_field: "key".to_owned(),
        product_category_field: "category".to_owned(),
        product_price_field: "price".to_owned(),
    }
}

fn create(runtime: &mut DesignerRuntime) {
    let expected_revision = runtime.observe_occurrence().revision;
    let response = runtime
        .handle(DesignerRequest::CreateKeyedGroupedSum {
            expected_revision,
            definition: definition(),
        })
        .unwrap();
    let DesignerResponse::KeyedGroupedSumPublished(published) = response else {
        panic!("unexpected create response {response:?}");
    };
    assert_eq!(published.result.diagnostics.len(), 0);
}

fn query(runtime: &mut DesignerRuntime) -> KeyedGroupedSumProjection {
    let response = runtime
        .handle(DesignerRequest::QueryKeyedGroupedSum {
            definition_id: DEFINITION.to_owned(),
        })
        .unwrap();
    let DesignerResponse::KeyedGroupedSum(result) = response else {
        panic!("unexpected query response {response:?}");
    };
    result
}

fn group(result: &KeyedGroupedSumProjection, category: &str) -> f64 {
    result
        .groups
        .iter()
        .find(|group| group.category == category)
        .unwrap_or_else(|| panic!("missing group {category}"))
        .value
}

fn set_price(runtime: &mut DesignerRuntime, price: &str) {
    let expected_revision = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision,
            target: "product_1.price".into(),
            input: ScalarEditInput::Number {
                input: price.to_owned(),
            },
        })
        .unwrap();
}

#[test]
fn j4_fixture_reports_pen_and_note_groups_with_a_matching_total() {
    let mut runtime = fixture(200.0);
    create(&mut runtime);
    let result = query(&mut runtime);
    assert_eq!(result.revision, runtime.observe_occurrence().revision);
    assert_eq!(group(&result, "Paper"), 1000.0);
    assert_eq!(group(&result, "Stationery"), 800.0);
    assert_eq!(result.total, Some(1800.0));
    assert_eq!(result.diagnostics.len(), 0);
}

#[test]
fn j5_price_change_updates_groups_and_the_total_in_one_revision() {
    let mut runtime = fixture(200.0);
    create(&mut runtime);
    set_price(&mut runtime, "250");
    let result = query(&mut runtime);
    // The total comes from the same evaluated groups, never a cached 1800.
    assert_eq!(result.revision, runtime.observe_occurrence().revision);
    assert_eq!(group(&result, "Paper"), 1000.0);
    assert_eq!(group(&result, "Stationery"), 1000.0);
    assert_eq!(result.total, Some(2000.0));
}

#[test]
fn unavailable_results_never_report_a_zero_or_stale_total() {
    let mut runtime = fixture(200.0);
    create(&mut runtime);
    assert_eq!(query(&mut runtime).total, Some(1800.0));
    // A sale that matches no catalog key makes the whole result unavailable.
    let expected_revision = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision,
            target: "sale_1.key".into(),
            input: ScalarEditInput::Text {
                value: "MISSING".to_owned(),
            },
        })
        .unwrap();
    let unavailable = query(&mut runtime);
    assert_eq!(unavailable.revision, runtime.observe_occurrence().revision);
    assert_eq!(unavailable.groups.len(), 0);
    assert_eq!(unavailable.total, None);
    assert_ne!(unavailable.diagnostics.len(), 0);
    // Restoring the key restores both the groups and the total.
    let expected_revision = runtime.observe_occurrence().revision;
    runtime
        .handle(DesignerRequest::EditScalar {
            expected_revision,
            target: "sale_1.key".into(),
            input: ScalarEditInput::Text {
                value: "PEN".to_owned(),
            },
        })
        .unwrap();
    let restored = query(&mut runtime);
    assert_eq!(restored.total, Some(1800.0));
}

#[test]
fn an_available_result_with_no_contributing_rows_is_zero() {
    let mut document = document(200.0);
    document
        .entities
        .retain(|_, entity| entity.schema != SchemaId::from("sales"));
    let mut runtime = DesignerRuntime::from_document(document, OCCURRENCE).unwrap();
    create(&mut runtime);
    let result = query(&mut runtime);
    // Complete with no rows is an available zero, not an unavailable result.
    assert_eq!(result.groups.len(), 0);
    assert_eq!(result.total, Some(0.0));
    assert_eq!(result.diagnostics.len(), 0);
}
