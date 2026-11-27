use tachiko_semantic_core::Document;

use super::MergeConflictV2;

pub(super) fn collect_conflicts(
    base: &Document,
    left: &Document,
    right: &Document,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    super::add_complete_subject_conflicts(base, left, right, conflicts);
    super::add_entity_subject_conflicts(base, left, right, conflicts);
    super::add_rank_conflicts(base, left, right, conflicts);
    super::add_constraint_conflicts(base, left, right, conflicts);
    super::add_name_conflicts(base, left, right, conflicts);
    super::add_field_order_conflicts(base, left, right, conflicts);
    conflicts.sort_by(|left, right| {
        left.target
            .canonical_cmp(&right.target)
            .then_with(|| {
                left.facet
                    .canonical_rank(&left.target)
                    .unwrap_or(u8::MAX)
                    .cmp(&right.facet.canonical_rank(&right.target).unwrap_or(u8::MAX))
            })
            .then_with(|| left.kind.canonical_rank().cmp(&right.kind.canonical_rank()))
    });
}
