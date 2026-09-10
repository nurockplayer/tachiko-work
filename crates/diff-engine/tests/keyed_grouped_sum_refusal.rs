use tachiko_diff_engine::{DiffError, diff};
use tachiko_semantic_core::{
    Document, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
};

fn definition(id: &str, suffix: &str) -> KeyedGroupedSumDefinition {
    KeyedGroupedSumDefinition {
        id: KeyedGroupedSumDefinitionId::from(id),
        orders: KeyedGroupedSumOrdersBinding {
            schema: format!("orders-{suffix}").into(),
            lookup_key_field: "lookup".into(),
            quantity_field: "quantity".into(),
        },
        products: KeyedGroupedSumProductsBinding {
            schema: format!("products-{suffix}").into(),
            key_field: "key".into(),
            category_field: "category".into(),
            price_field: "price".into(),
        },
    }
}

fn document() -> Document {
    Document::empty("doc", "title")
}

fn assert_refused(before: &Document, after: &Document) {
    assert!(matches!(
        diff(before, after),
        Err(DiffError::UnsupportedKeyedGroupedSumDefinitionChange)
    ));
}

#[test]
fn definition_create_is_a_whole_request_refusal() {
    let before = document();
    let mut after = before.clone();
    let value = definition("summary", "a");
    after
        .keyed_grouped_sum_definitions
        .insert(value.id.clone(), value);
    assert_refused(&before, &after);
}

#[test]
fn definition_update_is_a_whole_request_refusal() {
    let mut before = document();
    let first = definition("summary", "a");
    before
        .keyed_grouped_sum_definitions
        .insert(first.id.clone(), first);
    let mut after = before.clone();
    let second = definition("summary", "b");
    after
        .keyed_grouped_sum_definitions
        .insert(second.id.clone(), second);
    assert_refused(&before, &after);
}

#[test]
fn definition_delete_is_a_whole_request_refusal() {
    let mut before = document();
    let value = definition("summary", "a");
    before
        .keyed_grouped_sum_definitions
        .insert(value.id.clone(), value);
    let after = document();
    assert_refused(&before, &after);
}
