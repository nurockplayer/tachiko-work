//! In-process canonical direct-state evidence for the accepted v2 profile.

use std::collections::BTreeMap;

use tachiko_formula_engine::{CalculationError, calculate};
use tachiko_semantic_core::{
    Diagnostic, Document, DocumentId, EntityId, FieldConstraint, FieldDefinition, FieldId,
    FieldKey, FieldType, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionError,
    KeyedGroupedSumDefinitionId, KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding,
    SchemaId, SchemaKey, Value, validate_complete_formula_constraints, validate_document_core,
    validate_keyed_grouped_sum_definitions,
};
use thiserror::Error;

/// The only canonical delta contract admitted by this implementation slice.
pub const CANONICAL_SEMANTIC_DELTA_V2: &str = "tachiko.semantic-delta/v2";

/// Which admitted input supplied a diagnostic or calculation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeltaInputSide {
    Before,
    After,
}

/// Failure to produce complete canonical evidence from the selected inputs.
#[derive(Debug, Error)]
pub enum CanonicalDeltaError {
    #[error("unsupported canonical semantic delta contract: {contract}")]
    UnsupportedContract { contract: String },
    #[error("canonical revision delta requires equal DocumentId values ({before} != {after})")]
    DocumentIdMismatch {
        before: DocumentId,
        after: DocumentId,
    },
    #[error("{side:?} document is not admitted for canonical delta: {diagnostics:?}")]
    InvalidInput {
        side: DeltaInputSide,
        diagnostics: Vec<Diagnostic>,
    },
    #[error("{side:?} keyed grouped-sum definitions are invalid: {source}")]
    InvalidKeyedGroupedSumDefinitions {
        side: DeltaInputSide,
        #[source]
        source: KeyedGroupedSumDefinitionError,
    },
    #[error("could not calculate the before document: {0}")]
    BeforeCalculation(#[source] CalculationError),
    #[error("could not calculate the after document: {0}")]
    AfterCalculation(#[source] CalculationError),
}

/// Complete immutable payload for a schema definition, excluding its target ID.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaDefinitionPayload {
    pub key: SchemaKey,
    pub display_name: Option<String>,
    pub fields: BTreeMap<FieldId, FieldDefinitionPayload>,
    pub field_order: Option<Vec<FieldId>>,
}

/// Complete immutable payload for a field definition, excluding its target ID.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDefinitionPayload {
    pub key: FieldKey,
    pub display_name: Option<String>,
    pub field_type: FieldType,
    pub required: bool,
    pub constraint: FieldConstraint,
    pub number_format: Option<tachiko_semantic_core::NumberFormat>,
}

/// Complete immutable payload for an entity state, excluding its target ID.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityDefinitionPayload {
    pub key: tachiko_semantic_core::EntityKey,
    pub schema: SchemaId,
    pub fields: BTreeMap<FieldId, Value>,
}

/// Complete direct meaning of one saved definition, excluding its typed target ID.
/// Presence is represented by the fact variant, never by an optional binding or result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyedGroupedSumDefinitionPayload {
    pub orders: KeyedGroupedSumOrdersBinding,
    pub products: KeyedGroupedSumProductsBinding,
}

impl From<&KeyedGroupedSumDefinition> for KeyedGroupedSumDefinitionPayload {
    fn from(definition: &KeyedGroupedSumDefinition) -> Self {
        Self {
            orders: definition.orders.clone(),
            products: definition.products.clone(),
        }
    }
}

/// Closed v2 vocabulary of direct semantic facts.
#[derive(Clone, Debug, PartialEq)]
pub enum CanonicalDirectFact {
    DocumentTitleChanged {
        document: DocumentId,
        before: String,
        after: String,
    },
    SchemaCreated {
        schema: SchemaId,
        definition: SchemaDefinitionPayload,
    },
    SchemaDeleted {
        schema: SchemaId,
        definition: SchemaDefinitionPayload,
    },
    SchemaKeyChanged {
        schema: SchemaId,
        before: SchemaKey,
        after: SchemaKey,
    },
    SchemaDisplayNameChanged {
        schema: SchemaId,
        before: Option<String>,
        after: Option<String>,
    },
    SchemaFieldOrderChanged {
        schema: SchemaId,
        before: Option<Vec<FieldId>>,
        after: Option<Vec<FieldId>>,
    },
    SchemaFieldDisplayNameChanged {
        schema: SchemaId,
        field: FieldId,
        before: Option<String>,
        after: Option<String>,
    },
    SchemaFieldCreated {
        schema: SchemaId,
        field: FieldId,
        definition: FieldDefinitionPayload,
    },
    SchemaFieldDeleted {
        schema: SchemaId,
        field: FieldId,
        definition: FieldDefinitionPayload,
    },
    SchemaFieldKeyChanged {
        schema: SchemaId,
        field: FieldId,
        before: FieldKey,
        after: FieldKey,
    },
    SchemaFieldTypeChanged {
        schema: SchemaId,
        field: FieldId,
        before: FieldType,
        after: FieldType,
    },
    SchemaFieldRequirednessChanged {
        schema: SchemaId,
        field: FieldId,
        before: bool,
        after: bool,
    },
    SchemaFieldNumberFormatChanged {
        schema: SchemaId,
        field: FieldId,
        before: Option<tachiko_semantic_core::NumberFormat>,
        after: Option<tachiko_semantic_core::NumberFormat>,
    },
    SchemaFieldConstraintChanged {
        schema: SchemaId,
        field: FieldId,
        before: FieldConstraint,
        after: FieldConstraint,
    },
    EntityCreated {
        entity: EntityId,
        definition: EntityDefinitionPayload,
    },
    EntityDeleted {
        entity: EntityId,
        definition: EntityDefinitionPayload,
    },
    EntityKeyChanged {
        entity: EntityId,
        before: tachiko_semantic_core::EntityKey,
        after: tachiko_semantic_core::EntityKey,
    },
    EntitySchemaChanged {
        entity: EntityId,
        before: SchemaId,
        after: SchemaId,
    },
    EntityFieldValueCreated {
        entity: EntityId,
        schema: SchemaId,
        field: FieldId,
        after: Value,
    },
    EntityFieldValueChanged {
        entity: EntityId,
        schema: SchemaId,
        field: FieldId,
        before: Value,
        after: Value,
    },
    EntityFieldValueCleared {
        entity: EntityId,
        schema: SchemaId,
        field: FieldId,
        before: Value,
    },
    KeyedGroupedSumDefinitionCreated {
        definition: KeyedGroupedSumDefinitionId,
        after: KeyedGroupedSumDefinitionPayload,
    },
    KeyedGroupedSumDefinitionDeleted {
        definition: KeyedGroupedSumDefinitionId,
        before: KeyedGroupedSumDefinitionPayload,
    },
    KeyedGroupedSumDefinitionChanged {
        definition: KeyedGroupedSumDefinitionId,
        before: KeyedGroupedSumDefinitionPayload,
        after: KeyedGroupedSumDefinitionPayload,
    },
}

impl CanonicalDirectFact {
    fn ordering_key(&self) -> (u8, Vec<&str>, u8) {
        use CanonicalDirectFact as F;
        match self {
            F::KeyedGroupedSumDefinitionCreated { definition, .. } => {
                (5, vec![definition.as_str()], 0)
            }
            F::KeyedGroupedSumDefinitionDeleted { definition, .. } => {
                (5, vec![definition.as_str()], 1)
            }
            F::KeyedGroupedSumDefinitionChanged { definition, .. } => {
                (5, vec![definition.as_str()], 2)
            }
            F::DocumentTitleChanged { document, .. } => (0, vec![document.as_str()], 0),
            F::SchemaCreated { schema, .. } => (1, vec![schema.as_str()], 0),
            F::SchemaDeleted { schema, .. } => (1, vec![schema.as_str()], 1),
            F::SchemaKeyChanged { schema, .. } => (1, vec![schema.as_str()], 2),
            F::SchemaDisplayNameChanged { schema, .. } => (1, vec![schema.as_str()], 3),
            F::SchemaFieldOrderChanged { schema, .. } => (1, vec![schema.as_str()], 4),
            F::SchemaFieldDisplayNameChanged { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 6)
            }
            F::SchemaFieldCreated { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 0)
            }
            F::SchemaFieldDeleted { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 1)
            }
            F::SchemaFieldKeyChanged { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 2)
            }
            F::SchemaFieldTypeChanged { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 3)
            }
            F::SchemaFieldRequirednessChanged { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 4)
            }
            F::SchemaFieldConstraintChanged { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 5)
            }
            F::SchemaFieldNumberFormatChanged { schema, field, .. } => {
                (2, vec![schema.as_str(), field.as_str()], 7)
            }
            F::EntityCreated { entity, .. } => (3, vec![entity.as_str()], 0),
            F::EntityDeleted { entity, .. } => (3, vec![entity.as_str()], 1),
            F::EntityKeyChanged { entity, .. } => (3, vec![entity.as_str()], 2),
            F::EntitySchemaChanged { entity, .. } => (3, vec![entity.as_str()], 3),
            F::EntityFieldValueCreated {
                entity,
                schema,
                field,
                ..
            } => (4, vec![entity.as_str(), schema.as_str(), field.as_str()], 0),
            F::EntityFieldValueChanged {
                entity,
                schema,
                field,
                ..
            } => (4, vec![entity.as_str(), schema.as_str(), field.as_str()], 1),
            F::EntityFieldValueCleared {
                entity,
                schema,
                field,
                ..
            } => (4, vec![entity.as_str(), schema.as_str(), field.as_str()], 2),
        }
    }
}

/// Canonically ordered direct evidence for one continuing document.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalSemanticDelta {
    document_id: DocumentId,
    facts: Vec<CanonicalDirectFact>,
}

impl CanonicalSemanticDelta {
    #[must_use]
    pub const fn contract(&self) -> &'static str {
        CANONICAL_SEMANTIC_DELTA_V2
    }

    #[must_use]
    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    #[must_use]
    pub fn facts(&self) -> &[CanonicalDirectFact] {
        &self.facts
    }
}

/// Compare admitted document states and produce complete v2 direct facts.
///
/// # Errors
/// Returns an explicit refusal if the selector, document scope, either input,
/// complete calculation, or saved-definition binding validation fails.
pub fn canonical_delta(
    contract: &str,
    before: &Document,
    after: &Document,
) -> Result<CanonicalSemanticDelta, CanonicalDeltaError> {
    if contract != CANONICAL_SEMANTIC_DELTA_V2 {
        return Err(CanonicalDeltaError::UnsupportedContract {
            contract: contract.to_owned(),
        });
    }
    if before.id != after.id {
        return Err(CanonicalDeltaError::DocumentIdMismatch {
            before: before.id.clone(),
            after: after.id.clone(),
        });
    }
    admit_input(before, DeltaInputSide::Before)?;
    admit_input(after, DeltaInputSide::After)?;
    let before_values = calculate(before).map_err(CanonicalDeltaError::BeforeCalculation)?;
    let after_values = calculate(after).map_err(CanonicalDeltaError::AfterCalculation)?;
    for (document, side, calculation) in [
        (before, DeltaInputSide::Before, &before_values),
        (after, DeltaInputSide::After, &after_values),
    ] {
        let diagnostics = validate_complete_formula_constraints(document, calculation.values());
        if !diagnostics.is_empty() {
            return Err(CanonicalDeltaError::InvalidInput { side, diagnostics });
        }
    }
    validate_keyed_grouped_sum_definitions(before).map_err(|source| {
        CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::Before,
            source,
        }
    })?;
    validate_keyed_grouped_sum_definitions(after).map_err(|source| {
        CanonicalDeltaError::InvalidKeyedGroupedSumDefinitions {
            side: DeltaInputSide::After,
            source,
        }
    })?;
    let mut facts = Vec::new();
    compare_direct_state(before, after, &mut facts);
    compare_saved_definitions(before, after, &mut facts);
    facts.sort_by(|left, right| left.ordering_key().cmp(&right.ordering_key()));
    Ok(CanonicalSemanticDelta {
        document_id: before.id.clone(),
        facts,
    })
}

fn admit_input(document: &Document, side: DeltaInputSide) -> Result<(), CanonicalDeltaError> {
    let diagnostics = validate_document_core(document);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(CanonicalDeltaError::InvalidInput { side, diagnostics })
    }
}

fn schema_payload(schema: &tachiko_semantic_core::Schema) -> SchemaDefinitionPayload {
    SchemaDefinitionPayload {
        key: schema.key.clone(),
        display_name: schema.display_name.clone(),
        fields: schema
            .fields
            .iter()
            .map(|(id, field)| (id.clone(), field_payload(field)))
            .collect(),
        field_order: schema.field_order.clone(),
    }
}

fn field_payload(field: &FieldDefinition) -> FieldDefinitionPayload {
    FieldDefinitionPayload {
        key: field.key.clone(),
        display_name: field.display_name.clone(),
        field_type: field.field_type.clone(),
        required: field.required,
        constraint: field.constraint.clone(),
        number_format: field.number_format.clone(),
    }
}

fn entity_payload(entity: &tachiko_semantic_core::Entity) -> EntityDefinitionPayload {
    EntityDefinitionPayload {
        key: entity.key.clone(),
        schema: entity.schema.clone(),
        fields: entity.fields.clone(),
    }
}

fn compare_direct_state(before: &Document, after: &Document, facts: &mut Vec<CanonicalDirectFact>) {
    use CanonicalDirectFact as F;
    if before.title != after.title {
        facts.push(F::DocumentTitleChanged {
            document: before.id.clone(),
            before: before.title.clone(),
            after: after.title.clone(),
        });
    }
    let schemas = before
        .schemas
        .keys()
        .chain(after.schemas.keys())
        .collect::<std::collections::BTreeSet<_>>();
    for schema_id in schemas {
        match (before.schemas.get(schema_id), after.schemas.get(schema_id)) {
            (None, Some(schema)) => facts.push(F::SchemaCreated {
                schema: schema_id.clone(),
                definition: schema_payload(schema),
            }),
            (Some(schema), None) => facts.push(F::SchemaDeleted {
                schema: schema_id.clone(),
                definition: schema_payload(schema),
            }),
            (Some(old), Some(new)) => compare_schema(schema_id, old, new, facts),
            (None, None) => unreachable!(),
        }
    }
    let entities = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .collect::<std::collections::BTreeSet<_>>();
    for entity_id in entities {
        match (
            before.entities.get(entity_id),
            after.entities.get(entity_id),
        ) {
            (None, Some(entity)) => facts.push(F::EntityCreated {
                entity: entity_id.clone(),
                definition: entity_payload(entity),
            }),
            (Some(entity), None) => facts.push(F::EntityDeleted {
                entity: entity_id.clone(),
                definition: entity_payload(entity),
            }),
            (Some(old), Some(new)) => compare_entity(entity_id, old, new, facts),
            (None, None) => unreachable!(),
        }
    }
}

fn compare_saved_definitions(
    before: &Document,
    after: &Document,
    facts: &mut Vec<CanonicalDirectFact>,
) {
    use CanonicalDirectFact as F;
    let ids = before
        .keyed_grouped_sum_definitions
        .keys()
        .chain(after.keyed_grouped_sum_definitions.keys())
        .collect::<std::collections::BTreeSet<_>>();
    for id in ids {
        match (
            before.keyed_grouped_sum_definitions.get(id),
            after.keyed_grouped_sum_definitions.get(id),
        ) {
            (None, Some(new)) => facts.push(F::KeyedGroupedSumDefinitionCreated {
                definition: id.clone(),
                after: new.into(),
            }),
            (Some(old), None) => facts.push(F::KeyedGroupedSumDefinitionDeleted {
                definition: id.clone(),
                before: old.into(),
            }),
            (Some(old), Some(new)) if old != new => {
                facts.push(F::KeyedGroupedSumDefinitionChanged {
                    definition: id.clone(),
                    before: old.into(),
                    after: new.into(),
                });
            }
            _ => {}
        }
    }
}

fn compare_schema(
    id: &SchemaId,
    before: &tachiko_semantic_core::Schema,
    after: &tachiko_semantic_core::Schema,
    facts: &mut Vec<CanonicalDirectFact>,
) {
    use CanonicalDirectFact as F;
    if before.key != after.key {
        facts.push(F::SchemaKeyChanged {
            schema: id.clone(),
            before: before.key.clone(),
            after: after.key.clone(),
        });
    }
    if before.display_name != after.display_name {
        facts.push(F::SchemaDisplayNameChanged {
            schema: id.clone(),
            before: before.display_name.clone(),
            after: after.display_name.clone(),
        });
    }
    if before.field_order != after.field_order {
        facts.push(F::SchemaFieldOrderChanged {
            schema: id.clone(),
            before: before.field_order.clone(),
            after: after.field_order.clone(),
        });
    }
    let fields = before
        .fields
        .keys()
        .chain(after.fields.keys())
        .collect::<std::collections::BTreeSet<_>>();
    for field_id in fields {
        match (before.fields.get(field_id), after.fields.get(field_id)) {
            (None, Some(field)) => facts.push(F::SchemaFieldCreated {
                schema: id.clone(),
                field: field_id.clone(),
                definition: field_payload(field),
            }),
            (Some(field), None) => facts.push(F::SchemaFieldDeleted {
                schema: id.clone(),
                field: field_id.clone(),
                definition: field_payload(field),
            }),
            (Some(old), Some(new)) => {
                if old.key != new.key {
                    facts.push(F::SchemaFieldKeyChanged {
                        schema: id.clone(),
                        field: field_id.clone(),
                        before: old.key.clone(),
                        after: new.key.clone(),
                    });
                }
                if old.display_name != new.display_name {
                    facts.push(F::SchemaFieldDisplayNameChanged {
                        schema: id.clone(),
                        field: field_id.clone(),
                        before: old.display_name.clone(),
                        after: new.display_name.clone(),
                    });
                }
                if old.field_type != new.field_type {
                    facts.push(F::SchemaFieldTypeChanged {
                        schema: id.clone(),
                        field: field_id.clone(),
                        before: old.field_type.clone(),
                        after: new.field_type.clone(),
                    });
                }
                if old.required != new.required {
                    facts.push(F::SchemaFieldRequirednessChanged {
                        schema: id.clone(),
                        field: field_id.clone(),
                        before: old.required,
                        after: new.required,
                    });
                }
                if old.number_format != new.number_format {
                    facts.push(F::SchemaFieldNumberFormatChanged {
                        schema: id.clone(),
                        field: field_id.clone(),
                        before: old.number_format.clone(),
                        after: new.number_format.clone(),
                    });
                }
                if old.constraint != new.constraint {
                    facts.push(F::SchemaFieldConstraintChanged {
                        schema: id.clone(),
                        field: field_id.clone(),
                        before: old.constraint.clone(),
                        after: new.constraint.clone(),
                    });
                }
            }
            (None, None) => unreachable!(),
        }
    }
}

fn compare_entity(
    id: &EntityId,
    before: &tachiko_semantic_core::Entity,
    after: &tachiko_semantic_core::Entity,
    facts: &mut Vec<CanonicalDirectFact>,
) {
    use CanonicalDirectFact as F;
    if before.key != after.key {
        facts.push(F::EntityKeyChanged {
            entity: id.clone(),
            before: before.key.clone(),
            after: after.key.clone(),
        });
    }
    if before.schema != after.schema {
        facts.push(F::EntitySchemaChanged {
            entity: id.clone(),
            before: before.schema.clone(),
            after: after.schema.clone(),
        });
    }
    let fields = before
        .fields
        .keys()
        .chain(after.fields.keys())
        .collect::<std::collections::BTreeSet<_>>();
    for field_id in fields {
        let before_value = before.fields.get(field_id);
        let after_value = after.fields.get(field_id);
        match (before_value, after_value) {
            (None, Some(value)) => facts.push(F::EntityFieldValueCreated {
                entity: id.clone(),
                schema: after.schema.clone(),
                field: field_id.clone(),
                after: value.clone(),
            }),
            (Some(value), None) => facts.push(F::EntityFieldValueCleared {
                entity: id.clone(),
                schema: before.schema.clone(),
                field: field_id.clone(),
                before: value.clone(),
            }),
            (Some(old), Some(new)) if old != new && before.schema == after.schema => {
                facts.push(F::EntityFieldValueChanged {
                    entity: id.clone(),
                    schema: before.schema.clone(),
                    field: field_id.clone(),
                    before: old.clone(),
                    after: new.clone(),
                });
            }
            (Some(old), Some(new)) if before.schema != after.schema => {
                facts.push(F::EntityFieldValueCleared {
                    entity: id.clone(),
                    schema: before.schema.clone(),
                    field: field_id.clone(),
                    before: old.clone(),
                });
                facts.push(F::EntityFieldValueCreated {
                    entity: id.clone(),
                    schema: after.schema.clone(),
                    field: field_id.clone(),
                    after: new.clone(),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
