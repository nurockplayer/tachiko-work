//! Storage-owned DTO and canonical codec for direct `.ro` JSON v2.

use std::collections::BTreeMap;

use serde::Deserialize;
use tachiko_semantic_core::{
    ColumnAggregateFunction, ColumnRef, ComparisonOperator, Date, Document, DocumentId, Entity,
    EntityId, EntityKey, Expression, FieldDefinition, FieldId, FieldKey, FieldRef, FieldType,
    MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, Number, Schema, SchemaId, SchemaKey, Value,
};

pub(crate) const FORMAT_VERSION: u32 = 2;

#[derive(Debug)]
pub(crate) enum CodecError {
    Json(serde_json::Error),
    InvalidRepresentation(String),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentV2 {
    pub(crate) format_version: u32,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) schemas: BTreeMap<String, SchemaV2>,
    pub(crate) entities: BTreeMap<String, EntityV2>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaV2 {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) fields: BTreeMap<String, FieldDefinitionV2>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldDefinitionV2 {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) field_type: FieldTypeV2,
    pub(crate) required: bool,
    #[serde(default, deserialize_with = "crate::number_format::optional_v2")]
    pub(crate) number_format: Option<crate::number_format::NumberFormatDto>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum FieldTypeV2 {
    Number,
    Text,
    Boolean,
    Date,
    Reference { schema: String },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntityV2 {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) schema: String,
    pub(crate) fields: BTreeMap<String, ValueV2>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum ValueV2 {
    Number(f64),
    Text(String),
    Boolean(bool),
    Date(Date),
    Reference(String),
    Formula(ExpressionV2),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "op",
    content = "args",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum ExpressionV2 {
    Number(f64),
    Reference(FieldRefV2),
    SameRowReference(String),
    ColumnTotal(ColumnRefV2),
    ColumnAggregate(ColumnAggregateArgsV2),
    Add(BinaryArgsV2),
    Subtract(BinaryArgsV2),
    Multiply(BinaryArgsV2),
    Divide(BinaryArgsV2),
    Minimum(BinaryArgsV2),
    Maximum(BinaryArgsV2),
    Compare(ComparisonArgsV2),
    If(IfArgsV2),
    And(Vec<ExpressionV2>),
    Or(Vec<ExpressionV2>),
    Not(Box<ExpressionV2>),
    Round(RoundArgsV2),
    IsBlank(Box<ExpressionV2>),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ColumnAggregateArgsV2 {
    function: ColumnAggregateFunction,
    column: ColumnRefV2,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ColumnRefV2 {
    pub(crate) schema: String,
    pub(crate) field: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldRefV2 {
    pub(crate) entity: String,
    pub(crate) field: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BinaryArgsV2 {
    pub(crate) left: Box<ExpressionV2>,
    pub(crate) right: Box<ExpressionV2>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComparisonArgsV2 {
    pub(crate) operator: String,
    pub(crate) left: Box<ExpressionV2>,
    pub(crate) right: Box<ExpressionV2>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IfArgsV2 {
    pub(crate) condition: Box<ExpressionV2>,
    pub(crate) then_value: Box<ExpressionV2>,
    pub(crate) else_value: Box<ExpressionV2>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RoundArgsV2 {
    pub(crate) value: Box<ExpressionV2>,
    #[serde(deserialize_with = "deserialize_round_digits")]
    pub(crate) digits: u8,
}

fn deserialize_round_digits<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    // The strict JSON frontend normalizes numeric tokens to binary64, including
    // integer-valued representation parameters such as ROUND precision.
    let digits = f64::deserialize(deserializer)?;
    if !(0.0..=10.0).contains(&digits) || digits.fract() != 0.0 {
        return Err(serde::de::Error::custom(
            "ROUND digits must be an integer in 0..=10",
        ));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // The representation value is a checked integer in 0..=10.
    Ok(digits as u8)
}

impl DocumentV2 {
    pub(crate) fn from_semantic(document: &Document) -> Result<Self, CodecError> {
        if !document.keyed_grouped_sum_definitions.is_empty() {
            return invalid(
                "direct .ro/v2 cannot encode saved keyed grouped-sum definitions; use .roproj/v2"
                    .to_owned(),
            );
        }
        validate_semantic_expressions(document)?;
        Ok(Self {
            format_version: FORMAT_VERSION,
            id: document.id.to_string(),
            title: document.title.clone(),
            schemas: document
                .schemas
                .iter()
                .map(|(id, schema)| (id.to_string(), SchemaV2::from_semantic(schema)))
                .collect(),
            entities: document
                .entities
                .iter()
                .map(|(id, entity)| (id.to_string(), EntityV2::from_semantic(entity)))
                .collect(),
        })
    }

    pub(crate) fn validate(&self) -> Result<(), CodecError> {
        if self.format_version != FORMAT_VERSION {
            return invalid(format!(
                "format_version must be {FORMAT_VERSION}, found {}",
                self.format_version
            ));
        }
        validate_stable_id("document id", &self.id)?;
        for (schema_id, schema) in &self.schemas {
            validate_stable_id("schema store key", schema_id)?;
            schema.validate(schema_id, &self.schemas)?;
        }
        for (entity_id, entity) in &self.entities {
            validate_stable_id("entity store key", entity_id)?;
            entity.validate(entity_id, &self.schemas, &self.entities)?;
        }
        Ok(())
    }

    pub(crate) fn into_semantic(self) -> Result<Document, CodecError> {
        self.validate()?;
        Ok(Document {
            id: DocumentId::from(self.id),
            title: self.title,
            schemas: self
                .schemas
                .into_iter()
                .map(|(id, schema)| (SchemaId::from(id), schema.into_semantic()))
                .collect(),
            entities: self
                .entities
                .into_iter()
                .map(|(id, entity)| Ok((EntityId::from(id), entity.into_semantic()?)))
                .collect::<Result<_, CodecError>>()?,
            keyed_grouped_sum_definitions: BTreeMap::new(),
        })
    }
}

impl SchemaV2 {
    fn from_semantic(schema: &Schema) -> Self {
        Self {
            id: schema.id.to_string(),
            key: schema.key.to_string(),
            fields: schema
                .fields
                .iter()
                .map(|(id, definition)| {
                    (id.to_string(), FieldDefinitionV2::from_semantic(definition))
                })
                .collect(),
        }
    }

    fn validate(
        &self,
        map_id: &str,
        schemas: &BTreeMap<String, SchemaV2>,
    ) -> Result<(), CodecError> {
        validate_stable_id("schema id", &self.id)?;
        if map_id != self.id {
            return invalid(format!(
                "schema store key '{map_id}' does not match nested id '{}'",
                self.id
            ));
        }
        for (field_id, field) in &self.fields {
            validate_stable_id("field store key", field_id)?;
            field.validate(field_id, schemas)?;
        }
        Ok(())
    }

    fn into_semantic(self) -> Schema {
        Schema {
            field_order: None,
            id: SchemaId::from(self.id),
            key: SchemaKey::from(self.key),
            display_name: None,
            fields: self
                .fields
                .into_iter()
                .map(|(id, field)| (FieldId::from(id), field.into_semantic()))
                .collect(),
        }
    }
}

impl FieldDefinitionV2 {
    fn from_semantic(definition: &FieldDefinition) -> Self {
        Self {
            id: definition.id.to_string(),
            key: definition.key.to_string(),
            field_type: FieldTypeV2::from_semantic(&definition.field_type),
            required: definition.required,
            number_format: definition.number_format.as_ref().map(Into::into),
        }
    }

    fn validate(
        &self,
        map_id: &str,
        schemas: &BTreeMap<String, SchemaV2>,
    ) -> Result<(), CodecError> {
        validate_stable_id("field id", &self.id)?;
        if map_id != self.id {
            return invalid(format!(
                "field store key '{map_id}' does not match nested id '{}'",
                self.id
            ));
        }
        if let FieldTypeV2::Reference { schema } = &self.field_type {
            if !schemas.contains_key(schema) {
                return invalid(format!("reference target schema '{schema}' does not exist"));
            }
        }
        Ok(())
    }

    fn into_semantic(self) -> FieldDefinition {
        FieldDefinition {
            id: FieldId::from(self.id),
            key: FieldKey::from(self.key),
            display_name: None,
            number_format: self.number_format.map(Into::into),
            field_type: self.field_type.into_semantic(),
            required: self.required,
            constraint: tachiko_semantic_core::FieldConstraint::None,
        }
    }
}

impl FieldTypeV2 {
    fn from_semantic(field_type: &FieldType) -> Self {
        match field_type {
            FieldType::Number => Self::Number,
            FieldType::Text => Self::Text,
            FieldType::Boolean => Self::Boolean,
            FieldType::Date => Self::Date,
            FieldType::Reference { schema } => Self::Reference {
                schema: schema.to_string(),
            },
        }
    }

    fn into_semantic(self) -> FieldType {
        match self {
            Self::Number => FieldType::Number,
            Self::Text => FieldType::Text,
            Self::Boolean => FieldType::Boolean,
            Self::Date => FieldType::Date,
            Self::Reference { schema } => FieldType::Reference {
                schema: SchemaId::from(schema),
            },
        }
    }
}

impl EntityV2 {
    fn from_semantic(entity: &Entity) -> Self {
        Self {
            id: entity.id.to_string(),
            key: entity.key.to_string(),
            schema: entity.schema.to_string(),
            fields: entity
                .fields
                .iter()
                .map(|(id, value)| (id.to_string(), ValueV2::from_semantic(value)))
                .collect(),
        }
    }

    fn validate(
        &self,
        map_id: &str,
        schemas: &BTreeMap<String, SchemaV2>,
        entities: &BTreeMap<String, EntityV2>,
    ) -> Result<(), CodecError> {
        validate_stable_id("entity id", &self.id)?;
        if map_id != self.id {
            return invalid(format!(
                "entity store key '{map_id}' does not match nested id '{}'",
                self.id
            ));
        }
        let schema = schemas.get(&self.schema).ok_or_else(|| {
            CodecError::InvalidRepresentation(format!(
                "entity '{}' targets missing schema '{}'",
                self.id, self.schema
            ))
        })?;
        for (field_id, value) in &self.fields {
            if !schema.fields.contains_key(field_id) {
                return invalid(format!(
                    "entity '{}' field '{field_id}' is not declared by schema '{}'",
                    self.id, self.schema
                ));
            }
            value.validate(schemas, entities)?;
        }
        Ok(())
    }

    fn into_semantic(self) -> Result<Entity, CodecError> {
        Ok(Entity {
            id: EntityId::from(self.id),
            key: EntityKey::from(self.key),
            schema: SchemaId::from(self.schema),
            fields: self
                .fields
                .into_iter()
                .map(|(id, value)| Ok((FieldId::from(id), value.into_semantic()?)))
                .collect::<Result<_, CodecError>>()?,
        })
    }
}

impl ValueV2 {
    fn from_semantic(value: &Value) -> Self {
        match value {
            Value::Number(number) => Self::Number(number.get()),
            Value::Text(text) => Self::Text(text.clone()),
            Value::Boolean(boolean) => Self::Boolean(*boolean),
            Value::Date(date) => Self::Date(*date),
            Value::Reference(entity) => Self::Reference(entity.to_string()),
            Value::Formula(expression) => Self::Formula(ExpressionV2::from_semantic(expression)),
        }
    }

    fn validate(
        &self,
        schemas: &BTreeMap<String, SchemaV2>,
        entities: &BTreeMap<String, EntityV2>,
    ) -> Result<(), CodecError> {
        match self {
            Self::Number(number) => validate_number(*number),
            Self::Text(_) | Self::Boolean(_) | Self::Date(_) => Ok(()),
            Self::Reference(entity) => {
                if entities.contains_key(entity) {
                    Ok(())
                } else {
                    invalid(format!("entity reference target '{entity}' does not exist"))
                }
            }
            Self::Formula(expression) => expression.validate(schemas, entities),
        }
    }

    fn into_semantic(self) -> Result<Value, CodecError> {
        Ok(match self {
            Self::Number(number) => Value::Number(number_value(number)?),
            Self::Text(text) => Value::Text(text),
            Self::Boolean(boolean) => Value::Boolean(boolean),
            Self::Date(date) => Value::Date(date),
            Self::Reference(entity) => Value::Reference(EntityId::from(entity)),
            Self::Formula(expression) => Value::Formula(expression.into_semantic()?),
        })
    }
}

impl ExpressionV2 {
    fn from_semantic(expression: &Expression) -> Self {
        match expression {
            Expression::Number(number) => Self::Number(number.get()),
            Expression::Reference(reference) => Self::Reference(FieldRefV2 {
                entity: reference.entity.to_string(),
                field: reference.field.to_string(),
            }),
            Expression::SameRowReference(field) => Self::SameRowReference(field.to_string()),
            Expression::ColumnTotal(column) => Self::ColumnTotal(ColumnRefV2 {
                schema: column.schema.to_string(),
                field: column.field.to_string(),
            }),
            Expression::ColumnAggregate { function, column } => {
                Self::ColumnAggregate(ColumnAggregateArgsV2 {
                    function: *function,
                    column: ColumnRefV2 {
                        schema: column.schema.to_string(),
                        field: column.field.to_string(),
                    },
                })
            }
            Expression::Add { left, right } => Self::Add(BinaryArgsV2::from_semantic(left, right)),
            Expression::Subtract { left, right } => {
                Self::Subtract(BinaryArgsV2::from_semantic(left, right))
            }
            Expression::Multiply { left, right } => {
                Self::Multiply(BinaryArgsV2::from_semantic(left, right))
            }
            Expression::Divide { left, right } => {
                Self::Divide(BinaryArgsV2::from_semantic(left, right))
            }
            Expression::Minimum { left, right } => {
                Self::Minimum(BinaryArgsV2::from_semantic(left, right))
            }
            Expression::Maximum { left, right } => {
                Self::Maximum(BinaryArgsV2::from_semantic(left, right))
            }
            Expression::Compare {
                operator,
                left,
                right,
            } => Self::Compare(ComparisonArgsV2 {
                operator: operator.as_str().to_owned(),
                left: Box::new(Self::from_semantic(left)),
                right: Box::new(Self::from_semantic(right)),
            }),
            Expression::If {
                condition,
                then_value,
                else_value,
            } => Self::If(IfArgsV2 {
                condition: Box::new(Self::from_semantic(condition)),
                then_value: Box::new(Self::from_semantic(then_value)),
                else_value: Box::new(Self::from_semantic(else_value)),
            }),
            Expression::And(expressions) => {
                Self::And(expressions.iter().map(Self::from_semantic).collect())
            }
            Expression::Or(expressions) => {
                Self::Or(expressions.iter().map(Self::from_semantic).collect())
            }
            Expression::Not(expression) => Self::Not(Box::new(Self::from_semantic(expression))),
            Expression::Round { value, digits } => Self::Round(RoundArgsV2 {
                value: Box::new(Self::from_semantic(value)),
                digits: *digits,
            }),
            Expression::IsBlank(expression) => {
                Self::IsBlank(Box::new(Self::from_semantic(expression)))
            }
        }
    }

    fn validate(
        &self,
        schemas: &BTreeMap<String, SchemaV2>,
        entities: &BTreeMap<String, EntityV2>,
    ) -> Result<(), CodecError> {
        let mut nodes = 0_usize;
        let mut stack = vec![(self, 1_usize)];
        while let Some((node, depth)) = stack.pop() {
            if depth > MAX_EXPRESSION_DEPTH {
                return invalid(format!(
                    "formula expression exceeds {MAX_EXPRESSION_DEPTH}-depth limit"
                ));
            }
            nodes += 1;
            if nodes > MAX_EXPRESSION_NODES {
                return invalid(format!(
                    "formula expression exceeds {MAX_EXPRESSION_NODES}-node limit"
                ));
            }
            match node {
                Self::Number(number) => validate_number(*number)?,
                Self::Reference(reference) => reference.validate(schemas, entities)?,
                Self::SameRowReference(field) => {
                    if !schemas
                        .values()
                        .any(|schema| schema.fields.contains_key(field))
                    {
                        return invalid(format!(
                            "same-row formula field '{field}' does not exist in any schema"
                        ));
                    }
                }
                Self::ColumnTotal(column)
                | Self::ColumnAggregate(ColumnAggregateArgsV2 { column, .. }) => {
                    let Some(schema) = schemas.get(&column.schema) else {
                        return invalid(format!(
                            "formula column schema '{}' does not exist",
                            column.schema
                        ));
                    };
                    if !schema.fields.contains_key(&column.field) {
                        return invalid(format!(
                            "formula column field '{}' does not exist in schema '{}'",
                            column.field, column.schema
                        ));
                    }
                }
                Self::Add(args)
                | Self::Subtract(args)
                | Self::Multiply(args)
                | Self::Divide(args)
                | Self::Minimum(args)
                | Self::Maximum(args) => {
                    stack.push((&args.right, depth + 1));
                    stack.push((&args.left, depth + 1));
                }
                Self::Compare(args) => {
                    comparison_operator(&args.operator)?;
                    stack.push((&args.right, depth + 1));
                    stack.push((&args.left, depth + 1));
                }
                Self::If(args) => {
                    stack.push((&args.else_value, depth + 1));
                    stack.push((&args.then_value, depth + 1));
                    stack.push((&args.condition, depth + 1));
                }
                Self::And(expressions) | Self::Or(expressions) => {
                    stack.extend(expressions.iter().map(|expression| (expression, depth + 1)));
                }
                Self::Not(expression) => stack.push((expression, depth + 1)),
                Self::Round(args) => {
                    if args.digits > 10 {
                        return invalid(format!(
                            "ROUND digits must be between 0 and 10, found {}",
                            args.digits
                        ));
                    }
                    stack.push((&args.value, depth + 1));
                }
                Self::IsBlank(expression) => {
                    if !matches!(
                        expression.as_ref(),
                        Self::Reference(_) | Self::SameRowReference(_)
                    ) {
                        return invalid("ISBLANK requires a field reference".to_owned());
                    }
                    stack.push((expression, depth + 1));
                }
            }
        }
        Ok(())
    }

    fn into_semantic(self) -> Result<Expression, CodecError> {
        Ok(match self {
            Self::Number(number) => Expression::Number(number_value(number)?),
            Self::Reference(reference) => Expression::Reference(FieldRef {
                entity: EntityId::from(reference.entity),
                field: FieldId::from(reference.field),
            }),
            Self::SameRowReference(field) => Expression::SameRowReference(FieldId::from(field)),
            Self::ColumnTotal(column) => Expression::ColumnTotal(ColumnRef::new(
                SchemaId::from(column.schema),
                FieldId::from(column.field),
            )),
            Self::ColumnAggregate(args) => Expression::ColumnAggregate {
                function: args.function,
                column: ColumnRef::new(
                    SchemaId::from(args.column.schema),
                    FieldId::from(args.column.field),
                ),
            },
            Self::Add(args) => Expression::Add {
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::Subtract(args) => Expression::Subtract {
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::Multiply(args) => Expression::Multiply {
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::Divide(args) => Expression::Divide {
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::Minimum(args) => Expression::Minimum {
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::Maximum(args) => Expression::Maximum {
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::Compare(args) => Expression::Compare {
                operator: comparison_operator(&args.operator)?,
                left: Box::new(args.left.into_semantic()?),
                right: Box::new(args.right.into_semantic()?),
            },
            Self::If(args) => Expression::If {
                condition: Box::new(args.condition.into_semantic()?),
                then_value: Box::new(args.then_value.into_semantic()?),
                else_value: Box::new(args.else_value.into_semantic()?),
            },
            Self::And(expressions) => Expression::And(
                expressions
                    .into_iter()
                    .map(ExpressionV2::into_semantic)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Or(expressions) => Expression::Or(
                expressions
                    .into_iter()
                    .map(ExpressionV2::into_semantic)
                    .collect::<Result<_, _>>()?,
            ),
            Self::Not(expression) => Expression::Not(Box::new(expression.into_semantic()?)),
            Self::Round(args) => Expression::Round {
                value: Box::new(args.value.into_semantic()?),
                digits: args.digits,
            },
            Self::IsBlank(expression) => Expression::IsBlank(Box::new(expression.into_semantic()?)),
        })
    }
}

fn validate_semantic_expressions(document: &Document) -> Result<(), CodecError> {
    for entity in document.entities.values() {
        for value in entity.fields.values() {
            if let Value::Formula(expression) = value {
                validate_semantic_expression(expression)?;
            }
        }
    }
    Ok(())
}

fn validate_semantic_expression(expression: &Expression) -> Result<(), CodecError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(expression, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return invalid(format!(
                "formula expression exceeds {MAX_EXPRESSION_DEPTH}-depth limit"
            ));
        }
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return invalid(format!(
                "formula expression exceeds {MAX_EXPRESSION_NODES}-node limit"
            ));
        }
        match node {
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right }
            | Expression::Compare { left, right, .. } => {
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
            Expression::If {
                condition,
                then_value,
                else_value,
            } => {
                stack.push((else_value, depth + 1));
                stack.push((then_value, depth + 1));
                stack.push((condition, depth + 1));
            }
            Expression::And(expressions) | Expression::Or(expressions) => {
                stack.extend(expressions.iter().map(|expression| (expression, depth + 1)));
            }
            Expression::Not(expression) => stack.push((expression, depth + 1)),
            Expression::Round { value, digits } => {
                if *digits > 10 {
                    return invalid(format!(
                        "ROUND digits must be between 0 and 10, found {digits}"
                    ));
                }
                stack.push((value, depth + 1));
            }
            Expression::IsBlank(expression) => {
                if !matches!(
                    expression.as_ref(),
                    Expression::Reference(_) | Expression::SameRowReference(_)
                ) {
                    return invalid("ISBLANK requires a field reference".to_owned());
                }
                stack.push((expression, depth + 1));
            }
            Expression::Number(_)
            | Expression::Reference(_)
            | Expression::SameRowReference(_)
            | Expression::ColumnTotal(_)
            | Expression::ColumnAggregate { .. } => {}
        }
    }
    Ok(())
}

fn comparison_operator(operator: &str) -> Result<ComparisonOperator, CodecError> {
    match operator {
        "=" => Ok(ComparisonOperator::Equal),
        "<>" => Ok(ComparisonOperator::NotEqual),
        "<" => Ok(ComparisonOperator::Less),
        "<=" => Ok(ComparisonOperator::LessOrEqual),
        ">" => Ok(ComparisonOperator::Greater),
        ">=" => Ok(ComparisonOperator::GreaterOrEqual),
        _ => invalid(format!("unknown formula comparison operator '{operator}'")),
    }
}

impl FieldRefV2 {
    fn validate(
        &self,
        schemas: &BTreeMap<String, SchemaV2>,
        entities: &BTreeMap<String, EntityV2>,
    ) -> Result<(), CodecError> {
        let entity = entities.get(&self.entity).ok_or_else(|| {
            CodecError::InvalidRepresentation(format!(
                "formula target entity '{}' does not exist",
                self.entity
            ))
        })?;
        let schema = schemas.get(&entity.schema).ok_or_else(|| {
            CodecError::InvalidRepresentation(format!(
                "formula target entity '{}' targets missing schema '{}'",
                self.entity, entity.schema
            ))
        })?;
        if schema.fields.contains_key(&self.field) {
            Ok(())
        } else {
            invalid(format!(
                "formula target field '{}' does not exist in entity '{}' schema",
                self.field, self.entity
            ))
        }
    }
}

impl BinaryArgsV2 {
    fn from_semantic(left: &Expression, right: &Expression) -> Self {
        Self {
            left: Box::new(ExpressionV2::from_semantic(left)),
            right: Box::new(ExpressionV2::from_semantic(right)),
        }
    }
}

pub(crate) fn encode(document: &DocumentV2) -> Result<String, CodecError> {
    document.validate()?;
    let mut output = String::new();
    write_document(&mut output, document, 0)?;
    output.push('\n');
    Ok(output)
}

fn write_document(
    output: &mut String,
    document: &DocumentV2,
    indent: usize,
) -> Result<(), CodecError> {
    output.push_str("{\n");
    member_number(
        output,
        indent + 1,
        "format_version",
        f64::from(FORMAT_VERSION),
        true,
    )?;
    member_string(output, indent + 1, "id", &document.id, true)?;
    member_string(output, indent + 1, "title", &document.title, true)?;
    member_prefix(output, indent + 1, "schemas")?;
    write_map(output, &document.schemas, indent + 1, write_schema)?;
    output.push_str(",\n");
    member_prefix(output, indent + 1, "entities")?;
    write_map(output, &document.entities, indent + 1, write_entity)?;
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_schema(output: &mut String, schema: &SchemaV2, indent: usize) -> Result<(), CodecError> {
    output.push_str("{\n");
    member_string(output, indent + 1, "id", &schema.id, true)?;
    member_string(output, indent + 1, "key", &schema.key, true)?;
    member_prefix(output, indent + 1, "fields")?;
    write_map(output, &schema.fields, indent + 1, write_field_definition)?;
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_field_definition(
    output: &mut String,
    field: &FieldDefinitionV2,
    indent: usize,
) -> Result<(), CodecError> {
    output.push_str("{\n");
    member_string(output, indent + 1, "id", &field.id, true)?;
    member_string(output, indent + 1, "key", &field.key, true)?;
    member_prefix(output, indent + 1, "field_type")?;
    write_field_type(output, &field.field_type, indent + 1)?;
    output.push_str(",\n");
    member_bool(output, indent + 1, "required", field.required, false)?;
    if let Some(format) = &field.number_format {
        output.push_str(",\n");
        member_prefix(output, indent + 1, "number_format")?;
        crate::number_format::write(output, format, indent + 1).map_err(CodecError::Json)?;
    }
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_field_type(
    output: &mut String,
    field_type: &FieldTypeV2,
    indent: usize,
) -> Result<(), CodecError> {
    output.push_str("{\n");
    match field_type {
        FieldTypeV2::Number => member_string(output, indent + 1, "type", "number", false)?,
        FieldTypeV2::Text => member_string(output, indent + 1, "type", "text", false)?,
        FieldTypeV2::Boolean => member_string(output, indent + 1, "type", "boolean", false)?,
        FieldTypeV2::Date => member_string(output, indent + 1, "type", "date", false)?,
        FieldTypeV2::Reference { schema } => {
            member_string(output, indent + 1, "type", "reference", true)?;
            member_string(output, indent + 1, "schema", schema, false)?;
        }
    }
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_entity(output: &mut String, entity: &EntityV2, indent: usize) -> Result<(), CodecError> {
    output.push_str("{\n");
    member_string(output, indent + 1, "id", &entity.id, true)?;
    member_string(output, indent + 1, "key", &entity.key, true)?;
    member_string(output, indent + 1, "schema", &entity.schema, true)?;
    member_prefix(output, indent + 1, "fields")?;
    write_map(output, &entity.fields, indent + 1, write_value)?;
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_value(output: &mut String, value: &ValueV2, indent: usize) -> Result<(), CodecError> {
    output.push_str("{\n");
    match value {
        ValueV2::Number(number) => {
            member_string(output, indent + 1, "kind", "number", true)?;
            member_number(output, indent + 1, "value", *number, false)?;
        }
        ValueV2::Text(text) => {
            member_string(output, indent + 1, "kind", "text", true)?;
            member_string(output, indent + 1, "value", text, false)?;
        }
        ValueV2::Boolean(boolean) => {
            member_string(output, indent + 1, "kind", "boolean", true)?;
            member_bool(output, indent + 1, "value", *boolean, false)?;
        }
        ValueV2::Date(date) => {
            member_string(output, indent + 1, "kind", "date", true)?;
            member_string(output, indent + 1, "value", &date.to_string(), false)?;
        }
        ValueV2::Reference(entity) => {
            member_string(output, indent + 1, "kind", "reference", true)?;
            member_string(output, indent + 1, "value", entity, false)?;
        }
        ValueV2::Formula(expression) => {
            member_string(output, indent + 1, "kind", "formula", true)?;
            member_prefix(output, indent + 1, "value")?;
            write_expression(output, expression, indent + 1)?;
        }
    }
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_expression(
    output: &mut String,
    expression: &ExpressionV2,
    indent: usize,
) -> Result<(), CodecError> {
    output.push_str("{\n");
    match expression {
        ExpressionV2::Number(number) => {
            member_string(output, indent + 1, "op", "number", true)?;
            member_number(output, indent + 1, "args", *number, false)?;
        }
        ExpressionV2::Reference(reference) => {
            member_string(output, indent + 1, "op", "reference", true)?;
            member_prefix(output, indent + 1, "args")?;
            write_field_ref(output, reference, indent + 1)?;
        }
        ExpressionV2::SameRowReference(field) => {
            member_string(output, indent + 1, "op", "same_row_reference", true)?;
            member_string(output, indent + 1, "args", field, false)?;
        }
        ExpressionV2::ColumnTotal(column) => {
            member_string(output, indent + 1, "op", "column_total", true)?;
            member_prefix(output, indent + 1, "args")?;
            write_column_ref(output, column, indent + 1)?;
        }
        ExpressionV2::ColumnAggregate(args) => write_column_aggregate(output, args, indent)?,
        ExpressionV2::Add(args) => write_binary(output, indent, "add", args)?,
        ExpressionV2::Subtract(args) => write_binary(output, indent, "subtract", args)?,
        ExpressionV2::Multiply(args) => write_binary(output, indent, "multiply", args)?,
        ExpressionV2::Divide(args) => write_binary(output, indent, "divide", args)?,
        ExpressionV2::Minimum(args) => write_binary(output, indent, "minimum", args)?,
        ExpressionV2::Maximum(args) => write_binary(output, indent, "maximum", args)?,
        ExpressionV2::Compare(args) => {
            member_string(output, indent + 1, "op", "compare", true)?;
            member_prefix(output, indent + 1, "args")?;
            output.push_str("{\n");
            member_string(output, indent + 2, "operator", &args.operator, true)?;
            write_expression_member(output, indent + 2, "left", &args.left, true)?;
            write_expression_member(output, indent + 2, "right", &args.right, false)?;
            push_indent(output, indent + 1);
            output.push('}');
        }
        ExpressionV2::If(args) => {
            member_string(output, indent + 1, "op", "if", true)?;
            member_prefix(output, indent + 1, "args")?;
            output.push_str("{\n");
            write_expression_member(output, indent + 2, "condition", &args.condition, true)?;
            write_expression_member(output, indent + 2, "then_value", &args.then_value, true)?;
            write_expression_member(output, indent + 2, "else_value", &args.else_value, false)?;
            push_indent(output, indent + 1);
            output.push('}');
        }
        ExpressionV2::And(arguments) | ExpressionV2::Or(arguments) => {
            let op = if matches!(expression, ExpressionV2::And(_)) {
                "and"
            } else {
                "or"
            };
            member_string(output, indent + 1, "op", op, true)?;
            member_prefix(output, indent + 1, "args")?;
            output.push_str("[\n");
            for (index, argument) in arguments.iter().enumerate() {
                push_indent(output, indent + 2);
                write_expression(output, argument, indent + 2)?;
                if index + 1 < arguments.len() {
                    output.push(',');
                }
                output.push('\n');
            }
            push_indent(output, indent + 1);
            output.push(']');
        }
        ExpressionV2::Not(value) | ExpressionV2::IsBlank(value) => {
            let op = if matches!(expression, ExpressionV2::Not(_)) {
                "not"
            } else {
                "is_blank"
            };
            member_string(output, indent + 1, "op", op, true)?;
            member_prefix(output, indent + 1, "args")?;
            write_expression(output, value, indent + 1)?;
        }
        ExpressionV2::Round(args) => {
            member_string(output, indent + 1, "op", "round", true)?;
            member_prefix(output, indent + 1, "args")?;
            output.push_str("{\n");
            write_expression_member(output, indent + 2, "value", &args.value, true)?;
            member_number(output, indent + 2, "digits", f64::from(args.digits), false)?;
            output.push('\n');
            push_indent(output, indent + 1);
            output.push('}');
        }
    }
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_expression_member(
    output: &mut String,
    indent: usize,
    name: &str,
    expression: &ExpressionV2,
    comma: bool,
) -> Result<(), CodecError> {
    member_prefix(output, indent, name)?;
    write_expression(output, expression, indent)?;
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn write_binary(
    output: &mut String,
    indent: usize,
    operator: &str,
    args: &BinaryArgsV2,
) -> Result<(), CodecError> {
    member_string(output, indent + 1, "op", operator, true)?;
    member_prefix(output, indent + 1, "args")?;
    output.push_str("{\n");
    member_prefix(output, indent + 2, "left")?;
    write_expression(output, &args.left, indent + 2)?;
    output.push_str(",\n");
    member_prefix(output, indent + 2, "right")?;
    write_expression(output, &args.right, indent + 2)?;
    output.push('\n');
    push_indent(output, indent + 1);
    output.push('}');
    Ok(())
}

fn write_field_ref(
    output: &mut String,
    reference: &FieldRefV2,
    indent: usize,
) -> Result<(), CodecError> {
    output.push_str("{\n");
    member_string(output, indent + 1, "entity", &reference.entity, true)?;
    member_string(output, indent + 1, "field", &reference.field, false)?;
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_column_aggregate(
    output: &mut String,
    args: &ColumnAggregateArgsV2,
    indent: usize,
) -> Result<(), CodecError> {
    member_string(output, indent + 1, "op", "column_aggregate", true)?;
    member_prefix(output, indent + 1, "args")?;
    output.push_str("{\n");
    member_string(
        output,
        indent + 2,
        "function",
        args.function.wire_name(),
        true,
    )?;
    member_prefix(output, indent + 2, "column")?;
    write_column_ref(output, &args.column, indent + 2)?;
    output.push('\n');
    push_indent(output, indent + 1);
    output.push('}');
    Ok(())
}

fn write_column_ref(
    output: &mut String,
    column: &ColumnRefV2,
    indent: usize,
) -> Result<(), CodecError> {
    output.push_str("{\n");
    member_string(output, indent + 1, "schema", &column.schema, true)?;
    member_string(output, indent + 1, "field", &column.field, false)?;
    output.push('\n');
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn write_map<T>(
    output: &mut String,
    values: &BTreeMap<String, T>,
    indent: usize,
    write: fn(&mut String, &T, usize) -> Result<(), CodecError>,
) -> Result<(), CodecError> {
    if values.is_empty() {
        output.push_str("{}");
        return Ok(());
    }
    output.push_str("{\n");
    for (index, (id, value)) in values.iter().enumerate() {
        push_indent(output, indent + 1);
        write_json_string(output, id)?;
        output.push_str(": ");
        write(output, value, indent + 1)?;
        if index + 1 != values.len() {
            output.push(',');
        }
        output.push('\n');
    }
    push_indent(output, indent);
    output.push('}');
    Ok(())
}

fn member_prefix(output: &mut String, indent: usize, name: &str) -> Result<(), CodecError> {
    push_indent(output, indent);
    write_json_string(output, name)?;
    output.push_str(": ");
    Ok(())
}

fn member_string(
    output: &mut String,
    indent: usize,
    name: &str,
    value: &str,
    comma: bool,
) -> Result<(), CodecError> {
    member_prefix(output, indent, name)?;
    write_json_string(output, value)?;
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn member_number(
    output: &mut String,
    indent: usize,
    name: &str,
    value: f64,
    comma: bool,
) -> Result<(), CodecError> {
    member_prefix(output, indent, name)?;
    let number = Number::new(value)
        .map_err(|_| CodecError::InvalidRepresentation("number must be finite".to_owned()))?;
    let mut buffer = ryu_js::Buffer::new();
    output.push_str(buffer.format_finite(number.get()));
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn member_bool(
    output: &mut String,
    indent: usize,
    name: &str,
    value: bool,
    comma: bool,
) -> Result<(), CodecError> {
    member_prefix(output, indent, name)?;
    output.push_str(if value { "true" } else { "false" });
    if comma {
        output.push(',');
    }
    output.push('\n');
    Ok(())
}

fn write_json_string(output: &mut String, value: &str) -> Result<(), CodecError> {
    output.push_str(&serde_json::to_string(value).map_err(CodecError::Json)?);
    Ok(())
}

fn push_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push_str("  ");
    }
}

fn validate_stable_id(kind: &str, id: &str) -> Result<(), CodecError> {
    if id.is_empty() {
        invalid(format!("{kind} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_number(number: f64) -> Result<(), CodecError> {
    number_value(number).map(|_| ())
}

fn number_value(number: f64) -> Result<Number, CodecError> {
    Number::new(number)
        .map_err(|_| CodecError::InvalidRepresentation("number must be finite".to_owned()))
}

fn invalid<T>(message: String) -> Result<T, CodecError> {
    Err(CodecError::InvalidRepresentation(message))
}
