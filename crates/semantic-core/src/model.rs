use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::BTreeMap,
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

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
    pub key: SchemaKey,
    pub fields: BTreeMap<FieldId, FieldDefinition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDefinition {
    pub id: FieldId,
    pub key: FieldKey,
    pub field_type: FieldType,
    pub required: bool,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", content = "args", rename_all = "snake_case")]
pub enum Expression {
    Number(Number),
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
