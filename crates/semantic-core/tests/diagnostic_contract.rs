use std::collections::BTreeMap;

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

#[test]
fn stable_observation_excludes_presentation() {
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

    assert_eq!(first.stable_observation(), second.stable_observation());
    assert_ne!(first.path, second.path);
    assert_ne!(first.message, second.message);
}

#[test]
fn multi_subject_duplicate_is_stable_across_human_key_rename() {
    let before = validate_document_core(&duplicate_schema_keys("old-key"));
    let after = validate_document_core(&duplicate_schema_keys("new-key"));

    let before = before
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::DUPLICATE_KEY)
        .expect("duplicate diagnostic");
    let after = after
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::DUPLICATE_KEY)
        .expect("duplicate diagnostic after rename");

    assert_eq!(
        before.subjects,
        vec![
            SemanticSubject::Schema(SchemaId::from("schema-a")),
            SemanticSubject::Schema(SchemaId::from("schema-b")),
        ]
    );
    assert_eq!(before.stable_observation(), after.stable_observation());
    assert_ne!(before.path, after.path);
}

#[test]
fn deterministic_order_uses_stable_observations_before_presentation() {
    let provider = DiagnosticProvider::new("test.validator");
    let subject = SemanticSubject::Document("document-id".into());
    let later_code = Diagnostic::new(
        DiagnosticCode::new("test.z"),
        DiagnosticSeverity::Error,
        vec![subject.clone()],
        provider,
    )
    .with_presentation("a.presentation.path", "first by presentation");
    let earlier_code = Diagnostic::new(
        DiagnosticCode::new("test.a"),
        DiagnosticSeverity::Error,
        vec![subject],
        provider,
    )
    .with_presentation("z.presentation.path", "last by presentation");

    let mut diagnostics = [later_code, earlier_code];
    diagnostics.sort();

    assert_eq!(diagnostics[0].code, DiagnosticCode::new("test.a"));
}
