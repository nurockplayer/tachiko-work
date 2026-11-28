use std::collections::BTreeSet;

use tachiko_semantic_core::{Document, Schema, SchemaId};

use super::{
    ConflictFacetV2, ConflictTarget, MergeConflictV2, MergeValueV2, SchemaFieldSubjectV2,
    subject_presence_conflicts,
};

#[derive(Clone, Copy)]
pub(super) struct SchemaVersions<'a> {
    pub(super) base: &'a Schema,
    pub(super) left: &'a Schema,
    pub(super) right: &'a Schema,
}

pub(super) fn add_field_subject_conflicts(
    base: &Document,
    schema_id: &SchemaId,
    schemas: SchemaVersions<'_>,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    let field_ids: BTreeSet<_> = schemas
        .base
        .fields
        .keys()
        .chain(schemas.left.fields.keys())
        .chain(schemas.right.fields.keys())
        .cloned()
        .collect();
    for field_id in field_ids {
        let b = schemas.base.fields.get(&field_id);
        let l = schemas.left.fields.get(&field_id);
        let r = schemas.right.fields.get(&field_id);
        if subject_presence_conflicts(b, l, r) {
            conflicts.push(MergeConflictV2::new(
                &base.id,
                ConflictTarget::SchemaField {
                    schema: schema_id.clone(),
                    field: field_id,
                },
                ConflictFacetV2::Subject,
                b.map(|field| MergeValueV2::SchemaFieldSubject(SchemaFieldSubjectV2::from(field))),
                l.map(|field| MergeValueV2::SchemaFieldSubject(SchemaFieldSubjectV2::from(field))),
                r.map(|field| MergeValueV2::SchemaFieldSubject(SchemaFieldSubjectV2::from(field))),
            ));
        }
    }
}
