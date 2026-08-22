use std::{collections::BTreeMap, fmt};

use crate::{
    Document, EntityId, EntityKey, FieldAddress, FieldId, FieldKey, FieldRef, SchemaId, SchemaKey,
};

/// Deterministic runtime-only human-address lookup state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressIndex {
    schemas: BTreeMap<SchemaKey, SchemaId>,
    entities: BTreeMap<EntityKey, EntityId>,
    fields: BTreeMap<SchemaId, BTreeMap<FieldKey, FieldId>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddressIndexError {
    SchemaStoreIdMismatch {
        stored: SchemaId,
        nested: SchemaId,
    },
    FieldStoreIdMismatch {
        schema: SchemaId,
        stored: FieldId,
        nested: FieldId,
    },
    EntityStoreIdMismatch {
        stored: EntityId,
        nested: EntityId,
    },
    DuplicateSchemaKey {
        key: SchemaKey,
        ids: Vec<SchemaId>,
    },
    DuplicateEntityKey {
        key: EntityKey,
        ids: Vec<EntityId>,
    },
    DuplicateFieldKey {
        schema: SchemaId,
        key: FieldKey,
        ids: Vec<FieldId>,
    },
    MissingSchemaKey {
        key: SchemaKey,
    },
    MissingEntityKey {
        key: EntityKey,
    },
    MissingFieldKey {
        entity: EntityKey,
        field: FieldKey,
    },
    MissingSchemaFieldKey {
        schema: SchemaId,
        field: FieldKey,
    },
    MissingBoundEntity {
        entity: EntityId,
    },
    MissingBoundSchema {
        schema: SchemaId,
    },
    MissingBoundField {
        entity: EntityId,
        field: FieldId,
    },
    BoundAddressMismatch {
        reference: FieldRef,
    },
}

impl AddressIndex {
    /// Build derived address lookup state from one document snapshot.
    ///
    /// # Errors
    ///
    /// Returns the first duplicate category in deterministic schema/field/entity
    /// order, with conflicting stable IDs sorted by their opaque tokens.
    pub fn build(document: &Document) -> Result<Self, AddressIndexError> {
        for (stored, schema) in &document.schemas {
            if stored != &schema.id {
                return Err(AddressIndexError::SchemaStoreIdMismatch {
                    stored: stored.clone(),
                    nested: schema.id.clone(),
                });
            }
            for (field_stored, field) in &schema.fields {
                if field_stored != &field.id {
                    return Err(AddressIndexError::FieldStoreIdMismatch {
                        schema: stored.clone(),
                        stored: field_stored.clone(),
                        nested: field.id.clone(),
                    });
                }
            }
        }
        for (stored, entity) in &document.entities {
            if stored != &entity.id {
                return Err(AddressIndexError::EntityStoreIdMismatch {
                    stored: stored.clone(),
                    nested: entity.id.clone(),
                });
            }
        }

        let schema_groups = document.schemas.values().fold(
            BTreeMap::<SchemaKey, Vec<SchemaId>>::new(),
            |mut groups, schema| {
                groups
                    .entry(schema.key.clone())
                    .or_default()
                    .push(schema.id.clone());
                groups
            },
        );
        if let Some((key, ids)) = schema_groups.iter().find(|(_, ids)| ids.len() > 1) {
            return Err(AddressIndexError::DuplicateSchemaKey {
                key: key.clone(),
                ids: ids.clone(),
            });
        }

        let mut fields = BTreeMap::new();
        for (schema_id, schema) in &document.schemas {
            let field_groups = schema.fields.values().fold(
                BTreeMap::<FieldKey, Vec<FieldId>>::new(),
                |mut groups, field| {
                    groups
                        .entry(field.key.clone())
                        .or_default()
                        .push(field.id.clone());
                    groups
                },
            );
            if let Some((key, ids)) = field_groups.iter().find(|(_, ids)| ids.len() > 1) {
                return Err(AddressIndexError::DuplicateFieldKey {
                    schema: schema_id.clone(),
                    key: key.clone(),
                    ids: ids.clone(),
                });
            }
            fields.insert(
                schema_id.clone(),
                field_groups
                    .into_iter()
                    .map(|(key, mut ids)| (key, ids.remove(0)))
                    .collect(),
            );
        }

        let entity_groups = document.entities.values().fold(
            BTreeMap::<EntityKey, Vec<EntityId>>::new(),
            |mut groups, entity| {
                groups
                    .entry(entity.key.clone())
                    .or_default()
                    .push(entity.id.clone());
                groups
            },
        );
        if let Some((key, ids)) = entity_groups.iter().find(|(_, ids)| ids.len() > 1) {
            return Err(AddressIndexError::DuplicateEntityKey {
                key: key.clone(),
                ids: ids.clone(),
            });
        }

        Ok(Self {
            schemas: schema_groups
                .into_iter()
                .map(|(key, mut ids)| (key, ids.remove(0)))
                .collect(),
            entities: entity_groups
                .into_iter()
                .map(|(key, mut ids)| (key, ids.remove(0)))
                .collect(),
            fields,
        })
    }

    /// Resolve a human schema key to its stable schema id.
    ///
    /// # Errors
    ///
    /// Returns [`AddressIndexError::MissingSchemaKey`] when the key is absent.
    pub fn schema_id(&self, key: &SchemaKey) -> Result<&SchemaId, AddressIndexError> {
        self.schemas
            .get(key)
            .ok_or_else(|| AddressIndexError::MissingSchemaKey { key: key.clone() })
    }

    /// Resolve a human entity key to its stable entity id.
    ///
    /// # Errors
    ///
    /// Returns [`AddressIndexError::MissingEntityKey`] when the key is absent.
    pub fn entity_id(&self, key: &EntityKey) -> Result<&EntityId, AddressIndexError> {
        self.entities
            .get(key)
            .ok_or_else(|| AddressIndexError::MissingEntityKey { key: key.clone() })
    }

    /// Resolve a human field key within a stable schema.
    ///
    /// # Errors
    ///
    /// Returns [`AddressIndexError::MissingFieldKey`] when the key is absent.
    pub fn field_id(
        &self,
        schema: &SchemaId,
        entity: &EntityKey,
        key: &FieldKey,
    ) -> Result<&FieldId, AddressIndexError> {
        self.fields
            .get(schema)
            .and_then(|fields| fields.get(key))
            .ok_or_else(|| AddressIndexError::MissingFieldKey {
                entity: entity.clone(),
                field: key.clone(),
            })
    }

    /// Resolve a field key in a schema without fabricating an entity address.
    ///
    /// # Errors
    ///
    /// Returns a schema-scoped missing-field error.
    pub fn schema_field_id(
        &self,
        schema: &SchemaId,
        key: &FieldKey,
    ) -> Result<&FieldId, AddressIndexError> {
        self.fields
            .get(schema)
            .and_then(|fields| fields.get(key))
            .ok_or_else(|| AddressIndexError::MissingSchemaFieldKey {
                schema: schema.clone(),
                field: key.clone(),
            })
    }

    /// Resolve one human address to a stable bound field reference.
    ///
    /// # Errors
    ///
    /// Returns a typed missing/duplicate address error.
    pub fn resolve_field(
        &self,
        document: &Document,
        address: &FieldAddress,
    ) -> Result<FieldRef, AddressIndexError> {
        let entity_id = self.entity_id(&address.entity)?.clone();
        let entity = document.entities.get(&entity_id).ok_or_else(|| {
            AddressIndexError::MissingBoundEntity {
                entity: entity_id.clone(),
            }
        })?;
        let field_id = self
            .field_id(&entity.schema, &address.entity, &address.field)?
            .clone();
        Ok(FieldRef::new(entity_id, field_id))
    }

    /// Project a stable reference through the current human address and prove
    /// that resolving the address returns the same IDs.
    ///
    /// # Errors
    ///
    /// Returns a typed stable-target or round-trip mismatch error.
    pub fn field_address(
        &self,
        document: &Document,
        reference: &FieldRef,
    ) -> Result<FieldAddress, AddressIndexError> {
        let entity = document.entities.get(&reference.entity).ok_or_else(|| {
            AddressIndexError::MissingBoundEntity {
                entity: reference.entity.clone(),
            }
        })?;
        let schema = document.schemas.get(&entity.schema).ok_or_else(|| {
            AddressIndexError::MissingBoundSchema {
                schema: entity.schema.clone(),
            }
        })?;
        let field = schema.fields.get(&reference.field).ok_or_else(|| {
            AddressIndexError::MissingBoundField {
                entity: reference.entity.clone(),
                field: reference.field.clone(),
            }
        })?;
        let address = FieldAddress {
            entity: entity.key.clone(),
            field: field.key.clone(),
        };
        if self.resolve_field(document, &address)? != *reference {
            return Err(AddressIndexError::BoundAddressMismatch {
                reference: reference.clone(),
            });
        }
        Ok(address)
    }
}

impl Document {
    /// Resolve a human field address against a freshly derived deterministic
    /// index for this snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed duplicate or missing-address error.
    pub fn resolve_field(&self, address: &FieldAddress) -> Result<FieldRef, AddressIndexError> {
        AddressIndex::build(self)?.resolve_field(self, address)
    }

    /// Project a bound reference to its current human address with a round-trip
    /// identity proof.
    ///
    /// # Errors
    ///
    /// Returns a typed duplicate, stale-target, or mismatch error.
    pub fn field_address(&self, reference: &FieldRef) -> Result<FieldAddress, AddressIndexError> {
        AddressIndex::build(self)?.field_address(self, reference)
    }
}

impl fmt::Display for AddressIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AddressIndexError {}
