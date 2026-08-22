//! Immutable storage DTOs for the legacy direct `.ro` JSON v1 profile.

use std::collections::BTreeMap;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::SerializeStruct,
};
use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
    FieldType, Schema, SchemaId, Value,
};

const FORMAT_VERSION: u32 = 1;

/// Errors specific to interpreting the frozen v1 storage representation.
#[derive(Debug)]
pub(crate) enum CodecError {
    Json(serde_json::Error),
    InvalidRepresentation(String),
}

/// Decode the storage-owned direct `.ro` v1 DTO, then convert it explicitly
/// into the semantic document contract.
pub(crate) fn decode(source: &str) -> Result<Document, CodecError> {
    let document = decode_dto(source)?;
    Ok(document.into_semantic())
}

/// Decode and validate the immutable v1 DTO without crossing into the current
/// semantic model. Future explicit migrations consume this seam so they can
/// map every legacy address before rewriting any typed relationship.
pub(crate) fn decode_dto(source: &str) -> Result<DocumentV1, CodecError> {
    let document: DocumentV1 = serde_json::from_str(source).map_err(CodecError::Json)?;
    document.validate()?;
    Ok(document)
}

/// Encode a semantic document through the frozen storage-owned direct `.ro`
/// v1 DTO.
pub(crate) fn encode(document: &Document) -> Result<String, CodecError> {
    let document = DocumentV1::from_semantic(document);
    encode_dto(&document)
}

/// Validate and canonically encode a v1 DTO after conversion or a future
/// explicit migration step.
pub(crate) fn encode_dto(document: &DocumentV1) -> Result<String, CodecError> {
    document.validate()?;
    let mut encoded = serde_json::to_string_pretty(document).map_err(CodecError::Json)?;
    encoded.push('\n');
    Ok(encoded)
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct DocumentIdV1(pub(crate) String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct SchemaIdV1(pub(crate) String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct EntityIdV1(pub(crate) String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct FieldIdV1(pub(crate) String);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentV1 {
    pub(crate) format_version: u32,
    pub(crate) id: DocumentIdV1,
    pub(crate) title: String,
    pub(crate) schemas: BTreeMap<SchemaIdV1, SchemaV1>,
    pub(crate) entities: BTreeMap<EntityIdV1, EntityV1>,
}

impl DocumentV1 {
    pub(crate) fn validate(&self) -> Result<(), CodecError> {
        if self.format_version != FORMAT_VERSION {
            return invalid(format!(
                "format_version must be {FORMAT_VERSION}, found {}",
                self.format_version
            ));
        }

        validate_id("document id", &self.id.0)?;

        for (schema_id, schema) in &self.schemas {
            validate_id("schema map key", &schema_id.0)?;
            schema.validate(schema_id, &self.schemas)?;
        }

        for (entity_id, entity) in &self.entities {
            validate_id("entity map key", &entity_id.0)?;
            entity.validate(entity_id, &self.schemas)?;
        }

        Ok(())
    }

    pub(crate) fn into_semantic(self) -> Document {
        Document {
            id: DocumentId::from(self.id.0),
            title: self.title,
            schemas: self
                .schemas
                .into_iter()
                .map(|(id, schema)| (SchemaId::from(id.0), schema.into_semantic()))
                .collect(),
            entities: self
                .entities
                .into_iter()
                .map(|(id, entity)| (EntityId::from(id.0), entity.into_semantic()))
                .collect(),
        }
    }

    pub(crate) fn from_semantic(document: &Document) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            id: DocumentIdV1(document.id.as_str().to_owned()),
            title: document.title.clone(),
            schemas: document
                .schemas
                .iter()
                .map(|(id, schema)| {
                    (
                        SchemaIdV1(id.as_str().to_owned()),
                        SchemaV1::from_semantic(schema),
                    )
                })
                .collect(),
            entities: document
                .entities
                .iter()
                .map(|(id, entity)| {
                    (
                        EntityIdV1(id.as_str().to_owned()),
                        EntityV1::from_semantic(entity),
                    )
                })
                .collect(),
        }
    }
}

impl Serialize for DocumentV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("DocumentV1", 5)?;
        record.serialize_field("format_version", &self.format_version)?;
        record.serialize_field("id", &self.id)?;
        record.serialize_field("title", &self.title)?;
        record.serialize_field("schemas", &self.schemas)?;
        record.serialize_field("entities", &self.entities)?;
        record.end()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaV1 {
    pub(crate) id: SchemaIdV1,
    pub(crate) fields: BTreeMap<FieldIdV1, FieldDefinitionV1>,
}

impl SchemaV1 {
    fn validate(
        &self,
        map_key: &SchemaIdV1,
        schemas: &BTreeMap<SchemaIdV1, SchemaV1>,
    ) -> Result<(), CodecError> {
        validate_id("schema id", &self.id.0)?;
        if map_key != &self.id {
            return invalid(format!(
                "schema map key '{}' does not match nested id '{}'",
                map_key.0, self.id.0
            ));
        }

        for (field_id, definition) in &self.fields {
            validate_id("schema field id", &field_id.0)?;
            definition.validate(schemas)?;
        }
        Ok(())
    }

    fn into_semantic(self) -> Schema {
        Schema {
            id: SchemaId::from(self.id.0),
            fields: self
                .fields
                .into_iter()
                .map(|(id, definition)| (FieldId::from(id.0), definition.into_semantic()))
                .collect(),
        }
    }

    fn from_semantic(schema: &Schema) -> Self {
        Self {
            id: SchemaIdV1(schema.id.as_str().to_owned()),
            fields: schema
                .fields
                .iter()
                .map(|(id, definition)| {
                    (
                        FieldIdV1(id.as_str().to_owned()),
                        FieldDefinitionV1::from_semantic(definition),
                    )
                })
                .collect(),
        }
    }
}

impl Serialize for SchemaV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("SchemaV1", 2)?;
        record.serialize_field("id", &self.id)?;
        record.serialize_field("fields", &self.fields)?;
        record.end()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldDefinitionV1 {
    pub(crate) field_type: FieldTypeV1,
    pub(crate) required: bool,
}

impl FieldDefinitionV1 {
    fn validate(&self, schemas: &BTreeMap<SchemaIdV1, SchemaV1>) -> Result<(), CodecError> {
        self.field_type.validate(schemas)
    }

    fn into_semantic(self) -> FieldDefinition {
        FieldDefinition {
            field_type: self.field_type.into_semantic(),
            required: self.required,
        }
    }

    fn from_semantic(definition: &FieldDefinition) -> Self {
        Self {
            field_type: FieldTypeV1::from_semantic(&definition.field_type),
            required: definition.required,
        }
    }
}

impl Serialize for FieldDefinitionV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("FieldDefinitionV1", 2)?;
        record.serialize_field("field_type", &self.field_type)?;
        record.serialize_field("required", &self.required)?;
        record.end()
    }
}

#[derive(Debug)]
pub(crate) enum FieldTypeV1 {
    Number,
    Text,
    Boolean,
    Reference { schema: SchemaIdV1 },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldTypeV1Wire {
    #[serde(rename = "type")]
    tag: String,
    #[serde(default)]
    schema: Presence<SchemaIdV1>,
}

#[derive(Default)]
enum Presence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> Deserialize<'de> for Presence<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PresenceVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for PresenceVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Presence<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("null or a present JSON value")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Presence::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                T::deserialize(deserializer).map(Presence::Value)
            }
        }

        deserializer.deserialize_option(PresenceVisitor(std::marker::PhantomData))
    }
}

impl<'de> Deserialize<'de> for FieldTypeV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FieldTypeV1Wire::deserialize(deserializer)?;
        match (wire.tag.as_str(), wire.schema) {
            ("number", Presence::Missing) => Ok(Self::Number),
            ("text", Presence::Missing) => Ok(Self::Text),
            ("boolean", Presence::Missing) => Ok(Self::Boolean),
            ("reference", Presence::Value(schema)) => Ok(Self::Reference { schema }),
            ("reference", Presence::Missing | Presence::Null) => Err(D::Error::custom(
                "v1 reference field type requires member 'schema'",
            )),
            ("number" | "text" | "boolean", Presence::Null | Presence::Value(_)) => {
                Err(D::Error::custom(format!(
                    "v1 field type '{}' does not allow member 'schema'",
                    wire.tag
                )))
            }
            (tag, _) => Err(D::Error::custom(format!("unknown v1 field type '{tag}'"))),
        }
    }
}

impl FieldTypeV1 {
    fn validate(&self, schemas: &BTreeMap<SchemaIdV1, SchemaV1>) -> Result<(), CodecError> {
        if let Self::Reference { schema } = self {
            validate_id("reference field target schema", &schema.0)?;
            if !schemas.contains_key(schema) {
                return invalid(format!(
                    "reference field target schema '{}' does not exist",
                    schema.0
                ));
            }
        }
        Ok(())
    }

    fn into_semantic(self) -> FieldType {
        match self {
            Self::Number => FieldType::Number,
            Self::Text => FieldType::Text,
            Self::Boolean => FieldType::Boolean,
            Self::Reference { schema } => FieldType::Reference {
                schema: SchemaId::from(schema.0),
            },
        }
    }

    fn from_semantic(field_type: &FieldType) -> Self {
        match field_type {
            FieldType::Number => Self::Number,
            FieldType::Text => Self::Text,
            FieldType::Boolean => Self::Boolean,
            FieldType::Reference { schema } => Self::Reference {
                schema: SchemaIdV1(schema.as_str().to_owned()),
            },
        }
    }
}

impl Serialize for FieldTypeV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Number => {
                let mut record = serializer.serialize_struct("FieldTypeV1", 1)?;
                record.serialize_field("type", "number")?;
                record.end()
            }
            Self::Text => {
                let mut record = serializer.serialize_struct("FieldTypeV1", 1)?;
                record.serialize_field("type", "text")?;
                record.end()
            }
            Self::Boolean => {
                let mut record = serializer.serialize_struct("FieldTypeV1", 1)?;
                record.serialize_field("type", "boolean")?;
                record.end()
            }
            Self::Reference { schema } => {
                let mut record = serializer.serialize_struct("FieldTypeV1", 2)?;
                record.serialize_field("type", "reference")?;
                record.serialize_field("schema", schema)?;
                record.end()
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntityV1 {
    pub(crate) id: EntityIdV1,
    pub(crate) schema: SchemaIdV1,
    pub(crate) fields: BTreeMap<FieldIdV1, ValueV1>,
}

impl EntityV1 {
    fn validate(
        &self,
        map_key: &EntityIdV1,
        schemas: &BTreeMap<SchemaIdV1, SchemaV1>,
    ) -> Result<(), CodecError> {
        validate_id("entity id", &self.id.0)?;
        if map_key != &self.id {
            return invalid(format!(
                "entity map key '{}' does not match nested id '{}'",
                map_key.0, self.id.0
            ));
        }
        validate_id("entity schema", &self.schema.0)?;
        let Some(schema) = schemas.get(&self.schema) else {
            return invalid(format!("entity schema '{}' does not exist", self.schema.0));
        };

        for (field_id, value) in &self.fields {
            validate_id("entity field id", &field_id.0)?;
            if !schema.fields.contains_key(field_id) {
                return invalid(format!(
                    "entity field '{}' is not declared by schema '{}'",
                    field_id.0, self.schema.0
                ));
            }
            value.validate()?;
        }

        Ok(())
    }

    fn into_semantic(self) -> Entity {
        Entity {
            id: EntityId::from(self.id.0),
            schema: SchemaId::from(self.schema.0),
            fields: self
                .fields
                .into_iter()
                .map(|(id, value)| (FieldId::from(id.0), value.into_semantic()))
                .collect(),
        }
    }

    fn from_semantic(entity: &Entity) -> Self {
        Self {
            id: EntityIdV1(entity.id.as_str().to_owned()),
            schema: SchemaIdV1(entity.schema.as_str().to_owned()),
            fields: entity
                .fields
                .iter()
                .map(|(id, value)| {
                    (
                        FieldIdV1(id.as_str().to_owned()),
                        ValueV1::from_semantic(value),
                    )
                })
                .collect(),
        }
    }
}

impl Serialize for EntityV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("EntityV1", 3)?;
        record.serialize_field("id", &self.id)?;
        record.serialize_field("schema", &self.schema)?;
        record.serialize_field("fields", &self.fields)?;
        record.end()
    }
}

#[derive(Debug)]
pub(crate) enum ValueV1 {
    Number(f64),
    Text(String),
    Boolean(bool),
    Reference(EntityIdV1),
    Formula(ExpressionV1),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueV1Wire {
    kind: String,
    value: serde_json::Value,
}

impl<'de> Deserialize<'de> for ValueV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ValueV1Wire::deserialize(deserializer)?;
        match wire.kind.as_str() {
            "number" => serde_json::from_value(wire.value)
                .map(Self::Number)
                .map_err(D::Error::custom),
            "text" => serde_json::from_value(wire.value)
                .map(Self::Text)
                .map_err(D::Error::custom),
            "boolean" => serde_json::from_value(wire.value)
                .map(Self::Boolean)
                .map_err(D::Error::custom),
            "reference" => serde_json::from_value::<String>(wire.value)
                .map(|entity| Self::Reference(EntityIdV1(entity)))
                .map_err(D::Error::custom),
            "formula" => serde_json::from_value(wire.value)
                .map(Self::Formula)
                .map_err(D::Error::custom),
            kind => Err(D::Error::custom(format!("unknown v1 value kind '{kind}'"))),
        }
    }
}

impl ValueV1 {
    fn validate(&self) -> Result<(), CodecError> {
        match self {
            Self::Number(number) => validate_finite("number value", *number),
            Self::Text(_) | Self::Boolean(_) => Ok(()),
            Self::Reference(entity) => validate_id("entity reference", &entity.0),
            Self::Formula(expression) => expression.validate(),
        }
    }

    fn into_semantic(self) -> Value {
        match self {
            Self::Number(number) => Value::Number(number),
            Self::Text(text) => Value::Text(text),
            Self::Boolean(boolean) => Value::Boolean(boolean),
            Self::Reference(entity) => Value::Reference(EntityId::from(entity.0)),
            Self::Formula(expression) => Value::Formula(expression.into_semantic()),
        }
    }

    fn from_semantic(value: &Value) -> Self {
        match value {
            Value::Number(number) => Self::Number(*number),
            Value::Text(text) => Self::Text(text.clone()),
            Value::Boolean(boolean) => Self::Boolean(*boolean),
            Value::Reference(entity) => Self::Reference(EntityIdV1(entity.as_str().to_owned())),
            Value::Formula(expression) => Self::Formula(ExpressionV1::from_semantic(expression)),
        }
    }
}

impl Serialize for ValueV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("ValueV1", 2)?;
        match self {
            Self::Number(number) => {
                record.serialize_field("kind", "number")?;
                record.serialize_field("value", number)?;
            }
            Self::Text(text) => {
                record.serialize_field("kind", "text")?;
                record.serialize_field("value", text)?;
            }
            Self::Boolean(boolean) => {
                record.serialize_field("kind", "boolean")?;
                record.serialize_field("value", boolean)?;
            }
            Self::Reference(entity) => {
                record.serialize_field("kind", "reference")?;
                record.serialize_field("value", entity)?;
            }
            Self::Formula(expression) => {
                record.serialize_field("kind", "formula")?;
                record.serialize_field("value", expression)?;
            }
        }
        record.end()
    }
}

#[derive(Debug)]
pub(crate) enum ExpressionV1 {
    Number(f64),
    Reference(FieldRefV1),
    Add(BinaryArgsV1),
    Subtract(BinaryArgsV1),
    Multiply(BinaryArgsV1),
    Divide(BinaryArgsV1),
    Minimum(BinaryArgsV1),
    Maximum(BinaryArgsV1),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpressionV1Wire {
    op: String,
    args: serde_json::Value,
}

impl<'de> Deserialize<'de> for ExpressionV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ExpressionV1Wire::deserialize(deserializer)?;
        match wire.op.as_str() {
            "number" => serde_json::from_value(wire.args)
                .map(Self::Number)
                .map_err(D::Error::custom),
            "reference" => serde_json::from_value(wire.args)
                .map(Self::Reference)
                .map_err(D::Error::custom),
            "add" => serde_json::from_value(wire.args)
                .map(Self::Add)
                .map_err(D::Error::custom),
            "subtract" => serde_json::from_value(wire.args)
                .map(Self::Subtract)
                .map_err(D::Error::custom),
            "multiply" => serde_json::from_value(wire.args)
                .map(Self::Multiply)
                .map_err(D::Error::custom),
            "divide" => serde_json::from_value(wire.args)
                .map(Self::Divide)
                .map_err(D::Error::custom),
            "minimum" => serde_json::from_value(wire.args)
                .map(Self::Minimum)
                .map_err(D::Error::custom),
            "maximum" => serde_json::from_value(wire.args)
                .map(Self::Maximum)
                .map_err(D::Error::custom),
            op => Err(D::Error::custom(format!("unknown v1 expression op '{op}'"))),
        }
    }
}

impl ExpressionV1 {
    fn validate(&self) -> Result<(), CodecError> {
        match self {
            Self::Number(number) => validate_finite("formula number", *number),
            Self::Reference(reference) => reference.validate(),
            Self::Add(args)
            | Self::Subtract(args)
            | Self::Multiply(args)
            | Self::Divide(args)
            | Self::Minimum(args)
            | Self::Maximum(args) => args.validate(),
        }
    }

    fn into_semantic(self) -> Expression {
        match self {
            Self::Number(number) => Expression::Number(number),
            Self::Reference(reference) => Expression::Reference(reference.into_semantic()),
            Self::Add(args) => Expression::Add {
                left: Box::new(args.left.into_semantic()),
                right: Box::new(args.right.into_semantic()),
            },
            Self::Subtract(args) => Expression::Subtract {
                left: Box::new(args.left.into_semantic()),
                right: Box::new(args.right.into_semantic()),
            },
            Self::Multiply(args) => Expression::Multiply {
                left: Box::new(args.left.into_semantic()),
                right: Box::new(args.right.into_semantic()),
            },
            Self::Divide(args) => Expression::Divide {
                left: Box::new(args.left.into_semantic()),
                right: Box::new(args.right.into_semantic()),
            },
            Self::Minimum(args) => Expression::Minimum {
                left: Box::new(args.left.into_semantic()),
                right: Box::new(args.right.into_semantic()),
            },
            Self::Maximum(args) => Expression::Maximum {
                left: Box::new(args.left.into_semantic()),
                right: Box::new(args.right.into_semantic()),
            },
        }
    }

    fn from_semantic(expression: &Expression) -> Self {
        match expression {
            Expression::Number(number) => Self::Number(*number),
            Expression::Reference(reference) => {
                Self::Reference(FieldRefV1::from_semantic(reference))
            }
            Expression::Add { left, right } => Self::Add(BinaryArgsV1::from_semantic(left, right)),
            Expression::Subtract { left, right } => {
                Self::Subtract(BinaryArgsV1::from_semantic(left, right))
            }
            Expression::Multiply { left, right } => {
                Self::Multiply(BinaryArgsV1::from_semantic(left, right))
            }
            Expression::Divide { left, right } => {
                Self::Divide(BinaryArgsV1::from_semantic(left, right))
            }
            Expression::Minimum { left, right } => {
                Self::Minimum(BinaryArgsV1::from_semantic(left, right))
            }
            Expression::Maximum { left, right } => {
                Self::Maximum(BinaryArgsV1::from_semantic(left, right))
            }
        }
    }
}

impl Serialize for ExpressionV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("ExpressionV1", 2)?;
        match self {
            Self::Number(number) => {
                record.serialize_field("op", "number")?;
                record.serialize_field("args", number)?;
            }
            Self::Reference(reference) => {
                record.serialize_field("op", "reference")?;
                record.serialize_field("args", reference)?;
            }
            Self::Add(args) => serialize_binary_expression(&mut record, "add", args)?,
            Self::Subtract(args) => serialize_binary_expression(&mut record, "subtract", args)?,
            Self::Multiply(args) => serialize_binary_expression(&mut record, "multiply", args)?,
            Self::Divide(args) => serialize_binary_expression(&mut record, "divide", args)?,
            Self::Minimum(args) => serialize_binary_expression(&mut record, "minimum", args)?,
            Self::Maximum(args) => serialize_binary_expression(&mut record, "maximum", args)?,
        }
        record.end()
    }
}

fn serialize_binary_expression<S>(
    record: &mut S,
    operator: &'static str,
    arguments: &BinaryArgsV1,
) -> Result<(), S::Error>
where
    S: SerializeStruct,
{
    record.serialize_field("op", operator)?;
    record.serialize_field("args", arguments)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldRefV1 {
    pub(crate) entity: EntityIdV1,
    pub(crate) field: FieldIdV1,
}

impl FieldRefV1 {
    fn validate(&self) -> Result<(), CodecError> {
        validate_id("formula reference entity", &self.entity.0)?;
        validate_id("formula reference field", &self.field.0)
    }

    fn into_semantic(self) -> FieldRef {
        FieldRef {
            entity: EntityId::from(self.entity.0),
            field: FieldId::from(self.field.0),
        }
    }

    fn from_semantic(reference: &FieldRef) -> Self {
        Self {
            entity: EntityIdV1(reference.entity.as_str().to_owned()),
            field: FieldIdV1(reference.field.as_str().to_owned()),
        }
    }
}

impl Serialize for FieldRefV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("FieldRefV1", 2)?;
        record.serialize_field("entity", &self.entity)?;
        record.serialize_field("field", &self.field)?;
        record.end()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BinaryArgsV1 {
    pub(crate) left: Box<ExpressionV1>,
    pub(crate) right: Box<ExpressionV1>,
}

impl BinaryArgsV1 {
    fn validate(&self) -> Result<(), CodecError> {
        self.left.validate()?;
        self.right.validate()
    }

    fn from_semantic(left: &Expression, right: &Expression) -> Self {
        Self {
            left: Box::new(ExpressionV1::from_semantic(left)),
            right: Box::new(ExpressionV1::from_semantic(right)),
        }
    }
}

impl Serialize for BinaryArgsV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("BinaryArgsV1", 2)?;
        record.serialize_field("left", &self.left)?;
        record.serialize_field("right", &self.right)?;
        record.end()
    }
}

fn validate_id(kind: &str, identifier: &str) -> Result<(), CodecError> {
    if is_legacy_id(identifier) {
        Ok(())
    } else {
        invalid(format!(
            "{kind} '{identifier}' must use the legacy identifier grammar"
        ))
    }
}

fn is_legacy_id(identifier: &str) -> bool {
    let mut bytes = identifier.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };

    matches!(first, b'a'..=b'z' | b'0'..=b'9')
        && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

fn validate_finite(kind: &str, number: f64) -> Result<(), CodecError> {
    if number.is_finite() {
        Ok(())
    } else {
        invalid(format!("{kind} must be finite"))
    }
}

fn invalid<T>(message: String) -> Result<T, CodecError> {
    Err(CodecError::InvalidRepresentation(message))
}

#[cfg(test)]
mod tests {
    use super::{
        EntityIdV1, ExpressionV1, FieldIdV1, FieldTypeV1, SchemaIdV1, ValueV1, decode_dto,
    };

    #[test]
    fn dto_seam_exposes_every_typed_id_occurrence_before_semantic_conversion() {
        let source = r#"{
  "format_version": 1,
  "id": "legacy-doc",
  "title": "Inventory",
  "schemas": {
    "source": {
      "id": "source",
      "fields": {
        "calc": {"field_type": {"type": "number"}, "required": true},
        "link": {"field_type": {"type": "reference", "schema": "target"}, "required": true}
      }
    },
    "target": {
      "id": "target",
      "fields": {
        "number": {"field_type": {"type": "number"}, "required": true}
      }
    }
  },
  "entities": {
    "source-entity": {
      "id": "source-entity",
      "schema": "source",
      "fields": {
        "calc": {"kind": "formula", "value": {"op": "reference", "args": {"entity": "target-entity", "field": "number"}}},
        "link": {"kind": "reference", "value": "target-entity"}
      }
    },
    "target-entity": {
      "id": "target-entity",
      "schema": "target",
      "fields": {"number": {"kind": "number", "value": 1.0}}
    }
  }
}"#;

        let dto = decode_dto(source).unwrap();
        assert_eq!(dto.id.0, "legacy-doc");

        let source_schema = dto.schemas.get(&SchemaIdV1("source".to_owned())).unwrap();
        assert_eq!(source_schema.id.0, "source");
        let link_definition = source_schema
            .fields
            .get(&FieldIdV1("link".to_owned()))
            .unwrap();
        assert!(matches!(
            &link_definition.field_type,
            FieldTypeV1::Reference { schema } if schema.0 == "target"
        ));

        let source_entity = dto
            .entities
            .get(&EntityIdV1("source-entity".to_owned()))
            .unwrap();
        assert_eq!(source_entity.id.0, "source-entity");
        assert_eq!(source_entity.schema.0, "source");
        assert!(matches!(
            source_entity.fields.get(&FieldIdV1("link".to_owned())),
            Some(ValueV1::Reference(entity)) if entity.0 == "target-entity"
        ));
        assert!(matches!(
            source_entity.fields.get(&FieldIdV1("calc".to_owned())),
            Some(ValueV1::Formula(ExpressionV1::Reference(reference)))
                if reference.entity.0 == "target-entity" && reference.field.0 == "number"
        ));
    }
}
