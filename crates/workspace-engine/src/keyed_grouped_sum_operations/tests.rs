use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Document, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId, FieldKey,
    FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
    KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
    SchemaKey, Value,
};

use super::{
    FORMULA_UNAVAILABLE, INPUT_MISSING, INPUT_WRONG_KIND, KeyedGroupedSumOutcome,
    LOOKUP_AMBIGUOUS_KEY, LOOKUP_MISSING_KEY, MAX_KEYED_GROUPED_SUM_AMBIGUITY_CANDIDATES,
    NUMBER_NON_FINITE, RESULT_TOO_LARGE, evaluate_keyed_grouped_sum,
};

const ORDERS: &str = "orders";
const PRODUCTS: &str = "products";
const ORDER_KEY: &str = "order-key";
const QUANTITY: &str = "quantity";
const PRODUCT_KEY: &str = "product-key";
const CATEGORY: &str = "category";
const PRICE: &str = "price";
const DEFINITION: &str = "summary";

fn field(id: &str, field_type: FieldType) -> (FieldId, FieldDefinition) {
    let id = FieldId::from(id);
    let key = FieldKey::from(format!("key_{id}"));
    (
        id.clone(),
        FieldDefinition {
            id,
            key,
            display_name: None,
            number_format: None,
            date_format: None,
            field_type,
            required: true,
            constraint: tachiko_semantic_core::FieldConstraint::None,
        },
    )
}

fn base_document() -> Document {
    let mut document = Document::empty("doc", "Grouped Sum");
    document.schemas.insert(
        SchemaId::from(ORDERS),
        Schema {
            headerless: false,
            field_order: None,
            id: SchemaId::from(ORDERS),
            key: SchemaKey::from("orders"),
            display_name: None,
            fields: BTreeMap::from([
                field(ORDER_KEY, FieldType::Text),
                field(QUANTITY, FieldType::Number),
            ]),
        },
    );
    document.schemas.insert(
        SchemaId::from(PRODUCTS),
        Schema {
            headerless: false,
            field_order: None,
            id: SchemaId::from(PRODUCTS),
            key: SchemaKey::from("products"),
            display_name: None,
            fields: BTreeMap::from([
                field(PRODUCT_KEY, FieldType::Text),
                field(CATEGORY, FieldType::Text),
                field(PRICE, FieldType::Number),
            ]),
        },
    );
    for (id, key, category, price) in [
        ("p100", "P-100", "hardware", 2.0),
        ("p200", "P-200", "services", 5.0),
    ] {
        document.entities.insert(
            EntityId::from(id),
            Entity {
                rank: None,
                id: EntityId::from(id),
                key: EntityKey::from(id),
                schema: SchemaId::from(PRODUCTS),
                fields: BTreeMap::from([
                    (FieldId::from(PRODUCT_KEY), Value::Text(key.to_owned())),
                    (FieldId::from(CATEGORY), Value::Text(category.to_owned())),
                    (
                        FieldId::from(PRICE),
                        Value::Number(Number::new(price).unwrap()),
                    ),
                ]),
            },
        );
    }
    for (id, key, quantity) in [
        ("o100a", "P-100", 3.0),
        ("o100b", "P-100", -1.0),
        ("o200", "P-200", 2.0),
    ] {
        document.entities.insert(
            EntityId::from(id),
            Entity {
                rank: None,
                id: EntityId::from(id),
                key: EntityKey::from(id),
                schema: SchemaId::from(ORDERS),
                fields: BTreeMap::from([
                    (FieldId::from(ORDER_KEY), Value::Text(key.to_owned())),
                    (
                        FieldId::from(QUANTITY),
                        Value::Number(Number::new(quantity).unwrap()),
                    ),
                ]),
            },
        );
    }
    let id = KeyedGroupedSumDefinitionId::from(DEFINITION);
    document.keyed_grouped_sum_definitions.insert(
        id.clone(),
        KeyedGroupedSumDefinition {
            id,
            orders: KeyedGroupedSumOrdersBinding {
                schema: SchemaId::from(ORDERS),
                lookup_key_field: FieldId::from(ORDER_KEY),
                quantity_field: FieldId::from(QUANTITY),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: SchemaId::from(PRODUCTS),
                key_field: FieldId::from(PRODUCT_KEY),
                category_field: FieldId::from(CATEGORY),
                price_field: FieldId::from(PRICE),
            },
        },
    );
    document
}

fn evaluate(document: &Document) -> KeyedGroupedSumOutcome {
    evaluate_keyed_grouped_sum(document, &KeyedGroupedSumDefinitionId::from(DEFINITION))
}

#[test]
fn canonical_fixture_is_exact_and_source_order_independent() {
    let document = base_document();
    let KeyedGroupedSumOutcome::Complete(groups) = evaluate(&document) else {
        panic!("fixture must be complete");
    };
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].category, "hardware");
    assert_eq!(groups[0].value, Number::new(4.0).unwrap());
    assert_eq!(groups[1].category, "services");
    assert_eq!(groups[1].value, Number::new(10.0).unwrap());
}

#[test]
fn exact_text_lookup_never_case_folds_or_normalizes() {
    let mut document = base_document();
    document
        .entities
        .get_mut("p100")
        .unwrap()
        .fields
        .insert(FieldId::from(PRODUCT_KEY), Value::Text("é".to_owned()));
    document
        .entities
        .get_mut("o100a")
        .unwrap()
        .fields
        .insert(FieldId::from(ORDER_KEY), Value::Text("e\u{301}".to_owned()));
    document
        .entities
        .get_mut("o100b")
        .unwrap()
        .fields
        .insert(FieldId::from(ORDER_KEY), Value::Text("É".to_owned()));
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&document) else {
        panic!("non-exact keys must be unavailable");
    };
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == LOOKUP_MISSING_KEY)
            .count(),
        2
    );
}

#[test]
fn ambiguity_returns_the_complete_stable_candidate_set_and_no_groups() {
    let mut document = base_document();
    document
        .entities
        .get_mut("p200")
        .unwrap()
        .fields
        .insert(FieldId::from(PRODUCT_KEY), Value::Text("P-100".to_owned()));
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&document) else {
        panic!("duplicate key must make the whole result unavailable");
    };
    let ambiguous = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == LOOKUP_AMBIGUOUS_KEY)
        .unwrap();
    assert_eq!(
        ambiguous.candidates,
        vec![EntityId::from("p100"), EntityId::from("p200")]
    );
}

#[test]
fn empty_orders_returns_a_complete_empty_group_map() {
    let mut document = base_document();
    document
        .entities
        .retain(|_, entity| entity.schema != SchemaId::from(ORDERS));
    assert_eq!(
        evaluate(&document),
        KeyedGroupedSumOutcome::Complete(Vec::new())
    );
}

#[test]
fn stable_bindings_survive_human_key_renames_and_relevant_edits_recompute() {
    let mut document = base_document();
    document.schemas.get_mut(ORDERS).unwrap().key = SchemaKey::from("renamed_orders");
    document
        .schemas
        .get_mut(PRODUCTS)
        .unwrap()
        .fields
        .get_mut(PRICE)
        .unwrap()
        .key = FieldKey::from("renamed_price");
    document.entities.get_mut("p100").unwrap().fields.insert(
        FieldId::from(PRICE),
        Value::Number(Number::new(4.0).unwrap()),
    );
    let KeyedGroupedSumOutcome::Complete(groups) = evaluate(&document) else {
        panic!("stable bindings must remain current");
    };
    assert_eq!(groups[0].value, Number::new(8.0).unwrap());
}

#[test]
fn required_formula_operand_uses_complete_calculation_and_unrelated_failure_blocks() {
    let mut document = base_document();
    document.entities.get_mut("p100").unwrap().fields.insert(
        FieldId::from(PRICE),
        Value::Formula(Expression::Number(Number::new(2.0).unwrap())),
    );
    document.entities.get_mut("p200").unwrap().fields.insert(
        FieldId::from(PRICE),
        Value::Formula(Expression::Divide {
            left: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            right: Box::new(Expression::Number(Number::new(0.0).unwrap())),
        }),
    );
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&document) else {
        panic!("failed complete calculation must block formula-backed grouped sum");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == FORMULA_UNAVAILABLE)
    );
}

#[test]
fn contribution_order_and_number_zero_are_deterministic() {
    let mut document = base_document();
    for entity in document.entities.values_mut() {
        if entity.schema == SchemaId::from(ORDERS) {
            entity
                .fields
                .insert(FieldId::from(ORDER_KEY), Value::Text("P-100".to_owned()));
        }
    }
    document.entities.get_mut("o100a").unwrap().fields.insert(
        FieldId::from(QUANTITY),
        Value::Number(Number::new(1.0e16).unwrap()),
    );
    document.entities.get_mut("o100b").unwrap().fields.insert(
        FieldId::from(QUANTITY),
        Value::Number(Number::new(-1.0e16).unwrap()),
    );
    document.entities.get_mut("o200").unwrap().fields.insert(
        FieldId::from(QUANTITY),
        Value::Number(Number::new(1.0).unwrap()),
    );
    let KeyedGroupedSumOutcome::Complete(groups) = evaluate(&document) else {
        panic!("finite cancellation fixture must be complete");
    };
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].value.to_bits(), 0);
}

#[test]
fn missing_key_is_not_replayed_after_product_key_edit() {
    let mut document = base_document();
    document
        .entities
        .get_mut("p200")
        .unwrap()
        .fields
        .insert(FieldId::from(PRODUCT_KEY), Value::Text("P-300".to_owned()));
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&document) else {
        panic!("missing P-200 must be unavailable");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == LOOKUP_MISSING_KEY)
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == LOOKUP_AMBIGUOUS_KEY)
    );
}

#[test]
fn missing_or_wrong_typed_required_input_makes_the_whole_definition_unavailable() {
    let mut missing = base_document();
    missing
        .entities
        .get_mut("p100")
        .unwrap()
        .fields
        .remove(&FieldId::from(CATEGORY));
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&missing) else {
        panic!("missing required input must not publish partial groups");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == INPUT_MISSING)
    );

    let mut wrong_kind = base_document();
    wrong_kind.entities.get_mut("o100a").unwrap().fields.insert(
        FieldId::from(QUANTITY),
        Value::Text("not-a-number".to_owned()),
    );
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&wrong_kind) else {
        panic!("wrong typed required input must not publish partial groups");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == INPUT_WRONG_KIND)
    );
}

#[test]
fn non_finite_contribution_or_reduction_makes_the_whole_definition_unavailable() {
    let mut contribution_overflow = base_document();
    contribution_overflow
        .entities
        .get_mut("p100")
        .unwrap()
        .fields
        .insert(
            FieldId::from(PRICE),
            Value::Number(Number::new(1.0e308).unwrap()),
        );
    contribution_overflow
        .entities
        .get_mut("o100a")
        .unwrap()
        .fields
        .insert(
            FieldId::from(QUANTITY),
            Value::Number(Number::new(2.0).unwrap()),
        );
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&contribution_overflow) else {
        panic!("non-finite multiplication must not publish partial groups");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == NUMBER_NON_FINITE)
    );

    let mut reduction_overflow = base_document();
    reduction_overflow
        .entities
        .get_mut("p100")
        .unwrap()
        .fields
        .insert(
            FieldId::from(PRICE),
            Value::Number(Number::new(1.0e308).unwrap()),
        );
    for order in ["o100a", "o100b", "o200"] {
        let entity = reduction_overflow.entities.get_mut(order).unwrap();
        entity
            .fields
            .insert(FieldId::from(ORDER_KEY), Value::Text("P-100".to_owned()));
        entity.fields.insert(
            FieldId::from(QUANTITY),
            Value::Number(Number::new(1.0).unwrap()),
        );
    }
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&reduction_overflow) else {
        panic!("non-finite reduction must not publish partial groups");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == NUMBER_NON_FINITE)
    );
}

#[test]
fn over_limit_ambiguity_is_unavailable_without_a_partial_candidate_set() {
    let mut document = base_document();
    for serial in 0..MAX_KEYED_GROUPED_SUM_AMBIGUITY_CANDIDATES {
        let id = EntityId::from(format!("duplicate-{serial}"));
        document.entities.insert(
            id.clone(),
            Entity {
                rank: None,
                id,
                key: EntityKey::from(format!("duplicate-{serial}")),
                schema: SchemaId::from(PRODUCTS),
                fields: BTreeMap::from([
                    (FieldId::from(PRODUCT_KEY), Value::Text("P-100".to_owned())),
                    (FieldId::from(CATEGORY), Value::Text("hardware".to_owned())),
                    (
                        FieldId::from(PRICE),
                        Value::Number(Number::new(2.0).unwrap()),
                    ),
                ]),
            },
        );
    }
    let KeyedGroupedSumOutcome::Unavailable(diagnostics) = evaluate(&document) else {
        panic!("over-limit ambiguity must not publish partial candidates or groups");
    };
    let result_too_large = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == RESULT_TOO_LARGE)
        .expect("over-limit ambiguity must identify the unavailable outcome");
    assert_eq!(result_too_large.candidates.as_slice(), []);
}
