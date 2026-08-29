//! Provisional resident semantic state for first-party interactive hosts.
//!
//! These Rust types are workspace-internal implementation details. They do not
//! define a public session, revision, result, serialization, or transport
//! contract.

use std::collections::{BTreeMap, BTreeSet};

use tachiko_formula_engine::{CalculationOutcome, calculate_complete};

use super::{
    AddressIndex, CalculatedField, Diagnostic, Document, EntityId, EntityInspection, Expression,
    FieldAddress, FieldRef, SemanticSubject, ValidationReport, Value, WorkspaceError,
    calculate_fields,
    formula_operations::FormulaCalculationOutcome,
    patch_lifecycle::{
        DocumentScopeId, SemanticPublicationAuthority, SemanticPublicationError, SemanticRevision,
        TrustedInstant,
    },
    validation_report,
};

/// One Rust-authoritative semantic document occurrence retained across calls.
pub struct ResidentWorkspaceSession {
    document_scope: DocumentScopeId,
    document: Document,
    revision: SemanticRevision,
    generation: u64,
}

impl ResidentWorkspaceSession {
    /// Start a resident occurrence at its initial internal revision.
    #[must_use]
    pub fn new(document_scope: DocumentScopeId, document: Document) -> Self {
        Self {
            document_scope,
            document,
            revision: revision_for(0),
            generation: 0,
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
            revision: self.revision.clone(),
            value: validation_report(&self.document),
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
            revision: self.revision.clone(),
            value: calculate_fields(&self.document)?,
        })
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
    /// Returns the existing typed field lookup or address-projection failure.
    pub fn query_fields(
        &self,
        requested: &[FieldRef],
    ) -> Result<ResidentQueryResult<Vec<ResidentFieldProjection>>, WorkspaceError> {
        let requested = requested.iter().cloned().collect::<BTreeSet<_>>();
        let calculation = calculate_complete(&self.document);
        let report = validation_report(&self.document);
        let addresses = AddressIndex::build(&self.document)?;
        let fields = requested
            .into_iter()
            .map(|field| {
                let value = self
                    .document
                    .entities
                    .get(&field.entity)
                    .and_then(|entity| entity.fields.get(&field.field))
                    .ok_or_else(|| WorkspaceError::MissingField {
                        field: field.clone(),
                    })?;
                let presentation_address = addresses.field_address(&self.document, &field)?;
                let subject = SemanticSubject::EntityField(field.clone());
                let diagnostics = report
                    .diagnostics()
                    .iter()
                    .filter(|diagnostic| diagnostic.subjects.contains(&subject))
                    .cloned()
                    .collect();
                let (stored_value, formula_definition, calculated_value) = match value {
                    Value::Formula(expression) => (
                        None,
                        Some(expression.clone()),
                        Some(formula_calculation(&calculation, &field)),
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
            revision: self.revision.clone(),
            value: fields,
        })
    }

    /// Derive revision-tagged projection invalidation from changed stable fields.
    ///
    /// This performs a fresh full-oracle dependency extraction and retains no
    /// engine state. A client may pair the result with an execution receipt
    /// only when both identify the same resulting revision.
    #[must_use]
    pub fn projection_invalidation(
        &self,
        changed: &[FieldRef],
    ) -> ResidentQueryResult<ResidentProjectionInvalidation> {
        let changed = changed.iter().cloned().collect::<BTreeSet<_>>();
        let calculation = calculate_complete(&self.document);
        let dependencies = match &calculation {
            CalculationOutcome::Complete(calculation) => calculation.dependencies(),
            CalculationOutcome::Failed(failures) => failures.dependencies(),
        };
        let affected_calculations = affected_by(dependencies, &changed);

        ResidentQueryResult {
            revision: self.revision.clone(),
            value: ResidentProjectionInvalidation {
                changed_fields: changed.into_iter().collect(),
                affected_calculations,
            },
        }
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
        }
    }
}

/// One query observation pinned to the resident revision it read.
#[derive(Clone, Debug, PartialEq)]
pub struct ResidentQueryResult<T> {
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

/// Stable subjects whose cached projections are stale at one resident revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentProjectionInvalidation {
    pub changed_fields: Vec<FieldRef>,
    pub affected_calculations: Vec<FieldRef>,
}

impl<T> ResidentQueryResult<T> {
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

    /// Report whether this detached query observation predates a revision.
    #[must_use]
    pub fn is_stale_against(&self, current: &SemanticRevision) -> bool {
        &self.revision != current
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

        self.session.document = candidate;
        self.session.generation = next_generation;
        self.session.revision = resulting_revision.clone();

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

fn formula_calculation(
    calculation: &CalculationOutcome,
    field: &FieldRef,
) -> FormulaCalculationOutcome {
    match calculation {
        CalculationOutcome::Complete(calculation) => calculation.value(field).map_or(
            FormulaCalculationOutcome::Unavailable,
            FormulaCalculationOutcome::Value,
        ),
        CalculationOutcome::Failed(failures) => failures
            .failures()
            .get(field)
            .map_or(FormulaCalculationOutcome::Unavailable, |failure| {
                FormulaCalculationOutcome::Failure(failure.clone())
            }),
    }
}

fn affected_by(
    dependencies: &BTreeMap<FieldRef, BTreeSet<FieldRef>>,
    changed: &BTreeSet<FieldRef>,
) -> Vec<FieldRef> {
    let mut frontier = changed.clone();
    let mut affected = BTreeSet::new();
    loop {
        let next = dependencies
            .iter()
            .filter(|(formula, inputs)| {
                !affected.contains(*formula) && !inputs.is_disjoint(&frontier)
            })
            .map(|(formula, _)| formula.clone())
            .collect::<BTreeSet<_>>();
        if next.is_empty() {
            break;
        }
        frontier.clone_from(&next);
        affected.extend(next);
    }
    affected.retain(|field| !changed.contains(field));
    affected.into_iter().collect()
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
