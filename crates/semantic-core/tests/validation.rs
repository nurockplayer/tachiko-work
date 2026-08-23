use std::collections::BTreeMap;

use tachiko_semantic_core::{
    DiagnosticCode, Document, Entity, EntityId, EntityKey, is_valid_identifier, validate_document,
};

#[test]
fn public_identifier_predicate_accepts_exactly_the_stable_path_grammar() {
    for valid in ["a", "0", "iron_sword", "weapon-2", "9lives"] {
        assert!(is_valid_identifier(valid), "expected '{valid}' to be valid");
    }

    for invalid in [
        "",
        "_hidden",
        "-hidden",
        "IronSword",
        "iron sword",
        "iron.sword",
        "café",
    ] {
        assert!(
            !is_valid_identifier(invalid),
            "expected '{invalid}' to be invalid"
        );
    }
}

#[test]
fn public_identifier_predicate_matches_human_key_validation() {
    for identifier in ["balance", "2d-balance", "", "Balance", "balance.data"] {
        let mut document = Document::empty("opaque stable id", "Balance");
        document.entities.insert(
            EntityId::from("entity stable id"),
            Entity {
                id: EntityId::from("entity stable id"),
                key: EntityKey::from(identifier),
                schema: "missing but opaque".into(),
                fields: BTreeMap::default(),
            },
        );
        let has_identifier_diagnostic = validate_document(&document).iter().any(|diagnostic| {
            diagnostic.path == "entities.entity stable id.key"
                && matches!(
                    diagnostic.code,
                    DiagnosticCode::EMPTY_KEY | DiagnosticCode::INVALID_KEY
                )
        });

        assert_eq!(
            has_identifier_diagnostic,
            !is_valid_identifier(identifier),
            "predicate and validation diverged for '{identifier}'"
        );
    }
}
