use std::collections::BTreeSet;

use super::{FieldId, Schema, SchemaId};

#[cfg(test)]
mod tests;

pub(super) fn record_targets(
    schema: &Schema,
    constraint_targets: &BTreeSet<(SchemaId, FieldId)>,
    collection_lifecycle_targets: &mut BTreeSet<(SchemaId, Option<FieldId>)>,
) -> Option<(SchemaId, FieldId)> {
    for definition in schema.fields.values() {
        let target = (schema.id.clone(), definition.id.clone());
        if constraint_targets.contains(&target) {
            return Some(target);
        }
        collection_lifecycle_targets.insert((target.0, Some(target.1)));
    }
    None
}
