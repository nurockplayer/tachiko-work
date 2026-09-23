//! Constraint-aware, version-owned model conflict evidence.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_semantic_core::{
    Document, DocumentId, FieldConstraint, FieldDefinition, FieldId, FieldKey, FieldType, Schema,
    SchemaId, SchemaKey, Value,
};

use crate::{
    ConflictFacet, ConflictFact, ConflictKind, ConflictTarget, EntitySubject, MergeCandidate,
    MergeConflict, MergeOutcome, MergeValue, OptionalChoice, choose, choose_optional, merge,
};

/// The explicitly selected constraint-aware semantic conflict contract.
pub const SEMANTIC_CONFLICT_V2: &str = "tachiko.semantic-conflict/v2";

/// Closed v2 facets. Constraint is versioned here and does not widen v1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictFacetV2 {
    Subject,
    Title,
    Key,
    FieldType,
    Requiredness,
    Schema,
    StoredValue,
    Constraint,
}

impl ConflictFacetV2 {
    const fn canonical_rank(self, target: &ConflictTarget) -> Option<u8> {
        match (target, self) {
            (ConflictTarget::Document(_), Self::Title)
            | (
                ConflictTarget::Schema(_)
                | ConflictTarget::SchemaField { .. }
                | ConflictTarget::Entity(_),
                Self::Subject,
            )
            | (ConflictTarget::StoredEntityField { .. }, Self::StoredValue) => Some(0),
            (
                ConflictTarget::Schema(_)
                | ConflictTarget::SchemaField { .. }
                | ConflictTarget::Entity(_),
                Self::Key,
            ) => Some(1),
            (ConflictTarget::SchemaField { .. }, Self::FieldType)
            | (ConflictTarget::Entity(_), Self::Schema) => Some(2),
            (ConflictTarget::SchemaField { .. }, Self::Requiredness) => Some(3),
            (ConflictTarget::SchemaField { .. }, Self::Constraint) => Some(4),
            _ => None,
        }
    }

    /// Validate this facet against the existing closed stable target family.
    ///
    /// # Errors
    ///
    /// Returns the unsupported target/facet pair when this facet cannot name
    /// the supplied target.
    pub fn validate_target(self, target: &ConflictTarget) -> Result<(), UnsupportedTargetFacetV2> {
        if self.canonical_rank(target).is_some() {
            Ok(())
        } else {
            Err(UnsupportedTargetFacetV2 {
                target: target.clone(),
                facet: self,
            })
        }
    }
}

/// Typed target/facet compatibility failure in conflict v2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedTargetFacetV2 {
    pub target: ConflictTarget,
    pub facet: ConflictFacetV2,
}

impl std::fmt::Display for UnsupportedTargetFacetV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unsupported semantic conflict v2 target/facet pairing: {:?} / {:?}",
            self.target, self.facet
        )
    }
}

impl std::error::Error for UnsupportedTargetFacetV2 {}

/// Complete constraint-aware schema subject payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaSubjectV2 {
    pub key: SchemaKey,
    pub fields: BTreeMap<FieldId, SchemaFieldSubjectV2>,
}

impl From<&Schema> for SchemaSubjectV2 {
    fn from(schema: &Schema) -> Self {
        Self {
            key: schema.key.clone(),
            fields: schema
                .fields
                .iter()
                .map(|(field_id, field)| (field_id.clone(), SchemaFieldSubjectV2::from(field)))
                .collect(),
        }
    }
}

/// Complete constraint-aware schema-field subject payload.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaFieldSubjectV2 {
    pub key: FieldKey,
    pub field_type: FieldType,
    pub required: bool,
    pub constraint: FieldConstraint,
}

impl From<&FieldDefinition> for SchemaFieldSubjectV2 {
    fn from(field: &FieldDefinition) -> Self {
        Self {
            key: field.key.clone(),
            field_type: field.field_type.clone(),
            required: field.required,
            constraint: field.constraint.clone(),
        }
    }
}

/// Closed v2 fact values, including the atomic constraint facet.
#[derive(Clone, Debug, PartialEq)]
pub enum MergeValueV2 {
    DocumentTitle(String),
    SchemaSubject(SchemaSubjectV2),
    SchemaFieldSubject(SchemaFieldSubjectV2),
    EntitySubject(EntitySubject),
    SchemaId(SchemaId),
    SchemaKey(SchemaKey),
    EntityKey(tachiko_semantic_core::EntityKey),
    FieldKey(FieldKey),
    FieldType(FieldType),
    Required(bool),
    FieldValue(Value),
    Constraint(FieldConstraint),
}

/// Explicit v2 absence/presence evidence.
#[derive(Clone, Debug, PartialEq)]
pub enum ConflictFactV2 {
    Absent,
    Present(MergeValueV2),
}

impl From<Option<MergeValueV2>> for ConflictFactV2 {
    fn from(value: Option<MergeValueV2>) -> Self {
        value.map_or(Self::Absent, Self::Present)
    }
}

/// One immutable constraint-aware structural conflict.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeConflictV2 {
    document: DocumentId,
    target: ConflictTarget,
    facet: ConflictFacetV2,
    kind: ConflictKind,
    base: ConflictFactV2,
    left: ConflictFactV2,
    right: ConflictFactV2,
}

impl MergeConflictV2 {
    fn new(
        document: &DocumentId,
        target: ConflictTarget,
        facet: ConflictFacetV2,
        base: Option<MergeValueV2>,
        left: Option<MergeValueV2>,
        right: Option<MergeValueV2>,
    ) -> Self {
        assert!(facet.validate_target(&target).is_ok());
        let kind = classify(base.as_ref(), left.as_ref(), right.as_ref())
            .expect("merge v2 emitted facts that do not form a structural conflict");
        Self {
            document: document.clone(),
            target,
            facet,
            kind,
            base: base.into(),
            left: left.into(),
            right: right.into(),
        }
    }

    #[must_use]
    pub const fn contract(&self) -> &'static str {
        SEMANTIC_CONFLICT_V2
    }

    #[must_use]
    pub fn document(&self) -> &DocumentId {
        &self.document
    }

    #[must_use]
    pub fn target(&self) -> &ConflictTarget {
        &self.target
    }

    #[must_use]
    pub const fn facet(&self) -> ConflictFacetV2 {
        self.facet
    }

    #[must_use]
    pub const fn kind(&self) -> ConflictKind {
        self.kind
    }

    #[must_use]
    pub fn base(&self) -> &ConflictFactV2 {
        &self.base
    }

    #[must_use]
    pub fn left(&self) -> &ConflictFactV2 {
        &self.left
    }

    #[must_use]
    pub fn right(&self) -> &ConflictFactV2 {
        &self.right
    }
}

/// Conflict-free v2 candidate, structural conflicts, or closed keyed-map refusal.
#[derive(Clone, Debug, PartialEq)]
pub enum MergeOutcomeV2 {
    Merged(MergeCandidate),
    Conflicted(Vec<MergeConflictV2>),
    UnsupportedKeyedGroupedSumDefinitionChange,
}

/// Reconcile model facts under the constraint-aware v2 conflict profile.
///
/// This is model-level structural reconciliation. Workspace-engine owns input
/// admission, complete candidate validation, projection, and canonical delta.
///
/// # Panics
///
/// Panics only if the frozen v1 merge violates its invariant that a conflict-
/// free outcome contains a candidate.
#[must_use]
pub fn merge_v2(base: &Document, left: &Document, right: &Document) -> MergeOutcomeV2 {
    if base.keyed_grouped_sum_definitions != left.keyed_grouped_sum_definitions
        || base.keyed_grouped_sum_definitions != right.keyed_grouped_sum_definitions
    {
        return MergeOutcomeV2::UnsupportedKeyedGroupedSumDefinitionChange;
    }

    let base_v1 = without_constraints(base);
    let left_v1 = without_constraints(left);
    let right_v1 = without_constraints(right);
    let (candidate, mut conflicts) = match merge(&base_v1, &left_v1, &right_v1) {
        MergeOutcome::Merged(candidate) => (Some(candidate), Vec::new()),
        MergeOutcome::Conflicted(conflicts) => (
            None,
            conflicts
                .iter()
                .filter(|conflict| !is_subject_conflict(conflict))
                .map(|conflict| convert_conflict(conflict, base, left, right))
                .collect(),
        ),
        MergeOutcome::UnsupportedKeyedGroupedSumDefinitionChange => {
            return MergeOutcomeV2::UnsupportedKeyedGroupedSumDefinitionChange;
        }
        MergeOutcome::UnsupportedFieldConstraintChange => {
            unreachable!("v2 merge removes constraints before invoking frozen v1")
        }
    };

    add_complete_subject_conflicts(base, left, right, &mut conflicts);
    add_constraint_conflicts(base, left, right, &mut conflicts);
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

    if !conflicts.is_empty() {
        return MergeOutcomeV2::Conflicted(conflicts);
    }

    let mut candidate = candidate.expect("v1 conflict-free result must retain a candidate");
    apply_selected_constraints(base, left, right, &mut candidate);
    MergeOutcomeV2::Merged(candidate)
}

#[cfg(test)]
mod tests;

fn without_constraints(document: &Document) -> Document {
    let mut document = document.clone();
    for schema in document.schemas.values_mut() {
        for field in schema.fields.values_mut() {
            field.constraint = FieldConstraint::None;
        }
    }
    document
}

fn is_subject_conflict(conflict: &MergeConflict) -> bool {
    conflict.facet() == ConflictFacet::Subject
        && matches!(
            conflict.target(),
            ConflictTarget::Schema(_) | ConflictTarget::SchemaField { .. }
        )
}

fn classify(
    base: Option<&MergeValueV2>,
    left: Option<&MergeValueV2>,
    right: Option<&MergeValueV2>,
) -> Option<ConflictKind> {
    match (base, left, right) {
        (None, Some(_), Some(_)) => Some(ConflictKind::ConcurrentAddition),
        (Some(_), None, Some(_)) | (Some(_), Some(_), None) => Some(ConflictKind::DeleteModify),
        (Some(_), Some(_), Some(_)) => Some(ConflictKind::ConcurrentChange),
        _ => None,
    }
}

fn convert_conflict(
    conflict: &MergeConflict,
    base: &Document,
    left: &Document,
    right: &Document,
) -> MergeConflictV2 {
    MergeConflictV2::new(
        conflict.document(),
        conflict.target().clone(),
        convert_facet(conflict.facet()),
        upgrade_fact(conflict.base(), base, conflict.target()),
        upgrade_fact(conflict.left(), left, conflict.target()),
        upgrade_fact(conflict.right(), right, conflict.target()),
    )
}

fn convert_facet(facet: ConflictFacet) -> ConflictFacetV2 {
    match facet {
        ConflictFacet::Subject => ConflictFacetV2::Subject,
        ConflictFacet::Title => ConflictFacetV2::Title,
        ConflictFacet::Key => ConflictFacetV2::Key,
        ConflictFacet::FieldType => ConflictFacetV2::FieldType,
        ConflictFacet::Requiredness => ConflictFacetV2::Requiredness,
        ConflictFacet::Schema => ConflictFacetV2::Schema,
        ConflictFacet::StoredValue => ConflictFacetV2::StoredValue,
    }
}

fn upgrade_fact(
    fact: &ConflictFact,
    source: &Document,
    target: &ConflictTarget,
) -> Option<MergeValueV2> {
    match fact {
        ConflictFact::Absent => None,
        ConflictFact::Present(MergeValue::SchemaSubject(_)) => {
            let ConflictTarget::Schema(schema_id) = target else {
                unreachable!("schema subject conflict has a schema target")
            };
            Some(MergeValueV2::SchemaSubject(SchemaSubjectV2::from(
                source
                    .schemas
                    .get(schema_id)
                    .expect("present source schema"),
            )))
        }
        ConflictFact::Present(MergeValue::SchemaFieldSubject(_)) => {
            let ConflictTarget::SchemaField { schema, field } = target else {
                unreachable!("schema-field subject conflict has a schema-field target")
            };
            Some(MergeValueV2::SchemaFieldSubject(
                SchemaFieldSubjectV2::from(
                    source
                        .schemas
                        .get(schema)
                        .and_then(|schema| schema.fields.get(field))
                        .expect("present source schema field"),
                ),
            ))
        }
        ConflictFact::Present(value) => Some(match value {
            MergeValue::DocumentTitle(value) => MergeValueV2::DocumentTitle(value.clone()),
            MergeValue::SchemaSubject(_) | MergeValue::SchemaFieldSubject(_) => unreachable!(),
            MergeValue::EntitySubject(value) => MergeValueV2::EntitySubject(value.clone()),
            MergeValue::SchemaId(value) => MergeValueV2::SchemaId(value.clone()),
            MergeValue::SchemaKey(value) => MergeValueV2::SchemaKey(value.clone()),
            MergeValue::EntityKey(value) => MergeValueV2::EntityKey(value.clone()),
            MergeValue::FieldKey(value) => MergeValueV2::FieldKey(value.clone()),
            MergeValue::FieldType(value) => MergeValueV2::FieldType(value.clone()),
            MergeValue::Required(value) => MergeValueV2::Required(*value),
            MergeValue::FieldValue(value) => MergeValueV2::FieldValue(value.clone()),
        }),
    }
}

fn add_complete_subject_conflicts(
    base: &Document,
    left: &Document,
    right: &Document,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    let schema_ids: BTreeSet<_> = base
        .schemas
        .keys()
        .chain(left.schemas.keys())
        .chain(right.schemas.keys())
        .cloned()
        .collect();
    for schema_id in schema_ids {
        let b = base.schemas.get(&schema_id);
        let l = left.schemas.get(&schema_id);
        let r = right.schemas.get(&schema_id);
        match (b, l, r) {
            (Some(b), Some(l), Some(r)) => {
                add_field_subject_conflicts(base, &schema_id, b, l, r, conflicts);
            }
            _ => {
                if subject_presence_conflicts(b, l, r) {
                    conflicts.push(MergeConflictV2::new(
                        &base.id,
                        ConflictTarget::Schema(schema_id),
                        ConflictFacetV2::Subject,
                        b.map(|schema| MergeValueV2::SchemaSubject(SchemaSubjectV2::from(schema))),
                        l.map(|schema| MergeValueV2::SchemaSubject(SchemaSubjectV2::from(schema))),
                        r.map(|schema| MergeValueV2::SchemaSubject(SchemaSubjectV2::from(schema))),
                    ));
                }
            }
        }
    }
}

fn add_field_subject_conflicts(
    base: &Document,
    schema_id: &SchemaId,
    base_schema: &Schema,
    left_schema: &Schema,
    right_schema: &Schema,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    let field_ids: BTreeSet<_> = base_schema
        .fields
        .keys()
        .chain(left_schema.fields.keys())
        .chain(right_schema.fields.keys())
        .cloned()
        .collect();
    for field_id in field_ids {
        let b = base_schema.fields.get(&field_id);
        let l = left_schema.fields.get(&field_id);
        let r = right_schema.fields.get(&field_id);
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

fn subject_presence_conflicts<T: Clone + PartialEq>(
    base: Option<&T>,
    left: Option<&T>,
    right: Option<&T>,
) -> bool {
    match (base, left, right) {
        (Some(_), Some(_), Some(_)) => false,
        _ => matches!(choose_optional(base, left, right), OptionalChoice::Conflict),
    }
}

fn add_constraint_conflicts(
    base: &Document,
    left: &Document,
    right: &Document,
    conflicts: &mut Vec<MergeConflictV2>,
) {
    for (schema_id, base_schema) in &base.schemas {
        let (Some(left_schema), Some(right_schema)) =
            (left.schemas.get(schema_id), right.schemas.get(schema_id))
        else {
            continue;
        };
        for (field_id, base_field) in &base_schema.fields {
            let (Some(left_field), Some(right_field)) = (
                left_schema.fields.get(field_id),
                right_schema.fields.get(field_id),
            ) else {
                continue;
            };
            if choose(
                &base_field.constraint,
                &left_field.constraint,
                &right_field.constraint,
            )
            .is_none()
            {
                conflicts.push(MergeConflictV2::new(
                    &base.id,
                    ConflictTarget::SchemaField {
                        schema: schema_id.clone(),
                        field: field_id.clone(),
                    },
                    ConflictFacetV2::Constraint,
                    Some(MergeValueV2::Constraint(base_field.constraint.clone())),
                    Some(MergeValueV2::Constraint(left_field.constraint.clone())),
                    Some(MergeValueV2::Constraint(right_field.constraint.clone())),
                ));
            }
        }
    }
}

fn apply_selected_constraints(
    base: &Document,
    left: &Document,
    right: &Document,
    candidate: &mut MergeCandidate,
) {
    for (schema_id, output_schema) in &mut candidate.document.schemas {
        let schemas = (
            base.schemas.get(schema_id),
            left.schemas.get(schema_id),
            right.schemas.get(schema_id),
        );
        match schemas {
            (Some(base_schema), Some(left_schema), Some(right_schema)) => {
                for (field_id, output_field) in &mut output_schema.fields {
                    let fields = (
                        base_schema.fields.get(field_id),
                        left_schema.fields.get(field_id),
                        right_schema.fields.get(field_id),
                    );
                    let selected = match fields {
                        (Some(b), Some(l), Some(r)) => {
                            choose(&b.constraint, &l.constraint, &r.constraint)
                        }
                        (b, l, r) => match choose_optional(b, l, r) {
                            OptionalChoice::Chosen(Some(field)) => Some(field.constraint.clone()),
                            OptionalChoice::Chosen(None) => None,
                            OptionalChoice::Conflict => {
                                unreachable!("conflicting optional field emitted a conflict")
                            }
                        },
                    };
                    if let Some(constraint) = selected {
                        output_field.constraint = constraint;
                    }
                }
            }
            (b, l, r) => {
                if let OptionalChoice::Chosen(Some(selected_schema)) = choose_optional(b, l, r) {
                    for (field_id, output_field) in &mut output_schema.fields {
                        if let Some(selected_field) = selected_schema.fields.get(field_id) {
                            output_field.constraint = selected_field.constraint.clone();
                        }
                    }
                }
            }
        }
    }
}
