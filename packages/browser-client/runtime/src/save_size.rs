//! Upper bound on the bytes one document's v3 save writes.
//!
//! The v3 codec stores JSON, so raw document bytes are not the saved size:
//! control characters escape to six bytes and identities repeat per cell and
//! per formula reference. This module mirrors the writers in
//! `crates/storage/src/roproj/v3.rs`, `crates/storage/src/number_format.rs`
//! and the framing of the app-private bundle and the portable package, and
//! counts the result without allocating a string per value.
//!
//! Every count is a `u64`: on the browser's wasm32 target `usize` is 32 bits,
//! and a document within the working-size limits can carry more than 4 GiB of
//! written formula references. Saturating `u64` arithmetic keeps that bound
//! an over-estimate instead of wrapping below the save limit.

use tachiko_storage::encode_presentation;
use tachiko_workspace_engine::{
    ColumnRef, ComparisonOperator, ConditionalAggregate, Document, Entity, Expression,
    FieldConstraint, FieldDefinition, FieldType, KeyedGroupedSumDefinition, Number, NumberFormat,
    NumberFormatKind, PresentationStore, Schema, Value,
};

use crate::DesignerError;

/// Every non-payload byte one save adds around the file bodies. The larger,
/// portable ZIP framing is used so one bound covers both exports: 21 entries
/// (the package manifest, the nineteen payload files and the optional
/// presentation member) at 30-byte local and 46-byte central headers plus the
/// 22-byte end record, every name written in both headers, and the fixed
/// 228-byte `package.json`. The app-private bundle's framing is smaller still.
const EXPORT_FRAMING_BYTES: u64 = 2_880;

/// Bytes `export_project_v3` can write for this document, never less than it
/// does. It also covers `export_portable_ro`, whose framing is larger.
///
/// Callers add [`presentation_saved_bytes`]; presentation is not part of
/// `Document`.
pub(crate) fn saved_bytes_upper_bound(document: &Document) -> u64 {
    #[cfg(test)]
    record_bound_call();
    total([
        manifest_bytes(document),
        schemas_bytes(document),
        definitions_bytes(document),
        entities_bytes(document),
        EXPORT_FRAMING_BYTES,
    ])
}

/// Bytes the optional `presentation.jsonl` member adds to a save.
pub(crate) fn presentation_saved_bytes(store: &PresentationStore) -> Result<u64, DesignerError> {
    Ok(encode_presentation(store)?.map_or(0, |bytes| length(bytes.len())))
}

#[cfg(test)]
thread_local! {
    static SAVED_BOUND_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_bound_call() {
    SAVED_BOUND_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
}

/// How many times this test thread computed the bound, to prove that ordinary
/// profiles never do.
#[cfg(test)]
pub(crate) fn saved_bound_calls() -> usize {
    SAVED_BOUND_CALLS.with(std::cell::Cell::get)
}

/// A host length as a saved-size term.
fn length(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Saturating sum of size terms.
fn total(terms: impl IntoIterator<Item = u64>) -> u64 {
    terms.into_iter().fold(0_u64, u64::saturating_add)
}

fn flag(value: bool) -> u64 {
    u64::from(value)
}

/// `serde_json`'s exact escaped length, without the surrounding quotes.
///
/// `"` and `\` are two bytes, `\b \t \n \f \r` are two, every other byte below
/// `0x20` is six, and every other byte is one. Non-ASCII bytes are never
/// escaped.
fn escaped_bytes(value: &str) -> u64 {
    value.bytes().fold(0_u64, |sum, byte| {
        sum.saturating_add(match byte {
            b'"' | b'\\' | 0x08 | 0x09 | 0x0A | 0x0C | 0x0D => 2,
            0x00..=0x1F => 6,
            _ => 1,
        })
    })
}

fn string_bytes(value: &str) -> u64 {
    escaped_bytes(value).saturating_add(2)
}

fn indent_bytes(indent: u64) -> u64 {
    indent.saturating_mul(2)
}

fn pretty_prefix(indent: u64, name: &str) -> u64 {
    total([indent_bytes(indent), string_bytes(name), 2])
}

fn pretty_string(indent: u64, name: &str, value: &str, comma: bool) -> u64 {
    total([
        pretty_prefix(indent, name),
        string_bytes(value),
        flag(comma),
        1,
    ])
}

fn pretty_literal(indent: u64, name: &str, value_bytes: u64, comma: bool) -> u64 {
    total([pretty_prefix(indent, name), value_bytes, flag(comma), 1])
}

/// Bytes `ryu_js` writes for one number, the formatter `v3.rs` uses. `Number`
/// already rejects non-finite values and normalizes either zero sign, which is
/// exactly the writer's normalization.
fn number_bytes(number: Number) -> u64 {
    number_text_bytes(number.get())
}

/// The writer's `format_finite` call on a value that is already a number
/// parameter, such as `round` digits.
fn number_text_bytes(value: f64) -> u64 {
    let mut buffer = ryu_js::Buffer::new();
    length(
        buffer
            .format_finite(if value == 0.0 { 0.0 } else { value })
            .len(),
    )
}

fn pretty_number(indent: u64, name: &str, number: Number, comma: bool) -> u64 {
    total([
        pretty_prefix(indent, name),
        number_bytes(number),
        flag(comma),
        1,
    ])
}

fn compact_prefix(name: &str) -> u64 {
    total([string_bytes(name), 1])
}

fn compact_string(name: &str, value: &str, comma: bool) -> u64 {
    total([compact_prefix(name), string_bytes(value), flag(comma)])
}

fn compact_literal(name: &str, value_bytes: u64, comma: bool) -> u64 {
    total([compact_prefix(name), value_bytes, flag(comma)])
}

fn compact_number(name: &str, number: Number, comma: bool) -> u64 {
    total([compact_prefix(name), number_bytes(number), flag(comma)])
}

fn manifest_bytes(document: &Document) -> u64 {
    total([
        2, // "{\n"
        pretty_string(1, "format", "tachiko.roproj", true),
        pretty_literal(1, "format_version", 1, true),
        pretty_prefix(1, "document"),
        2, // "{\n"
        pretty_string(2, "id", document.id.as_str(), true),
        pretty_string(2, "title", &document.title, false),
        indent_bytes(1),
        2, // "}\n"
        2, // "}\n"
    ])
}

fn schemas_bytes(document: &Document) -> u64 {
    let count = document.schemas.len();
    if count == 0 {
        return 3; // "[]\n"
    }
    let entries = document
        .schemas
        .values()
        .enumerate()
        .fold(0_u64, |sum, (index, schema)| {
            sum.saturating_add(total([schema_bytes(schema), 1, flag(index + 1 != count)]))
        });
    total([
        2, // "[\n"
        entries, 2, // "]\n"
    ])
}

fn schema_bytes(schema: &Schema) -> u64 {
    let indent = 1;
    let field_count = schema.fields.len();
    total([
        indent_bytes(indent),
        2, // "{\n"
        pretty_string(indent + 1, "id", schema.id.as_str(), true),
        pretty_string(indent + 1, "key", schema.key.as_str(), true),
        schema.display_name.as_ref().map_or(0, |name| {
            pretty_string(indent + 1, "display_name", name, true)
        }),
        schema.field_order.as_ref().map_or(0, |order| {
            total([
                pretty_prefix(indent + 1, "field_order"),
                1, // "["
                order.iter().fold(0_u64, |sum, id| {
                    sum.saturating_add(string_bytes(id.as_str()))
                }),
                length(order.len()).saturating_sub(1).saturating_mul(2),
                3, // "],\n"
            ])
        }),
        pretty_prefix(indent + 1, "fields"),
        if schema.fields.is_empty() {
            3 // "[]\n"
        } else {
            let entries = schema
                .fields
                .values()
                .enumerate()
                .fold(0_u64, |sum, (index, field)| {
                    sum.saturating_add(total([
                        field_bytes(field, indent + 2),
                        1,
                        flag(index + 1 != field_count),
                    ]))
                });
            total([
                2, // "[\n"
                entries,
                indent_bytes(indent + 1),
                2, // "]\n"
            ])
        },
        indent_bytes(indent),
        1, // "}"
    ])
}

fn field_bytes(field: &FieldDefinition, indent: u64) -> u64 {
    total([
        indent_bytes(indent),
        2, // "{\n"
        pretty_string(indent + 1, "id", field.id.as_str(), true),
        pretty_string(indent + 1, "key", field.key.as_str(), true),
        field.display_name.as_ref().map_or(0, |name| {
            pretty_string(indent + 1, "display_name", name, true)
        }),
        pretty_prefix(indent + 1, "field_type"),
        field_type_bytes(&field.field_type, indent + 1),
        2, // ",\n"
        pretty_literal(
            indent + 1,
            "required",
            if field.required { 4 } else { 5 },
            true,
        ),
        pretty_prefix(indent + 1, "constraint"),
        constraint_bytes(&field.constraint, indent + 1),
        field.number_format.as_ref().map_or(0, |format| {
            total([
                2, // ",\n"
                pretty_prefix(indent + 1, "number_format"),
                number_format_bytes(format, indent + 1),
            ])
        }),
        field.date_format.map_or(0, |_| {
            total([
                2, // ",\n"
                pretty_prefix(indent + 1, "date_format"),
                // The longest stored name; one wider name costs a few bytes.
                string_bytes("month_day_year"),
            ])
        }),
        1, // final '\n'
        indent_bytes(indent),
        1, // "}"
    ])
}

fn field_type_bytes(field_type: &FieldType, indent: u64) -> u64 {
    total([
        2, // "{\n"
        match field_type {
            FieldType::Number => pretty_string(indent + 1, "type", "number", false),
            FieldType::Text => pretty_string(indent + 1, "type", "text", false),
            FieldType::Boolean => pretty_string(indent + 1, "type", "boolean", false),
            FieldType::Date => pretty_string(indent + 1, "type", "date", false),
            FieldType::Reference { schema } => total([
                pretty_string(indent + 1, "type", "reference", true),
                pretty_string(indent + 1, "schema", schema.as_str(), false),
            ]),
        },
        indent_bytes(indent),
        1, // "}"
    ])
}

fn constraint_bytes(constraint: &FieldConstraint, indent: u64) -> u64 {
    total([
        2, // "{\n"
        match constraint {
            FieldConstraint::None => pretty_string(indent + 1, "type", "none", false),
            FieldConstraint::TextLiteralSet { values } => total([
                pretty_string(indent + 1, "type", "text_literal_set", true),
                pretty_prefix(indent + 1, "values"),
                if values.is_empty() {
                    3 // "[]\n"
                } else {
                    let count = values.len();
                    let entries = values
                        .iter()
                        .enumerate()
                        .fold(0_u64, |sum, (index, value)| {
                            sum.saturating_add(total([
                                indent_bytes(indent + 2),
                                string_bytes(value),
                                flag(index + 1 != count),
                                1,
                            ]))
                        });
                    total([
                        2, // "[\n"
                        entries,
                        indent_bytes(indent + 1),
                        2, // "]\n"
                    ])
                },
            ]),
            FieldConstraint::NumberInclusiveRange { min, max } => total([
                pretty_string(indent + 1, "type", "number_inclusive_range", true),
                pretty_number(indent + 1, "min", *min, true),
                pretty_number(indent + 1, "max", *max, false),
            ]),
        },
        indent_bytes(indent),
        1, // "}"
    ])
}

/// Bytes of the pretty JSON `crates/storage/src/number_format.rs` writes. The
/// caller's line prefix already emitted `indent` for the first line and is
/// re-emitted after every newline.
fn number_format_bytes(format: &NumberFormat, indent: u64) -> u64 {
    let line = total([1, indent_bytes(indent)]);
    total([
        1, // "{"
        line,
        2,
        string_bytes("decimals"),
        2,
        decimal_digits(format.decimals),
        1,
        line,
        2,
        string_bytes("kind"),
        2,
        1, // "{"
        match &format.kind {
            NumberFormatKind::Plain => {
                total([line, 4, string_bytes("type"), 2, string_bytes("plain")])
            }
            NumberFormatKind::Percent => {
                total([line, 4, string_bytes("type"), 2, string_bytes("percent")])
            }
            NumberFormatKind::Currency { code } => total([
                line,
                4,
                string_bytes("type"),
                2,
                string_bytes("currency"),
                1,
                line,
                4,
                string_bytes("code"),
                2,
                string_bytes(code),
            ]),
        },
        line,
        4, // "  },"
        line,
        2,
        string_bytes("thousands_separator"),
        2,
        if format.thousands_separator { 4 } else { 5 },
        line,
        1, // "}"
    ])
}

fn decimal_digits(value: u8) -> u64 {
    if value >= 100 {
        3
    } else if value >= 10 {
        2
    } else {
        1
    }
}

fn definitions_bytes(document: &Document) -> u64 {
    let count = document.keyed_grouped_sum_definitions.len();
    if count == 0 {
        return 3; // "[]\n"
    }
    let entries = document
        .keyed_grouped_sum_definitions
        .values()
        .enumerate()
        .fold(0_u64, |sum, (index, definition)| {
            sum.saturating_add(total([
                definition_bytes(definition),
                1,
                flag(index + 1 != count),
            ]))
        });
    total([
        2, // "[\n"
        entries, 2, // "]\n"
    ])
}

fn definition_bytes(definition: &KeyedGroupedSumDefinition) -> u64 {
    let indent = 1;
    total([
        indent_bytes(indent),
        2, // "{\n"
        pretty_string(indent + 1, "id", definition.id.as_str(), true),
        pretty_prefix(indent + 1, "orders"),
        2, // "{\n"
        pretty_string(
            indent + 2,
            "schema",
            definition.orders.schema.as_str(),
            true,
        ),
        pretty_string(
            indent + 2,
            "lookup_key_field",
            definition.orders.lookup_key_field.as_str(),
            true,
        ),
        pretty_string(
            indent + 2,
            "quantity_field",
            definition.orders.quantity_field.as_str(),
            false,
        ),
        indent_bytes(indent + 1),
        3, // "},\n"
        pretty_prefix(indent + 1, "products"),
        2, // "{\n"
        pretty_string(
            indent + 2,
            "schema",
            definition.products.schema.as_str(),
            true,
        ),
        pretty_string(
            indent + 2,
            "key_field",
            definition.products.key_field.as_str(),
            true,
        ),
        pretty_string(
            indent + 2,
            "category_field",
            definition.products.category_field.as_str(),
            true,
        ),
        pretty_string(
            indent + 2,
            "price_field",
            definition.products.price_field.as_str(),
            false,
        ),
        indent_bytes(indent + 1),
        2, // "}\n"
        indent_bytes(indent),
        1, // "}"
    ])
}

fn entities_bytes(document: &Document) -> u64 {
    document.entities.values().fold(0_u64, |sum, entity| {
        sum.saturating_add(total([entity_bytes(entity), 1]))
    })
}

fn entity_bytes(entity: &Entity) -> u64 {
    total([
        1, // "{"
        compact_string("id", entity.id.as_str(), true),
        compact_string("key", entity.key.as_str(), true),
        compact_string("schema", entity.schema.as_str(), true),
        compact_prefix("fields"),
        1, // "{"
        entity_fields_bytes(entity),
        2, // "}}"
    ])
}

/// Bytes one entity's stored `fields` object contributes, without the wrapper
/// around it. A value edit that only changes stored cells can diff this value
/// instead of walking the whole document.
pub(crate) fn entity_fields_bytes(entity: &Entity) -> u64 {
    let field_count = entity.fields.len();
    entity
        .fields
        .iter()
        .enumerate()
        .fold(0_u64, |sum, (index, (field, value))| {
            sum.saturating_add(total([
                string_bytes(field.as_str()),
                1,
                value_bytes(value),
                flag(index + 1 != field_count),
            ]))
        })
}

fn value_bytes(value: &Value) -> u64 {
    match value {
        Value::Number(number) => total([
            1,
            compact_string("kind", "number", true),
            compact_number("value", *number, false),
            1,
        ]),
        Value::Text(text) => total([
            1,
            compact_string("kind", "text", true),
            compact_string("value", text, false),
            1,
        ]),
        Value::Boolean(boolean) => total([
            1,
            compact_string("kind", "boolean", true),
            compact_literal("value", if *boolean { 4 } else { 5 }, false),
            1,
        ]),
        Value::Date(_) => total([
            1,
            compact_string("kind", "date", true),
            compact_literal("value", 12, false),
            1,
        ]),
        Value::Reference(entity) => total([
            1,
            compact_string("kind", "reference", true),
            compact_string("value", entity.as_str(), false),
            1,
        ]),
        Value::Formula(expression) => total([
            1,
            compact_string("kind", "formula", true),
            compact_prefix("value"),
            expression_bytes(expression),
            1,
        ]),
    }
}

fn column_bytes(column: &ColumnRef) -> u64 {
    total([
        1, // "{"
        compact_string("schema", column.schema.as_str(), true),
        compact_string("field", column.field.as_str(), false),
        1, // "}"
    ])
}

fn binary_bytes(operator: &str, left: &Expression, right: &Expression) -> u64 {
    total([
        1, // "{"
        compact_string("op", operator, true),
        compact_prefix("args"),
        1, // "{"
        compact_prefix("left"),
        expression_bytes(left),
        1, // ","
        compact_prefix("right"),
        expression_bytes(right),
        2, // "}}"
    ])
}

fn compare_bytes(operator: ComparisonOperator, left: &Expression, right: &Expression) -> u64 {
    total([
        1, // "{"
        compact_string("op", "compare", true),
        compact_prefix("args"),
        1, // "{"
        compact_string("operator", operator.as_str(), true),
        compact_prefix("left"),
        expression_bytes(left),
        1, // ","
        compact_prefix("right"),
        expression_bytes(right),
        2, // "}}"
    ])
}

#[allow(clippy::too_many_lines)] // One arm per closed-world expression variant keeps the mirror auditable.
fn expression_bytes(expression: &Expression) -> u64 {
    match expression {
        Expression::Number(number) => total([
            1,
            compact_string("op", "number", true),
            compact_number("args", *number, false),
            1,
        ]),
        Expression::Reference(reference) => total([
            1,
            compact_string("op", "reference", true),
            compact_prefix("args"),
            1, // "{"
            compact_string("entity", reference.entity.as_str(), true),
            compact_string("field", reference.field.as_str(), false),
            2, // "}}"
        ]),
        Expression::SameRowReference(field) => total([
            1,
            compact_string("op", "same_row_reference", true),
            compact_string("args", field.as_str(), false),
            1,
        ]),
        Expression::ColumnTotal(column) => total([
            1,
            compact_string("op", "column_total", true),
            compact_prefix("args"),
            column_bytes(column),
            1,
        ]),
        Expression::ColumnAggregate { function, column } => total([
            1,
            compact_string("op", "column_aggregate", true),
            compact_prefix("args"),
            1, // "{"
            compact_string("function", function.wire_name(), true),
            compact_prefix("column"),
            column_bytes(column),
            2, // "}}"
        ]),
        Expression::ColumnConditional {
            aggregate,
            criteria,
        } => {
            let (aggregate, values) = match aggregate.as_ref() {
                ConditionalAggregate::Sum(column) => ("sum", Some(column)),
                ConditionalAggregate::Average(column) => ("average", Some(column)),
                ConditionalAggregate::Count => ("count", None),
            };
            let count = criteria.len();
            let criteria_bytes =
                criteria
                    .iter()
                    .enumerate()
                    .fold(0_u64, |sum, (index, criterion)| {
                        let value = criterion.value.as_ref().map_or(0, |value| {
                            total([
                                1, // ","
                                compact_prefix("value"),
                                expression_bytes(value),
                            ])
                        });
                        sum.saturating_add(total([
                            1, // "{"
                            compact_prefix("column"),
                            column_bytes(&criterion.column),
                            1, // ","
                            compact_string("operator", criterion.operator.as_str(), false),
                            value,
                            1, // "}"
                            flag(index + 1 != count),
                        ]))
                    });
            total([
                1, // "{"
                compact_string("op", "column_conditional", true),
                compact_prefix("args"),
                1, // "{"
                compact_string("aggregate", aggregate, true),
                values.map_or(0, |column| {
                    total([
                        compact_prefix("values"),
                        column_bytes(column),
                        1, // ","
                    ])
                }),
                compact_prefix("criteria"),
                1, // "["
                criteria_bytes,
                1, // "]"
                2, // "}}"
            ])
        }
        Expression::Add { left, right } => binary_bytes("add", left, right),
        Expression::Subtract { left, right } => binary_bytes("subtract", left, right),
        Expression::Multiply { left, right } => binary_bytes("multiply", left, right),
        Expression::Divide { left, right } => binary_bytes("divide", left, right),
        Expression::Minimum { left, right } => binary_bytes("minimum", left, right),
        Expression::Maximum { left, right } => binary_bytes("maximum", left, right),
        Expression::Compare {
            operator,
            left,
            right,
        } => compare_bytes(*operator, left, right),
        Expression::If {
            condition,
            then_value,
            else_value,
        } => total([
            1, // "{"
            compact_string("op", "if", true),
            compact_prefix("args"),
            1, // "{"
            compact_prefix("condition"),
            expression_bytes(condition),
            1, // ","
            compact_prefix("then_value"),
            expression_bytes(then_value),
            1, // ","
            compact_prefix("else_value"),
            expression_bytes(else_value),
            2, // "}}"
        ]),
        Expression::IfError { value, fallback } => total([
            1,
            compact_string("op", "if_error", true),
            compact_prefix("args"),
            1,
            compact_prefix("value"),
            expression_bytes(value),
            1,
            compact_prefix("fallback"),
            expression_bytes(fallback),
            2,
        ]),
        Expression::And(expressions) | Expression::Or(expressions) => {
            let operator = if matches!(expression, Expression::And(_)) {
                "and"
            } else {
                "or"
            };
            let count = expressions.len();
            total([
                1, // "{"
                compact_string("op", operator, true),
                compact_prefix("args"),
                1, // "["
                expressions
                    .iter()
                    .enumerate()
                    .fold(0_u64, |sum, (index, expression)| {
                        sum.saturating_add(total([
                            expression_bytes(expression),
                            flag(index + 1 != count),
                        ]))
                    }),
                2, // "]}"
            ])
        }
        Expression::Not(value) => total([
            1,
            compact_string("op", "not", true),
            compact_prefix("args"),
            expression_bytes(value),
            1,
        ]),
        Expression::Abs(value) => total([
            1,
            compact_string("op", "abs", true),
            compact_prefix("args"),
            expression_bytes(value),
            1,
        ]),
        Expression::Round { value, digits } => total([
            1, // "{"
            compact_string("op", "round", true),
            compact_prefix("args"),
            1, // "{"
            compact_prefix("value"),
            expression_bytes(value),
            1, // ","
            compact_literal("digits", number_text_bytes(f64::from(*digits)), false),
            2, // "}}"
        ]),
        Expression::RoundUp { value, digits } => total([
            1, // "{"
            compact_string("op", "round_up", true),
            compact_prefix("args"),
            1, // "{"
            compact_prefix("value"),
            expression_bytes(value),
            1, // ","
            compact_literal("digits", number_text_bytes(f64::from(*digits)), false),
            2, // "}}"
        ]),
        Expression::RoundDown { value, digits } => total([
            1, // "{"
            compact_string("op", "round_down", true),
            compact_prefix("args"),
            1, // "{"
            compact_prefix("value"),
            expression_bytes(value),
            1, // ","
            compact_literal("digits", number_text_bytes(f64::from(*digits)), false),
            2, // "}}"
        ]),
        Expression::IsBlank(value) => total([
            1,
            compact_string("op", "is_blank", true),
            compact_prefix("args"),
            expression_bytes(value),
            1,
        ]),
    }
}

#[cfg(test)]
mod iferror_tests {
    use super::expression_bytes;
    use tachiko_storage::to_canonical_string;
    use tachiko_workspace_engine::{
        Document, Entity, Expression, FieldConstraint, FieldDefinition, FieldType, Number, Schema,
        Value,
    };

    #[test]
    fn iferror_size_matches_the_canonical_expression() {
        let expression = Expression::IfError {
            value: Box::new(Expression::Number(Number::new(1.0).unwrap())),
            fallback: Box::new(Expression::Number(Number::new(2.0).unwrap())),
        };
        let mut document = Document::empty("size", "Size");
        document.schemas.insert(
            "schema".into(),
            Schema {
                id: "schema".into(),
                key: "schema".into(),
                display_name: None,
                field_order: None,
                fields: [(
                    "result".into(),
                    FieldDefinition {
                        id: "result".into(),
                        key: "result".into(),
                        display_name: None,
                        field_type: FieldType::Number,
                        required: true,
                        constraint: FieldConstraint::None,
                        number_format: None,
                        date_format: None,
                    },
                )]
                .into(),
            },
        );
        document.entities.insert(
            "row".into(),
            Entity {
                id: "row".into(),
                key: "row".into(),
                schema: "schema".into(),
                fields: [("result".into(), Value::Formula(expression.clone()))].into(),
            },
        );
        let canonical: serde_json::Value =
            serde_json::from_str(&to_canonical_string(&document).unwrap()).unwrap();
        let written =
            serde_json::to_vec(&canonical["entities"]["row"]["fields"]["result"]["value"]).unwrap();
        assert_eq!(
            expression_bytes(&expression),
            u64::try_from(written.len()).unwrap()
        );
    }
}
