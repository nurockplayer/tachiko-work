use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Document, FieldId, FieldType, SchemaId};

/// Stable opaque identity of one saved keyed grouped-sum definition.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct KeyedGroupedSumDefinitionId(String);

impl KeyedGroupedSumDefinitionId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for KeyedGroupedSumDefinitionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for KeyedGroupedSumDefinitionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for KeyedGroupedSumDefinitionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable Orders-side bindings for the single Accepted saved definition family.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyedGroupedSumOrdersBinding {
    pub schema: SchemaId,
    pub lookup_key_field: FieldId,
    pub quantity_field: FieldId,
}

/// Stable Products-side bindings for the single Accepted saved definition family.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyedGroupedSumProductsBinding {
    pub schema: SchemaId,
    pub key_field: FieldId,
    pub category_field: FieldId,
    pub price_field: FieldId,
}

/// Saved live keyed lookup + category grouped-sum meaning admitted by ADR-0036.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyedGroupedSumDefinition {
    pub id: KeyedGroupedSumDefinitionId,
    pub orders: KeyedGroupedSumOrdersBinding,
    pub products: KeyedGroupedSumProductsBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyedGroupedSumBindingRole {
    OrdersLookupKey,
    OrdersQuantity,
    ProductsKey,
    ProductsCategory,
    ProductsPrice,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyedGroupedSumDefinitionError {
    EmptyId,
    StoreIdMismatch {
        stored: KeyedGroupedSumDefinitionId,
        declared: KeyedGroupedSumDefinitionId,
    },
    MissingSchema {
        role: &'static str,
        schema: SchemaId,
    },
    MissingField {
        role: KeyedGroupedSumBindingRole,
        schema: SchemaId,
        field: FieldId,
    },
    WrongFieldType {
        role: KeyedGroupedSumBindingRole,
        schema: SchemaId,
        field: FieldId,
        expected: FieldType,
        actual: FieldType,
    },
}

impl fmt::Display for KeyedGroupedSumDefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => {
                formatter.write_str("keyed grouped-sum definition id must not be empty")
            }
            Self::StoreIdMismatch { stored, declared } => write!(
                formatter,
                "keyed grouped-sum store id '{stored}' does not match declared id '{declared}'"
            ),
            Self::MissingSchema { role, schema } => {
                write!(formatter, "{role} schema '{schema}' does not exist")
            }
            Self::MissingField {
                role,
                schema,
                field,
            } => write!(
                formatter,
                "{role:?} field '{field}' does not exist in schema '{schema}'"
            ),
            Self::WrongFieldType {
                role,
                schema,
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{role:?} field '{field}' in schema '{schema}' must be {expected:?}, found {actual:?}"
            ),
        }
    }
}

impl std::error::Error for KeyedGroupedSumDefinitionError {}

/// Validate every saved definition and its stable binding roles against one snapshot.
///
/// # Errors
///
/// Returns the first stable-definition or binding-role failure in definition-ID order.
pub fn validate_keyed_grouped_sum_definitions(
    document: &Document,
) -> Result<(), KeyedGroupedSumDefinitionError> {
    for (stored_id, definition) in &document.keyed_grouped_sum_definitions {
        if stored_id.as_str().is_empty() || definition.id.as_str().is_empty() {
            return Err(KeyedGroupedSumDefinitionError::EmptyId);
        }
        if stored_id != &definition.id {
            return Err(KeyedGroupedSumDefinitionError::StoreIdMismatch {
                stored: stored_id.clone(),
                declared: definition.id.clone(),
            });
        }
        validate_definition(document, definition)?;
    }
    Ok(())
}

fn validate_definition(
    document: &Document,
    definition: &KeyedGroupedSumDefinition,
) -> Result<(), KeyedGroupedSumDefinitionError> {
    let orders = document
        .schemas
        .get(&definition.orders.schema)
        .ok_or_else(|| KeyedGroupedSumDefinitionError::MissingSchema {
            role: "orders",
            schema: definition.orders.schema.clone(),
        })?;
    let products = document
        .schemas
        .get(&definition.products.schema)
        .ok_or_else(|| KeyedGroupedSumDefinitionError::MissingSchema {
            role: "products",
            schema: definition.products.schema.clone(),
        })?;

    require_type(
        orders,
        &definition.orders.lookup_key_field,
        KeyedGroupedSumBindingRole::OrdersLookupKey,
        FieldType::Text,
    )?;
    require_type(
        orders,
        &definition.orders.quantity_field,
        KeyedGroupedSumBindingRole::OrdersQuantity,
        FieldType::Number,
    )?;
    require_type(
        products,
        &definition.products.key_field,
        KeyedGroupedSumBindingRole::ProductsKey,
        FieldType::Text,
    )?;
    require_type(
        products,
        &definition.products.category_field,
        KeyedGroupedSumBindingRole::ProductsCategory,
        FieldType::Text,
    )?;
    require_type(
        products,
        &definition.products.price_field,
        KeyedGroupedSumBindingRole::ProductsPrice,
        FieldType::Number,
    )?;
    Ok(())
}

fn require_type(
    schema: &crate::Schema,
    field: &FieldId,
    role: KeyedGroupedSumBindingRole,
    expected: FieldType,
) -> Result<(), KeyedGroupedSumDefinitionError> {
    let definition =
        schema
            .fields
            .get(field)
            .ok_or_else(|| KeyedGroupedSumDefinitionError::MissingField {
                role,
                schema: schema.id.clone(),
                field: field.clone(),
            })?;
    if definition.field_type != expected {
        return Err(KeyedGroupedSumDefinitionError::WrongFieldType {
            role,
            schema: schema.id.clone(),
            field: field.clone(),
            expected,
            actual: definition.field_type.clone(),
        });
    }
    Ok(())
}
