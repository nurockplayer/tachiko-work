use tachiko_merge_engine::{MergeOutcome, merge};
use tachiko_semantic_core::{
    Document, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding,
    KeyedGroupedSumProductsBinding,
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

// ADR-0044: delta support grants no conflict facet or partial merge candidate.
#[test]
fn conflict_v2_refuses_all_definition_changing_requests_atomically() {
    use tachiko_merge_engine::{MergeOutcomeV2, merge_v2};
    use tachiko_semantic_core::{FieldConstraint, FieldDefinition, FieldType, Schema};
    let mut empty = document();
    for id in ["orders-a", "orders-b", "products-a", "products-b"] {
        empty.schemas.insert(
            id.into(),
            Schema {
                id: id.into(),
                key: id.into(),
                display_name: Some(format!("Table {id}")),
                fields: ["lookup", "quantity", "key", "category", "price"]
                    .into_iter()
                    .map(|field| {
                        (
                            field.into(),
                            FieldDefinition {
                                id: field.into(),
                                key: field.into(),
                                display_name: None,
                                field_type: if field == "quantity" || field == "price" {
                                    FieldType::Number
                                } else {
                                    FieldType::Text
                                },
                                required: false,
                                constraint: FieldConstraint::None,
                            },
                        )
                    })
                    .collect(),
            },
        );
    }
    let mut present = empty.clone();
    present
        .keyed_grouped_sum_definitions
        .insert("summary".into(), definition("summary", "a"));
    let mut changed = present.clone();
    changed
        .keyed_grouped_sum_definitions
        .insert("summary".into(), definition("summary", "b"));
    for (base, left, right) in [
        (&empty, &present, &empty), // one-sided creation
        (&empty, &empty, &present),
        (&empty, &present, &present),   // equal concurrent additions
        (&present, &changed, &present), // in-place edit
        (&present, &present, &changed),
        (&present, &changed, &changed),
        (&present, &empty, &present), // one-sided deletion
        (&present, &present, &empty),
        (&present, &empty, &empty), // matching deletion
    ] {
        for input in [base, left, right] {
            tachiko_semantic_core::validate_keyed_grouped_sum_definitions(input).unwrap();
        }
        let mut left = left.clone();
        let mut right = right.clone();
        left.title = "Left conflict must not escape".into();
        right.title = "Right conflict must not escape".into();
        assert!(matches!(
            merge_v2(base, &left, &right),
            MergeOutcomeV2::UnsupportedKeyedGroupedSumDefinitionChange
        ));
    }
    // Unchanged maps remain supported, including names, and are still validated.
    assert!(matches!(
        merge_v2(&present, &present, &present),
        MergeOutcomeV2::Merged(_)
    ));
    // Input/candidate admission belongs to workspace-engine; its existing
    // semantic_conflict_v2_acceptance tests validate each input role.
}
