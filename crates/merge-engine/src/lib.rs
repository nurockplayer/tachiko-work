//! Semantic three-way merge for Tachiko Work documents.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use tachiko_semantic_core::{
    Document, DocumentId, Entity, EntityId, EntityKey, FieldDefinition, FieldId, FieldKey,
    FieldType, Schema, SchemaId, SchemaKey, Value,
};

pub const SEMANTIC_CONFLICT_V1: &str = "tachiko.semantic-conflict/v1";

#[derive(Clone, Debug, PartialEq)]
pub enum MergeOutcome {
    Merged(MergeCandidate),
    Conflicted(Vec<MergeConflict>),
}

/// Conflict-free structural result awaiting workspace finalization.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeCandidate {
    document: Document,
    unmaterialized_fields: Vec<UnmaterializedStoredFact>,
}

impl MergeCandidate {
    /// Consume the candidate into its semantic state and finalization evidence.
    #[must_use]
    pub fn into_parts(self) -> (Document, Vec<UnmaterializedStoredFact>) {
        (self.document, self.unmaterialized_fields)
    }

    /// Stored facts that could not retain their schema-qualified target in the candidate state.
    #[must_use]
    pub fn unmaterialized_fields(&self) -> &[UnmaterializedStoredFact] {
        &self.unmaterialized_fields
    }
}

impl std::ops::Deref for MergeCandidate {
    type Target = Document;

    fn deref(&self) -> &Self::Target {
        &self.document
    }
}

/// A selected stored fact whose qualified target cannot be represented by the selected schema.
#[derive(Clone, Debug, PartialEq)]
pub struct UnmaterializedStoredFact {
    entity: EntityId,
    source_schema: SchemaId,
    selected_schema: SchemaId,
    field: FieldId,
    value: Value,
}

impl UnmaterializedStoredFact {
    #[must_use]
    pub fn entity(&self) -> &EntityId {
        &self.entity
    }

    #[must_use]
    pub fn source_schema(&self) -> &SchemaId {
        &self.source_schema
    }

    #[must_use]
    pub fn selected_schema(&self) -> &SchemaId {
        &self.selected_schema
    }

    #[must_use]
    pub fn field(&self) -> &FieldId {
        &self.field
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MergeConflict {
    contract: SemanticConflictContract,
    document: DocumentId,
    target: ConflictTarget,
    facet: ConflictFacet,
    kind: ConflictKind,
    base: ConflictFact,
    left: ConflictFact,
    right: ConflictFact,
}

impl MergeConflict {
    fn new(
        document: &DocumentId,
        target: ConflictTarget,
        facet: ConflictFacet,
        base: Option<MergeValue>,
        left: Option<MergeValue>,
        right: Option<MergeValue>,
    ) -> Self {
        assert!(
            target.allows(facet),
            "merge engine emitted an invalid target/facet pair"
        );
        let kind = ConflictKind::classify(base.as_ref(), left.as_ref(), right.as_ref())
            .expect("merge engine emitted facts that are not a Semantic Conflict v1 conflict");
        Self {
            contract: SemanticConflictContract::V1,
            document: document.clone(),
            target,
            facet,
            kind,
            base: ConflictFact::from(base),
            left: ConflictFact::from(left),
            right: ConflictFact::from(right),
        }
    }

    #[must_use]
    pub const fn contract(&self) -> SemanticConflictContract {
        self.contract
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
    pub const fn facet(&self) -> ConflictFacet {
        self.facet
    }

    #[must_use]
    pub const fn kind(&self) -> ConflictKind {
        self.kind
    }

    #[must_use]
    pub fn base(&self) -> &ConflictFact {
        &self.base
    }

    #[must_use]
    pub fn left(&self) -> &ConflictFact {
        &self.left
    }

    #[must_use]
    pub fn right(&self) -> &ConflictFact {
        &self.right
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticConflictContract {
    V1,
}

impl SemanticConflictContract {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => SEMANTIC_CONFLICT_V1,
        }
    }
}

impl TryFrom<&str> for SemanticConflictContract {
    type Error = UnsupportedConflictContract;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            SEMANTIC_CONFLICT_V1 => Ok(Self::V1),
            _ => Err(UnsupportedConflictContract(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedConflictContract(String);

impl std::fmt::Display for UnsupportedConflictContract {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unsupported semantic conflict contract '{}'",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedConflictContract {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictTarget {
    Document(DocumentId),
    Schema(SchemaId),
    SchemaField {
        schema: SchemaId,
        field: FieldId,
    },
    Entity(EntityId),
    StoredEntityField {
        entity: EntityId,
        schema: SchemaId,
        field: FieldId,
    },
}

impl ConflictTarget {
    const fn subject_rank(&self) -> u8 {
        match self {
            Self::Document(_) => 0,
            Self::Schema(_) => 1,
            Self::SchemaField { .. } => 2,
            Self::Entity(_) => 3,
            Self::StoredEntityField { .. } => 4,
        }
    }

    fn canonical_cmp(&self, other: &Self) -> Ordering {
        self.subject_rank()
            .cmp(&other.subject_rank())
            .then_with(|| match (self, other) {
                (Self::Document(left), Self::Document(right)) => left.cmp(right),
                (Self::Schema(left), Self::Schema(right)) => left.cmp(right),
                (
                    Self::SchemaField {
                        schema: left_schema,
                        field: left_field,
                    },
                    Self::SchemaField {
                        schema: right_schema,
                        field: right_field,
                    },
                ) => (left_schema, left_field).cmp(&(right_schema, right_field)),
                (Self::Entity(left), Self::Entity(right)) => left.cmp(right),
                (
                    Self::StoredEntityField {
                        entity: left_entity,
                        schema: left_schema,
                        field: left_field,
                    },
                    Self::StoredEntityField {
                        entity: right_entity,
                        schema: right_schema,
                        field: right_field,
                    },
                ) => (left_entity, left_schema, left_field).cmp(&(
                    right_entity,
                    right_schema,
                    right_field,
                )),
                _ => Ordering::Equal,
            })
    }

    #[must_use]
    pub const fn allows(&self, facet: ConflictFacet) -> bool {
        facet.canonical_rank(self).is_some()
    }

    /// Check that a direct facet is valid for this closed target family.
    ///
    /// # Errors
    ///
    /// Returns a typed compatibility error for an unsupported pairing.
    pub fn validate_facet(&self, facet: ConflictFacet) -> Result<(), UnsupportedTargetFacet> {
        if self.allows(facet) {
            Ok(())
        } else {
            Err(UnsupportedTargetFacet {
                target: self.clone(),
                facet,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedTargetFacet {
    pub target: ConflictTarget,
    pub facet: ConflictFacet,
}

impl std::fmt::Display for UnsupportedTargetFacet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unsupported semantic conflict target/facet pairing: {:?} / {:?}",
            self.target, self.facet
        )
    }
}

impl std::error::Error for UnsupportedTargetFacet {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictFacet {
    Subject,
    Title,
    Key,
    FieldType,
    Requiredness,
    Schema,
    StoredValue,
}

impl ConflictFacet {
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
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictKind {
    ConcurrentAddition,
    DeleteModify,
    ConcurrentChange,
}

impl ConflictKind {
    const fn canonical_rank(self) -> u8 {
        match self {
            Self::ConcurrentAddition => 0,
            Self::DeleteModify => 1,
            Self::ConcurrentChange => 2,
        }
    }

    fn classify(
        base: Option<&MergeValue>,
        left: Option<&MergeValue>,
        right: Option<&MergeValue>,
    ) -> Option<Self> {
        match (base, left, right) {
            (None, Some(_), Some(_)) => Some(Self::ConcurrentAddition),
            (Some(_), None, Some(_)) | (Some(_), Some(_), None) => Some(Self::DeleteModify),
            (Some(_), Some(_), Some(_)) => Some(Self::ConcurrentChange),
            _ => None,
        }
    }
}

impl TryFrom<&str> for ConflictKind {
    type Error = UnsupportedConflictKind;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "concurrent_addition" => Ok(Self::ConcurrentAddition),
            "delete_modify" => Ok(Self::DeleteModify),
            "concurrent_change" => Ok(Self::ConcurrentChange),
            _ => Err(UnsupportedConflictKind(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedConflictKind(String);

impl std::fmt::Display for UnsupportedConflictKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "unsupported semantic conflict kind '{}'", self.0)
    }
}

impl std::error::Error for UnsupportedConflictKind {}

#[derive(Clone, Debug, PartialEq)]
pub enum ConflictFact {
    Absent,
    Present(MergeValue),
}

impl From<Option<MergeValue>> for ConflictFact {
    fn from(value: Option<MergeValue>) -> Self {
        value.map_or(Self::Absent, Self::Present)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MergeValue {
    DocumentTitle(String),
    SchemaSubject(SchemaSubject),
    SchemaFieldSubject(SchemaFieldSubject),
    EntitySubject(EntitySubject),
    SchemaId(SchemaId),
    SchemaKey(SchemaKey),
    EntityKey(EntityKey),
    FieldKey(FieldKey),
    FieldType(FieldType),
    Required(bool),
    FieldValue(Value),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaSubject {
    pub key: SchemaKey,
    pub fields: BTreeMap<FieldId, SchemaFieldSubject>,
}

impl From<&Schema> for SchemaSubject {
    fn from(schema: &Schema) -> Self {
        Self {
            key: schema.key.clone(),
            fields: schema
                .fields
                .iter()
                .map(|(field, definition)| (field.clone(), SchemaFieldSubject::from(definition)))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaFieldSubject {
    pub key: FieldKey,
    pub field_type: FieldType,
    pub required: bool,
}

impl From<&FieldDefinition> for SchemaFieldSubject {
    fn from(field: &FieldDefinition) -> Self {
        Self {
            key: field.key.clone(),
            field_type: field.field_type.clone(),
            required: field.required,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntitySubject {
    pub key: EntityKey,
    pub schema: SchemaId,
    pub fields: BTreeMap<FieldId, Value>,
}

impl From<&Entity> for EntitySubject {
    fn from(entity: &Entity) -> Self {
        Self {
            key: entity.key.clone(),
            schema: entity.schema.clone(),
            fields: entity.fields.clone(),
        }
    }
}

/// Merge semantic changes from `ours` and `theirs` against their common `base`.
///
/// This engine owns model-level reconciliation only. Workspace-engine applies
/// semantic validation and operation-specific gates to inputs and candidates.
#[must_use]
pub fn merge(base: &Document, ours: &Document, theirs: &Document) -> MergeOutcome {
    let mut conflicts = Vec::new();
    let mut unmaterialized_fields = Vec::new();
    let title = merge_scalar(
        ConflictCoordinate::new(
            &base.id,
            ConflictTarget::Document(base.id.clone()),
            ConflictFacet::Title,
        ),
        &base.title,
        &ours.title,
        &theirs.title,
        |title| MergeValue::DocumentTitle(title.clone()),
        &mut conflicts,
    );
    let schemas = merge_schemas(
        &base.id,
        &base.schemas,
        &ours.schemas,
        &theirs.schemas,
        &mut conflicts,
    );
    let entities = merge_entities(
        &base.id,
        &base.entities,
        &ours.entities,
        &theirs.entities,
        &mut unmaterialized_fields,
        &mut conflicts,
    );

    if !conflicts.is_empty() {
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
        return MergeOutcome::Conflicted(conflicts);
    }

    let (Some(title), Some(schemas), Some(entities)) = (title, schemas, entities) else {
        unreachable!("missing merge selection must have produced a conflict")
    };
    MergeOutcome::Merged(MergeCandidate {
        document: Document {
            id: base.id.clone(),
            title,
            schemas,
            entities,
        },
        unmaterialized_fields,
    })
}

fn merge_schemas(
    document: &DocumentId,
    base: &BTreeMap<SchemaId, Schema>,
    ours: &BTreeMap<SchemaId, Schema>,
    theirs: &BTreeMap<SchemaId, Schema>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<SchemaId, Schema>> {
    let schema_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut schemas = BTreeMap::new();
    let mut complete = true;

    for schema_id in schema_ids {
        match (
            base.get(&schema_id),
            ours.get(&schema_id),
            theirs.get(&schema_id),
        ) {
            (Some(base), Some(ours), Some(theirs)) => {
                if let Some(schema) =
                    merge_schema(document, &schema_id, base, ours, theirs, conflicts)
                {
                    schemas.insert(schema_id, schema);
                } else {
                    complete = false;
                }
            }
            (base, ours, theirs) => match merge_optional(
                ConflictCoordinate::new(
                    document,
                    ConflictTarget::Schema(schema_id.clone()),
                    ConflictFacet::Subject,
                ),
                base,
                ours,
                theirs,
                |schema| MergeValue::SchemaSubject(SchemaSubject::from(schema)),
                conflicts,
            ) {
                OptionalChoice::Chosen(Some(schema)) => {
                    schemas.insert(schema_id, schema);
                }
                OptionalChoice::Chosen(None) => {}
                OptionalChoice::Conflict => complete = false,
            },
        }
    }

    complete.then_some(schemas)
}

fn merge_schema(
    document: &DocumentId,
    schema_id: &SchemaId,
    base: &Schema,
    ours: &Schema,
    theirs: &Schema,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<Schema> {
    let fields = merge_schema_fields(
        document,
        schema_id,
        &base.fields,
        &ours.fields,
        &theirs.fields,
        conflicts,
    );
    let key = merge_scalar(
        ConflictCoordinate::new(
            document,
            ConflictTarget::Schema(schema_id.clone()),
            ConflictFacet::Key,
        ),
        &base.key,
        &ours.key,
        &theirs.key,
        |key| MergeValue::SchemaKey(key.clone()),
        conflicts,
    );

    Some(Schema {
        id: schema_id.clone(),
        key: key?,
        fields: fields?,
    })
}

fn merge_schema_fields(
    document: &DocumentId,
    schema_id: &SchemaId,
    base: &BTreeMap<FieldId, FieldDefinition>,
    ours: &BTreeMap<FieldId, FieldDefinition>,
    theirs: &BTreeMap<FieldId, FieldDefinition>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<FieldId, FieldDefinition>> {
    let field_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut fields = BTreeMap::new();
    let mut complete = true;

    for field_id in field_ids {
        match (
            base.get(&field_id),
            ours.get(&field_id),
            theirs.get(&field_id),
        ) {
            (Some(base), Some(ours), Some(theirs)) => {
                if let Some(field) = merge_field_definition(
                    document, schema_id, &field_id, base, ours, theirs, conflicts,
                ) {
                    fields.insert(field_id, field);
                } else {
                    complete = false;
                }
            }
            (base, ours, theirs) => match merge_optional(
                ConflictCoordinate::new(
                    document,
                    ConflictTarget::SchemaField {
                        schema: schema_id.clone(),
                        field: field_id.clone(),
                    },
                    ConflictFacet::Subject,
                ),
                base,
                ours,
                theirs,
                |field| MergeValue::SchemaFieldSubject(SchemaFieldSubject::from(field)),
                conflicts,
            ) {
                OptionalChoice::Chosen(Some(field)) => {
                    fields.insert(field_id, field);
                }
                OptionalChoice::Chosen(None) => {}
                OptionalChoice::Conflict => complete = false,
            },
        }
    }

    complete.then_some(fields)
}

fn merge_field_definition(
    document: &DocumentId,
    schema_id: &SchemaId,
    field_id: &FieldId,
    base: &FieldDefinition,
    ours: &FieldDefinition,
    theirs: &FieldDefinition,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<FieldDefinition> {
    let target = || ConflictTarget::SchemaField {
        schema: schema_id.clone(),
        field: field_id.clone(),
    };
    let key = merge_scalar(
        ConflictCoordinate::new(document, target(), ConflictFacet::Key),
        &base.key,
        &ours.key,
        &theirs.key,
        |key| MergeValue::FieldKey(key.clone()),
        conflicts,
    );
    let field_type = merge_scalar(
        ConflictCoordinate::new(document, target(), ConflictFacet::FieldType),
        &base.field_type,
        &ours.field_type,
        &theirs.field_type,
        |field_type| MergeValue::FieldType(field_type.clone()),
        conflicts,
    );
    let required = merge_scalar(
        ConflictCoordinate::new(document, target(), ConflictFacet::Requiredness),
        &base.required,
        &ours.required,
        &theirs.required,
        |required| MergeValue::Required(*required),
        conflicts,
    );

    Some(FieldDefinition {
        id: field_id.clone(),
        key: key?,
        field_type: field_type?,
        required: required?,
    })
}

fn merge_entities(
    document: &DocumentId,
    base: &BTreeMap<EntityId, Entity>,
    ours: &BTreeMap<EntityId, Entity>,
    theirs: &BTreeMap<EntityId, Entity>,
    unmaterialized_fields: &mut Vec<UnmaterializedStoredFact>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<BTreeMap<EntityId, Entity>> {
    let entity_ids: BTreeSet<_> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();
    let mut entities = BTreeMap::new();
    let mut complete = true;

    for entity_id in entity_ids {
        match (
            base.get(&entity_id),
            ours.get(&entity_id),
            theirs.get(&entity_id),
        ) {
            (Some(base), Some(ours), Some(theirs)) => {
                if let Some(entity) = merge_entity(
                    document,
                    &entity_id,
                    base,
                    ours,
                    theirs,
                    unmaterialized_fields,
                    conflicts,
                ) {
                    entities.insert(entity_id, entity);
                } else {
                    complete = false;
                }
            }
            (base, ours, theirs) => match merge_optional(
                ConflictCoordinate::new(
                    document,
                    ConflictTarget::Entity(entity_id.clone()),
                    ConflictFacet::Subject,
                ),
                base,
                ours,
                theirs,
                |entity| MergeValue::EntitySubject(EntitySubject::from(entity)),
                conflicts,
            ) {
                OptionalChoice::Chosen(Some(entity)) => {
                    entities.insert(entity_id, entity);
                }
                OptionalChoice::Chosen(None) => {}
                OptionalChoice::Conflict => complete = false,
            },
        }
    }

    complete.then_some(entities)
}

fn merge_entity(
    document: &DocumentId,
    entity_id: &EntityId,
    base: &Entity,
    ours: &Entity,
    theirs: &Entity,
    unmaterialized_fields: &mut Vec<UnmaterializedStoredFact>,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<Entity> {
    let schema = merge_scalar(
        ConflictCoordinate::new(
            document,
            ConflictTarget::Entity(entity_id.clone()),
            ConflictFacet::Schema,
        ),
        &base.schema,
        &ours.schema,
        &theirs.schema,
        |schema| MergeValue::SchemaId(schema.clone()),
        conflicts,
    );
    let key = merge_scalar(
        ConflictCoordinate::new(
            document,
            ConflictTarget::Entity(entity_id.clone()),
            ConflictFacet::Key,
        ),
        &base.key,
        &ours.key,
        &theirs.key,
        |key| MergeValue::EntityKey(key.clone()),
        conflicts,
    );
    let mut field_evidence = FieldMergeEvidence {
        unmaterialized_fields,
        conflicts,
    };
    let fields = merge_entity_fields(
        document,
        entity_id,
        schema.as_ref(),
        base,
        ours,
        theirs,
        &mut field_evidence,
    );

    Some(Entity {
        id: entity_id.clone(),
        key: key?,
        schema: schema?,
        fields: fields?,
    })
}

struct FieldMergeEvidence<'evidence> {
    unmaterialized_fields: &'evidence mut Vec<UnmaterializedStoredFact>,
    conflicts: &'evidence mut Vec<MergeConflict>,
}

fn merge_entity_fields(
    document: &DocumentId,
    entity_id: &EntityId,
    selected_schema: Option<&SchemaId>,
    base: &Entity,
    ours: &Entity,
    theirs: &Entity,
    evidence: &mut FieldMergeEvidence<'_>,
) -> Option<BTreeMap<FieldId, Value>> {
    let field_targets: BTreeSet<_> = base
        .fields
        .keys()
        .map(|field| (base.schema.clone(), field.clone()))
        .chain(
            ours.fields
                .keys()
                .map(|field| (ours.schema.clone(), field.clone())),
        )
        .chain(
            theirs
                .fields
                .keys()
                .map(|field| (theirs.schema.clone(), field.clone())),
        )
        .collect();
    let mut fields = BTreeMap::new();
    let mut complete = selected_schema.is_some();

    for (schema_id, field_id) in field_targets {
        match merge_optional(
            ConflictCoordinate::new(
                document,
                ConflictTarget::StoredEntityField {
                    entity: entity_id.clone(),
                    schema: schema_id.clone(),
                    field: field_id.clone(),
                },
                ConflictFacet::StoredValue,
            ),
            qualified_field(base, &schema_id, &field_id),
            qualified_field(ours, &schema_id, &field_id),
            qualified_field(theirs, &schema_id, &field_id),
            |field| MergeValue::FieldValue(field.clone()),
            evidence.conflicts,
        ) {
            OptionalChoice::Chosen(Some(field)) => {
                if let Some(selected_schema) = selected_schema {
                    if selected_schema == &schema_id {
                        fields.insert(field_id, field);
                    } else {
                        evidence
                            .unmaterialized_fields
                            .push(UnmaterializedStoredFact {
                                entity: entity_id.clone(),
                                source_schema: schema_id,
                                selected_schema: selected_schema.clone(),
                                field: field_id,
                                value: field,
                            });
                    }
                }
            }
            OptionalChoice::Chosen(None) => {}
            OptionalChoice::Conflict => complete = false,
        }
    }

    complete.then_some(fields)
}

fn qualified_field<'entity>(
    entity: &'entity Entity,
    schema: &SchemaId,
    field: &FieldId,
) -> Option<&'entity Value> {
    (entity.schema == *schema)
        .then(|| entity.fields.get(field))
        .flatten()
}

struct ConflictCoordinate<'document> {
    document: &'document DocumentId,
    target: ConflictTarget,
    facet: ConflictFacet,
}

impl<'document> ConflictCoordinate<'document> {
    fn new(document: &'document DocumentId, target: ConflictTarget, facet: ConflictFacet) -> Self {
        Self {
            document,
            target,
            facet,
        }
    }
}

fn merge_scalar<T: Clone + PartialEq>(
    coordinate: ConflictCoordinate<'_>,
    base: &T,
    ours: &T,
    theirs: &T,
    value: impl Fn(&T) -> MergeValue,
    conflicts: &mut Vec<MergeConflict>,
) -> Option<T> {
    let selected = choose(base, ours, theirs);
    if selected.is_none() {
        conflicts.push(MergeConflict::new(
            coordinate.document,
            coordinate.target,
            coordinate.facet,
            Some(value(base)),
            Some(value(ours)),
            Some(value(theirs)),
        ));
    }
    selected
}

fn merge_optional<T: Clone + PartialEq>(
    coordinate: ConflictCoordinate<'_>,
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
    value: impl Fn(&T) -> MergeValue,
    conflicts: &mut Vec<MergeConflict>,
) -> OptionalChoice<T> {
    let selected = choose_optional(base, ours, theirs);
    if matches!(selected, OptionalChoice::Conflict) {
        conflicts.push(MergeConflict::new(
            coordinate.document,
            coordinate.target,
            coordinate.facet,
            base.map(&value),
            ours.map(&value),
            theirs.map(value),
        ));
    }
    selected
}

fn choose<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> Option<T> {
    if ours == theirs {
        Some(ours.clone())
    } else if ours == base {
        Some(theirs.clone())
    } else if theirs == base {
        Some(ours.clone())
    } else {
        None
    }
}

fn choose_optional<T: Clone + PartialEq>(
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
) -> OptionalChoice<T> {
    if ours == theirs {
        OptionalChoice::Chosen(ours.cloned())
    } else if ours == base {
        OptionalChoice::Chosen(theirs.cloned())
    } else if theirs == base {
        OptionalChoice::Chosen(ours.cloned())
    } else {
        OptionalChoice::Conflict
    }
}

enum OptionalChoice<T> {
    Chosen(Option<T>),
    Conflict,
}

#[cfg(test)]
mod tests {
    use super::ConflictKind;

    #[test]
    fn conflict_kind_ranks_match_semantic_conflict_v1() {
        assert_eq!(ConflictKind::ConcurrentAddition.canonical_rank(), 0);
        assert_eq!(ConflictKind::DeleteModify.canonical_rank(), 1);
        assert_eq!(ConflictKind::ConcurrentChange.canonical_rank(), 2);
    }
}
