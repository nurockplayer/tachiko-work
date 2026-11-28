use tachiko_semantic_core::Document;

use super::MergeOutcome;

pub(super) fn first_refusal(
    base: &Document,
    ours: &Document,
    theirs: &Document,
) -> Option<MergeOutcome> {
    if [base, ours, theirs]
        .iter()
        .any(|document| document.has_row_ranks())
    {
        return Some(MergeOutcome::UnsupportedRowRank);
    }
    if [base, ours, theirs]
        .iter()
        .any(|document| document.has_number_formats() || document.has_date_formats())
    {
        return Some(MergeOutcome::UnsupportedNumberFormat);
    }
    if [base, ours, theirs]
        .iter()
        .any(|document| document.has_field_orders())
    {
        return Some(MergeOutcome::UnsupportedFieldOrder);
    }
    if [base, ours, theirs]
        .iter()
        .any(|document| document.has_display_names())
    {
        return Some(MergeOutcome::UnsupportedDisplayName);
    }
    if has_field_constraints(base) || has_field_constraints(ours) || has_field_constraints(theirs) {
        return Some(MergeOutcome::UnsupportedFieldConstraintChange);
    }
    if base.keyed_grouped_sum_definitions != ours.keyed_grouped_sum_definitions
        || base.keyed_grouped_sum_definitions != theirs.keyed_grouped_sum_definitions
    {
        return Some(MergeOutcome::UnsupportedKeyedGroupedSumDefinitionChange);
    }
    None
}

fn has_field_constraints(document: &Document) -> bool {
    document
        .schemas
        .values()
        .flat_map(|schema| schema.fields.values())
        .any(|field| !field.constraint.is_none())
}
