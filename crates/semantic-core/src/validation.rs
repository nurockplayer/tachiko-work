use std::collections::BTreeMap;

use crate::{
    Diagnostic, DiagnosticCode, DiagnosticFact, DiagnosticProvider, DiagnosticSeverity, Document,
    Expression, FieldRef, FieldType, Schema, SemanticSubject, Value,
};

const CORE_PROVIDER: DiagnosticProvider = DiagnosticProvider::new("tachiko.semantic-core");

impl DiagnosticCode {
    pub const EMPTY_STABLE_ID: Self = Self::new("core.empty_stable_id");
    pub const EMPTY_KEY: Self = Self::new("core.empty_key");
    pub const INVALID_KEY: Self = Self::new("core.invalid_key");
    pub const DUPLICATE_KEY: Self = Self::new("core.duplicate_key");
    pub const EMPTY_TITLE: Self = Self::new("core.empty_title");
    pub const KEY_MISMATCH: Self = Self::new("core.key_mismatch");
    pub const MISSING_SCHEMA: Self = Self::new("core.missing_schema");
    pub const MISSING_REQUIRED_FIELD: Self = Self::new("core.missing_required_field");
    pub const UNEXPECTED_FIELD: Self = Self::new("core.unexpected_field");
    pub const TYPE_MISMATCH: Self = Self::new("core.type_mismatch");
    pub const MISSING_REFERENCE: Self = Self::new("core.missing_reference");
    pub const REFERENCE_TYPE_MISMATCH: Self = Self::new("core.reference_type_mismatch");
    pub const MISSING_FORMULA_REFERENCE: Self = Self::new("core.missing_formula_reference");
    pub const FORMULA_REFERENCE_TYPE_MISMATCH: Self =
        Self::new("core.formula_reference_type_mismatch");
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

fn core_diagnostic(
    path: impl Into<String>,
    code: DiagnosticCode,
    message: impl Into<String>,
    subjects: Vec<SemanticSubject>,
) -> Diagnostic {
    Diagnostic::new(code, DiagnosticSeverity::Error, subjects, CORE_PROVIDER)
        .with_presentation(path, message)
}

#[must_use]
pub fn validate_document(document: &Document) -> Vec<Diagnostic> {
    validate_document_internal(document, true)
}

/// Validate rules owned by semantic-core without duplicating formula-engine's
/// authoritative formula-reference and calculation oracle.
///
/// The legacy [`validate_document`] entry point retains formula reference
/// compatibility for storage and other low-level consumers.
#[must_use]
pub fn validate_document_core(document: &Document) -> Vec<Diagnostic> {
    validate_document_internal(document, false)
}

fn validate_document_internal(
    document: &Document,
    include_formula_references: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    validate_stable_id(
        document.id.as_str(),
        "id",
        "document",
        SemanticSubject::Document(document.id.clone()),
        &mut diagnostics,
    );
    if document.title.trim().is_empty() {
        diagnostics.push(core_diagnostic(
            "title",
            DiagnosticCode::EMPTY_TITLE,
            "document title must not be empty",
            vec![SemanticSubject::Document(document.id.clone())],
        ));
    }

    validate_schema_keys(document, &mut diagnostics);
    validate_entity_keys(document, &mut diagnostics);
    validate_schemas(document, &mut diagnostics);
    validate_entities(document, include_formula_references, &mut diagnostics);

    diagnostics.sort();
    diagnostics
}

fn validate_schemas(document: &Document, diagnostics: &mut Vec<Diagnostic>) {
    for (schema_id, schema) in &document.schemas {
        let schema_path = format!("schemas.{schema_id}");
        if schema_id != &schema.id {
            diagnostics.push(core_diagnostic(
                format!("{schema_path}.id"),
                DiagnosticCode::KEY_MISMATCH,
                format!(
                    "schema store key '{schema_id}' does not match stable id '{}'",
                    schema.id
                ),
                vec![
                    SemanticSubject::Schema(schema_id.clone()),
                    SemanticSubject::Schema(schema.id.clone()),
                ],
            ));
        }
        validate_stable_id(
            schema.id.as_str(),
            &format!("{schema_path}.id"),
            "schema",
            SemanticSubject::Schema(schema_id.clone()),
            diagnostics,
        );
        validate_human_key(
            schema.key.as_str(),
            &format!("{schema_path}.key"),
            "schema",
            SemanticSubject::Schema(schema_id.clone()),
            diagnostics,
        );
        validate_field_keys(schema_id, schema, &schema_path, diagnostics);

        for (field_id, definition) in &schema.fields {
            let field_path = format!("{schema_path}.fields.{field_id}");
            let field_subject = SemanticSubject::SchemaField {
                schema: schema_id.clone(),
                field: field_id.clone(),
            };
            if field_id != &definition.id {
                diagnostics.push(core_diagnostic(
                    format!("{field_path}.id"),
                    DiagnosticCode::KEY_MISMATCH,
                    format!(
                        "field store key '{field_id}' does not match stable id '{}'",
                        definition.id
                    ),
                    vec![
                        field_subject.clone(),
                        SemanticSubject::SchemaField {
                            schema: schema_id.clone(),
                            field: definition.id.clone(),
                        },
                    ],
                ));
            }
            validate_stable_id(
                definition.id.as_str(),
                &format!("{field_path}.id"),
                "field",
                field_subject.clone(),
                diagnostics,
            );
            validate_human_key(
                definition.key.as_str(),
                &format!("{field_path}.key"),
                "field",
                field_subject.clone(),
                diagnostics,
            );
            if let FieldType::Reference { schema: target } = &definition.field_type {
                if !document.schemas.contains_key(target) {
                    diagnostics.push(
                        core_diagnostic(
                            format!("{field_path}.field_type.schema"),
                            DiagnosticCode::MISSING_SCHEMA,
                            format!("reference target schema '{target}' does not exist"),
                            vec![field_subject],
                        )
                        .with_related_subjects(vec![SemanticSubject::Schema(target.clone())]),
                    );
                }
            }
        }
    }
}

fn validate_entities(
    document: &Document,
    include_formula_references: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (entity_id, entity) in &document.entities {
        let entity_path = format!("entities.{entity_id}");
        let entity_subject = SemanticSubject::Entity(entity_id.clone());
        if entity_id != &entity.id {
            diagnostics.push(core_diagnostic(
                format!("{entity_path}.id"),
                DiagnosticCode::KEY_MISMATCH,
                format!(
                    "entity store key '{entity_id}' does not match stable id '{}'",
                    entity.id
                ),
                vec![
                    entity_subject.clone(),
                    SemanticSubject::Entity(entity.id.clone()),
                ],
            ));
        }
        validate_stable_id(
            entity.id.as_str(),
            &format!("{entity_path}.id"),
            "entity",
            entity_subject.clone(),
            diagnostics,
        );
        validate_human_key(
            entity.key.as_str(),
            &format!("{entity_path}.key"),
            "entity",
            entity_subject.clone(),
            diagnostics,
        );

        let Some(schema) = document.schemas.get(&entity.schema) else {
            diagnostics.push(
                core_diagnostic(
                    format!("{entity_path}.schema"),
                    DiagnosticCode::MISSING_SCHEMA,
                    format!("schema '{}' does not exist", entity.schema),
                    vec![entity_subject],
                )
                .with_related_subjects(vec![SemanticSubject::Schema(entity.schema.clone())]),
            );
            continue;
        };

        validate_required_fields(schema, entity_id, entity, diagnostics);

        for (field, value) in &entity.fields {
            let field_path = format!("{entity_path}.fields.{field}");
            let field_ref = FieldRef::new(entity_id.clone(), field.clone());
            let Some(definition) = schema.fields.get(field) else {
                diagnostics.push(
                    core_diagnostic(
                        field_path,
                        DiagnosticCode::UNEXPECTED_FIELD,
                        format!("field '{field}' is not declared by schema '{}'", schema.id),
                        vec![SemanticSubject::EntityField(field_ref)],
                    )
                    .with_related_subjects(vec![
                        SemanticSubject::SchemaField {
                            schema: schema.id.clone(),
                            field: field.clone(),
                        },
                    ]),
                );
                continue;
            };
            validate_value(
                document,
                &field_ref,
                value,
                &definition.field_type,
                &field_path,
                include_formula_references,
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
        diagnostics.push(core_diagnostic(
            format!("schema_keys.{key}"),
            DiagnosticCode::DUPLICATE_KEY,
            format!("schema key '{key}' is ambiguous across stable ids {ids:?}"),
            ids.into_iter().map(SemanticSubject::Schema).collect(),
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
        diagnostics.push(core_diagnostic(
            format!("entity_keys.{key}"),
            DiagnosticCode::DUPLICATE_KEY,
            format!("entity key '{key}' is ambiguous across stable ids {ids:?}"),
            ids.into_iter().map(SemanticSubject::Entity).collect(),
        ));
    }
}

fn validate_field_keys(
    schema_id: &crate::SchemaId,
    schema: &Schema,
    schema_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for field in schema.fields.values() {
        groups
            .entry(field.key.clone())
            .or_default()
            .push(field.id.clone());
    }
    for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
        diagnostics.push(core_diagnostic(
            format!("{schema_path}.field_keys.{key}"),
            DiagnosticCode::DUPLICATE_KEY,
            format!("field key '{key}' is ambiguous across stable ids {ids:?}"),
            ids.into_iter()
                .map(|field| SemanticSubject::SchemaField {
                    schema: schema_id.clone(),
                    field,
                })
                .collect(),
        ));
    }
}

fn validate_stable_id(
    value: &str,
    path: &str,
    kind: &str,
    subject: SemanticSubject,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_empty() {
        diagnostics.push(core_diagnostic(
            path,
            DiagnosticCode::EMPTY_STABLE_ID,
            format!("{kind} stable id must not be empty"),
            vec![subject],
        ));
    }
}

fn validate_human_key(
    value: &str,
    path: &str,
    kind: &str,
    subject: SemanticSubject,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.is_empty() {
        diagnostics.push(core_diagnostic(
            path,
            DiagnosticCode::EMPTY_KEY,
            format!("{kind} key must not be empty"),
            vec![subject],
        ));
    } else if !is_valid_identifier(value) {
        diagnostics.push(core_diagnostic(
            path,
            DiagnosticCode::INVALID_KEY,
            format!(
                "{kind} key '{value}' must use only a-z, 0-9, '_' or '-', starting with a letter or digit"
            ),
            vec![subject],
        ));
    }
}

fn validate_required_fields(
    schema: &Schema,
    entity_id: &crate::EntityId,
    entity: &crate::Entity,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (field, definition) in &schema.fields {
        if definition.required && !entity.fields.contains_key(field) {
            diagnostics.push(core_diagnostic(
                format!("entities.{entity_id}.fields.{field}"),
                DiagnosticCode::MISSING_REQUIRED_FIELD,
                format!("required field '{}' is missing", definition.key),
                vec![SemanticSubject::EntityField(FieldRef::new(
                    entity_id.clone(),
                    field.clone(),
                ))],
            ));
        }
    }
}

fn validate_value(
    document: &Document,
    field: &FieldRef,
    value: &Value,
    expected: &FieldType,
    path: &str,
    include_formula_references: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match (expected, value) {
        (FieldType::Number, Value::Number(_))
        | (FieldType::Text, Value::Text(_))
        | (FieldType::Boolean, Value::Boolean(_)) => {}
        (FieldType::Number, Value::Formula(expression)) => {
            if include_formula_references {
                validate_expression(document, field, expression, path, diagnostics);
            }
        }
        (FieldType::Reference { schema }, Value::Reference(entity_id)) => {
            let Some(target) = document.entities.get(entity_id) else {
                diagnostics.push(
                    core_diagnostic(
                        path,
                        DiagnosticCode::MISSING_REFERENCE,
                        format!("referenced entity stable id '{entity_id}' does not exist"),
                        vec![SemanticSubject::EntityField(field.clone())],
                    )
                    .with_related_subjects(vec![SemanticSubject::Entity(entity_id.clone())]),
                );
                return;
            };
            if &target.schema != schema {
                diagnostics.push(
                    core_diagnostic(
                        path,
                        DiagnosticCode::REFERENCE_TYPE_MISMATCH,
                        format!(
                            "field '{field}' expects schema '{schema}', but entity '{entity_id}' uses schema '{}'",
                            target.schema
                        ),
                        vec![SemanticSubject::EntityField(field.clone())],
                    )
                    .with_related_subjects(vec![SemanticSubject::Entity(entity_id.clone())])
                    .with_fact(DiagnosticFact::new("expected_schema", schema.as_str()))
                    .with_fact(DiagnosticFact::new("actual_schema", target.schema.as_str())),
                );
            }
        }
        _ => diagnostics.push(
            core_diagnostic(
                path,
                DiagnosticCode::TYPE_MISMATCH,
                format!(
                    "field '{field}' expects {}, but found {}",
                    field_type_name(expected),
                    value_type_name(value)
                ),
                vec![SemanticSubject::EntityField(field.clone())],
            )
            .with_fact(DiagnosticFact::new(
                "expected_kind",
                field_type_name(expected),
            ))
            .with_fact(DiagnosticFact::new("actual_kind", value_type_name(value))),
        ),
    }
}

fn validate_expression(
    document: &Document,
    owner: &FieldRef,
    expression: &Expression,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut stack = vec![expression];
    while let Some(node) = stack.pop() {
        match node {
            Expression::Number(_) => {}
            Expression::Reference(reference) => {
                validate_formula_reference(document, owner, reference, path, diagnostics);
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
    owner: &FieldRef,
    reference: &FieldRef,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entity) = document.entities.get(&reference.entity) else {
        diagnostics.push(
            core_diagnostic(
                path,
                DiagnosticCode::MISSING_FORMULA_REFERENCE,
                format!(
                    "formula target entity stable id '{}' does not exist",
                    reference.entity
                ),
                vec![SemanticSubject::EntityField(owner.clone())],
            )
            .with_related_subjects(vec![SemanticSubject::EntityField(reference.clone())]),
        );
        return;
    };
    let Some(schema) = document.schemas.get(&entity.schema) else {
        diagnostics.push(
            core_diagnostic(
                path,
                DiagnosticCode::MISSING_FORMULA_REFERENCE,
                format!(
                    "formula target schema stable id '{}' does not exist",
                    entity.schema
                ),
                vec![SemanticSubject::EntityField(owner.clone())],
            )
            .with_related_subjects(vec![
                SemanticSubject::EntityField(reference.clone()),
                SemanticSubject::Schema(entity.schema.clone()),
            ]),
        );
        return;
    };
    let Some(definition) = schema.fields.get(&reference.field) else {
        diagnostics.push(
            core_diagnostic(
                path,
                DiagnosticCode::MISSING_FORMULA_REFERENCE,
                format!(
                    "formula target field stable id '{}' does not exist",
                    reference.field
                ),
                vec![SemanticSubject::EntityField(owner.clone())],
            )
            .with_related_subjects(vec![SemanticSubject::EntityField(reference.clone())]),
        );
        return;
    };
    if definition.field_type != FieldType::Number {
        diagnostics.push(
            core_diagnostic(
                path,
                DiagnosticCode::FORMULA_REFERENCE_TYPE_MISMATCH,
                format!("formula reference '{reference}' does not target a numeric field"),
                vec![SemanticSubject::EntityField(owner.clone())],
            )
            .with_related_subjects(vec![SemanticSubject::EntityField(reference.clone())])
            .with_fact(DiagnosticFact::new("expected_kind", "number"))
            .with_fact(DiagnosticFact::new(
                "actual_kind",
                field_type_name(&definition.field_type),
            )),
        );
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
