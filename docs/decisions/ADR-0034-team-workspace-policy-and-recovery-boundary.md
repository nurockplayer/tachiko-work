# ADR-0034: Team workspace policy and recovery boundary

## Status

Accepted

Decision issue: [#11](https://github.com/nurockplayer/tachiko-work/issues/11)

Specified by:
[Semantic Authorization and Approval Specification](../specs/semantic-authorization.md),
[Semantic API Specification](../specs/semantic-api.md),
[Semantic Operation Log Model](../specs/operation-log-model.md), and
[Collaboration Model Specification](../specs/collaboration-model.md)

Related authority:
[ADR-0026](ADR-0026-scoped-semantic-authorization-and-approval.md),
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md),
[ADR-0032](ADR-0032-semantic-execution-and-transition-taxonomy.md), and
[ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)

## Context

ADR-0026 defines the provider-neutral MVP authorization and approval contract:
trusted Human and Delegated principal occurrences, independently checked
capability dimensions, document-local stable-ID semantic scope, trusted
authorization-footprint derivation, default-deny Grants, exact finite Human
Approval, replay/revocation behavior, minimum security/provenance evidence, and
separation between semantic and external-effect authority.

ADR-0029 keeps current semantic state and complete snapshots authoritative while
making general history optional. ADR-0032 keeps `Command | AtomicBatch` as the
only normative semantic intent/publication vocabulary and distinguishes Execute
attempts, revision occurrences, receipts, and optional retained transitions.
ADR-0033 defines snapshot-first history/checkpoint guarantees and truthful
partial-failure recovery without making history an execution or authorization
authority.

Team operation still needs a boundary for reusable policy, administration,
multi-document workflows, independently failing effects, recovery, retry, and
additional audit evidence. Without one, implementations could accidentally
turn `AtomicBatch` into a distributed transaction, mutable team labels into
semantic authority, optional history into recovery authority, or a post-install
failure into the false claim that publication did not occur.

This decision fixes only that residual boundary. It does not select production
DTOs, storage, coordinators, external-effect adapters, enterprise identity
mechanics, or runtime implementation.

## Decision

### 1. Semantic atomicity remains one document publication

`Command | AtomicBatch` remains the complete normative semantic intent and
publication vocabulary. One `AtomicBatch` is ordered and all-or-nothing only
for one semantic publication against one exact current document base. Its
members create no intermediate revision occurrences.

`AtomicBatch` MUST NOT be interpreted as atomic host persistence, required
security/provenance evidence persistence, optional history/checkpoint
persistence, Git activity, external effects, collaboration coordination, or
multi-document publication. This decision introduces no new semantic
transaction or mutation value.

### 2. Coordinated effect domains remain independently truthful

A workflow may coordinate these domains:

- semantic publication;
- host or browser persistence;
- ADR-0026 required security/provenance evidence;
- ADR-0033 optional history, checkpoints, and replay verification;
- Git evidence;
- external effects; and
- collaboration or service coordination.

Each domain retains its own authority and actual outcome. A later failure,
unknown result, retry, compensation, or reporting gap in one domain MUST NOT
rewrite the known outcome of another domain. In particular, an installed
semantic revision remains installed even when persistence, evidence recording,
Git, an external effect, coordination, or response delivery later fails.

An implementation may present a composite workflow result, but it MUST preserve
enough distinction to report known success, known failure, not-attempted work,
and an outcome that still requires reconciliation without flattening them into
one misleading success or failure claim. This logical requirement does not
freeze a result enum, DTO, wire shape, or durable record format.

### 3. Multi-document workflows are orchestration, not atomic mutation

A multi-document workflow is orchestration over separately exact-base,
separately authorized document publications. Every document publication passes
the ordinary Semantic API admission, authorization, Approval when applicable,
validation, gate, and publication boundary for that document.

M06 promises no all-or-nothing multi-document mutation, distributed commit, or
automatic rollback. If one document publishes and another does not, the result
is explicit partial success. The published revision MUST NOT be erased,
rewound, or represented as never having occurred merely to make the workflow
appear atomic.

A later decision may add a dedicated multi-document transaction contract only
when concrete product evidence justifies its authority, isolation, durability,
failure, recovery, and compatibility guarantees. No such contract is selected
here.

### 4. Recovery reconciles current authority and moves forward

Recovery begins from current authoritative snapshots plus genuine durable
evidence from the relevant domains. Optional history or checkpoints may improve
reconciliation under ADR-0033, but they do not become execution,
authorization, or current-state authority.

If semantic publication occurred before a later failure, recovery preserves
`publication happened + later domain failed or is unknown`. A semantic
compensation, undo, or revert is a new authorized `Command | AtomicBatch`
against the exact current base. It creates a new revision occurrence when it
publishes. Recovery MUST NOT delete, rewind, retarget, or fabricate prior
publication facts.

A retry is a new attempt unless a separately authoritative adapter can prove it
is returning the established outcome of an earlier delivery. Before semantic
publication, retry rechecks the current base, principal occurrences, live
Grants, effective policy, Approval where applicable, semantic gates, and every
other ordinary publication condition. Proposal identity, revision identity,
snapshot identity, retained-transition identity, receipt identity, checkpoint
identity, and Git identity are not retry-idempotency identities.

When an external effect has an uncertain outcome, the host/effect boundary MUST
reconcile that outcome before repeating the effect. External correlation,
delivery deduplication, and idempotency policy belong to that boundary and MUST
NOT be derived by requirement from semantic proposal, revision, snapshot,
transition, receipt, checkpoint, or Git identity.

### 5. Reusable team policy constrains ADR-0026 authority from above

A trusted host may administer reusable team policy that selects or constrains
existing ADR-0026 authorization requirements across:

- Query, Propose, Execute, and Approve actions;
- immutable operation-family meaning;
- Value, Formula, Structure, Schema, and Destructive mutation classes; and
- the existing document-local stable-ID semantic scope atoms.

Selection determines which already-required policy profile applies;
constraint may deny or add stricter conditions. Policy MUST NOT remove a tuple
from the trusted `AuthorizationFootprint`, weaken complete Grant coverage, or
otherwise reduce authority required by ADR-0026.

Reusable policy does not mint semantic authority by itself. Concrete authority
still resolves through the trusted authorization domain and the applicable live
ADR-0026 Grants, Approval, footprint, policy-version, and publication checks.
Policy MUST NOT create ambient session trust, provider/model privilege, a
second semantic scope model, delegated self-escalation, or transferable
authority.

Team, project, workspace, organization, directory, group, path, branch, Git,
storage, provider, model, login, and display-name facts may help the trusted
host select administration policy. They MUST NOT become semantic identity,
document-local semantic scope, or authority merely by naming or grouping a
subject. Exact role catalogues, organization models, tenancy, and identity-
provider mappings remain Deferred.

Policy meaning and effective-policy changes obey ADR-0026's immutable-version
and uninterrupted-selection laws. A changed policy meaning requires a new
version and transition; mutation under a reused version fails closed. A policy
change cannot revive or broaden an Approval, and a transition away and back
does not restore an older Approval.

### 6. Administration requires explicit Human authority

Policy and Grant administration require an explicitly authorized Human action
at the trusted host boundary. Human principal class alone does not grant
administration authority. A Delegated principal cannot self-grant, expand its
authority, change the effective policy, or transitively delegate administration
authority.

For reusable team policy, this requirement narrows ADR-0026's Provisional
Grant-issuance alternative: trusted host policy may select and enforce
provisioning constraints, but it MUST NOT independently issue a Grant without
an explicitly authorized Human provisioning action. ADR-0026's allowance for
issuance by trusted host policy therefore does not apply to this team-policy
profile. Exact non-team provisioning defaults remain Provisional under
ADR-0026.

Revocation, expiry, disablement, replacement, and effective-policy transitions
take effect through the existing live ADR-0026 checks. Reusable policy MUST NOT
turn expired, revoked, disabled, consumed, stale, or replaced occurrences into
authority or transfer authority through reused aliases.

A host policy may require a fresh explicit Human review gate for additional
Human-originated schema, destructive, publication, permission, or
administration actions. For semantic publication, such a gate composes with
ADR-0026 exact Approval; it does not redefine Approval as authority for a non-
semantic administration action. Policy MUST NOT waive ADR-0026's required
Approval for Delegated-origin or Delegated-authority semantic publication. The
additional review remains an authorization policy condition; it does not change
semantic command meaning or make review evidence semantic state.

Separation of duties, quorum, multi-party approval, and approval chains may be
added later as explicitly versioned, evidence-backed host policy capabilities.
They are not universal semantic-core rules and are not required by this
decision.

### 7. Team audit is evidence, not authority

A team deployment may retain additional principal-administration,
policy-selection, Grant-administration, review, coordination, and external-
effect evidence. A later concrete profile may make named records durable,
exportable, retention-governed, or redactable.

Those records are not semantic Document state, mutation input, authorization,
Approval, publication, revision, or retained-transition authority. Retention,
redaction, compaction, or loss MUST disclose the resulting gaps truthfully and
MUST NOT manufacture authorization, publication, non-publication, external-
effect, or history facts.

ADR-0026 minimum security/provenance and replay-protection evidence remains
independently required where applicable even when general history or additional
team audit is absent. Conversely, additional team audit does not prove the
ADR-0026 contract unless the required facts remain independently identifiable
and valid.

### 8. Concrete mechanisms remain separately owned

This decision authorizes no production implementation.

Separately Ready Issues must own any concrete:

- public DTO, wire, storage, or identifier contract;
- cross-effect or multi-document coordinator and its durability/isolation
  guarantees;
- external-effect adapter, correlation, or idempotency protocol;
- team/organization role catalogue, tenancy, IAM, SSO, or SCIM mechanics;
- separation-of-duties, quorum, or approval-chain product workflow;
- durable team audit, retention, redaction, or export profile;
- history/checkpoint/replay engine under ADR-0033;
- runtime authorization or host persistence changes; or
- offline causality, CRDT, OT, or resynchronization work.

Issue #47 retains cross-version migration behavior, Issue #50 retains offline
causality and selective CRDT/OT, and Issue #53 retains exact commitment bytes,
digest/signature algorithms, and trust semantics.

## Required pressure tests

Future concrete contracts and implementations must preserve these logical
outcomes:

1. **Post-publication persistence failure** — the semantic revision remains a
   successful publication while persistence is reported separately as failed
   or uncertain.
2. **Partial multi-document success** — publication to document A followed by
   denial or failure for document B preserves A's new revision and reports the
   partial result without automatic rollback.
3. **Forward compensation** — restoring prior-equivalent content uses a new
   authorized command against the current base and, when published, creates a
   new revision occurrence.
4. **External outcome unknown** — the host reconciles the external system
   before retry and does not infer effect idempotency from proposal or revision
   identity.
5. **Policy narrows authority** — a reusable policy restriction can deny an
   otherwise representable request, but policy selection alone cannot replace a
   required live Grant or Approval.
6. **No ambient team scope** — team, role, path, branch, provider, or login
   membership does not match an ADR-0026 document-local scope atom by itself.
7. **Delegated self-escalation denied** — a Delegated principal cannot issue or
   widen its own policy, Grant, or administration authority.
8. **Policy transition invalidates Approval** — changing effective policy and
   later returning to the prior version does not revive the earlier Approval.
9. **Audit gap remains truthful** — missing or redacted optional team audit
   never fabricates authorization, publication, effect success, or complete
   history and cannot satisfy missing ADR-0026 minimum evidence.
10. **Effect failure does not rewrite publication** — a failed Git, external,
    coordination, reporting, or evidence effect cannot turn a known installed
    revision into pre-publication failure or `NoChange`.

## Consequences

- Team policy can reduce repeated administration and review configuration
  without creating a broad role DSL or ambient agent authority.
- Multi-document and multi-effect workflows expose partial outcomes honestly
  instead of promising an unimplemented distributed transaction.
- Recovery remains compatible with snapshot-first authority and optional
  history by reconciling evidence and moving forward.
- Required security evidence, optional history, Git, and external effects can
  evolve independently while preserving their actual outcomes.
- Concrete enterprise identity, audit, transaction, persistence, and adapter
  work still requires separately accepted scope and Ready implementation.

## Rejected alternatives

- **Treat AtomicBatch as a distributed transaction:** rejected because semantic
  publication does not define host, durability, multi-document, or external-
  effect atomicity.
- **Automatically roll back or rewind an installed revision:** rejected because
  it falsifies publication history and bypasses current-base authorization.
- **Use optional history/checkpoints for execution or authorization:** rejected
  because ADR-0029 and ADR-0033 keep snapshots authoritative and history
  evidentiary.
- **Ambient session-wide agent or provider trust:** rejected because it defeats
  ADR-0026's default-deny relational authority and exact Approval boundary.
- **Blindly retry an uncertain external effect:** rejected because repeating an
  unknown outcome may duplicate an irreversible effect.
- **Make team names, roles, paths, or Git coordinates semantic scope:** rejected
  because mutable administrative locators cannot replace document-local stable
  semantic identity.
- **Require universal four-eyes or quorum in semantic core:** rejected because
  stronger review is a host policy capability whose product evidence and exact
  mechanics remain future work.
- **Treat audit as semantic or authorization authority:** rejected because
  retained evidence can be incomplete, redacted, or unavailable without
  redefining current semantic meaning or legitimate authority.

## Related

- [Issue #11](https://github.com/nurockplayer/tachiko-work/issues/11)
- [Semantic Authorization and Approval Specification](../specs/semantic-authorization.md)
- [Semantic API Specification](../specs/semantic-api.md)
- [Semantic Operation Log Model](../specs/operation-log-model.md)
- [Collaboration Model Specification](../specs/collaboration-model.md)
- [Security Model](../specs/security-model.md)
- [Decision traceability protocol](../governance/decision-traceability.md)
