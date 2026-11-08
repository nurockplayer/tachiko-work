//! Provisional trusted lifecycle for revision-pinned semantic proposals.
//!
//! This module implements Issue #29's snapshot-oriented proposal, review,
//! authorization, Approval, publication, and verification seam. Its Rust
//! shapes are implementation details rather than a Stable wire or SDK
//! contract. Concrete resident sessions and revision mechanics remain owned by
//! Issue #93; callers supply those mechanics through
//! [`SemanticPublicationAuthority`].
//!
//! The trusted host owns the mutable registry, principal/Grant provisioning,
//! and every [`TrustedInstant`] supplied here. Public Rust visibility is an
//! in-process host integration seam, not a client credential or transport
//! surface: an adapter must never accept a client-provided tick, and must fail
//! before calling the lifecycle when trusted time is unavailable. The concrete
//! clock and hostile-client adapter remain Issues #30 and #93
//! host/security-profile work.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use tachiko_diff_engine::{
    CANONICAL_SEMANTIC_DELTA_V2, CanonicalSemanticDelta, canonical_delta, canonical_delta_scoped,
};
use tachiko_formula_engine::Calculation;
use tachiko_semantic_core::{
    FieldConstraint, KeyedGroupedSumDefinition, KeyedGroupedSumDefinitionId,
};
use thiserror::Error;

use super::{
    ColumnCriterion, ColumnRef, ConditionalAggregate, Document, DocumentId, Entity, EntityId,
    EntityKey, Expression, FieldDefinition, FieldEditPlanning, FieldId, FieldKey, FieldRef,
    FieldType, Number, PreparedEvaluation, PreparedVerdict, SchemaId, SemanticChange,
    ValidationReport, Value, WorkspaceError, apply_field_value_candidate_with_planning,
    finalize_edit_with_validation, formula_inverse_restore_candidate_in_place,
    formula_restore_candidate_in_place, formula_update_candidate_in_place,
    unset_field_candidate_in_place,
};

/// What a run of `AppendEntity` commands has already proved about the candidate:
/// its entity keys, once its whole structure has been checked.
#[derive(Default)]
struct EntityAppendPlanning {
    keys: Option<BTreeSet<EntityKey>>,
}

#[cfg(test)]
#[path = "candidate_allocation_tests.rs"]
mod candidate_allocation_tests;

#[cfg(test)]
#[path = "name_admission_tests.rs"]
mod name_admission_tests;

#[cfg(test)]
#[path = "evaluation_memo_tests.rs"]
mod evaluation_memo_tests;

#[cfg(test)]
#[path = "disclosure_differential_tests.rs"]
mod disclosure_differential_tests;

macro_rules! opaque_text_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

opaque_text_id!(ApprovalId);
opaque_text_id!(AuthorizationDomainId);
opaque_text_id!(AuthorizationPolicyVersion);
opaque_text_id!(DocumentScopeId);
opaque_text_id!(GrantId);
opaque_text_id!(PolicyMeaningId);
opaque_text_id!(PrincipalId);
opaque_text_id!(ProposalId);
opaque_text_id!(SemanticApiContract);
opaque_text_id!(SemanticRevision);

/// Host-supplied trusted logical time used for provisional validity checks.
///
/// Only the trusted in-process host may construct or supply this value. This
/// provisional constructor is not an authorization credential and must not be
/// projected through an untrusted adapter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TrustedInstant(u64);

impl TrustedInstant {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Trusted immutable classification of one principal occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrincipalKind {
    Human,
    Delegated,
}

/// Independently authorized Semantic API actions.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AuthorizationAction {
    Query,
    Propose,
    Approve,
    Execute,
}

/// Current provisional operation-family catalogue.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationFamily {
    AppendEntity,
    RemoveEntity,
    SetFieldValue,
    FormulaReasoning,
    NumberOverrideScenario,
    FormulaUpdate,
    FormulaInverseRestore,
    AnalysisQuery,
    FieldCapabilityDiscovery,
    KeyedGroupedSumDefinition,
    /// Bounded schema-field evolution using stable field identities.
    SchemaFieldMutation,
}

/// Accepted MVP semantic mutation classes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MutationClass {
    Value,
    Formula,
    Structure,
    Schema,
    Destructive,
}

/// Closed document-local semantic scope subject.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticScope {
    Document,
    Schema(SchemaId),
    SchemaField {
        schema: SchemaId,
        field: FieldId,
    },
    Entity {
        entity: EntityId,
        schema: SchemaId,
    },
    EntityField {
        entity: EntityId,
        schema: SchemaId,
        field: FieldId,
    },
}

/// A semantic scope atom qualified by one protected document occurrence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScopedSemanticSubject {
    document_scope: DocumentScopeId,
    document: DocumentId,
    subject: SemanticScope,
}

impl ScopedSemanticSubject {
    #[must_use]
    pub fn new(
        document_scope: DocumentScopeId,
        document: DocumentId,
        subject: SemanticScope,
    ) -> Self {
        Self {
            document_scope,
            document,
            subject,
        }
    }

    #[must_use]
    pub fn document_scope(&self) -> &DocumentScopeId {
        &self.document_scope
    }

    #[must_use]
    pub fn document(&self) -> &DocumentId {
        &self.document
    }

    #[must_use]
    pub fn subject(&self) -> &SemanticScope {
        &self.subject
    }
}

/// One complete Grant binding. Dimensions are never independently crossed.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GrantRequirement {
    Query {
        family: OperationFamily,
        scope: ScopedSemanticSubject,
    },
    Mutation {
        action: AuthorizationAction,
        family: OperationFamily,
        mutation_class: MutationClass,
        scope: ScopedSemanticSubject,
    },
}

impl GrantRequirement {
    #[must_use]
    pub fn query(family: OperationFamily, scope: ScopedSemanticSubject) -> Self {
        Self::Query { family, scope }
    }

    /// Construct a non-Query relational capability requirement.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::InvalidGrantRequirement`] when `action`
    /// is Query, because Query has no mutation-class dimension.
    pub fn mutation(
        action: AuthorizationAction,
        family: OperationFamily,
        mutation_class: MutationClass,
        scope: ScopedSemanticSubject,
    ) -> Result<Self, PatchLifecycleError> {
        if action == AuthorizationAction::Query {
            return Err(PatchLifecycleError::InvalidGrantRequirement);
        }
        Ok(Self::Mutation {
            action,
            family,
            mutation_class,
            scope,
        })
    }
}

/// Immutable trusted-host Grant issuance record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    id: GrantId,
    issuer: PrincipalId,
    subject: PrincipalId,
    requirements: BTreeSet<GrantRequirement>,
    expires_at: Option<TrustedInstant>,
}

impl Grant {
    #[must_use]
    pub fn new(
        id: GrantId,
        issuer: PrincipalId,
        subject: PrincipalId,
        requirements: Vec<GrantRequirement>,
        expires_at: Option<TrustedInstant>,
    ) -> Self {
        Self {
            id,
            issuer,
            subject,
            requirements: requirements.into_iter().collect(),
            expires_at,
        }
    }

    #[must_use]
    pub fn id(&self) -> &GrantId {
        &self.id
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DisclosureRequirement {
    pub family: OperationFamily,
    pub scope: ScopedSemanticSubject,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AssociatedWriteRequirement {
    pub family: OperationFamily,
    pub mutation_class: MutationClass,
    pub scope: ScopedSemanticSubject,
}

/// Trusted relational authorization footprint for one exact proposal body.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuthorizationFootprint {
    pub disclosure_requirements: BTreeSet<DisclosureRequirement>,
    pub associated_write_requirements: BTreeSet<AssociatedWriteRequirement>,
}

impl AuthorizationFootprint {
    #[must_use]
    pub fn mutation_classes(&self) -> BTreeSet<MutationClass> {
        self.associated_write_requirements
            .iter()
            .map(|requirement| requirement.mutation_class)
            .collect()
    }
}

/// Typed stable-ID semantic command used identically by Propose and Execute.
#[derive(Clone, Debug, PartialEq)]
pub enum SemanticCommand {
    /// Append one complete independently addressable collection.
    ///
    /// The schema and its entities are admitted as one structural command so
    /// a collection copy can never publish with only part of its identity
    /// graph installed.
    AppendCollection {
        schema: super::Schema,
        entities: Vec<Entity>,
    },
    /// Remove one complete collection and its entities as one inverse action.
    RemoveCollection {
        schema: SchemaId,
        entities: Vec<EntityId>,
    },
    /// Append one typed entity with a fresh stable identity to an existing schema.
    AppendEntity {
        entity: Entity,
    },
    /// Remove one stable identity; the final batch must repair inbound references.
    RemoveEntity {
        entity: EntityId,
    },
    /// Add one scalar field, with complete values when required or sparse values for optional Text/Number.
    AppendSchemaField {
        schema: SchemaId,
        field: FieldDefinition,
        values: BTreeMap<EntityId, Value>,
    },
    /// Remove one scalar field and its exact sparse existing stored values.
    RemoveSchemaField {
        schema: SchemaId,
        field: FieldDefinition,
        values: BTreeMap<EntityId, Value>,
    },
    /// Change only the human key of one stable scalar field.
    RenameSchemaField {
        schema: SchemaId,
        field: FieldId,
        key: FieldKey,
    },
    /// Set or clear only the durable name of a continuing stable field.
    SetSchemaFieldDisplayName {
        schema: SchemaId,
        field: FieldId,
        display_name: Option<String>,
    },
    /// Change requiredness without filling or coercing any values (ADR-0045).
    SetSchemaFieldRequiredness {
        schema: SchemaId,
        field: FieldId,
        required: bool,
    },
    /// Replace the complete durable constraint facet of one stable field.
    SetFieldNumberFormat {
        schema: SchemaId,
        field: FieldId,
        number_format: Option<super::NumberFormat>,
    },
    /// Replace the durable presentation order of one schema's fields; `None` is id order.
    SetSchemaFieldOrder {
        schema: SchemaId,
        order: Option<Vec<FieldId>>,
    },
    SetFieldConstraint {
        schema: SchemaId,
        field: FieldId,
        constraint: FieldConstraint,
    },
    SetFieldValue {
        field: FieldRef,
        value: Value,
    },
    /// Remove one stored value from an optional declared field.
    ///
    /// Explicit authoring and inverse operations keep absence distinct from a
    /// scalar placeholder while still using the normal lifecycle.
    UnsetField {
        field: FieldRef,
    },
    FormulaUpdate(FormulaUpdateCommand),
    FormulaInverseRestore(FormulaInverseRestoreCommand),
    /// Reinstall one host-cleared formula over an absent optional Number slot.
    ///
    /// Formula initialization stays rejected on the generic value path; this
    /// app-private command exists so one bounded session Undo can restore the
    /// formula that a clear removed, and so the host can copy an admitted
    /// formula into a blank cell. It is host-only and shares the
    /// `FormulaInverseRestore` trust boundary: no wire decoder produces it, so
    /// it never widens the generic value path.
    FormulaRestore(FormulaRestoreCommand),
    /// Persist one complete saved keyed lookup + grouped-sum definition.
    ///
    /// The definition is a document-level structure rather than an inferred
    /// formula or view setting, so its proposal remains subject to the normal
    /// lifecycle authorization and publication boundary.
    UpsertKeyedGroupedSumDefinition {
        definition: KeyedGroupedSumDefinition,
    },
    /// Remove one saved keyed lookup + grouped-sum definition by stable ID.
    RemoveKeyedGroupedSumDefinition {
        definition: KeyedGroupedSumDefinitionId,
    },
}

impl SemanticCommand {
    #[must_use]
    pub fn set_field_value(field: FieldRef, value: Value) -> Self {
        Self::SetFieldValue { field, value }
    }

    #[must_use]
    pub fn formula_inverse_restore(field: FieldRef, value: Number) -> Self {
        Self::FormulaInverseRestore(FormulaInverseRestoreCommand::new(field, value))
    }

    /// Host-history only: the expression must be one the host read from an
    /// admitted document formula that its own clear removed. No wire decoder
    /// produces this command, and the candidate still passes full document
    /// validation, but the engine cannot prove that provenance by itself.
    #[must_use]
    pub fn formula_restore(field: FieldRef, expression: Expression) -> Self {
        Self::FormulaRestore(FormulaRestoreCommand::new(field, expression))
    }

    /// Rough heap and inline bytes this command holds, for bounding retained
    /// history. Not an encoding size.
    #[must_use]
    pub fn approx_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + command_payload_approx_bytes(self)
    }
}

/// Nested storage one command owns, without its inline size. Every variant is
/// matched, so a new variant carrying storage cannot silently weigh nothing.
fn command_payload_approx_bytes(command: &SemanticCommand) -> usize {
    match command {
        SemanticCommand::AppendCollection { schema, entities } => {
            schema_approx_bytes(schema) + entity_list_approx_bytes(entities)
        }
        SemanticCommand::RemoveCollection { schema, entities } => {
            schema.as_str().len() + entity_id_list_approx_bytes(entities)
        }
        SemanticCommand::AppendEntity { entity } => entity_approx_bytes(entity),
        SemanticCommand::RemoveEntity { entity } => entity.as_str().len(),
        SemanticCommand::AppendSchemaField {
            schema,
            field,
            values,
        }
        | SemanticCommand::RemoveSchemaField {
            schema,
            field,
            values,
        } => {
            schema.as_str().len()
                + field_definition_approx_bytes(field)
                + values_approx_bytes(values)
        }
        SemanticCommand::RenameSchemaField { schema, field, key } => {
            schema.as_str().len() + field.as_str().len() + key.as_str().len()
        }
        SemanticCommand::SetSchemaFieldDisplayName {
            schema,
            field,
            display_name,
        } => {
            schema.as_str().len()
                + field.as_str().len()
                + display_name.as_ref().map_or(0, String::len)
        }
        SemanticCommand::SetSchemaFieldRequiredness { schema, field, .. } => {
            schema.as_str().len() + field.as_str().len()
        }
        SemanticCommand::SetFieldNumberFormat {
            schema,
            field,
            number_format,
        } => {
            schema.as_str().len()
                + field.as_str().len()
                + number_format.as_ref().map_or(0, number_format_approx_bytes)
        }
        SemanticCommand::SetSchemaFieldOrder { schema, order } => {
            schema.as_str().len() + order.as_deref().map_or(0, field_id_list_approx_bytes)
        }
        SemanticCommand::SetFieldConstraint {
            schema,
            field,
            constraint,
        } => schema.as_str().len() + field.as_str().len() + constraint_approx_bytes(constraint),
        SemanticCommand::SetFieldValue { field, value } => {
            field_approx_bytes(field) + value_approx_bytes(value)
        }
        SemanticCommand::UnsetField { field } => field_approx_bytes(field),
        SemanticCommand::FormulaUpdate(command) => formula_update_approx_bytes(command),
        SemanticCommand::FormulaInverseRestore(command) => field_approx_bytes(command.target()),
        SemanticCommand::FormulaRestore(command) => {
            field_approx_bytes(command.target()) + expression_approx_bytes(command.expression())
        }
        SemanticCommand::UpsertKeyedGroupedSumDefinition { definition } => {
            definition_approx_bytes(definition)
        }
        SemanticCommand::RemoveKeyedGroupedSumDefinition { definition } => {
            definition.as_str().len()
        }
    }
}

fn entity_list_approx_bytes(entities: &[Entity]) -> usize {
    entities
        .iter()
        .map(|entity| std::mem::size_of::<Entity>() + entity_approx_bytes(entity))
        .sum()
}

fn entity_id_list_approx_bytes(entities: &[EntityId]) -> usize {
    entities
        .iter()
        .map(|entity| std::mem::size_of::<EntityId>() + entity.as_str().len())
        .sum()
}

fn field_id_list_approx_bytes(order: &[FieldId]) -> usize {
    order
        .iter()
        .map(|id| std::mem::size_of::<FieldId>() + id.as_str().len())
        .sum()
}

fn formula_update_approx_bytes(command: &FormulaUpdateCommand) -> usize {
    field_approx_bytes(command.target())
        + expression_approx_bytes(command.expression())
        + command
            .references()
            .iter()
            .map(|reference| {
                std::mem::size_of::<FieldRef>()
                    + BTREE_ENTRY_OVERHEAD
                    + field_approx_bytes(reference)
            })
            .sum::<usize>()
}

/// Estimated per-entry bookkeeping of one `BTreeMap`/`BTreeSet` node. This is
/// an estimate (tree links and node slack), never an encoding size.
const BTREE_ENTRY_OVERHEAD: usize = 64;

fn field_approx_bytes(field: &FieldRef) -> usize {
    field.entity.as_str().len() + field.field.as_str().len()
}

fn column_ref_approx_bytes(column: &ColumnRef) -> usize {
    column.schema.as_str().len() + column.field.as_str().len()
}

fn value_approx_bytes(value: &Value) -> usize {
    match value {
        Value::Number(_) | Value::Boolean(_) | Value::Date(_) => 0,
        Value::Text(text) => text.len(),
        Value::Reference(entity) => entity.as_str().len(),
        Value::Formula(expression) => expression_approx_bytes(expression),
    }
}

/// Count formula nodes iteratively and add each node's own heap; admitted
/// formulas are shallow but the walk never relies on the call stack.
fn expression_approx_bytes(expression: &Expression) -> usize {
    let mut nodes = 0usize;
    let mut nested = 0usize;
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        nodes += 1;
        nested += expression_node_approx_bytes(node);
        pending.extend(node.children());
    }
    nodes.saturating_mul(std::mem::size_of::<Expression>()) + nested
}

/// Heap owned directly by one expression node, without its child nodes.
fn expression_node_approx_bytes(node: &Expression) -> usize {
    match node {
        Expression::Reference(field) => field_approx_bytes(field),
        Expression::SameRowReference(field) => field.as_str().len(),
        Expression::ColumnTotal(column) | Expression::ColumnAggregate { column, .. } => {
            column_ref_approx_bytes(column)
        }
        Expression::ColumnConditional {
            aggregate,
            criteria,
        } => {
            std::mem::size_of::<ConditionalAggregate>()
                + match aggregate.as_ref() {
                    ConditionalAggregate::Sum(column) | ConditionalAggregate::Average(column) => {
                        column_ref_approx_bytes(column)
                    }
                    ConditionalAggregate::Count => 0,
                }
                + criteria
                    .iter()
                    .map(|criterion| {
                        std::mem::size_of::<ColumnCriterion>()
                            + column_ref_approx_bytes(&criterion.column)
                    })
                    .sum::<usize>()
        }
        Expression::Number(_)
        | Expression::Add { .. }
        | Expression::Subtract { .. }
        | Expression::Multiply { .. }
        | Expression::Divide { .. }
        | Expression::Minimum { .. }
        | Expression::Maximum { .. }
        | Expression::Compare { .. }
        | Expression::If { .. }
        | Expression::And(_)
        | Expression::Or(_)
        | Expression::Not(_)
        | Expression::Round { .. }
        | Expression::IsBlank(_) => 0,
    }
}

fn constraint_approx_bytes(constraint: &FieldConstraint) -> usize {
    match constraint {
        FieldConstraint::None | FieldConstraint::NumberInclusiveRange { .. } => 0,
        FieldConstraint::TextLiteralSet { values } => values
            .iter()
            .map(|value| std::mem::size_of::<String>() + value.len())
            .sum(),
    }
}

fn number_format_approx_bytes(number_format: &super::NumberFormat) -> usize {
    match &number_format.kind {
        super::NumberFormatKind::Plain | super::NumberFormatKind::Percent => 0,
        super::NumberFormatKind::Currency { code } => code.len(),
    }
}

fn field_definition_approx_bytes(field: &FieldDefinition) -> usize {
    field.id.as_str().len()
        + field.key.as_str().len()
        + field.display_name.as_ref().map_or(0, String::len)
        + constraint_approx_bytes(&field.constraint)
        + field
            .number_format
            .as_ref()
            .map_or(0, number_format_approx_bytes)
}

fn schema_approx_bytes(schema: &super::Schema) -> usize {
    schema.id.as_str().len()
        + schema.key.as_str().len()
        + schema.display_name.as_ref().map_or(0, String::len)
        + schema
            .fields
            .values()
            .map(|field| {
                std::mem::size_of::<FieldId>()
                    + std::mem::size_of::<FieldDefinition>()
                    + BTREE_ENTRY_OVERHEAD
                    + field_definition_approx_bytes(field)
            })
            .sum::<usize>()
        + schema.field_order.as_ref().map_or(0, |order| {
            order
                .iter()
                .map(|id| std::mem::size_of::<FieldId>() + id.as_str().len())
                .sum()
        })
}

fn entity_approx_bytes(entity: &Entity) -> usize {
    entity.id.as_str().len()
        + entity.key.as_str().len()
        + entity.schema.as_str().len()
        + entity
            .fields
            .iter()
            .map(|(field, value)| {
                std::mem::size_of::<FieldId>()
                    + std::mem::size_of::<Value>()
                    + BTREE_ENTRY_OVERHEAD
                    + field.as_str().len()
                    + value_approx_bytes(value)
            })
            .sum::<usize>()
}

fn values_approx_bytes(values: &BTreeMap<EntityId, Value>) -> usize {
    values
        .iter()
        .map(|(entity, value)| {
            std::mem::size_of::<EntityId>()
                + std::mem::size_of::<Value>()
                + BTREE_ENTRY_OVERHEAD
                + entity.as_str().len()
                + value_approx_bytes(value)
        })
        .sum()
}

fn definition_approx_bytes(definition: &KeyedGroupedSumDefinition) -> usize {
    definition.id.as_str().len()
        + definition.orders.schema.as_str().len()
        + definition.orders.lookup_key_field.as_str().len()
        + definition.orders.quantity_field.as_str().len()
        + definition.products.schema.as_str().len()
        + definition.products.key_field.as_str().len()
        + definition.products.category_field.as_str().len()
        + definition.products.price_field.as_str().len()
}

/// One admitted formula-update Command with complete bound meaning.
///
/// The fields stay private so authoring text cannot bypass the trusted
/// parse/bind/type-check admission boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaUpdateCommand {
    target: FieldRef,
    expression: Expression,
    references: BTreeSet<FieldRef>,
}

impl FormulaUpdateCommand {
    pub(crate) fn new(document: &Document, target: FieldRef, expression: Expression) -> Self {
        let references =
            tachiko_formula_engine::extract_dependencies(document, &target, &expression);
        Self {
            target,
            expression,
            references,
        }
    }

    #[must_use]
    pub fn target(&self) -> &FieldRef {
        &self.target
    }

    #[must_use]
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    #[must_use]
    pub fn references(&self) -> &BTreeSet<FieldRef> {
        &self.references
    }
}

/// One app-private replacement of a stored formula with a scalar: either the
/// inverse of a prior `FormulaUpdate` or a host-deliberate scalar edit over a
/// formula cell.
///
/// This command is deliberately separate from generic `SetFieldValue`: it can
/// write only a Number over a currently stored formula through the ordinary
/// lifecycle, and does not make formula cells generally scalar-editable.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaInverseRestoreCommand {
    target: FieldRef,
    value: Number,
}

impl FormulaInverseRestoreCommand {
    pub(crate) fn new(target: FieldRef, value: Number) -> Self {
        Self { target, value }
    }

    #[must_use]
    pub fn target(&self) -> &FieldRef {
        &self.target
    }

    #[must_use]
    pub fn value(&self) -> Number {
        self.value
    }
}

/// One app-private installation of an admitted formula into a blank cell: a
/// formula the host cleared, or a host copy of a document formula.
///
/// The expression can only come from an already-admitted document formula, so
/// this command never parses or binds authoring text; it stays separate from
/// generic `SetFieldValue` so an absent slot cannot become a formula through
/// the ordinary value path.
///
/// Host-only, with the same trust boundary as
/// [`FormulaInverseRestoreCommand`]: the host constructs it from history it
/// recorded or from a formula already in the document, no wire decoder
/// produces it, and it is never reachable from authoring input.
#[derive(Clone, Debug, PartialEq)]
pub struct FormulaRestoreCommand {
    target: FieldRef,
    expression: Expression,
}

impl FormulaRestoreCommand {
    pub(crate) fn new(target: FieldRef, expression: Expression) -> Self {
        Self { target, expression }
    }

    #[must_use]
    pub fn target(&self) -> &FieldRef {
        &self.target
    }

    #[must_use]
    pub fn expression(&self) -> &Expression {
        &self.expression
    }
}

/// One Command or one ordered, non-empty `AtomicBatch`.
#[derive(Clone, Debug, PartialEq)]
pub enum SemanticPatchBody {
    Command(SemanticCommand),
    AtomicBatch(AtomicBatch),
}

/// Ordered non-empty semantic command batch with private membership storage.
#[derive(Clone, Debug, PartialEq)]
pub struct AtomicBatch {
    commands: Vec<SemanticCommand>,
}

impl SemanticPatchBody {
    #[must_use]
    pub fn command(command: SemanticCommand) -> Self {
        Self::Command(command)
    }

    /// Construct a non-empty ordered `AtomicBatch`.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::EmptyAtomicBatch`] for an empty batch.
    pub fn atomic_batch(commands: Vec<SemanticCommand>) -> Result<Self, PatchLifecycleError> {
        if commands.is_empty() {
            return Err(PatchLifecycleError::EmptyAtomicBatch);
        }
        Ok(Self::AtomicBatch(AtomicBatch { commands }))
    }

    fn commands(&self) -> &[SemanticCommand] {
        match self {
            Self::Command(command) => std::slice::from_ref(command),
            Self::AtomicBatch(batch) => &batch.commands,
        }
    }
}

/// Representation-neutral exact semantic change/base binding.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactChangeBinding {
    semantic_api_contract: SemanticApiContract,
    base_revision: SemanticRevision,
    body: SemanticPatchBody,
    evidence_profile: EvidenceProfile,
}

impl ExactChangeBinding {
    #[must_use]
    pub fn semantic_api_contract(&self) -> &SemanticApiContract {
        &self.semantic_api_contract
    }

    #[must_use]
    pub fn base_revision(&self) -> &SemanticRevision {
        &self.base_revision
    }

    #[must_use]
    pub fn body(&self) -> &SemanticPatchBody {
        &self.body
    }

    #[must_use]
    fn evidence_profile(&self) -> EvidenceProfile {
        self.evidence_profile
    }
}

/// Construction-bound, closed evidence profile for one lifecycle occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceProfile {
    V1,
    V2,
}

/// Immutable revision-pinned proposal occurrence.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticPatch {
    id: ProposalId,
    exact_change: ExactChangeBinding,
}

impl SemanticPatch {
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        &self.id
    }

    #[must_use]
    pub fn exact_change(&self) -> &ExactChangeBinding {
        &self.exact_change
    }
}

/// Inputs supplied to the trusted proposal-issuance boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalRequest {
    id: ProposalId,
    base_revision: SemanticRevision,
    body: SemanticPatchBody,
    originator: PrincipalId,
}

impl ProposalRequest {
    #[must_use]
    pub fn new(
        id: ProposalId,
        base_revision: SemanticRevision,
        body: SemanticPatchBody,
        originator: PrincipalId,
    ) -> Self {
        Self {
            id,
            base_revision,
            body,
            originator,
        }
    }
}

/// External lifecycle evidence associated with an immutable proposal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchLifecycleState {
    Draft,
    Planned,
    Previewed,
    Validated,
    AwaitingApproval,
    Approved,
    Applied,
    Verified,
    Rejected,
    ValidationFailed,
    Stale,
    /// Proved no-publication host conflict; retry remains possible.
    RetryableConflict,
    /// Terminal integrity or post-publication verification conflict.
    Conflict,
    Expired,
}

/// Mutation-risk projection derived from accepted mutation classes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchRisk {
    pub mutation_classes: BTreeSet<MutationClass>,
}

/// Machine-readable formula dependency/impact evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormulaImpactEvidence {
    pub field: FieldRef,
    pub before: Number,
    pub after: Number,
    pub causes: Vec<FieldRef>,
}

/// Disclosure-scoped review evidence for one current proposal.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchPreview {
    pub proposal: SemanticPatch,
    pub semantic_changes: Vec<SemanticChange>,
    pub formula_impacts: Vec<FormulaImpactEvidence>,
    pub validation_report: ValidationReport,
    pub authorization_footprint: AuthorizationFootprint,
    pub risk: PatchRisk,
}

/// Stored value and effective numeric calculation for one constrained field instance.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintAffectedValue {
    pub field: FieldRef,
    pub stored: Value,
    pub calculated_number: Option<Number>,
}

/// Complete before/after instance evidence for one changed field constraint.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintReviewEvidence {
    pub schema: SchemaId,
    pub field: FieldId,
    pub before: Vec<ConstraintAffectedValue>,
    pub after: Vec<ConstraintAffectedValue>,
}

/// Disclosure-authorized v2 review evidence with a complete canonical delta.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchPreviewV2 {
    pub proposal: SemanticPatch,
    pub delta: CanonicalSemanticDelta,
    pub constraint_reviews: Vec<ConstraintReviewEvidence>,
    pub formula_impacts: Vec<FormulaImpactEvidence>,
    pub validation_report: ValidationReport,
    pub authorization_footprint: AuthorizationFootprint,
    pub risk: PatchRisk,
}

/// Trusted proposal authorization/provenance evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalProvenance {
    pub authorization_domain: AuthorizationDomainId,
    pub proposal_id: ProposalId,
    pub exact_change: ExactChangeBinding,
    pub originator: PrincipalId,
    pub propose_grants: BTreeSet<GrantId>,
    pub authorization_footprint: AuthorizationFootprint,
    pub policy_version: AuthorizationPolicyVersion,
}

/// Finite one-shot Approval state in the trusted registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalStatus {
    Active,
    Consumed,
    Revoked,
    Expired,
}

/// Complete Accepted structural Approval binding.
#[derive(Clone, Debug, PartialEq)]
pub struct ApprovalBinding {
    authorization_domain: AuthorizationDomainId,
    proposal_id: ProposalId,
    exact_change: ExactChangeBinding,
    originator: PrincipalId,
    executor: PrincipalId,
    associated_write_requirements: BTreeSet<AssociatedWriteRequirement>,
    policy_version: AuthorizationPolicyVersion,
}

/// Immutable Approval occurrence returned by the issuance boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct Approval {
    id: ApprovalId,
    binding: ApprovalBinding,
    approver: PrincipalId,
    issued_at: TrustedInstant,
    expires_at: TrustedInstant,
    approve_grants: BTreeSet<GrantId>,
}

impl Approval {
    #[must_use]
    pub fn id(&self) -> &ApprovalId {
        &self.id
    }

    #[must_use]
    pub fn binding(&self) -> &ApprovalBinding {
        &self.binding
    }

    #[must_use]
    pub fn approver(&self) -> &PrincipalId {
        &self.approver
    }

    #[must_use]
    pub const fn issued_at(&self) -> TrustedInstant {
        self.issued_at
    }

    #[must_use]
    pub const fn expires_at(&self) -> TrustedInstant {
        self.expires_at
    }

    #[must_use]
    pub fn approve_grants(&self) -> &BTreeSet<GrantId> {
        &self.approve_grants
    }
}

/// Inputs to exact Human Approval issuance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRequest {
    id: ApprovalId,
    proposal_id: ProposalId,
    approver: PrincipalId,
    executor: PrincipalId,
    expires_at: TrustedInstant,
}

impl ApprovalRequest {
    #[must_use]
    pub fn new(
        id: ApprovalId,
        proposal_id: ProposalId,
        approver: PrincipalId,
        executor: PrincipalId,
        expires_at: TrustedInstant,
    ) -> Self {
        Self {
            id,
            proposal_id,
            approver,
            executor,
            expires_at,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalExecutionEvidence {
    pub approval_id: ApprovalId,
    pub approver: PrincipalId,
    pub status: ApprovalStatus,
}

/// Verified trusted receipt or disclosure-reduced execution response.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionReceipt {
    pub proposal_id: ProposalId,
    pub originator: PrincipalId,
    pub executor: PrincipalId,
    pub approval: Option<ApprovalExecutionEvidence>,
    pub propose_grants: BTreeSet<GrantId>,
    pub approve_grants: BTreeSet<GrantId>,
    pub execute_grants: BTreeSet<GrantId>,
    pub authorization_footprint: Option<AuthorizationFootprint>,
    pub policy_version: AuthorizationPolicyVersion,
    pub base_revision: SemanticRevision,
    pub resulting_revision: SemanticRevision,
    pub verified: bool,
    pub semantic_changes: Vec<SemanticChange>,
    pub formula_impacts: Vec<FormulaImpactEvidence>,
    pub validation_report: Option<ValidationReport>,
}

/// Explicitly disclosed or redacted v2 evidence in an execution response.
#[derive(Clone, Debug, PartialEq)]
pub enum V2ReceiptEvidence {
    Disclosed(CanonicalSemanticDelta),
    Redacted,
}

/// Verified v2 execution receipt; direct facts are accessible only with Query.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionReceiptV2 {
    pub proposal_id: ProposalId,
    pub originator: PrincipalId,
    pub executor: PrincipalId,
    pub approval: Option<ApprovalExecutionEvidence>,
    pub propose_grants: BTreeSet<GrantId>,
    pub approve_grants: BTreeSet<GrantId>,
    pub execute_grants: BTreeSet<GrantId>,
    pub authorization_footprint: Option<AuthorizationFootprint>,
    pub policy_version: AuthorizationPolicyVersion,
    pub base_revision: SemanticRevision,
    pub resulting_revision: SemanticRevision,
    pub verified: bool,
    evidence: V2ReceiptEvidence,
    pub formula_impacts: Vec<FormulaImpactEvidence>,
    pub validation_report: Option<ValidationReport>,
}

impl ExecutionReceiptV2 {
    /// Return the explicit full or redacted evidence selected for this receipt.
    #[must_use]
    pub fn evidence(&self) -> &V2ReceiptEvidence {
        &self.evidence
    }

    /// Return the complete base-to-result delta when the receipt is disclosable.
    ///
    /// # Errors
    ///
    /// Returns `DisclosureDenied` for a receipt reduced for an executor without
    /// current Query authority.
    pub fn delta(&self) -> Result<&CanonicalSemanticDelta, PatchLifecycleError> {
        match &self.evidence {
            V2ReceiptEvidence::Disclosed(delta) => Ok(delta),
            V2ReceiptEvidence::Redacted => Err(PatchLifecycleError::DisclosureDenied),
        }
    }
}

/// Host/runtime seam that owns concrete revision and state-install mechanics.
///
/// An error from `publish_if_current` must prove that the candidate was not
/// published. The lifecycle holds exclusive proposal/Approval state while this
/// method runs and consumes Approval immediately after a successful return.
pub trait SemanticPublicationAuthority {
    /// Capture one coherent protected-occurrence/document/revision tuple.
    ///
    /// The document is borrowed: a reader that only inspects the current
    /// occurrence must not copy it.
    fn current_snapshot(&self) -> (DocumentScopeId, &Document, SemanticRevision);

    /// Resolve one publication attempt under an exclusive guard. First compare
    /// `expected_document_scope`; an occurrence change returns
    /// [`SemanticPublicationError::DocumentScopeMismatch`] without invoking
    /// authorization for the old occurrence. Otherwise the callback must run
    /// exactly once with a fresh trusted instant inside that guard, immediately
    /// before returning a semantic Stale/Conflict/NoChange outcome or installing the
    /// candidate. Install only while `expected_revision` remains current and
    /// the callback accepts, then capture and return the exact installed
    /// document occurrence, immutable document snapshot, distinct resulting
    /// revision, and the callback's authorization evidence before releasing
    /// the guard. A later publication may advance [`Self::current_snapshot`]
    /// without changing this successful result.
    ///
    /// The candidate is borrowed so the lifecycle keeps the exact evaluated
    /// occurrence for post-publication verification; the authority installs
    /// its own copy only on success.
    ///
    /// After authorization and exact-base checks, compare the complete typed
    /// candidate with the protected current Document using stored semantic
    /// equality. Equality returns [`SemanticPublicationError::NoChange`]
    /// without installation, revision advancement, or publication invalidation.
    /// A diff, calculated output, or projection is not equality authority.
    ///
    /// # Errors
    ///
    /// Every error must prove that `candidate` was not published. In
    /// particular, a callback returning `None` must produce
    /// [`SemanticPublicationError::AuthorizationDenied`]. A semantic failure
    /// returned without invoking the callback is treated as undisclosable by
    /// the lifecycle even though such a host violates this contract.
    fn publish_if_current<Authorization>(
        &mut self,
        expected_document_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: &Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<Authorization>,
    ) -> Result<
        (DocumentScopeId, &Document, SemanticRevision, Authorization),
        SemanticPublicationError,
    >;
}

/// Proved no-publication result from the host publication boundary.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SemanticPublicationError {
    #[error("the final semantic candidate is unchanged; nothing was published")]
    NoChange,
    #[error("protected document occurrence changed")]
    DocumentScopeMismatch,
    #[error("semantic base is stale")]
    Stale,
    #[error("semantic publication conflicted")]
    Conflict,
    #[error("publication-boundary authorization was denied")]
    AuthorizationDenied,
}

/// Machine-distinguishable lifecycle outcomes. Exact Rust names are
/// provisional; the Accepted semantic meanings are preserved.
#[derive(Debug, Error)]
pub enum PatchLifecycleError {
    /// Authorized final-state equality, not a publication receipt. This
    /// provisional Result mapping leaves the exact proposal retryable; each
    /// attempt must repeat all current checks, including disclosure.
    #[error("the final semantic candidate is unchanged; nothing was published")]
    NoChange,
    #[error("the AtomicBatch must contain at least one command")]
    EmptyAtomicBatch,
    #[error("proposal evidence profile does not match this review or execution entry point")]
    EvidenceProfileMismatch,
    #[error("a batch writes the same schema-field constraint facet more than once")]
    DuplicateFieldConstraintWrite { schema: SchemaId, field: FieldId },
    #[error("Query requirements cannot carry a mutation class")]
    InvalidGrantRequirement,
    #[error("the configured document occurrence does not match the supplied document")]
    DocumentScopeMismatch,
    #[error("principal is unknown")]
    UnknownPrincipal,
    #[error("principal is disabled")]
    PrincipalDisabled,
    #[error("principal identity was already registered")]
    PrincipalIdAlreadyExists,
    #[error("principal kind is immutable for one occurrence")]
    PrincipalKindMismatch,
    #[error("Grant identity was already issued")]
    GrantIdAlreadyExists,
    #[error("Grant is empty, malformed, or refers to an invalid principal")]
    InvalidGrant,
    #[error("Grant does not exist")]
    GrantNotFound,
    #[error("proposal identity was already issued")]
    ProposalIdAlreadyExists,
    #[error("proposal does not exist")]
    ProposalNotFound,
    #[error("proposal is not executable")]
    ProposalNotExecutable,
    #[error("semantic proposal base is stale")]
    Stale,
    #[error("semantic command was rejected: {source}")]
    CommandRejected {
        #[source]
        source: Box<WorkspaceError>,
    },
    #[error("authoritative semantic validation rejected the candidate")]
    ValidationFailed { report: ValidationReport },
    #[error("{action:?} capability is insufficient")]
    InsufficientCapability { action: AuthorizationAction },
    #[error("semantic review disclosure is not authorized")]
    DisclosureDenied,
    #[error("approver must be an authenticated Human principal")]
    ApproverMustBeHuman,
    #[error("the named approver has not received an authorized current preview")]
    ReviewRequired,
    #[error("Approval expiry must be finite and later than issuance")]
    InvalidApprovalExpiry,
    #[error("Approval identity was already issued")]
    ApprovalIdAlreadyExists,
    #[error("Approval does not exist")]
    ApprovalNotFound,
    #[error("exact Human Approval is required")]
    ApprovalRequired,
    #[error("exact Human Approval is not required for this proposal and executor")]
    ApprovalNotRequired,
    #[error("Approval does not bind this exact proposal and executor")]
    ApprovalBindingMismatch,
    #[error("a semantic scope requirement could not be derived from the exact change")]
    ScopeDerivationFailed,
    #[error("Approval expired")]
    ApprovalExpired,
    #[error("Approval was revoked")]
    ApprovalRevoked,
    #[error("Approval was already consumed")]
    ApprovalConsumed,
    #[error("authorization is denied")]
    AuthorizationDenied,
    #[error("authorization policy changed or continuity was lost")]
    AuthorizationPolicyChanged,
    #[error("an authorization-policy identifier was reused for changed meaning")]
    PolicyMeaningConflict,
    #[error("authorization-policy selection continuity can no longer be represented")]
    PolicySelectionExhausted,
    #[error("semantic publication conflicted")]
    Conflict,
    #[error("post-publication semantic verification failed")]
    VerificationFailed,
}

#[derive(Clone, Debug)]
struct PrincipalRecord {
    kind: PrincipalKind,
    active: bool,
}

#[derive(Clone, Debug)]
struct StoredGrant {
    grant: Grant,
    revoked: bool,
}

#[derive(Clone, Debug)]
struct ProposalRecord {
    patch: SemanticPatch,
    originator: PrincipalId,
    footprint: Option<AuthorizationFootprint>,
    propose_grants: BTreeSet<GrantId>,
    policy_version: AuthorizationPolicyVersion,
    history: Vec<PatchLifecycleState>,
    reviewed_by: BTreeSet<PrincipalId>,
    /// Issuance order inside this registry; never part of semantic meaning.
    sequence: u64,
}

#[derive(Clone, Debug)]
struct StoredApproval {
    approval: Approval,
    status: ApprovalStatus,
    policy_selection: u64,
}

#[derive(Clone, Debug)]
struct EvaluatedPatch {
    document: Document,
    evidence: PatchEvidence,
    formula_impacts: Vec<FormulaImpactEvidence>,
    validation_report: ValidationReport,
    footprint: AuthorizationFootprint,
}

/// The last successful evaluation, reused only for an identical base document and patch.
struct EvaluationMemo {
    base: Document,
    patch: SemanticPatch,
    evaluated: EvaluatedPatch,
}

enum ProfiledPreview {
    V1(PatchPreview),
    V2(PatchPreviewV2),
}

enum ProfiledReceipt {
    V1(ExecutionReceipt),
    V2(ExecutionReceiptV2),
}

/// Exhaustive profile-owned review evidence; v2 never travels through a v1 list.
#[derive(Clone, Debug, PartialEq)]
enum PatchEvidence {
    V1(Vec<SemanticChange>),
    V2(CanonicalSemanticDelta),
}

struct PublicationAuthorization {
    approve_grants: BTreeSet<GrantId>,
    execute_grants: BTreeSet<GrantId>,
    can_disclose: bool,
}

struct PublicationAttempt<'a> {
    proposal_id: &'a ProposalId,
    proposal: &'a ProposalRecord,
    approval: Option<StoredApproval>,
    executor: &'a PrincipalId,
    footprint: &'a AuthorizationFootprint,
    current_document_scope: &'a DocumentScopeId,
    current_revision: &'a SemanticRevision,
    candidate: &'a Document,
}

struct PublishedCandidate<'authority> {
    document_scope: DocumentScopeId,
    document: &'authority Document,
    revision: SemanticRevision,
    approval: Option<ApprovalExecutionEvidence>,
    authorization: PublicationAuthorization,
}

/// Provisional trusted in-process proposal and Approval lifecycle registry.
pub struct PatchLifecycle {
    authorization_domain: AuthorizationDomainId,
    document_scope: DocumentScopeId,
    document: DocumentId,
    semantic_api_contract: SemanticApiContract,
    evidence_profile: EvidenceProfile,
    principals: BTreeMap<PrincipalId, PrincipalRecord>,
    grants: BTreeMap<GrantId, StoredGrant>,
    proposals: BTreeMap<ProposalId, ProposalRecord>,
    approvals: BTreeMap<ApprovalId, StoredApproval>,
    execution_receipts: Vec<ExecutionReceipt>,
    execution_receipts_v2: Vec<ExecutionReceiptV2>,
    /// Monotonic issuance order, so retention can keep the newest proposals
    /// even when their opaque textual ids do not sort by age.
    next_proposal_sequence: u64,
    policy_meanings: BTreeMap<AuthorizationPolicyVersion, PolicyMeaningId>,
    effective_policy: AuthorizationPolicyVersion,
    policy_selection: u64,
    evaluation_memo: Option<EvaluationMemo>,
    #[cfg(test)]
    evaluations: std::cell::Cell<u32>,
}

impl PatchLifecycle {
    #[must_use]
    pub fn new(
        authorization_domain: AuthorizationDomainId,
        document_scope: DocumentScopeId,
        document: DocumentId,
        semantic_api_contract: SemanticApiContract,
        effective_policy: AuthorizationPolicyVersion,
        policy_meaning: PolicyMeaningId,
    ) -> Self {
        Self::new_with_profile(
            authorization_domain,
            document_scope,
            document,
            semantic_api_contract,
            effective_policy,
            policy_meaning,
            EvidenceProfile::V1,
        )
    }

    /// Construct a lifecycle occurrence explicitly bound to canonical v2 evidence.
    #[must_use]
    pub fn new_v2(
        authorization_domain: AuthorizationDomainId,
        document_scope: DocumentScopeId,
        document: DocumentId,
        semantic_api_contract: SemanticApiContract,
        effective_policy: AuthorizationPolicyVersion,
        policy_meaning: PolicyMeaningId,
    ) -> Self {
        Self::new_with_profile(
            authorization_domain,
            document_scope,
            document,
            semantic_api_contract,
            effective_policy,
            policy_meaning,
            EvidenceProfile::V2,
        )
    }

    /// Whether proposals in this registry bind the v2 evidence vocabulary.
    /// Entry-point selection must follow the registry, even when a publication
    /// removes the last fact that required v2.
    #[must_use]
    pub fn uses_v2_evidence(&self) -> bool {
        self.evidence_profile == EvidenceProfile::V2
    }

    /// Create a request-local v2 registry using this registry's current trusted
    /// principal, grant and policy state. No proposal, Approval or receipt is
    /// copied or reinterpreted. The synchronous caller must finish the request
    /// before changing the source registry's authorization state.
    #[must_use]
    pub fn fork_for_v2_request(&self) -> Self {
        Self {
            authorization_domain: self.authorization_domain.clone(),
            document_scope: self.document_scope.clone(),
            document: self.document.clone(),
            semantic_api_contract: self.semantic_api_contract.clone(),
            evidence_profile: EvidenceProfile::V2,
            principals: self.principals.clone(),
            grants: self.grants.clone(),
            proposals: BTreeMap::new(),
            approvals: BTreeMap::new(),
            execution_receipts: Vec::new(),
            execution_receipts_v2: Vec::new(),
            next_proposal_sequence: 0,
            policy_meanings: self.policy_meanings.clone(),
            effective_policy: self.effective_policy.clone(),
            policy_selection: self.policy_selection,
            evaluation_memo: None,
            #[cfg(test)]
            evaluations: std::cell::Cell::new(0),
        }
    }

    fn new_with_profile(
        authorization_domain: AuthorizationDomainId,
        document_scope: DocumentScopeId,
        document: DocumentId,
        semantic_api_contract: SemanticApiContract,
        effective_policy: AuthorizationPolicyVersion,
        policy_meaning: PolicyMeaningId,
        evidence_profile: EvidenceProfile,
    ) -> Self {
        Self {
            authorization_domain,
            document_scope,
            document,
            semantic_api_contract,
            evidence_profile,
            principals: BTreeMap::new(),
            grants: BTreeMap::new(),
            proposals: BTreeMap::new(),
            approvals: BTreeMap::new(),
            execution_receipts: Vec::new(),
            execution_receipts_v2: Vec::new(),
            next_proposal_sequence: 0,
            policy_meanings: BTreeMap::from([(effective_policy.clone(), policy_meaning)]),
            effective_policy,
            policy_selection: 0,
            evaluation_memo: None,
            #[cfg(test)]
            evaluations: std::cell::Cell::new(0),
        }
    }

    /// Register one trusted non-reusable principal occurrence.
    ///
    /// # Errors
    ///
    /// Rejects identity reuse, including attempted kind reclassification.
    pub fn register_principal(
        &mut self,
        id: PrincipalId,
        kind: PrincipalKind,
    ) -> Result<(), PatchLifecycleError> {
        if let Some(existing) = self.principals.get(&id) {
            return Err(if existing.kind == kind {
                PatchLifecycleError::PrincipalIdAlreadyExists
            } else {
                PatchLifecycleError::PrincipalKindMismatch
            });
        }
        self.principals
            .insert(id, PrincipalRecord { kind, active: true });
        Ok(())
    }

    /// Whether one trusted principal occurrence is active and Delegated.
    ///
    /// This host-composition check intentionally collapses unknown, disabled,
    /// and non-Delegated occurrences to `false`; callers must not use it as a
    /// client-visible principal-discovery surface.
    #[must_use]
    pub fn is_active_delegated_principal(&self, id: &PrincipalId) -> bool {
        self.principals
            .get(id)
            .is_some_and(|principal| principal.active && principal.kind == PrincipalKind::Delegated)
    }

    /// Disable one principal occurrence without reassigning its identity.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::UnknownPrincipal`] for an unknown ID.
    pub fn disable_principal(&mut self, id: &PrincipalId) -> Result<(), PatchLifecycleError> {
        self.principals
            .get_mut(id)
            .ok_or(PatchLifecycleError::UnknownPrincipal)?
            .active = false;
        Ok(())
    }

    /// Provision one immutable Grant through the trusted host boundary.
    ///
    /// # Errors
    ///
    /// Rejects reused IDs, empty or malformed Grants, disabled or Delegated
    /// issuers, or unresolved issuer/subject occurrences.
    pub fn provision_grant(&mut self, grant: Grant) -> Result<(), PatchLifecycleError> {
        if self.grants.contains_key(&grant.id) {
            return Err(PatchLifecycleError::GrantIdAlreadyExists);
        }
        if grant.requirements.is_empty()
            || grant.requirements.iter().any(|requirement| {
                matches!(
                    requirement,
                    GrantRequirement::Mutation {
                        action: AuthorizationAction::Query,
                        ..
                    }
                )
            })
            || self
                .principals
                .get(&grant.issuer)
                .is_none_or(|issuer| !issuer.active || issuer.kind == PrincipalKind::Delegated)
            || !self.principals.contains_key(&grant.subject)
        {
            return Err(PatchLifecycleError::InvalidGrant);
        }
        self.grants.insert(
            grant.id.clone(),
            StoredGrant {
                grant,
                revoked: false,
            },
        );
        Ok(())
    }

    /// Terminally revoke one Grant occurrence.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::GrantNotFound`] when absent.
    pub fn revoke_grant(&mut self, id: &GrantId) -> Result<(), PatchLifecycleError> {
        self.grants
            .get_mut(id)
            .ok_or(PatchLifecycleError::GrantNotFound)?
            .revoked = true;
        Ok(())
    }

    /// Select a new effective policy occurrence.
    ///
    /// Every call is a transition, including rollback to a previously selected
    /// version. Reusing a version for different meaning fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::PolicyMeaningConflict`] for identifier
    /// reuse with changed meaning, or
    /// [`PatchLifecycleError::PolicySelectionExhausted`] rather than losing
    /// transition continuity.
    pub fn transition_effective_policy(
        &mut self,
        version: AuthorizationPolicyVersion,
        meaning: PolicyMeaningId,
    ) -> Result<(), PatchLifecycleError> {
        let next_selection = self
            .policy_selection
            .checked_add(1)
            .ok_or(PatchLifecycleError::PolicySelectionExhausted)?;
        if let Some(existing) = self.policy_meanings.get(&version) {
            if existing != &meaning {
                return Err(PatchLifecycleError::PolicyMeaningConflict);
            }
        } else {
            self.policy_meanings.insert(version.clone(), meaning);
        }
        self.effective_policy = version;
        self.policy_selection = next_selection;
        Ok(())
    }

    /// Authorize one complete saved keyed grouped-sum Query against a trusted
    /// current snapshot.
    ///
    /// The caller supplies only the saved definition identity. This lifecycle
    /// derives the complete disclosure footprint from the exact definition and
    /// protected document: both candidate memberships, every Orders key and
    /// quantity, the full Products key universe, the matched Products'
    /// category/price, and—only when a required matched operand is
    /// formula-backed—every document formula root. It deliberately performs
    /// authorization before an adapter evaluates or projects a result, so a
    /// partial aggregate or diagnostic cannot become a disclosure fallback.
    ///
    /// # Errors
    ///
    /// Returns an authorization error when the principal lacks complete Query
    /// coverage, or a scope-derivation error when the requested definition or
    /// any of its stable bindings cannot be resolved in this document.
    pub fn authorize_keyed_grouped_sum_query(
        &self,
        document: &Document,
        definition_id: &KeyedGroupedSumDefinitionId,
        subject: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<BTreeSet<GrantId>, PatchLifecycleError> {
        if document.id != self.document {
            return Err(PatchLifecycleError::DocumentScopeMismatch);
        }
        let requirements = self.keyed_grouped_sum_query_disclosures(document, definition_id)?;
        self.authorize_query(subject, &requirements, now)
    }

    /// Issue and authoritatively evaluate one immutable proposal occurrence.
    ///
    /// # Errors
    ///
    /// Returns stale, command, validation, identity, principal, or Propose
    /// authorization failure without semantic publication.
    pub fn propose(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        request: ProposalRequest,
        now: TrustedInstant,
    ) -> Result<SemanticPatch, PatchLifecycleError> {
        self.propose_inner(
            document_scope,
            document,
            current_revision,
            request,
            now,
            None,
        )
    }

    /// Issue one proposal reusing a caller-prepared candidate evaluation.
    ///
    /// The prepared evaluation replaces this proposal's candidate validation,
    /// calculation and base calculation only when it names the same base
    /// revision and evaluates the exact planned candidate; every other case
    /// takes the same path as [`Self::propose`].
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`Self::propose`].
    pub fn propose_prepared(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        request: ProposalRequest,
        now: TrustedInstant,
        prepared: Option<&PreparedEvaluation<'_>>,
    ) -> Result<SemanticPatch, PatchLifecycleError> {
        self.propose_inner(
            document_scope,
            document,
            current_revision,
            request,
            now,
            prepared,
        )
    }

    #[allow(clippy::too_many_lines)] // Proposal checks stay ordered at one admission boundary.
    fn propose_inner(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        request: ProposalRequest,
        now: TrustedInstant,
        prepared: Option<&PreparedEvaluation<'_>>,
    ) -> Result<SemanticPatch, PatchLifecycleError> {
        let prepared = prepared.filter(|prepared| {
            prepared.base_revision() == current_revision && prepared.base_document() == document
        });
        self.require_document(document_scope, document)?;
        if self.proposals.contains_key(&request.id) {
            return Err(PatchLifecycleError::ProposalIdAlreadyExists);
        }
        self.require_active_principal(&request.originator)?;

        let patch = SemanticPatch {
            id: request.id.clone(),
            exact_change: ExactChangeBinding {
                semantic_api_contract: self.semantic_api_contract.clone(),
                base_revision: request.base_revision,
                body: request.body,
                evidence_profile: self.evidence_profile,
            },
        };
        let mut record = ProposalRecord {
            patch: patch.clone(),
            originator: request.originator,
            footprint: None,
            propose_grants: BTreeSet::new(),
            policy_version: self.effective_policy.clone(),
            history: vec![PatchLifecycleState::Draft, PatchLifecycleState::Planned],
            reviewed_by: BTreeSet::new(),
            sequence: self.next_proposal_sequence,
        };
        self.next_proposal_sequence = self.next_proposal_sequence.saturating_add(1);
        let conservative_disclosure = self.body_document_disclosure(patch.exact_change.body());

        if patch.exact_change.base_revision != *current_revision {
            let originator = record.originator.clone();
            return if self
                .authorize_query(&originator, &conservative_disclosure, now)
                .is_ok()
            {
                Err(PatchLifecycleError::Stale)
            } else {
                Err(PatchLifecycleError::AuthorizationDenied)
            };
        }

        if let Some((schema, field)) = duplicate_constraint_write(patch.exact_change.body()) {
            if self
                .authorize_query(&record.originator, &conservative_disclosure, now)
                .is_err()
            {
                return Err(PatchLifecycleError::DisclosureDenied);
            }
            return Err(PatchLifecycleError::DuplicateFieldConstraintWrite { schema, field });
        }

        let (candidate, writes) = match self.plan_commands(document, patch.exact_change.body()) {
            Ok(planned) => planned,
            Err(source) => {
                let originator = record.originator.clone();
                if self
                    .authorize_query(&originator, &conservative_disclosure, now)
                    .is_err()
                {
                    return Err(PatchLifecycleError::DisclosureDenied);
                }
                return Err(PatchLifecycleError::CommandRejected {
                    source: Box::new(source),
                });
            }
        };
        let provisional_footprint = AuthorizationFootprint {
            disclosure_requirements: BTreeSet::new(),
            associated_write_requirements: writes,
        };
        let propose_grants = self.authorize_mutation(
            &record.originator,
            AuthorizationAction::Propose,
            &provisional_footprint.associated_write_requirements,
            now,
        )?;
        record.propose_grants = propose_grants;
        // Proposal identity is now issued. An invalid candidate may not yield a
        // safe semantic diff/impact projection, so retain exact writes with a
        // conservative complete disclosure boundary. Successful evaluation
        // replaces this with the precise derived footprint below.
        record.footprint = Some(AuthorizationFootprint {
            disclosure_requirements: conservative_disclosure.clone(),
            associated_write_requirements: provisional_footprint
                .associated_write_requirements
                .clone(),
        });

        // A failed evaluation must not leave an earlier evaluation reusable.
        self.evaluation_memo = None;
        let evaluated = match self.finalize_evaluation(
            document,
            candidate,
            patch.exact_change.body(),
            provisional_footprint.associated_write_requirements.clone(),
            prepared,
        ) {
            Ok(evaluated) => evaluated,
            Err(error) => {
                let originator = record.originator.clone();
                record.history.push(
                    if matches!(&error, PatchLifecycleError::ValidationFailed { .. }) {
                        PatchLifecycleState::ValidationFailed
                    } else {
                        PatchLifecycleState::Rejected
                    },
                );
                self.proposals.insert(patch.id.clone(), record);
                if self
                    .authorize_query(&originator, &conservative_disclosure, now)
                    .is_err()
                {
                    return Err(PatchLifecycleError::DisclosureDenied);
                }
                return Err(error);
            }
        };
        record.footprint = Some(evaluated.footprint.clone());
        self.proposals.insert(patch.id.clone(), record);
        self.evaluation_memo = Some(EvaluationMemo {
            base: document.clone(),
            patch: patch.clone(),
            evaluated,
        });
        Ok(patch)
    }

    /// Return disclosure-authorized review evidence for a current proposal.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::DisclosureDenied`] unless the viewer is
    /// active and has current Query authority for the retained proposal's
    /// complete disclosure footprint. Missing proposal occurrences are hidden
    /// by the same outcome. Once disclosure is authorized, returns a typed
    /// terminal, stale, or semantic failure.
    pub fn preview(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        proposal_id: &ProposalId,
        viewer: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<PatchPreview, PatchLifecycleError> {
        match self.preview_profile(
            document_scope,
            document,
            current_revision,
            proposal_id,
            viewer,
            now,
            EvidenceProfile::V1,
        )? {
            ProfiledPreview::V1(preview) => Ok(preview),
            ProfiledPreview::V2(_) => Err(PatchLifecycleError::EvidenceProfileMismatch),
        }
    }

    /// Return a disclosure-authorized v2 preview with complete canonical facts.
    ///
    /// # Errors
    ///
    /// Returns a profile mismatch, stale or non-previewable proposal, or a
    /// disclosure/validation failure without publishing semantic state.
    pub fn preview_v2(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        proposal_id: &ProposalId,
        viewer: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<PatchPreviewV2, PatchLifecycleError> {
        match self.preview_profile(
            document_scope,
            document,
            current_revision,
            proposal_id,
            viewer,
            now,
            EvidenceProfile::V2,
        )? {
            ProfiledPreview::V2(preview) => Ok(preview),
            ProfiledPreview::V1(_) => Err(PatchLifecycleError::EvidenceProfileMismatch),
        }
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the public profile-specific preview context.
    fn preview_profile(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        proposal_id: &ProposalId,
        viewer: &PrincipalId,
        now: TrustedInstant,
        expected_profile: EvidenceProfile,
    ) -> Result<ProfiledPreview, PatchLifecycleError> {
        self.require_document(document_scope, document)?;
        let record = self.select_disclosable_proposal(proposal_id, viewer, now)?;
        if record.patch.exact_change.evidence_profile() != expected_profile {
            return Err(PatchLifecycleError::EvidenceProfileMismatch);
        }
        let stored_footprint = record
            .footprint
            .clone()
            .ok_or(PatchLifecycleError::ProposalNotExecutable)?;
        Self::require_nonterminal_proposal(&record, true)?;
        if record.patch.exact_change.base_revision != *current_revision {
            self.append_state(proposal_id, PatchLifecycleState::Stale);
            return Err(PatchLifecycleError::Stale);
        }
        let patch = record.patch;
        self.with_evaluated(document, &patch, |lifecycle, evaluated| {
            if evaluated.footprint != stored_footprint {
                lifecycle.append_state(proposal_id, PatchLifecycleState::Conflict);
                return Err(PatchLifecycleError::ApprovalBindingMismatch);
            }
            let constraint_reviews = if matches!(&evaluated.evidence, PatchEvidence::V2(_)) {
                constraint_review_evidence(
                    document,
                    &evaluated.document,
                    patch.exact_change.body(),
                )?
            } else {
                Vec::new()
            };
            let risk = PatchRisk {
                mutation_classes: evaluated.footprint.mutation_classes(),
            };
            let proposal_record = lifecycle
                .proposals
                .get_mut(proposal_id)
                .ok_or(PatchLifecycleError::ProposalNotFound)?;
            proposal_record.reviewed_by.insert(viewer.clone());
            for state in [
                PatchLifecycleState::Previewed,
                PatchLifecycleState::Validated,
                PatchLifecycleState::AwaitingApproval,
            ] {
                push_once(&mut proposal_record.history, state);
            }
            match &evaluated.evidence {
                PatchEvidence::V1(semantic_changes) => Ok(ProfiledPreview::V1(PatchPreview {
                    proposal: patch.clone(),
                    semantic_changes: semantic_changes.clone(),
                    formula_impacts: evaluated.formula_impacts.clone(),
                    validation_report: evaluated.validation_report.clone(),
                    authorization_footprint: evaluated.footprint.clone(),
                    risk,
                })),
                PatchEvidence::V2(delta) => Ok(ProfiledPreview::V2(PatchPreviewV2 {
                    proposal: patch.clone(),
                    delta: delta.clone(),
                    constraint_reviews,
                    formula_impacts: evaluated.formula_impacts.clone(),
                    validation_report: evaluated.validation_report.clone(),
                    authorization_footprint: evaluated.footprint.clone(),
                    risk,
                })),
            }
        })
    }

    /// Issue one exact finite Human Approval after authorized review.
    ///
    /// # Errors
    ///
    /// Returns identity, review, stale, semantic, policy, expiry, unnecessary-
    /// Approval, or Approve authorization failure without semantic publication.
    pub fn approve(
        &mut self,
        document_scope: &DocumentScopeId,
        document: &Document,
        current_revision: &SemanticRevision,
        request: ApprovalRequest,
        now: TrustedInstant,
    ) -> Result<Approval, PatchLifecycleError> {
        self.require_document(document_scope, document)?;
        if self.approvals.contains_key(&request.id) {
            return Err(PatchLifecycleError::ApprovalIdAlreadyExists);
        }
        if request.expires_at <= now {
            return Err(PatchLifecycleError::InvalidApprovalExpiry);
        }
        if self.require_active_principal(&request.approver)? != PrincipalKind::Human {
            return Err(PatchLifecycleError::ApproverMustBeHuman);
        }
        self.require_active_principal(&request.executor)?;
        let proposal =
            self.select_disclosable_proposal(&request.proposal_id, &request.approver, now)?;
        let footprint = proposal
            .footprint
            .clone()
            .ok_or(PatchLifecycleError::ProposalNotExecutable)?;
        Self::require_nonterminal_proposal(&proposal, true)?;
        if !self.execution_requires_approval(&proposal, &request.executor)? {
            return Err(PatchLifecycleError::ApprovalNotRequired);
        }
        if !proposal.reviewed_by.contains(&request.approver) {
            return Err(PatchLifecycleError::ReviewRequired);
        }
        let approve_grants = self.authorize_mutation(
            &request.approver,
            AuthorizationAction::Approve,
            &footprint.associated_write_requirements,
            now,
        )?;
        if proposal.patch.exact_change.base_revision != *current_revision {
            self.append_state(&request.proposal_id, PatchLifecycleState::Stale);
            return Err(PatchLifecycleError::Stale);
        }
        let associated_write_requirements =
            self.with_evaluated(document, &proposal.patch, |_, evaluated| {
                if evaluated.footprint != footprint {
                    return Err(PatchLifecycleError::ApprovalBindingMismatch);
                }
                Ok(evaluated.footprint.associated_write_requirements.clone())
            })?;
        let approval = Approval {
            id: request.id.clone(),
            binding: ApprovalBinding {
                authorization_domain: self.authorization_domain.clone(),
                proposal_id: request.proposal_id.clone(),
                exact_change: proposal.patch.exact_change.clone(),
                originator: proposal.originator,
                executor: request.executor,
                associated_write_requirements,
                policy_version: self.effective_policy.clone(),
            },
            approver: request.approver,
            issued_at: now,
            expires_at: request.expires_at,
            approve_grants,
        };
        self.approvals.insert(
            request.id,
            StoredApproval {
                approval: approval.clone(),
                status: ApprovalStatus::Active,
                policy_selection: self.policy_selection,
            },
        );
        self.append_state(&request.proposal_id, PatchLifecycleState::Approved);
        Ok(approval)
    }

    /// Terminally revoke one Active Approval occurrence.
    ///
    /// # Errors
    ///
    /// Returns a typed absent or terminal-state outcome.
    pub fn revoke_approval(&mut self, id: &ApprovalId) -> Result<(), PatchLifecycleError> {
        let stored = self
            .approvals
            .get_mut(id)
            .ok_or(PatchLifecycleError::ApprovalNotFound)?;
        match stored.status {
            ApprovalStatus::Active => {
                stored.status = ApprovalStatus::Revoked;
                Ok(())
            }
            ApprovalStatus::Consumed => Err(PatchLifecycleError::ApprovalConsumed),
            ApprovalStatus::Revoked => Err(PatchLifecycleError::ApprovalRevoked),
            ApprovalStatus::Expired => Err(PatchLifecycleError::ApprovalExpired),
        }
    }

    /// Execute one current exact proposal through the host publication seam.
    ///
    /// # Errors
    ///
    /// Returns typed authorization, Approval, stale, semantic, publication, or
    /// verification failure. A proved pre-publication failure consumes no
    /// Approval and publishes no candidate.
    pub fn execute(
        &mut self,
        proposal_id: &ProposalId,
        approval_id: Option<&ApprovalId>,
        executor: &PrincipalId,
        publication: &mut impl SemanticPublicationAuthority,
        now: TrustedInstant,
    ) -> Result<ExecutionReceipt, PatchLifecycleError> {
        match self.execute_profile(
            proposal_id,
            approval_id,
            executor,
            publication,
            now,
            EvidenceProfile::V1,
        )? {
            ProfiledReceipt::V1(receipt) => Ok(receipt),
            ProfiledReceipt::V2(_) => Err(PatchLifecycleError::EvidenceProfileMismatch),
        }
    }

    /// Execute a current v2 proposal through the same guarded publication path.
    ///
    /// # Errors
    ///
    /// Returns authorization, stale, approval, validation, publication, or
    /// profile errors without accepting an unverified publication.
    pub fn execute_v2(
        &mut self,
        proposal_id: &ProposalId,
        approval_id: Option<&ApprovalId>,
        executor: &PrincipalId,
        publication: &mut impl SemanticPublicationAuthority,
        now: TrustedInstant,
    ) -> Result<ExecutionReceiptV2, PatchLifecycleError> {
        match self.execute_profile(
            proposal_id,
            approval_id,
            executor,
            publication,
            now,
            EvidenceProfile::V2,
        )? {
            ProfiledReceipt::V2(receipt) => Ok(receipt),
            ProfiledReceipt::V1(_) => Err(PatchLifecycleError::EvidenceProfileMismatch),
        }
    }

    #[allow(clippy::too_many_lines)] // Preserve the common checked publication boundary.
    fn execute_profile(
        &mut self,
        proposal_id: &ProposalId,
        approval_id: Option<&ApprovalId>,
        executor: &PrincipalId,
        publication: &mut impl SemanticPublicationAuthority,
        now: TrustedInstant,
        expected_profile: EvidenceProfile,
    ) -> Result<ProfiledReceipt, PatchLifecycleError> {
        // Taking the slot up front makes the memo unusable after this
        // execution whatever its outcome, so no base document outlives it and
        // a retried execute evaluates again.
        let retained = self.evaluation_memo.take();
        let (current_document_scope, document, current_revision) = publication.current_snapshot();
        if self
            .require_document(&current_document_scope, document)
            .is_err()
        {
            return Err(PatchLifecycleError::AuthorizationDenied);
        }
        let (proposal, footprint, approval) =
            self.select_execution_context(proposal_id, approval_id, executor)?;
        let can_disclose = self
            .authorize_query(executor, &footprint.disclosure_requirements, now)
            .is_ok();
        if proposal.patch.exact_change.evidence_profile() != expected_profile {
            return Err(if can_disclose {
                PatchLifecycleError::EvidenceProfileMismatch
            } else {
                PatchLifecycleError::AuthorizationDenied
            });
        }
        Self::require_nonterminal_proposal(&proposal, can_disclose)?;

        if proposal.patch.exact_change.base_revision != current_revision {
            self.append_state(proposal_id, PatchLifecycleState::Stale);
            return Err(if can_disclose {
                PatchLifecycleError::Stale
            } else {
                PatchLifecycleError::AuthorizationDenied
            });
        }
        self.require_execution_approval_active(
            proposal_id,
            &proposal,
            approval.as_ref(),
            executor,
            &footprint,
            now,
        )?;

        let evaluated = match retained {
            Some(memo) if memo.base == *document && memo.patch == proposal.patch => memo.evaluated,
            _ => self.evaluate_patch(document, &proposal.patch)?,
        };
        if evaluated.footprint != footprint {
            return Err(PatchLifecycleError::ApprovalBindingMismatch);
        }
        self.authorize_publication_boundary(
            &proposal,
            approval.as_ref(),
            executor,
            &footprint,
            now,
        )?;
        let evaluated_report = evaluated.validation_report.clone();
        let published = self.publish_candidate(
            publication,
            PublicationAttempt {
                proposal_id,
                proposal: &proposal,
                approval,
                executor,
                footprint: &footprint,
                current_document_scope: &current_document_scope,
                current_revision: &current_revision,
                candidate: &evaluated.document,
            },
        )?;
        let can_disclose = published.authorization.can_disclose;
        let mut receipt = match evaluated.evidence {
            PatchEvidence::V1(semantic_changes) => {
                let receipt = ExecutionReceipt {
                    proposal_id: proposal_id.clone(),
                    originator: proposal.originator.clone(),
                    executor: executor.clone(),
                    approval: published.approval,
                    propose_grants: proposal.propose_grants.clone(),
                    approve_grants: published.authorization.approve_grants,
                    execute_grants: published.authorization.execute_grants,
                    authorization_footprint: Some(footprint),
                    policy_version: self.effective_policy.clone(),
                    base_revision: current_revision.clone(),
                    resulting_revision: published.revision.clone(),
                    verified: false,
                    semantic_changes,
                    formula_impacts: evaluated.formula_impacts,
                    validation_report: Some(evaluated.validation_report),
                };
                self.execution_receipts.push(receipt.clone());
                ProfiledReceipt::V1(receipt)
            }
            PatchEvidence::V2(delta) => {
                let receipt = ExecutionReceiptV2 {
                    proposal_id: proposal_id.clone(),
                    originator: proposal.originator.clone(),
                    executor: executor.clone(),
                    approval: published.approval,
                    propose_grants: proposal.propose_grants.clone(),
                    approve_grants: published.authorization.approve_grants,
                    execute_grants: published.authorization.execute_grants,
                    authorization_footprint: Some(footprint),
                    policy_version: self.effective_policy.clone(),
                    base_revision: current_revision.clone(),
                    resulting_revision: published.revision.clone(),
                    verified: false,
                    evidence: V2ReceiptEvidence::Disclosed(delta),
                    formula_impacts: evaluated.formula_impacts,
                    validation_report: Some(evaluated.validation_report),
                };
                self.execution_receipts_v2.push(receipt.clone());
                ProfiledReceipt::V2(receipt)
            }
        };
        let verification_report = match self.verify_publication(
            proposal_id,
            &current_revision,
            &published.revision,
            (&evaluated.document, &evaluated_report),
            &published.document_scope,
            published.document,
        ) {
            Ok(report) => report,
            Err(error) if can_disclose => return Err(error),
            Err(_) => return Err(PatchLifecycleError::AuthorizationDenied),
        };
        self.append_state(proposal_id, PatchLifecycleState::Verified);
        match &mut receipt {
            ProfiledReceipt::V1(receipt) => {
                receipt.validation_report = Some(verification_report);
                receipt.verified = true;
                if let Some(stored) = self.execution_receipts.last_mut() {
                    *stored = receipt.clone();
                }
                if !can_disclose {
                    receipt.authorization_footprint = None;
                    receipt.semantic_changes.clear();
                    receipt.formula_impacts.clear();
                    receipt.validation_report = None;
                }
            }
            ProfiledReceipt::V2(receipt) => {
                receipt.validation_report = Some(verification_report);
                receipt.verified = true;
                if let Some(stored) = self.execution_receipts_v2.last_mut() {
                    *stored = receipt.clone();
                }
                if !can_disclose {
                    receipt.authorization_footprint = None;
                    receipt.formula_impacts.clear();
                    receipt.validation_report = None;
                    receipt.evidence = V2ReceiptEvidence::Redacted;
                }
            }
        }
        Ok(receipt)
    }

    fn select_execution_context(
        &self,
        proposal_id: &ProposalId,
        approval_id: Option<&ApprovalId>,
        executor: &PrincipalId,
    ) -> Result<
        (
            ProposalRecord,
            AuthorizationFootprint,
            Option<StoredApproval>,
        ),
        PatchLifecycleError,
    > {
        if self.require_active_principal(executor).is_err() {
            return Err(PatchLifecycleError::AuthorizationDenied);
        }
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or(PatchLifecycleError::AuthorizationDenied)?
            .clone();
        let footprint = proposal
            .footprint
            .clone()
            .ok_or(PatchLifecycleError::AuthorizationDenied)?;
        let approval =
            match self.select_execution_approval(approval_id, executor, &proposal, &footprint) {
                Ok(approval) => approval,
                Err(
                    PatchLifecycleError::UnknownPrincipal
                    | PatchLifecycleError::PrincipalDisabled
                    | PatchLifecycleError::ApprovalRequired
                    | PatchLifecycleError::ApprovalBindingMismatch
                    | PatchLifecycleError::AuthorizationDenied,
                ) => return Err(PatchLifecycleError::AuthorizationDenied),
                Err(error) => return Err(error),
            };
        Ok((proposal, footprint, approval))
    }

    fn select_disclosable_proposal(
        &self,
        proposal_id: &ProposalId,
        viewer: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<ProposalRecord, PatchLifecycleError> {
        if self.require_active_principal(viewer).is_err() {
            return Err(PatchLifecycleError::DisclosureDenied);
        }
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or(PatchLifecycleError::DisclosureDenied)?
            .clone();
        let disclosure_requirements = proposal.footprint.as_ref().map_or_else(
            || self.document_disclosure(),
            |footprint| footprint.disclosure_requirements.clone(),
        );
        self.authorize_query(viewer, &disclosure_requirements, now)
            .map_err(|_| PatchLifecycleError::DisclosureDenied)?;
        Ok(proposal)
    }

    fn require_nonterminal_proposal(
        proposal: &ProposalRecord,
        can_disclose: bool,
    ) -> Result<(), PatchLifecycleError> {
        if proposal_is_terminal(proposal) {
            Err(if can_disclose {
                PatchLifecycleError::ProposalNotExecutable
            } else {
                PatchLifecycleError::AuthorizationDenied
            })
        } else {
            Ok(())
        }
    }

    fn select_execution_approval(
        &self,
        approval_id: Option<&ApprovalId>,
        executor: &PrincipalId,
        proposal: &ProposalRecord,
        footprint: &AuthorizationFootprint,
    ) -> Result<Option<StoredApproval>, PatchLifecycleError> {
        let approval_id = Self::validate_execution_approval_presence(
            self.execution_requires_approval(proposal, executor)?,
            approval_id,
        )?;
        let approval = approval_id
            .map(|id| {
                self.approvals
                    .get(id)
                    .ok_or(PatchLifecycleError::AuthorizationDenied)
                    .cloned()
            })
            .transpose()?;
        self.validate_execution_approval_selection(
            proposal,
            approval.as_ref(),
            executor,
            footprint,
        )?;
        Ok(approval)
    }

    fn validate_execution_approval_selection(
        &self,
        proposal: &ProposalRecord,
        approval: Option<&StoredApproval>,
        executor: &PrincipalId,
        footprint: &AuthorizationFootprint,
    ) -> Result<(), PatchLifecycleError> {
        let Some(stored) = approval else {
            return Ok(());
        };
        if stored.approval.binding.executor != *executor {
            return Err(PatchLifecycleError::AuthorizationDenied);
        }
        let binding = &stored.approval.binding;
        if binding.authorization_domain != self.authorization_domain
            || binding.proposal_id != proposal.patch.id
            || binding.exact_change != proposal.patch.exact_change
            || binding.originator != proposal.originator
            || binding.associated_write_requirements != footprint.associated_write_requirements
        {
            return Err(PatchLifecycleError::ApprovalBindingMismatch);
        }
        if stored.status == ApprovalStatus::Consumed {
            return Err(PatchLifecycleError::ApprovalConsumed);
        }
        if binding.policy_version != self.effective_policy
            || stored.policy_selection != self.policy_selection
        {
            return Err(PatchLifecycleError::AuthorizationPolicyChanged);
        }
        Ok(())
    }

    fn validate_execution_approval_presence<T>(
        approval_required: bool,
        approval: Option<T>,
    ) -> Result<Option<T>, PatchLifecycleError> {
        match (approval_required, approval) {
            (true, Some(approval)) => Ok(Some(approval)),
            (true, None) => Err(PatchLifecycleError::ApprovalRequired),
            (false, Some(_)) => Err(PatchLifecycleError::ApprovalBindingMismatch),
            (false, None) => Ok(None),
        }
    }

    fn execution_requires_approval(
        &self,
        proposal: &ProposalRecord,
        executor: &PrincipalId,
    ) -> Result<bool, PatchLifecycleError> {
        let executor_kind = self.require_active_principal(executor)?;
        let originator = self
            .principals
            .get(&proposal.originator)
            .ok_or(PatchLifecycleError::UnknownPrincipal)?;
        let approval_required = executor_kind == PrincipalKind::Delegated
            || originator.kind == PrincipalKind::Delegated;
        if approval_required && !originator.active {
            return Err(PatchLifecycleError::PrincipalDisabled);
        }
        Ok(approval_required)
    }

    fn validate_execution_approval(
        &self,
        proposal: &ProposalRecord,
        approval: Option<&StoredApproval>,
        executor: &PrincipalId,
        footprint: &AuthorizationFootprint,
        now: TrustedInstant,
    ) -> Result<(), PatchLifecycleError> {
        let approval = Self::validate_execution_approval_presence(
            self.execution_requires_approval(proposal, executor)?,
            approval,
        )?;
        self.validate_execution_approval_selection(proposal, approval, executor, footprint)?;
        let Some(stored) = approval else {
            return Ok(());
        };
        match stored.status {
            ApprovalStatus::Active => {}
            ApprovalStatus::Consumed => return Err(PatchLifecycleError::ApprovalConsumed),
            ApprovalStatus::Revoked => return Err(PatchLifecycleError::ApprovalRevoked),
            ApprovalStatus::Expired => return Err(PatchLifecycleError::ApprovalExpired),
        }
        if now >= stored.approval.expires_at {
            return Err(PatchLifecycleError::ApprovalExpired);
        }
        if self.require_active_principal(&stored.approval.approver)? != PrincipalKind::Human {
            return Err(PatchLifecycleError::AuthorizationDenied);
        }
        Ok(())
    }

    fn require_execution_approval_active(
        &mut self,
        proposal_id: &ProposalId,
        proposal: &ProposalRecord,
        approval: Option<&StoredApproval>,
        executor: &PrincipalId,
        footprint: &AuthorizationFootprint,
        now: TrustedInstant,
    ) -> Result<(), PatchLifecycleError> {
        let result = self.validate_execution_approval(proposal, approval, executor, footprint, now);
        if matches!(result, Err(PatchLifecycleError::ApprovalExpired))
            && approval.is_some_and(|stored| {
                stored.status == ApprovalStatus::Active && now >= stored.approval.expires_at
            })
        {
            let stored = approval.ok_or(PatchLifecycleError::ApprovalNotFound)?;
            let approval = self
                .approvals
                .get_mut(&stored.approval.id)
                .ok_or(PatchLifecycleError::ApprovalNotFound)?;
            approval.status = ApprovalStatus::Expired;
            self.append_state(proposal_id, PatchLifecycleState::Expired);
        }
        result
    }

    fn authorize_execution(
        &self,
        approval: Option<&StoredApproval>,
        executor: &PrincipalId,
        footprint: &AuthorizationFootprint,
        now: TrustedInstant,
    ) -> Result<(BTreeSet<GrantId>, BTreeSet<GrantId>), PatchLifecycleError> {
        let approve_grants = if let Some(stored) = approval {
            self.require_recorded_approve_grants(stored, now)?;
            self.authorize_mutation(
                &stored.approval.approver,
                AuthorizationAction::Approve,
                &footprint.associated_write_requirements,
                now,
            )?
        } else {
            BTreeSet::new()
        };
        let execute_grants = self.authorize_mutation(
            executor,
            AuthorizationAction::Execute,
            &footprint.associated_write_requirements,
            now,
        )?;
        Ok((approve_grants, execute_grants))
    }

    fn authorize_publication_boundary(
        &self,
        proposal: &ProposalRecord,
        approval: Option<&StoredApproval>,
        executor: &PrincipalId,
        footprint: &AuthorizationFootprint,
        now: TrustedInstant,
    ) -> Result<(BTreeSet<GrantId>, BTreeSet<GrantId>), PatchLifecycleError> {
        self.validate_execution_approval(proposal, approval, executor, footprint, now)?;
        self.authorize_execution(approval, executor, footprint, now)
    }

    fn publish_candidate<'authority>(
        &mut self,
        publication: &'authority mut impl SemanticPublicationAuthority,
        attempt: PublicationAttempt<'_>,
    ) -> Result<PublishedCandidate<'authority>, PatchLifecycleError> {
        let PublicationAttempt {
            proposal_id,
            proposal,
            approval,
            executor,
            footprint,
            current_document_scope,
            current_revision,
            candidate,
        } = attempt;
        let mut reserved = if let Some(stored) = approval {
            let approval = self
                .approvals
                .remove(&stored.approval.id)
                .ok_or(PatchLifecycleError::ApprovalNotFound)?;
            Some(approval)
        } else {
            None
        };

        let mut boundary_error = None;
        let mut boundary_disclosure = None;
        let publication_result = publication.publish_if_current(
            current_document_scope,
            current_revision,
            candidate,
            |boundary_now| match self.authorize_publication_boundary(
                proposal,
                reserved.as_ref(),
                executor,
                footprint,
                boundary_now,
            ) {
                Ok((approve_grants, execute_grants)) => {
                    let can_disclose = self
                        .authorize_query(executor, &footprint.disclosure_requirements, boundary_now)
                        .is_ok();
                    boundary_disclosure = Some(can_disclose);
                    Some(PublicationAuthorization {
                        approve_grants,
                        execute_grants,
                        can_disclose,
                    })
                }
                Err(error) => {
                    boundary_error = Some(error);
                    None
                }
            },
        );
        let (
            installed_document_scope,
            installed_document,
            resulting_revision,
            publication_authorization,
        ) = match publication_result {
            Ok(success) => success,
            Err(error) => {
                return Err(self.restore_after_publication_failure(
                    proposal_id,
                    reserved,
                    boundary_error,
                    error,
                    boundary_disclosure,
                ));
            }
        };

        let approval_evidence = reserved.as_mut().map(|stored| {
            stored.status = ApprovalStatus::Consumed;
            ApprovalExecutionEvidence {
                approval_id: stored.approval.id.clone(),
                approver: stored.approval.approver.clone(),
                status: ApprovalStatus::Consumed,
            }
        });
        if let Some(approval) = reserved {
            self.approvals
                .insert(approval.approval.id.clone(), approval);
        }
        self.append_state(proposal_id, PatchLifecycleState::Applied);
        Ok(PublishedCandidate {
            document_scope: installed_document_scope,
            document: installed_document,
            revision: resulting_revision,
            approval: approval_evidence,
            authorization: publication_authorization,
        })
    }

    fn restore_after_publication_failure(
        &mut self,
        proposal_id: &ProposalId,
        mut reserved: Option<StoredApproval>,
        boundary_error: Option<PatchLifecycleError>,
        publication_error: SemanticPublicationError,
        boundary_disclosure: Option<bool>,
    ) -> PatchLifecycleError {
        let approval_expired = matches!(
            boundary_error.as_ref(),
            Some(PatchLifecycleError::ApprovalExpired)
        );
        if approval_expired {
            if let Some(stored) = reserved.as_mut() {
                stored.status = ApprovalStatus::Expired;
            }
        }
        if let Some(approval) = reserved {
            self.approvals
                .insert(approval.approval.id.clone(), approval);
        }
        if approval_expired {
            self.append_state(proposal_id, PatchLifecycleState::Expired);
        }
        let (state, disclosed_error) = match publication_error {
            SemanticPublicationError::NoChange => {
                // The guard proved current base and live authority. Approval
                // was restored above; no Applied/Verified state or publication
                // receipt exists. Equality itself requires Query disclosure.
                return if boundary_disclosure == Some(true) {
                    PatchLifecycleError::NoChange
                } else {
                    PatchLifecycleError::AuthorizationDenied
                };
            }
            SemanticPublicationError::DocumentScopeMismatch => {
                return PatchLifecycleError::AuthorizationDenied;
            }
            SemanticPublicationError::Stale => {
                (PatchLifecycleState::Stale, PatchLifecycleError::Stale)
            }
            SemanticPublicationError::Conflict => (
                PatchLifecycleState::RetryableConflict,
                PatchLifecycleError::Conflict,
            ),
            SemanticPublicationError::AuthorizationDenied => {
                return boundary_error.unwrap_or(PatchLifecycleError::AuthorizationDenied);
            }
        };
        self.append_state(proposal_id, state);
        if boundary_disclosure == Some(true) {
            disclosed_error
        } else {
            PatchLifecycleError::AuthorizationDenied
        }
    }

    fn verify_publication(
        &mut self,
        proposal_id: &ProposalId,
        base_revision: &SemanticRevision,
        resulting_revision: &SemanticRevision,
        evaluated: (&Document, &ValidationReport),
        installed_document_scope: &DocumentScopeId,
        installed_document: &Document,
    ) -> Result<ValidationReport, PatchLifecycleError> {
        let (candidate, evaluated_report) = evaluated;
        if installed_document_scope != &self.document_scope {
            self.append_state(proposal_id, PatchLifecycleState::Conflict);
            return Err(PatchLifecycleError::VerificationFailed);
        }
        // The installed document must be the evaluated candidate itself, so the
        // evaluated report is the installed document's report by construction:
        // validation is a pure function of the document.
        if resulting_revision == base_revision
            || installed_document != candidate
            || !evaluated_report.is_valid()
        {
            self.append_state(proposal_id, PatchLifecycleState::Conflict);
            return Err(PatchLifecycleError::VerificationFailed);
        }
        Ok(evaluated_report.clone())
    }

    /// Read the unredacted trusted in-process receipt store.
    ///
    /// This is host-side provenance, not an executor/client projection. The
    /// value returned directly from [`Self::execute`] is independently reduced
    /// when the executor lacks Query authority.
    #[must_use]
    pub fn execution_receipts(&self) -> &[ExecutionReceipt] {
        &self.execution_receipts
    }

    /// Read the complete trusted v2 receipt registry, including direct deltas.
    #[must_use]
    pub fn execution_receipts_v2(&self) -> &[ExecutionReceiptV2] {
        &self.execution_receipts_v2
    }

    /// Drop settled records except the latest `keep` of each kind. A proposal
    /// that hasn't been executed or refused yet always stays.
    ///
    /// Receipts stay in publication order, so the last `keep` of each receipt
    /// vector survive. Proposals keep the `keep` most recently issued settled
    /// records; pending, retryable and expired proposals are never dropped.
    /// Nothing reads the stored records during a request: callers use the
    /// receipt returned by execution.
    pub fn discard_settled_records(&mut self, keep: usize) {
        if self.execution_receipts.len() > keep {
            let excess = self.execution_receipts.len() - keep;
            self.execution_receipts.drain(..excess);
        }
        if self.execution_receipts_v2.len() > keep {
            let excess = self.execution_receipts_v2.len() - keep;
            self.execution_receipts_v2.drain(..excess);
        }
        let mut settled: Vec<(u64, ProposalId)> = self
            .proposals
            .iter()
            .filter(|(_, record)| proposal_is_terminal(record))
            .map(|(id, record)| (record.sequence, id.clone()))
            .collect();
        settled.sort_by_key(|(sequence, _)| *sequence);
        let excess = settled.len().saturating_sub(keep);
        for (_, id) in settled.into_iter().take(excess) {
            self.proposals.remove(&id);
        }
    }

    /// Read the current trusted Approval registry state.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::ApprovalNotFound`] when absent.
    pub fn approval_status(&self, id: &ApprovalId) -> Result<ApprovalStatus, PatchLifecycleError> {
        self.approvals
            .get(id)
            .map(|approval| approval.status)
            .ok_or(PatchLifecycleError::ApprovalNotFound)
    }

    /// Read external lifecycle evidence for one proposal occurrence.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::ProposalNotFound`] when absent.
    pub fn proposal_history(
        &self,
        id: &ProposalId,
    ) -> Result<&[PatchLifecycleState], PatchLifecycleError> {
        self.proposals
            .get(id)
            .map(|proposal| proposal.history.as_slice())
            .ok_or(PatchLifecycleError::ProposalNotFound)
    }

    /// Read trusted proposal issuance provenance.
    ///
    /// # Errors
    ///
    /// Returns [`PatchLifecycleError::DisclosureDenied`] unless `viewer` is
    /// active and has current Query authority over the complete retained
    /// proposal footprint; missing proposal occurrences are hidden by the same
    /// outcome. A disclosure-authorized retained failure returns
    /// [`PatchLifecycleError::ProposalNotExecutable`].
    pub fn proposal_provenance(
        &self,
        id: &ProposalId,
        viewer: &PrincipalId,
        now: TrustedInstant,
    ) -> Result<ProposalProvenance, PatchLifecycleError> {
        let proposal = self.select_disclosable_proposal(id, viewer, now)?;
        let footprint = proposal
            .footprint
            .clone()
            .ok_or(PatchLifecycleError::ProposalNotExecutable)?;
        Ok(ProposalProvenance {
            authorization_domain: self.authorization_domain.clone(),
            proposal_id: id.clone(),
            exact_change: proposal.patch.exact_change,
            originator: proposal.originator,
            propose_grants: proposal.propose_grants,
            authorization_footprint: footprint,
            policy_version: proposal.policy_version,
        })
    }

    pub(crate) fn require_document(
        &self,
        document_scope: &DocumentScopeId,
        document: &Document,
    ) -> Result<(), PatchLifecycleError> {
        if document_scope == &self.document_scope && document.id == self.document {
            Ok(())
        } else {
            Err(PatchLifecycleError::DocumentScopeMismatch)
        }
    }

    fn document_disclosure(&self) -> BTreeSet<DisclosureRequirement> {
        BTreeSet::from([DisclosureRequirement {
            family: OperationFamily::SetFieldValue,
            scope: ScopedSemanticSubject::new(
                self.document_scope.clone(),
                self.document.clone(),
                SemanticScope::Document,
            ),
        }])
    }

    fn body_document_disclosure(
        &self,
        body: &SemanticPatchBody,
    ) -> BTreeSet<DisclosureRequirement> {
        body.commands()
            .iter()
            .map(|command| DisclosureRequirement {
                family: match command {
                    SemanticCommand::AppendCollection { .. }
                    | SemanticCommand::AppendEntity { .. } => OperationFamily::AppendEntity,
                    SemanticCommand::RemoveCollection { .. }
                    | SemanticCommand::RemoveEntity { .. } => OperationFamily::RemoveEntity,
                    SemanticCommand::AppendSchemaField { .. }
                    | SemanticCommand::RemoveSchemaField { .. }
                    | SemanticCommand::RenameSchemaField { .. }
                    | SemanticCommand::SetSchemaFieldDisplayName { .. }
                    | SemanticCommand::SetSchemaFieldRequiredness { .. }
                    | SemanticCommand::SetFieldNumberFormat { .. }
                    | SemanticCommand::SetSchemaFieldOrder { .. }
                    | SemanticCommand::SetFieldConstraint { .. } => {
                        OperationFamily::SchemaFieldMutation
                    }
                    SemanticCommand::SetFieldValue { .. } | SemanticCommand::UnsetField { .. } => {
                        OperationFamily::SetFieldValue
                    }
                    SemanticCommand::FormulaUpdate(_) | SemanticCommand::FormulaRestore(_) => {
                        OperationFamily::FormulaUpdate
                    }
                    SemanticCommand::FormulaInverseRestore(_) => {
                        OperationFamily::FormulaInverseRestore
                    }
                    SemanticCommand::UpsertKeyedGroupedSumDefinition { .. }
                    | SemanticCommand::RemoveKeyedGroupedSumDefinition { .. } => {
                        OperationFamily::KeyedGroupedSumDefinition
                    }
                },
                scope: ScopedSemanticSubject::new(
                    self.document_scope.clone(),
                    self.document.clone(),
                    SemanticScope::Document,
                ),
            })
            .collect()
    }

    pub(crate) fn require_active_principal(
        &self,
        id: &PrincipalId,
    ) -> Result<PrincipalKind, PatchLifecycleError> {
        let principal = self
            .principals
            .get(id)
            .ok_or(PatchLifecycleError::UnknownPrincipal)?;
        if !principal.active {
            return Err(PatchLifecycleError::PrincipalDisabled);
        }
        Ok(principal.kind)
    }

    fn plan_append_entity(
        &self,
        base: &Document,
        candidate: &mut Document,
        entity: &Entity,
        planning: &mut EntityAppendPlanning,
        writes: &mut BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<(), WorkspaceError> {
        if let Some(keys) = &planning.keys {
            super::validate_appended_entity_key(keys, &entity.key)?;
        } else {
            #[cfg(test)]
            super::planning_counters::record_address_index_build();
            super::validate_new_entity_key(candidate, &entity.key)?;
        }
        if entity.id.as_str().is_empty() {
            return Err(WorkspaceError::EmptyGeneratedId {
                kind: super::SemanticIdKind::Entity,
            });
        }
        // Freshness is relative to both the exact base and the working
        // candidate. Removing a base identity earlier in this batch must not
        // turn append authority into value/schema replacement authority.
        if base.entities.contains_key(&entity.id) || candidate.entities.contains_key(&entity.id) {
            return Err(WorkspaceError::GeneratedIdCollision {
                kind: super::SemanticIdKind::Entity,
                id: entity.id.to_string(),
            });
        }
        if !candidate.schemas.contains_key(&entity.schema) {
            return Err(WorkspaceError::MissingSchema {
                schema: entity.schema.clone(),
            });
        }
        // Bound formula structure is an admission invariant even when a
        // later batch step removes the newly appended entity. The run's first
        // append checks the whole document; every append then checks only the
        // entity it adds.
        if planning.keys.is_none() {
            super::preflight_formula_structures(candidate)?;
        }
        super::preflight_entity_formulas(&entity.id, entity)?;
        let scope = ScopedSemanticSubject::new(
            self.document_scope.clone(),
            self.document.clone(),
            SemanticScope::Schema(entity.schema.clone()),
        );
        writes.insert(AssociatedWriteRequirement {
            family: OperationFamily::AppendEntity,
            mutation_class: MutationClass::Structure,
            scope: scope.clone(),
        });
        if entity
            .fields
            .values()
            .any(|value| matches!(value, Value::Formula(_)))
        {
            writes.insert(AssociatedWriteRequirement {
                family: OperationFamily::AppendEntity,
                mutation_class: MutationClass::Formula,
                scope,
            });
        }
        candidate.entities.insert(entity.id.clone(), entity.clone());
        match &mut planning.keys {
            Some(keys) => {
                keys.insert(entity.key.clone());
            }
            None => {
                planning.keys = Some(
                    candidate
                        .entities
                        .values()
                        .map(|entity| entity.key.clone())
                        .collect(),
                );
            }
        }
        Ok(())
    }

    fn plan_append_collection(
        &self,
        base: &Document,
        candidate: &mut Document,
        schema: &super::Schema,
        entities: &[Entity],
        writes: &mut BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<(), WorkspaceError> {
        if base.schemas.contains_key(&schema.id) || candidate.schemas.contains_key(&schema.id) {
            return Err(WorkspaceError::GeneratedIdCollision {
                kind: super::SemanticIdKind::Schema,
                id: schema.id.to_string(),
            });
        }
        if !super::is_valid_identifier(schema.key.as_str()) {
            return Err(WorkspaceError::InvalidSchemaKey {
                schema: schema.key.clone(),
            });
        }
        #[cfg(test)]
        super::planning_counters::record_address_index_build();
        let schema_key_exists = super::AddressIndex::build(candidate)
            .is_ok_and(|index| index.schema_id(&schema.key).is_ok());
        if schema_key_exists {
            return Err(WorkspaceError::SchemaKeyAlreadyExists {
                schema: schema.key.clone(),
            });
        }
        #[cfg(test)]
        super::planning_counters::record_document_clone();
        let mut appended = candidate.clone();
        appended.schemas.insert(schema.id.clone(), schema.clone());
        let mut seen_entities = BTreeSet::new();
        for entity in entities {
            if entity.schema != schema.id {
                return Err(WorkspaceError::MissingSchema {
                    schema: entity.schema.clone(),
                });
            }
            if !seen_entities.insert(entity.id.clone())
                || base.entities.contains_key(&entity.id)
                || appended.entities.contains_key(&entity.id)
            {
                return Err(WorkspaceError::GeneratedIdCollision {
                    kind: super::SemanticIdKind::Entity,
                    id: entity.id.to_string(),
                });
            }
            appended.entities.insert(entity.id.clone(), entity.clone());
        }
        super::preflight_formula_structures(&appended)?;
        writes.insert(AssociatedWriteRequirement {
            family: OperationFamily::AppendEntity,
            mutation_class: MutationClass::Structure,
            scope: ScopedSemanticSubject::new(
                self.document_scope.clone(),
                self.document.clone(),
                SemanticScope::Document,
            ),
        });
        writes.insert(AssociatedWriteRequirement {
            family: OperationFamily::AppendEntity,
            mutation_class: MutationClass::Schema,
            scope: ScopedSemanticSubject::new(
                self.document_scope.clone(),
                self.document.clone(),
                SemanticScope::Document,
            ),
        });
        if entities.iter().any(|entity| {
            entity
                .fields
                .values()
                .any(|value| matches!(value, Value::Formula(_)))
        }) {
            writes.insert(AssociatedWriteRequirement {
                family: OperationFamily::AppendEntity,
                mutation_class: MutationClass::Formula,
                scope: ScopedSemanticSubject::new(
                    self.document_scope.clone(),
                    self.document.clone(),
                    SemanticScope::Document,
                ),
            });
        }
        *candidate = appended;
        Ok(())
    }

    fn plan_remove_collection(
        &self,
        _base: &Document,
        candidate: &mut Document,
        schema: &SchemaId,
        entities: &[EntityId],
        writes: &mut BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<(), WorkspaceError> {
        if !candidate.schemas.contains_key(schema) {
            return Err(WorkspaceError::MissingSchema {
                schema: schema.clone(),
            });
        }
        let expected = candidate
            .entities
            .values()
            .filter(|entity| entity.schema == *schema)
            .map(|entity| entity.id.clone())
            .collect::<BTreeSet<_>>();
        let selected = entities.iter().cloned().collect::<BTreeSet<_>>();
        if expected != selected {
            return Err(WorkspaceError::MissingEntityId {
                entity: entities
                    .iter()
                    .find(|entity| !expected.contains(*entity))
                    .cloned()
                    .or_else(|| {
                        expected
                            .iter()
                            .find(|entity| !selected.contains(*entity))
                            .cloned()
                    })
                    .unwrap_or_else(|| EntityId::from("collection")),
            });
        }
        let contains_formula = entities.iter().any(|entity| {
            candidate.entities.get(entity).is_some_and(|record| {
                record
                    .fields
                    .values()
                    .any(|value| matches!(value, Value::Formula(_)))
            })
        });
        for entity in entities {
            candidate.entities.remove(entity);
        }
        candidate.schemas.remove(schema);
        for class in [
            MutationClass::Structure,
            MutationClass::Schema,
            MutationClass::Destructive,
        ] {
            writes.insert(AssociatedWriteRequirement {
                family: OperationFamily::RemoveEntity,
                mutation_class: class,
                scope: ScopedSemanticSubject::new(
                    self.document_scope.clone(),
                    self.document.clone(),
                    SemanticScope::Document,
                ),
            });
        }
        if contains_formula {
            writes.insert(AssociatedWriteRequirement {
                family: OperationFamily::RemoveEntity,
                mutation_class: MutationClass::Formula,
                scope: ScopedSemanticSubject::new(
                    self.document_scope.clone(),
                    self.document.clone(),
                    SemanticScope::Document,
                ),
            });
        }
        Ok(())
    }

    fn schema_scope(&self, schema: &SchemaId) -> ScopedSemanticSubject {
        ScopedSemanticSubject::new(
            self.document_scope.clone(),
            self.document.clone(),
            SemanticScope::Schema(schema.clone()),
        )
    }

    fn schema_field_scope(&self, schema: &SchemaId, field: &FieldId) -> ScopedSemanticSubject {
        ScopedSemanticSubject::new(
            self.document_scope.clone(),
            self.document.clone(),
            SemanticScope::SchemaField {
                schema: schema.clone(),
                field: field.clone(),
            },
        )
    }

    fn plan_append_schema_field(
        &self,
        base: &Document,
        candidate: &mut Document,
        schema_id: &SchemaId,
        field: &FieldDefinition,
        values: &BTreeMap<EntityId, Value>,
        writes: &mut BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<(), WorkspaceError> {
        if !matches!(
            field.field_type,
            FieldType::Text | FieldType::Number | FieldType::Boolean | FieldType::Date
        ) {
            return Err(WorkspaceError::TypeMismatch {
                field: FieldRef::new("schema", field.id.clone()),
            });
        }
        let schema =
            candidate
                .schemas
                .get(schema_id)
                .ok_or_else(|| WorkspaceError::MissingSchema {
                    schema: schema_id.clone(),
                })?;
        if !base.schemas.contains_key(schema_id) || schema.fields.contains_key(&field.id) {
            return Err(WorkspaceError::GeneratedIdCollision {
                kind: super::SemanticIdKind::Field,
                id: field.id.to_string(),
            });
        }
        if schema
            .fields
            .values()
            .any(|existing| existing.key == field.key)
        {
            return Err(WorkspaceError::FieldKeyAlreadyExists {
                schema: schema.key.clone(),
                field: field.key.clone(),
            });
        }
        let entities = candidate
            .entities
            .values()
            .filter(|entity| entity.schema == *schema_id)
            .map(|entity| entity.id.clone())
            .collect::<BTreeSet<_>>();
        if (field.required && values.keys().cloned().collect::<BTreeSet<_>>() != entities)
            || values.keys().any(|id| !entities.contains(id))
        {
            return Err(WorkspaceError::MissingEntityId {
                entity: entities
                    .iter()
                    .find(|id| !values.contains_key(*id))
                    .cloned()
                    .unwrap_or_else(|| EntityId::from("field-values")),
            });
        }
        for (entity, value) in values {
            if !matches!(
                (value, &field.field_type),
                (Value::Text(_), FieldType::Text)
                    | (Value::Number(_), FieldType::Number)
                    | (Value::Boolean(_), FieldType::Boolean)
                    | (Value::Date(_), FieldType::Date)
            ) {
                return Err(WorkspaceError::TypeMismatch {
                    field: FieldRef::new(entity.clone(), field.id.clone()),
                });
            }
        }
        let definition = candidate
            .schemas
            .get_mut(schema_id)
            .expect("checked schema");
        definition.insert_field(field.clone());
        for (entity, value) in values {
            candidate
                .entities
                .get_mut(entity)
                .expect("checked entity")
                .fields
                .insert(field.id.clone(), value.clone());
        }
        // Check the post-append shape so the exact inverse of removing a sole
        // optional field can restore it from an empty schema. Failed admission
        // discards this private candidate without publishing a prefix.
        let schema = candidate.schemas.get(schema_id).expect("checked schema");
        if !field.required && !has_native_scalar_shape(candidate, schema) {
            return Err(WorkspaceError::TypeMismatch {
                field: FieldRef::new("schema", field.id.clone()),
            });
        }
        let scope = self.schema_field_scope(schema_id, &field.id);
        for mutation_class in [MutationClass::Structure, MutationClass::Schema] {
            writes.insert(AssociatedWriteRequirement {
                family: OperationFamily::SchemaFieldMutation,
                mutation_class,
                scope: scope.clone(),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Admission verifies every removal invariant before mutation.
    fn plan_remove_schema_field(
        &self,
        candidate: &mut Document,
        schema_id: &SchemaId,
        field: &FieldDefinition,
        values: &BTreeMap<EntityId, Value>,
        writes: &mut BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<(), WorkspaceError> {
        let schema =
            candidate
                .schemas
                .get(schema_id)
                .ok_or_else(|| WorkspaceError::MissingSchema {
                    schema: schema_id.clone(),
                })?;
        if schema.fields.get(&field.id) != Some(field) {
            return Err(WorkspaceError::MissingField {
                field: FieldRef::new("schema", field.id.clone()),
            });
        }
        // The inverse optional append must admit this exact restored shape.
        if !field.required && !has_native_scalar_shape(candidate, schema) {
            return Err(WorkspaceError::TypeMismatch {
                field: FieldRef::new("schema", field.id.clone()),
            });
        }
        if !matches!(
            field.field_type,
            FieldType::Text | FieldType::Number | FieldType::Boolean | FieldType::Date
        ) {
            return Err(WorkspaceError::MissingField {
                field: FieldRef::new("schema", field.id.clone()),
            });
        }
        let entities = candidate
            .entities
            .values()
            .filter(|entity| entity.schema == *schema_id)
            .map(|entity| entity.id.clone())
            .collect::<BTreeSet<_>>();
        if values.keys().any(|id| !entities.contains(id))
            || entities.iter().any(|id| {
                candidate
                    .entities
                    .get(id)
                    .and_then(|entity| entity.fields.get(&field.id))
                    != values.get(id)
            })
        {
            return Err(WorkspaceError::MissingEntityId {
                entity: entities
                    .iter()
                    .find(|id| {
                        values.get(*id)
                            != candidate
                                .entities
                                .get(*id)
                                .and_then(|entity| entity.fields.get(&field.id))
                    })
                    .cloned()
                    .unwrap_or_else(|| EntityId::from("field-values")),
            });
        }
        if values
            .values()
            .any(|value| matches!(value, Value::Formula(_)))
        {
            return Err(WorkspaceError::TypeMismatch {
                field: FieldRef::new(schema_id.to_string(), field.id.clone()),
            });
        }
        let schema_entities = entities.clone();
        let field_is_referenced_by_formula = candidate
            .entities
            .values()
            .flat_map(|entity| {
                entity
                    .fields
                    .iter()
                    .map(move |(field_id, value)| (entity.id.clone(), field_id, value))
            })
            .any(|(entity_id, field_id, value)| match value {
                Value::Formula(expression) => expression_references(
                    candidate,
                    &FieldRef::new(entity_id, field_id.clone()),
                    expression,
                )
                .iter()
                .any(|reference| {
                    reference.field == field.id && schema_entities.contains(&reference.entity)
                }),
                _ => false,
            });
        // A column total names the column itself, so it blocks removal even
        // when the schema has no rows to expand into references.
        let field_is_totaled_by_formula = candidate.entities.values().any(|entity| {
            entity.fields.values().any(|value| {
                matches!(value, Value::Formula(expression)
                    if expression_names_column(expression, schema_id, &field.id))
            })
        });
        let field_is_referenced_by_definition = candidate
            .keyed_grouped_sum_definitions
            .values()
            .any(|definition| {
                (definition.orders.schema == *schema_id
                    && (definition.orders.lookup_key_field == field.id
                        || definition.orders.quantity_field == field.id))
                    || (definition.products.schema == *schema_id
                        && (definition.products.key_field == field.id
                            || definition.products.category_field == field.id
                            || definition.products.price_field == field.id))
            });
        if field_is_referenced_by_formula
            || field_is_totaled_by_formula
            || field_is_referenced_by_definition
        {
            return Err(WorkspaceError::SchemaFieldReferenced {
                field: FieldRef::new(schema_id.to_string(), field.id.clone()),
            });
        }
        let definition = candidate
            .schemas
            .get_mut(schema_id)
            .expect("checked schema");
        definition.fields.remove(&field.id);
        definition.normalize_field_order();
        for entity in &entities {
            candidate
                .entities
                .get_mut(entity)
                .expect("checked entity")
                .fields
                .remove(&field.id);
        }
        let scope = self.schema_field_scope(schema_id, &field.id);
        for mutation_class in [
            MutationClass::Structure,
            MutationClass::Schema,
            MutationClass::Destructive,
        ] {
            writes.insert(AssociatedWriteRequirement {
                family: OperationFamily::SchemaFieldMutation,
                mutation_class,
                scope: scope.clone(),
            });
        }
        Ok(())
    }

    fn plan_rename_schema_field(
        &self,
        candidate: &mut Document,
        schema_id: &SchemaId,
        field_id: &FieldId,
        key: &FieldKey,
        writes: &mut BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<(), WorkspaceError> {
        let schema =
            candidate
                .schemas
                .get(schema_id)
                .ok_or_else(|| WorkspaceError::MissingSchema {
                    schema: schema_id.clone(),
                })?;
        if schema
            .fields
            .values()
            .any(|field| field.id != *field_id && field.key == *key)
        {
            return Err(WorkspaceError::FieldKeyAlreadyExists {
                schema: schema.key.clone(),
                field: key.clone(),
            });
        }
        let field = candidate
            .schemas
            .get_mut(schema_id)
            .expect("checked schema")
            .fields
            .get_mut(field_id)
            .ok_or_else(|| WorkspaceError::MissingField {
                field: FieldRef::new("schema", field_id.clone()),
            })?;
        if !matches!(
            field.field_type,
            FieldType::Text | FieldType::Number | FieldType::Boolean | FieldType::Date
        ) {
            return Err(WorkspaceError::MissingField {
                field: FieldRef::new("schema", field_id.clone()),
            });
        }
        if field.key == *key {
            return Err(WorkspaceError::NoChange {
                field: FieldRef::new("schema", field_id.clone()),
            });
        }
        field.key = key.clone();
        writes.insert(AssociatedWriteRequirement {
            family: OperationFamily::SchemaFieldMutation,
            mutation_class: MutationClass::Schema,
            scope: self.schema_field_scope(schema_id, field_id),
        });
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Keep the closed command catalogue together.
    fn plan_commands(
        &self,
        document: &Document,
        body: &SemanticPatchBody,
    ) -> Result<(Document, BTreeSet<AssociatedWriteRequirement>), WorkspaceError> {
        admit_command_names(body)?;
        #[cfg(test)]
        super::planning_counters::record_document_clone();
        let mut candidate = document.clone();
        let mut writes = BTreeSet::new();
        let mut field_edits = FieldEditPlanning::new();
        let mut append_entities = EntityAppendPlanning::default();
        for command in body.commands() {
            // Only the cell edits keep the batch state; every other command
            // may change the structures and keys it rests on.
            if !matches!(
                command,
                SemanticCommand::SetFieldValue { .. }
                    | SemanticCommand::UnsetField { .. }
                    | SemanticCommand::FormulaUpdate(_)
                    | SemanticCommand::FormulaRestore(_)
                    | SemanticCommand::FormulaInverseRestore(_)
            ) {
                field_edits = FieldEditPlanning::new();
            }
            // Only a contiguous run of appends keeps this proof, so any other
            // command starts the next run from a fresh document check.
            if !matches!(command, SemanticCommand::AppendEntity { .. }) {
                append_entities = EntityAppendPlanning::default();
            }
            match command {
                SemanticCommand::AppendCollection { schema, entities } => {
                    self.plan_append_collection(
                        document,
                        &mut candidate,
                        schema,
                        entities,
                        &mut writes,
                    )?;
                }
                SemanticCommand::RemoveCollection { schema, entities } => {
                    self.plan_remove_collection(
                        document,
                        &mut candidate,
                        schema,
                        entities,
                        &mut writes,
                    )?;
                }
                SemanticCommand::AppendEntity { entity } => {
                    self.plan_append_entity(
                        document,
                        &mut candidate,
                        entity,
                        &mut append_entities,
                        &mut writes,
                    )?;
                }
                SemanticCommand::RemoveEntity { entity } => {
                    let record = candidate.entities.get(entity).ok_or_else(|| {
                        WorkspaceError::MissingEntityId {
                            entity: entity.clone(),
                        }
                    })?;
                    let scope = ScopedSemanticSubject::new(
                        self.document_scope.clone(),
                        self.document.clone(),
                        SemanticScope::Entity {
                            entity: entity.clone(),
                            schema: record.schema.clone(),
                        },
                    );
                    for mutation_class in [MutationClass::Structure, MutationClass::Destructive] {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::RemoveEntity,
                            mutation_class,
                            scope: scope.clone(),
                        });
                    }
                    if record
                        .fields
                        .values()
                        .any(|value| matches!(value, Value::Formula(_)))
                    {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::RemoveEntity,
                            mutation_class: MutationClass::Formula,
                            scope,
                        });
                    }
                    candidate.entities.remove(entity);
                }
                SemanticCommand::AppendSchemaField {
                    schema,
                    field,
                    values,
                } => {
                    self.plan_append_schema_field(
                        document,
                        &mut candidate,
                        schema,
                        field,
                        values,
                        &mut writes,
                    )?;
                }
                SemanticCommand::RemoveSchemaField {
                    schema,
                    field,
                    values,
                } => {
                    self.plan_remove_schema_field(
                        &mut candidate,
                        schema,
                        field,
                        values,
                        &mut writes,
                    )?;
                }
                SemanticCommand::RenameSchemaField { schema, field, key } => {
                    self.plan_rename_schema_field(&mut candidate, schema, field, key, &mut writes)?;
                }
                SemanticCommand::SetSchemaFieldDisplayName {
                    schema,
                    field,
                    display_name,
                } => {
                    let definition = candidate
                        .schemas
                        .get_mut(schema)
                        .ok_or_else(|| WorkspaceError::MissingSchema {
                            schema: schema.clone(),
                        })?
                        .fields
                        .get_mut(field)
                        .ok_or_else(|| WorkspaceError::MissingSchemaField {
                            schema: schema.clone(),
                            field: field.clone(),
                        })?;
                    definition.display_name.clone_from(display_name);
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::SchemaFieldMutation,
                        mutation_class: MutationClass::Schema,
                        scope: self.schema_field_scope(schema, field),
                    });
                }
                SemanticCommand::SetSchemaFieldRequiredness {
                    schema,
                    field,
                    required,
                } => {
                    let record = candidate.schemas.get(schema).ok_or_else(|| {
                        WorkspaceError::MissingSchema {
                            schema: schema.clone(),
                        }
                    })?;
                    let definition = record.fields.get(field).ok_or_else(|| {
                        WorkspaceError::MissingSchemaField {
                            schema: schema.clone(),
                            field: field.clone(),
                        }
                    })?;
                    if !matches!(
                        definition.field_type,
                        FieldType::Text | FieldType::Number | FieldType::Boolean | FieldType::Date
                    ) || !has_native_scalar_shape(&candidate, record)
                    {
                        return Err(WorkspaceError::TypeMismatch {
                            field: FieldRef::new("schema", field.clone()),
                        });
                    }
                    if *required {
                        if let Some(entity) = candidate.entities.values().find(|entity| {
                            entity.schema == *schema && !entity.fields.contains_key(field)
                        }) {
                            return Err(WorkspaceError::MissingField {
                                field: FieldRef::new(entity.id.clone(), field.clone()),
                            });
                        }
                    }
                    candidate
                        .schemas
                        .get_mut(schema)
                        .expect("checked schema")
                        .fields
                        .get_mut(field)
                        .expect("checked field")
                        .required = *required;
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::SchemaFieldMutation,
                        mutation_class: MutationClass::Schema,
                        scope: self.schema_field_scope(schema, field),
                    });
                }
                SemanticCommand::SetFieldNumberFormat {
                    schema,
                    field,
                    number_format,
                } => {
                    let definition = candidate
                        .schemas
                        .get_mut(schema)
                        .ok_or_else(|| WorkspaceError::MissingSchema {
                            schema: schema.clone(),
                        })?
                        .fields
                        .get_mut(field)
                        .ok_or_else(|| WorkspaceError::MissingSchemaField {
                            schema: schema.clone(),
                            field: field.clone(),
                        })?;
                    if definition.field_type != FieldType::Number {
                        return Err(WorkspaceError::InvalidNumberFormat {
                            source: super::NumberFormatError::NonNumberField,
                        });
                    }
                    if let Some(format) = number_format {
                        format
                            .validate_for(&definition.field_type)
                            .map_err(|source| WorkspaceError::InvalidNumberFormat { source })?;
                    }
                    definition.number_format.clone_from(number_format);
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::SchemaFieldMutation,
                        mutation_class: MutationClass::Schema,
                        scope: self.schema_field_scope(schema, field),
                    });
                }
                SemanticCommand::SetSchemaFieldOrder { schema, order } => {
                    let definition = candidate.schemas.get_mut(schema).ok_or_else(|| {
                        WorkspaceError::MissingSchema {
                            schema: schema.clone(),
                        }
                    })?;
                    // A proposed order must name every field once; it is
                    // never repaired.
                    if let Some(order) = order {
                        let unique = order.iter().collect::<std::collections::BTreeSet<_>>();
                        if order.len() != definition.fields.len()
                            || unique.len() != order.len()
                            || !order.iter().all(|id| definition.fields.contains_key(id))
                        {
                            return Err(WorkspaceError::InvalidFieldOrder {
                                schema: schema.clone(),
                            });
                        }
                    }
                    definition.field_order.clone_from(order);
                    definition.normalize_field_order();
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::SchemaFieldMutation,
                        mutation_class: MutationClass::Schema,
                        scope: self.schema_scope(schema),
                    });
                }
                SemanticCommand::SetFieldConstraint {
                    schema,
                    field,
                    constraint,
                } => {
                    let schema_record = candidate.schemas.get_mut(schema).ok_or_else(|| {
                        WorkspaceError::MissingSchema {
                            schema: schema.clone(),
                        }
                    })?;
                    let definition = schema_record.fields.get_mut(field).ok_or_else(|| {
                        WorkspaceError::MissingSchemaField {
                            schema: schema.clone(),
                            field: field.clone(),
                        }
                    })?;
                    definition.constraint = constraint.clone();
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::SchemaFieldMutation,
                        mutation_class: MutationClass::Schema,
                        scope: self.schema_field_scope(schema, field),
                    });
                }
                SemanticCommand::SetFieldValue { field, value } => {
                    let entity = candidate.entities.get(&field.entity).ok_or_else(|| {
                        WorkspaceError::MissingEntityId {
                            entity: field.entity.clone(),
                        }
                    })?;
                    let existing = entity.fields.get(&field.field);
                    let mutation_classes = classify_field_transition(existing, value);
                    let scope = self.field_scope(&candidate, field)?;
                    for mutation_class in mutation_classes {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::SetFieldValue,
                            mutation_class,
                            scope: scope.clone(),
                        });
                    }
                    apply_field_value_candidate_with_planning(
                        &mut candidate,
                        field,
                        value,
                        &mut field_edits,
                    )?;
                }
                SemanticCommand::UnsetField { field } => {
                    let entity = candidate.entities.get(&field.entity).ok_or_else(|| {
                        WorkspaceError::MissingEntityId {
                            entity: field.entity.clone(),
                        }
                    })?;
                    let existing = entity.fields.get(&field.field);
                    let scope = self.field_scope(&candidate, field)?;
                    for mutation_class in [MutationClass::Value, MutationClass::Destructive] {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::SetFieldValue,
                            mutation_class,
                            scope: scope.clone(),
                        });
                    }
                    if matches!(existing, Some(Value::Formula(_))) {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::SetFieldValue,
                            mutation_class: MutationClass::Formula,
                            scope,
                        });
                    }
                    unset_field_candidate_in_place(&mut candidate, field, &mut field_edits)?;
                }
                SemanticCommand::FormulaUpdate(command) => {
                    let field = command.target();
                    let scope = self.field_scope(&candidate, field)?;
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::FormulaUpdate,
                        mutation_class: MutationClass::Formula,
                        scope,
                    });
                    formula_update_candidate_in_place(
                        &mut candidate,
                        field,
                        command.expression(),
                        &mut field_edits,
                    )?;
                }
                SemanticCommand::FormulaInverseRestore(command) => {
                    let field = command.target();
                    let scope = self.field_scope(&candidate, field)?;
                    for mutation_class in [
                        MutationClass::Value,
                        MutationClass::Formula,
                        MutationClass::Destructive,
                    ] {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::FormulaInverseRestore,
                            mutation_class,
                            scope: scope.clone(),
                        });
                    }
                    formula_inverse_restore_candidate_in_place(
                        &mut candidate,
                        field,
                        command.value(),
                        &mut field_edits,
                    )?;
                }
                SemanticCommand::FormulaRestore(command) => {
                    let field = command.target();
                    let scope = self.field_scope(&candidate, field)?;
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::FormulaUpdate,
                        mutation_class: MutationClass::Formula,
                        scope,
                    });
                    formula_restore_candidate_in_place(
                        &mut candidate,
                        field,
                        command.expression(),
                        &mut field_edits,
                    )?;
                }
                SemanticCommand::UpsertKeyedGroupedSumDefinition { definition } => {
                    // ADR-0036: create and update both require only
                    // `Structure`; an in-place update with the same stable
                    // identity is not a `Destructive` capability.
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::KeyedGroupedSumDefinition,
                        mutation_class: MutationClass::Structure,
                        scope: ScopedSemanticSubject::new(
                            self.document_scope.clone(),
                            self.document.clone(),
                            SemanticScope::Document,
                        ),
                    });
                    candidate
                        .keyed_grouped_sum_definitions
                        .insert(definition.id.clone(), definition.clone());
                }
                SemanticCommand::RemoveKeyedGroupedSumDefinition { definition } => {
                    if candidate
                        .keyed_grouped_sum_definitions
                        .remove(definition)
                        .is_none()
                    {
                        return Err(WorkspaceError::MissingKeyedGroupedSumDefinition {
                            definition: definition.clone(),
                        });
                    }
                    let scope = ScopedSemanticSubject::new(
                        self.document_scope.clone(),
                        self.document.clone(),
                        SemanticScope::Document,
                    );
                    for mutation_class in [MutationClass::Structure, MutationClass::Destructive] {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::KeyedGroupedSumDefinition,
                            mutation_class,
                            scope: scope.clone(),
                        });
                    }
                }
            }
        }
        Ok((candidate, writes))
    }

    /// Validate one snapshot and preflight its formula projections.
    ///
    /// Invalid documents keep the `ValidationFailed` shape; every other
    /// failure keeps the `CommandRejected` shape.
    fn validate_and_preflight(
        document: &Document,
    ) -> Result<(ValidationReport, Calculation), PatchLifecycleError> {
        let (report, calculation) =
            super::validated_calculation_and_report(document).map_err(|error| match error {
                WorkspaceError::InvalidDocument { report, .. } => {
                    PatchLifecycleError::ValidationFailed { report }
                }
                source => PatchLifecycleError::CommandRejected {
                    source: Box::new(source),
                },
            })?;
        super::preflight_formula_projections(document).map_err(|source| {
            PatchLifecycleError::CommandRejected {
                source: Box::new(source),
            }
        })?;
        Ok((report, calculation))
    }

    fn finalize_evaluation(
        &self,
        base: &Document,
        candidate: Document,
        body: &SemanticPatchBody,
        associated_write_requirements: BTreeSet<AssociatedWriteRequirement>,
        prepared: Option<&PreparedEvaluation<'_>>,
    ) -> Result<EvaluatedPatch, PatchLifecycleError> {
        #[cfg(test)]
        self.evaluations.set(self.evaluations.get() + 1);
        if self.evidence_profile == EvidenceProfile::V2 {
            return self.finalize_evaluation_v2(
                base,
                candidate,
                body,
                associated_write_requirements,
                prepared,
            );
        }
        reject_legacy_number_formats(base, &candidate, body)?;
        reject_legacy_field_orders(base, &candidate, body)?;
        if contains_constraint_command(body)
            || document_has_field_constraints(base)
            || document_has_field_constraints(&candidate)
        {
            return Err(PatchLifecycleError::CommandRejected {
                source: Box::new(WorkspaceError::Diff(
                    super::DiffError::UnsupportedFieldConstraintChange,
                )),
            });
        }
        if base.has_display_names() || candidate.has_display_names() {
            return Err(PatchLifecycleError::CommandRejected {
                source: Box::new(WorkspaceError::Diff(
                    super::DiffError::UnsupportedDisplayName,
                )),
            });
        }
        // V1 has no evidence vocabulary for saved grouped-summary definition
        // changes, and formulas cannot read definitions. Only definition-only
        // batches may use an empty projection; mixed batches must be rejected
        // so this path cannot omit ordinary evidence or per-change admission.
        if base.keyed_grouped_sum_definitions != candidate.keyed_grouped_sum_definitions {
            if body.commands().iter().any(|command| {
                !matches!(
                    command,
                    SemanticCommand::UpsertKeyedGroupedSumDefinition { .. }
                        | SemanticCommand::RemoveKeyedGroupedSumDefinition { .. }
                )
            }) {
                return Err(PatchLifecycleError::CommandRejected {
                    source: Box::new(WorkspaceError::Diff(
                        super::DiffError::UnsupportedKeyedGroupedSumDefinitionChange,
                    )),
                });
            }
            let (validation_report, _calculation) = Self::validate_and_preflight(&candidate)?;
            let disclosure_requirements = self.derive_disclosures(base, &candidate, body, &[])?;
            return Ok(EvaluatedPatch {
                validation_report,
                document: candidate,
                evidence: PatchEvidence::V1(Vec::new()),
                formula_impacts: Vec::new(),
                footprint: AuthorizationFootprint {
                    disclosure_requirements,
                    associated_write_requirements,
                },
            });
        }
        let (edit, validation_report) =
            match finalize_edit_with_validation(base, candidate, prepared) {
                Ok(result) => result,
                Err(WorkspaceError::InvalidDocument { report, .. }) => {
                    return Err(PatchLifecycleError::ValidationFailed { report });
                }
                Err(source) => {
                    return Err(PatchLifecycleError::CommandRejected {
                        source: Box::new(source),
                    });
                }
            };
        let semantic_changes = edit.diff.changes().to_vec();
        let formula_impacts = formula_impacts(&semantic_changes);
        let disclosure_requirements =
            self.derive_disclosures(base, &edit.document, body, &semantic_changes)?;
        Ok(EvaluatedPatch {
            document: edit.document,
            evidence: PatchEvidence::V1(semantic_changes),
            formula_impacts,
            validation_report,
            footprint: AuthorizationFootprint {
                disclosure_requirements,
                associated_write_requirements,
            },
        })
    }

    fn finalize_evaluation_v2(
        &self,
        base: &Document,
        candidate: Document,
        body: &SemanticPatchBody,
        associated_write_requirements: BTreeSet<AssociatedWriteRequirement>,
        prepared: Option<&PreparedEvaluation<'_>>,
    ) -> Result<EvaluatedPatch, PatchLifecycleError> {
        // Only a successful evaluation of this exact candidate against a base
        // that today's path would accept can replace today's validation,
        // calculation and evidence work; every other case keeps it unchanged.
        let reusable = prepared
            .filter(|prepared| {
                prepared.document() == &candidate
                    && matches!(prepared.verdict(), PreparedVerdict::Valid)
                    && prepared.base_report_valid()
                    && super::validate_keyed_grouped_sum_definitions(base).is_ok()
            })
            .and_then(|prepared| {
                Some((
                    prepared.report(),
                    prepared.base_calculation()?,
                    prepared.calculation()?,
                    prepared.projection_changes(),
                ))
            });
        // Formula impact remains derived evidence. Run the frozen formula/diff
        // projection over snapshots without names, constraints, or saved definitions.
        // Formulas cannot read definitions; both real snapshots passed complete
        // validation above. Direct v2 evidence is always produced separately.
        let (validation_report, delta, formula_changes) =
            if let Some((report, base_calculation, candidate_calculation, changes)) = reusable {
                // The prepared candidate is already completely validated and
                // calculated. Both snapshots still need the formula projection
                // preflight today's path runs, base first.
                super::preflight_formula_projections(base).map_err(|source| {
                    PatchLifecycleError::CommandRejected {
                        source: Box::new(source),
                    }
                })?;
                super::preflight_formula_projections(&candidate).map_err(|source| {
                    PatchLifecycleError::CommandRejected {
                        source: Box::new(source),
                    }
                })?;
                // Both documents are admitted above and the prepared evaluation
                // found every differing schema and entity, so the delta only
                // has to compare those, the title and the saved definitions.
                let delta = canonical_delta_scoped(
                    CANONICAL_SEMANTIC_DELTA_V2,
                    base,
                    &candidate,
                    base_calculation,
                    candidate_calculation,
                    &changes.diff_scope(),
                )
                .map_err(|source| PatchLifecycleError::CommandRejected {
                    source: Box::new(WorkspaceError::CanonicalDelta(Box::new(source))),
                })?;
                // The prepared evaluation found every differing schema and
                // entity, so the projection only has to compare those.
                let formula_changes = super::diff_changes_scoped(
                    &formula_projection_document(base),
                    &formula_projection_document(&candidate),
                    base_calculation,
                    candidate_calculation,
                    &changes.diff_scope(),
                );
                (report.clone(), delta, formula_changes)
            } else {
                let (_, base_calculation) = Self::validate_and_preflight(base)?;
                #[cfg(test)]
                super::evaluation_counters::record_base_calculation();
                let (report, candidate_calculation) = Self::validate_and_preflight(&candidate)?;
                let delta = canonical_delta(CANONICAL_SEMANTIC_DELTA_V2, base, &candidate)
                    .map_err(|source| PatchLifecycleError::CommandRejected {
                        source: Box::new(WorkspaceError::CanonicalDelta(Box::new(source))),
                    })?;
                let formula_projection = super::diff_with_calculations(
                    &formula_projection_document(base),
                    &formula_projection_document(&candidate),
                    &base_calculation,
                    &candidate_calculation,
                )
                .map_err(|source| PatchLifecycleError::CommandRejected {
                    source: Box::new(source.into()),
                })?;
                (report, delta, formula_projection.changes().to_vec())
            };
        let formula_impacts = formula_impacts(&formula_changes);
        let disclosure_requirements =
            self.derive_disclosures(base, &candidate, body, &formula_changes)?;
        Ok(EvaluatedPatch {
            document: candidate,
            evidence: PatchEvidence::V2(delta),
            formula_impacts,
            validation_report,
            footprint: AuthorizationFootprint {
                disclosure_requirements,
                associated_write_requirements,
            },
        })
    }

    fn evaluate_patch(
        &self,
        document: &Document,
        patch: &SemanticPatch,
    ) -> Result<EvaluatedPatch, PatchLifecycleError> {
        let (candidate, writes) = self
            .plan_commands(document, patch.exact_change.body())
            .map_err(|source| PatchLifecycleError::CommandRejected {
                source: Box::new(source),
            })?;
        self.finalize_evaluation(document, candidate, patch.exact_change.body(), writes, None)
    }

    /// Run one read-only step over the retained evaluation for this exact base
    /// and patch, or over a fresh evaluation when the memo cannot be reused.
    ///
    /// The memo is moved out for the duration so the step can borrow it while
    /// it mutates the lifecycle, and put back unchanged afterwards. A miss
    /// leaves the retained slot untouched, as before.
    fn with_evaluated<R>(
        &mut self,
        document: &Document,
        patch: &SemanticPatch,
        read: impl FnOnce(&mut Self, &EvaluatedPatch) -> Result<R, PatchLifecycleError>,
    ) -> Result<R, PatchLifecycleError> {
        let memo = self.evaluation_memo.take();
        let retained = memo
            .as_ref()
            .filter(|memo| memo.base == *document && memo.patch == *patch);
        let result = match retained {
            Some(memo) => read(self, &memo.evaluated),
            None => match self.evaluate_patch(document, patch) {
                Ok(evaluated) => read(self, &evaluated),
                Err(error) => Err(error),
            },
        };
        self.evaluation_memo = memo;
        result
    }

    pub(crate) fn field_scope(
        &self,
        document: &Document,
        field: &FieldRef,
    ) -> Result<ScopedSemanticSubject, WorkspaceError> {
        let entity = document.entities.get(&field.entity).ok_or_else(|| {
            WorkspaceError::MissingEntityId {
                entity: field.entity.clone(),
            }
        })?;
        if !document
            .schemas
            .get(&entity.schema)
            .is_some_and(|schema| schema.fields.contains_key(&field.field))
        {
            return Err(WorkspaceError::MissingField {
                field: field.clone(),
            });
        }
        Ok(ScopedSemanticSubject::new(
            self.document_scope.clone(),
            self.document.clone(),
            SemanticScope::EntityField {
                entity: field.entity.clone(),
                schema: entity.schema.clone(),
                field: field.field.clone(),
            },
        ))
    }

    fn entity_scope(
        &self,
        document: &Document,
        entity: &EntityId,
    ) -> Result<ScopedSemanticSubject, PatchLifecycleError> {
        let entity_record = document
            .entities
            .get(entity)
            .ok_or(PatchLifecycleError::ScopeDerivationFailed)?;
        Ok(ScopedSemanticSubject::new(
            self.document_scope.clone(),
            self.document.clone(),
            SemanticScope::Entity {
                entity: entity.clone(),
                schema: entity_record.schema.clone(),
            },
        ))
    }

    fn derive_disclosures(
        &self,
        before: &Document,
        after: &Document,
        body: &SemanticPatchBody,
        changes: &[SemanticChange],
    ) -> Result<BTreeSet<DisclosureRequirement>, PatchLifecycleError> {
        let mut derivation = DisclosureDerivation::new(self, before, after, body);
        for command in body.commands() {
            derivation.insert_command_disclosures(command)?;
        }
        for change in changes {
            derivation.insert_change_disclosures(change)?;
        }
        Ok(derivation.disclosures)
    }

    fn keyed_grouped_sum_query_disclosures(
        &self,
        document: &Document,
        definition_id: &KeyedGroupedSumDefinitionId,
    ) -> Result<BTreeSet<DisclosureRequirement>, PatchLifecycleError> {
        let definition = document
            .keyed_grouped_sum_definitions
            .get(definition_id)
            .ok_or(PatchLifecycleError::ScopeDerivationFailed)?;
        validate_keyed_grouped_sum_bindings(document, definition)?;

        let mut disclosures =
            BTreeSet::from([self.keyed_grouped_sum_disclosure(SemanticScope::Document)]);

        // Candidate membership for both bound schemas.
        for subject in [
            SemanticScope::Schema(definition.orders.schema.clone()),
            SemanticScope::Schema(definition.products.schema.clone()),
        ] {
            disclosures.insert(self.keyed_grouped_sum_disclosure(subject));
        }

        // Required type facts for the always-bound Orders operands and the
        // complete Products key universe.
        for subject in keyed_grouped_sum_type_facts(definition) {
            disclosures.insert(self.keyed_grouped_sum_disclosure(subject));
        }

        let orders_fields = [
            &definition.orders.lookup_key_field,
            &definition.orders.quantity_field,
        ];
        let orders_entities =
            keyed_grouped_sum_schema_entities(document, &definition.orders.schema);
        let products_entities =
            keyed_grouped_sum_schema_entities(document, &definition.products.schema);

        // Orders membership plus every current Order's local key and quantity
        // are dependencies.
        for order in &orders_entities {
            for field in orders_fields {
                self.insert_keyed_grouped_sum_entity_field(
                    document,
                    &order.id,
                    field,
                    &mut disclosures,
                )?;
            }
        }

        // The entire Products key universe is a dependency, not only the rows
        // that currently match.
        for product in &products_entities {
            self.insert_keyed_grouped_sum_entity_field(
                document,
                &product.id,
                &definition.products.key_field,
                &mut disclosures,
            )?;
        }

        // A successful exact match additionally depends on that product's
        // category and price values. Unmatched and ambiguous Products disclose
        // only their key.
        let (matched_products, formula_operand_required) =
            keyed_grouped_sum_matched_products(document, definition);
        for product in &products_entities {
            if !matched_products.contains(&product.id) {
                continue;
            }
            for field in [
                &definition.products.category_field,
                &definition.products.price_field,
            ] {
                self.insert_keyed_grouped_sum_entity_field(
                    document,
                    &product.id,
                    field,
                    &mut disclosures,
                )?;
            }
        }
        if !matched_products.is_empty() {
            for subject in [
                SemanticScope::SchemaField {
                    schema: definition.products.schema.clone(),
                    field: definition.products.category_field.clone(),
                },
                SemanticScope::SchemaField {
                    schema: definition.products.schema.clone(),
                    field: definition.products.price_field.clone(),
                },
            ] {
                disclosures.insert(self.keyed_grouped_sum_disclosure(subject));
            }
        }

        // A required successfully matched formula-backed operand needs the
        // whole authoritative document Calculation outcome, including every
        // formula root that determines whether a complete CalculationState
        // exists. It is required only when such an operand participates, so an
        // all-literal Query never discloses unrelated formula roots.
        if formula_operand_required {
            self.insert_keyed_grouped_sum_formula_roots(document, &mut disclosures)?;
        }
        Ok(disclosures)
    }

    fn insert_keyed_grouped_sum_entity_field(
        &self,
        document: &Document,
        entity: &EntityId,
        field: &FieldId,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        disclosures.insert(DisclosureRequirement {
            family: OperationFamily::KeyedGroupedSumDefinition,
            scope: self
                .field_scope(document, &FieldRef::new(entity.clone(), field.clone()))
                .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?,
        });
        Ok(())
    }

    fn insert_keyed_grouped_sum_formula_roots(
        &self,
        document: &Document,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        for entity in document.entities.values() {
            for (field_id, value) in &entity.fields {
                if let Value::Formula(expression) = value {
                    self.insert_keyed_grouped_sum_entity_field(
                        document,
                        &entity.id,
                        field_id,
                        disclosures,
                    )?;
                    let owner = FieldRef::new(entity.id.clone(), field_id.clone());
                    for reference in expression_references(document, &owner, expression) {
                        self.insert_keyed_grouped_sum_entity_field(
                            document,
                            &reference.entity,
                            &reference.field,
                            disclosures,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn keyed_grouped_sum_disclosure(&self, subject: SemanticScope) -> DisclosureRequirement {
        DisclosureRequirement {
            family: OperationFamily::KeyedGroupedSumDefinition,
            scope: ScopedSemanticSubject::new(
                self.document_scope.clone(),
                self.document.clone(),
                subject,
            ),
        }
    }

    pub(crate) fn authorize_query(
        &self,
        subject: &PrincipalId,
        requirements: &BTreeSet<DisclosureRequirement>,
        now: TrustedInstant,
    ) -> Result<BTreeSet<GrantId>, PatchLifecycleError> {
        self.require_active_principal(subject)?;
        let mut used = BTreeSet::new();
        for required in requirements {
            let grant_id = self
                .grants
                .iter()
                .find_map(|(id, stored)| {
                    (Self::grant_is_live_for(stored, subject, now)
                        && stored.grant.requirements.iter().any(|binding| {
                            matches!(
                                binding,
                                GrantRequirement::Query { family, scope }
                                    if *family == required.family
                                        && scope_covers(scope, &required.scope)
                            )
                        }))
                    .then(|| id.clone())
                })
                .ok_or(PatchLifecycleError::InsufficientCapability {
                    action: AuthorizationAction::Query,
                })?;
            used.insert(grant_id);
        }
        Ok(used)
    }

    fn authorize_mutation(
        &self,
        subject: &PrincipalId,
        action: AuthorizationAction,
        requirements: &BTreeSet<AssociatedWriteRequirement>,
        now: TrustedInstant,
    ) -> Result<BTreeSet<GrantId>, PatchLifecycleError> {
        self.require_active_principal(subject)?;
        let mut used = BTreeSet::new();
        for required in requirements {
            let grant_id = self
                .grants
                .iter()
                .find_map(|(id, stored)| {
                    (Self::grant_is_live_for(stored, subject, now)
                        && stored.grant.requirements.iter().any(|binding| {
                            matches!(
                                binding,
                                GrantRequirement::Mutation {
                                    action: binding_action,
                                    family,
                                    mutation_class,
                                    scope,
                                } if *binding_action == action
                                    && *family == required.family
                                    && *mutation_class == required.mutation_class
                                    && scope_covers(scope, &required.scope)
                            )
                        }))
                    .then(|| id.clone())
                })
                .ok_or(PatchLifecycleError::InsufficientCapability { action })?;
            used.insert(grant_id);
        }
        Ok(used)
    }

    fn require_recorded_approve_grants(
        &self,
        stored: &StoredApproval,
        now: TrustedInstant,
    ) -> Result<(), PatchLifecycleError> {
        for id in &stored.approval.approve_grants {
            let grant = self
                .grants
                .get(id)
                .ok_or(PatchLifecycleError::InsufficientCapability {
                    action: AuthorizationAction::Approve,
                })?;
            if !Self::grant_is_live_for(grant, &stored.approval.approver, now) {
                return Err(PatchLifecycleError::InsufficientCapability {
                    action: AuthorizationAction::Approve,
                });
            }
        }
        for required in &stored.approval.binding.associated_write_requirements {
            let covered = stored.approval.approve_grants.iter().any(|id| {
                self.grants.get(id).is_some_and(|grant| {
                    Self::grant_is_live_for(grant, &stored.approval.approver, now)
                        && grant.grant.requirements.iter().any(|binding| {
                            matches!(
                                binding,
                                GrantRequirement::Mutation {
                                    action: AuthorizationAction::Approve,
                                    family,
                                    mutation_class,
                                    scope,
                                } if *family == required.family
                                    && *mutation_class == required.mutation_class
                                    && scope_covers(scope, &required.scope)
                            )
                        })
                })
            });
            if !covered {
                return Err(PatchLifecycleError::InsufficientCapability {
                    action: AuthorizationAction::Approve,
                });
            }
        }
        Ok(())
    }

    fn grant_is_live_for(stored: &StoredGrant, subject: &PrincipalId, now: TrustedInstant) -> bool {
        !stored.revoked
            && stored.grant.subject == *subject
            && stored.grant.expires_at.is_none_or(|expires| now < expires)
    }

    fn append_state(&mut self, id: &ProposalId, state: PatchLifecycleState) {
        if let Some(proposal) = self.proposals.get_mut(id) {
            if proposal_is_terminal(proposal) {
                return;
            }
            push_once(&mut proposal.history, state);
        }
    }
}

/// Which admitted document one derivation step runs against.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DisclosureDocumentSide {
    Before,
    After,
}

impl DisclosureDocumentSide {
    fn document<'document>(
        self,
        before: &'document Document,
        after: &'document Document,
    ) -> &'document Document {
        match self {
            Self::Before => before,
            Self::After => after,
        }
    }
}

/// How one field's scope resolves against the two admitted documents.
#[derive(Clone, Copy)]
enum FieldScopeSource {
    /// Command-derived fields resolve against the candidate, then the base.
    AfterThenBefore,
    /// A calculation walk resolves against the document it walks.
    Side(DisclosureDocumentSide),
}

/// The patch body's commands indexed by the field or entity they touch.
///
/// Every arm of `command_families_for_field` is indexed here, including the
/// entity-wide commands, so one semantic change never re-walks the batch.
#[derive(Default)]
struct PatchBodyIndex {
    field_families: BTreeMap<FieldRef, BTreeSet<OperationFamily>>,
    appended_entities: BTreeSet<EntityId>,
    removed_entities: BTreeSet<EntityId>,
    schema_field_values: BTreeSet<FieldRef>,
    requiredness_fields: BTreeSet<(SchemaId, FieldId)>,
    removed_schema_fields: BTreeSet<(SchemaId, FieldId)>,
    appended_schema_fields: BTreeSet<(SchemaId, FieldId)>,
    renamed_schema_fields: BTreeSet<(SchemaId, FieldId)>,
}

impl PatchBodyIndex {
    fn new(body: &SemanticPatchBody) -> Self {
        let mut index = Self::default();
        for command in body.commands() {
            match command {
                SemanticCommand::AppendCollection { entities, .. } => index
                    .appended_entities
                    .extend(entities.iter().map(|entity| entity.id.clone())),
                SemanticCommand::RemoveCollection { entities, .. } => {
                    index.removed_entities.extend(entities.iter().cloned());
                }
                SemanticCommand::AppendEntity { entity } => {
                    index.appended_entities.insert(entity.id.clone());
                }
                SemanticCommand::RemoveEntity { entity } => {
                    index.removed_entities.insert(entity.clone());
                }
                SemanticCommand::AppendSchemaField {
                    schema,
                    field,
                    values,
                } => {
                    index
                        .appended_schema_fields
                        .insert((schema.clone(), field.id.clone()));
                    index.schema_field_values.extend(
                        values
                            .keys()
                            .map(|entity| FieldRef::new(entity.clone(), field.id.clone())),
                    );
                }
                SemanticCommand::RemoveSchemaField {
                    schema,
                    field,
                    values,
                } => {
                    index
                        .removed_schema_fields
                        .insert((schema.clone(), field.id.clone()));
                    index.schema_field_values.extend(
                        values
                            .keys()
                            .map(|entity| FieldRef::new(entity.clone(), field.id.clone())),
                    );
                }
                SemanticCommand::RenameSchemaField { schema, field, .. } => {
                    index
                        .renamed_schema_fields
                        .insert((schema.clone(), field.clone()));
                }
                SemanticCommand::SetSchemaFieldRequiredness { schema, field, .. } => {
                    index
                        .requiredness_fields
                        .insert((schema.clone(), field.clone()));
                }
                SemanticCommand::SetFieldValue { field, .. }
                | SemanticCommand::UnsetField { field } => {
                    index
                        .field_families
                        .entry(field.clone())
                        .or_default()
                        .insert(OperationFamily::SetFieldValue);
                }
                SemanticCommand::FormulaUpdate(command) => {
                    index
                        .field_families
                        .entry(command.target().clone())
                        .or_default()
                        .insert(OperationFamily::FormulaUpdate);
                }
                SemanticCommand::FormulaRestore(command) => {
                    index
                        .field_families
                        .entry(command.target().clone())
                        .or_default()
                        .insert(OperationFamily::FormulaUpdate);
                }
                SemanticCommand::FormulaInverseRestore(command) => {
                    index
                        .field_families
                        .entry(command.target().clone())
                        .or_default()
                        .insert(OperationFamily::FormulaInverseRestore);
                }
                SemanticCommand::SetFieldConstraint { .. }
                | SemanticCommand::SetSchemaFieldDisplayName { .. }
                | SemanticCommand::SetFieldNumberFormat { .. }
                | SemanticCommand::SetSchemaFieldOrder { .. }
                | SemanticCommand::UpsertKeyedGroupedSumDefinition { .. }
                | SemanticCommand::RemoveKeyedGroupedSumDefinition { .. } => {}
            }
        }
        index
    }

    fn families(&self, field: &FieldRef) -> BTreeSet<OperationFamily> {
        let mut families = self.field_families.get(field).cloned().unwrap_or_default();
        if self.appended_entities.contains(&field.entity) {
            families.insert(OperationFamily::AppendEntity);
        }
        if self.removed_entities.contains(&field.entity) {
            families.insert(OperationFamily::RemoveEntity);
        }
        if self.schema_field_values.contains(field) {
            families.insert(OperationFamily::SchemaFieldMutation);
        }
        families
    }
}

/// One derivation's shared work: the patch-body index, each column's member
/// slots, each walked closure, and the accumulated disclosures.
///
/// `DisclosureRequirement` insertion is idempotent, so every set below only
/// removes repeated work; it never changes the resulting requirement set.
struct DisclosureDerivation<'a> {
    lifecycle: &'a PatchLifecycle,
    before: &'a Document,
    after: &'a Document,
    index: PatchBodyIndex,
    /// Each column's member slots per admitted document, built on first use.
    column_slots: BTreeMap<(DisclosureDocumentSide, ColumnRef), Vec<FieldRef>>,
    /// Columns whose slots were already disclosed for one family and document.
    inserted_columns: BTreeSet<(OperationFamily, DisclosureDocumentSide, ColumnRef)>,
    /// Columns whose whole closure was already walked for one family and document.
    walked_columns: BTreeSet<(OperationFamily, DisclosureDocumentSide, ColumnRef)>,
    /// Fields whose closure was already inserted for one family and document.
    visited: BTreeSet<(OperationFamily, DisclosureDocumentSide, FieldRef)>,
    disclosures: BTreeSet<DisclosureRequirement>,
}

/// Test-only count of column-slot walks, so a work bound can be asserted
/// without timing. One walk enumerates one column's member slots once.
#[cfg(test)]
mod disclosure_slot_walk_count {
    use std::cell::Cell;

    thread_local! {
        static WALKS: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn reset() {
        WALKS.with(|walks| walks.set(0));
    }

    pub(super) fn record() {
        WALKS.with(|walks| walks.set(walks.get() + 1));
    }

    pub(super) fn count() -> usize {
        WALKS.with(Cell::get)
    }
}

impl<'a> DisclosureDerivation<'a> {
    fn new(
        lifecycle: &'a PatchLifecycle,
        before: &'a Document,
        after: &'a Document,
        body: &SemanticPatchBody,
    ) -> Self {
        Self {
            lifecycle,
            before,
            after,
            index: PatchBodyIndex::new(body),
            column_slots: BTreeMap::new(),
            inserted_columns: BTreeSet::new(),
            walked_columns: BTreeSet::new(),
            visited: BTreeSet::new(),
            disclosures: BTreeSet::new(),
        }
    }

    #[allow(clippy::too_many_lines)] // Closed command catalogue derives disclosures atomically.
    fn insert_command_disclosures(
        &mut self,
        command: &SemanticCommand,
    ) -> Result<(), PatchLifecycleError> {
        match command {
            SemanticCommand::AppendCollection { .. } | SemanticCommand::AppendEntity { .. } => {
                self.insert_document_disclosure(OperationFamily::AppendEntity);
                Ok(())
            }
            SemanticCommand::RemoveCollection { .. } | SemanticCommand::RemoveEntity { .. } => {
                self.insert_document_disclosure(OperationFamily::RemoveEntity);
                Ok(())
            }
            SemanticCommand::AppendSchemaField { .. }
            | SemanticCommand::RemoveSchemaField { .. }
            | SemanticCommand::RenameSchemaField { .. }
            | SemanticCommand::SetSchemaFieldDisplayName { .. }
            | SemanticCommand::SetFieldNumberFormat { .. }
            | SemanticCommand::SetSchemaFieldOrder { .. }
            | SemanticCommand::SetSchemaFieldRequiredness { .. } => {
                self.insert_document_disclosure(OperationFamily::SchemaFieldMutation);
                Ok(())
            }
            SemanticCommand::SetFieldConstraint { schema, field, .. } => {
                self.disclosures.insert(DisclosureRequirement {
                    family: OperationFamily::SchemaFieldMutation,
                    scope: self.lifecycle.schema_field_scope(schema, field),
                });
                for side in [
                    DisclosureDocumentSide::Before,
                    DisclosureDocumentSide::After,
                ] {
                    let document = side.document(self.before, self.after);
                    for entity in document
                        .entities
                        .values()
                        .filter(|entity| entity.schema == *schema)
                    {
                        let Some(value) = entity.fields.get(field) else {
                            continue;
                        };
                        let owner = FieldRef::new(entity.id.clone(), field.clone());
                        self.insert_value_disclosures_for(
                            OperationFamily::SchemaFieldMutation,
                            &owner,
                            value,
                        )?;
                        if let Value::Formula(expression) = value {
                            self.insert_calculation_dependency_disclosures(
                                OperationFamily::SchemaFieldMutation,
                                side,
                                &owner,
                                expression,
                            )?;
                        }
                    }
                }
                Ok(())
            }
            SemanticCommand::SetFieldValue { field, value } => {
                self.insert_field_disclosure(field)?;
                self.insert_value_disclosures(field, value)
            }
            SemanticCommand::UnsetField { field } => {
                self.insert_field_disclosure_for(OperationFamily::SetFieldValue, field)?;
                let before = self.before;
                let Some(value) = before
                    .entities
                    .get(&field.entity)
                    .and_then(|entity| entity.fields.get(&field.field))
                else {
                    return Ok(());
                };
                self.insert_value_disclosures_for(OperationFamily::SetFieldValue, field, value)
            }
            SemanticCommand::FormulaUpdate(command) => {
                self.insert_field_disclosure_for(OperationFamily::FormulaUpdate, command.target())?;
                for reference in command.references() {
                    self.insert_field_disclosure_for(OperationFamily::FormulaUpdate, reference)?;
                }
                Ok(())
            }
            SemanticCommand::FormulaRestore(command) => {
                self.insert_field_disclosure_for(OperationFamily::FormulaUpdate, command.target())?;
                // A column total reads every entity of its schema, so the
                // reference set expands against the admitted document. The
                // command is built from host history where no document is at
                // hand (the capacity edit path keeps none), so the expansion
                // happens here, once both documents exist.
                for (reference, _) in command.expression().reference_uses(command.target()) {
                    self.insert_field_disclosure_for(OperationFamily::FormulaUpdate, &reference)?;
                }
                for column in command.expression().column_totals() {
                    self.insert_column_disclosures(
                        OperationFamily::FormulaUpdate,
                        DisclosureDocumentSide::After,
                        column,
                    )?;
                }
                Ok(())
            }
            SemanticCommand::FormulaInverseRestore(command) => {
                self.insert_field_disclosure_for(
                    OperationFamily::FormulaInverseRestore,
                    command.target(),
                )?;
                self.insert_value_disclosures_for(
                    OperationFamily::FormulaInverseRestore,
                    command.target(),
                    &Value::Number(command.value()),
                )
            }
            SemanticCommand::UpsertKeyedGroupedSumDefinition { .. }
            | SemanticCommand::RemoveKeyedGroupedSumDefinition { .. } => {
                self.insert_document_disclosure(OperationFamily::KeyedGroupedSumDefinition);
                Ok(())
            }
        }
    }

    fn insert_document_disclosure(&mut self, family: OperationFamily) {
        self.disclosures.insert(DisclosureRequirement {
            family,
            scope: ScopedSemanticSubject::new(
                self.lifecycle.document_scope.clone(),
                self.lifecycle.document.clone(),
                SemanticScope::Document,
            ),
        });
    }

    #[allow(clippy::too_many_lines)] // Closed semantic-change catalogue derives disclosures atomically.
    fn insert_change_disclosures(
        &mut self,
        change: &SemanticChange,
    ) -> Result<(), PatchLifecycleError> {
        match change {
            SemanticChange::SchemaAdded { .. }
            | SemanticChange::SchemaRemoved { .. }
            | SemanticChange::SchemaFieldAdded { .. }
            | SemanticChange::SchemaFieldRemoved { .. }
            | SemanticChange::EntityAdded { .. }
            | SemanticChange::EntityRemoved { .. } => Ok(()),
            SemanticChange::SchemaFieldChanged { schema, field, .. }
                if self
                    .index
                    .requiredness_fields
                    .contains(&(schema.clone(), field.clone())) =>
            {
                self.insert_document_disclosure(OperationFamily::SchemaFieldMutation);
                Ok(())
            }
            // Removing a field and appending it under the same id in one batch
            // replaces its definition, which nets out to one changed field.
            SemanticChange::SchemaFieldChanged { schema, field, .. }
                if self
                    .index
                    .removed_schema_fields
                    .contains(&(schema.clone(), field.clone()))
                    && self
                        .index
                        .appended_schema_fields
                        .contains(&(schema.clone(), field.clone())) =>
            {
                self.insert_document_disclosure(OperationFamily::SchemaFieldMutation);
                Ok(())
            }
            SemanticChange::FieldKeyChanged { schema, field, .. }
                if self
                    .index
                    .renamed_schema_fields
                    .contains(&(schema.clone(), field.clone())) =>
            {
                self.insert_document_disclosure(OperationFamily::SchemaFieldMutation);
                Ok(())
            }
            SemanticChange::FieldAdded { field, value } => {
                let families = self.index.families(field);
                if families == BTreeSet::from([OperationFamily::SchemaFieldMutation]) {
                    self.insert_document_disclosure(OperationFamily::SchemaFieldMutation);
                    return Ok(());
                }
                // A value-slot initialization is not a schema/identity mutation.
                // Admit only the existing optional declaration and its exact
                // SetFieldValue command; arbitrary added fields remain closed.
                // The formula exceptions are an authored formula update and
                // the app-private restore of a formula the host cleared; both
                // can only target an optional Number slot through their own
                // commands.
                let definition = crate::field_definition(self.before, field)
                    .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?;
                let existing = self.before.entities[&field.entity].fields.get(&field.field);
                let formula_restore = definition.field_type == FieldType::Number
                    && !definition.required
                    && matches!(value, Value::Formula(_))
                    && families.contains(&OperationFamily::FormulaUpdate);
                if existing.is_some()
                    || definition.required
                    || (matches!(value, Value::Formula(_)) && !formula_restore)
                    || (!formula_restore
                        && families != BTreeSet::from([OperationFamily::SetFieldValue]))
                {
                    return Err(PatchLifecycleError::ScopeDerivationFailed);
                }
                // A formula into a blank slot discloses under its own family,
                // as it does over a present value.
                let family = if formula_restore {
                    OperationFamily::FormulaUpdate
                } else {
                    OperationFamily::SetFieldValue
                };
                self.insert_field_disclosure_for(family, field)?;
                self.insert_value_disclosures_for(family, field, value)
            }
            SemanticChange::FieldRemoved { field, value } => {
                let mut families = self.index.families(field);
                if families == BTreeSet::from([OperationFamily::SchemaFieldMutation]) {
                    self.insert_document_disclosure(OperationFamily::SchemaFieldMutation);
                    return Ok(());
                }
                if families.is_empty() {
                    families.insert(OperationFamily::SetFieldValue);
                }
                for family in families {
                    self.insert_field_disclosure_for(family, field)?;
                    self.insert_value_disclosures_for(family, field, value)?;
                }
                Ok(())
            }
            SemanticChange::FieldChanged {
                field,
                before: old_value,
                after: new_value,
            } => {
                let mut families = self.index.families(field);
                if families.is_empty() {
                    families.insert(OperationFamily::SetFieldValue);
                }
                for family in families {
                    self.insert_field_disclosure_for(family, field)?;
                    self.insert_value_disclosures_for(family, field, old_value)?;
                    self.insert_value_disclosures_for(family, field, new_value)?;
                }
                Ok(())
            }
            SemanticChange::FormulaImpact { field, causes, .. } => {
                let mut family_causes = BTreeMap::<OperationFamily, Vec<&FieldRef>>::new();
                for cause in causes {
                    let mut families = self.index.families(cause);
                    if families.is_empty() {
                        families.insert(OperationFamily::SetFieldValue);
                    }
                    for family in families {
                        family_causes.entry(family).or_default().push(cause);
                    }
                }
                for (family, causes) in family_causes {
                    self.insert_field_disclosure_for(family, field)?;
                    for cause in causes {
                        self.insert_field_disclosure_for(family, cause)?;
                    }
                    for side in [
                        DisclosureDocumentSide::Before,
                        DisclosureDocumentSide::After,
                    ] {
                        let expression = side
                            .document(self.before, self.after)
                            .entities
                            .get(&field.entity)
                            .and_then(|entity| entity.fields.get(&field.field))
                            .and_then(|value| match value {
                                Value::Formula(expression) => Some(expression.clone()),
                                _ => None,
                            });
                        let Some(expression) = expression else {
                            continue;
                        };
                        self.insert_calculation_dependency_disclosures(
                            family,
                            side,
                            field,
                            &expression,
                        )?;
                    }
                }
                Ok(())
            }
            _ => Err(PatchLifecycleError::ScopeDerivationFailed),
        }
    }

    /// Insert one field's disclosure, resolving its scope against the candidate
    /// first and the base second.
    fn insert_field_disclosure_for(
        &mut self,
        family: OperationFamily,
        field: &FieldRef,
    ) -> Result<(), PatchLifecycleError> {
        self.insert_field_disclosure_in(family, FieldScopeSource::AfterThenBefore, field)
    }

    fn insert_field_disclosure(&mut self, field: &FieldRef) -> Result<(), PatchLifecycleError> {
        self.insert_field_disclosure_for(OperationFamily::SetFieldValue, field)
    }

    fn insert_field_disclosure_in(
        &mut self,
        family: OperationFamily,
        source: FieldScopeSource,
        field: &FieldRef,
    ) -> Result<(), PatchLifecycleError> {
        let scope = match source {
            FieldScopeSource::AfterThenBefore => self
                .lifecycle
                .field_scope(self.after, field)
                .or_else(|_| self.lifecycle.field_scope(self.before, field)),
            FieldScopeSource::Side(side) => self
                .lifecycle
                .field_scope(side.document(self.before, self.after), field),
        }
        .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?;
        self.disclosures
            .insert(DisclosureRequirement { family, scope });
        Ok(())
    }

    fn insert_value_disclosures(
        &mut self,
        owner: &FieldRef,
        value: &Value,
    ) -> Result<(), PatchLifecycleError> {
        self.insert_value_disclosures_for(OperationFamily::SetFieldValue, owner, value)
    }

    fn insert_value_disclosures_for(
        &mut self,
        family: OperationFamily,
        owner: &FieldRef,
        value: &Value,
    ) -> Result<(), PatchLifecycleError> {
        match value {
            Value::Reference(entity) => {
                let scope = self
                    .lifecycle
                    .entity_scope(self.after, entity)
                    .or_else(|_| self.lifecycle.entity_scope(self.before, entity))?;
                self.disclosures
                    .insert(DisclosureRequirement { family, scope });
            }
            Value::Formula(expression) => {
                for (reference, _) in expression.reference_uses(owner) {
                    self.insert_field_disclosure_in(
                        family,
                        FieldScopeSource::AfterThenBefore,
                        &reference,
                    )?;
                }
                for column in expression.column_totals() {
                    self.insert_column_disclosures(family, DisclosureDocumentSide::After, column)?;
                }
            }
            Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Date(_) => {}
        }
        Ok(())
    }

    /// Insert one column's member slots once per family and document.
    fn insert_column_disclosures(
        &mut self,
        family: OperationFamily,
        side: DisclosureDocumentSide,
        column: &ColumnRef,
    ) -> Result<(), PatchLifecycleError> {
        if !self.inserted_columns.insert((family, side, column.clone())) {
            return Ok(());
        }
        let slots = self.column_slots(side, column).to_vec();
        #[cfg(test)]
        {
            disclosure_slot_walk_count::record();
        }
        for field in &slots {
            self.insert_field_disclosure_in(family, FieldScopeSource::AfterThenBefore, field)?;
        }
        Ok(())
    }

    fn column_slots(&mut self, side: DisclosureDocumentSide, column: &ColumnRef) -> &[FieldRef] {
        let document = side.document(self.before, self.after);
        self.column_slots
            .entry((side, column.clone()))
            .or_insert_with(|| {
                document
                    .entities
                    .iter()
                    .filter(|(_, entity)| entity.schema == column.schema)
                    .map(|(entity_id, _)| FieldRef::new(entity_id.clone(), column.field.clone()))
                    .collect()
            })
    }

    /// Add one column's member slots to a calculation walk once per family and
    /// document. A later walk of the same column has already inserted the whole
    /// closure of every slot.
    fn push_column_closure(
        &mut self,
        family: OperationFamily,
        side: DisclosureDocumentSide,
        column: &ColumnRef,
        pending: &mut Vec<FieldRef>,
    ) {
        if !self.walked_columns.insert((family, side, column.clone())) {
            return;
        }
        let slots = self.column_slots(side, column).to_vec();
        #[cfg(test)]
        {
            disclosure_slot_walk_count::record();
        }
        pending.extend(slots);
    }

    fn insert_calculation_dependency_disclosures(
        &mut self,
        family: OperationFamily,
        side: DisclosureDocumentSide,
        owner: &FieldRef,
        expression: &Expression,
    ) -> Result<(), PatchLifecycleError> {
        let mut pending = expression
            .reference_uses(owner)
            .into_iter()
            .map(|(reference, _)| reference)
            .collect::<Vec<_>>();
        for column in expression.column_totals() {
            self.push_column_closure(family, side, column, &mut pending);
        }
        while let Some(field) = pending.pop() {
            if !self.visited.insert((family, side, field.clone())) {
                continue;
            }
            self.insert_field_disclosure_in(family, FieldScopeSource::Side(side), &field)?;
            let document = side.document(self.before, self.after);
            let entity = document
                .entities
                .get(&field.entity)
                .ok_or(PatchLifecycleError::ScopeDerivationFailed)?;
            let Some(value) = entity.fields.get(&field.field) else {
                // A blank slot of a declared field (a column total reads every
                // row, blanks included) is disclosed but has nothing further
                // to traverse.
                let declared = document
                    .schemas
                    .get(&entity.schema)
                    .is_some_and(|schema| schema.fields.contains_key(&field.field));
                if declared {
                    continue;
                }
                return Err(PatchLifecycleError::ScopeDerivationFailed);
            };
            if let Value::Formula(expression) = value {
                pending.extend(
                    expression
                        .reference_uses(&field)
                        .into_iter()
                        .map(|(reference, _)| reference),
                );
                for column in expression.column_totals() {
                    self.push_column_closure(family, side, column, &mut pending);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
fn command_families_for_field(
    body: &SemanticPatchBody,
    field: &FieldRef,
) -> BTreeSet<OperationFamily> {
    body.commands()
        .iter()
        .filter_map(|command| match command {
            SemanticCommand::AppendCollection { entities, .. }
                if entities.iter().any(|entity| entity.id == field.entity) =>
            {
                Some(OperationFamily::AppendEntity)
            }
            SemanticCommand::RemoveCollection { entities, .. }
                if entities.iter().any(|entity| entity == &field.entity) =>
            {
                Some(OperationFamily::RemoveEntity)
            }
            SemanticCommand::AppendEntity { entity } if entity.id == field.entity => {
                Some(OperationFamily::AppendEntity)
            }
            SemanticCommand::RemoveEntity { entity } if entity == &field.entity => {
                Some(OperationFamily::RemoveEntity)
            }
            SemanticCommand::FormulaUpdate(command) if command.target() == field => {
                Some(OperationFamily::FormulaUpdate)
            }
            SemanticCommand::FormulaRestore(command) if command.target() == field => {
                Some(OperationFamily::FormulaUpdate)
            }
            SemanticCommand::FormulaInverseRestore(command) if command.target() == field => {
                Some(OperationFamily::FormulaInverseRestore)
            }
            SemanticCommand::SetFieldValue { field: target, .. } if target == field => {
                Some(OperationFamily::SetFieldValue)
            }
            SemanticCommand::UnsetField { field: target } if target == field => {
                Some(OperationFamily::SetFieldValue)
            }
            SemanticCommand::AppendSchemaField {
                field: definition,
                values,
                ..
            } if definition.id == field.field && values.contains_key(&field.entity) => {
                Some(OperationFamily::SchemaFieldMutation)
            }
            SemanticCommand::RemoveSchemaField {
                field: definition,
                values,
                ..
            } if definition.id == field.field && values.contains_key(&field.entity) => {
                Some(OperationFamily::SchemaFieldMutation)
            }
            _ => None,
        })
        .collect()
}

/// Structural authoring admission only; no identity-prefix or import provenance inference.
fn has_native_scalar_shape(document: &Document, schema: &super::Schema) -> bool {
    !schema.fields.is_empty()
        && schema.fields.values().all(|field| {
            matches!(
                field.field_type,
                FieldType::Text | FieldType::Number | FieldType::Boolean | FieldType::Date
            )
        })
        && document
            .entities
            .values()
            .filter(|entity| entity.schema == schema.id)
            .all(|entity| {
                entity
                    .fields
                    .keys()
                    .all(|id| schema.fields.contains_key(id))
                    && schema.fields.iter().all(|(id, field)| {
                        entity.fields.get(id).map_or(!field.required, |value| {
                            matches!(
                                (&field.field_type, value),
                                (FieldType::Text, Value::Text(_))
                                    | (FieldType::Number, Value::Number(_) | Value::Formula(_))
                                    | (FieldType::Boolean, Value::Boolean(_))
                                    | (FieldType::Date, Value::Date(_))
                            )
                        })
                    })
            })
}

fn classify_field_transition(
    existing: Option<&Value>,
    replacement: &Value,
) -> BTreeSet<MutationClass> {
    if matches!(replacement, Value::Formula(_)) {
        BTreeSet::from([MutationClass::Formula])
    } else if matches!(existing, Some(Value::Formula(_))) {
        BTreeSet::from([
            MutationClass::Value,
            MutationClass::Formula,
            MutationClass::Destructive,
        ])
    } else {
        BTreeSet::from([MutationClass::Value])
    }
}

fn formula_impacts(changes: &[SemanticChange]) -> Vec<FormulaImpactEvidence> {
    changes
        .iter()
        .filter_map(|change| match change {
            SemanticChange::FormulaImpact {
                field,
                before,
                after,
                causes,
            } => Some(FormulaImpactEvidence {
                field: field.clone(),
                before: *before,
                after: *after,
                causes: causes.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn duplicate_constraint_write(body: &SemanticPatchBody) -> Option<(SchemaId, FieldId)> {
    let mut constraint_targets = BTreeSet::new();
    let mut field_lifecycle_targets = BTreeSet::new();
    let mut collection_lifecycle_targets = BTreeSet::new();
    for command in body.commands() {
        match command {
            SemanticCommand::SetFieldConstraint { schema, field, .. } => {
                let target = (schema.clone(), field.clone());
                if !constraint_targets.insert(target.clone())
                    || field_lifecycle_targets.contains(&target)
                    || collection_lifecycle_targets.contains(&(schema.clone(), None))
                    || collection_lifecycle_targets.contains(&(schema.clone(), Some(field.clone())))
                {
                    return Some(target);
                }
            }
            SemanticCommand::AppendSchemaField { schema, field, .. }
            | SemanticCommand::RemoveSchemaField { schema, field, .. } => {
                let target = (schema.clone(), field.id.clone());
                if constraint_targets.contains(&target) {
                    return Some(target);
                }
                field_lifecycle_targets.insert(target);
            }
            SemanticCommand::AppendCollection { schema, .. } => {
                for field in schema
                    .fields
                    .values()
                    .map(|definition| definition.id.clone())
                {
                    let target = (schema.id.clone(), field);
                    if constraint_targets.contains(&target) {
                        return Some(target);
                    }
                    collection_lifecycle_targets.insert((target.0.clone(), Some(target.1.clone())));
                }
            }
            SemanticCommand::RemoveCollection { schema, .. } => {
                if let Some(target) = constraint_targets
                    .iter()
                    .find(|(constraint_schema, _)| constraint_schema == schema)
                {
                    return Some(target.clone());
                }
                collection_lifecycle_targets.insert((schema.clone(), None));
            }
            _ => {}
        }
    }
    None
}

fn reject_legacy_field_orders(
    base: &Document,
    candidate: &Document,
    body: &SemanticPatchBody,
) -> Result<(), PatchLifecycleError> {
    if base.has_field_orders()
        || candidate.has_field_orders()
        || body
            .commands()
            .iter()
            .any(|command| matches!(command, SemanticCommand::SetSchemaFieldOrder { .. }))
    {
        return Err(PatchLifecycleError::CommandRejected {
            source: Box::new(WorkspaceError::Diff(
                super::DiffError::UnsupportedFieldOrder,
            )),
        });
    }
    Ok(())
}

fn reject_legacy_number_formats(
    base: &Document,
    candidate: &Document,
    body: &SemanticPatchBody,
) -> Result<(), PatchLifecycleError> {
    if base.has_number_formats()
        || candidate.has_number_formats()
        || body
            .commands()
            .iter()
            .any(|command| matches!(command, SemanticCommand::SetFieldNumberFormat { .. }))
    {
        return Err(PatchLifecycleError::CommandRejected {
            source: Box::new(WorkspaceError::Diff(
                super::DiffError::UnsupportedNumberFormat,
            )),
        });
    }
    Ok(())
}

fn contains_constraint_command(body: &SemanticPatchBody) -> bool {
    body.commands()
        .iter()
        .any(|command| matches!(command, SemanticCommand::SetFieldConstraint { .. }))
}

fn document_has_field_constraints(document: &Document) -> bool {
    document.schemas.values().any(|schema| {
        schema
            .fields
            .values()
            .any(|field| field.constraint != FieldConstraint::None)
    })
}

fn constraint_review_evidence(
    before: &Document,
    after: &Document,
    body: &SemanticPatchBody,
) -> Result<Vec<ConstraintReviewEvidence>, PatchLifecycleError> {
    let targets = body
        .commands()
        .iter()
        .filter_map(|command| match command {
            SemanticCommand::SetFieldConstraint { schema, field, .. } => {
                Some((schema.clone(), field.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let before_calculations = calculation_values(before)?;
    let after_calculations = calculation_values(after)?;
    let mut reviews = targets
        .into_iter()
        .map(|(schema, field)| {
            Ok(ConstraintReviewEvidence {
                schema: schema.clone(),
                field: field.clone(),
                before: constraint_affected_values(before, &schema, &field, &before_calculations),
                after: constraint_affected_values(after, &schema, &field, &after_calculations),
            })
        })
        .collect::<Result<Vec<_>, PatchLifecycleError>>()?;
    reviews.sort_by(|left, right| (&left.schema, &left.field).cmp(&(&right.schema, &right.field)));
    Ok(reviews)
}

fn calculation_values(
    document: &Document,
) -> Result<BTreeMap<FieldRef, Number>, PatchLifecycleError> {
    super::calculate_fields(document)
        .map(|calculated| {
            calculated
                .into_iter()
                .map(|value| (value.field, value.value))
                .collect()
        })
        .map_err(|source| PatchLifecycleError::CommandRejected {
            source: Box::new(source),
        })
}

fn constraint_affected_values(
    document: &Document,
    schema: &SchemaId,
    field: &FieldId,
    calculations: &BTreeMap<FieldRef, Number>,
) -> Vec<ConstraintAffectedValue> {
    document
        .entities
        .values()
        .filter(|entity| entity.schema == *schema)
        .filter_map(|entity| {
            let stored = entity.fields.get(field)?.clone();
            let field = FieldRef::new(entity.id.clone(), field.clone());
            Some(ConstraintAffectedValue {
                calculated_number: calculations.get(&field).copied(),
                field,
                stored,
            })
        })
        .collect()
}

// Used only for derived formula impacts after complete v2 admission, never direct evidence.
fn formula_projection_document(document: &Document) -> Document {
    let mut document = document.clone();
    document.keyed_grouped_sum_definitions.clear();
    for schema in document.schemas.values_mut() {
        schema.display_name = None;
        schema.field_order = None;
        for field in schema.fields.values_mut() {
            field.constraint = FieldConstraint::None;
            field.number_format = None;
            field.display_name = None;
        }
    }
    document
}

// Fail closed when a stable binding cannot be resolved to a current schema
// field; no partial footprint is derived from an unresolvable definition.
fn validate_keyed_grouped_sum_bindings(
    document: &Document,
    definition: &KeyedGroupedSumDefinition,
) -> Result<(), PatchLifecycleError> {
    let orders_schema = document
        .schemas
        .get(&definition.orders.schema)
        .ok_or(PatchLifecycleError::ScopeDerivationFailed)?;
    let products_schema = document
        .schemas
        .get(&definition.products.schema)
        .ok_or(PatchLifecycleError::ScopeDerivationFailed)?;
    for field in [
        &definition.orders.lookup_key_field,
        &definition.orders.quantity_field,
    ] {
        if !orders_schema.fields.contains_key(field) {
            return Err(PatchLifecycleError::ScopeDerivationFailed);
        }
    }
    for field in [
        &definition.products.key_field,
        &definition.products.category_field,
        &definition.products.price_field,
    ] {
        if !products_schema.fields.contains_key(field) {
            return Err(PatchLifecycleError::ScopeDerivationFailed);
        }
    }
    Ok(())
}

// Type facts that are disclosed for every Query regardless of matches.
fn keyed_grouped_sum_type_facts(definition: &KeyedGroupedSumDefinition) -> [SemanticScope; 3] {
    [
        SemanticScope::SchemaField {
            schema: definition.orders.schema.clone(),
            field: definition.orders.lookup_key_field.clone(),
        },
        SemanticScope::SchemaField {
            schema: definition.orders.schema.clone(),
            field: definition.orders.quantity_field.clone(),
        },
        SemanticScope::SchemaField {
            schema: definition.products.schema.clone(),
            field: definition.products.key_field.clone(),
        },
    ]
}

fn keyed_grouped_sum_schema_entities<'a>(
    document: &'a Document,
    schema: &SchemaId,
) -> Vec<&'a Entity> {
    document
        .entities
        .values()
        .filter(|entity| &entity.schema == schema)
        .collect()
}

// Determines which Products rows a definition exposes, mirroring the
// evaluator's exact Text match: one candidate is a successful match, zero or
// many are not. The boolean reports whether a successfully matched required
// operand is formula-backed, which pulls in the whole Calculation footprint.
fn keyed_grouped_sum_matched_products(
    document: &Document,
    definition: &KeyedGroupedSumDefinition,
) -> (BTreeSet<EntityId>, bool) {
    let mut product_index: BTreeMap<&str, Vec<&Entity>> = BTreeMap::new();
    for product in document
        .entities
        .values()
        .filter(|entity| entity.schema == definition.products.schema)
    {
        if let Some(Value::Text(key)) = product.fields.get(&definition.products.key_field) {
            product_index.entry(key.as_str()).or_default().push(product);
        }
    }
    let mut matched = BTreeSet::new();
    let mut formula_operand_required = false;
    for order in document
        .entities
        .values()
        .filter(|entity| entity.schema == definition.orders.schema)
    {
        let Some(Value::Text(key)) = order.fields.get(&definition.orders.lookup_key_field) else {
            continue;
        };
        let Some(candidates) = product_index.get(key.as_str()) else {
            continue;
        };
        if candidates.len() != 1 {
            continue;
        }
        let product = candidates[0];
        matched.insert(product.id.clone());
        formula_operand_required |= matches!(
            order.fields.get(&definition.orders.quantity_field),
            Some(Value::Formula(_))
        );
        formula_operand_required |= matches!(
            product.fields.get(&definition.products.price_field),
            Some(Value::Formula(_))
        );
    }
    (matched, formula_operand_required)
}

fn expression_names_column(expression: &Expression, schema: &SchemaId, field: &FieldId) -> bool {
    expression
        .column_totals()
        .into_iter()
        .any(|column| &column.schema == schema && &column.field == field)
}

fn expression_references(
    document: &Document,
    owner: &FieldRef,
    expression: &Expression,
) -> BTreeSet<FieldRef> {
    let mut references = expression
        .reference_uses(owner)
        .into_iter()
        .map(|(reference, _)| reference)
        .collect::<BTreeSet<_>>();
    for column in expression.column_totals() {
        for (entity_id, entity) in &document.entities {
            if entity.schema == column.schema {
                references.insert(FieldRef::new(entity_id.clone(), column.field.clone()));
            }
        }
    }
    references
}

fn scope_covers(granted: &ScopedSemanticSubject, required: &ScopedSemanticSubject) -> bool {
    if granted.document_scope != required.document_scope || granted.document != required.document {
        return false;
    }
    match (&granted.subject, &required.subject) {
        (SemanticScope::Document, _) => true,
        (
            SemanticScope::Schema(granted),
            SemanticScope::Schema(required)
            | SemanticScope::SchemaField {
                schema: required, ..
            }
            | SemanticScope::Entity {
                schema: required, ..
            }
            | SemanticScope::EntityField {
                schema: required, ..
            },
        ) => granted == required,
        (
            SemanticScope::SchemaField {
                schema: granted_schema,
                field: granted_field,
            },
            SemanticScope::SchemaField {
                schema: required_schema,
                field: required_field,
            }
            | SemanticScope::EntityField {
                schema: required_schema,
                field: required_field,
                ..
            },
        ) => granted_schema == required_schema && granted_field == required_field,
        (
            SemanticScope::Entity {
                entity: granted_entity,
                schema: granted_schema,
            },
            SemanticScope::Entity {
                entity: required_entity,
                schema: required_schema,
            }
            | SemanticScope::EntityField {
                entity: required_entity,
                schema: required_schema,
                ..
            },
        ) => granted_entity == required_entity && granted_schema == required_schema,
        (
            SemanticScope::EntityField {
                entity: granted_entity,
                schema: granted_schema,
                field: granted_field,
            },
            SemanticScope::EntityField {
                entity: required_entity,
                schema: required_schema,
                field: required_field,
            },
        ) => {
            granted_entity == required_entity
                && granted_schema == required_schema
                && granted_field == required_field
        }
        _ => false,
    }
}

fn push_once(history: &mut Vec<PatchLifecycleState>, state: PatchLifecycleState) {
    if !history.contains(&state) {
        history.push(state);
    }
}

fn proposal_is_terminal(proposal: &ProposalRecord) -> bool {
    proposal.history.iter().any(|state| {
        matches!(
            state,
            PatchLifecycleState::Verified
                | PatchLifecycleState::Rejected
                | PatchLifecycleState::ValidationFailed
                | PatchLifecycleState::Stale
                | PatchLifecycleState::Conflict
        )
    })
}

/// Admit the complete supplied request before cloning or changing a candidate.
/// Later writes/removals cannot hide an earlier spelling failure. Uniqueness
/// remains a declaration invariant of the final candidate, not this scan.
fn admit_command_names(body: &SemanticPatchBody) -> Result<(), WorkspaceError> {
    let mut failures = Vec::new();
    for (command_index, command) in body.commands().iter().enumerate() {
        let mut check = |name: Option<&str>, subject: super::SemanticSubject| {
            if let Some(name) = name {
                for failure in super::display_name_failures(name) {
                    failures.push(super::CommandNameFailure {
                        command_index,
                        subject: subject.clone(),
                        reason: failure.reason(),
                    });
                }
            }
        };
        match command {
            SemanticCommand::AppendCollection { schema, .. } => {
                check(
                    schema.display_name.as_deref(),
                    super::SemanticSubject::Schema(schema.id.clone()),
                );
                for field in schema.fields.values() {
                    check(
                        field.display_name.as_deref(),
                        super::SemanticSubject::SchemaField {
                            schema: schema.id.clone(),
                            field: field.id.clone(),
                        },
                    );
                }
            }
            SemanticCommand::AppendSchemaField { schema, field, .. }
            | SemanticCommand::RemoveSchemaField { schema, field, .. } => {
                check(
                    field.display_name.as_deref(),
                    super::SemanticSubject::SchemaField {
                        schema: schema.clone(),
                        field: field.id.clone(),
                    },
                );
            }
            SemanticCommand::SetSchemaFieldDisplayName {
                schema,
                field,
                display_name,
            } => {
                check(
                    display_name.as_deref(),
                    super::SemanticSubject::SchemaField {
                        schema: schema.clone(),
                        field: field.clone(),
                    },
                );
            }
            SemanticCommand::RemoveCollection { .. }
            | SemanticCommand::AppendEntity { .. }
            | SemanticCommand::RemoveEntity { .. }
            | SemanticCommand::RenameSchemaField { .. }
            | SemanticCommand::SetFieldNumberFormat { .. }
            | SemanticCommand::SetSchemaFieldOrder { .. }
            | SemanticCommand::SetFieldConstraint { .. }
            | SemanticCommand::SetSchemaFieldRequiredness { .. }
            | SemanticCommand::SetFieldValue { .. }
            | SemanticCommand::UnsetField { .. }
            | SemanticCommand::FormulaUpdate(_)
            | SemanticCommand::FormulaInverseRestore(_)
            | SemanticCommand::FormulaRestore(_)
            | SemanticCommand::UpsertKeyedGroupedSumDefinition { .. }
            | SemanticCommand::RemoveKeyedGroupedSumDefinition { .. } => {}
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(WorkspaceError::NameAdmission { failures })
    }
}

#[cfg(test)]
mod tests {
    use crate::keyed_grouped_sum_operations::{
        KeyedGroupedSumGroup, KeyedGroupedSumOutcome, evaluate_keyed_grouped_sum,
    };
    use tachiko_semantic_core::{
        ComparisonOperator, EntityKey, FieldDefinition, FieldKey, FieldType,
        KeyedGroupedSumOrdersBinding, KeyedGroupedSumProductsBinding, Schema, SchemaKey,
    };

    use super::*;

    fn schema_field_command(field: &str, append: bool) -> SemanticCommand {
        let definition = FieldDefinition {
            id: FieldId::from(field),
            key: FieldKey::from(field),
            display_name: None,
            number_format: None,
            field_type: FieldType::Number,
            required: true,
            constraint: FieldConstraint::None,
        };
        if append {
            SemanticCommand::AppendSchemaField {
                schema: SchemaId::from("schema"),
                field: definition,
                values: BTreeMap::new(),
            }
        } else {
            SemanticCommand::RemoveSchemaField {
                schema: SchemaId::from("schema"),
                field: definition,
                values: BTreeMap::new(),
            }
        }
    }

    fn collection_command(schema_id: &str, append: bool) -> SemanticCommand {
        if append {
            SemanticCommand::AppendCollection {
                schema: Schema {
                    field_order: None,
                    id: SchemaId::from(schema_id),
                    key: SchemaKey::from(schema_id),
                    display_name: None,
                    fields: BTreeMap::from([(
                        FieldId::from("field"),
                        FieldDefinition {
                            id: FieldId::from("field"),
                            key: FieldKey::from("field"),
                            display_name: None,
                            number_format: None,
                            field_type: FieldType::Number,
                            required: true,
                            constraint: FieldConstraint::None,
                        },
                    )]),
                },
                entities: Vec::new(),
            }
        } else {
            SemanticCommand::RemoveCollection {
                schema: SchemaId::from(schema_id),
                entities: Vec::new(),
            }
        }
    }

    #[test]
    fn constraint_write_rejects_duplicate_and_field_lifecycle_targets() {
        let constraint = || SemanticCommand::SetFieldConstraint {
            schema: SchemaId::from("schema"),
            field: FieldId::from("field"),
            constraint: FieldConstraint::None,
        };

        for commands in [
            vec![constraint(), constraint()],
            vec![constraint(), schema_field_command("field", true)],
            vec![schema_field_command("field", false), constraint()],
            vec![collection_command("schema", true), constraint()],
            vec![constraint(), collection_command("schema", true)],
            vec![collection_command("schema", false), constraint()],
            vec![constraint(), collection_command("schema", false)],
        ] {
            let body = SemanticPatchBody::atomic_batch(commands).unwrap();
            assert_eq!(
                duplicate_constraint_write(&body),
                Some((SchemaId::from("schema"), FieldId::from("field")))
            );
        }

        let unrelated = SemanticPatchBody::atomic_batch(vec![
            constraint(),
            schema_field_command("other", true),
        ])
        .unwrap();
        assert_eq!(duplicate_constraint_write(&unrelated), None);

        let unrelated_collection =
            SemanticPatchBody::atomic_batch(vec![constraint(), collection_command("other", true)])
                .unwrap();
        assert_eq!(duplicate_constraint_write(&unrelated_collection), None);

        let unrelated_removal =
            SemanticPatchBody::atomic_batch(vec![constraint(), collection_command("other", false)])
                .unwrap();
        assert_eq!(duplicate_constraint_write(&unrelated_removal), None);
    }

    #[test]
    fn missing_entity_has_a_distinct_scope_derivation_failure() {
        let lifecycle = PatchLifecycle::new(
            AuthorizationDomainId::from("domain"),
            DocumentScopeId::from("document-occurrence"),
            DocumentId::from("document"),
            SemanticApiContract::from("semantic-v1"),
            AuthorizationPolicyVersion::from("policy-v1"),
            PolicyMeaningId::from("policy-v1-meaning"),
        );
        let document = Document {
            id: DocumentId::from("document"),
            title: "Document".to_owned(),
            schemas: BTreeMap::new(),
            entities: BTreeMap::new(),
            keyed_grouped_sum_definitions: BTreeMap::new(),
        };

        let error = lifecycle
            .entity_scope(&document, &EntityId::from("missing"))
            .unwrap_err();

        assert!(matches!(error, PatchLifecycleError::ScopeDerivationFailed));
    }

    // The complete trusted-footprint fixture is intentionally literal so each
    // definition binding and disclosed entity field remains reviewable here.
    #[allow(clippy::too_many_lines)]
    fn keyed_grouped_sum_document() -> Document {
        let mut document = Document::empty("document", "Grouped summary");
        let orders = SchemaId::from("orders");
        let products = SchemaId::from("products");
        document.schemas.insert(
            orders.clone(),
            Schema {
                field_order: None,
                id: orders.clone(),
                key: SchemaKey::from("orders"),
                display_name: None,
                fields: BTreeMap::from([
                    (
                        FieldId::from("order-key"),
                        FieldDefinition {
                            id: FieldId::from("order-key"),
                            key: FieldKey::from("product_key"),
                            display_name: None,
                            number_format: None,
                            field_type: FieldType::Text,
                            required: true,
                            constraint: tachiko_semantic_core::FieldConstraint::None,
                        },
                    ),
                    (
                        FieldId::from("quantity"),
                        FieldDefinition {
                            id: FieldId::from("quantity"),
                            key: FieldKey::from("quantity"),
                            display_name: None,
                            number_format: None,
                            field_type: FieldType::Number,
                            required: true,
                            constraint: tachiko_semantic_core::FieldConstraint::None,
                        },
                    ),
                ]),
            },
        );
        document.schemas.insert(
            products.clone(),
            Schema {
                field_order: None,
                id: products.clone(),
                key: SchemaKey::from("products"),
                display_name: None,
                fields: BTreeMap::from([
                    (
                        FieldId::from("product-key"),
                        FieldDefinition {
                            id: FieldId::from("product-key"),
                            key: FieldKey::from("product_key"),
                            display_name: None,
                            number_format: None,
                            field_type: FieldType::Text,
                            required: true,
                            constraint: tachiko_semantic_core::FieldConstraint::None,
                        },
                    ),
                    (
                        FieldId::from("category"),
                        FieldDefinition {
                            id: FieldId::from("category"),
                            key: FieldKey::from("category"),
                            display_name: None,
                            number_format: None,
                            field_type: FieldType::Text,
                            required: true,
                            constraint: tachiko_semantic_core::FieldConstraint::None,
                        },
                    ),
                    (
                        FieldId::from("price"),
                        FieldDefinition {
                            id: FieldId::from("price"),
                            key: FieldKey::from("price"),
                            display_name: None,
                            number_format: None,
                            field_type: FieldType::Number,
                            required: true,
                            constraint: tachiko_semantic_core::FieldConstraint::None,
                        },
                    ),
                ]),
            },
        );
        document.entities.insert(
            EntityId::from("order"),
            Entity {
                id: EntityId::from("order"),
                key: EntityKey::from("order"),
                schema: orders.clone(),
                fields: BTreeMap::from([
                    (FieldId::from("order-key"), Value::Text("P-100".to_owned())),
                    (
                        FieldId::from("quantity"),
                        Value::Number(Number::new(1.0).unwrap()),
                    ),
                ]),
            },
        );
        document.entities.insert(
            EntityId::from("product"),
            Entity {
                id: EntityId::from("product"),
                key: EntityKey::from("product"),
                schema: products.clone(),
                fields: BTreeMap::from([
                    (
                        FieldId::from("product-key"),
                        Value::Text("P-100".to_owned()),
                    ),
                    (
                        FieldId::from("category"),
                        Value::Text("hardware".to_owned()),
                    ),
                    (
                        FieldId::from("price"),
                        Value::Number(Number::new(2.0).unwrap()),
                    ),
                ]),
            },
        );
        let definition = KeyedGroupedSumDefinitionId::from("summary");
        document.keyed_grouped_sum_definitions.insert(
            definition.clone(),
            KeyedGroupedSumDefinition {
                id: definition,
                orders: KeyedGroupedSumOrdersBinding {
                    schema: orders,
                    lookup_key_field: FieldId::from("order-key"),
                    quantity_field: FieldId::from("quantity"),
                },
                products: KeyedGroupedSumProductsBinding {
                    schema: products,
                    key_field: FieldId::from("product-key"),
                    category_field: FieldId::from("category"),
                    price_field: FieldId::from("price"),
                },
            },
        );
        document
    }

    #[test]
    fn keyed_grouped_sum_query_requires_authorized_trusted_footprint() {
        let document = keyed_grouped_sum_document();
        let mut lifecycle = PatchLifecycle::new(
            AuthorizationDomainId::from("domain"),
            DocumentScopeId::from("document-occurrence"),
            document.id.clone(),
            SemanticApiContract::from("semantic-v1"),
            AuthorizationPolicyVersion::from("policy-v1"),
            PolicyMeaningId::from("policy-v1-meaning"),
        );
        let authority = PrincipalId::from("authority");
        let reader = PrincipalId::from("reader");
        lifecycle
            .register_principal(authority.clone(), PrincipalKind::Human)
            .unwrap();
        lifecycle
            .register_principal(reader.clone(), PrincipalKind::Human)
            .unwrap();

        let error = lifecycle
            .authorize_keyed_grouped_sum_query(
                &document,
                &KeyedGroupedSumDefinitionId::from("summary"),
                &reader,
                TrustedInstant::new(1),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            PatchLifecycleError::InsufficientCapability {
                action: AuthorizationAction::Query
            }
        ));

        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("complete-query"),
                authority,
                reader.clone(),
                vec![GrantRequirement::query(
                    OperationFamily::KeyedGroupedSumDefinition,
                    ScopedSemanticSubject::new(
                        DocumentScopeId::from("document-occurrence"),
                        document.id.clone(),
                        SemanticScope::Document,
                    ),
                )],
                None,
            ))
            .unwrap();
        assert_eq!(
            lifecycle
                .authorize_keyed_grouped_sum_query(
                    &document,
                    &KeyedGroupedSumDefinitionId::from("summary"),
                    &reader,
                    TrustedInstant::new(1),
                )
                .unwrap(),
            BTreeSet::from([GrantId::from("complete-query")])
        );
    }

    fn keyed_grouped_sum_lifecycle(document: &Document) -> (PatchLifecycle, PrincipalId) {
        let mut lifecycle = PatchLifecycle::new(
            AuthorizationDomainId::from("domain"),
            DocumentScopeId::from("document-occurrence"),
            document.id.clone(),
            SemanticApiContract::from("semantic-v1"),
            AuthorizationPolicyVersion::from("policy-v1"),
            PolicyMeaningId::from("policy-v1-meaning"),
        );
        lifecycle
            .register_principal(PrincipalId::from("authority"), PrincipalKind::Human)
            .unwrap();
        let editor = PrincipalId::from("editor");
        lifecycle
            .register_principal(editor.clone(), PrincipalKind::Human)
            .unwrap();
        (lifecycle, editor)
    }

    fn keyed_grouped_sum_requirement(
        action: AuthorizationAction,
        class: MutationClass,
        document: &Document,
    ) -> GrantRequirement {
        GrantRequirement::mutation(
            action,
            OperationFamily::KeyedGroupedSumDefinition,
            class,
            ScopedSemanticSubject::new(
                DocumentScopeId::from("document-occurrence"),
                document.id.clone(),
                SemanticScope::Document,
            ),
        )
        .unwrap()
    }

    #[test]
    fn changed_definitions_reject_mixed_batches_even_with_net_zero_ordinary_edits() {
        let document = keyed_grouped_sum_document();
        let (lifecycle, _) = keyed_grouped_sum_lifecycle(&document);
        let mut definition = document.keyed_grouped_sum_definitions
            [&KeyedGroupedSumDefinitionId::from("summary")]
            .clone();
        definition.id = KeyedGroupedSumDefinitionId::from("fresh");
        let field = FieldRef::new("product", "price");
        let edit = SemanticCommand::set_field_value(
            field.clone(),
            Value::Number(Number::new(3.0).unwrap()),
        );
        let restore = SemanticCommand::set_field_value(
            field.clone(),
            document.entities[&field.entity].fields[&field.field].clone(),
        );
        for command in [
            SemanticCommand::UpsertKeyedGroupedSumDefinition { definition },
            SemanticCommand::RemoveKeyedGroupedSumDefinition {
                definition: KeyedGroupedSumDefinitionId::from("summary"),
            },
        ] {
            for commands in [
                vec![command.clone(), edit.clone(), restore.clone()],
                vec![edit.clone(), restore.clone(), command.clone()],
            ] {
                let body = SemanticPatchBody::atomic_batch(commands).unwrap();
                let (candidate, writes) = lifecycle.plan_commands(&document, &body).unwrap();
                assert_eq!(candidate.entities, document.entities);
                let error = lifecycle
                    .finalize_evaluation(&document, candidate, &body, writes, None)
                    .unwrap_err();
                let PatchLifecycleError::CommandRejected { source } = error else {
                    panic!("expected command rejection, got {error:?}");
                };
                assert!(matches!(
                    source.as_ref(),
                    WorkspaceError::Diff(
                        crate::DiffError::UnsupportedKeyedGroupedSumDefinitionChange
                    )
                ));
            }
        }
    }

    #[test]
    fn keyed_grouped_sum_delete_requires_structure_and_destructive_command_authority() {
        let document = keyed_grouped_sum_document();
        let (mut lifecycle, editor) = keyed_grouped_sum_lifecycle(&document);
        let query = GrantRequirement::query(
            OperationFamily::KeyedGroupedSumDefinition,
            ScopedSemanticSubject::new(
                DocumentScopeId::from("document-occurrence"),
                document.id.clone(),
                SemanticScope::Document,
            ),
        );
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("structure-only"),
                PrincipalId::from("authority"),
                editor.clone(),
                vec![
                    query.clone(),
                    keyed_grouped_sum_requirement(
                        AuthorizationAction::Propose,
                        MutationClass::Structure,
                        &document,
                    ),
                ],
                None,
            ))
            .unwrap();
        let delete = || {
            ProposalRequest::new(
                ProposalId::from("remove-summary"),
                SemanticRevision::from("resident/0"),
                SemanticPatchBody::command(SemanticCommand::RemoveKeyedGroupedSumDefinition {
                    definition: KeyedGroupedSumDefinitionId::from("summary"),
                }),
                editor.clone(),
            )
        };
        let error = lifecycle
            .propose(
                &DocumentScopeId::from("document-occurrence"),
                &document,
                &SemanticRevision::from("resident/0"),
                delete(),
                TrustedInstant::new(1),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            PatchLifecycleError::InsufficientCapability {
                action: AuthorizationAction::Propose
            }
        ));

        let (mut lifecycle, editor) = keyed_grouped_sum_lifecycle(&document);
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("structure-and-destructive"),
                PrincipalId::from("authority"),
                editor.clone(),
                vec![
                    query,
                    keyed_grouped_sum_requirement(
                        AuthorizationAction::Propose,
                        MutationClass::Structure,
                        &document,
                    ),
                    keyed_grouped_sum_requirement(
                        AuthorizationAction::Propose,
                        MutationClass::Destructive,
                        &document,
                    ),
                ],
                None,
            ))
            .unwrap();
        lifecycle
            .propose(
                &DocumentScopeId::from("document-occurrence"),
                &document,
                &SemanticRevision::from("resident/0"),
                delete(),
                TrustedInstant::new(1),
            )
            .unwrap();
    }

    fn keyed_grouped_sum_query_requirement(subject: SemanticScope) -> DisclosureRequirement {
        DisclosureRequirement {
            family: OperationFamily::KeyedGroupedSumDefinition,
            scope: ScopedSemanticSubject::new(
                DocumentScopeId::from("document-occurrence"),
                DocumentId::from("document"),
                subject,
            ),
        }
    }

    fn keyed_grouped_sum_schema_field(schema: &str, field: &str) -> SemanticScope {
        SemanticScope::SchemaField {
            schema: SchemaId::from(schema),
            field: FieldId::from(field),
        }
    }

    fn keyed_grouped_sum_entity_field(entity: &str, schema: &str, field: &str) -> SemanticScope {
        SemanticScope::EntityField {
            entity: EntityId::from(entity),
            schema: SchemaId::from(schema),
            field: FieldId::from(field),
        }
    }

    // Adds one Products row whose key matches no Orders row. Its category and
    // price are never disclosed by a Query.
    fn keyed_grouped_sum_unmatched_product_document() -> Document {
        let mut document = keyed_grouped_sum_document();
        document.entities.insert(
            EntityId::from("unmatched-product"),
            Entity {
                id: EntityId::from("unmatched-product"),
                key: EntityKey::from("unmatched-product"),
                schema: SchemaId::from("products"),
                fields: BTreeMap::from([
                    (
                        FieldId::from("product-key"),
                        Value::Text("P-404".to_owned()),
                    ),
                    (
                        FieldId::from("category"),
                        Value::Text("unmatched".to_owned()),
                    ),
                    (
                        FieldId::from("price"),
                        Value::Number(Number::new(9.0).unwrap()),
                    ),
                ]),
            },
        );
        document
    }

    fn keyed_grouped_sum_query_footprint(document: &Document) -> BTreeSet<DisclosureRequirement> {
        let (lifecycle, _) = keyed_grouped_sum_lifecycle(document);
        lifecycle
            .keyed_grouped_sum_query_disclosures(
                document,
                &KeyedGroupedSumDefinitionId::from("summary"),
            )
            .unwrap()
    }

    #[test]
    fn keyed_grouped_sum_create_and_update_require_structure_only() {
        let document = keyed_grouped_sum_document();
        let upsert = |id: &str| {
            let mut definition = document
                .keyed_grouped_sum_definitions
                .values()
                .next()
                .expect("fixture definition")
                .clone();
            definition.id = KeyedGroupedSumDefinitionId::from(id);
            SemanticPatchBody::command(SemanticCommand::UpsertKeyedGroupedSumDefinition {
                definition,
            })
        };

        let (mut lifecycle, editor) = keyed_grouped_sum_lifecycle(&document);
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("structure-only"),
                PrincipalId::from("authority"),
                editor.clone(),
                vec![keyed_grouped_sum_requirement(
                    AuthorizationAction::Propose,
                    MutationClass::Structure,
                    &document,
                )],
                None,
            ))
            .unwrap();
        for (proposal, definition) in [("create-summary", "fresh"), ("update-summary", "summary")] {
            lifecycle
                .propose(
                    &DocumentScopeId::from("document-occurrence"),
                    &document,
                    &SemanticRevision::from("resident/0"),
                    ProposalRequest::new(
                        ProposalId::from(proposal),
                        SemanticRevision::from("resident/0"),
                        upsert(definition),
                        editor.clone(),
                    ),
                    TrustedInstant::new(1),
                )
                .unwrap();
        }

        // `Destructive` alone is not the upsert capability.
        let (mut lifecycle, editor) = keyed_grouped_sum_lifecycle(&document);
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("destructive-only"),
                PrincipalId::from("authority"),
                editor.clone(),
                vec![keyed_grouped_sum_requirement(
                    AuthorizationAction::Propose,
                    MutationClass::Destructive,
                    &document,
                )],
                None,
            ))
            .unwrap();
        let error = lifecycle
            .propose(
                &DocumentScopeId::from("document-occurrence"),
                &document,
                &SemanticRevision::from("resident/0"),
                ProposalRequest::new(
                    ProposalId::from("destructive-upsert"),
                    SemanticRevision::from("resident/0"),
                    upsert("fresh"),
                    editor.clone(),
                ),
                TrustedInstant::new(1),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            PatchLifecycleError::InsufficientCapability {
                action: AuthorizationAction::Propose
            }
        ));
    }

    #[test]
    fn keyed_grouped_sum_query_footprint_is_least_privilege_for_unmatched_products() {
        let document = keyed_grouped_sum_unmatched_product_document();
        let footprint = keyed_grouped_sum_query_footprint(&document);

        let expected = BTreeSet::from([
            keyed_grouped_sum_query_requirement(SemanticScope::Document),
            keyed_grouped_sum_query_requirement(SemanticScope::Schema(SchemaId::from("orders"))),
            keyed_grouped_sum_query_requirement(SemanticScope::Schema(SchemaId::from("products"))),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_schema_field(
                "orders",
                "order-key",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_schema_field(
                "orders", "quantity",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_schema_field(
                "products",
                "product-key",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_schema_field(
                "products", "category",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_schema_field(
                "products", "price",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order",
                "orders",
                "order-key",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order", "orders", "quantity",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "product",
                "products",
                "product-key",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "unmatched-product",
                "products",
                "product-key",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "product", "products", "category",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "product", "products", "price",
            )),
        ]);
        assert_eq!(footprint, expected);
    }

    #[test]
    fn keyed_grouped_sum_query_footprint_includes_calculation_roots_only_when_required() {
        let with_unrelated_root = || {
            let mut document = keyed_grouped_sum_unmatched_product_document();
            document
                .entities
                .get_mut(&EntityId::from("unmatched-product"))
                .expect("unmatched product")
                .fields
                .insert(
                    FieldId::from("price"),
                    Value::Formula(Expression::Number(Number::new(9.0).unwrap())),
                );
            document
        };
        let unrelated_root = keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
            "unmatched-product",
            "products",
            "price",
        ));

        // No required operand is formula-backed, so the unrelated root stays
        // out even though it is a document formula.
        assert!(
            !keyed_grouped_sum_query_footprint(&with_unrelated_root()).contains(&unrelated_root)
        );

        // A formula-backed required quantity pulls in the whole Calculation
        // footprint, including the unrelated root.
        let mut quantity_backed = with_unrelated_root();
        quantity_backed
            .entities
            .get_mut(&EntityId::from("order"))
            .expect("order")
            .fields
            .insert(
                FieldId::from("quantity"),
                Value::Formula(Expression::Number(Number::new(1.0).unwrap())),
            );
        assert!(keyed_grouped_sum_query_footprint(&quantity_backed).contains(&unrelated_root));

        // The same holds for a formula-backed matched price.
        let mut price_backed = with_unrelated_root();
        price_backed
            .entities
            .get_mut(&EntityId::from("product"))
            .expect("matched product")
            .fields
            .insert(
                FieldId::from("price"),
                Value::Formula(Expression::Number(Number::new(2.0).unwrap())),
            );
        assert!(keyed_grouped_sum_query_footprint(&price_backed).contains(&unrelated_root));
    }

    // Steward HOLD oracle: `quantity` is required by the grouped definition,
    // and its two-hop bound formula chain is quantity -> quantity-mid ->
    // quantity-base. The unrelated formula is present to prove that required
    // formula calculation includes every root and its own static input, while
    // unrelated-scalar must remain outside the trusted footprint.
    fn keyed_grouped_sum_formula_dependency_document() -> Document {
        let mut document = keyed_grouped_sum_document();
        let orders = document
            .schemas
            .get_mut(&SchemaId::from("orders"))
            .expect("orders schema");
        for field in [
            "quantity-mid",
            "quantity-base",
            "unrelated-formula",
            "unrelated-input",
            "unrelated-scalar",
        ] {
            let id = FieldId::from(field);
            orders.fields.insert(
                id.clone(),
                FieldDefinition {
                    id,
                    key: FieldKey::from(field),
                    display_name: None,
                    number_format: None,
                    field_type: FieldType::Number,
                    required: true,
                    constraint: tachiko_semantic_core::FieldConstraint::None,
                },
            );
        }
        let order = document
            .entities
            .get_mut(&EntityId::from("order"))
            .expect("orders entity");
        order.fields.insert(
            FieldId::from("quantity"),
            Value::Formula(Expression::Reference(FieldRef::new(
                "order",
                "quantity-mid",
            ))),
        );
        order.fields.insert(
            FieldId::from("quantity-mid"),
            Value::Formula(Expression::Reference(FieldRef::new(
                "order",
                "quantity-base",
            ))),
        );
        order.fields.insert(
            FieldId::from("quantity-base"),
            Value::Number(Number::new(3.0).unwrap()),
        );
        order.fields.insert(
            FieldId::from("unrelated-formula"),
            Value::Formula(Expression::Reference(FieldRef::new(
                "order",
                "unrelated-input",
            ))),
        );
        order.fields.insert(
            FieldId::from("unrelated-input"),
            Value::Number(Number::new(11.0).unwrap()),
        );
        order.fields.insert(
            FieldId::from("unrelated-scalar"),
            Value::Number(Number::new(13.0).unwrap()),
        );
        document
    }

    fn formula_dependency_oracle_scopes() -> BTreeSet<DisclosureRequirement> {
        BTreeSet::from([
            // This narrow set isolates the additional formula-root and static
            // dependency facts. Existing focused coverage owns the ordinary
            // definition membership, binding type, and local value scopes.
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order", "orders", "quantity",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order",
                "orders",
                "quantity-mid",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order",
                "orders",
                "quantity-base",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order",
                "orders",
                "unrelated-formula",
            )),
            keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
                "order",
                "orders",
                "unrelated-input",
            )),
        ])
    }

    fn query_grant_requirements(
        requirements: impl IntoIterator<Item = DisclosureRequirement>,
    ) -> Vec<GrantRequirement> {
        requirements
            .into_iter()
            .map(|requirement| GrantRequirement::query(requirement.family, requirement.scope))
            .collect()
    }

    #[test]
    fn keyed_grouped_sum_formula_dependency_footprint_matches_steward_oracle() {
        let document = keyed_grouped_sum_formula_dependency_document();
        let footprint = keyed_grouped_sum_query_footprint(&document);
        let oracle = formula_dependency_oracle_scopes();
        let unrelated_scalar = keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
            "order",
            "orders",
            "unrelated-scalar",
        ));

        // This is intentionally RED on the frozen implementation: it records
        // ADR-0036/ADR-0018 dependency closure independently of its current
        // formula-root-only implementation.
        assert!(oracle.is_subset(&footprint));
        assert!(!footprint.contains(&unrelated_scalar));
    }

    #[test]
    fn keyed_grouped_sum_formula_dependency_narrow_grant_denies_then_succeeds() {
        let document = keyed_grouped_sum_formula_dependency_document();
        let oracle = formula_dependency_oracle_scopes();
        let omitted = keyed_grouped_sum_query_requirement(keyed_grouped_sum_entity_field(
            "order",
            "orders",
            "quantity-base",
        ));
        let (mut lifecycle, reader) = keyed_grouped_sum_lifecycle(&document);
        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("narrow-without-referenced-input"),
                PrincipalId::from("authority"),
                reader.clone(),
                query_grant_requirements(oracle.iter().filter(|scope| *scope != &omitted).cloned()),
                None,
            ))
            .unwrap();
        let error = lifecycle
            .authorize_query(&reader, &oracle, TrustedInstant::new(1))
            .unwrap_err();
        assert!(matches!(
            error,
            PatchLifecycleError::InsufficientCapability {
                action: AuthorizationAction::Query
            }
        ));

        lifecycle
            .provision_grant(Grant::new(
                GrantId::from("referenced-input"),
                PrincipalId::from("authority"),
                reader.clone(),
                query_grant_requirements([omitted]),
                None,
            ))
            .unwrap();
        assert!(
            lifecycle
                .authorize_query(&reader, &oracle, TrustedInstant::new(1))
                .is_ok()
        );
        assert_eq!(
            evaluate_keyed_grouped_sum(&document, &KeyedGroupedSumDefinitionId::from("summary")),
            KeyedGroupedSumOutcome::Complete(vec![KeyedGroupedSumGroup {
                category: "hardware".to_owned(),
                value: Number::new(6.0).unwrap(),
            }])
        );
    }

    #[test]
    fn keyed_grouped_sum_all_literal_control_excludes_unrelated_formula_closure() {
        let mut document = keyed_grouped_sum_formula_dependency_document();
        document
            .entities
            .get_mut(&EntityId::from("order"))
            .expect("orders entity")
            .fields
            .insert(
                FieldId::from("quantity"),
                Value::Number(Number::new(3.0).unwrap()),
            );
        let footprint = keyed_grouped_sum_query_footprint(&document);
        for field in [
            "quantity-mid",
            "quantity-base",
            "unrelated-formula",
            "unrelated-input",
            "unrelated-scalar",
        ] {
            assert!(!footprint.contains(&keyed_grouped_sum_query_requirement(
                keyed_grouped_sum_entity_field("order", "orders", field),
            )));
        }
    }

    #[test]
    fn approx_bytes_counts_text_length_and_formula_nodes() {
        let field = FieldRef::new("entity", "field");
        let short = SemanticCommand::set_field_value(field.clone(), Value::Text("x".to_owned()));
        let long = SemanticCommand::set_field_value(field.clone(), Value::Text("x".repeat(1024)));
        assert_eq!(long.approx_bytes() - short.approx_bytes(), 1023);

        let leaf = Expression::Number(Number::new(1.0).unwrap());
        let one_node =
            SemanticCommand::set_field_value(field.clone(), Value::Formula(leaf.clone()));
        let two_nodes = SemanticCommand::set_field_value(
            field,
            Value::Formula(Expression::Add {
                left: Box::new(leaf),
                right: Box::new(Expression::Number(Number::new(2.0).unwrap())),
            }),
        );
        assert_eq!(
            two_nodes.approx_bytes() - one_node.approx_bytes(),
            2 * std::mem::size_of::<Expression>()
        );
    }

    fn assert_weight_grows(small: &SemanticCommand, large: &SemanticCommand, minimum: usize) {
        let small_bytes = small.approx_bytes();
        let large_bytes = large.approx_bytes();
        assert!(
            large_bytes >= small_bytes + minimum,
            "expected at least {minimum} more bytes, got {small_bytes} -> {large_bytes}"
        );
    }

    fn plain_field(field: &str) -> FieldDefinition {
        FieldDefinition {
            id: FieldId::from(field),
            key: FieldKey::from(field),
            display_name: None,
            number_format: None,
            field_type: FieldType::Text,
            required: false,
            constraint: FieldConstraint::None,
        }
    }

    fn entity_with(id: &str, fields: BTreeMap<FieldId, Value>) -> Entity {
        Entity {
            id: EntityId::from(id),
            key: EntityKey::from(id),
            schema: SchemaId::from("s"),
            fields,
        }
    }

    fn value_command(value: Value) -> SemanticCommand {
        SemanticCommand::SetFieldValue {
            field: FieldRef::new("e", "f"),
            value,
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One nested-item pair per storage site.
    fn nested_storage_grows_the_weight() {
        // Vec<Entity> in AppendCollection.
        let one_entity = SemanticCommand::AppendCollection {
            schema: Schema {
                id: SchemaId::from("s"),
                key: SchemaKey::from("s"),
                display_name: None,
                fields: BTreeMap::new(),
                field_order: None,
            },
            entities: vec![entity_with("e1", BTreeMap::new())],
        };
        let two_entities = SemanticCommand::AppendCollection {
            schema: Schema {
                id: SchemaId::from("s"),
                key: SchemaKey::from("s"),
                display_name: None,
                fields: BTreeMap::new(),
                field_order: None,
            },
            entities: vec![
                entity_with("e1", BTreeMap::new()),
                entity_with("e2", BTreeMap::new()),
            ],
        };
        assert_weight_grows(&one_entity, &two_entities, std::mem::size_of::<Entity>());

        // Vec<EntityId> in RemoveCollection.
        assert_weight_grows(
            &SemanticCommand::RemoveCollection {
                schema: SchemaId::from("s"),
                entities: vec![EntityId::from("e1")],
            },
            &SemanticCommand::RemoveCollection {
                schema: SchemaId::from("s"),
                entities: vec![EntityId::from("e1"), EntityId::from("e2")],
            },
            std::mem::size_of::<EntityId>(),
        );

        // BTreeMap<FieldId, Value> inside one Entity.
        let one_value = BTreeMap::from([(FieldId::from("f"), Value::Text("a".to_owned()))]);
        let two_values = BTreeMap::from([
            (FieldId::from("f"), Value::Text("a".to_owned())),
            (FieldId::from("g"), Value::Text("b".to_owned())),
        ]);
        assert_weight_grows(
            &SemanticCommand::AppendEntity {
                entity: entity_with("e1", one_value.clone()),
            },
            &SemanticCommand::AppendEntity {
                entity: entity_with("e1", two_values.clone()),
            },
            std::mem::size_of::<FieldId>() + std::mem::size_of::<Value>(),
        );

        // BTreeMap<EntityId, Value> in both field commands.
        let one_entity_value =
            BTreeMap::from([(EntityId::from("e1"), Value::Text("a".to_owned()))]);
        let two_entity_values = BTreeMap::from([
            (EntityId::from("e1"), Value::Text("a".to_owned())),
            (EntityId::from("e2"), Value::Text("b".to_owned())),
        ]);
        assert_weight_grows(
            &SemanticCommand::AppendSchemaField {
                schema: SchemaId::from("s"),
                field: plain_field("f"),
                values: one_entity_value.clone(),
            },
            &SemanticCommand::AppendSchemaField {
                schema: SchemaId::from("s"),
                field: plain_field("f"),
                values: two_entity_values.clone(),
            },
            std::mem::size_of::<EntityId>() + std::mem::size_of::<Value>(),
        );
        assert_weight_grows(
            &SemanticCommand::RemoveSchemaField {
                schema: SchemaId::from("s"),
                field: plain_field("f"),
                values: one_entity_value,
            },
            &SemanticCommand::RemoveSchemaField {
                schema: SchemaId::from("s"),
                field: plain_field("f"),
                values: two_entity_values,
            },
            std::mem::size_of::<EntityId>() + std::mem::size_of::<Value>(),
        );

        // Vec<FieldId> in SetSchemaFieldOrder.
        assert_weight_grows(
            &SemanticCommand::SetSchemaFieldOrder {
                schema: SchemaId::from("s"),
                order: Some(vec![FieldId::from("f")]),
            },
            &SemanticCommand::SetSchemaFieldOrder {
                schema: SchemaId::from("s"),
                order: Some(vec![FieldId::from("f"), FieldId::from("g")]),
            },
            std::mem::size_of::<FieldId>(),
        );

        // Vec<String> in a text-literal constraint.
        assert_weight_grows(
            &SemanticCommand::SetFieldConstraint {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                constraint: FieldConstraint::TextLiteralSet {
                    values: vec!["a".to_owned()],
                },
            },
            &SemanticCommand::SetFieldConstraint {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                constraint: FieldConstraint::TextLiteralSet {
                    values: vec!["a".to_owned(), "bb".to_owned()],
                },
            },
            std::mem::size_of::<String>(),
        );

        // Longer identities.
        assert_weight_grows(
            &SemanticCommand::RenameSchemaField {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                key: FieldKey::from("k"),
            },
            &SemanticCommand::RenameSchemaField {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                key: FieldKey::from("longer-key"),
            },
            9,
        );
        assert_weight_grows(
            &SemanticCommand::SetSchemaFieldDisplayName {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                display_name: None,
            },
            &SemanticCommand::SetSchemaFieldDisplayName {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                display_name: Some("display".to_owned()),
            },
            7,
        );
        assert_weight_grows(
            &SemanticCommand::SetFieldNumberFormat {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                number_format: None,
            },
            &SemanticCommand::SetFieldNumberFormat {
                schema: SchemaId::from("s"),
                field: FieldId::from("f"),
                number_format: Some(crate::NumberFormat {
                    decimals: 0,
                    kind: crate::NumberFormatKind::Currency {
                        code: "JPY".to_owned(),
                    },
                    thousands_separator: false,
                }),
            },
            3,
        );
        assert_weight_grows(
            &SemanticCommand::UnsetField {
                field: FieldRef::new("e", "f"),
            },
            &SemanticCommand::UnsetField {
                field: FieldRef::new("entity", "field-name"),
            },
            14,
        );

        // BTreeSet<FieldRef> in FormulaUpdate.
        let formula = Expression::Number(Number::new(1.0).unwrap());
        assert_weight_grows(
            &SemanticCommand::FormulaUpdate(FormulaUpdateCommand {
                target: FieldRef::new("e", "f"),
                expression: formula.clone(),
                references: BTreeSet::from([FieldRef::new("e", "f")]),
            }),
            &SemanticCommand::FormulaUpdate(FormulaUpdateCommand {
                target: FieldRef::new("e", "f"),
                expression: formula.clone(),
                references: BTreeSet::from([FieldRef::new("e", "f"), FieldRef::new("e2", "f2")]),
            }),
            std::mem::size_of::<FieldRef>(),
        );

        // A longer identity inside each formula-carrying command.
        let short_reference = Expression::Reference(FieldRef::new("e", "f"));
        let long_reference = Expression::Reference(FieldRef::new("entity", "field"));
        assert_weight_grows(
            &value_command(Value::Formula(short_reference.clone())),
            &value_command(Value::Formula(long_reference.clone())),
            9,
        );
        assert_weight_grows(
            &SemanticCommand::FormulaRestore(FormulaRestoreCommand {
                target: FieldRef::new("e", "f"),
                expression: short_reference.clone(),
            }),
            &SemanticCommand::FormulaRestore(FormulaRestoreCommand {
                target: FieldRef::new("e", "f"),
                expression: long_reference,
            }),
            9,
        );
        assert_weight_grows(
            &SemanticCommand::FormulaUpdate(FormulaUpdateCommand {
                target: FieldRef::new("e", "f"),
                expression: short_reference,
                references: BTreeSet::new(),
            }),
            &SemanticCommand::FormulaUpdate(FormulaUpdateCommand {
                target: FieldRef::new("e", "f"),
                expression: Expression::Reference(FieldRef::new("entity", "field")),
                references: BTreeSet::new(),
            }),
            9,
        );

        // Other formula node kinds carry their own identities.
        assert_weight_grows(
            &value_command(Value::Formula(Expression::SameRowReference(FieldId::from(
                "f",
            )))),
            &value_command(Value::Formula(Expression::SameRowReference(FieldId::from(
                "field-name",
            )))),
            9,
        );
        assert_weight_grows(
            &value_command(Value::Formula(Expression::ColumnTotal(ColumnRef::new(
                "s", "f",
            )))),
            &value_command(Value::Formula(Expression::ColumnTotal(ColumnRef::new(
                "schema", "field",
            )))),
            9,
        );
        assert_weight_grows(
            &value_command(Value::Formula(Expression::ColumnAggregate {
                function: crate::ColumnAggregateFunction::Average,
                column: ColumnRef::new("s", "f"),
            })),
            &value_command(Value::Formula(Expression::ColumnAggregate {
                function: crate::ColumnAggregateFunction::Average,
                column: ColumnRef::new("schema", "field"),
            })),
            9,
        );

        // ColumnConditional owns its aggregate box, criteria vector and columns.
        let criterion = |field: &str| ColumnCriterion {
            column: ColumnRef::new("s", field),
            operator: ComparisonOperator::Equal,
            value: None,
        };
        assert_weight_grows(
            &value_command(Value::Formula(Expression::ColumnConditional {
                aggregate: Box::new(ConditionalAggregate::Count),
                criteria: vec![criterion("f")],
            })),
            &value_command(Value::Formula(Expression::ColumnConditional {
                aggregate: Box::new(ConditionalAggregate::Count),
                criteria: vec![criterion("f"), criterion("g")],
            })),
            std::mem::size_of::<ColumnCriterion>(),
        );

        // Saved-definition identities.
        let definition = KeyedGroupedSumDefinition {
            id: KeyedGroupedSumDefinitionId::from("d"),
            orders: KeyedGroupedSumOrdersBinding {
                schema: SchemaId::from("o"),
                lookup_key_field: FieldId::from("k"),
                quantity_field: FieldId::from("q"),
            },
            products: KeyedGroupedSumProductsBinding {
                schema: SchemaId::from("p"),
                key_field: FieldId::from("k"),
                category_field: FieldId::from("c"),
                price_field: FieldId::from("v"),
            },
        };
        let mut longer = definition.clone();
        longer.id = KeyedGroupedSumDefinitionId::from("definition-long");
        assert_weight_grows(
            &SemanticCommand::UpsertKeyedGroupedSumDefinition { definition },
            &SemanticCommand::UpsertKeyedGroupedSumDefinition { definition: longer },
            14,
        );
        assert_weight_grows(
            &SemanticCommand::RemoveKeyedGroupedSumDefinition {
                definition: KeyedGroupedSumDefinitionId::from("d"),
            },
            &SemanticCommand::RemoveKeyedGroupedSumDefinition {
                definition: KeyedGroupedSumDefinitionId::from("definition-long"),
            },
            14,
        );
    }
}
