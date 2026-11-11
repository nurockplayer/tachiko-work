//! ADR-0049 stored presentation: cell styles, tables, rules and cell bindings.
//!
//! The store sits beside [`Document`]; it is never part of the semantic document
//! and a change to it is never a semantic change.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    ConditionalAggregate, Document, EntityId, Expression, FieldId, MAX_EXPRESSION_DEPTH,
    MAX_EXPRESSION_NODES, NumberFormat, NumberFormatError, Schema, SchemaId, display_name_failures,
};

/// Maximum number of styles a document may hold.
pub const MAX_STYLES: usize = 4096;

/// Maximum number of conditional rules one table may hold.
pub const MAX_RULES_PER_TABLE: usize = 64;

macro_rules! presentation_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

presentation_id!(StyleId);
presentation_id!(RuleId);

/// One palette entry. Absent members mean "no opinion".
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CellStyle {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub fill: Option<String>,
    pub text_color: Option<String>,
    pub alignment: Option<CellAlignment>,
    pub wrap: Option<bool>,
    pub borders: Option<Vec<BorderSide>>,
    pub number_format: Option<NumberFormat>,
    pub date_format: Option<DateFormat>,
}

/// Horizontal cell alignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellAlignment {
    Left,
    Center,
    Right,
}

/// One drawn border side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorderSide {
    Top,
    Right,
    Bottom,
    Left,
}

/// Closed ADR-0049 Date display format set.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    Iso,
    Mdy,
    Dmy,
    YmdSlash,
    YmdCjk,
    MonthDayYear,
    DayMonthYear,
}

/// Footer aggregate choice for one field; only the choice is stored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FooterAggregate {
    Sum,
    Count,
    Average,
    Minimum,
    Maximum,
}

/// Presentation of one whole table.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TablePresentation {
    pub title: Option<String>,
    pub footer: BTreeMap<FieldId, FooterAggregate>,
    pub column_styles: BTreeMap<FieldId, StyleId>,
    pub column_widths: BTreeMap<FieldId, u32>,
}

/// One conditional formatting rule. Its identity is the map key that holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalRule {
    pub schema: SchemaId,
    pub rank: String,
    pub fields: Vec<FieldId>,
    pub condition: Expression,
    pub style: StyleId,
}

/// Stored presentation records, keyed by their stable identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationStore {
    pub styles: BTreeMap<StyleId, CellStyle>,
    pub tables: BTreeMap<SchemaId, TablePresentation>,
    pub rules: BTreeMap<RuleId, ConditionalRule>,
    pub cells: BTreeMap<(EntityId, FieldId), StyleId>,
}

impl PresentationStore {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
            && self.tables.is_empty()
            && self.rules.is_empty()
            && self.cells.is_empty()
    }
}

/// A typed presentation validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationError {
    TooManyStyles {
        found: usize,
        limit: usize,
    },
    TooManyRules {
        schema: SchemaId,
        found: usize,
        limit: usize,
    },
    MissingSchema {
        schema: SchemaId,
    },
    MissingEntity {
        entity: EntityId,
    },
    MissingField {
        schema: SchemaId,
        field: FieldId,
    },
    CellFieldNotOnEntitySchema {
        entity: EntityId,
        field: FieldId,
    },
    MissingStyle {
        style: StyleId,
    },
    UnusedStyle {
        style: StyleId,
    },
    InvalidColumnWidth {
        schema: SchemaId,
        field: FieldId,
        width: u32,
    },
    EmptyTable {
        schema: SchemaId,
    },
    EmptyRuleFields {
        rule: RuleId,
    },
    DuplicateRuleField {
        rule: RuleId,
        field: FieldId,
    },
    InvalidTitle {
        schema: SchemaId,
        failures: Vec<crate::DisplayNameFailure>,
    },
    InvalidRank {
        rule: RuleId,
        rank: String,
    },
    InvalidNumberFormat {
        style: StyleId,
        error: NumberFormatError,
    },
    InvalidColor {
        style: StyleId,
        member: &'static str,
        value: String,
    },
    DuplicateBorderSide {
        style: StyleId,
        side: BorderSide,
    },
    BorderSidesOutOfOrder {
        style: StyleId,
    },
    RuleConditionReference {
        rule: RuleId,
    },
    RuleExpressionTooDeep {
        rule: RuleId,
        limit: usize,
    },
    RuleExpressionTooLarge {
        rule: RuleId,
        limit: usize,
    },
}

impl fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyStyles { found, limit } => {
                write!(
                    formatter,
                    "presentation holds {found} styles, over the limit {limit}"
                )
            }
            Self::TooManyRules {
                schema,
                found,
                limit,
            } => write!(
                formatter,
                "table '{schema}' holds {found} rules, over the limit {limit}"
            ),
            Self::MissingSchema { schema } => {
                write!(formatter, "presentation names missing schema '{schema}'")
            }
            Self::MissingEntity { entity } => {
                write!(formatter, "presentation names missing entity '{entity}'")
            }
            Self::MissingField { schema, field } => {
                write!(
                    formatter,
                    "presentation names missing field '{schema}.{field}'"
                )
            }
            Self::CellFieldNotOnEntitySchema { entity, field } => write!(
                formatter,
                "cell ({entity}, {field}) names a field that is not on the entity's schema"
            ),
            Self::MissingStyle { style } => {
                write!(formatter, "presentation references missing style '{style}'")
            }
            Self::UnusedStyle { style } => {
                write!(formatter, "presentation style '{style}' is used by nothing")
            }
            Self::InvalidColumnWidth {
                schema,
                field,
                width,
            } => write!(
                formatter,
                "column width for '{schema}.{field}' is out of range: {width}"
            ),
            Self::EmptyTable { schema } => {
                write!(formatter, "table presentation '{schema}' has no content")
            }
            Self::EmptyRuleFields { rule } => {
                write!(formatter, "rule '{rule}' paints no field")
            }
            Self::DuplicateRuleField { rule, field } => {
                write!(formatter, "rule '{rule}' lists field '{field}' twice")
            }
            Self::InvalidTitle { schema, failures } => write!(
                formatter,
                "table '{schema}' title fails the display-name rules: {failures:?}"
            ),
            Self::InvalidRank { rule, rank } => write!(
                formatter,
                "rule '{rule}' rank '{rank}' is outside the ADR-0048 grammar"
            ),
            Self::InvalidNumberFormat { style, error } => {
                write!(
                    formatter,
                    "style '{style}' number_format is invalid: {error}"
                )
            }
            Self::InvalidColor {
                style,
                member,
                value,
            } => write!(
                formatter,
                "style '{style}' {member} '{value}' is not '#' and six uppercase hexadecimal digits"
            ),
            Self::DuplicateBorderSide { style, side } => {
                write!(formatter, "style '{style}' lists border {side:?} twice")
            }
            Self::BorderSidesOutOfOrder { style } => write!(
                formatter,
                "style '{style}' borders are not in top, right, bottom, left order"
            ),
            Self::RuleConditionReference { rule } => write!(
                formatter,
                "rule '{rule}' condition holds a record reference"
            ),
            Self::RuleExpressionTooDeep { rule, limit } => write!(
                formatter,
                "rule '{rule}' condition exceeds the {limit}-depth expression limit"
            ),
            Self::RuleExpressionTooLarge { rule, limit } => write!(
                formatter,
                "rule '{rule}' condition exceeds the {limit}-node expression limit"
            ),
        }
    }
}

impl std::error::Error for PresentationError {}

fn rule_rank_is_valid(rank: &str) -> bool {
    !rank.is_empty()
        && rank.len() <= 64
        && rank.bytes().all(|byte| byte.is_ascii_alphanumeric())
        && !rank.ends_with('0')
}

const fn border_rank(side: BorderSide) -> u8 {
    match side {
        BorderSide::Top => 0,
        BorderSide::Right => 1,
        BorderSide::Bottom => 2,
        BorderSide::Left => 3,
    }
}

fn is_canonical_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..]
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'A'..=b'F'))
}

fn validate_style_spelling(style_id: &StyleId, style: &CellStyle) -> Result<(), PresentationError> {
    for (member, color) in [("fill", &style.fill), ("text_color", &style.text_color)] {
        let Some(value) = color else { continue };
        if !is_canonical_color(value) {
            return Err(PresentationError::InvalidColor {
                style: style_id.clone(),
                member,
                value: value.clone(),
            });
        }
    }
    let Some(borders) = &style.borders else {
        return Ok(());
    };
    for pair in borders.windows(2) {
        match border_rank(pair[0]).cmp(&border_rank(pair[1])) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(PresentationError::DuplicateBorderSide {
                    style: style_id.clone(),
                    side: pair[0],
                });
            }
            Ordering::Greater => {
                return Err(PresentationError::BorderSidesOutOfOrder {
                    style: style_id.clone(),
                });
            }
        }
    }
    Ok(())
}

fn check_column(document: &Document, column: &crate::ColumnRef) -> Result<(), PresentationError> {
    let schema =
        document
            .schemas
            .get(&column.schema)
            .ok_or_else(|| PresentationError::MissingSchema {
                schema: column.schema.clone(),
            })?;
    if !schema.fields.contains_key(&column.field) {
        return Err(PresentationError::MissingField {
            schema: column.schema.clone(),
            field: column.field.clone(),
        });
    }
    Ok(())
}

fn validate_rule_condition(
    rule_id: &RuleId,
    schema: &Schema,
    condition: &Expression,
    document: &Document,
) -> Result<(), PresentationError> {
    let mut nodes = 0_usize;
    let mut stack = vec![(condition, 1_usize)];
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(PresentationError::RuleExpressionTooDeep {
                rule: rule_id.clone(),
                limit: MAX_EXPRESSION_DEPTH,
            });
        }
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return Err(PresentationError::RuleExpressionTooLarge {
                rule: rule_id.clone(),
                limit: MAX_EXPRESSION_NODES,
            });
        }
        match node {
            Expression::Reference(_) => {
                return Err(PresentationError::RuleConditionReference {
                    rule: rule_id.clone(),
                });
            }
            Expression::SameRowReference(field) => {
                if !schema.fields.contains_key(field) {
                    return Err(PresentationError::MissingField {
                        schema: schema.id.clone(),
                        field: field.clone(),
                    });
                }
            }
            Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                check_column(document, column)?;
            }
            Expression::ColumnConditional {
                aggregate,
                criteria,
            } => {
                match aggregate.as_ref() {
                    ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) => {
                        check_column(document, column)?;
                    }
                    ConditionalAggregate::Count => {}
                }
                for criterion in criteria {
                    check_column(document, &criterion.column)?;
                }
            }
            _ => {}
        }
        stack.extend(node.children().into_iter().map(|child| (child, depth + 1)));
    }
    Ok(())
}

/// Drop every presentation record whose target is gone from `document`.
///
/// A live occurrence keeps its store whole through a delete and Undo, so Undo
/// restores the records with no history work. A save writes the result of this
/// function instead, and a file never holds a dangling reference. The result
/// passes [`validate_presentation`] whenever the input passed it against an
/// earlier version of the document.
#[must_use]
pub fn prune_presentation(store: &PresentationStore, document: &Document) -> PresentationStore {
    let tables = store
        .tables
        .iter()
        .filter_map(|(schema_id, table)| {
            let schema = document.schemas.get(schema_id)?;
            let footer = table
                .footer
                .iter()
                .filter(|(field, _)| schema.fields.contains_key(*field))
                .map(|(field, aggregate)| (field.clone(), *aggregate))
                .collect::<BTreeMap<_, _>>();
            let column_styles = table
                .column_styles
                .iter()
                .filter(|(field, _)| schema.fields.contains_key(*field))
                .map(|(field, style)| (field.clone(), style.clone()))
                .collect::<BTreeMap<_, _>>();
            let column_widths = table
                .column_widths
                .iter()
                .filter(|(field, _)| schema.fields.contains_key(*field))
                .map(|(field, width)| (field.clone(), *width))
                .collect::<BTreeMap<_, _>>();
            if table.title.is_none()
                && footer.is_empty()
                && column_styles.is_empty()
                && column_widths.is_empty()
            {
                return None;
            }
            Some((
                schema_id.clone(),
                TablePresentation {
                    title: table.title.clone(),
                    footer,
                    column_styles,
                    column_widths,
                },
            ))
        })
        .collect::<BTreeMap<_, _>>();

    let rules = store
        .rules
        .iter()
        .filter(|(_, rule)| rule_survives_prune(rule, document))
        .map(|(rule_id, rule)| (rule_id.clone(), rule.clone()))
        .collect::<BTreeMap<_, _>>();

    let cells = store
        .cells
        .iter()
        .filter(|entry| {
            let (entity_id, field) = entry.0;
            document
                .entities
                .get(entity_id)
                .and_then(|entity| document.schemas.get(&entity.schema))
                .is_some_and(|schema| schema.fields.contains_key(field))
        })
        .map(|(key, style)| (key.clone(), style.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut used = BTreeSet::new();
    for style in tables
        .values()
        .flat_map(|table| table.column_styles.values())
    {
        used.insert(style.clone());
    }
    used.extend(rules.values().map(|rule| rule.style.clone()));
    used.extend(cells.values().cloned());
    let styles = store
        .styles
        .iter()
        .filter(|(style_id, _)| used.contains(*style_id))
        .map(|(style_id, style)| (style_id.clone(), style.clone()))
        .collect();

    PresentationStore {
        styles,
        tables,
        rules,
        cells,
    }
}

fn rule_survives_prune(rule: &ConditionalRule, document: &Document) -> bool {
    let Some(schema) = document.schemas.get(&rule.schema) else {
        return false;
    };
    if rule
        .fields
        .iter()
        .any(|field| !schema.fields.contains_key(field))
    {
        return false;
    }
    let mut stack = vec![&rule.condition];
    while let Some(node) = stack.pop() {
        match node {
            Expression::SameRowReference(field) => {
                if !schema.fields.contains_key(field) {
                    return false;
                }
            }
            Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
                if !column_exists(document, column) {
                    return false;
                }
            }
            Expression::ColumnConditional {
                aggregate,
                criteria,
            } => {
                let aggregate_column = match aggregate.as_ref() {
                    ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) => {
                        Some(column)
                    }
                    ConditionalAggregate::Count => None,
                };
                if aggregate_column.is_some_and(|column| !column_exists(document, column)) {
                    return false;
                }
                if criteria
                    .iter()
                    .any(|criterion| !column_exists(document, &criterion.column))
                {
                    return false;
                }
            }
            _ => {}
        }
        stack.extend(node.children());
    }
    true
}

fn column_exists(document: &Document, column: &crate::ColumnRef) -> bool {
    document
        .schemas
        .get(&column.schema)
        .is_some_and(|schema| schema.fields.contains_key(&column.field))
}

/// Check a presentation store against the document it belongs to.
///
/// # Errors
///
/// Returns a typed [`PresentationError`] for a dangling or unused reference, a
/// form a rule condition may not use, an out-of-bounds store, a title, rank or
/// number format outside its declared grammar, or a color or border list with
/// no canonical spelling.
#[allow(clippy::too_many_lines)] // One ordered refusal pass keeps the error precedence visible.
pub fn validate_presentation(
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

    for (style_id, style) in &store.styles {
        validate_style_spelling(style_id, style)?;
        if let Some(format) = &style.number_format {
            format
                .validate_for(&crate::FieldType::Number)
                .map_err(|error| PresentationError::InvalidNumberFormat {
                    style: style_id.clone(),
                    error,
                })?;
        }
    }

    for (schema_id, table) in &store.tables {
        let schema =
            document
                .schemas
                .get(schema_id)
                .ok_or_else(|| PresentationError::MissingSchema {
                    schema: schema_id.clone(),
                })?;
        if table.title.is_none()
            && table.footer.is_empty()
            && table.column_styles.is_empty()
            && table.column_widths.is_empty()
        {
            return Err(PresentationError::EmptyTable {
                schema: schema_id.clone(),
            });
        }
        if let Some(title) = &table.title {
            let failures = display_name_failures(title);
            if !failures.is_empty() {
                return Err(PresentationError::InvalidTitle {
                    schema: schema_id.clone(),
                    failures,
                });
            }
        }
        for field in table
            .footer
            .keys()
            .chain(table.column_styles.keys())
            .chain(table.column_widths.keys())
        {
            if !schema.fields.contains_key(field) {
                return Err(PresentationError::MissingField {
                    schema: schema_id.clone(),
                    field: field.clone(),
                });
            }
        }
        for (field, width) in &table.column_widths {
            if !(1..=65280).contains(width) {
                return Err(PresentationError::InvalidColumnWidth {
                    schema: schema_id.clone(),
                    field: field.clone(),
                    width: *width,
                });
            }
        }
        for style in table.column_styles.values() {
            if !store.styles.contains_key(style) {
                return Err(PresentationError::MissingStyle {
                    style: style.clone(),
                });
            }
        }
    }

    for (rule_id, rule) in &store.rules {
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
    }

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
