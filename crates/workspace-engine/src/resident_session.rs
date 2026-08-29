//! Provisional resident semantic state for first-party interactive hosts.
//!
//! These Rust types are workspace-internal implementation details. They do not
//! define a public session, revision, result, serialization, or transport
//! contract.

use super::{
    CalculatedField, Document, ValidationReport, WorkspaceError, calculate_fields,
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
