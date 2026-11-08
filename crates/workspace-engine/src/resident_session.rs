//! Provisional resident semantic state for first-party interactive hosts.
//!
//! These Rust types are workspace-internal implementation details. They do not
//! define a public session, revision, result, serialization, or transport
//! contract. They are trusted runtime primitives rather than client endpoints;
//! host adapters must enforce ADR-0026 Query authorization before projecting
//! their observations outside the trusted composition boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use tachiko_diff_engine::DiffScope;
use tachiko_formula_engine::{
    Calculation, CalculationOutcome, IncrementalCalculationTransition, IncrementalCalculationWork,
    RetainedCalculationState,
};

use super::{
    AddressIndex, AddressIndexError, CalculatedField, Diagnostic, Document, EntityId,
    EntityInspection, Expression, FieldAddress, FieldId, FieldRef, PreparedEvaluation,
    PreparedInstallation, SchemaId, SemanticSubject, ValidationReport, ValidationRole, Value,
    WorkspaceError,
    formula_operations::{FormulaCalculationOutcome, calculation_for, calculation_formula_keys},
    instrumentation, invalid_document,
    patch_lifecycle::{
        DocumentScopeId, SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision,
        TrustedInstant,
    },
    validate_entity_field_core, validate_formula_value_constraint,
    validation_report_for_retained_calculation,
};

/// One occurrence's identity, unique within this process.
///
/// It is deliberately neither `Clone` nor `Copy`: a cloned session would share
/// the id and could then accept the other copy's prepared installation, so a
/// clone must issue a new id through [`SessionOccurrence::next`]. Numbers wrap
/// only after 2^64 sessions, which is not handled.
pub(crate) struct SessionOccurrence(u64);

impl SessionOccurrence {
    /// Issue an id no other live occurrence already used.
    fn next() -> Self {
        // Only uniqueness matters, never ordering or any relation to other
        // state, so the relaxed ordering is enough.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Proof that one occurrence's document at one revision is the base a
/// lifecycle call names.
///
/// The proof borrows the occurrence's current document instead of copying it,
/// so it cannot outlive the session or describe a document the session has
/// replaced. Only [`ResidentWorkspaceSession`] issues one. Every replacement of
/// a session's document also advances its revision, so the occurrence and
/// revision pair names exactly one document.
pub struct DocumentWitness<'session> {
    occurrence: &'session SessionOccurrence,
    revision: &'session SemanticRevision,
    document: &'session Document,
}

impl<'session> DocumentWitness<'session> {
    pub(crate) const fn occurrence_id(&self) -> u64 {
        self.occurrence.0
    }

    pub(crate) const fn revision(&self) -> &'session SemanticRevision {
        self.revision
    }

    pub(crate) const fn document(&self) -> &'session Document {
        self.document
    }

    /// Build a witness for a test's explicit identity triple.
    #[cfg(test)]
    pub(crate) const fn for_test(
        occurrence: &'session SessionOccurrence,
        revision: &'session SemanticRevision,
        document: &'session Document,
    ) -> Self {
        Self {
            occurrence,
            revision,
            document,
        }
    }
}

/// One Rust-authoritative semantic document occurrence retained across calls.
pub struct ResidentWorkspaceSession {
    document_scope: DocumentScopeId,
    /// The occurrence this session identifies. It is not `Clone`, so deriving
    /// `Clone` on the session cannot compile: a clone would share the id and
    /// could accept this occurrence's prepared installation.
    occurrence: SessionOccurrence,
    document: Document,
    retained: ResidentDerivedState,
    revision: SemanticRevision,
    generation: u64,
    measurements: ResidentRuntimeMeasurements,
    /// Whether `document` is known to pass the formula projection preflight.
    /// A publication that establishes it sets the flag; any other publication
    /// or a fresh session leaves it unknown, so the base preflight still runs
    /// and keeps its exact error.
    preflight_succeeded: bool,
}

impl ResidentWorkspaceSession {
    /// Start a resident occurrence at its initial internal revision.
    #[must_use]
    pub fn new(document_scope: DocumentScopeId, document: Document) -> Self {
        let (retained, calculation_work) = ResidentDerivedState::rebuild(&document);
        Self {
            document_scope,
            occurrence: SessionOccurrence::next(),
            document,
            retained,
            revision: revision_for(0),
            generation: 0,
            measurements: ResidentRuntimeMeasurements::initial(calculation_work),
            preflight_succeeded: false,
        }
    }

    /// Return the current opaque semantic revision token.
    #[must_use]
    pub fn revision(&self) -> &SemanticRevision {
        &self.revision
    }

    /// Borrow this occurrence's protected document scope.
    #[must_use]
    pub fn document_scope(&self) -> &DocumentScopeId {
        &self.document_scope
    }

    /// Borrow the installed document without detaching a snapshot.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Issue the identity proof for this occurrence's current document.
    ///
    /// The proof borrows the document and names the occurrence and revision
    /// that issued it, so a lifecycle call can bind a memo base to this exact
    /// document without copying it.
    #[must_use]
    pub fn witness(&self) -> DocumentWitness<'_> {
        DocumentWitness {
            occurrence: &self.occurrence,
            revision: &self.revision,
            document: &self.document,
        }
    }

    /// Borrow this session's occurrence for a test's explicit witness.
    #[cfg(test)]
    pub(crate) const fn occurrence(&self) -> &SessionOccurrence {
        &self.occurrence
    }

    /// Borrow the current core direct-dependency facts in stable-ID order.
    ///
    /// Entries describe formulas, including failed calculations. Hosts must
    /// resolve query targets and enforce disclosure before projecting these
    /// trusted facts, just as for the other resident queries.
    #[must_use]
    pub fn direct_field_dependencies(&self, field: &FieldRef) -> Option<BTreeSet<FieldRef>> {
        self.retained.calculation_state.dependencies_of(field)
    }

    /// Return the formulas whose expanded dependencies contain `field`.
    #[must_use]
    pub fn direct_field_dependents(&self, field: &FieldRef) -> BTreeSet<FieldRef> {
        self.retained.calculation_state.direct_dependents(field)
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

    /// Fold each Number column over `members`, in the given order.
    ///
    /// `members` are collection entities in collection (stable-ID) order and
    /// `fields` are that collection's Number field ids. Every cell is
    /// classified exactly as [`Self::query_fields`] projects it: a calculated
    /// value or a stored Number adds, a failed or unavailable calculation or
    /// another stored value makes its column `None`, and a field the entity
    /// does not hold is skipped. Additions follow member order, so each total
    /// is bit-identical to summing the projected fields in that order.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::MissingEntityId`] for a member absent from
    /// the resident revision, matching [`Self::query_entities`].
    pub fn query_column_totals(
        &self,
        members: &[EntityId],
        fields: &[FieldId],
    ) -> Result<ResidentQueryResult<Vec<ResidentColumnTotal>>, WorkspaceError> {
        // `(sum, complete)`, folded like the runtime footer: `complete` turns
        // off on the first cell that is not a usable Number.
        let mut fold = vec![(0.0_f64, true); fields.len()];
        for id in members {
            let entity = self
                .document
                .entities
                .get(id)
                .ok_or_else(|| WorkspaceError::MissingEntityId { entity: id.clone() })?;
            for (position, field) in fields.iter().enumerate() {
                let Some(value) = entity.fields.get(field) else {
                    continue;
                };
                match value {
                    Value::Formula(_) => match retained_calculation_for(
                        &self.retained.calculation_state,
                        &FieldRef::new(id.clone(), field.clone()),
                    ) {
                        FormulaCalculationOutcome::Value(value) => {
                            fold[position].0 += value.get();
                        }
                        FormulaCalculationOutcome::Failure(_)
                        | FormulaCalculationOutcome::Unavailable => fold[position].1 = false,
                    },
                    Value::Number(value) => fold[position].0 += value.get(),
                    Value::Text(_) | Value::Boolean(_) | Value::Date(_) | Value::Reference(_) => {
                        fold[position].1 = false;
                    }
                }
            }
        }
        Ok(ResidentQueryResult {
            document_scope: self.document_scope.clone(),
            revision: self.revision.clone(),
            value: fields
                .iter()
                .zip(fold)
                .map(|(field, (sum, complete))| ResidentColumnTotal {
                    field: field.clone(),
                    total: complete.then_some(sum).filter(|sum| sum.is_finite()),
                })
                .collect(),
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

    /// Evaluate one candidate against this revision for later proposal reuse
    /// and publication.
    ///
    /// The candidate's report and calculation are derived from this revision's
    /// retained state through the same projection changes publication uses, so
    /// a lifecycle proposal can reuse both instead of recalculating either
    /// document. This revision's complete calculation is lent to the result as
    /// the base. A failed retained outcome contributes no base calculation and
    /// leaves the proposal on its existing recalculation path.
    ///
    /// The prepared derived state, the projection changes and the derived-state
    /// transition are kept instead of discarded; [`PreparedEvaluation`] can hand
    /// them to publication as a [`PreparedInstallation`].
    #[must_use]
    pub fn prepare_evaluation(&self, candidate: Document) -> PreparedEvaluation<'_> {
        let base_calculation = {
            let _phase = instrumentation::PhaseTimer::new("validate/prepare/base_outcome");
            self.retained.calculation_state.complete()
        };
        let base_report_valid = self.retained.validation_report.is_valid();
        let mut changes = {
            let _phase = instrumentation::PhaseTimer::new("validate/prepare/projection_changes");
            projection_changes(&self.document, &candidate)
        };
        // The prepared path evaluates the candidate exactly once, as it did
        // through `semantic_validation`; the update is that one evaluation. It
        // rebuilds on its own whenever an incremental update cannot express the
        // change.
        #[cfg(test)]
        super::evaluation_counters::record_candidate_calculation();
        let mut retained = {
            let _phase = instrumentation::PhaseTimer::new("validate/prepare/retained_clone");
            self.retained.clone()
        };
        let before_diagnostics = std::mem::take(&mut retained.diagnostics_by_field);
        let transition = {
            let _phase = instrumentation::PhaseTimer::new("validate/prepare/retained_update");
            retained.update(&self.document, &candidate, &changes)
        };
        // Publication folds the diagnostics projection changes into the same
        // changes; prepare has the same before and after state, so it does too.
        {
            let _phase = instrumentation::PhaseTimer::new("validate/prepare/diagnostics_changes");
            collect_diagnostic_projection_changes(
                &self.document,
                &before_diagnostics,
                &candidate,
                &retained.diagnostics_by_field,
                &mut changes,
            );
        }
        let candidate_preflight_succeeded = {
            let _phase = instrumentation::PhaseTimer::new("validate/prepare/preflight_decision");
            candidate_preflight_succeeded(
                &self.document,
                &candidate,
                &changes,
                self.preflight_succeeded,
            )
        };
        PreparedEvaluation::new(
            PreparedInstallation {
                base_occurrence: self.occurrence.0,
                base_revision: self.revision.clone(),
                candidate,
                retained,
                changes,
                transition,
                candidate_preflight_succeeded,
            },
            &self.document,
            base_calculation,
            base_report_valid,
            self.preflight_succeeded,
        )
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
            prepared: None,
            projection_invalidation: None,
        }
    }
}

/// Decide whether `candidate` passes the formula projection preflight without
/// re-projecting what the base already proved.
///
/// Preflight reads keys, schema definitions and formula expressions, never
/// values. An edit that leaves the address index and every schema definition
/// alone can therefore change the outcome only through a changed formula:
/// when there is none, the base's known-success outcome is the candidate's;
/// when there is one, only those formulas need projecting. Every other change,
/// and an unknown base, runs the full preflight so the outcome is exact.
fn candidate_preflight_succeeded(
    base: &Document,
    candidate: &Document,
    changes: &ProjectionChanges,
    base_preflight_succeeded: bool,
) -> bool {
    if !base_preflight_succeeded
        || changes.address_index_changed
        || changes.unaddressable_calculation_change
    {
        return super::preflight_formula_projections(candidate).is_ok();
    }
    let changed_formula = changes.fields.iter().any(|field| {
        super::field_holds_formula(base, field) || super::field_holds_formula(candidate, field)
    });
    if !changed_formula {
        return true;
    }
    match super::preflight_changed_formula_projections(candidate, &changes.fields) {
        Ok(()) => true,
        // A partial failure cannot prove it is the full run's first error, so
        // the full preflight decides the outcome and keeps that error.
        Err(_) => super::preflight_formula_projections(candidate).is_ok(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResidentDerivedState {
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
        before: &Document,
        document: &Document,
        changes: &ProjectionChanges,
    ) -> DerivedStateTransition {
        let previous_address_index_valid = self.address_index.is_ok();
        if changes.address_index_changed {
            let _phase = instrumentation::PhaseTimer::new("rebuild/address_index");
            self.address_index = AddressIndex::build(document);
        }
        let address_index_safe = previous_address_index_valid && self.address_index.is_ok();
        // A structural change no field root can describe keeps the retained
        // state equal to a full rebuild by taking that branch. It isn't an
        // address-index fallback, so it isn't counted as one.
        let incremental_safe = address_index_safe && !changes.unaddressable_calculation_change;
        let calculation = {
            let _phase = instrumentation::PhaseTimer::new("retained_update/calculation_update");
            if incremental_safe {
                self.calculation_state
                    .update(document, &changes.calculation_roots)
            } else {
                let before = {
                    let _phase = instrumentation::PhaseTimer::new("rebuild/index_before");
                    self.calculation_state.outcome()
                };
                let old_affected = {
                    let _phase = instrumentation::PhaseTimer::new("rebuild/index_before");
                    self.calculation_state
                        .affected_by_all(&changes.calculation_roots)
                };
                let (rebuilt, work) = {
                    let _phase = instrumentation::PhaseTimer::new("rebuild/state");
                    RetainedCalculationState::rebuild(document)
                };
                let new_affected = {
                    let _phase = instrumentation::PhaseTimer::new("rebuild/index_after");
                    rebuilt.affected_by_all(&changes.calculation_roots)
                };
                let after = {
                    let _phase = instrumentation::PhaseTimer::new("rebuild/index_after");
                    rebuilt.outcome()
                };
                self.calculation_state = rebuilt;
                let changed_calculation_projections = {
                    let _phase = instrumentation::PhaseTimer::new("rebuild/index_after");
                    changed_calculation_outcomes(&before, &after)
                };
                IncrementalCalculationTransition {
                    work,
                    affected_calculations: old_affected
                        .into_iter()
                        .chain(new_affected)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    changed_calculation_projections,
                }
            }
        };
        self.validation_report = {
            let _phase = instrumentation::PhaseTimer::new("retained_update/validation_report");
            self.validation_report_for_update(
                before,
                document,
                changes,
                incremental_safe,
                &calculation,
            )
        };
        self.diagnostics_by_field = index_field_diagnostics(&self.validation_report);
        DerivedStateTransition {
            work: DerivedStateTransitionWork {
                calculation: calculation.work,
                address_index_rebuilt: changes.address_index_changed,
                fell_back_to_full_calculation: !address_index_safe,
            },
            affected_calculations: calculation.affected_calculations,
            changed_calculation_projections: calculation.changed_calculation_projections,
        }
    }

    /// Decide the candidate's report, validating only the fields the update
    /// touched when no whole-document check can have moved.
    fn validation_report_for_update(
        &self,
        before: &Document,
        document: &Document,
        changes: &ProjectionChanges,
        incremental_safe: bool,
        calculation: &IncrementalCalculationTransition,
    ) -> ValidationReport {
        let local = locally_validated_report(
            self.validation_report.is_valid(),
            before,
            document,
            changes,
            incremental_safe,
            &self.calculation_state,
            calculation,
        );
        if let Some(report) = local {
            #[cfg(test)]
            local_validation_counters::record_local();
            report
        } else {
            #[cfg(test)]
            local_validation_counters::record_full();
            validation_report_for_retained_calculation(document, &self.calculation_state)
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

    /// The candidate's validation report, as full semantic validation gives it.
    pub(crate) fn report(&self) -> &ValidationReport {
        &self.validation_report
    }

    /// The candidate's complete calculation, exactly when the report is valid.
    pub(crate) fn calculation(&self) -> Option<&Calculation> {
        self.calculation_state
            .complete()
            .filter(|_| self.report().is_valid())
    }

    fn diagnostics_for_field(&self, field: &FieldRef) -> Vec<Diagnostic> {
        self.diagnostics_by_field
            .get(field)
            .cloned()
            .unwrap_or_default()
    }
}

/// The candidate's report built from the changed fields alone, or `None` when
/// any check that could change is not local.
///
/// Whole-document validation splits into checks a value edit cannot move and
/// checks local to one field or formula. Stable ids, human keys, duplicate
/// keys, display names, field order and schema declarations read schema and
/// entity identity, and [`ProjectionChanges`] names every structural edit that
/// could touch them: an entity added, removed, moved, re-keyed or renamed sets
/// `address_index_changed`, and a schema or field definition edit sets
/// `unaddressable_calculation_change`, `schemas` or `schema_fields`. An edit
/// that leaves all of those alone cannot change any of those checks.
///
/// What remains is decided by one field alone: its declared type, stored value
/// constraint, reference target, declaration and a required field's presence.
/// Formula failures are not local — their diagnostics depend on the whole
/// failure graph — so a candidate whose calculation failed falls back. Formula
/// value constraints are local to formulas whose calculated value can change,
/// which the calculation transition names.
///
/// The fast path returns only the empty report: a local diagnostic falls back
/// to whole-document validation, which reports it with the rest.
fn locally_validated_report(
    base_report_valid: bool,
    before: &Document,
    after: &Document,
    changes: &ProjectionChanges,
    incremental_safe: bool,
    calculation: &RetainedCalculationState,
    transition: &IncrementalCalculationTransition,
) -> Option<ValidationReport> {
    if !base_report_valid
        || !incremental_safe
        || changes.address_index_changed
        || !changes.schemas.is_empty()
        || !changes.schema_fields.is_empty()
        || before.id != after.id
        || before.title != after.title
    {
        return None;
    }
    if changes
        .fields
        .iter()
        .any(|field| !entity_keeps_its_schema(before, after, &field.entity))
    {
        return None;
    }
    // A failed candidate's formula diagnostics span the failure graph, which
    // whole-document validation alone resolves, so fall back to it.
    let values = calculation.complete_values()?;
    let mut local = Vec::new();
    for field in &changes.fields {
        local.extend(validate_entity_field_core(after, field));
    }
    for field in changes
        .fields
        .iter()
        .chain(&transition.affected_calculations)
        .chain(&transition.changed_calculation_projections)
    {
        local.extend(validate_formula_value_constraint(after, field, values));
    }
    local.is_empty().then(|| ValidationReport::new(Vec::new()))
}

/// Whether a changed field's entity keeps its schema across the edit. Every
/// changed field must, or its values and its whole-document checks may not be
/// comparable to the base's.
fn entity_keeps_its_schema(before: &Document, after: &Document, entity: &EntityId) -> bool {
    match (before.entities.get(entity), after.entities.get(entity)) {
        (Some(before), Some(after)) => before.schema == after.schema,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DerivedStateTransitionWork {
    calculation: IncrementalCalculationWork,
    address_index_rebuilt: bool,
    fell_back_to_full_calculation: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DerivedStateTransition {
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

/// One Number column's footer total over a set of members.
///
/// `None` means the column cannot be totaled: one of its cells is a failed or
/// unavailable calculation, another stored value, or the sum is not finite.
/// A field no member holds totals `Some(0.0)`.
#[derive(Clone, Debug, PartialEq)]
pub struct ResidentColumnTotal {
    pub field: FieldId,
    pub total: Option<f64>,
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
    /// Schema presentation targets, independent of resident entities.
    pub schemas: Vec<SchemaId>,
    /// Schema-field presentation targets in stable (`SchemaId`, `FieldId`) order.
    pub schema_fields: Vec<(SchemaId, FieldId)>,
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
    prepared: Option<PreparedInstallation>,
    projection_invalidation: Option<ResidentProjectionInvalidation>,
}

impl<Time> ResidentPublicationAuthority<'_, Time> {
    /// Offer an evaluation prepared against a candidate so publication can
    /// install its derived state instead of deriving it a second time.
    ///
    /// The installation is only used when it was prepared by this session
    /// occurrence, against the expected revision, and for exactly the candidate
    /// being published; otherwise publication takes its existing path. A
    /// refused or failed publication drops it.
    #[must_use]
    pub fn with_prepared(mut self, installation: PreparedInstallation) -> Self {
        self.prepared = Some(installation);
        self
    }

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
    fn current_snapshot(&self) -> (DocumentScopeId, &Document, SemanticRevision) {
        (
            self.session.document_scope.clone(),
            &self.session.document,
            self.session.revision.clone(),
        )
    }

    fn current_snapshot_witness(&self) -> Option<DocumentWitness<'_>> {
        Some(self.session.witness())
    }

    fn publish_if_current<Authorization>(
        &mut self,
        expected_document_scope: &DocumentScopeId,
        expected_revision: &SemanticRevision,
        candidate: &Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<Authorization>,
    ) -> Result<
        (DocumentScopeId, &Document, SemanticRevision, Authorization),
        SemanticPublicationError,
    > {
        let _phase = instrumentation::PhaseTimer::new("publish_if_current");
        // Any installation belongs to this one attempt: it is used or dropped,
        // and never carried into a later compare-and-publish call.
        let prepared = self.prepared.take();
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
        let unchanged = {
            let _phase = instrumentation::PhaseTimer::new("publish_if_current/equality");
            instrumentation::record_publication_document_comparison();
            candidate == &self.session.document
        };
        if unchanged {
            return Err(SemanticPublicationError::NoChange);
        }
        let next_generation = self
            .session
            .generation
            .checked_add(1)
            .ok_or(SemanticPublicationError::Conflict)?;
        let resulting_revision = revision_for(next_generation);
        let installs_prepared = prepared.as_ref().is_some_and(|installation| {
            let _phase =
                instrumentation::PhaseTimer::new("publish_if_current/installation_equality");
            installation.base_occurrence == self.session.occurrence.0
                && installation.base_revision == *expected_revision
                && installation.candidate == *candidate
        });
        let (projection_invalidation, transition_work) = match prepared {
            // The installation is exactly this publication: prepared by this
            // occurrence, from the same base revision and for the same candidate
            // document. Revisions repeat across sessions, so the occurrence is
            // part of the identity. Installing its derived state gives what the
            // unassisted path derives below, without deriving it twice.
            Some(installation) if installs_prepared => {
                let _phase =
                    instrumentation::PhaseTimer::new("publish_if_current/install_prepared");
                let (invalidation, transition_work) = {
                    let _phase = instrumentation::PhaseTimer::new("install_prepared/invalidation");
                    invalidation_from_transition(
                        installation.changes,
                        installation.transition,
                        self.session.document_scope.clone(),
                        expected_revision.clone(),
                        resulting_revision.clone(),
                    )
                };
                self.session.retained = installation.retained;
                self.session.document = installation.candidate;
                self.session.preflight_succeeded = installation.candidate_preflight_succeeded;
                #[cfg(test)]
                installation_counters::record_prepared_installation();
                (invalidation, transition_work)
            }
            _ => {
                let (invalidation, transition_work) = {
                    let _phase = instrumentation::PhaseTimer::new(
                        "publish_if_current/projection_invalidation",
                    );
                    projection_invalidation(
                        &self.session.document,
                        &mut self.session.retained,
                        candidate,
                        self.session.document_scope.clone(),
                        expected_revision.clone(),
                        resulting_revision.clone(),
                    )
                };
                self.session.document = {
                    let _phase = instrumentation::PhaseTimer::new(
                        "publish_if_current/fallback_install_clone",
                    );
                    candidate.clone()
                };
                // This path never ran the candidate's preflight, so its
                // outcome is unknown and the next base must preflight again.
                self.session.preflight_succeeded = false;
                (invalidation, transition_work)
            }
        };

        self.session.generation = next_generation;
        self.session.revision = resulting_revision.clone();
        self.session.measurements.record_transition(transition_work);
        self.projection_invalidation = Some(projection_invalidation);

        Ok((
            self.session.document_scope.clone(),
            &self.session.document,
            resulting_revision,
            authorization,
        ))
    }
}

fn revision_for(generation: u64) -> SemanticRevision {
    SemanticRevision::from(format!("resident/{generation}"))
}

/// Test-only count of publications that installed a prepared installation.
/// It proves the differential publication tests exercise both paths, not just
/// the fallback.
#[cfg(test)]
pub(crate) mod installation_counters {
    use std::cell::Cell;

    thread_local! {
        static PREPARED_INSTALLATIONS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record_prepared_installation() {
        PREPARED_INSTALLATIONS.with(|count| count.set(count.get() + 1));
    }

    pub(crate) fn prepared_installations() -> usize {
        PREPARED_INSTALLATIONS.with(Cell::get)
    }
}

/// Test-only counts of updates that built their report from the changed fields
/// and of those that fell back to whole-document validation. They prove the
/// differential tests exercise both paths.
#[cfg(test)]
pub(crate) mod local_validation_counters {
    use std::cell::Cell;

    thread_local! {
        static LOCAL_REPORTS: Cell<usize> = const { Cell::new(0) };
        static FULL_REPORTS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn reset() {
        LOCAL_REPORTS.with(|count| count.set(0));
        FULL_REPORTS.with(|count| count.set(0));
    }

    pub(crate) fn record_local() {
        LOCAL_REPORTS.with(|count| count.set(count.get() + 1));
    }

    pub(crate) fn record_full() {
        FULL_REPORTS.with(|count| count.set(count.get() + 1));
    }

    /// Return `(local reports, full reports)` since the last [`reset`].
    pub(crate) fn counts() -> (usize, usize) {
        (LOCAL_REPORTS.with(Cell::get), FULL_REPORTS.with(Cell::get))
    }
}

fn projection_invalidation(
    before: &Document,
    retained: &mut ResidentDerivedState,
    after: &Document,
    document_scope: DocumentScopeId,
    base_revision: SemanticRevision,
    resulting_revision: SemanticRevision,
) -> (ResidentProjectionInvalidation, DerivedStateTransitionWork) {
    let mut changes = projection_changes(before, after);
    let before_diagnostics = std::mem::take(&mut retained.diagnostics_by_field);
    let transition = {
        let _phase = instrumentation::PhaseTimer::new("projection_invalidation/retained_update");
        retained.update(before, after, &changes)
    };
    collect_diagnostic_projection_changes(
        before,
        &before_diagnostics,
        after,
        &retained.diagnostics_by_field,
        &mut changes,
    );

    invalidation_from_transition(
        changes,
        transition,
        document_scope,
        base_revision,
        resulting_revision,
    )
}

/// Build one projection invalidation from already-computed projection changes
/// and a derived-state transition.
///
/// Publication and a prepared installation share this construction so the two
/// paths cannot drift.
fn invalidation_from_transition(
    changes: ProjectionChanges,
    transition: DerivedStateTransition,
    document_scope: DocumentScopeId,
    base_revision: SemanticRevision,
    resulting_revision: SemanticRevision,
) -> (ResidentProjectionInvalidation, DerivedStateTransitionWork) {
    let affected_calculations = {
        let _phase = instrumentation::PhaseTimer::new("invalidation/affected_calculations");
        transition
            .affected_calculations
            .into_iter()
            .chain(transition.changed_calculation_projections)
            .collect::<BTreeSet<_>>()
    };

    (
        ResidentProjectionInvalidation {
            document_scope,
            base_revision,
            resulting_revision,
            schemas: changes.schemas.into_iter().collect(),
            schema_fields: changes.schema_fields.into_iter().collect(),
            entities: changes.entities.into_iter().collect(),
            fields: changes.fields.into_iter().collect(),
            affected_calculations: affected_calculations.into_iter().collect(),
        },
        transition.work,
    )
}

/// Every projection change between two documents, collected the same way for
/// publication and for evaluation preparation.
fn projection_changes(before: &Document, after: &Document) -> ProjectionChanges {
    let entity_ids = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .collect::<BTreeSet<_>>();
    let mut changes = ProjectionChanges::default();
    collect_entity_projection_changes(before, after, &entity_ids, &mut changes);
    collect_schema_projection_changes(before, after, &entity_ids, &mut changes);
    changes
}

#[derive(Debug, Default)]
pub(crate) struct ProjectionChanges {
    schemas: BTreeSet<SchemaId>,
    schema_fields: BTreeSet<(SchemaId, FieldId)>,
    entities: BTreeSet<EntityId>,
    fields: BTreeSet<FieldRef>,
    calculation_roots: BTreeSet<FieldRef>,
    address_index_changed: bool,
    /// A structural change that no field root can fully describe: an entity
    /// added, removed, or moved to another schema, a field's semantic
    /// definition changed, or a schema added or removed. Roots come from the populated fields an entity holds, but a
    /// formula may read one of its blank fields, and the retained graph also
    /// tracks blank column slots and a schema map that no field root expresses;
    /// updating from roots alone would leave state that differs from a full
    /// rebuild. These are rare structural edits, so a full rebuild is
    /// acceptable.
    unaddressable_calculation_change: bool,
}

impl ProjectionChanges {
    /// The ids a scoped diff has to compare: every schema whose definition may
    /// differ, including one a changed field definition names, and every entity
    /// whose definition or values may differ.
    pub(crate) fn diff_scope(&self) -> DiffScope {
        let mut schemas = self.schemas.clone();
        schemas.extend(self.schema_fields.iter().map(|(schema, _)| schema.clone()));
        let mut entities = self.entities.clone();
        entities.extend(self.fields.iter().map(|field| field.entity.clone()));
        DiffScope { schemas, entities }
    }
}

fn collect_entity_projection_changes(
    before: &Document,
    after: &Document,
    entity_ids: &BTreeSet<&EntityId>,
    changes: &mut ProjectionChanges,
) {
    instrumentation::record_publication_entities_inspected(
        u64::try_from(entity_ids.len()).unwrap_or(u64::MAX),
    );
    for entity_id in entity_ids {
        let entity_id = *entity_id;
        match (
            before.entities.get(entity_id),
            after.entities.get(entity_id),
        ) {
            (Some(before_entity), Some(after_entity)) => {
                if before_entity == after_entity {
                    continue;
                }
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
                    // The roots above name only the fields the entity holds a
                    // value for, but another entity's formula may read one of
                    // its blank fields, and the graph's blank slots and schema
                    // entry have no root at all.
                    changes.unaddressable_calculation_change = true;
                }

                instrumentation::record_publication_fields_inspected(
                    u64::try_from(field_ids.len()).unwrap_or(u64::MAX),
                );
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
                // As above: only the populated fields become roots, while a
                // formula may read a blank one and the graph's blank slots and
                // schema entry have no root.
                changes.unaddressable_calculation_change = true;
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
    entity_ids: &BTreeSet<&EntityId>,
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
        // A schema added or removed changes the graph's schema map and its
        // read columns without any entity field root to name it, even when no
        // entity holds a field of that schema.
        if before_schema.is_some() != after_schema.is_some() {
            changes.unaddressable_calculation_change = true;
        }
        if before_schema.map(|schema| (&schema.id, &schema.key))
            != after_schema.map(|schema| (&schema.id, &schema.key))
        {
            changes.address_index_changed = true;
        }
        // The column order (R4a) is schema presentation too.
        if before_schema.map(|schema| {
            (
                &schema.id,
                &schema.key,
                &schema.display_name,
                &schema.field_order,
            )
        }) != after_schema.map(|schema| {
            (
                &schema.id,
                &schema.key,
                &schema.display_name,
                &schema.field_order,
            )
        }) {
            changes.schemas.insert(schema_id.clone());
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
            instrumentation::record_publication_fields_inspected(1);
            let before_definition = before_schema.and_then(|schema| schema.fields.get(&field_id));
            let after_definition = after_schema.and_then(|schema| schema.fields.get(&field_id));
            if before_definition == after_definition {
                continue;
            }
            changes
                .schema_fields
                .insert((schema_id.clone(), field_id.clone()));
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
                let entity_id = *entity_id;
                instrumentation::record_publication_fields_inspected(1);
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
            // A definition change reaches every reader of the field, including
            // readers of entities that leave it blank, which have no field root,
            // and the retained graph's column slots, schema map and validation
            // depend on it too. These are rare structural edits, so a full
            // rebuild is acceptable.
            if semantic_definition_changed {
                changes.unaddressable_calculation_change = true;
            }
        }
    }
}

fn collect_diagnostic_projection_changes(
    before: &Document,
    before_diagnostics: &BTreeMap<FieldRef, Vec<Diagnostic>>,
    after: &Document,
    after_diagnostics: &BTreeMap<FieldRef, Vec<Diagnostic>>,
    changes: &mut ProjectionChanges,
) {
    if before_diagnostics == after_diagnostics {
        return;
    }
    let field_refs = before_diagnostics
        .keys()
        .chain(after_diagnostics.keys())
        .collect::<BTreeSet<_>>();
    instrumentation::record_publication_fields_inspected(
        u64::try_from(field_refs.len()).unwrap_or(u64::MAX),
    );
    for field in field_refs {
        if before_diagnostics.get(field) == after_diagnostics.get(field) {
            continue;
        }
        let present = before
            .entities
            .get(&field.entity)
            .is_some_and(|entity| entity.fields.contains_key(&field.field))
            || after
                .entities
                .get(&field.entity)
                .is_some_and(|entity| entity.fields.contains_key(&field.field));
        if present {
            changes.fields.insert(field.clone());
        }
    }
}

fn changed_calculation_outcomes(
    before: &CalculationOutcome,
    after: &CalculationOutcome,
) -> Vec<FieldRef> {
    calculation_formula_keys(before)
        .into_iter()
        .chain(calculation_formula_keys(after))
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
#[path = "projection_diff_tests.rs"]
mod projection_diff_tests;

#[cfg(test)]
#[path = "incremental_preparation_tests.rs"]
mod incremental_preparation_tests;

#[cfg(test)]
#[path = "local_validation_tests.rs"]
mod local_validation_tests;

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedTime;

    impl TrustedPublicationTimeSource for FixedTime {
        fn now(&mut self) -> TrustedInstant {
            TrustedInstant::new(1)
        }
    }

    fn assert_empty_table_name_invalidation(schema_name: bool) {
        let mut before = Document::empty("document", "Names");
        let field = tachiko_semantic_core::FieldDefinition {
            id: "f".into(),
            key: "item".into(),
            display_name: None,
            number_format: None,
            field_type: tachiko_semantic_core::FieldType::Text,
            required: false,
            constraint: tachiko_semantic_core::FieldConstraint::None,
        };
        let schema = tachiko_semantic_core::Schema {
            field_order: None,
            id: "s".into(),
            key: "table".into(),
            display_name: None,
            fields: BTreeMap::from([(field.id.clone(), field)]),
        };
        before.schemas.insert(schema.id.clone(), schema);
        let mut after = before.clone();
        let schema = after.schemas.get_mut("s").unwrap();
        if schema_name {
            schema.display_name = Some("Table Name".into());
        } else {
            schema.fields.get_mut("f").unwrap().display_name = Some("Item Name".into());
        }
        let mut session = ResidentWorkspaceSession::new("occurrence".into(), before.clone());
        let mut time = FixedTime;
        let snapshot = session.export_snapshot();
        let mut publication = session.publication_authority(&mut time);
        publication
            .publish_if_current(
                snapshot.document_scope(),
                snapshot.revision(),
                &after,
                |_| Some(()),
            )
            .unwrap();
        let actual = publication.projection_invalidation.as_ref().unwrap();
        // Compare against the full no-change invalidation, including any independent
        // schema targets: entity-only lists cannot distinguish these empty tables.
        let mut control = ResidentWorkspaceSession::new("occurrence".into(), before.clone());
        let (unchanged, _) = projection_invalidation(
            &before,
            &mut control.retained,
            &before,
            actual.document_scope.clone(),
            actual.base_revision.clone(),
            actual.resulting_revision.clone(),
        );
        assert_ne!(
            actual, &unchanged,
            "label-only publication must invalidate presentation"
        );
        assert_eq!(
            actual.schemas,
            if schema_name {
                vec!["s".into()]
            } else {
                vec![]
            }
        );
        assert_eq!(
            actual.schema_fields,
            if schema_name {
                vec![]
            } else {
                vec![("s".into(), "f".into())]
            }
        );
        assert_eq!(actual.entities, []);
        assert_eq!(actual.fields, []);
        assert_eq!(actual.affected_calculations, []);
    }

    #[test]
    fn empty_table_column_name_invalidates_presentation() {
        assert_empty_table_name_invalidation(false);
    }

    #[test]
    fn schema_label_only_invalidates_presentation() {
        assert_empty_table_name_invalidation(true);
    }

    #[test]
    fn no_change_clears_prior_invalidation_without_advancing_or_updating_derived_state() {
        let mut session = ResidentWorkspaceSession::new(
            DocumentScopeId::from("occurrence"),
            Document::empty("document", "Before"),
        );
        let before = session.export_snapshot();
        let mut candidate = before.document().clone();
        candidate.title = "After".to_owned();
        let mut time = FixedTime;
        let mut publication = session.publication_authority(&mut time);
        let (scope, installed, revision, ()) = publication
            .publish_if_current(
                before.document_scope(),
                before.revision(),
                &candidate,
                |_| Some(()),
            )
            .unwrap();
        let installed = Document::clone(installed);
        assert!(publication.projection_invalidation.is_some());
        let measurements = publication.session.measurements;
        let result = publication.publish_if_current(&scope, &revision, &installed, |_| Some(()));
        assert_eq!(result, Err(SemanticPublicationError::NoChange));
        assert!(publication.projection_invalidation.is_none());
        assert_eq!(publication.session.document, installed);
        assert_eq!(publication.session.revision, revision);
        assert_eq!(publication.session.measurements, measurements);
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
                &candidate,
                |_| Some(()),
            )
            .unwrap_err();

        assert_eq!(error, SemanticPublicationError::Conflict);
        assert_eq!(session.export_snapshot(), before);
    }
}
