use std::collections::BTreeMap;
use tachiko_diff_engine::{CANONICAL_SEMANTIC_DELTA_V2, CanonicalDirectFact, canonical_delta};
use tachiko_merge_engine::{
    ConflictFacetV2, ConflictFactV2, ConflictKind, MergeOutcomeV2, MergeValueV2, merge_v2,
};
use tachiko_workspace_engine::*;

fn base() -> Document {
    let field = FieldDefinition {
        id: "f".into(),
        key: "item".into(),
        display_name: None,
        number_format: None,
        field_type: FieldType::Text,
        required: false,
        constraint: FieldConstraint::None,
    };
    let schema = Schema {
        id: "s".into(),
        key: "table".into(),
        display_name: None,
        fields: BTreeMap::from([(field.id.clone(), field)]),
    };
    let mut document = Document::empty("names", "Names");
    document.schemas.insert(schema.id.clone(), schema);
    document
}

fn field(document: &mut Document) -> &mut FieldDefinition {
    document
        .schemas
        .get_mut("s")
        .unwrap()
        .fields
        .get_mut("f")
        .unwrap()
}

#[test]
fn storing_a_name_equal_to_key_fallback_is_still_a_direct_fact() {
    let base = base();
    let mut after = base.clone();
    field(&mut after).display_name = Some("item".into());
    let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &base, &after).unwrap();
    assert!(
        matches!(delta.facts(), [CanonicalDirectFact::SchemaFieldDisplayNameChanged { before: None, after: Some(name), .. }] if name == "item")
    );
}

#[test]
fn schema_names_have_their_own_present_conflict_facet_and_equal_clears_merge() {
    let mut base = base();
    let mut left = base.clone();
    let mut right = base.clone();
    left.schemas.get_mut("s").unwrap().display_name = Some("My Expenses".into());
    right.schemas.get_mut("s").unwrap().display_name = Some("経費".into());
    let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &base, &left).unwrap();
    assert!(matches!(
        delta.facts(),
        [CanonicalDirectFact::SchemaDisplayNameChanged { .. }]
    ));
    let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &left, &right) else {
        panic!("unequal schema labels conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].facet(), ConflictFacetV2::DisplayName);
    assert_eq!(
        conflicts[0].base(),
        &ConflictFactV2::Present(MergeValueV2::DisplayName(None))
    );
    base.schemas.get_mut("s").unwrap().display_name = Some("Table".into());
    left.schemas.get_mut("s").unwrap().display_name = None;
    right.schemas.get_mut("s").unwrap().display_name = None;
    let MergeOutcomeV2::Merged(candidate) = merge_v2(&base, &left, &right) else {
        panic!("equal clears merge");
    };
    assert_eq!(candidate.schemas["s"].display_name, None);
}

#[test]
fn label_only_delta_has_present_optional_values_and_complete_payloads() {
    let base = base();
    let mut after = base.clone();
    field(&mut after).display_name = Some("項目".into());
    let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &base, &after).unwrap();
    assert_eq!(
        delta.facts(),
        [CanonicalDirectFact::SchemaFieldDisplayNameChanged {
            schema: "s".into(),
            field: "f".into(),
            before: None,
            after: Some("項目".into())
        }]
    );
    assert!(matches!(
        tachiko_diff_engine::diff(&base, &after),
        Err(tachiko_diff_engine::DiffError::UnsupportedDisplayName)
    ));
    let clearing = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &after, &base).unwrap();
    assert_eq!(
        clearing.facts(),
        [CanonicalDirectFact::SchemaFieldDisplayNameChanged {
            schema: "s".into(),
            field: "f".into(),
            before: Some("項目".into()),
            after: None
        }]
    );
    after.schemas.get_mut("s").unwrap().display_name = Some("Table Name".into());
    let empty = Document::empty("names", "Names");
    let created = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, &empty, &after).unwrap();
    let [CanonicalDirectFact::SchemaCreated { definition, .. }] = created.facts() else {
        panic!("one parent fact");
    };
    assert_eq!(definition.display_name.as_deref(), Some("Table Name"));
    assert_eq!(definition.fields["f"].display_name.as_deref(), Some("項目"));
}

#[test]
fn concurrent_names_on_a_nameless_subject_are_present_concurrent_changes() {
    let base = base();
    let mut left = base.clone();
    let mut right = base.clone();
    field(&mut left).display_name = Some("Item".into());
    field(&mut right).display_name = Some("項目".into());
    let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &left, &right) else {
        panic!("concurrent names conflict");
    };
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.facet(), ConflictFacetV2::DisplayName);
    assert_eq!(conflict.kind(), ConflictKind::ConcurrentChange);
    assert_eq!(
        conflict.base(),
        &ConflictFactV2::Present(MergeValueV2::DisplayName(None))
    );
    assert_eq!(
        conflict.left(),
        &ConflictFactV2::Present(MergeValueV2::DisplayName(Some("Item".into())))
    );
    assert_eq!(
        conflict.right(),
        &ConflictFactV2::Present(MergeValueV2::DisplayName(Some("項目".into())))
    );
    assert!(matches!(
        tachiko_merge_engine::merge(&base, &left, &right),
        tachiko_merge_engine::MergeOutcome::UnsupportedDisplayName
    ));
}

#[test]
fn equal_names_and_independent_key_edits_merge_without_losing_labels() {
    let base = base();
    let mut left = base.clone();
    let mut right = base.clone();
    field(&mut left).display_name = Some("Item".into());
    field(&mut right).key = "price".into();
    let WorkspaceMergeOutcomeV2::Merged(preview) =
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &left, &right).unwrap()
    else {
        panic!("independent facets merge");
    };
    let merged = &preview.document;
    assert_eq!(merged.schemas["s"].fields["f"].key.as_str(), "price");
    assert_eq!(
        merged.schemas["s"].fields["f"].display_name.as_deref(),
        Some("Item")
    );
    field(&mut right).display_name = Some("Item".into());
    assert!(matches!(
        merge_v2(&base, &left, &right),
        MergeOutcomeV2::Merged(_)
    ));
}

#[test]
fn parent_delete_modify_evidence_retains_names_and_suppresses_children() {
    let base = base();
    let mut left = base.clone();
    let mut right = base.clone();
    left.schemas.clear();
    field(&mut right).display_name = Some("Item".into());
    let MergeOutcomeV2::Conflicted(conflicts) = merge_v2(&base, &left, &right) else {
        panic!("deletion conflicts with name modification");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].facet(), ConflictFacetV2::Subject);
    let ConflictFactV2::Present(MergeValueV2::SchemaSubject(subject)) = conflicts[0].right() else {
        panic!("complete parent");
    };
    assert_eq!(subject.fields["f"].display_name.as_deref(), Some("Item"));
}

#[test]
fn cross_object_names_collide_at_candidate_validation_not_structural_conflict() {
    let base = base();
    let mut left = base.clone();
    let mut right = base.clone();
    for (document, id) in [(&mut left, "g"), (&mut right, "h")] {
        let mut new = field(document).clone();
        new.id = id.into();
        new.key = id.into();
        new.display_name = Some("項目".into());
        document
            .schemas
            .get_mut("s")
            .unwrap()
            .fields
            .insert(new.id.clone(), new);
    }
    assert!(matches!(
        merge_v2(&base, &left, &right),
        MergeOutcomeV2::Merged(_)
    ));
    assert!(matches!(
        merge_documents_v2(SEMANTIC_CONFLICT_V2, &base, &left, &right),
        Err(WorkspaceError::InvalidDocument {
            role: ValidationRole::MergeCandidate,
            ..
        })
    ));
}
