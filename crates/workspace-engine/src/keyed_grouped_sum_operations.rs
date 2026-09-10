//! Authoritative saved keyed lookup + grouped-sum evaluation for ADR-0036.
//!
//! This module owns semantic evaluation only. UI projection and persistence are
//! adapters over this result and must not implement a second evaluator.

use std::collections::BTreeMap;

use tachiko_formula_engine::{Calculation, CalculationOutcome, calculate_complete};
use tachiko_semantic_core::{
    Document, Entity, EntityId, FieldId, FieldRef, KeyedGroupedSumDefinition,
    KeyedGroupedSumDefinitionId, Number, Value, validate_keyed_grouped_sum_definitions,
};

/// Provisional finite profile for a complete group-map projection.
pub const MAX_KEYED_GROUPED_SUM_GROUPS: usize = 256;
/// Provisional finite profile for a complete ambiguity-candidate projection.
pub const MAX_KEYED_GROUPED_SUM_AMBIGUITY_CANDIDATES: usize = 256;

pub const LOOKUP_MISSING_KEY: &str = "lookup.missing_key";
pub const LOOKUP_AMBIGUOUS_KEY: &str = "lookup.ambiguous_key";
pub const DEFINITION_INVALID: &str = "keyed_grouped_sum.invalid_definition";
pub const INPUT_MISSING: &str = "keyed_grouped_sum.missing_input";
pub const INPUT_WRONG_KIND: &str = "keyed_grouped_sum.wrong_input_kind";
pub const FORMULA_UNAVAILABLE: &str = "keyed_grouped_sum.formula_unavailable";
pub const NUMBER_NON_FINITE: &str = "keyed_grouped_sum.non_finite_number";
pub const RESULT_TOO_LARGE: &str = "keyed_grouped_sum.result_too_large";

/// One complete grouped value. Presentation order is deliberately not semantic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyedGroupedSumGroup {
    pub category: String,
    pub value: Number,
}

/// Deterministic structured failure fact. No partial groups accompany failures.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeyedGroupedSumDiagnostic {
    pub code: &'static str,
    pub entity: Option<EntityId>,
    pub field: Option<FieldId>,
    pub lookup_key: Option<String>,
    pub candidates: Vec<EntityId>,
}

impl KeyedGroupedSumDiagnostic {
    fn definition(code: &'static str) -> Self {
        Self {
            code,
            entity: None,
            field: None,
            lookup_key: None,
            candidates: Vec::new(),
        }
    }

    fn input(code: &'static str, entity: &EntityId, field: &FieldId) -> Self {
        Self {
            code,
            entity: Some(entity.clone()),
            field: Some(field.clone()),
            lookup_key: None,
            candidates: Vec::new(),
        }
    }

    fn lookup(code: &'static str, entity: &EntityId, key: &str, candidates: Vec<EntityId>) -> Self {
        Self {
            code,
            entity: Some(entity.clone()),
            field: None,
            lookup_key: Some(key.to_owned()),
            candidates,
        }
    }
}

/// Complete current result or whole-definition unavailability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyedGroupedSumOutcome {
    Complete(Vec<KeyedGroupedSumGroup>),
    Unavailable(Vec<KeyedGroupedSumDiagnostic>),
}

/// Evaluate one saved definition against the current accepted snapshot.
///
/// Exact Text lookup is ordinary Rust `String` equality over decoded Unicode
/// scalar sequences: no normalization, case folding, locale handling, wildcard,
/// or numeric coercion occurs.
#[must_use]
pub fn evaluate_keyed_grouped_sum(
    document: &Document,
    definition_id: &KeyedGroupedSumDefinitionId,
) -> KeyedGroupedSumOutcome {
    if validate_keyed_grouped_sum_definitions(document).is_err() {
        return KeyedGroupedSumOutcome::Unavailable(vec![KeyedGroupedSumDiagnostic::definition(
            DEFINITION_INVALID,
        )]);
    }
    let Some(definition) = document.keyed_grouped_sum_definitions.get(definition_id) else {
        return KeyedGroupedSumOutcome::Unavailable(vec![KeyedGroupedSumDiagnostic::definition(
            DEFINITION_INVALID,
        )]);
    };

    let products = entities_for_schema(document, &definition.products.schema);
    let orders = entities_for_schema(document, &definition.orders.schema);
    let mut diagnostics = Vec::new();
    let mut product_index: BTreeMap<String, Vec<&Entity>> = BTreeMap::new();

    // Complete Product membership/key universe is part of lookup meaning.
    for product in products {
        match product.fields.get(&definition.products.key_field) {
            Some(Value::Text(key)) => product_index.entry(key.clone()).or_default().push(product),
            Some(_) => diagnostics.push(KeyedGroupedSumDiagnostic::input(
                INPUT_WRONG_KIND,
                &product.id,
                &definition.products.key_field,
            )),
            None => diagnostics.push(KeyedGroupedSumDiagnostic::input(
                INPUT_MISSING,
                &product.id,
                &definition.products.key_field,
            )),
        }
    }
    if !diagnostics.is_empty() {
        return unavailable(diagnostics);
    }

    let mut matched = Vec::new();
    let mut formula_operand_required = false;
    for order in orders {
        let lookup_key = match order.fields.get(&definition.orders.lookup_key_field) {
            Some(Value::Text(key)) => key,
            Some(_) => {
                diagnostics.push(KeyedGroupedSumDiagnostic::input(
                    INPUT_WRONG_KIND,
                    &order.id,
                    &definition.orders.lookup_key_field,
                ));
                continue;
            }
            None => {
                diagnostics.push(KeyedGroupedSumDiagnostic::input(
                    INPUT_MISSING,
                    &order.id,
                    &definition.orders.lookup_key_field,
                ));
                continue;
            }
        };
        let candidates = product_index
            .get(lookup_key)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        match candidates {
            [] => diagnostics.push(KeyedGroupedSumDiagnostic::lookup(
                LOOKUP_MISSING_KEY,
                &order.id,
                lookup_key,
                Vec::new(),
            )),
            [product] => {
                formula_operand_required |= matches!(
                    order.fields.get(&definition.orders.quantity_field),
                    Some(Value::Formula(_))
                );
                formula_operand_required |= matches!(
                    product.fields.get(&definition.products.price_field),
                    Some(Value::Formula(_))
                );
                matched.push((order, *product));
            }
            many => {
                let candidate_ids = many
                    .iter()
                    .map(|entity| entity.id.clone())
                    .collect::<Vec<_>>();
                if candidate_ids.len() > MAX_KEYED_GROUPED_SUM_AMBIGUITY_CANDIDATES {
                    diagnostics.push(KeyedGroupedSumDiagnostic::lookup(
                        RESULT_TOO_LARGE,
                        &order.id,
                        lookup_key,
                        Vec::new(),
                    ));
                } else {
                    diagnostics.push(KeyedGroupedSumDiagnostic::lookup(
                        LOOKUP_AMBIGUOUS_KEY,
                        &order.id,
                        lookup_key,
                        candidate_ids,
                    ));
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return unavailable(diagnostics);
    }

    // ADR-0036 requires the authoritative complete Calculation when a required
    // operand is formula-backed, including unrelated roots whose failure makes
    // that complete Calculation unavailable.
    let calculation = if formula_operand_required {
        match calculate_complete(document) {
            CalculationOutcome::Complete(calculation) => Some(calculation),
            CalculationOutcome::Failed(_) => {
                return KeyedGroupedSumOutcome::Unavailable(vec![
                    KeyedGroupedSumDiagnostic::definition(FORMULA_UNAVAILABLE),
                ]);
            }
        }
    } else {
        None
    };

    let mut contributions: BTreeMap<String, Vec<Number>> = BTreeMap::new();
    for (order, product) in matched {
        let Some(category) = text_input(
            product,
            &definition.products.category_field,
            &mut diagnostics,
        ) else {
            continue;
        };
        let Some(quantity) = number_input(
            document,
            calculation.as_ref(),
            order,
            &definition.orders.quantity_field,
            &mut diagnostics,
        ) else {
            continue;
        };
        let Some(price) = number_input(
            document,
            calculation.as_ref(),
            product,
            &definition.products.price_field,
            &mut diagnostics,
        ) else {
            continue;
        };
        let Ok(amount) = Number::new(price.get() * quantity.get()) else {
            diagnostics.push(KeyedGroupedSumDiagnostic::input(
                NUMBER_NON_FINITE,
                &order.id,
                &definition.orders.quantity_field,
            ));
            continue;
        };
        contributions.entry(category).or_default().push(amount);
    }
    if !diagnostics.is_empty() {
        return unavailable(diagnostics);
    }
    if contributions.len() > MAX_KEYED_GROUPED_SUM_GROUPS {
        return KeyedGroupedSumOutcome::Unavailable(vec![KeyedGroupedSumDiagnostic::definition(
            RESULT_TOO_LARGE,
        )]);
    }

    let mut groups = Vec::with_capacity(contributions.len());
    for (category, mut values) in contributions {
        values.sort_unstable();
        let mut sum = Number::new(0.0).expect("semantic zero is finite");
        for value in values {
            let Ok(next) = Number::new(sum.get() + value.get()) else {
                diagnostics.push(KeyedGroupedSumDiagnostic::definition(NUMBER_NON_FINITE));
                break;
            };
            sum = next;
        }
        if !diagnostics.is_empty() {
            return unavailable(diagnostics);
        }
        groups.push(KeyedGroupedSumGroup {
            category,
            value: sum,
        });
    }
    KeyedGroupedSumOutcome::Complete(groups)
}

fn entities_for_schema<'a>(
    document: &'a Document,
    schema: &tachiko_semantic_core::SchemaId,
) -> Vec<&'a Entity> {
    document
        .entities
        .values()
        .filter(|entity| &entity.schema == schema)
        .collect()
}

fn text_input(
    entity: &Entity,
    field: &FieldId,
    diagnostics: &mut Vec<KeyedGroupedSumDiagnostic>,
) -> Option<String> {
    match entity.fields.get(field) {
        Some(Value::Text(value)) => Some(value.clone()),
        Some(_) => {
            diagnostics.push(KeyedGroupedSumDiagnostic::input(
                INPUT_WRONG_KIND,
                &entity.id,
                field,
            ));
            None
        }
        None => {
            diagnostics.push(KeyedGroupedSumDiagnostic::input(
                INPUT_MISSING,
                &entity.id,
                field,
            ));
            None
        }
    }
}

fn number_input(
    _document: &Document,
    calculation: Option<&Calculation>,
    entity: &Entity,
    field: &FieldId,
    diagnostics: &mut Vec<KeyedGroupedSumDiagnostic>,
) -> Option<Number> {
    match entity.fields.get(field) {
        Some(Value::Number(value)) => Some(*value),
        Some(Value::Formula(_)) => {
            let target = FieldRef::new(entity.id.clone(), field.clone());
            match calculation.and_then(|state| state.value(&target)) {
                Some(value) => Some(value),
                None => {
                    diagnostics.push(KeyedGroupedSumDiagnostic::input(
                        FORMULA_UNAVAILABLE,
                        &entity.id,
                        field,
                    ));
                    None
                }
            }
        }
        Some(_) => {
            diagnostics.push(KeyedGroupedSumDiagnostic::input(
                INPUT_WRONG_KIND,
                &entity.id,
                field,
            ));
            None
        }
        None => {
            diagnostics.push(KeyedGroupedSumDiagnostic::input(
                INPUT_MISSING,
                &entity.id,
                field,
            ));
            None
        }
    }
}

fn unavailable(mut diagnostics: Vec<KeyedGroupedSumDiagnostic>) -> KeyedGroupedSumOutcome {
    diagnostics.sort();
    diagnostics.dedup();
    KeyedGroupedSumOutcome::Unavailable(diagnostics)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tachiko_semantic_core::{
        Document, Entity, EntityId, EntityKey, Expression, FieldDefinition, FieldId, FieldKey,
        FieldRef, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
        KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Number, Schema, SchemaId,
        SchemaKey, Value,
    };

    use super::{
        FORMULA_UNAVAILABLE, KeyedGroupedSumOutcome, LOOKUP_AMBIGUOUS_KEY, LOOKUP_MISSING_KEY,
        evaluate_keyed_grouped_sum,
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
                field_type,
                required: true,
            },
        )
    }

    fn base_document() -> Document {
        let mut document = Document::empty("doc", "Grouped Sum");
        document.schemas.insert(
            SchemaId::from(ORDERS),
            Schema {
                id: SchemaId::from(ORDERS),
                key: SchemaKey::from("orders"),
                fields: BTreeMap::from([
                    field(ORDER_KEY, FieldType::Text),
                    field(QUANTITY, FieldType::Number),
                ]),
            },
        );
        document.schemas.insert(
            SchemaId::from(PRODUCTS),
            Schema {
                id: SchemaId::from(PRODUCTS),
                key: SchemaKey::from("products"),
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
}
