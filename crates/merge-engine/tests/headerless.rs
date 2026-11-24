use std::collections::BTreeMap;
use tachiko_merge_engine::{
    ConflictFactV2, MergeOutcome, MergeOutcomeV2, MergeValueV2, merge, merge_v2,
};
use tachiko_semantic_core::{Document, Entity, Schema, Value};

fn document() -> Document {
    let mut document = Document::empty("d", "Blank");
    document.schemas.insert(
        "s".into(),
        Schema {
            id: "s".into(),
            key: "s".into(),
            display_name: None,
            headerless: false,
            field_order: None,
            fields: BTreeMap::new(),
        },
    );
    document
}

#[test]
fn headerless_merges_independently_from_cells_and_preserves_both_sides_order() {
    let base = document();
    let mut left = base.clone();
    left.schemas.get_mut("s").unwrap().headerless = true;
    let mut right = base.clone();
    right.title = "Updated".into();
    right.entities.insert(
        "r".into(),
        Entity {
            id: "r".into(),
            key: "r".into(),
            schema: "s".into(),
            rank: None,
            fields: BTreeMap::<_, Value>::new(),
        },
    );
    for (left, right) in [(&left, &right), (&right, &left), (&left, &left)] {
        let MergeOutcomeV2::Merged(candidate) = merge_v2(&base, left, right) else {
            panic!("must merge")
        };
        assert!(candidate.schemas["s"].headerless);
        if left != right {
            assert_eq!(candidate.title, "Updated");
            assert!(candidate.entities.contains_key("r"));
        }
    }
    assert!(matches!(
        merge(&base, &left, &right),
        MergeOutcome::UnsupportedHeaderless
    ));
    let mut prior_headerless = base.clone();
    prior_headerless.schemas.get_mut("s").unwrap().headerless = true;
    let MergeOutcomeV2::Merged(candidate) = merge_v2(&prior_headerless, &base, &prior_headerless)
    else {
        panic!("must merge")
    };
    assert!(!candidate.schemas["s"].headerless);
}

#[test]
fn headerless_delete_modify_and_concurrent_addition_keep_complete_conflict_evidence() {
    let base = document();
    let mut headerless = base.clone();
    headerless.schemas.get_mut("s").unwrap().headerless = true;
    let mut empty = base.clone();
    empty.schemas.clear();
    for (base, left, right) in [(&base, &empty, &headerless), (&empty, &base, &headerless)] {
        let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(base, left, right) else {
            panic!("schema subject conflict required")
        };
        assert!(conflicts.iter().any(|conflict| matches!(conflict.right(),
            ConflictFactV2::Present(MergeValueV2::SchemaSubject(schema)) if schema.headerless)));
    }
}
