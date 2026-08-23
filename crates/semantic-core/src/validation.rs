use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Document, Expression, FieldId, FieldRef, FieldType, Schema, Value};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    EmptyStableId,
    EmptyKey,
    InvalidKey,
    DuplicateKey,
    EmptyTitle,
    KeyMismatch,
    MissingSchema,
    MissingRequiredField,
    UnexpectedField,
    TypeMismatch,
    MissingReference,
    ReferenceTypeMismatch,
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

/// Return whether a human-facing semantic key follows the authoring grammar.
///
/// Keys are non-empty lowercase ASCII paths. The first character must be a
/// letter or digit; subsequent characters may also contain `_` and `-`.
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

    validate_stable_id(document.id.as_str(), "id", "document", &mut diagnostics);
    if document.title.trim().is_empty() {
        diagnostics.push(Diagnostic::new(
            "title",
            DiagnosticCode::EmptyTitle,
            "document title must not be empty",
        ));
    }

    validate_schema_keys(document, &mut diagnostics);
    validate_entity_keys(document, &mut diagnostics);
    validate_schemas(document, &mut diagnostics);
    validate_entities(document, &mut diagnostics);

    diagnostics.sort();
    diagnostics
}

fn validate_schemas(document: &Document, diagnostics: &mut Vec<Diagnostic>) {
    for (schema_id, schema) in &document.schemas {
        let schema_path = format!("schemas.{schema_id}");
        if schema_id != &schema.id {
            diagnostics.push(Diagnostic::new(
                format!("{schema_path}.id"),
                DiagnosticCode::KeyMismatch,
                format!(
                    "schema store key '{schema_id}' does not match stable id '{}'",
                    schema.id
                ),
            ));
        }
        validate_stable_id(
            schema.id.as_str(),
            &format!("{schema_path}.id"),
            "schema",
            diagnostics,
        );
        validate_human_key(
            schema.key.as_str(),
            &format!("{schema_path}.key"),
            "schema",
            diagnostics,
        );
        validate_field_keys(schema, &schema_path, diagnostics);

        for (field_id, definition) in &schema.fields {
            let field_path = format!("{schema_path}.fields.{field_id}");
            if field_id != &definition.id {
                diagnostics.push(Diagnostic::new(
                    format!("{field_path}.id"),
                    DiagnosticCode::KeyMismatch,
                    format!(
                        "field store key '{field_id}' does not match stable id '{}'",
                        definition.id
                    ),
                ));
            }
            validate_stable_id(
                definition.id.as_str(),
                &format!("{field_path}.id"),
                "field",
                diagnostics,
            );
            validate_human_key(
                definition.key.as_str(),
                &format!("{field_path}.key"),
                "field",
                diagnostics,
            );
            if let FieldType::Reference { schema: target } = &definition.field_type {
                if !document.schemas.contains_key(target) {
                    diagnostics.push(Diagnostic::new(
                        format!("{field_path}.field_type.schema"),
                        DiagnosticCode::MissingSchema,
                        format!("reference target schema '{target}' does not exist"),
                    ));
                }
            }
        }
    }
}

fn validate_entities(document: &Document, diagnostics: &mut Vec<Diagnostic>) {
    for (entity_id, entity) in &document.entities {
        let entity_path = format!("entities.{entity_id}");
        if entity_id != &entity.id {
            diagnostics.push(Diagnostic::new(
                format!("{entity_path}.id"),
                DiagnosticCode::KeyMismatch,
                format!(
                    "entity store key '{entity_id}' does not match stable id '{}'",
                    entity.id
                ),
            ));
        }
        validate_stable_id(
            entity.id.as_str(),
            &format!("{entity_path}.id"),
            "entity",
            diagnostics,
        );
        validate_human_key(
            entity.key.as_str(),
            &format!("{entity_path}.key"),
            "entity",
            diagnostics,
        );

        let Some(schema) = document.schemas.get(&entity.schema) else {
            diagnostics.push(Diagnostic::new(
                format!("{entity_path}.schema"),
                DiagnosticCode::MissingSchema,
                format!("schema '{}' does not exist", entity.schema),
            ));
            continue;
        };

        validate_required_fields(schema, entity_id.as_str(), entity, diagnostics);

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
                diagnostics,
            );
        }
    }
}

fn validate_schema_keys(document: &Document, diagnostics: &mut Vec<Diagnostic>) {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for schema in document.schemas.values() {
        groups
            .entry(schema.key.clone())
            .or_default()
            .push(schema.id.clone());
    }
    for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
        diagnostics.push(Diagnostic::new(
            format!("schema_keys.{key}"),
            DiagnosticCode::DuplicateKey,
            format!("schema key '{key}' is ambiguous across stable ids {ids:?}"),
        ));
    }
}

fn validate_entity_keys(document: &Document, diagnostics: &mut Vec<Diagnostic>) {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for entity in document.entities.values() {
        groups
            .entry(entity.key.clone())
            .or_default()
            .push(entity.id.clone());
    }
    for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
        diagnostics.push(Diagnostic::new(
            format!("entity_keys.{key}"),
            DiagnosticCode::DuplicateKey,
            format!("entity key '{key}' is ambiguous across stable ids {ids:?}"),
        ));
    }
}

fn validate_field_keys(schema: &Schema, schema_path: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for field in schema.fields.values() {
        groups
            .entry(field.key.clone())
            .or_default()
            .push(field.id.clone());
    }
    for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
        diagnostics.push(Diagnostic::new(
            format!("{schema_path}.field_keys.{key}"),
            DiagnosticCode::DuplicateKey,
            format!("field key '{key}' is ambiguous across stable ids {ids:?}"),
        ));
    }
}

fn validate_stable_id(value: &str, path: &str, kind: &str, diagnostics: &mut Vec<Diagnostic>) {
    if value.is_empty() {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::EmptyStableId,
            format!("{kind} stable id must not be empty"),
        ));
    }
}

fn validate_human_key(value: &str, path: &str, kind: &str, diagnostics: &mut Vec<Diagnostic>) {
    if value.is_empty() {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::EmptyKey,
            format!("{kind} key must not be empty"),
        ));
    } else if !is_valid_identifier(value) {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::InvalidKey,
            format!(
                "{kind} key '{value}' must use only a-z, 0-9, '_' or '-', starting with a letter or digit"
            ),
        ));
    }
}

fn validate_required_fields(
    schema: &Schema,
    entity_id: &str,
    entity: &crate::Entity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (field, definition) in &schema.fields {
        if definition.required && !entity.fields.contains_key(field) {
            diagnostics.push(Diagnostic::new(
                format!("entities.{entity_id}.fields.{field}"),
                DiagnosticCode::MissingRequiredField,
                format!("required field '{}' is missing", definition.key),
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
        (FieldType::Number, Value::Number(_))
        | (FieldType::Text, Value::Text(_))
        | (FieldType::Boolean, Value::Boolean(_)) => {}
        (FieldType::Number, Value::Formula(expression)) => {
            validate_expression(document, expression, path, diagnostics);
        }
        (FieldType::Reference { schema }, Value::Reference(entity_id)) => {
            let Some(target) = document.entities.get(entity_id) else {
                diagnostics.push(Diagnostic::new(
                    path,
                    DiagnosticCode::MissingReference,
                    format!("referenced entity stable id '{entity_id}' does not exist"),
                ));
                return;
            };
            if &target.schema != schema {
                diagnostics.push(Diagnostic::new(
                    path,
                    DiagnosticCode::ReferenceTypeMismatch,
                    format!(
                        "field '{field}' expects schema '{schema}', but entity '{entity_id}' uses schema '{}'",
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
    let mut stack = vec![expression];
    while let Some(node) = stack.pop() {
        match node {
            Expression::Number(_) => {}
            Expression::Reference(reference) => {
                validate_formula_reference(document, reference, path, diagnostics);
            }
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                stack.push(right);
                stack.push(left);
            }
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
            format!(
                "formula target entity stable id '{}' does not exist",
                reference.entity
            ),
        ));
        return;
    };
    let Some(schema) = document.schemas.get(&entity.schema) else {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::MissingFormulaReference,
            format!(
                "formula target schema stable id '{}' does not exist",
                entity.schema
            ),
        ));
        return;
    };
    let Some(definition) = schema.fields.get(&reference.field) else {
        diagnostics.push(Diagnostic::new(
            path,
            DiagnosticCode::MissingFormulaReference,
            format!(
                "formula target field stable id '{}' does not exist",
                reference.field
            ),
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
