use std::collections::BTreeSet;

use super::{AnalysisFailure, DisclosureRequirement, Document, EntityId, SchemaId};

pub(super) fn resolve_default_domain(
    document: &Document,
    schema: &SchemaId,
    document_requirement: &DisclosureRequirement,
    requirements: &mut BTreeSet<DisclosureRequirement>,
) -> (Vec<EntityId>, Option<AnalysisFailure>) {
    let mut candidates = Vec::new();
    let mut deferred_failure = None;
    for (key, entity) in &document.entities {
        if &entity.schema != schema {
            continue;
        }
        if key == &entity.id {
            candidates.push(key.clone());
        } else {
            requirements.insert(document_requirement.clone());
            deferred_failure.get_or_insert_with(|| AnalysisFailure::IncoherentCandidateIdentity {
                key: key.clone(),
                entity: entity.id.clone(),
            });
        }
    }
    (candidates, deferred_failure)
}
