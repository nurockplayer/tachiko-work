use tachiko_semantic_core::{Document, DocumentId};

use crate::{ConflictTarget, choose};

use super::{ConflictFacetV2, MergeConflictV2, MergeValueV2};

#[derive(Clone, Copy)]
struct NameVersions<'a> {
    base: Option<&'a str>,
    left: Option<&'a str>,
    right: Option<&'a str>,
}

fn name_conflict(
    document: &DocumentId,
    target: ConflictTarget,
    names: NameVersions<'_>,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    let NameVersions { base, left, right } = names;
    if choose(&base, &left, &right).is_none() {
        conflicts.push(MergeConflictV2::new(
            document,
            target,
            ConflictFacetV2::DisplayName,
            Some(MergeValueV2::DisplayName(base.map(str::to_owned))),
            Some(MergeValueV2::DisplayName(left.map(str::to_owned))),
            Some(MergeValueV2::DisplayName(right.map(str::to_owned))),
        ));
    }
}

pub(super) fn add_name_conflicts(
    base: &Document,
    left: &Document,
    right: &Document,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    for (schema_id, b) in &base.schemas {
        let (Some(l), Some(r)) = (left.schemas.get(schema_id), right.schemas.get(schema_id)) else {
            continue;
        };
        name_conflict(
            &base.id,
            ConflictTarget::Schema(schema_id.clone()),
            NameVersions {
                base: b.display_name.as_deref(),
                left: l.display_name.as_deref(),
                right: r.display_name.as_deref(),
            },
            conflicts,
        );
        for (field_id, b) in &b.fields {
            let (Some(l), Some(r)) = (l.fields.get(field_id), r.fields.get(field_id)) else {
                continue;
            };
            name_conflict(
                &base.id,
                ConflictTarget::SchemaField {
                    schema: schema_id.clone(),
                    field: field_id.clone(),
                },
                NameVersions {
                    base: b.display_name.as_deref(),
                    left: l.display_name.as_deref(),
                    right: r.display_name.as_deref(),
                },
                conflicts,
            );
        }
    }
}
