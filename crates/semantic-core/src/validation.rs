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

fn key_mismatch_diagnostic(
    path: impl Into<String>,
    message: impl Into<String>,
    subjects: Vec<SemanticSubject>,
    store_id: &str,
    declared_id: &str,
) -> Diagnostic {
    core_diagnostic(path, DiagnosticCode::KEY_MISMATCH, message, subjects)
        .with_fact(DiagnosticFact::new("store_id", store_id))
        .with_fact(DiagnosticFact::new("declared_id", declared_id))
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

/// Run the accepted semantic validator with a research-only cancellation poll.
///
/// This Issue #175 probe is compiled only for the research feature. Ordinary
/// validation continues through the production implementation below.
#[cfg(feature = "issue-175-research")]
#[must_use]
pub fn validate_document_cancellable(
    document: &Document,
    cancelled: impl FnMut() -> bool,
) -> Option<Vec<Diagnostic>> {
    issue_175_research::validate_document_cancellable(document, cancelled)
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
            diagnostics.push(key_mismatch_diagnostic(
                format!("{schema_path}.id"),
                format!(
                    "schema store key '{schema_id}' does not match stable id '{}'",
                    schema.id
                ),
                vec![
                    SemanticSubject::Schema(schema_id.clone()),
                    SemanticSubject::Schema(schema.id.clone()),
                ],
                schema_id.as_str(),
                schema.id.as_str(),
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
                diagnostics.push(key_mismatch_diagnostic(
                    format!("{field_path}.id"),
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
                    field_id.as_str(),
                    definition.id.as_str(),
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
            diagnostics.push(key_mismatch_diagnostic(
                format!("{entity_path}.id"),
                format!(
                    "entity store key '{entity_id}' does not match stable id '{}'",
                    entity.id
                ),
                vec![
                    entity_subject.clone(),
                    SemanticSubject::Entity(entity.id.clone()),
                ],
                entity_id.as_str(),
                entity.id.as_str(),
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
    for (schema_id, schema) in &document.schemas {
        groups
            .entry(schema.key.clone())
            .or_default()
            .push(schema_id.clone());
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
    for (entity_id, entity) in &document.entities {
        groups
            .entry(entity.key.clone())
            .or_default()
            .push(entity_id.clone());
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
    for (field_id, field) in &schema.fields {
        groups
            .entry(field.key.clone())
            .or_default()
            .push(field_id.clone());
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

#[cfg(feature = "issue-175-research")]
mod issue_175_research {
    use std::collections::BTreeMap;

    use super::{
        Diagnostic, DiagnosticCode, DiagnosticFact, Document, Expression, FieldRef, FieldType,
        Schema, SemanticSubject, Value, core_diagnostic, field_type_name, is_valid_identifier,
        key_mismatch_diagnostic, validate_formula_reference, value_type_name,
    };

    /// Run the accepted semantic validator with a research-only cancellation poll.
    ///
    /// Returns `None` when `cancelled` requests cancellation. A completed result is
    /// byte-for-byte the same ordered diagnostic set as [`validate_document`].
    #[cfg(feature = "issue-175-research")]
    #[must_use]
    pub(super) fn validate_document_cancellable(
        document: &Document,
        mut cancelled: impl FnMut() -> bool,
    ) -> Option<Vec<Diagnostic>> {
        validate_document_controlled(document, true, &mut cancelled).ok()
    }

    #[derive(Debug)]
    struct ValidationCancelled;

    type ValidationControl = Result<(), ValidationCancelled>;

    fn validate_document_controlled(
        document: &Document,
        include_formula_references: bool,
        cancelled: &mut impl FnMut() -> bool,
    ) -> Result<Vec<Diagnostic>, ValidationCancelled> {
        let mut diagnostics = Vec::new();
        poll_cancellation(cancelled)?;

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

        validate_schema_keys(document, &mut diagnostics, cancelled)?;
        validate_entity_keys(document, &mut diagnostics, cancelled)?;
        validate_schemas(document, &mut diagnostics, cancelled)?;
        validate_entities(
            document,
            include_formula_references,
            &mut diagnostics,
            cancelled,
        )?;

        poll_cancellation(cancelled)?;
        diagnostics.sort();
        Ok(diagnostics)
    }

    fn poll_cancellation(cancelled: &mut impl FnMut() -> bool) -> ValidationControl {
        if cancelled() {
            Err(ValidationCancelled)
        } else {
            Ok(())
        }
    }

    fn validate_schemas(
        document: &Document,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        for (schema_id, schema) in &document.schemas {
            poll_cancellation(cancelled)?;
            let schema_path = format!("schemas.{schema_id}");
            if schema_id != &schema.id {
                diagnostics.push(key_mismatch_diagnostic(
                    format!("{schema_path}.id"),
                    format!(
                        "schema store key '{schema_id}' does not match stable id '{}'",
                        schema.id
                    ),
                    vec![
                        SemanticSubject::Schema(schema_id.clone()),
                        SemanticSubject::Schema(schema.id.clone()),
                    ],
                    schema_id.as_str(),
                    schema.id.as_str(),
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
            validate_field_keys(schema_id, schema, &schema_path, diagnostics, cancelled)?;

            for (field_id, definition) in &schema.fields {
                poll_cancellation(cancelled)?;
                let field_path = format!("{schema_path}.fields.{field_id}");
                let field_subject = SemanticSubject::SchemaField {
                    schema: schema_id.clone(),
                    field: field_id.clone(),
                };
                if field_id != &definition.id {
                    diagnostics.push(key_mismatch_diagnostic(
                        format!("{field_path}.id"),
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
                        field_id.as_str(),
                        definition.id.as_str(),
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
        Ok(())
    }

    fn validate_entities(
        document: &Document,
        include_formula_references: bool,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        for (entity_id, entity) in &document.entities {
            poll_cancellation(cancelled)?;
            let entity_path = format!("entities.{entity_id}");
            let entity_subject = SemanticSubject::Entity(entity_id.clone());
            if entity_id != &entity.id {
                diagnostics.push(key_mismatch_diagnostic(
                    format!("{entity_path}.id"),
                    format!(
                        "entity store key '{entity_id}' does not match stable id '{}'",
                        entity.id
                    ),
                    vec![
                        entity_subject.clone(),
                        SemanticSubject::Entity(entity.id.clone()),
                    ],
                    entity_id.as_str(),
                    entity.id.as_str(),
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

            validate_required_fields(schema, entity_id, entity, diagnostics, cancelled)?;

            for (field, value) in &entity.fields {
                poll_cancellation(cancelled)?;
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
                    cancelled,
                )?;
            }
        }
        Ok(())
    }

    fn validate_schema_keys(
        document: &Document,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for (schema_id, schema) in &document.schemas {
            poll_cancellation(cancelled)?;
            groups
                .entry(schema.key.clone())
                .or_default()
                .push(schema_id.clone());
        }
        for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
            poll_cancellation(cancelled)?;
            diagnostics.push(core_diagnostic(
                format!("schema_keys.{key}"),
                DiagnosticCode::DUPLICATE_KEY,
                format!("schema key '{key}' is ambiguous across stable ids {ids:?}"),
                ids.into_iter().map(SemanticSubject::Schema).collect(),
            ));
        }
        Ok(())
    }

    fn validate_entity_keys(
        document: &Document,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for (entity_id, entity) in &document.entities {
            poll_cancellation(cancelled)?;
            groups
                .entry(entity.key.clone())
                .or_default()
                .push(entity_id.clone());
        }
        for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
            poll_cancellation(cancelled)?;
            diagnostics.push(core_diagnostic(
                format!("entity_keys.{key}"),
                DiagnosticCode::DUPLICATE_KEY,
                format!("entity key '{key}' is ambiguous across stable ids {ids:?}"),
                ids.into_iter().map(SemanticSubject::Entity).collect(),
            ));
        }
        Ok(())
    }

    fn validate_field_keys(
        schema_id: &crate::SchemaId,
        schema: &Schema,
        schema_path: &str,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for (field_id, field) in &schema.fields {
            poll_cancellation(cancelled)?;
            groups
                .entry(field.key.clone())
                .or_default()
                .push(field_id.clone());
        }
        for (key, ids) in groups.into_iter().filter(|(_, ids)| ids.len() > 1) {
            poll_cancellation(cancelled)?;
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
        Ok(())
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
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        for (field, definition) in &schema.fields {
            poll_cancellation(cancelled)?;
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
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_value(
        document: &Document,
        field: &FieldRef,
        value: &Value,
        expected: &FieldType,
        path: &str,
        include_formula_references: bool,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        poll_cancellation(cancelled)?;
        match (expected, value) {
            (FieldType::Number, Value::Number(_))
            | (FieldType::Text, Value::Text(_))
            | (FieldType::Boolean, Value::Boolean(_)) => {}
            (FieldType::Number, Value::Formula(expression)) => {
                if include_formula_references {
                    validate_expression(document, field, expression, path, diagnostics, cancelled)?;
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
                    return Ok(());
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
        Ok(())
    }

    fn validate_expression(
        document: &Document,
        owner: &FieldRef,
        expression: &Expression,
        path: &str,
        diagnostics: &mut Vec<Diagnostic>,
        cancelled: &mut impl FnMut() -> bool,
    ) -> ValidationControl {
        let mut stack = vec![expression];
        let mut nodes = 0_usize;
        while let Some(node) = stack.pop() {
            if nodes % 64 == 0 {
                poll_cancellation(cancelled)?;
            }
            nodes += 1;
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
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use std::collections::BTreeMap;

        use crate::{
            Diagnostic, Entity, EntityId, Expression, FieldDefinition, FieldId, FieldRef,
            FieldType, Number, Schema, SchemaId, Value,
        };

        use super::{Document, validate_document_cancellable, validate_expression};

        #[test]
        fn completed_research_validation_matches_the_accepted_validator() {
            let mut document = Document::empty("", " ");
            document.schemas.insert(
                SchemaId::from("schema-store-id"),
                Schema {
                    id: SchemaId::from("schema-declared-id"),
                    key: "Invalid Schema Key".into(),
                    fields: BTreeMap::from([
                        (
                            FieldId::from("name"),
                            FieldDefinition {
                                id: FieldId::from("name"),
                                key: "name".into(),
                                field_type: FieldType::Text,
                                required: true,
                            },
                        ),
                        (
                            FieldId::from("amount"),
                            FieldDefinition {
                                id: FieldId::from("different-amount-id"),
                                key: "Invalid Amount Key".into(),
                                field_type: FieldType::Number,
                                required: true,
                            },
                        ),
                        (
                            FieldId::from("calc"),
                            FieldDefinition {
                                id: FieldId::from("calc"),
                                key: "calc".into(),
                                field_type: FieldType::Number,
                                required: true,
                            },
                        ),
                    ]),
                },
            );
            document.entities.insert(
                EntityId::from("entity-store-id"),
                Entity {
                    id: EntityId::from("entity-declared-id"),
                    key: "Invalid Entity Key".into(),
                    schema: SchemaId::from("schema-store-id"),
                    fields: BTreeMap::from([
                        (FieldId::from("amount"), Value::Text("wrong".to_owned())),
                        (
                            FieldId::from("calc"),
                            Value::Formula(Expression::Reference(FieldRef::new(
                                "entity-store-id",
                                "name",
                            ))),
                        ),
                        (FieldId::from("unexpected"), Value::Boolean(true)),
                    ]),
                },
            );

            assert_eq!(
                validate_document_cancellable(&document, || false),
                Some(super::super::validate_document(&document)),
            );
        }

        #[test]
        fn cancelled_research_validation_never_publishes_diagnostics() {
            let document = Document::empty("document", "Document");

            assert_eq!(validate_document_cancellable(&document, || true), None);
        }

        #[test]
        fn formula_traversal_polls_cancellation_at_the_bounded_node_interval() {
            let document = Document::empty("document", "Document");
            let owner = FieldRef::new("entity", "field");
            let one = || Expression::Number(Number::new(1.0).unwrap());
            let mut expression = one();
            for _ in 0..65 {
                expression = Expression::Add {
                    left: Box::new(expression),
                    right: Box::new(one()),
                };
            }
            let mut diagnostics = Vec::<Diagnostic>::new();
            let mut polls = 0_usize;

            let result = validate_expression(
                &document,
                &owner,
                &expression,
                "entities.entity.fields.field",
                &mut diagnostics,
                &mut || {
                    polls += 1;
                    polls == 2
                },
            );

            assert!(result.is_err());
            assert_eq!(polls, 2);
            assert!(diagnostics.is_empty());
        }
    }
}
