//! Provider-neutral admission boundary for hostile AI-facing requests.
//!
//! This module is a provisional in-process adapter seam, not a public wire
//! protocol or an authentication mechanism. An untrusted request can supply
//! typed semantic intent and non-authoritative evidence only. A trusted host
//! supplies effective identity and time, while `workspace-engine` remains the
//! authoritative proposal, authorization, validation, Approval, publication,
//! and verification boundary.

use tachiko_workspace_engine::{
    Document, ValidationReport,
    patch_lifecycle::{
        ApprovalId, ExecutionReceipt, PatchLifecycle, PatchLifecycleError, PrincipalId, ProposalId,
        ProposalRequest, SemanticPatch, SemanticPatchBody, SemanticPublicationAuthority,
        SemanticRevision, TrustedInstant,
    },
};
use thiserror::Error;

/// Stable symbolic machine classification for an AI-boundary denial.
///
/// The Rust error shape remains provisional. Once published, each symbolic
/// code keeps one meaning and is not silently reused for another failure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AiBoundaryCode(&'static str);

impl AiBoundaryCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for AiBoundaryCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Machine codes emitted by the current provider-facing security boundary.
pub mod boundary_codes {
    use super::AiBoundaryCode;

    pub const TRUSTED_CONTEXT_UNAVAILABLE: AiBoundaryCode =
        AiBoundaryCode("security.trusted_context_unavailable");
    pub const RAW_MUTATION_DENIED: AiBoundaryCode = AiBoundaryCode("security.raw_mutation_denied");
    pub const HOST_EFFECT_DENIED: AiBoundaryCode = AiBoundaryCode("security.host_effect_denied");
    pub const AUTHORIZATION_DENIED: AiBoundaryCode =
        AiBoundaryCode("security.authorization_denied");
    pub const APPROVAL_DENIED: AiBoundaryCode = AiBoundaryCode("security.approval_denied");
    pub const PROPOSAL_REJECTED: AiBoundaryCode = AiBoundaryCode("semantic.proposal_rejected");
    pub const STALE: AiBoundaryCode = AiBoundaryCode("semantic.stale");
    pub const SEMANTIC_GATE_REJECTED: AiBoundaryCode = AiBoundaryCode("semantic.gate_rejected");
    pub const PUBLICATION_CONFLICT: AiBoundaryCode =
        AiBoundaryCode("semantic.publication_conflict");
    pub const VERIFICATION_FAILED: AiBoundaryCode = AiBoundaryCode("semantic.verification_failed");
}

/// Host-proven source class for one item presented to an AI orchestrator.
///
/// These labels control instruction/data treatment only. No variant grants a
/// Principal, capability, Approval, or host effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiContextSource {
    SystemInstruction,
    DeveloperInstruction,
    UserInstruction,
    TrustedSemanticMetadata,
    UntrustedData(UntrustedDataSource),
}

impl AiContextSource {
    #[must_use]
    pub const fn treatment(self) -> AiContextTreatment {
        match self {
            Self::SystemInstruction | Self::DeveloperInstruction | Self::UserInstruction => {
                AiContextTreatment::Instruction
            }
            Self::TrustedSemanticMetadata => AiContextTreatment::TrustedSemanticMetadata,
            Self::UntrustedData(_) => AiContextTreatment::UntrustedData,
        }
    }
}

/// How an orchestrator must treat one host-proven context source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiContextTreatment {
    Instruction,
    TrustedSemanticMetadata,
    UntrustedData,
}

/// Origins that always remain untrusted data at this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UntrustedDataSource {
    DocumentContent,
    ImportedContent,
    PluginResult,
    ModelOutput,
    ClientRequest,
}

/// Non-authoritative content retained as proposal or review evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntrustedData {
    source: UntrustedDataSource,
    content: String,
}

impl UntrustedData {
    #[must_use]
    pub fn new(source: UntrustedDataSource, content: impl Into<String>) -> Self {
        Self {
            source,
            content: content.into(),
        }
    }

    #[must_use]
    pub const fn source(&self) -> UntrustedDataSource {
        self.source
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Raw mutation forms that are never AI semantic operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawMutationKind {
    SemanticState,
    StorageRepresentation,
}

/// Host/external effects kept outside the semantic AI capability domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostEffect {
    DurablePersistence,
    Filesystem,
    Network,
    Process,
    Git,
    Plugin,
    Deployment,
    Credentials,
}

/// Closed provider-facing operation classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiBoundaryOperation {
    SemanticProposal,
    SemanticExecution,
    RawMutation(RawMutationKind),
    HostEffect(HostEffect),
}

/// Disclosure-safe AI-boundary failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AiBoundaryError {
    #[error("trusted request identity or time is unavailable")]
    TrustedContextUnavailable,
    #[error("raw {kind:?} mutation is not an AI semantic operation")]
    RawMutationDenied { kind: RawMutationKind },
    #[error("{effect:?} requires a separate host capability domain")]
    HostEffectDenied { effect: HostEffect },
    #[error("semantic authorization is denied")]
    AuthorizationDenied,
    #[error("exact Human Approval is missing or unusable")]
    ApprovalDenied,
    #[error("the typed semantic proposal was rejected")]
    ProposalRejected,
    #[error("the semantic proposal base is stale")]
    Stale,
    #[error("the authoritative semantic gate rejected the candidate")]
    SemanticGateRejected { report: ValidationReport },
    #[error("semantic publication conflicted before publication")]
    PublicationConflict,
    #[error("post-publication semantic verification failed")]
    VerificationFailed,
}

impl AiBoundaryError {
    #[must_use]
    pub const fn code(&self) -> AiBoundaryCode {
        match self {
            Self::TrustedContextUnavailable => boundary_codes::TRUSTED_CONTEXT_UNAVAILABLE,
            Self::RawMutationDenied { .. } => boundary_codes::RAW_MUTATION_DENIED,
            Self::HostEffectDenied { .. } => boundary_codes::HOST_EFFECT_DENIED,
            Self::AuthorizationDenied => boundary_codes::AUTHORIZATION_DENIED,
            Self::ApprovalDenied => boundary_codes::APPROVAL_DENIED,
            Self::ProposalRejected => boundary_codes::PROPOSAL_REJECTED,
            Self::Stale => boundary_codes::STALE,
            Self::SemanticGateRejected { .. } => boundary_codes::SEMANTIC_GATE_REJECTED,
            Self::PublicationConflict => boundary_codes::PUBLICATION_CONFLICT,
            Self::VerificationFailed => boundary_codes::VERIFICATION_FAILED,
        }
    }

    #[must_use]
    pub const fn validation_report(&self) -> Option<&ValidationReport> {
        match self {
            Self::SemanticGateRejected { report } => Some(report),
            _ => None,
        }
    }
}

/// Check whether a provider-facing operation belongs to the semantic adapter.
///
/// Admission of a typed semantic operation is not authorization. Proposal and
/// execution must still use [`submit_semantic_proposal`] or
/// [`execute_semantic_proposal`], which delegate to the trusted lifecycle.
///
/// # Errors
///
/// Returns a stable raw-mutation or host-effect denial for every operation
/// outside the typed semantic proposal/execution boundary.
pub const fn admit_operation(operation: AiBoundaryOperation) -> Result<(), AiBoundaryError> {
    match operation {
        AiBoundaryOperation::SemanticProposal | AiBoundaryOperation::SemanticExecution => Ok(()),
        AiBoundaryOperation::RawMutation(kind) => Err(AiBoundaryError::RawMutationDenied { kind }),
        AiBoundaryOperation::HostEffect(effect) => {
            Err(AiBoundaryError::HostEffectDenied { effect })
        }
    }
}

/// Trusted host/session projection used by the hostile-client adapter.
///
/// Implementations belong to trusted composition code. An untrusted request
/// must never implement or select this context through a transport payload.
/// The adapter also verifies that the resolved occurrence is active and
/// `Delegated` in the trusted lifecycle; a Human session principal is not an
/// AI execution credential.
pub trait TrustedAiRequestContext {
    fn effective_principal(&self) -> Option<&PrincipalId>;
    fn trusted_instant(&self) -> Option<TrustedInstant>;
}

/// Untrusted typed proposal intent plus non-authoritative evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct AiProposalRequest {
    id: ProposalId,
    base_revision: SemanticRevision,
    body: SemanticPatchBody,
    evidence: Vec<UntrustedData>,
}

impl AiProposalRequest {
    #[must_use]
    pub fn new(
        id: ProposalId,
        base_revision: SemanticRevision,
        body: SemanticPatchBody,
        evidence: Vec<UntrustedData>,
    ) -> Self {
        Self {
            id,
            base_revision,
            body,
            evidence,
        }
    }
}

/// Trusted lifecycle proposal paired with the request's inert evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmittedSemanticProposal {
    patch: SemanticPatch,
    evidence: Vec<UntrustedData>,
}

impl SubmittedSemanticProposal {
    #[must_use]
    pub fn patch(&self) -> &SemanticPatch {
        &self.patch
    }

    #[must_use]
    pub fn evidence(&self) -> &[UntrustedData] {
        &self.evidence
    }
}

/// Submit typed intent through the trusted Propose lifecycle.
///
/// Effective originator and time come only from `context`, and the lifecycle
/// must prove that originator is an active Delegated occurrence. Evidence is
/// retained beside the resulting proposal but is never passed as validation,
/// authorization, Approval, or command meaning.
///
/// # Errors
///
/// Returns a disclosure-safe machine-classified boundary error when trusted
/// context is unavailable or the lifecycle rejects the proposal.
pub fn submit_semantic_proposal(
    lifecycle: &mut PatchLifecycle,
    context: &impl TrustedAiRequestContext,
    document_scope: &tachiko_workspace_engine::patch_lifecycle::DocumentScopeId,
    document: &Document,
    current_revision: &SemanticRevision,
    request: AiProposalRequest,
) -> Result<SubmittedSemanticProposal, AiBoundaryError> {
    admit_operation(AiBoundaryOperation::SemanticProposal)?;
    let (originator, now) = trusted_delegated_context(lifecycle, context)?;
    let AiProposalRequest {
        id,
        base_revision,
        body,
        evidence,
    } = request;
    let patch = lifecycle
        .propose(
            document_scope,
            document,
            current_revision,
            ProposalRequest::new(id, base_revision, body, originator),
            now,
        )
        .map_err(map_lifecycle_error)?;
    Ok(SubmittedSemanticProposal { patch, evidence })
}

/// Untrusted request to execute one already-issued exact proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiExecutionRequest {
    proposal_id: ProposalId,
    approval_id: Option<ApprovalId>,
}

impl AiExecutionRequest {
    #[must_use]
    pub fn new(proposal_id: ProposalId, approval_id: Option<ApprovalId>) -> Self {
        Self {
            proposal_id,
            approval_id,
        }
    }
}

/// Execute an exact proposal through the trusted authorization/publication lifecycle.
///
/// Effective executor and time come only from `context`, and the lifecycle
/// must prove that executor is an active Delegated occurrence. The caller-
/// supplied publication authority is trusted host composition and remains
/// separate from storage or other external effects.
///
/// # Errors
///
/// Returns a disclosure-safe machine-classified boundary error when trusted
/// context is unavailable or the lifecycle denies, rejects, conflicts, or
/// cannot verify the execution.
pub fn execute_semantic_proposal(
    lifecycle: &mut PatchLifecycle,
    context: &impl TrustedAiRequestContext,
    request: &AiExecutionRequest,
    publication: &mut impl SemanticPublicationAuthority,
) -> Result<ExecutionReceipt, AiBoundaryError> {
    admit_operation(AiBoundaryOperation::SemanticExecution)?;
    let (executor, now) = trusted_delegated_context(lifecycle, context)?;
    lifecycle
        .execute(
            &request.proposal_id,
            request.approval_id.as_ref(),
            &executor,
            publication,
            now,
        )
        .map_err(map_lifecycle_error)
}

fn trusted_delegated_context(
    lifecycle: &PatchLifecycle,
    context: &impl TrustedAiRequestContext,
) -> Result<(PrincipalId, TrustedInstant), AiBoundaryError> {
    let principal = context
        .effective_principal()
        .cloned()
        .ok_or(AiBoundaryError::TrustedContextUnavailable)?;
    let now = context
        .trusted_instant()
        .ok_or(AiBoundaryError::TrustedContextUnavailable)?;
    if !lifecycle.is_active_delegated_principal(&principal) {
        return Err(AiBoundaryError::AuthorizationDenied);
    }
    Ok((principal, now))
}

fn map_lifecycle_error(error: PatchLifecycleError) -> AiBoundaryError {
    match error {
        PatchLifecycleError::ValidationFailed { report } => {
            AiBoundaryError::SemanticGateRejected { report }
        }
        PatchLifecycleError::Stale => AiBoundaryError::Stale,
        PatchLifecycleError::ApproverMustBeHuman
        | PatchLifecycleError::ReviewRequired
        | PatchLifecycleError::InvalidApprovalExpiry
        | PatchLifecycleError::ApprovalIdAlreadyExists
        | PatchLifecycleError::ApprovalNotFound
        | PatchLifecycleError::ApprovalRequired
        | PatchLifecycleError::ApprovalNotRequired
        | PatchLifecycleError::ApprovalBindingMismatch
        | PatchLifecycleError::ApprovalExpired
        | PatchLifecycleError::ApprovalRevoked
        | PatchLifecycleError::ApprovalConsumed => AiBoundaryError::ApprovalDenied,
        PatchLifecycleError::EmptyAtomicBatch
        | PatchLifecycleError::CommandRejected { .. }
        | PatchLifecycleError::ScopeDerivationFailed => AiBoundaryError::ProposalRejected,
        PatchLifecycleError::Conflict => AiBoundaryError::PublicationConflict,
        PatchLifecycleError::VerificationFailed => AiBoundaryError::VerificationFailed,
        _ => AiBoundaryError::AuthorizationDenied,
    }
}
