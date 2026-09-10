use tachiko_merge_engine::{MergeOutcome, merge};
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

fn assert_refused(base: &Document, ours: &Document, theirs: &Document) {
    assert!(matches!(
        merge(base, ours, theirs),
        MergeOutcome::UnsupportedKeyedGroupedSumDefinitionChange
    ));
}

#[test]
fn definition_create_is_a_whole_request_refusal() {
    let base = document();
    let mut ours = base.clone();
    let value = definition("summary", "a");
    ours.keyed_grouped_sum_definitions
        .insert(value.id.clone(), value);
    assert_refused(&base, &ours, &base);
}

#[test]
fn definition_update_is_a_whole_request_refusal() {
    let mut base = document();
    let first = definition("summary", "a");
    base.keyed_grouped_sum_definitions
        .insert(first.id.clone(), first);
    let mut ours = base.clone();
    let second = definition("summary", "b");
    ours.keyed_grouped_sum_definitions
        .insert(second.id.clone(), second);
    assert_refused(&base, &ours, &base);
}

#[test]
fn definition_delete_is_a_whole_request_refusal() {
    let mut base = document();
    let value = definition("summary", "a");
    base.keyed_grouped_sum_definitions
        .insert(value.id.clone(), value);
    let ours = document();
    assert_refused(&base, &ours, &base);
}
