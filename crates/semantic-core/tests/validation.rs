use tachiko_semantic_core::{DiagnosticCode, Document, is_valid_identifier, validate_document};

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
fn public_identifier_predicate_matches_document_validation() {
    for identifier in ["balance", "2d-balance", "", "Balance", "balance.data"] {
        let document = Document::empty(identifier, "Balance");
        let has_identifier_diagnostic = validate_document(&document).iter().any(|diagnostic| {
            diagnostic.path == "id"
                && matches!(
                    diagnostic.code,
                    DiagnosticCode::EmptyIdentifier | DiagnosticCode::InvalidIdentifier
                )
        });

        assert_eq!(
            has_identifier_diagnostic,
            !is_valid_identifier(identifier),
            "predicate and validation diverged for '{identifier}'"
        );
    }
}
