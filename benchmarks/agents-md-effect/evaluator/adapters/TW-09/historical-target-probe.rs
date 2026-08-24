use std::collections::BTreeMap;

use serde_json::{Value as JsonValue, json};
use tachiko_semantic_core::{
    Diagnostic, DiagnosticCode, DiagnosticFact, DiagnosticProvider, DiagnosticSeverity, Document,
    Schema, SchemaId, SchemaKey, SemanticSubject, validate_document_core,
};

fn duplicate_schema_keys(key: &str) -> Document {
    let mut document = Document::empty("document-id", "Diagnostics");
    for id in ["schema-a", "schema-b"] {
        document.schemas.insert(
            SchemaId::from(id),
            Schema {
                id: SchemaId::from(id),
                key: SchemaKey::from(key),
                fields: BTreeMap::new(),
            },
        );
    }
    document
}

fn schema_subjects(subjects: &[SemanticSubject]) -> Vec<JsonValue> {
    subjects
        .iter()
        .map(|subject| match subject {
            SemanticSubject::Schema(id) => json!({"kind": "schema", "stable_id": id}),
            other => panic!("expected schema subject, received {other:?}"),
        })
        .collect()
}

fn main() {
    let semantic = Diagnostic::new(
        DiagnosticCode::new("test.rule"),
        DiagnosticSeverity::Error,
        vec![SemanticSubject::Schema(SchemaId::from("schema-id"))],
        DiagnosticProvider::new("test.validator"),
    )
    .with_related_subjects(vec![SemanticSubject::Schema(SchemaId::from("related-id"))])
    .with_fact(DiagnosticFact::new("expected_kind", "number"));
    let first = semantic
        .clone()
        .with_presentation("schemas.old-key", "old wording");
    let second = semantic.with_presentation("schemas.new-key", "new wording");
    let stable = first.stable_observation();

    let before = validate_document_core(&duplicate_schema_keys("old-key"));
    let after = validate_document_core(&duplicate_schema_keys("new-key"));
    let before_duplicate = before
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::DUPLICATE_KEY)
        .expect("duplicate diagnostic before rename");
    let after_duplicate = after
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::DUPLICATE_KEY)
        .expect("duplicate diagnostic after rename");

    let provider = DiagnosticProvider::new("test.validator");
    let subject = SemanticSubject::Document("document-id".into());
    let mut ordered = [
        Diagnostic::new(
            DiagnosticCode::new("test.z"),
            DiagnosticSeverity::Error,
            vec![subject.clone()],
            provider,
        )
        .with_presentation("a.presentation.path", "first by presentation"),
        Diagnostic::new(
            DiagnosticCode::new("test.a"),
            DiagnosticSeverity::Error,
            vec![subject],
            provider,
        )
        .with_presentation("z.presentation.path", "last by presentation"),
    ];
    ordered.sort();

    let observations = json!({
        "machine_fact": {
            "code": stable.code.as_str(),
            "classification": "semantic",
            "severity": "error",
            "provider": stable.provider.as_str(),
            "subjects": schema_subjects(&stable.subjects),
            "related_subjects": schema_subjects(&stable.related_subjects),
            "facts": stable.facts.iter().map(|fact| json!({
                "name": fact.name,
                "value": fact.value,
            })).collect::<Vec<_>>(),
        },
        "presentation_invariance": {
            "stable_facts_equal": first.stable_observation() == second.stable_observation(),
            "presentation_differs": first.path != second.path && first.message != second.message,
        },
        "renamed_duplicate": {
            "stable_facts_equal": before_duplicate.stable_observation()
                == after_duplicate.stable_observation(),
            "subjects": schema_subjects(&before_duplicate.subjects),
        },
        "stable_order": {
            "machine_codes": ordered.iter().map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
        },
    });
    println!("{observations}");
}
