use crate::{Diagnostic, DiagnosticCode, Document, FieldType, Schema, SchemaId, SemanticSubject};

use super::{
    core_diagnostic, key_mismatch_diagnostic, validate_field_constraint, validate_human_key,
    validate_stable_id,
};

pub(super) fn validate_schema_fields(
    document: &Document,
    schema_id: &SchemaId,
    schema: &Schema,
    schema_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
            || format!("{field_path}.id"),
            "field",
            || field_subject.clone(),
            diagnostics,
        );
        validate_human_key(
            definition.key.as_str(),
            || format!("{field_path}.key"),
            "field",
            || field_subject.clone(),
            diagnostics,
        );
        validate_field_constraint(definition, &field_path, field_subject.clone(), diagnostics);
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
