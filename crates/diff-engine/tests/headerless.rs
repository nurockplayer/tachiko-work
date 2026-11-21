use std::collections::BTreeMap;
use tachiko_diff_engine::{
    CANONICAL_SEMANTIC_DELTA_V2, CanonicalDirectFact, DiffError, canonical_delta, diff,
};
use tachiko_semantic_core::{Document, Schema};

fn document(headerless: bool) -> Document {
    let mut document = Document::empty("d", "Blank");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            headerless,
            field_order: None,
            fields: BTreeMap::new(),
        },
    );
    document
}

#[test]
fn headerless_delta_is_a_schema_fact_and_complete_creation_deletion_payload() {
    let before = document(false);
    let after = document(true);
    for (before, after, old, new) in [
        (&before, &after, false, true),
        (&after, &before, true, false),
    ] {
        let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, before, after).unwrap();
        assert_eq!(
            delta.facts(),
            &[CanonicalDirectFact::SchemaHeaderlessChanged {
                schema: "s".into(),
                before: old,
                after: new,
            }]
        );
    }
    let mut empty = before.clone();
    empty.schemas.clear();
    let created = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &empty, &after).unwrap();
    assert!(
        matches!(created.facts(), [CanonicalDirectFact::SchemaCreated { definition, .. }] if definition.headerless)
    );
    let deleted = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &after, &empty).unwrap();
    assert!(
        matches!(deleted.facts(), [CanonicalDirectFact::SchemaDeleted { definition, .. }] if definition.headerless)
    );
    assert!(matches!(
        diff(&before, &after),
        Err(DiffError::UnsupportedHeaderless)
    ));
    assert!(matches!(
        diff(&after, &after),
        Err(DiffError::UnsupportedHeaderless)
    ));
}
