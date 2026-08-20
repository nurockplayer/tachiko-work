use std::{borrow::Borrow, collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

macro_rules! identifier {
    ($name:ident) => {
        #[derive(
            Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

identifier!(DocumentId);
identifier!(SchemaId);
identifier!(EntityId);
identifier!(FieldId);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub id: DocumentId,
    pub title: String,
    pub schemas: BTreeMap<SchemaId, Schema>,
    pub entities: BTreeMap<EntityId, Entity>,
}

impl Document {
    #[must_use]
    pub fn empty(id: impl Into<DocumentId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            schemas: BTreeMap::new(),
            entities: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub id: SchemaId,
    pub fields: BTreeMap<FieldId, FieldDefinition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDefinition {
    pub field_type: FieldType,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    Number,
    Text,
    Boolean,
    Reference { schema: SchemaId },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub id: EntityId,
    pub schema: SchemaId,
    pub fields: BTreeMap<FieldId, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Number(f64),
    Text(String),
    Boolean(bool),
    Reference(EntityId),
    Formula(Expression),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum Expression {
    Number(f64),
    Reference(FieldRef),
    Add { left: Box<Self>, right: Box<Self> },
    Subtract { left: Box<Self>, right: Box<Self> },
    Multiply { left: Box<Self>, right: Box<Self> },
    Divide { left: Box<Self>, right: Box<Self> },
    Minimum { left: Box<Self>, right: Box<Self> },
    Maximum { left: Box<Self>, right: Box<Self> },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRef {
    pub entity: EntityId,
    pub field: FieldId,
}

impl FieldRef {
    #[must_use]
    pub fn new(entity: impl Into<EntityId>, field: impl Into<FieldId>) -> Self {
        Self {
            entity: entity.into(),
            field: field.into(),
        }
    }
}

impl fmt::Display for FieldRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.entity, self.field)
    }
}
