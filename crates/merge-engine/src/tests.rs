use super::ConflictKind;

#[test]
fn conflict_kind_ranks_match_semantic_conflict_v1() {
    assert_eq!(ConflictKind::ConcurrentAddition.canonical_rank(), 0);
    assert_eq!(ConflictKind::DeleteModify.canonical_rank(), 1);
    assert_eq!(ConflictKind::ConcurrentChange.canonical_rank(), 2);
}
