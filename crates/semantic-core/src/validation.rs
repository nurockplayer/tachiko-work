use serde::{Deserialize, Serialize};

use crate::{Document, Expression, FieldId, FieldRef, FieldType, Schema, Value};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    EmptyIdentifier,
    InvalidIdentifier,
    EmptyTitle,
    KeyMismatch,
    MissingSchema,
    MissingRequiredField,
    UnexpectedField,
    TypeMismatch,
    MissingReference,
    ReferenceTypeMismatch,
    NonFiniteNumber,
    MissingFormulaReference,
    FormulaReferenceTypeMismatch,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub path: String,
    pub code: DiagnosticCode,
    pub message: String,
}

/// Return whether a value follows Tachiko's stable semantic identifier grammar.
///
/// Identifiers are non-empty lowercase ASCII paths. The first character must
/// be a letter or digit; subsequent characters may also contain `_` and `-`.
#[must_use]
pub fn is_valid_identifier(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
}

impl Diagnostic {
    fn new(path: impl Into<String>, code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code,
            message: message.into(),
        }
    }
}

#[must_use]
pub fn validate_document(document: &Document) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    validate_identifier(document.id.as_str(), "id", "document", &mut diagnostics);
    if document.title.trim().is_empty() {
        diagnostics.push(Diagnostic::new(
            "title",
            DiagnosticCode::EmptyTitle,
            "document title must not be empty",
        ));
    }

    for (schema_key, schema) in &document.schemas {
        let schema_path = format!("schemas.{schema_key}");
        if schema_key != &schema.id {
            diagnostics.push(Diagnostic::new(
                format!("{schema_path}.id"),
                DiagnosticCode::KeyMismatch,
                format!(
                    "schema map key '{schema_key}' does not match id '{}'",
                    schema.id
                ),
            ));
        }
        validate_identifier(
            schema.id.as_str(),
            &format!("{schema_path}.id"),
            "schema",
            &mut diagnostics,
        );
        for field in schema.fields.keys() {
            validate_identifier(
                field.as_str(),
                &format!("{schema_path}.fields.{field}"),
                "field",
                &mut diagnostics,
            );
        }
    }

    for (entity_key, entity) in &document.entities {
        let entity_path = format!("entities.{entity_key}");
        if entity_key != &entity.id {
            diagnostics.push(Diagnostic::new(
                format!("{entity_path}.id"),
                DiagnosticCode::KeyMismatch,
                format!(
                    "entity map key '{entity_key}' does not match id '{}'",
                    entity.id
                ),
            ));
        }
        validate_identifier(
            entity.id.as_str(),
            &format!("{entity_path}.id"),
            "entity",
            &mut diagnostics,
        );

        let Some(schema) = document.schemas.get(&entity.schema) else {
            diagnostics.push(Diagnostic::new(
                format!("{entity_path}.schema"),
                DiagnosticCode::MissingSchema,
                format!("schema '{}' does not exist", entity.schema),
            ));
            continue;
        };

        validate_required_fields(schema, entity_key.as_str(), entity, &mut diagnostics);

        for (field, value) in &entity.fields {
            let field_path = format!("{entity_path}.fields.{field}");
            let Some(definition) = schema.fields.get(field) else {
                diagnostics.push(Diagnostic::new(
                    field_path,
                    DiagnosticCode::UnexpectedField,
                    format!("field '{field}' is not declared by schema '{}'", schema.id),
                ));
                continue;
            };
            validate_value(
                document,
                field,
                value,
                &definition.field_type,
                &field_path,
                &mut diagnostics,
            );
        }
    }

    diagnostics.sort();
    diagnostics
}

fn validate_identifier(
    identifier: &str,
    path: &str,
    kind: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if identifier.is_empty() {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::EmptyIdentifier,
            format!("{kind} id must not be empty"),
        ));
        return;
    }

    if !is_valid_identifier(identifier) {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::InvalidIdentifier,
            format!(
                "{kind} id '{identifier}' must be a lowercase identifier using only a-z, 0-9, '_' or '-', starting with a letter or digit"
            ),
        ));
    }
}

fn validate_required_fields(
    schema: &Schema,
    entity_key: &str,
    entity: &crate::Entity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (field, definition) in &schema.fields {
        if definition.required && !entity.fields.contains_key(field) {
            diagnostics.push(Diagnostic::new(
                format!("entities.{entity_key}.fields.{field}"),
                DiagnosticCode::MissingRequiredField,
                format!("required field '{field}' is missing"),
            ));
        }
    }
}

fn validate_value(
    document: &Document,
    field: &FieldId,
    value: &Value,
    expected: &FieldType,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match (expected, value) {
        (FieldType::Number, Value::Number(number)) => {
            validate_finite(*number, path, diagnostics);
        }
        (FieldType::Number, Value::Formula(expression)) => {
            validate_expression(document, expression, path, diagnostics);
        }
        (FieldType::Text, Value::Text(_)) | (FieldType::Boolean, Value::Boolean(_)) => {}
        (FieldType::Reference { schema }, Value::Reference(entity_id)) => {
            let Some(target) = document.entities.get(entity_id) else {
                diagnostics.push(Diagnostic::new(
                    path,
                    DiagnosticCode::MissingReference,
                    format!("referenced entity '{entity_id}' does not exist"),
                ));
                return;
            };
            if &target.schema != schema {
                diagnostics.push(Diagnostic::new(
                    path,
                    DiagnosticCode::ReferenceTypeMismatch,
                    format!(
                        "field '{field}' expects a reference to schema '{schema}', but '{entity_id}' uses schema '{}'",
                        target.schema
                    ),
                ));
            }
        }
        _ => diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::TypeMismatch,
            format!(
                "field '{field}' expects {}, but found {}",
                field_type_name(expected),
                value_type_name(value)
            ),
        )),
    }
}

fn validate_expression(
    document: &Document,
    expression: &Expression,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expression {
        Expression::Number(number) => validate_finite(*number, path, diagnostics),
        Expression::Reference(reference) => {
            validate_formula_reference(document, reference, path, diagnostics);
        }
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right }
        | Expression::Divide { left, right }
        | Expression::Minimum { left, right }
        | Expression::Maximum { left, right } => {
            validate_expression(document, left, path, diagnostics);
            validate_expression(document, right, path, diagnostics);
        }
    }
}

fn validate_formula_reference(
    document: &Document,
    reference: &FieldRef,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entity) = document.entities.get(&reference.entity) else {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::MissingFormulaReference,
            format!("formula reference '{reference}' has no target entity"),
        ));
        return;
    };
    let Some(schema) = document.schemas.get(&entity.schema) else {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::MissingFormulaReference,
            format!("formula reference '{reference}' has no target schema"),
        ));
        return;
    };
    let Some(definition) = schema.fields.get(&reference.field) else {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::MissingFormulaReference,
            format!("formula reference '{reference}' has no target field"),
        ));
        return;
    };
    if definition.field_type != FieldType::Number {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::FormulaReferenceTypeMismatch,
            format!("formula reference '{reference}' does not target a numeric field"),
        ));
    }
}

fn validate_finite(number: f64, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !number.is_finite() {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::NonFiniteNumber,
            "numeric values must be finite",
        ));
    }
}

fn field_type_name(field_type: &FieldType) -> &'static str {
    match field_type {
        FieldType::Number => "number",
        FieldType::Text => "text",
        FieldType::Boolean => "boolean",
        FieldType::Reference { .. } => "reference",
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Number(_) => "number",
        Value::Text(_) => "text",
        Value::Boolean(_) => "boolean",
        Value::Reference(_) => "reference",
        Value::Formula(_) => "formula",
    }
}
