//! Literal duplicate-key and cancellation acceptance for the allocation repair.
use std::collections::BTreeMap;

use tachiko_semantic_core::{
    Diagnostic, DiagnosticCode, DiagnosticProvider, DiagnosticSeverity, Document, Entity, Schema,
    SemanticSubject, validate_document, validate_document_core,
};

fn fixture() -> Document {
    let mut document = Document::empty("doc", "Duplicate keys");
    for id in ["s", "t"] {
        document.schemas.insert(
            id.into(),
            Schema {
                id: id.into(),
                key: id.into(),
                fields: BTreeMap::new(),
            },
        );
    }
    for (id, key, schema) in [
        ("z", "same", "s"),
        ("b", "other", "t"),
        ("a", "same", "t"),
        ("c", "same", "s"),
        ("d", "other", "s"),
        ("e", "unique_e", "s"),
        ("f", "unique_f", "t"),
        ("g", "unique_g", "s"),
    ] {
        document.entities.insert(
            id.into(),
            Entity {
                id: id.into(),
                key: key.into(),
                schema: schema.into(),
                fields: BTreeMap::new(),
            },
        );
    }
    document
}

fn expected() -> Vec<Diagnostic> {
    let mut diagnostics = vec![
        Diagnostic::new(
            DiagnosticCode::DUPLICATE_KEY,
            DiagnosticSeverity::Error,
            vec![SemanticSubject::Entity("b".into()), SemanticSubject::Entity("d".into())],
            DiagnosticProvider::new("tachiko.semantic-core"),
        ).with_presentation(
            "entity_keys.other",
            "entity key 'other' is ambiguous across stable ids [EntityId(\"b\"), EntityId(\"d\")]",
        ),
        Diagnostic::new(
            DiagnosticCode::DUPLICATE_KEY,
            DiagnosticSeverity::Error,
            vec![SemanticSubject::Entity("a".into()), SemanticSubject::Entity("c".into()), SemanticSubject::Entity("z".into())],
            DiagnosticProvider::new("tachiko.semantic-core"),
        ).with_presentation(
            "entity_keys.same",
            "entity key 'same' is ambiguous across stable ids [EntityId(\"a\"), EntityId(\"c\"), EntityId(\"z\")]",
        ),
    ];
    diagnostics.sort();
    diagnostics
}

#[test]
fn duplicate_groups_keep_literal_order_and_detached_owned_diagnostics() {
    let document = fixture();
    let legacy = validate_document(&document);
    let core = validate_document_core(&document);
    drop(document);
    assert_eq!(legacy, expected());
    assert_eq!(core, expected());
}

#[test]
fn unique_empty_and_single_entity_groups_produce_no_diagnostics() {
    let mut document = fixture();
    for entity in document.entities.values_mut() {
        entity.key = entity.id.as_str().into();
    }
    for count in [8, 1, 0] {
        let keep = document
            .entities
            .keys()
            .take(count)
            .cloned()
            .collect::<Vec<_>>();
        document.entities.retain(|id, _| keep.contains(id));
        assert_eq!(validate_document(&document), Vec::<Diagnostic>::new());
        assert_eq!(validate_document_core(&document), Vec::<Diagnostic>::new());
    }
}

#[cfg(feature = "issue-175-research")]
#[test]
fn cancellation_preserves_all_original_poll_boundaries() {
    use tachiko_semantic_core::validate_document_cancellable;
    let document = fixture();
    let mut polls = 0;
    assert_eq!(
        validate_document_cancellable(&document, || {
            polls += 1;
            false
        }),
        Some(expected())
    );
    assert_eq!(polls, 24);
    for stop in 1..=24 {
        let mut observed = 0;
        assert_eq!(
            validate_document_cancellable(&document, || {
                observed += 1;
                observed == stop
            }),
            None
        );
        assert_eq!(observed, stop);
    }
}
