//! Provisional resident semantic state for first-party interactive hosts.
//!
//! These Rust types are workspace-internal implementation details. They do not
//! define a public session, revision, result, serialization, or transport
//! contract. They are trusted runtime primitives rather than client endpoints;
//! host adapters must enforce ADR-0026 Query authorization before projecting
//! their observations outside the trusted composition boundary.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{
    CalculationOutcome, IncrementalCalculationTransition, IncrementalCalculationWork,
    RetainedCalculationState,
};

use super::{
    AddressIndex, AddressIndexError, CalculatedField, Diagnostic, Document, EntityId,
    EntityInspection, Expression, FieldAddress, FieldRef, SemanticSubject, ValidationReport,
    ValidationRole, Value, WorkspaceError,
    formula_operations::{FormulaCalculationOutcome, calculation_dependencies, calculation_for},
    invalid_document,
    patch_lifecycle::{
        DocumentScopeId, SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision,
        TrustedInstant,
    },
    validation_report_for_retained_calculation,
};

/// One Rust-authoritative semantic document occurrence retained across calls.
pub struct ResidentWorkspaceSession {
    document_scope: DocumentScopeId,
    document: Document,
    retained: ResidentDerivedState,
    revision: SemanticRevision,
    generation: u64,
    measurements: ResidentRuntimeMeasurements,
}

impl ResidentWorkspaceSession {
    /// Start a resident occurrence at its initial internal revision.
    #[must_use]
    pub fn new(document_scope: DocumentScopeId, document: Document) -> Self {
        let (retained, calculation_work) = ResidentDerivedState::rebuild(&document);
        Self {
            document_scope,
            document,
            retained,
            revision: revision_for(0),
            generation: 0,
            measurements: ResidentRuntimeMeasurements::initial(calculation_work),
        }
    }

    /// Return the current opaque semantic revision token.
    #[must_use]
    pub fn revision(&self) -> &SemanticRevision {
        &self.revision
    }

    /// Run the existing authoritative validation query against this revision.
    #[must_use]
    pub fn validation_report(&self) -> ResidentQueryResult<ValidationReport> {
        ResidentQueryResult {
            document_scope: self.document_scope.clone(),
            revision: self.revision.clone(),
            value: self.retained.validation_report.clone(),
        }
    }

    /// Run the existing authoritative calculation query against this revision.
    ///
    /// # Errors
    ///
    /// Returns the existing semantic, projection, or calculation failure
    /// without changing resident state or revision.
    pub fn calculate_fields(
        &self,
    ) -> Result<ResidentQueryResult<Vec<CalculatedField>>, WorkspaceError> {
        Ok(ResidentQueryResult {
            document_scope: self.document_scope.clone(),
            revision: self.revision.clone(),
            value: self.retained.calculate_fields(&self.document)?,
        })
    }

    /// Return deterministic runtime work evidence accumulated by this
    /// occurrence. Counters are implementation measurements only; they are not
    /// semantic identity, a cache protocol, or a performance guarantee.
    #[must_use]
    pub const fn runtime_measurements(&self) -> ResidentRuntimeMeasurements {
        self.measurements
    }

    /// Project only the requested entities in stable-ID order.
    ///
    /// The projection contains semantic identity and presentation metadata,
    /// but never clones field values or the complete document.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::MissingEntityId`] when any requested stable
    /// entity subject is absent from the resident revision.
    pub fn query_entities(
        &self,
        requested: &[EntityId],
    ) -> Result<ResidentQueryResult<Vec<EntityInspection>>, WorkspaceError> {
        let requested = requested.iter().cloned().collect::<BTreeSet<_>>();
        let entities = requested
            .into_iter()
            .map(|id| {
                let entity = self
                    .document
                    .entities
                    .get(&id)
                    .ok_or_else(|| WorkspaceError::MissingEntityId { entity: id.clone() })?;
                Ok(EntityInspection {
                    id,
                    key: entity.key.clone(),
                    schema: entity.schema.clone(),
                    fields: entity.fields.keys().cloned().collect(),
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()?;

        Ok(ResidentQueryResult {
            document_scope: self.document_scope.clone(),
            revision: self.revision.clone(),
            value: entities,
        })
    }

    /// Project only the requested stable field subjects in deterministic order.
    ///
    /// Stored literals, bound formula definitions, calculated outcomes,
    /// stable-subject diagnostics, and mutable human addresses remain separate
    /// so clients cannot confuse presentation or derived state with meaning.
    /// Full validation and calculation remain the correctness oracle; this
    /// method only bounds the returned projection.
    ///
    /// # Errors
    ///
    /// Returns the existing typed field or schema lookup failure.
    pub fn query_fields(
        &self,
        requested: &[FieldRef],
    ) -> Result<ResidentQueryResult<Vec<ResidentFieldProjection>>, WorkspaceError> {
        let requested = requested.iter().cloned().collect::<BTreeSet<_>>();
        let fields = requested
            .into_iter()
            .map(|field| {
                let entity = self.document.entities.get(&field.entity).ok_or_else(|| {
                    WorkspaceError::MissingField {
                        field: field.clone(),
                    }
                })?;
                let value = entity.fields.get(&field.field).ok_or_else(|| {
                    WorkspaceError::MissingField {
                        field: field.clone(),
                    }
                })?;
                let schema = self.document.schemas.get(&entity.schema).ok_or_else(|| {
                    WorkspaceError::MissingSchema {
                        schema: entity.schema.clone(),
                    }
                })?;
                let definition = schema.fields.get(&field.field).ok_or_else(|| {
                    WorkspaceError::MissingField {
                        field: field.clone(),
                    }
                })?;
                let presentation_address =
                    FieldAddress::new(entity.key.clone(), definition.key.clone());
                let diagnostics = self.retained.diagnostics_for_field(&field);
                let (stored_value, formula_definition, calculated_value) = match value {
                    Value::Formula(expression) => (
                        None,
                        Some(expression.clone()),
                        Some(retained_calculation_for(
                            &self.retained.calculation_state,
                            &field,
                        )),
                    ),
                    scalar => (Some(scalar.clone()), None, None),
                };

                Ok(ResidentFieldProjection {
                    field,
                    stored_value,
                    formula_definition,
                    calculated_value,
                    diagnostics,
                    presentation_address,
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()?;

        Ok(ResidentQueryResult {
            document_scope: self.document_scope.clone(),
            revision: self.revision.clone(),
            value: fields,
        })
    }

    /// Clone the full semantic state at an explicit host snapshot boundary.
    #[must_use]
    pub fn export_snapshot(&self) -> ResidentSnapshot {
        ResidentSnapshot {
            document_scope: self.document_scope.clone(),
            revision: self.revision.clone(),
            document: self.document.clone(),
        }
    }

    /// Borrow the resident state as the existing trusted publication seam.
    ///
    /// The time source remains a host capability and is invoked only inside
    /// the exclusive compare-and-publish call. Untrusted adapters must not
    /// construct or select it.
    pub fn publication_authority<'session, Time>(
        &'session mut self,
        time: &'session mut Time,
    ) -> ResidentPublicationAuthority<'session, Time>
    where
        Time: TrustedPublicationTimeSource,
    {
        ResidentPublicationAuthority {
            session: self,
            time,
            projection_invalidation: None,
        }
    }
}

#[derive(Clone, Debug)]
struct ResidentDerivedState {
    calculation_state: RetainedCalculationState,
    validation_report: ValidationReport,
    address_index: Result<AddressIndex, AddressIndexError>,
    diagnostics_by_field: BTreeMap<FieldRef, Vec<Diagnostic>>,
}

impl ResidentDerivedState {
    fn rebuild(document: &Document) -> (Self, IncrementalCalculationWork) {
        let (calculation_state, calculation_work) = RetainedCalculationState::rebuild(document);
        (
            Self::from_calculation(document, calculation_state, AddressIndex::build(document)),
            calculation_work,
        )
    }

    fn update(
        &mut self,
        document: &Document,
        changes: &ProjectionChanges,
    ) -> DerivedStateTransition {
        let previous_address_index_valid = self.address_index.is_ok();
        if changes.address_index_changed {
            self.address_index = AddressIndex::build(document);
        }
        let incremental_safe = previous_address_index_valid && self.address_index.is_ok();
        let calculation = if incremental_safe {
            self.calculation_state
                .update(document, &changes.calculation_roots)
        } else {
            let before = self.calculation_state.outcome();
            let old_affected = self
                .calculation_state
                .affected_by_all(&changes.calculation_roots);
            let (rebuilt, work) = RetainedCalculationState::rebuild(document);
            let new_affected = rebuilt.affected_by_all(&changes.calculation_roots);
            let after = rebuilt.outcome();
            self.calculation_state = rebuilt;
            IncrementalCalculationTransition {
                work,
                affected_calculations: old_affected
                    .into_iter()
                    .chain(new_affected)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                changed_calculation_projections: changed_calculation_outcomes(&before, &after),
            }
        };
        self.validation_report =
            validation_report_for_retained_calculation(document, &self.calculation_state);
        self.diagnostics_by_field = index_field_diagnostics(&self.validation_report);
        DerivedStateTransition {
            work: DerivedStateTransitionWork {
                calculation: calculation.work,
                address_index_rebuilt: changes.address_index_changed,
                fell_back_to_full_calculation: !incremental_safe,
            },
            affected_calculations: calculation.affected_calculations,
            changed_calculation_projections: calculation.changed_calculation_projections,
        }
    }

    fn from_calculation(
        document: &Document,
        calculation_state: RetainedCalculationState,
        address_index: Result<AddressIndex, AddressIndexError>,
    ) -> Self {
        let validation_report =
            validation_report_for_retained_calculation(document, &calculation_state);
        let diagnostics_by_field = index_field_diagnostics(&validation_report);
        Self {
            calculation_state,
            validation_report,
            address_index,
            diagnostics_by_field,
        }
    }

    fn calculate_fields(
        &self,
        document: &Document,
    ) -> Result<Vec<CalculatedField>, WorkspaceError> {
        if !self.validation_report.is_valid() {
            return Err(invalid_document(
                self.validation_report.clone(),
                ValidationRole::Current,
            ));
        }
        let values = self
            .calculation_state
            .complete_values()
            .expect("a diagnostic-free formula outcome is complete");
        let index = self
            .address_index
            .as_ref()
            .map_err(|source| WorkspaceError::from(source.clone()))?;
        let mut fields = values
            .iter()
            .map(|(field, value)| {
                Ok(CalculatedField {
                    field: field.clone(),
                    address: index.field_address(document, field)?,
                    value: *value,
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()?;
        fields.sort_by(|left, right| left.address.cmp(&right.address));
        Ok(fields)
    }

    fn diagnostics_for_field(&self, field: &FieldRef) -> Vec<Diagnostic> {
        self.diagnostics_by_field
            .get(field)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DerivedStateTransitionWork {
    calculation: IncrementalCalculationWork,
    address_index_rebuilt: bool,
    fell_back_to_full_calculation: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DerivedStateTransition {
    work: DerivedStateTransitionWork,
    affected_calculations: Vec<FieldRef>,
    changed_calculation_projections: Vec<FieldRef>,
}

/// Cumulative deterministic work evidence for one resident occurrence.
///
/// This provisional runtime-only measurement surface supports oracle and
/// benchmark tests. It is not serialized and does not define semantic identity,
/// client invalidation, a cache protocol, or a product SLA.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentRuntimeMeasurements {
    pub derived_state_rebuilds: usize,
    pub retained_before_state_reuses: usize,
    pub full_calculation_rebuilds: usize,
    pub incremental_calculation_updates: usize,
    pub calculation_nodes_recomputed: usize,
    pub calculation_nodes_reused: usize,
    pub reverse_edges_traversed: usize,
    pub validation_report_rebuilds: usize,
    pub address_index_rebuilds: usize,
    pub address_index_reuses: usize,
    pub calculation_fallbacks: usize,
}

impl ResidentRuntimeMeasurements {
    fn initial(calculation: IncrementalCalculationWork) -> Self {
        Self {
            derived_state_rebuilds: 1,
            full_calculation_rebuilds: calculation.full_rebuilds,
            incremental_calculation_updates: calculation.incremental_updates,
            calculation_nodes_recomputed: calculation.nodes_recomputed,
            calculation_nodes_reused: calculation.nodes_reused,
            reverse_edges_traversed: calculation.reverse_edges_traversed,
            validation_report_rebuilds: 1,
            address_index_rebuilds: 1,
            ..Self::default()
        }
    }

    fn record_transition(&mut self, work: DerivedStateTransitionWork) {
        self.derived_state_rebuilds = self.derived_state_rebuilds.saturating_add(1);
        if !work.fell_back_to_full_calculation {
            self.retained_before_state_reuses = self.retained_before_state_reuses.saturating_add(1);
        }
        self.full_calculation_rebuilds = self
            .full_calculation_rebuilds
            .saturating_add(work.calculation.full_rebuilds);
        self.incremental_calculation_updates = self
            .incremental_calculation_updates
            .saturating_add(work.calculation.incremental_updates);
        self.calculation_nodes_recomputed = self
            .calculation_nodes_recomputed
            .saturating_add(work.calculation.nodes_recomputed);
        self.calculation_nodes_reused = self
            .calculation_nodes_reused
            .saturating_add(work.calculation.nodes_reused);
        self.reverse_edges_traversed = self
            .reverse_edges_traversed
            .saturating_add(work.calculation.reverse_edges_traversed);
        self.validation_report_rebuilds = self.validation_report_rebuilds.saturating_add(1);
        if work.address_index_rebuilt {
            self.address_index_rebuilds = self.address_index_rebuilds.saturating_add(1);
        } else {
            self.address_index_reuses = self.address_index_reuses.saturating_add(1);
        }
        if work.fell_back_to_full_calculation {
            self.calculation_fallbacks = self.calculation_fallbacks.saturating_add(1);
        }
    }
}

fn index_field_diagnostics(report: &ValidationReport) -> BTreeMap<FieldRef, Vec<Diagnostic>> {
    let mut by_field = BTreeMap::<FieldRef, Vec<Diagnostic>>::new();
    for diagnostic in report.diagnostics() {
        for subject in &diagnostic.subjects {
            if let SemanticSubject::EntityField(field) = subject {
                by_field
                    .entry(field.clone())
                    .or_default()
                    .push(diagnostic.clone());
            }
        }
    }
    by_field
}

/// One query observation pinned to the resident occurrence and revision it read.
#[derive(Clone, Debug, PartialEq)]
pub struct ResidentQueryResult<T> {
    document_scope: DocumentScopeId,
    revision: SemanticRevision,
    value: T,
}

/// Bounded facts for one stable field at one resident revision.
#[derive(Clone, Debug, PartialEq)]
pub struct ResidentFieldProjection {
    pub field: FieldRef,
    pub stored_value: Option<Value>,
    pub formula_definition: Option<Expression>,
    pub calculated_value: Option<FormulaCalculationOutcome>,
    pub diagnostics: Vec<Diagnostic>,
    pub presentation_address: FieldAddress,
}

/// Stable subjects whose projections became stale across one scoped publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentProjectionInvalidation {
    pub document_scope: DocumentScopeId,
    pub base_revision: SemanticRevision,
    pub resulting_revision: SemanticRevision,
    pub entities: Vec<EntityId>,
    pub fields: Vec<FieldRef>,
    pub affected_calculations: Vec<FieldRef>,
}

impl<T> ResidentQueryResult<T> {
    #[must_use]
    pub fn document_scope(&self) -> &DocumentScopeId {
        &self.document_scope
    }

    #[must_use]
    pub fn revision(&self) -> &SemanticRevision {
        &self.revision
    }

    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    /// Report whether this detached observation belongs to another occurrence
    /// or predates the supplied revision in the same occurrence.
    #[must_use]
    pub fn is_stale_against(
        &self,
        document_scope: &DocumentScopeId,
        current: &SemanticRevision,
    ) -> bool {
        &self.document_scope != document_scope || &self.revision != current
    }
}

/// An explicit detached export of one resident semantic occurrence.
#[derive(Clone, Debug, PartialEq)]
pub struct ResidentSnapshot {
    document_scope: DocumentScopeId,
    revision: SemanticRevision,
    document: Document,
}

impl ResidentSnapshot {
    #[must_use]
    pub fn document_scope(&self) -> &DocumentScopeId {
        &self.document_scope
    }

    #[must_use]
    pub fn revision(&self) -> &SemanticRevision {
        &self.revision
    }

    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    #[must_use]
    pub fn into_document(self) -> Document {
        self.document
    }
}

/// Trusted host clock capability used only at the guarded publication seam.
pub trait TrustedPublicationTimeSource {
    fn now(&mut self) -> TrustedInstant;
}

/// Exclusive resident-state installation authority composed with a host clock.
pub struct ResidentPublicationAuthority<'session, Time> {
    session: &'session mut ResidentWorkspaceSession,
    time: &'session mut Time,
    projection_invalidation: Option<ResidentProjectionInvalidation>,
}

impl<Time> ResidentPublicationAuthority<'_, Time> {
    /// Return invalidation derived from this authority's exact successful
    /// publication when both revision tokens match that publication.
    ///
    /// The observation is ephemeral: a new publication attempt clears it,
    /// and dropping this exclusive authority discards it.
    #[must_use]
    pub fn projection_invalidation_for(
        &self,
        document_scope: &DocumentScopeId,
        base_revision: &SemanticRevision,
        resulting_revision: &SemanticRevision,
    ) -> Option<&ResidentProjectionInvalidation> {
        self.projection_invalidation
            .as_ref()
            .filter(|invalidation| {
                &invalidation.document_scope == document_scope
                    && &invalidation.base_revision == base_revision
                    && &invalidation.resulting_revision == resulting_revision
            })
    }
}

impl<Time> SemanticPublicationAuthority for ResidentPublicationAuthority<'_, Time>
where
    Time: TrustedPublicationTimeSource,
{
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision) {
        (
            self.session.document_scope.clone(),
            self.session.document.clone(),
            self.session.revision.clone(),
        )
    }

    fn publish_if_current<Authorization>(
        &mut self,
        expected_document_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<Authorization>,
    ) -> Result<
        (DocumentScopeId, Document, SemanticRevision, Authorization),
        SemanticPublicationError,
    > {
        self.projection_invalidation = None;
        if expected_document_scope != &self.session.document_scope {
            return Err(SemanticPublicationError::DocumentScopeMismatch);
        }
        let authorization =
            authorize(self.time.now()).ok_or(SemanticPublicationError::AuthorizationDenied)?;
        if expected_revision != &self.session.revision {
            return Err(SemanticPublicationError::Stale);
        }
        if candidate.id != self.session.document.id {
            return Err(SemanticPublicationError::Conflict);
        }
        let next_generation = self
            .session
            .generation
            .checked_add(1)
            .ok_or(SemanticPublicationError::Conflict)?;
        let resulting_revision = revision_for(next_generation);
        let (projection_invalidation, transition_work) = projection_invalidation(
            &self.session.document,
            &mut self.session.retained,
            &candidate,
            self.session.document_scope.clone(),
            expected_revision.clone(),
            resulting_revision.clone(),
        );

        self.session.document = candidate;
        self.session.generation = next_generation;
        self.session.revision = resulting_revision.clone();
        self.session.measurements.record_transition(transition_work);
        self.projection_invalidation = Some(projection_invalidation);

        Ok((
            self.session.document_scope.clone(),
            self.session.document.clone(),
            resulting_revision,
            authorization,
        ))
    }
}

fn revision_for(generation: u64) -> SemanticRevision {
    SemanticRevision::from(format!("resident/{generation}"))
}

fn projection_invalidation(
    before: &Document,
    retained: &mut ResidentDerivedState,
    after: &Document,
    document_scope: DocumentScopeId,
    base_revision: SemanticRevision,
    resulting_revision: SemanticRevision,
) -> (ResidentProjectionInvalidation, DerivedStateTransitionWork) {
    let entity_ids = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = ProjectionChanges::default();
    collect_entity_projection_changes(before, after, &entity_ids, &mut changes);
    collect_schema_projection_changes(before, after, &entity_ids, &mut changes);
    let before_diagnostics = std::mem::take(&mut retained.diagnostics_by_field);
    let transition = retained.update(after, &changes);
    collect_diagnostic_projection_changes(
        before,
        &before_diagnostics,
        after,
        &retained.diagnostics_by_field,
        &entity_ids,
        &mut changes,
    );

    let affected_calculations = transition
        .affected_calculations
        .into_iter()
        .chain(transition.changed_calculation_projections)
        .collect::<BTreeSet<_>>();

    (
        ResidentProjectionInvalidation {
            document_scope,
            base_revision,
            resulting_revision,
            entities: changes.entities.into_iter().collect(),
            fields: changes.fields.into_iter().collect(),
            affected_calculations: affected_calculations.into_iter().collect(),
        },
        transition.work,
    )
}

#[derive(Default)]
struct ProjectionChanges {
    entities: BTreeSet<EntityId>,
    fields: BTreeSet<FieldRef>,
    calculation_roots: BTreeSet<FieldRef>,
    address_index_changed: bool,
}

fn collect_entity_projection_changes(
    before: &Document,
    after: &Document,
    entity_ids: &BTreeSet<EntityId>,
    changes: &mut ProjectionChanges,
) {
    for entity_id in entity_ids {
        match (
            before.entities.get(entity_id),
            after.entities.get(entity_id),
        ) {
            (Some(before_entity), Some(after_entity)) => {
                let field_ids = before_entity
                    .fields
                    .keys()
                    .chain(after_entity.fields.keys())
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let presentation_changed = before_entity.key != after_entity.key;
                let schema_changed = before_entity.schema != after_entity.schema;
                if presentation_changed || schema_changed || before_entity.id != after_entity.id {
                    changes.address_index_changed = true;
                }

                if presentation_changed || schema_changed {
                    changes.entities.insert(entity_id.clone());
                    changes.fields.extend(
                        field_ids
                            .iter()
                            .cloned()
                            .map(|field_id| FieldRef::new(entity_id.clone(), field_id)),
                    );
                }
                if schema_changed {
                    changes.calculation_roots.extend(
                        field_ids
                            .iter()
                            .cloned()
                            .map(|field_id| FieldRef::new(entity_id.clone(), field_id)),
                    );
                }

                for field_id in field_ids {
                    if before_entity.fields.get(&field_id) != after_entity.fields.get(&field_id) {
                        let field = FieldRef::new(entity_id.clone(), field_id);
                        changes.fields.insert(field.clone());
                        changes.calculation_roots.insert(field);
                    }
                }
                if before_entity.fields.keys().ne(after_entity.fields.keys()) {
                    changes.entities.insert(entity_id.clone());
                }
            }
            (Some(entity), None) | (None, Some(entity)) => {
                changes.address_index_changed = true;
                changes.entities.insert(entity_id.clone());
                let entity_fields = entity
                    .fields
                    .keys()
                    .cloned()
                    .map(|field_id| FieldRef::new(entity_id.clone(), field_id))
                    .collect::<BTreeSet<_>>();
                changes.fields.extend(entity_fields.iter().cloned());
                changes.calculation_roots.extend(entity_fields);
            }
            (None, None) => unreachable!("entity ID came from one document"),
        }
    }
}

fn collect_schema_projection_changes(
    before: &Document,
    after: &Document,
    entity_ids: &BTreeSet<EntityId>,
    changes: &mut ProjectionChanges,
) {
    let schema_ids = before
        .schemas
        .keys()
        .chain(after.schemas.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for schema_id in schema_ids {
        let before_schema = before.schemas.get(&schema_id);
        let after_schema = after.schemas.get(&schema_id);
        if before_schema.map(|schema| (&schema.id, &schema.key))
            != after_schema.map(|schema| (&schema.id, &schema.key))
        {
            changes.address_index_changed = true;
        }
        let field_ids = before_schema
            .into_iter()
            .flat_map(|schema| schema.fields.keys())
            .chain(
                after_schema
                    .into_iter()
                    .flat_map(|schema| schema.fields.keys()),
            )
            .cloned()
            .collect::<BTreeSet<_>>();

        for field_id in field_ids {
            let before_definition = before_schema.and_then(|schema| schema.fields.get(&field_id));
            let after_definition = after_schema.and_then(|schema| schema.fields.get(&field_id));
            if before_definition == after_definition {
                continue;
            }
            let address_definition_changed = match (before_definition, after_definition) {
                (Some(before), Some(after)) => before.id != after.id || before.key != after.key,
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };
            changes.address_index_changed |= address_definition_changed;
            let semantic_definition_changed = match (before_definition, after_definition) {
                (Some(before), Some(after)) => {
                    before.id != after.id
                        || before.field_type != after.field_type
                        || before.required != after.required
                }
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };

            for entity_id in entity_ids {
                let before_has_field = before.entities.get(entity_id).is_some_and(|entity| {
                    entity.schema == schema_id && entity.fields.contains_key(&field_id)
                });
                let after_has_field = after.entities.get(entity_id).is_some_and(|entity| {
                    entity.schema == schema_id && entity.fields.contains_key(&field_id)
                });
                if before_has_field || after_has_field {
                    let field = FieldRef::new(entity_id.clone(), field_id.clone());
                    changes.fields.insert(field.clone());
                    if semantic_definition_changed {
                        changes.calculation_roots.insert(field);
                    }
                }
            }
        }
    }
}

fn collect_diagnostic_projection_changes(
    before: &Document,
    before_diagnostics: &BTreeMap<FieldRef, Vec<Diagnostic>>,
    after: &Document,
    after_diagnostics: &BTreeMap<FieldRef, Vec<Diagnostic>>,
    entity_ids: &BTreeSet<EntityId>,
    changes: &mut ProjectionChanges,
) {
    for entity_id in entity_ids {
        let field_ids = before
            .entities
            .get(entity_id)
            .into_iter()
            .flat_map(|entity| entity.fields.keys())
            .chain(
                after
                    .entities
                    .get(entity_id)
                    .into_iter()
                    .flat_map(|entity| entity.fields.keys()),
            )
            .cloned()
            .collect::<BTreeSet<_>>();

        for field_id in field_ids {
            let field = FieldRef::new(entity_id.clone(), field_id);
            if before_diagnostics.get(&field) != after_diagnostics.get(&field) {
                changes.fields.insert(field);
            }
        }
    }
}

fn changed_calculation_outcomes(
    before: &CalculationOutcome,
    after: &CalculationOutcome,
) -> Vec<FieldRef> {
    calculation_dependencies(before)
        .keys()
        .chain(calculation_dependencies(after).keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|field| calculation_for(before, field) != calculation_for(after, field))
        .collect()
}

fn retained_calculation_for(
    calculation: &RetainedCalculationState,
    target: &FieldRef,
) -> FormulaCalculationOutcome {
    if calculation.is_failed() {
        calculation
            .failure(target)
            .map_or(FormulaCalculationOutcome::Unavailable, |failure| {
                FormulaCalculationOutcome::Failure(failure.clone())
            })
    } else {
        calculation.value(target).map_or(
            FormulaCalculationOutcome::Unavailable,
            FormulaCalculationOutcome::Value,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedTime;

    impl TrustedPublicationTimeSource for FixedTime {
        fn now(&mut self) -> TrustedInstant {
            TrustedInstant::new(1)
        }
    }

    #[test]
    fn revision_exhaustion_fails_closed_without_installing_candidate() {
        let mut session = ResidentWorkspaceSession::new(
            DocumentScopeId::from("occurrence"),
            Document::empty("document", "Before"),
        );
        session.generation = u64::MAX;
        session.revision = revision_for(u64::MAX);
        let before = session.export_snapshot();
        let mut candidate = before.document().clone();
        candidate.title = "Must not install".to_owned();
        let mut time = FixedTime;

        let error = session
            .publication_authority(&mut time)
            .publish_if_current(
                before.document_scope(),
                before.revision(),
                candidate,
                |_| Some(()),
            )
            .unwrap_err();

        assert_eq!(error, SemanticPublicationError::Conflict);
        assert_eq!(session.export_snapshot(), before);
    }
}
