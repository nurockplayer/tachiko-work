//! Upper bound on the bytes one document's v3 save writes.
//!
//! The v3 codec stores JSON, so raw document bytes are not the saved size:
//! control characters escape to six bytes and identities repeat per cell and
//! per formula reference. This module mirrors the writers in
//! `crates/storage/src/roproj/v3.rs`, `crates/storage/src/number_format.rs`
//! and the framing of the app-private bundle and the portable package, and
//! counts the result without allocating a string per value.

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
const EXPORT_FRAMING_BYTES: usize = 2_880;

/// Bytes `export_project_v3` can write for this document, never less than it
/// does. It also covers `export_portable_ro`, whose framing is larger.
///
/// Callers add [`presentation_saved_bytes`]; presentation is not part of
/// `Document`.
pub(crate) fn saved_bytes_upper_bound(document: &Document) -> usize {
    #[cfg(test)]
    record_bound_call();
    manifest_bytes(document)
        .saturating_add(schemas_bytes(document))
        .saturating_add(definitions_bytes(document))
        .saturating_add(entities_bytes(document))
        .saturating_add(EXPORT_FRAMING_BYTES)
}

/// Bytes the optional `presentation.jsonl` member adds to a save.
pub(crate) fn presentation_saved_bytes(store: &PresentationStore) -> Result<usize, DesignerError> {
    Ok(encode_presentation(store)?.map_or(0, |bytes| bytes.len()))
}

#[cfg(test)]
thread_local! {
    static SAVED_BOUND_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_bound_call() {
    SAVED_BOUND_CALLS.with(|calls| calls.set(calls.get() + 1));
}

/// How many times this test thread computed the bound, to prove that ordinary
/// profiles never do.
#[cfg(test)]
pub(crate) fn saved_bound_calls() -> usize {
    SAVED_BOUND_CALLS.with(std::cell::Cell::get)
}

/// `serde_json`'s exact escaped length, without the surrounding quotes.
///
/// `"` and `\` are two bytes, `\b \t \n \f \r` are two, every other byte below
/// `0x20` is six, and every other byte is one. Non-ASCII bytes are never
/// escaped.
fn escaped_bytes(value: &str) -> usize {
    value
        .bytes()
        .map(|byte| match byte {
            b'"' | b'\\' | 0x08 | 0x09 | 0x0A | 0x0C | 0x0D => 2,
            0x00..=0x1F => 6,
            _ => 1,
        })
        .sum()
}

fn string_bytes(value: &str) -> usize {
    escaped_bytes(value) + 2
}

fn indent_bytes(indent: usize) -> usize {
    indent * 2
}

fn pretty_prefix(indent: usize, name: &str) -> usize {
    indent_bytes(indent) + string_bytes(name) + 2
}

fn pretty_string(indent: usize, name: &str, value: &str, comma: bool) -> usize {
    pretty_prefix(indent, name) + string_bytes(value) + usize::from(comma) + 1
}

fn pretty_literal(indent: usize, name: &str, value_bytes: usize, comma: bool) -> usize {
    pretty_prefix(indent, name) + value_bytes + usize::from(comma) + 1
}

/// Bytes `ryu_js` writes for one number, the formatter `v3.rs` uses. `Number`
/// already rejects non-finite values and normalizes either zero sign, which is
/// exactly the writer's normalization.
fn number_bytes(number: Number) -> usize {
    number_text_bytes(number.get())
}

/// The writer's `format_finite` call on a value that is already a number
/// parameter, such as `round` digits.
fn number_text_bytes(value: f64) -> usize {
    let mut buffer = ryu_js::Buffer::new();
    buffer
        .format_finite(if value == 0.0 { 0.0 } else { value })
        .len()
}

fn pretty_number(indent: usize, name: &str, number: Number, comma: bool) -> usize {
    pretty_prefix(indent, name) + number_bytes(number) + usize::from(comma) + 1
}

fn compact_prefix(name: &str) -> usize {
    string_bytes(name) + 1
}

fn compact_string(name: &str, value: &str, comma: bool) -> usize {
    compact_prefix(name) + string_bytes(value) + usize::from(comma)
}

fn compact_literal(name: &str, value_bytes: usize, comma: bool) -> usize {
    compact_prefix(name) + value_bytes + usize::from(comma)
}

fn compact_number(name: &str, number: Number, comma: bool) -> usize {
    compact_prefix(name) + number_bytes(number) + usize::from(comma)
}

fn manifest_bytes(document: &Document) -> usize {
    2 // "{\n"
        + pretty_string(1, "format", "tachiko.roproj", true)
        + pretty_literal(1, "format_version", 1, true)
        + pretty_prefix(1, "document")
        + 2 // "{\n"
        + pretty_string(2, "id", document.id.as_str(), true)
        + pretty_string(2, "title", &document.title, false)
        + indent_bytes(1)
        + 2 // "}\n"
        + 2 // "}\n"
}

fn schemas_bytes(document: &Document) -> usize {
    let count = document.schemas.len();
    if count == 0 {
        return 3; // "[]\n"
    }
    let entries = document
        .schemas
        .values()
        .enumerate()
        .map(|(index, schema)| schema_bytes(schema) + 1 + usize::from(index + 1 != count))
        .sum::<usize>();
    2 // "[\n"
        + entries
        + 2 // "]\n"
}

fn schema_bytes(schema: &Schema) -> usize {
    let indent = 1;
    let field_count = schema.fields.len();
    indent_bytes(indent)
        + 2 // "{\n"
        + pretty_string(indent + 1, "id", schema.id.as_str(), true)
        + pretty_string(indent + 1, "key", schema.key.as_str(), true)
        + schema.display_name.as_ref().map_or(0, |name| {
            pretty_string(indent + 1, "display_name", name, true)
        })
        + schema.field_order.as_ref().map_or(0, |order| {
            pretty_prefix(indent + 1, "field_order")
                + 1 // "["
                + order.iter().map(|id| string_bytes(id.as_str())).sum::<usize>()
                + 2 * order.len().saturating_sub(1)
                + 3 // "],\n"
        })
        + pretty_prefix(indent + 1, "fields")
        + if schema.fields.is_empty() {
            3 // "[]\n"
        } else {
            let entries = schema
                .fields
                .values()
                .enumerate()
                .map(|(index, field)| {
                    field_bytes(field, indent + 2) + 1 + usize::from(index + 1 != field_count)
                })
                .sum::<usize>();
            2 // "[\n"
                + entries
                + indent_bytes(indent + 1)
                + 2 // "]\n"
        }
        + indent_bytes(indent)
        + 1 // "}"
}

fn field_bytes(field: &FieldDefinition, indent: usize) -> usize {
    indent_bytes(indent)
        + 2 // "{\n"
        + pretty_string(indent + 1, "id", field.id.as_str(), true)
        + pretty_string(indent + 1, "key", field.key.as_str(), true)
        + field.display_name.as_ref().map_or(0, |name| {
            pretty_string(indent + 1, "display_name", name, true)
        })
        + pretty_prefix(indent + 1, "field_type")
        + field_type_bytes(&field.field_type, indent + 1)
        + 2 // ",\n"
        + pretty_literal(
            indent + 1,
            "required",
            if field.required { 4 } else { 5 },
            true,
        )
        + pretty_prefix(indent + 1, "constraint")
        + constraint_bytes(&field.constraint, indent + 1)
        + field.number_format.as_ref().map_or(0, |format| {
            2 // ",\n"
                + pretty_prefix(indent + 1, "number_format")
                + number_format_bytes(format, indent + 1)
        })
        + 1 // final '\n'
        + indent_bytes(indent)
        + 1 // "}"
}

fn field_type_bytes(field_type: &FieldType, indent: usize) -> usize {
    2 // "{\n"
        + match field_type {
            FieldType::Number => pretty_string(indent + 1, "type", "number", false),
            FieldType::Text => pretty_string(indent + 1, "type", "text", false),
            FieldType::Boolean => pretty_string(indent + 1, "type", "boolean", false),
            FieldType::Date => pretty_string(indent + 1, "type", "date", false),
            FieldType::Reference { schema } => {
                pretty_string(indent + 1, "type", "reference", true)
                    + pretty_string(indent + 1, "schema", schema.as_str(), false)
            }
        }
        + indent_bytes(indent)
        + 1 // "}"
}

fn constraint_bytes(constraint: &FieldConstraint, indent: usize) -> usize {
    2 // "{\n"
        + match constraint {
            FieldConstraint::None => pretty_string(indent + 1, "type", "none", false),
            FieldConstraint::TextLiteralSet { values } => {
                pretty_string(indent + 1, "type", "text_literal_set", true)
                    + pretty_prefix(indent + 1, "values")
                    + if values.is_empty() {
                        3 // "[]\n"
                    } else {
                        let count = values.len();
                        let entries = values
                            .iter()
                            .enumerate()
                            .map(|(index, value)| {
                                indent_bytes(indent + 2)
                                    + string_bytes(value)
                                    + usize::from(index + 1 != count)
                                    + 1
                            })
                            .sum::<usize>();
                        2 // "[\n"
                            + entries
                            + indent_bytes(indent + 1)
                            + 2 // "]\n"
                    }
            }
            FieldConstraint::NumberInclusiveRange { min, max } => {
                pretty_string(indent + 1, "type", "number_inclusive_range", true)
                    + pretty_number(indent + 1, "min", *min, true)
                    + pretty_number(indent + 1, "max", *max, false)
            }
        }
        + indent_bytes(indent)
        + 1 // "}"
}

/// Bytes of the pretty JSON `crates/storage/src/number_format.rs` writes. The
/// caller's line prefix already emitted `indent` for the first line and is
/// re-emitted after every newline.
fn number_format_bytes(format: &NumberFormat, indent: usize) -> usize {
    let line = 1 + indent_bytes(indent);
    1 // "{"
        + line
        + 2
        + string_bytes("decimals")
        + 2
        + decimal_digits(format.decimals)
        + 1
        + line
        + 2
        + string_bytes("kind")
        + 2
        + 1 // "{"
        + match &format.kind {
            NumberFormatKind::Plain => {
                line + 4 + string_bytes("type") + 2 + string_bytes("plain")
            }
            NumberFormatKind::Percent => {
                line + 4 + string_bytes("type") + 2 + string_bytes("percent")
            }
            NumberFormatKind::Currency { code } => {
                line + 4 + string_bytes("type") + 2 + string_bytes("currency") + 1
                    + line
                    + 4
                    + string_bytes("code")
                    + 2
                    + string_bytes(code)
            }
        }
        + line
        + 4 // "  },"
        + line
        + 2
        + string_bytes("thousands_separator")
        + 2
        + if format.thousands_separator { 4 } else { 5 }
        + line
        + 1 // "}"
}

fn decimal_digits(value: u8) -> usize {
    if value >= 100 {
        3
    } else if value >= 10 {
        2
    } else {
        1
    }
}

fn definitions_bytes(document: &Document) -> usize {
    let count = document.keyed_grouped_sum_definitions.len();
    if count == 0 {
        return 3; // "[]\n"
    }
    let entries = document
        .keyed_grouped_sum_definitions
        .values()
        .enumerate()
        .map(|(index, definition)| {
            definition_bytes(definition) + 1 + usize::from(index + 1 != count)
        })
        .sum::<usize>();
    2 // "[\n"
        + entries
        + 2 // "]\n"
}

fn definition_bytes(definition: &KeyedGroupedSumDefinition) -> usize {
    let indent = 1;
    indent_bytes(indent)
        + 2 // "{\n"
        + pretty_string(indent + 1, "id", definition.id.as_str(), true)
        + pretty_prefix(indent + 1, "orders")
        + 2 // "{\n"
        + pretty_string(indent + 2, "schema", definition.orders.schema.as_str(), true)
        + pretty_string(
            indent + 2,
            "lookup_key_field",
            definition.orders.lookup_key_field.as_str(),
            true,
        )
        + pretty_string(
            indent + 2,
            "quantity_field",
            definition.orders.quantity_field.as_str(),
            false,
        )
        + indent_bytes(indent + 1)
        + 3 // "},\n"
        + pretty_prefix(indent + 1, "products")
        + 2 // "{\n"
        + pretty_string(indent + 2, "schema", definition.products.schema.as_str(), true)
        + pretty_string(
            indent + 2,
            "key_field",
            definition.products.key_field.as_str(),
            true,
        )
        + pretty_string(
            indent + 2,
            "category_field",
            definition.products.category_field.as_str(),
            true,
        )
        + pretty_string(
            indent + 2,
            "price_field",
            definition.products.price_field.as_str(),
            false,
        )
        + indent_bytes(indent + 1)
        + 2 // "}\n"
        + indent_bytes(indent)
        + 1 // "}"
}

fn entities_bytes(document: &Document) -> usize {
    document
        .entities
        .values()
        .map(|entity| entity_bytes(entity) + 1)
        .sum()
}

fn entity_bytes(entity: &Entity) -> usize {
    let field_count = entity.fields.len();
    let entries = entity
        .fields
        .iter()
        .enumerate()
        .map(|(index, (field, value))| {
            string_bytes(field.as_str())
                + 1
                + value_bytes(value)
                + usize::from(index + 1 != field_count)
        })
        .sum::<usize>();
    1 // "{"
        + compact_string("id", entity.id.as_str(), true)
        + compact_string("key", entity.key.as_str(), true)
        + compact_string("schema", entity.schema.as_str(), true)
        + compact_prefix("fields")
        + 1 // "{"
        + entries
        + 2 // "}}"
}

fn value_bytes(value: &Value) -> usize {
    match value {
        Value::Number(number) => {
            1 + compact_string("kind", "number", true) + compact_number("value", *number, false) + 1
        }
        Value::Text(text) => {
            1 + compact_string("kind", "text", true) + compact_string("value", text, false) + 1
        }
        Value::Boolean(boolean) => {
            1 + compact_string("kind", "boolean", true)
                + compact_literal("value", if *boolean { 4 } else { 5 }, false)
                + 1
        }
        Value::Date(_) => {
            1 + compact_string("kind", "date", true) + compact_literal("value", 12, false) + 1
        }
        Value::Reference(entity) => {
            1 + compact_string("kind", "reference", true)
                + compact_string("value", entity.as_str(), false)
                + 1
        }
        Value::Formula(expression) => {
            1 + compact_string("kind", "formula", true)
                + compact_prefix("value")
                + expression_bytes(expression)
                + 1
        }
    }
}

fn column_bytes(column: &ColumnRef) -> usize {
    1 // "{"
        + compact_string("schema", column.schema.as_str(), true)
        + compact_string("field", column.field.as_str(), false)
        + 1 // "}"
}

fn binary_bytes(operator: &str, left: &Expression, right: &Expression) -> usize {
    1 // "{"
        + compact_string("op", operator, true)
        + compact_prefix("args")
        + 1 // "{"
        + compact_prefix("left")
        + expression_bytes(left)
        + 1 // ","
        + compact_prefix("right")
        + expression_bytes(right)
        + 2 // "}}"
}

fn compare_bytes(operator: ComparisonOperator, left: &Expression, right: &Expression) -> usize {
    1 // "{"
        + compact_string("op", "compare", true)
        + compact_prefix("args")
        + 1 // "{"
        + compact_string("operator", operator.as_str(), true)
        + compact_prefix("left")
        + expression_bytes(left)
        + 1 // ","
        + compact_prefix("right")
        + expression_bytes(right)
        + 2 // "}}"
}

#[allow(clippy::too_many_lines)] // One arm per closed-world expression variant keeps the mirror auditable.
fn expression_bytes(expression: &Expression) -> usize {
    match expression {
        Expression::Number(number) => {
            1 + compact_string("op", "number", true) + compact_number("args", *number, false) + 1
        }
        Expression::Reference(reference) => {
            1 + compact_string("op", "reference", true)
                + compact_prefix("args")
                + 1 // "{"
                + compact_string("entity", reference.entity.as_str(), true)
                + compact_string("field", reference.field.as_str(), false)
                + 2 // "}}"
        }
        Expression::SameRowReference(field) => {
            1 + compact_string("op", "same_row_reference", true)
                + compact_string("args", field.as_str(), false)
                + 1
        }
        Expression::ColumnTotal(column) => {
            1 + compact_string("op", "column_total", true)
                + compact_prefix("args")
                + column_bytes(column)
                + 1
        }
        Expression::ColumnAggregate { function, column } => {
            1 + compact_string("op", "column_aggregate", true)
                + compact_prefix("args")
                + 1 // "{"
                + compact_string("function", function.wire_name(), true)
                + compact_prefix("column")
                + column_bytes(column)
                + 2 // "}}"
        }
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
            let criteria_bytes = criteria
                .iter()
                .enumerate()
                .map(|(index, criterion)| {
                    let value = criterion.value.as_ref().map_or(0, |value| {
                        1 // ","
                            + compact_prefix("value")
                            + expression_bytes(value)
                    });
                    1 // "{"
                        + compact_prefix("column")
                        + column_bytes(&criterion.column)
                        + 1 // ","
                        + compact_string("operator", criterion.operator.as_str(), false)
                        + value
                        + 1 // "}"
                        + usize::from(index + 1 != count)
                })
                .sum::<usize>();
            1 // "{"
                + compact_string("op", "column_conditional", true)
                + compact_prefix("args")
                + 1 // "{"
                + compact_string("aggregate", aggregate, true)
                + values.map_or(0, |column| {
                    compact_prefix("values") + column_bytes(column) + 1 // ","
                })
                + compact_prefix("criteria")
                + 1 // "["
                + criteria_bytes
                + 1 // "]"
                + 2 // "}}"
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
        } => {
            1 // "{"
                + compact_string("op", "if", true)
                + compact_prefix("args")
                + 1 // "{"
                + compact_prefix("condition")
                + expression_bytes(condition)
                + 1 // ","
                + compact_prefix("then_value")
                + expression_bytes(then_value)
                + 1 // ","
                + compact_prefix("else_value")
                + expression_bytes(else_value)
                + 2 // "}}"
        }
        Expression::And(expressions) | Expression::Or(expressions) => {
            let operator = if matches!(expression, Expression::And(_)) {
                "and"
            } else {
                "or"
            };
            let count = expressions.len();
            1 // "{"
                + compact_string("op", operator, true)
                + compact_prefix("args")
                + 1 // "["
                + expressions
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| {
                        expression_bytes(expression) + usize::from(index + 1 != count)
                    })
                    .sum::<usize>()
                + 2 // "]}"
        }
        Expression::Not(value) => {
            1 + compact_string("op", "not", true)
                + compact_prefix("args")
                + expression_bytes(value)
                + 1
        }
        Expression::Round { value, digits } => {
            1 // "{"
                + compact_string("op", "round", true)
                + compact_prefix("args")
                + 1 // "{"
                + compact_prefix("value")
                + expression_bytes(value)
                + 1 // ","
                + compact_literal("digits", number_text_bytes(f64::from(*digits)), false)
                + 2 // "}}"
        }
        Expression::IsBlank(value) => {
            1 + compact_string("op", "is_blank", true)
                + compact_prefix("args")
                + expression_bytes(value)
                + 1
        }
    }
}
