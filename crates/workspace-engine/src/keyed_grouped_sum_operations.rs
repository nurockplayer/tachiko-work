//! Authoritative saved keyed lookup + grouped-sum evaluation for ADR-0036.
//!
//! This module owns semantic evaluation only. UI projection and persistence are
//! adapters over this result and must not implement a second evaluator.

use std::collections::BTreeMap;

use tachiko_formula_engine::{Calculation, CalculationOutcome, calculate_complete};
use tachiko_semantic_core::{
    Document, Entity, EntityId, FieldId, FieldRef, KeyedGroupedSumDefinitionId, Number, Value,
    validate_keyed_grouped_sum_definitions,
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
#[allow(
    clippy::too_many_lines,
    reason = "One evaluation stages the complete Product key index and any required authoritative calculation before reducing contributions, preserving complete diagnostics and an atomic outcome"
)]
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
        let candidates = product_index.get(lookup_key).map_or(&[][..], Vec::as_slice);
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
        let mut sum = Number::default();
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
            if let Some(value) = calculation.and_then(|state| state.value(&target)) {
                Some(value)
            } else {
                diagnostics.push(KeyedGroupedSumDiagnostic::input(
                    FORMULA_UNAVAILABLE,
                    &entity.id,
                    field,
                ));
                None
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
mod tests;
