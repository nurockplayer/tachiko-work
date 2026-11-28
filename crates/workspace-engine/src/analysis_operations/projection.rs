use std::collections::BTreeMap;

use super::{
    AnalysisGroup, AnalysisGroupKey, AnalysisProjection, NormalizedAnalysisDefinition, build_bucket,
};
use crate::{EntityId, FieldId, Number};

pub(super) fn assemble_projection(
    definition: &NormalizedAnalysisDefinition,
    selected: &[EntityId],
    groups: BTreeMap<AnalysisGroupKey, Vec<EntityId>>,
    metrics: &BTreeMap<FieldId, BTreeMap<EntityId, Number>>,
) -> AnalysisProjection {
    if definition.group_by.is_some() {
        AnalysisProjection::Grouped(
            groups
                .into_iter()
                .map(|(key, members)| AnalysisGroup {
                    key,
                    bucket: build_bucket(definition, &members, metrics),
                })
                .collect(),
        )
    } else {
        AnalysisProjection::Ungrouped(build_bucket(definition, selected, metrics))
    }
}
