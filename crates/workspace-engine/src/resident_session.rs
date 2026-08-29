//! Provisional resident semantic state for first-party interactive hosts.
//!
//! These Rust types are workspace-internal implementation details. They do not
//! define a public session, revision, result, serialization, or transport
//! contract. They are trusted runtime primitives rather than client endpoints;
//! host adapters must enforce ADR-0026 Query authorization before projecting
//! their observations outside the trusted composition boundary.

use std::collections::BTreeSet;

use tachiko_formula_engine::calculate_complete;

use super::{
    CalculatedField, Diagnostic, Document, EntityId, EntityInspection, Expression, FieldAddress,
    FieldRef, SemanticSubject, ValidationReport, Value, WorkspaceError, calculate_fields,
    formula_operations::{
        FormulaCalculationOutcome, affected_by_all, calculation_dependencies, calculation_for,
    },
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
            document_scope: self.document_scope.clone(),
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
            document_scope: self.document_scope.clone(),
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
        let calculation = calculate_complete(&self.document);
        let report = validation_report(&self.document);
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
                        Some(calculation_for(&calculation, &field)),
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
        let projection_invalidation = projection_invalidation(
            &self.session.document,
            &candidate,
            self.session.document_scope.clone(),
            expected_revision.clone(),
            resulting_revision.clone(),
        );

        self.session.document = candidate;
        self.session.generation = next_generation;
        self.session.revision = resulting_revision.clone();
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
    after: &Document,
    document_scope: DocumentScopeId,
    base_revision: SemanticRevision,
    resulting_revision: SemanticRevision,
) -> ResidentProjectionInvalidation {
    let entity_ids = before
        .entities
        .keys()
        .chain(after.entities.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = ProjectionChanges::default();
    collect_entity_projection_changes(before, after, &entity_ids, &mut changes);
    collect_schema_projection_changes(before, after, &entity_ids, &mut changes);

    let before_calculation = calculate_complete(before);
    let after_calculation = calculate_complete(after);
    let affected_calculations = affected_by_all(
        calculation_dependencies(&before_calculation),
        &changes.calculation_roots,
    )
    .into_iter()
    .chain(affected_by_all(
        calculation_dependencies(&after_calculation),
        &changes.calculation_roots,
    ))
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();

    ResidentProjectionInvalidation {
        document_scope,
        base_revision,
        resulting_revision,
        entities: changes.entities.into_iter().collect(),
        fields: changes.fields.into_iter().collect(),
        affected_calculations,
    }
}

#[derive(Default)]
struct ProjectionChanges {
    entities: BTreeSet<EntityId>,
    fields: BTreeSet<FieldRef>,
    calculation_roots: BTreeSet<FieldRef>,
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
