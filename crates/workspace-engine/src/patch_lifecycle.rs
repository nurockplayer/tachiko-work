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
use thiserror::Error;

use super::{
    Document, DocumentId, EntityId, Expression, FieldId, FieldRef, Number, SchemaId,
    SemanticChange, ValidationReport, Value, WorkspaceError, field_value_candidate, finalize_edit,
};

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
    SetFieldValue,
    FormulaReasoning,
    NumberOverrideScenario,
    FormulaUpdate,
    AnalysisQuery,
    FieldCapabilityDiscovery,
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
    SetFieldValue { field: FieldRef, value: Value },
    FormulaUpdate(FormulaUpdateCommand),
}

impl SemanticCommand {
    #[must_use]
    pub fn set_field_value(field: FieldRef, value: Value) -> Self {
        Self::SetFieldValue { field, value }
    }
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
    pub(crate) fn new(target: FieldRef, expression: Expression) -> Self {
        let references = tachiko_formula_engine::extract_dependencies(&expression);
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

/// Host/runtime seam that owns concrete revision and state-install mechanics.
///
/// An error from `publish_if_current` must prove that the candidate was not
/// published. The lifecycle holds exclusive proposal/Approval state while this
/// method runs and consumes Approval immediately after a successful return.
pub trait SemanticPublicationAuthority {
    /// Capture one coherent protected-occurrence/document/revision tuple.
    fn current_snapshot(&self) -> (DocumentScopeId, Document, SemanticRevision);

    /// Resolve one publication attempt under an exclusive guard. First compare
    /// `expected_document_scope`; an occurrence change returns
    /// [`SemanticPublicationError::DocumentScopeMismatch`] without invoking
    /// authorization for the old occurrence. Otherwise the callback must run
    /// exactly once with a fresh trusted instant inside that guard, immediately
    /// before returning a semantic Stale/Conflict outcome or installing the
    /// candidate. Install only while `expected_revision` remains current and
    /// the callback accepts, then capture and return the exact installed
    /// document occurrence, immutable document snapshot, distinct resulting
    /// revision, and the callback's authorization evidence before releasing
    /// the guard. A later publication may advance [`Self::current_snapshot`]
    /// without changing this successful result.
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
        candidate: Document,
        authorize: impl FnOnce(TrustedInstant) -> Option<Authorization>,
    ) -> Result<
        (DocumentScopeId, Document, SemanticRevision, Authorization),
        SemanticPublicationError,
    >;
}

/// Proved no-publication result from the host publication boundary.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SemanticPublicationError {
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
    #[error("the AtomicBatch must contain at least one command")]
    EmptyAtomicBatch,
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
    semantic_changes: Vec<SemanticChange>,
    formula_impacts: Vec<FormulaImpactEvidence>,
    validation_report: ValidationReport,
    footprint: AuthorizationFootprint,
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
    candidate: Document,
}

struct PublishedCandidate {
    document_scope: DocumentScopeId,
    document: Document,
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
    principals: BTreeMap<PrincipalId, PrincipalRecord>,
    grants: BTreeMap<GrantId, StoredGrant>,
    proposals: BTreeMap<ProposalId, ProposalRecord>,
    approvals: BTreeMap<ApprovalId, StoredApproval>,
    execution_receipts: Vec<ExecutionReceipt>,
    policy_meanings: BTreeMap<AuthorizationPolicyVersion, PolicyMeaningId>,
    effective_policy: AuthorizationPolicyVersion,
    policy_selection: u64,
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
        Self {
            authorization_domain,
            document_scope,
            document,
            semantic_api_contract,
            principals: BTreeMap::new(),
            grants: BTreeMap::new(),
            proposals: BTreeMap::new(),
            approvals: BTreeMap::new(),
            execution_receipts: Vec::new(),
            policy_meanings: BTreeMap::from([(effective_policy.clone(), policy_meaning)]),
            effective_policy,
            policy_selection: 0,
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
        };
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

        let evaluated = match self.finalize_evaluation(
            document,
            candidate,
            patch.exact_change.body(),
            provisional_footprint.associated_write_requirements.clone(),
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
        record.footprint = Some(evaluated.footprint);
        self.proposals.insert(patch.id.clone(), record);
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
        self.require_document(document_scope, document)?;
        let record = self.select_disclosable_proposal(proposal_id, viewer, now)?;
        let stored_footprint = record
            .footprint
            .clone()
            .ok_or(PatchLifecycleError::ProposalNotExecutable)?;
        Self::require_nonterminal_proposal(&record, true)?;
        if record.patch.exact_change.base_revision != *current_revision {
            self.append_state(proposal_id, PatchLifecycleState::Stale);
            return Err(PatchLifecycleError::Stale);
        }
        let evaluated = self.evaluate_patch(document, &record.patch)?;
        if evaluated.footprint != stored_footprint {
            self.append_state(proposal_id, PatchLifecycleState::Conflict);
            return Err(PatchLifecycleError::ApprovalBindingMismatch);
        }

        let proposal = record.patch;
        let risk = PatchRisk {
            mutation_classes: evaluated.footprint.mutation_classes(),
        };
        let proposal_record = self
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
        Ok(PatchPreview {
            proposal,
            semantic_changes: evaluated.semantic_changes,
            formula_impacts: evaluated.formula_impacts,
            validation_report: evaluated.validation_report,
            authorization_footprint: evaluated.footprint,
            risk,
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
        let evaluated = self.evaluate_patch(document, &proposal.patch)?;
        if evaluated.footprint != footprint {
            return Err(PatchLifecycleError::ApprovalBindingMismatch);
        }
        let approval = Approval {
            id: request.id.clone(),
            binding: ApprovalBinding {
                authorization_domain: self.authorization_domain.clone(),
                proposal_id: request.proposal_id.clone(),
                exact_change: proposal.patch.exact_change.clone(),
                originator: proposal.originator,
                executor: request.executor,
                associated_write_requirements: evaluated.footprint.associated_write_requirements,
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
        let (current_document_scope, document, current_revision) = publication.current_snapshot();
        if self
            .require_document(&current_document_scope, &document)
            .is_err()
        {
            return Err(PatchLifecycleError::AuthorizationDenied);
        }
        let (proposal, footprint, approval) =
            self.select_execution_context(proposal_id, approval_id, executor)?;
        let can_disclose = self
            .authorize_query(executor, &footprint.disclosure_requirements, now)
            .is_ok();
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

        let evaluated = self.evaluate_patch(&document, &proposal.patch)?;
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
        let candidate = evaluated.document.clone();
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
                candidate: evaluated.document,
            },
        )?;
        let can_disclose = published.authorization.can_disclose;
        let mut receipt = ExecutionReceipt {
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
            semantic_changes: evaluated.semantic_changes,
            formula_impacts: evaluated.formula_impacts,
            validation_report: Some(evaluated.validation_report),
        };
        self.execution_receipts.push(receipt.clone());
        let verification_report = match self.verify_publication(
            proposal_id,
            &current_revision,
            &published.revision,
            &candidate,
            &published.document_scope,
            &published.document,
        ) {
            Ok(report) => report,
            Err(error) if can_disclose => return Err(error),
            Err(_) => return Err(PatchLifecycleError::AuthorizationDenied),
        };
        receipt.validation_report = Some(verification_report);
        receipt.verified = true;
        self.append_state(proposal_id, PatchLifecycleState::Verified);
        if let Some(stored) = self.execution_receipts.last_mut() {
            *stored = receipt.clone();
        }
        if !can_disclose {
            receipt.authorization_footprint = None;
            receipt.semantic_changes.clear();
            receipt.formula_impacts.clear();
            receipt.validation_report = None;
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

    fn publish_candidate(
        &mut self,
        publication: &mut impl SemanticPublicationAuthority,
        attempt: PublicationAttempt<'_>,
    ) -> Result<PublishedCandidate, PatchLifecycleError> {
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
        candidate: &Document,
        installed_document_scope: &DocumentScopeId,
        installed_document: &Document,
    ) -> Result<ValidationReport, PatchLifecycleError> {
        if installed_document_scope != &self.document_scope {
            self.append_state(proposal_id, PatchLifecycleState::Conflict);
            return Err(PatchLifecycleError::VerificationFailed);
        }
        let report = super::validation_report(installed_document);
        if resulting_revision == base_revision
            || installed_document != candidate
            || !report.is_valid()
        {
            self.append_state(proposal_id, PatchLifecycleState::Conflict);
            return Err(PatchLifecycleError::VerificationFailed);
        }
        Ok(report)
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
                    SemanticCommand::SetFieldValue { .. } => OperationFamily::SetFieldValue,
                    SemanticCommand::FormulaUpdate(_) => OperationFamily::FormulaUpdate,
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

    fn plan_commands(
        &self,
        document: &Document,
        body: &SemanticPatchBody,
    ) -> Result<(Document, BTreeSet<AssociatedWriteRequirement>), WorkspaceError> {
        let mut candidate = document.clone();
        let mut writes = BTreeSet::new();
        for command in body.commands() {
            match command {
                SemanticCommand::SetFieldValue { field, value } => {
                    let entity = candidate.entities.get(&field.entity).ok_or_else(|| {
                        WorkspaceError::MissingEntityId {
                            entity: field.entity.clone(),
                        }
                    })?;
                    let existing = entity.fields.get(&field.field).ok_or_else(|| {
                        WorkspaceError::MissingField {
                            field: field.clone(),
                        }
                    })?;
                    let mutation_classes = classify_field_transition(existing, value);
                    let scope = self.field_scope(&candidate, field)?;
                    for mutation_class in mutation_classes {
                        writes.insert(AssociatedWriteRequirement {
                            family: OperationFamily::SetFieldValue,
                            mutation_class,
                            scope: scope.clone(),
                        });
                    }
                    candidate = field_value_candidate(&candidate, field, value)?;
                }
                SemanticCommand::FormulaUpdate(command) => {
                    let field = command.target();
                    let scope = self.field_scope(&candidate, field)?;
                    writes.insert(AssociatedWriteRequirement {
                        family: OperationFamily::FormulaUpdate,
                        mutation_class: MutationClass::Formula,
                        scope,
                    });
                    candidate = field_value_candidate(
                        &candidate,
                        field,
                        &Value::Formula(command.expression().clone()),
                    )?;
                }
            }
        }
        Ok((candidate, writes))
    }

    fn finalize_evaluation(
        &self,
        base: &Document,
        candidate: Document,
        body: &SemanticPatchBody,
        associated_write_requirements: BTreeSet<AssociatedWriteRequirement>,
    ) -> Result<EvaluatedPatch, PatchLifecycleError> {
        let edit = match finalize_edit(base, candidate) {
            Ok(edit) => edit,
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
        let validation_report = super::validation_report(&edit.document);
        let disclosure_requirements =
            self.derive_disclosures(base, &edit.document, body, &semantic_changes)?;
        Ok(EvaluatedPatch {
            document: edit.document,
            semantic_changes,
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
        self.finalize_evaluation(document, candidate, patch.exact_change.body(), writes)
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
        let mut disclosures = BTreeSet::new();
        for command in body.commands() {
            self.insert_command_disclosures(before, after, command, &mut disclosures)?;
        }
        for change in changes {
            self.insert_change_disclosures(before, after, body, change, &mut disclosures)?;
        }
        Ok(disclosures)
    }

    fn insert_command_disclosures(
        &self,
        before: &Document,
        after: &Document,
        command: &SemanticCommand,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        match command {
            SemanticCommand::SetFieldValue { field, value } => {
                self.insert_field_disclosure(before, after, field, disclosures)?;
                self.insert_value_disclosures(before, after, value, disclosures)
            }
            SemanticCommand::FormulaUpdate(command) => {
                self.insert_field_disclosure_for(
                    OperationFamily::FormulaUpdate,
                    before,
                    after,
                    command.target(),
                    disclosures,
                )?;
                for reference in command.references() {
                    self.insert_field_disclosure_for(
                        OperationFamily::FormulaUpdate,
                        before,
                        after,
                        reference,
                        disclosures,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn insert_change_disclosures(
        &self,
        before: &Document,
        after: &Document,
        body: &SemanticPatchBody,
        change: &SemanticChange,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        match change {
            SemanticChange::FieldChanged {
                field,
                before: old_value,
                after: new_value,
            } => {
                let mut families = command_families_for_field(body, field);
                if families.is_empty() {
                    families.insert(OperationFamily::SetFieldValue);
                }
                for family in families {
                    self.insert_field_disclosure_for(family, before, after, field, disclosures)?;
                    self.insert_value_disclosures_for(
                        family,
                        before,
                        after,
                        old_value,
                        disclosures,
                    )?;
                    self.insert_value_disclosures_for(
                        family,
                        before,
                        after,
                        new_value,
                        disclosures,
                    )?;
                }
                Ok(())
            }
            SemanticChange::FormulaImpact { field, causes, .. } => {
                let mut family_causes = BTreeMap::<OperationFamily, Vec<&FieldRef>>::new();
                for cause in causes {
                    let mut families = command_families_for_field(body, cause);
                    if families.is_empty() {
                        families.insert(OperationFamily::SetFieldValue);
                    }
                    for family in families {
                        family_causes.entry(family).or_default().push(cause);
                    }
                }
                for (family, causes) in family_causes {
                    self.insert_field_disclosure_for(family, before, after, field, disclosures)?;
                    for cause in causes {
                        self.insert_field_disclosure_for(
                            family,
                            before,
                            after,
                            cause,
                            disclosures,
                        )?;
                    }
                }
                Ok(())
            }
            _ => Err(PatchLifecycleError::ScopeDerivationFailed),
        }
    }

    fn insert_field_disclosure(
        &self,
        before: &Document,
        after: &Document,
        field: &FieldRef,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        self.insert_field_disclosure_for(
            OperationFamily::SetFieldValue,
            before,
            after,
            field,
            disclosures,
        )
    }

    fn insert_field_disclosure_for(
        &self,
        family: OperationFamily,
        before: &Document,
        after: &Document,
        field: &FieldRef,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        let scope = self
            .field_scope(after, field)
            .or_else(|_| self.field_scope(before, field))
            .map_err(|_| PatchLifecycleError::ScopeDerivationFailed)?;
        disclosures.insert(DisclosureRequirement { family, scope });
        Ok(())
    }

    fn insert_value_disclosures(
        &self,
        before: &Document,
        after: &Document,
        value: &Value,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        self.insert_value_disclosures_for(
            OperationFamily::SetFieldValue,
            before,
            after,
            value,
            disclosures,
        )
    }

    fn insert_value_disclosures_for(
        &self,
        family: OperationFamily,
        before: &Document,
        after: &Document,
        value: &Value,
        disclosures: &mut BTreeSet<DisclosureRequirement>,
    ) -> Result<(), PatchLifecycleError> {
        match value {
            Value::Reference(entity) => {
                let scope = self
                    .entity_scope(after, entity)
                    .or_else(|_| self.entity_scope(before, entity))?;
                disclosures.insert(DisclosureRequirement { family, scope });
            }
            Value::Formula(expression) => {
                for field in expression_references(expression) {
                    self.insert_field_disclosure_for(family, before, after, &field, disclosures)?;
                }
            }
            Value::Number(_) | Value::Text(_) | Value::Boolean(_) | Value::Date(_) => {}
        }
        Ok(())
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

fn command_families_for_field(
    body: &SemanticPatchBody,
    field: &FieldRef,
) -> BTreeSet<OperationFamily> {
    body.commands()
        .iter()
        .filter_map(|command| match command {
            SemanticCommand::FormulaUpdate(command) if command.target() == field => {
                Some(OperationFamily::FormulaUpdate)
            }
            SemanticCommand::SetFieldValue { field: target, .. } if target == field => {
                Some(OperationFamily::SetFieldValue)
            }
            _ => None,
        })
        .collect()
}

fn classify_field_transition(existing: &Value, replacement: &Value) -> BTreeSet<MutationClass> {
    if matches!(replacement, Value::Formula(_)) {
        BTreeSet::from([MutationClass::Formula])
    } else if matches!(existing, Value::Formula(_)) {
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

fn expression_references(expression: &Expression) -> BTreeSet<FieldRef> {
    let mut references = BTreeSet::new();
    let mut pending = vec![expression];
    while let Some(current) = pending.pop() {
        match current {
            Expression::Number(_) => {}
            Expression::Reference(field) => {
                references.insert(field.clone());
            }
            Expression::Add { left, right }
            | Expression::Subtract { left, right }
            | Expression::Multiply { left, right }
            | Expression::Divide { left, right }
            | Expression::Minimum { left, right }
            | Expression::Maximum { left, right } => {
                pending.push(right);
                pending.push(left);
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

#[cfg(test)]
mod tests {
    use super::*;

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
        };

        let error = lifecycle
            .entity_scope(&document, &EntityId::from("missing"))
            .unwrap_err();

        assert!(matches!(error, PatchLifecycleError::ScopeDerivationFailed));
    }
}
