use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::{KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId};

macro_rules! text_newtype {
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

// Stable IDs are nominal and opaque. Their textual representation is a storage
// and creation-boundary mechanism, not semantic meaning.
text_newtype!(DocumentId);
text_newtype!(SchemaId);
text_newtype!(EntityId);
text_newtype!(FieldId);

// Human keys are mutable authoring addresses and intentionally distinct from
// stable identity even though both currently use textual carriers.
text_newtype!(SchemaKey);
text_newtype!(EntityKey);
text_newtype!(FieldKey);

/// A finite binary64 semantic number with one canonical zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct Number(f64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidNumber;

impl Number {
    /// Construct a semantic Number, rejecting non-finite values and
    /// normalizing either IEEE zero sign to positive zero.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidNumber`] for NaN or positive/negative infinity.
    pub fn new(value: f64) -> Result<Self, InvalidNumber> {
        if !value.is_finite() {
            return Err(InvalidNumber);
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    #[must_use]
    pub fn get(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn to_bits(self) -> u64 {
        self.0.to_bits()
    }
}

impl TryFrom<f64> for Number {
    type Error = InvalidNumber;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Eq for Number {}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

impl fmt::Display for Number {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Display for InvalidNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("semantic numbers must be finite")
    }
}

impl std::error::Error for InvalidNumber {}

impl Serialize for Number {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// A date-only civil value in the proleptic Gregorian calendar.
///
/// The semantic domain is deliberately independent of time zones, time of
/// day, epochs, and locale. Years are limited to the four-digit range used by
/// the canonical `YYYY-MM-DD` representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidDate;

impl Date {
    /// Construct a valid proleptic Gregorian date.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidDate`] when the date is outside the `0001..=9999`
    /// year range or is not a valid Gregorian civil date.
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, InvalidDate> {
        if !(1..=9_999).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
        {
            return Err(InvalidDate);
        }
        Ok(Self { year, month, day })
    }

    /// Alias for [`Date::new`] that makes the civil components explicit at
    /// call sites.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidDate`] when the components do not form a valid
    /// proleptic Gregorian date.
    pub fn from_ymd(year: u16, month: u8, day: u8) -> Result<Self, InvalidDate> {
        Self::new(year, month, day)
    }

    /// Parse the exact canonical `YYYY-MM-DD` spelling.
    ///
    /// Two-digit years, alternate separators, surrounding whitespace, and
    /// non-canonical numeric spellings are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidDate`] when the input is not the exact canonical
    /// spelling of a valid proleptic Gregorian date.
    pub fn parse(input: &str) -> Result<Self, InvalidDate> {
        let bytes = input.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(InvalidDate);
        }
        if !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
        {
            return Err(InvalidDate);
        }
        let year = decimal_component(&bytes[0..4]);
        let month = u8::try_from(decimal_component(&bytes[5..7])).map_err(|_| InvalidDate)?;
        let day = u8::try_from(decimal_component(&bytes[8..10])).map_err(|_| InvalidDate)?;
        Self::new(year, month, day)
    }

    #[must_use]
    pub const fn year(self) -> u16 {
        self.year
    }

    #[must_use]
    pub const fn month(self) -> u8 {
        self.month
    }

    #[must_use]
    pub const fn day(self) -> u8 {
        self.day
    }

    /// Return the canonical `YYYY-MM-DD` spelling.
    #[must_use]
    pub fn to_canonical_string(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl std::str::FromStr for Date {
    type Err = InvalidDate;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

impl TryFrom<&str> for Date {
    type Error = InvalidDate;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        Self::parse(input)
    }
}

impl TryFrom<String> for Date {
    type Error = InvalidDate;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        Self::parse(&input)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

impl fmt::Display for InvalidDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("semantic dates must be valid Gregorian dates in YYYY-MM-DD range")
    }
}

impl std::error::Error for InvalidDate {}

impl Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).map_err(D::Error::custom)
    }
}

const fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0) && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn decimal_component(bytes: &[u8]) -> u16 {
    bytes
        .iter()
        .fold(0_u16, |value, byte| value * 10 + u16::from(byte - b'0'))
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub id: DocumentId,
    pub title: String,
    pub schemas: BTreeMap<SchemaId, Schema>,
    pub entities: BTreeMap<EntityId, Entity>,
    #[serde(default)]
    pub keyed_grouped_sum_definitions:
        BTreeMap<KeyedGroupedSumDefinitionId, KeyedGroupedSumDefinition>,
}

impl Document {
    #[must_use]
    pub fn empty(id: impl Into<DocumentId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            schemas: BTreeMap::new(),
            entities: BTreeMap::new(),
            keyed_grouped_sum_definitions: BTreeMap::new(),
        }
    }
}

// The manual implementation only adds an instrumentation hook. Semantics are
// identical to the derived `Clone`; the hook is an empty inline call unless the
// `capacity-instrumentation` feature is enabled.
impl Clone for Document {
    fn clone(&self) -> Self {
        crate::instrumentation::record_document_clone();
        Self {
            id: self.id.clone(),
            title: self.title.clone(),
            schemas: self.schemas.clone(),
            entities: self.entities.clone(),
            keyed_grouped_sum_definitions: self.keyed_grouped_sum_definitions.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub id: SchemaId,
    pub key: SchemaKey,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "stored_display_name"
    )]
    pub display_name: Option<String>,
    pub fields: BTreeMap<FieldId, FieldDefinition>,
    /// Presentation order of `fields`. `None` means field-id order. `Some` must be an exact
    /// permutation of `fields` and must differ from id order (see `normalize_field_order`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_order: Option<Vec<FieldId>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDefinition {
    pub id: FieldId,
    pub key: FieldKey,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "stored_display_name"
    )]
    pub display_name: Option<String>,
    pub field_type: FieldType,
    pub required: bool,
    pub constraint: FieldConstraint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<NumberFormat>,
}

fn stored_display_name<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

impl Schema {
    #[must_use]
    pub fn effective_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(self.key.as_str())
    }

    /// Fields in presentation order. Ids listed in `field_order` come first in that order;
    /// any field the order does not name follows in id order.
    pub fn ordered_fields(&self) -> impl Iterator<Item = &FieldDefinition> + '_ {
        let listed: &[FieldId] = self.field_order.as_deref().unwrap_or(&[]);
        let fields = &self.fields;
        listed.iter().filter_map(move |id| fields.get(id)).chain(
            fields
                .iter()
                .filter(move |(id, _)| !listed.contains(id))
                .map(|(_, field)| field),
        )
    }

    /// Adds a field after every existing column. When the order is implicit and
    /// the new id sorts before an existing one, the current order is made
    /// explicit first so no existing column moves.
    pub fn insert_field_last(&mut self, field: FieldDefinition) {
        if self.field_order.is_none() && self.fields.keys().any(|id| id > &field.id) {
            self.field_order = Some(self.fields.keys().cloned().collect());
        }
        self.fields.insert(field.id.clone(), field);
        self.normalize_field_order();
    }

    /// Drops ids no longer in `fields` and appends missing ids in id order.
    /// An explicit order equal to id order is kept: `None` is the client's
    /// default presentation, which need not be id order.
    pub fn normalize_field_order(&mut self) {
        let Some(order) = self.field_order.as_mut() else {
            return;
        };
        order.retain(|id| self.fields.contains_key(id));
        for id in self.fields.keys() {
            if !order.contains(id) {
                order.push(id.clone());
            }
        }
    }
}

impl FieldDefinition {
    #[must_use]
    pub fn effective_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(self.key.as_str())
    }
}

impl Document {
    #[must_use]
    pub fn has_number_formats(&self) -> bool {
        self.schemas
            .values()
            .flat_map(|schema| schema.fields.values())
            .any(|field| field.number_format.is_some())
    }

    #[must_use]
    pub fn has_field_orders(&self) -> bool {
        self.schemas
            .values()
            .any(|schema| schema.field_order.is_some())
    }

    #[must_use]
    pub fn has_display_names(&self) -> bool {
        self.schemas.values().any(|schema| {
            schema.display_name.is_some()
                || schema
                    .fields
                    .values()
                    .any(|field| field.display_name.is_some())
        })
    }
}

/// Closed durable constraint declaration owned by a schema field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldConstraint {
    None,
    TextLiteralSet { values: Vec<String> },
    NumberInclusiveRange { min: Number, max: Number },
}

impl<'de> Deserialize<'de> for FieldConstraint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            None(NoneConstraint),
            TextLiteralSet(TextLiteralSetConstraint),
            NumberInclusiveRange(NumberInclusiveRangeConstraint),
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum NoneTag {
            None,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum TextLiteralSetTag {
            TextLiteralSet,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum NumberInclusiveRangeTag {
            NumberInclusiveRange,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct NoneConstraint {
            #[serde(rename = "type")]
            _tag: NoneTag,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct TextLiteralSetConstraint {
            #[serde(rename = "type")]
            _tag: TextLiteralSetTag,
            values: Vec<String>,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct NumberInclusiveRangeConstraint {
            #[serde(rename = "type")]
            _tag: NumberInclusiveRangeTag,
            min: Number,
            max: Number,
        }

        match Wire::deserialize(deserializer)? {
            Wire::None(_) => Ok(Self::None),
            Wire::TextLiteralSet(constraint) => Ok(Self::TextLiteralSet {
                values: constraint.values,
            }),
            Wire::NumberInclusiveRange(constraint) => Ok(Self::NumberInclusiveRange {
                min: constraint.min,
                max: constraint.max,
            }),
        }
    }
}

impl FieldConstraint {
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldType {
    Number,
    Text,
    Boolean,
    Date,
    Reference { schema: SchemaId },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub id: EntityId,
    pub key: EntityKey,
    pub schema: SchemaId,
    pub fields: BTreeMap<FieldId, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Value {
    Number(Number),
    Text(String),
    Boolean(bool),
    Date(Date),
    Reference(EntityId),
    Formula(Expression),
}

/// Maximum number of nodes admitted in a semantic formula expression.
pub const MAX_EXPRESSION_NODES: usize = 256;

/// Maximum root-to-leaf node depth admitted in a semantic formula expression.
pub const MAX_EXPRESSION_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnAggregateFunction {
    Average,
    Count,
    Minimum,
    Maximum,
}

impl ColumnAggregateFunction {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Average => "average",
            Self::Count => "count",
            Self::Minimum => "minimum",
            Self::Maximum => "maximum",
        }
    }

    /// The function named by a `[schema.field:<suffix>]` spelling, any case.
    #[must_use]
    pub fn from_suffix(suffix: &str) -> Option<Self> {
        [Self::Average, Self::Count, Self::Minimum, Self::Maximum]
            .into_iter()
            .find(|function| suffix.eq_ignore_ascii_case(function.suffix()))
    }

    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Average => "average",
            Self::Count => "count",
            Self::Minimum => "min",
            Self::Maximum => "max",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum Expression {
    Number(Number),
    Reference(FieldRef),
    // The formula owner's own field: the entity is resolved from the formula
    // that contains this node, so the node stores only the field identity and
    // keeps meaning "this row" through copies and row-relative moves.
    SameRowReference(FieldId),
    // The sum of one Number column across every entity of a schema. The node
    // names the schema and field, never the rows, so adding, deleting or
    // renaming rows keeps the total meaning the whole column.
    ColumnTotal(ColumnRef),
    ColumnAggregate {
        function: ColumnAggregateFunction,
        column: ColumnRef,
    },
    Add {
        left: Box<Self>,
        right: Box<Self>,
    },
    Subtract {
        left: Box<Self>,
        right: Box<Self>,
    },
    Multiply {
        left: Box<Self>,
        right: Box<Self>,
    },
    Divide {
        left: Box<Self>,
        right: Box<Self>,
    },
    Minimum {
        left: Box<Self>,
        right: Box<Self>,
    },
    Maximum {
        left: Box<Self>,
        right: Box<Self>,
    },
    Compare {
        operator: ComparisonOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
    If {
        condition: Box<Self>,
        then_value: Box<Self>,
        else_value: Box<Self>,
    },
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
    Round {
        value: Box<Self>,
        digits: u8,
    },
    IsBlank(Box<Self>),
}

/// A condition is an intermediate formula type, never a stored scalar value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormulaExpressionType {
    Number,
    Condition,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl ComparisonOperator {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::NotEqual => "<>",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
        }
    }
}

impl Expression {
    /// Direct children in evaluation order, including both conditional branches.
    #[must_use]
    pub fn children(&self) -> Vec<&Self> {
        match self {
            Self::Number(_)
            | Self::Reference(_)
            | Self::SameRowReference(_)
            | Self::ColumnTotal(_)
            | Self::ColumnAggregate { .. } => Vec::new(),
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right }
            | Self::Compare { left, right, .. } => vec![left, right],
            Self::If {
                condition,
                then_value,
                else_value,
            } => vec![condition, then_value, else_value],
            Self::And(arguments) | Self::Or(arguments) => arguments.iter().collect(),
            Self::Not(value) | Self::Round { value, .. } | Self::IsBlank(value) => vec![value],
        }
    }

    /// Mutable children for identity-preserving formula remapping.
    pub fn children_mut(&mut self) -> Vec<&mut Self> {
        match self {
            Self::Number(_)
            | Self::Reference(_)
            | Self::SameRowReference(_)
            | Self::ColumnTotal(_)
            | Self::ColumnAggregate { .. } => Vec::new(),
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right }
            | Self::Compare { left, right, .. } => vec![left, right],
            Self::If {
                condition,
                then_value,
                else_value,
            } => vec![condition, then_value, else_value],
            Self::And(arguments) | Self::Or(arguments) => arguments.iter_mut().collect(),
            Self::Not(value) | Self::Round { value, .. } | Self::IsBlank(value) => vec![value],
        }
    }

    /// Infer the non-value condition type after structural bounds are checked.
    #[must_use]
    pub fn expression_type(&self) -> Option<FormulaExpressionType> {
        use FormulaExpressionType::{Condition, Number};
        match self {
            Self::Number(_)
            | Self::Reference(_)
            | Self::SameRowReference(_)
            | Self::ColumnTotal(_)
            | Self::ColumnAggregate { .. } => Some(Number),
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Minimum { left, right }
            | Self::Maximum { left, right } => (left.expression_type() == Some(Number)
                && right.expression_type() == Some(Number))
            .then_some(Number),
            Self::Compare { left, right, .. } => (left.expression_type() == Some(Number)
                && right.expression_type() == Some(Number))
            .then_some(Condition),
            Self::If {
                condition,
                then_value,
                else_value,
            } => (condition.expression_type() == Some(Condition)
                && then_value.expression_type() == Some(Number)
                && else_value.expression_type() == Some(Number))
            .then_some(Number),
            Self::And(arguments) | Self::Or(arguments) => (!arguments.is_empty()
                && arguments
                    .iter()
                    .all(|argument| argument.expression_type() == Some(Condition)))
            .then_some(Condition),
            Self::Not(value) => (value.expression_type() == Some(Condition)).then_some(Condition),
            Self::Round { value, digits } => {
                (*digits <= 10 && value.expression_type() == Some(Number)).then_some(Number)
            }
            Self::IsBlank(value) => matches!(
                value.as_ref(),
                Self::Reference(_) | Self::SameRowReference(_)
            )
            .then_some(Condition),
        }
    }

    /// References whose value is read on every evaluation: everything except
    /// IF's `then`/`else` branches (the condition is always read) and ISBLANK's
    /// presence-only probe. A missing value here fails the formula before
    /// evaluation; inside a branch it fails only when that branch is selected.
    #[must_use]
    pub fn unconditional_references(&self, owner: &FieldRef) -> BTreeSet<FieldRef> {
        let mut references = BTreeSet::new();
        let mut pending = vec![self];
        while let Some(node) = pending.pop() {
            match node {
                Self::Reference(reference) => {
                    references.insert(reference.clone());
                }
                Self::SameRowReference(field) => {
                    references.insert(FieldRef::new(owner.entity.clone(), field.clone()));
                }
                Self::If { condition, .. } => pending.push(condition),
                Self::IsBlank(_) => {}
                _ => pending.extend(node.children()),
            }
        }
        references
    }

    /// Every static reference and whether its use requires a Number field.
    #[must_use]
    pub fn reference_uses(&self, owner: &FieldRef) -> Vec<(FieldRef, bool)> {
        let mut uses = Vec::new();
        let mut pending = vec![(self, true)];
        while let Some((node, numeric)) = pending.pop() {
            match node {
                Self::Reference(reference) => uses.push((reference.clone(), numeric)),
                Self::SameRowReference(field) => {
                    uses.push((FieldRef::new(owner.entity.clone(), field.clone()), numeric));
                }
                Self::IsBlank(value) => pending.push((value, false)),
                _ => pending.extend(
                    node.children()
                        .into_iter()
                        .rev()
                        .map(|child| (child, numeric)),
                ),
            }
        }
        uses
    }

    /// Every column total the expression reads, in evaluation order.
    #[must_use]
    pub fn column_totals(&self) -> Vec<&ColumnRef> {
        let mut columns = Vec::new();
        let mut pending = vec![self];
        while let Some(node) = pending.pop() {
            match node {
                Self::ColumnTotal(column) | Self::ColumnAggregate { column, .. } => {
                    columns.push(column);
                }
                _ => pending.extend(node.children().into_iter().rev()),
            }
        }
        columns
    }
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

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldAddress {
    pub entity: EntityKey,
    pub field: FieldKey,
}

impl FieldAddress {
    #[must_use]
    pub fn new(entity: impl Into<EntityKey>, field: impl Into<FieldKey>) -> Self {
        Self {
            entity: entity.into(),
            field: field.into(),
        }
    }
}

impl fmt::Display for FieldAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.entity, self.field)
    }
}

/// A stable column identity: one schema field, independent of any row.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnRef {
    pub schema: SchemaId,
    pub field: FieldId,
}

impl ColumnRef {
    #[must_use]
    pub fn new(schema: impl Into<SchemaId>, field: impl Into<FieldId>) -> Self {
        Self {
            schema: schema.into(),
            field: field.into(),
        }
    }
}

/// Display-only presentation of a Number; never changes stored or calculated values.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumberFormat {
    pub decimals: u8,
    pub kind: NumberFormatKind,
    pub thousands_separator: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", from = "NumberFormatKindWire")]
pub enum NumberFormatKind {
    Plain,
    Currency { code: String },
    Percent,
}

// Unit variants of an internally tagged enum ignore extra keys, so the wire
// shape wraps each kind in a `deny_unknown_fields` struct to keep it closed.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NumberFormatKindWire {
    Plain(EmptyKindWire),
    Currency(CurrencyKindWire),
    Percent(EmptyKindWire),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyKindWire {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrencyKindWire {
    code: String,
}

impl From<NumberFormatKindWire> for NumberFormatKind {
    fn from(wire: NumberFormatKindWire) -> Self {
        match wire {
            NumberFormatKindWire::Plain(_) => Self::Plain,
            NumberFormatKindWire::Currency(CurrencyKindWire { code }) => Self::Currency { code },
            NumberFormatKindWire::Percent(_) => Self::Percent,
        }
    }
}

impl fmt::Display for ColumnRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.schema, self.field)
    }
}

/// A human column address used while authoring a formula.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnAddress {
    pub schema: SchemaKey,
    pub field: FieldKey,
}

impl ColumnAddress {
    #[must_use]
    pub fn new(schema: impl Into<SchemaKey>, field: impl Into<FieldKey>) -> Self {
        Self {
            schema: schema.into(),
            field: field.into(),
        }
    }
}

impl fmt::Display for ColumnAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.schema, self.field)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberFormatError {
    NonNumberField,
    DecimalsOutOfRange,
    InvalidCurrencyCode,
}

impl fmt::Display for NumberFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonNumberField => "number format is only valid on Number fields",
            Self::DecimalsOutOfRange => "number format decimals must be between 0 and 10",
            Self::InvalidCurrencyCode => "currency code must be three uppercase ASCII letters",
        })
    }
}

impl std::error::Error for NumberFormatError {}

impl NumberFormat {
    /// Validate the format declaration independently of storage or rendering.
    ///
    /// # Errors
    /// Returns a typed declaration error without coercion.
    pub fn validate_for(&self, field_type: &FieldType) -> Result<(), NumberFormatError> {
        if *field_type != FieldType::Number {
            return Err(NumberFormatError::NonNumberField);
        }
        if self.decimals > 10 {
            return Err(NumberFormatError::DecimalsOutOfRange);
        }
        if let NumberFormatKind::Currency { code } = &self.kind {
            if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_uppercase()) {
                return Err(NumberFormatError::InvalidCurrencyCode);
            }
        }
        Ok(())
    }
}
