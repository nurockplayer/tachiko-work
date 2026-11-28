use std::collections::{BTreeMap, BTreeSet};

use super::{
    ConditionalRule, MAX_RULES_PER_TABLE, MAX_STYLES, PresentationError, PresentationStore, RuleId,
    rule_rank_is_valid, validate_rule_condition,
};
use crate::{Document, SchemaId};

pub(super) fn validate_rule_inventory(
    store: &PresentationStore,
    document: &Document,
) -> Result<(), PresentationError> {
    if store.styles.len() > MAX_STYLES {
        return Err(PresentationError::TooManyStyles {
            found: store.styles.len(),
            limit: MAX_STYLES,
        });
    }

    let mut rules_per_schema = BTreeMap::<&SchemaId, usize>::new();
    for (rule_id, rule) in &store.rules {
        if !document.schemas.contains_key(&rule.schema) {
            return Err(PresentationError::MissingSchema {
                schema: rule.schema.clone(),
            });
        }
        *rules_per_schema.entry(&rule.schema).or_default() += 1;
        validate_rule_fields(rule_id, rule)?;
    }
    for (schema, found) in &rules_per_schema {
        if *found > MAX_RULES_PER_TABLE {
            return Err(PresentationError::TooManyRules {
                schema: (**schema).clone(),
                found: *found,
                limit: MAX_RULES_PER_TABLE,
            });
        }
    }

    Ok(())
}

fn validate_rule_fields(rule_id: &RuleId, rule: &ConditionalRule) -> Result<(), PresentationError> {
    if rule.fields.is_empty() {
        return Err(PresentationError::EmptyRuleFields {
            rule: rule_id.clone(),
        });
    }
    let mut seen = BTreeSet::new();
    for field in &rule.fields {
        if !seen.insert(field) {
            return Err(PresentationError::DuplicateRuleField {
                rule: rule_id.clone(),
                field: field.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_rule(
    rule_id: &RuleId,
    rule: &ConditionalRule,
    store: &PresentationStore,
    document: &Document,
) -> Result<(), PresentationError> {
    let schema =
        document
            .schemas
            .get(&rule.schema)
            .ok_or_else(|| PresentationError::MissingSchema {
                schema: rule.schema.clone(),
            })?;
    if !rule_rank_is_valid(&rule.rank) {
        return Err(PresentationError::InvalidRank {
            rule: rule_id.clone(),
            rank: rule.rank.clone(),
        });
    }
    for field in &rule.fields {
        if !schema.fields.contains_key(field) {
            return Err(PresentationError::MissingField {
                schema: rule.schema.clone(),
                field: field.clone(),
            });
        }
    }
    if !store.styles.contains_key(&rule.style) {
        return Err(PresentationError::MissingStyle {
            style: rule.style.clone(),
        });
    }
    validate_rule_condition(rule_id, schema, &rule.condition, document)?;

    Ok(())
}

pub(super) fn validate_cell_references(
    store: &PresentationStore,
    document: &Document,
) -> Result<(), PresentationError> {
    for ((entity_id, field), style) in &store.cells {
        let entity =
            document
                .entities
                .get(entity_id)
                .ok_or_else(|| PresentationError::MissingEntity {
                    entity: entity_id.clone(),
                })?;
        let Some(schema) = document.schemas.get(&entity.schema) else {
            return Err(PresentationError::MissingSchema {
                schema: entity.schema.clone(),
            });
        };
        if !schema.fields.contains_key(field) {
            return Err(PresentationError::CellFieldNotOnEntitySchema {
                entity: entity_id.clone(),
                field: field.clone(),
            });
        }
        if !store.styles.contains_key(style) {
            return Err(PresentationError::MissingStyle {
                style: style.clone(),
            });
        }
    }

    Ok(())
}

pub(super) fn validate_style_usage(store: &PresentationStore) -> Result<(), PresentationError> {
    let mut used = BTreeSet::new();
    for style in store
        .tables
        .values()
        .flat_map(|table| table.column_styles.values())
    {
        used.insert(style.clone());
    }
    for style in store.rules.values().map(|rule| &rule.style) {
        used.insert(style.clone());
    }
    for style in store.cells.values() {
        used.insert(style.clone());
    }
    for style in store.styles.keys() {
        if !used.contains(style) {
            return Err(PresentationError::UnusedStyle {
                style: style.clone(),
            });
        }
    }

    Ok(())
}
