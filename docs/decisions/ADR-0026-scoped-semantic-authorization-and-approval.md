# ADR-0026: Scoped semantic authorization and approval

## Status

Accepted

Decision issue: [#28](https://github.com/nurockplayer/tachiko-work/issues/28)

Specified by: [`semantic-authorization.md`](../specs/semantic-authorization.md)

Related authority: ADR-0007, ADR-0015, ADR-0019, ADR-0020, ADR-0022,
ADR-0024

Related implementation: [#29](https://github.com/nurockplayer/tachiko-work/issues/29),
[#30](https://github.com/nurockplayer/tachiko-work/issues/30), and
[#93](https://github.com/nurockplayer/tachiko-work/issues/93)

## Context

ADR-0007 establishes that AI is a delegated semantic client with no intrinsic
authority. Semantic validity and operation gating do not grant permission, and
authorization or approval cannot override semantic failure. Provider, model,
tool, prompt, confidence, and model-supplied metadata are provenance rather
than privilege or semantic truth.

ADR-0020 establishes the shared transport-neutral Semantic API meaning:
`Query`, typed `Command`, `Propose`, `Execute`, ordered all-or-nothing
`AtomicBatch`, validation/gates, and independently capability-addressable
operation families. ADR-0024 then establishes one immutable revision-pinned
`SemanticPatch` proposal occurrence around `Propose(Command | AtomicBatch)`,
with representation-neutral `ExactChangeBinding`, exact semantic-base binding,
and fail-closed stale behavior.

Those decisions intentionally do not define principals, grants, semantic
scope, approval lifetime, replay/revocation behavior, or minimum provenance.
The current `Suggestion { field, value, requires_approval }` implementation is
evidence of the MVP safety posture, not a durable authorization protocol.

The MVP needs the smallest provider-neutral contract that lets a trusted
application determine who may query, propose, approve, or execute; what exact
semantic subjects and mutation classes that authority covers; what one human
approved; and when that approval is no longer usable. It must not become
enterprise IAM, a role system, a generic policy language, or a portable
bearer-token protocol.

## Decision

### 1. Principals are opaque and domain-scoped

Every authorization-relevant action is attributed to one opaque
`PrincipalId`, meaningful within exactly one trusted authorization domain. The
MVP distinguishes:

- `Human`: a principal trusted by the identity/host boundary as human; and
- `Delegated`: an agent, automation, service, plugin, or similar delegated
  principal.

Principal class comes from the trusted identity/host boundary. A client,
request body, prompt, model response, imported document, or plugin result
cannot self-assert or upgrade it. Provider, model, tool, prompt, confidence,
and other model-supplied metadata may be retained as provenance but never
grant privilege.

`(authorization domain, PrincipalId)` identifies one non-reusable,
non-reassignable principal occurrence. Deleting, disabling, transferring,
recreating, or replacing an account cannot assign that PrincipalId to a
different authorization subject; a replacement subject receives a new
PrincipalId. Login names, email addresses, provider identifiers, aliases, and
similar account attributes are not authorization identity and their reuse
cannot retarget an existing Grant, Approval originator/executor binding, or
provenance record. Those references remain attached only to the original
principal occurrence and fail closed when that occurrence is disabled,
missing, unauthenticated, or unresolvable.

The authorization domain does not create a semantic `ProjectId` or
`WorkspaceId`. Exact account/login/identity-provider behavior, principal ID
encoding, and Rust/wire representations remain Provisional.

### 2. Capabilities preserve independent action and mutation dimensions

Every Semantic API operation or family remains independently
capability-addressable under ADR-0020. The minimum authority dimensions are:

```text
Query
Propose(MutationClass)
Execute(MutationClass)
Approve(MutationClass)
```

`MutationClass` is an orthogonal set:

- `Value`: stored non-formula typed values, including references;
- `Formula`: formula definitions or formula-bearing transitions;
- `Structure`: document/entity lifecycle, create, duplicate, rename,
  membership, and similar non-schema structure;
- `Schema`: schema/field definition, type, requiredness, or schema semantics;
  and
- `Destructive`: semantic fact loss, combined with another class.

Ordinary in-place replacement at one stable target is not automatically
`Destructive` merely because the prior value or definition changes. A Stable
command profile adds `Destructive` when the intended transition removes a
semantic object or fact without a same-kind replacement, or otherwise has
explicit data-loss meaning.

Examples:

- scalar or reference update requires `Value`;
- formula update requires `Formula`;
- entity create, rename, or duplicate requires `Structure`;
- schema definition change requires `Schema`;
- entity deletion requires `Structure + Destructive`;
- formula removal requires `Formula + Destructive`; and
- schema-field deletion requires `Schema + Destructive`.

No capability implies another action or mutation class. Query does not imply
Propose, Propose does not imply Execute, Approve does not imply Execute, and
Execute does not imply Approve. Exact capability identifier strings and the
complete operation-to-class catalogue remain Provisional.

### 3. Scope is stable-ID, document-local semantic scope

The accepted semantic scope atoms are:

```text
Document(DocumentId)
Schema(SchemaId)
SchemaField(SchemaId, FieldId)
Entity(EntityId)
EntityField(EntityId, FieldId)
```

Every atom is interpreted within one document-local semantic context. A stable
object ID from another document is not scope equivalence; exact source or wire
representation of that document qualification remains Provisional.

Their containment meaning is:

- `Document` covers every semantic subject in that document;
- `Schema` covers its definition, fields, and entities/instances belonging to
  it in the relevant base or candidate;
- `SchemaField` covers the field definition and its entity-field instances;
- `Entity` covers the entity and its field instances, but not its schema
  definition; and
- `EntityField` covers one exact field instance.

A Grant may contain a finite union of scope atoms. An operation that crosses
old and new containers requires coverage of both relevant sides.

Project, workspace, organization, tenant, path, JSON Pointer, wildcard string,
UI coordinate, Git coordinate, storage coordinate, natural-language scope, and
generic predicate scope are not part of this MVP contract.

### 4. Trusted authority derives the authorization footprint

The trusted semantic/application authority derives, from the typed operation
and the relevant base/candidate relationships:

```text
AuthorizationFootprint
- disclosure_scope
- associated_write_requirements
- canonical_write_scope
- mutation_classes
```

The client or agent cannot authoritatively declare its own footprint. Each
associated write requirement retains one mutation-class/canonical-write-scope
pair.
At a Propose, Approve, or Execute check, the trusted boundary combines the
requested action with every pair and requires same-Grant coverage for that
complete association. The action is check context, not part of the footprint
bound into Approval. `AtomicBatch` uses the union of those associated
per-command requirements. Flattened canonical-write-scope and mutation-class
sets are review summaries, not an authorization or Approval proof and not
permission to form their Cartesian product.

Canonical write scope includes direct targets, generated IDs, created or
deleted objects and their owning containers, explicit retargeting, and
command-defined canonical side effects. Purely derived formula recalculation
and `FormulaImpact` do not become canonical write scope.

Disclosure scope includes subjects revealed through Query results, preview,
semantic diff, dependencies, impact, diagnostics, and similar evidence. If the
trusted boundary cannot determine affected scope safely, it fails closed or
requires a broader explicit scope.

Propose authority does not grant arbitrary read access. Preview evidence that
would reveal subjects outside Query authority must be denied or safely reduced;
exact projection and redaction mechanisms remain #29/#30 work.

### 5. Grants are explicit, default-deny, and non-delegable by delegated principals

Conceptually, without fixing a DTO:

```text
Grant
- grant_id
- authorization_domain
- issuer_principal
- subject_principal
- capabilities
- semantic_scope
- validity
```

`(authorization_domain, grant_id)` identifies one non-reusable issuance
occurrence. Its issuer, subject, capabilities, semantic scope, and validity are
immutable. Trusted registry state records revocation for that occurrence;
revocation is terminal, and restored or equivalent authority requires a new
Grant occurrence and GrantId.

Authorization is denied unless sufficient live Grants cover the complete
requirement. For a requested mutation action, each derived `(mutation class,
scope)` pair must be covered together with that action by one same live Grant.
Multiple Grants may combine across distinct associated requirements, but
independently unioning their capabilities/classes and scopes must not create
crossed authority that no Grant actually contains. A Grant from another
authorization domain or for another subject grants nothing to the effective
principal.

Grants are issued only through a trusted host authorization authority, which
may act on an explicitly authorized Human provisioning action or trusted host
policy. Human principal class alone does not confer Grant-issuance authority. A
Delegated principal cannot self-grant, expand, or transitively delegate
authority. Grant validity and revocation are rechecked whenever the occurrence
is relied upon. Immediately before Execute, the trusted boundary rechecks the
authorizing Approve Grant references and sufficient live Execute authority as
specified below. Approval cannot extend, restore, or replace an expired or
revoked Grant.

ADR-0007's current-MVP `read`, `analysis`, `explanation`, and `Propose` entries
remain allowed product behavior through explicit trusted-host provisioning of
sufficient Query and Propose Grants for those supported flows. They are not
ambient authority or an exception to default deny. Exact provisioning defaults
and administration remain Provisional.

Exact Grant DTOs, capability strings, storage, administration, validity/clock
encoding, and bootstrap mechanisms remain Provisional.

### 6. Delegated origin or authority requires one exact Human Approval

Any `SemanticPatch` originated by a Delegated principal or executed using
Delegated authority requires one explicit Human Approval before Execute.
Query and Propose require Grants but no Approval.

For one proposal or AtomicBatch:

- one Human approver must hold live `Approve(...)` authority covering every
  associated mutation-class/scope write requirement;
- approval covers the exact whole batch;
- partial-batch approval is forbidden;
- several partial approvers cannot be combined;
- quorum, approval chains, standing/reusable policies, and autonomous approval
  are not part of the MVP; and
- no mandatory four-eyes separation is introduced.

A UI may present approve-and-execute as one human action, but Approval and
Execute remain logically separate records and checks.

### 7. Approval binds the exact proposal and authorization context

Conceptually:

```text
ApprovalBinding(A) =
    authorization domain
  + proposal occurrence ID
  + ADR-0024 ExactChangeBinding
  + originating principal
  + authorized executor principal
  + complete associated mutation-class/canonical-write-scope requirements
  + authorization-policy version
```

Approval additionally records an approval ID, the Human approver, issued time,
finite expiry, and the authorizing Approve Grant references.

The trusted boundary structurally verifies one immutable proposal occurrence
and its complete `ExactChangeBinding`, then binds the exact originator,
executor, complete associated write requirements, and supported policy version.
The Approval record separately identifies its trusted Human approver. Flattened
canonical-write-scope and mutation-class summaries may be retained for review
and provenance, but they do not replace the bound relation.
Disclosure scope is authorized independently for each Query,
preview, review, diagnostic, or result projection; changing a presentation
projection does not change what semantic publication was approved. Approval
does not bind only to chat text, intent prose, rendered diff, validation prose,
or model claims, and it is not transferable bearer authority.

Immediately before Execute, the trusted boundary rederives the authorization
requirement and requires sufficient live Execute Grants. The authorizing
Approve Grant references recorded on Approval must also remain valid and
covering. Expiry or revocation of a required referenced Approve Grant makes
Approval unusable; a newly issued equivalent Grant does not revive it.

Trusted structural binding is sufficient for the MVP when the trusted proposal
store proves immutability and non-reuse of `ProposalId`. This decision does not
select canonical proposal/approval bytes, a hash or digest algorithm,
signature, MAC, JSON/Serde layout, IPC/WASM DTO, or portable token. Approval
crossing an untrusted boundary requires a separately accepted
integrity-protected profile.

### 8. Approval is finite, revocable, and authorizes at most one publication

Approval has the lifecycle:

```text
Active -> Consumed | Revoked | Expired
```

Terminal states never return to Active. Successful semantic publication
atomically consumes Approval. Failure before publication does not consume it.
Every retry rechecks expiry, revocation, Grants, exact base,
`ExactChangeBinding`, associated mutation-class/scope write requirements,
policy version, and authoritative semantic gate.

Before use, approval can be revoked by its approver, a referenced Grant issuer,
or the trusted authorization authority. Loss of verifier use/revocation state
fails closed. An ephemeral or session-bound approval becomes invalid when its
verifier state no longer exists. A stale or re-proposed patch requires a new
Approval. Replay of a consumed Approval returns a machine-distinguishable
denial.

#29 owns concrete state storage and atomic consume-with-publication mechanics.

### 9. Approval becomes unusable without publication

Execute fails without publication when any of these holds:

1. `ProposalId` differs;
2. `ExactChangeBinding` differs;
3. the semantic base is stale;
4. the originator differs;
5. the executor differs;
6. the rederived associated mutation-class/scope write requirement differs;
7. the authorization-policy version is unsupported or mismatched;
8. Approval expired;
9. Approval was revoked;
10. Approval was already consumed;
11. the approver is not a trusted Human;
12. an authorizing Approve Grant reference is no longer valid or covering;
13. the executor's Execute Grants are no longer sufficient; or
14. the authoritative semantic gate rejects publication.

Approval may remain as historical evidence after becoming stale or unusable,
but cannot authorize another proposal or base.

### 10. Execute is a conjunction, not a shortcut

Before delegated publication, all of these independent requirements hold:

```text
supported immutable SemanticPatch
AND exact current base
AND authenticated executor
AND sufficient live Execute Grants
AND complete associated mutation-class/scope coverage for Execute
AND valid active Human Approval
AND live approver authority
AND authoritative semantic gate allows
AND no unauthorized external effect
```

Failure publishes no semantic state, advances no semantic revision, consumes
no Approval before successful publication, and performs none of the requested
storage, network, Git, process, or other host effects. Separately authorized
security/audit recording of the denial remains possible under #30/audit policy
and is not semantic publication or authority for the denied effect. Semantic
validity never grants authority; authorization never overrides semantic
failure.

### 11. Minimum proposal and execution provenance crosses adapter boundaries

Proposal provenance preserves at least:

- authorization domain;
- proposal ID and `ExactChangeBinding` reference;
- originating `PrincipalId`;
- Propose Grant references;
- `AuthorizationFootprint`; and
- authorization-policy version.

For AI-originated proposals, provider/model/tool/prompt-correlation facts should
be retained when available as opaque provenance. Full prompt or conversation
storage is not required, and those facts never grant privilege.

Successful Execute provenance preserves at least:

- proposal ID and exact-binding reference;
- originator, executor, and approver principals;
- Approval ID;
- Execute and Approve Grant references;
- mutation classes and canonical write scope;
- authorization-policy version;
- base semantic revision;
- resulting semantic revision;
- authoritative gate/report reference; and
- Approval terminal state `Consumed`; and
- agent/provider/model/tool facts at execution when known.

These facts may attach to an execution receipt or history record and cross
adapter boundaries. They are not semantic Document data. Exact durable storage,
receipt DTO, retention, and recovery links remain #29/#12 follow-up. Event
sourcing, CRDT, and a general operation-log protocol are not required. Denials
need machine-readable outcomes; durable denial logging remains #30/audit-policy
scope.

### 12. Semantic authorization never authorizes external effects

Semantic Grants and Approvals do not authorize filesystem or `.roproj` access,
network, process/shell, Git commit/push/merge, plugin invocation, deployment,
credentials, or another host effect. External effects belong to a separate
capability domain.

Authorization failures remain separate from semantic `ValidationReport`.
Principal unknown, capability denied, scope denied, approval required, approval
binding mismatch, approval expired/revoked/already used, approver authority
lost, stale proposal, and external effect denied retain machine-distinguishable
meaning. Exact symbolic codes, Rust enums, and wire representations remain
Provisional. A denial must not disclose semantic content outside Query
authority.

### 13. Ownership remains outside semantic-core Document meaning

- ADR-0024/#27 owns proposal identity, immutability, `ExactChangeBinding`, base,
  and stale semantics.
- ADR-0026/#28 owns Principal, capabilities, Grant, scope,
  `AuthorizationFootprint`, Approval semantics, expiry/replay/revocation, and
  minimum provenance.
- The trusted identity/host boundary owns Principal occurrence issuance and
  resolution and must preserve non-reassignment across its account lifecycle;
  exact account/provider mechanisms remain Provisional.
- #29 owns proposal/Approval lifecycle integration, Approval state
  implementation, atomic consume+publication, and execution receipts.
- #30 owns instruction/data separation, prompt-injection boundaries, raw
  mutation bypass prevention, external-effect enforcement, and security tests.
- #93 owns concrete semantic revision token, resident session, revision
  advance, concurrency, and state installation.
- #11 owns broader enterprise/team permissions, transaction/recovery, and
  reusable policy questions.
- #12/history work owns persisted history, event sourcing, undo, and retention
  architecture.
- storage/host may materialize only an already-authorized semantic result under
  separate host authority.

Authorization state and types do not become `semantic-core` Document meaning.
Exact crate/module placement remains Provisional. Trusted enforcement must not
exist only in `ai-api`, UI, or client convention.

## Required pressure tests

Future implementation must preserve these representation-neutral outcomes:

1. **Read-only principal** — covered Query succeeds; Propose and Execute deny.
2. **Proposal-only principal** — covered Propose succeeds without publication;
   Execute denies.
3. **Approved ordinary value update** — complete Value/write-scope authority,
   exact Human Approval, exact base, and successful gate publish exactly once.
4. **Denied formula update** — Value-only authority cannot authorize Formula.
5. **Denied destructive update** — non-destructive authority cannot authorize
   semantic fact loss.
6. **Changed proposal after approval** — binding mismatch denies and requires a
   new proposal/Approval.
7. **Stale base after approval** — ADR-0024 Stale denies before publication and
   requires a new proposal/Approval.
8. **Expired approval** — denies without publication.
9. **Revoked approval** — denies without publication.
10. **Replayed consumed approval** — returns the distinct replay denial.
11. **Provider/model change** — does not change privilege when the trusted
    principals/binding are unchanged, but is retained as provenance.
12. **External side effect under semantic-only authority** — denies in the
    separate effect domain.
13. **Approver or executor authority lost after approval** — an expired or
    revoked authorizing Approve Grant, or insufficient live Execute Grants,
    denies; an equivalent new Approve Grant does not revive Approval, while a
    different live Execute Grant set may satisfy the fresh executor recheck.
14. **Semantic gate failure despite valid authorization** — denies without
    publication and leaves Approval Active when no publication occurred.
15. **Multi-operation batch** — requires the union of associated write
    requirements and one whole-batch Approval; no prefix publishes.
16. **Cross-paired Grant or Approval** — Formula authority for one field plus
    Value authority for another cannot authorize Formula on the Value-scoped
    field or Value on the Formula-scoped field, and Approval cannot flatten the
    two pairs into independently unioned scope and class sets.
17. **Revoked Grant occurrence reissued** — reusing or reactivating a revoked
    GrantId is forbidden; a new equivalent Grant does not revive Approval that
    references the revoked occurrence.
18. **Originator/executor role disjuncts** — Delegated origin with a Human
    executor and Human origin with a Delegated executor both require Approval;
    substituting either bound principal denies.
19. **Principal replacement or alias reuse** — deleting, transferring,
    recreating, or replacing an account never reassigns its PrincipalId; a
    replacement subject receives a new PrincipalId and cannot inherit the
    original occurrence's Grants, Approval originator/executor bindings, or
    provenance through a reused login, email, provider identifier, or alias.

## Stability classification

Accepted:

- opaque, non-reusable, non-reassignable Principal occurrences within an
  authorization domain;
- trusted Human versus Delegated distinction for MVP policy;
- default deny and explicit Grants;
- non-reusable immutable Grant occurrences with terminal revocation;
- Query/Propose/Execute/Approve non-implication;
- Value/Formula/Structure/Schema/Destructive distinctions;
- stable-ID, document-local scope concepts and finite-union Grants;
- trusted `AuthorizationFootprint` derivation;
- associated mutation-class/scope coverage, combined with the requested action,
  without crossed-Grant unions;
- Human Approval for Delegated-origin or Delegated-authority publication;
- exact originator/executor/proposal/associated-write-requirement/policy
  binding;
- authorizing Approve Grant references remain live, with fresh executor
  authority rechecks before Execute;
- finite Approval consumed atomically with at most one successful publication;
- revocation, fail-closed lost state, and replay denial;
- trusted structural exact binding for the MVP;
- minimum proposal/execution provenance; and
- semantic/external-effect and authorization/validation separation.

Provisional:

- exact type, field, capability, and denial-code names;
- ID encodings and generators;
- Grant/Approval/provenance DTOs and storage;
- exact duration values and clock representation;
- complete operation catalogue and class mappings;
- crate/module placement; and
- result and wire formats.

Deferred:

- canonical bytes, digest/hash/signature/MAC, and portable approval tokens;
- project/workspace/organization scope;
- reusable/standing policies;
- quorum or multi-party approval;
- enterprise RBAC, SSO/SCIM, and tenancy;
- durable audit ledger, retention, and event sourcing;
- external-effect capability vocabulary; and
- concrete session/revision mechanics.

## Rejected alternatives

### Broad editor roles as the authority primitive

Rejected. `editor=true` hides schema, destructive, and external authority and
cannot express least privilege.

### Document-only reusable Grant scope

Rejected as the only scope primitive. Document scope remains the broad top
atom, but it cannot bound Query disclosure or least-privilege Entity,
EntityField, Schema, and SchemaField operations. The closed document-local
stable-ID atoms provide that bounded scope without introducing project,
organization, wildcard, or predicate policy languages.

### Provider or model identity as privilege

Rejected. Provider/model identity is provenance, never authorization or
semantic truth.

### Client-declared scope or generic policy/scope DSL

Rejected. Trusted typed-command analysis, closed semantic scope atoms, and
explicit Grants are sufficient for the MVP.

### Approval of chat prose or rendered diff alone

Rejected. Presentation cannot replace the immutable proposal occurrence and
structurally verified ADR-0024 `ExactChangeBinding`.

### Binding every Propose, Approve, and Execute GrantId into ApprovalBinding

Rejected. The exact proposal, originator, executor, associated
mutation-class/scope write requirements, and policy version are the smaller
stable authorization context. Authorizing Approve Grant references remain
immutable issuance and revocation dependencies, while Execute authority is
freshly rederived and may be satisfied by another live Grant set. Propose
authority is checked when the proposal is issued and retained as provenance;
it is not later execution authority.

### Selecting a digest or portable approval token now

Rejected for the MVP. A trusted proposal store and structural binding satisfy
the current boundary. Cross-boundary integrity needs a separately accepted
profile rather than accidentally freezing bytes, hashing, signing, or wire
semantics here.

### Fixing an Approval TTL recommendation now

Rejected. Finite expiry is the safety law needed by the MVP. A concrete default
or maximum duration depends on the future trusted clock, lifecycle, and host
security profile, so ADR-0026 deliberately recommends no duration.

### Persisting authorization state in the semantic Document

Rejected. Authorization and history evidence are neighboring application/host
concerns, not canonical product-semantic state.

### Consuming approval before publication

Rejected. Failure before semantic publication does not consume Approval;
successful publication and consumption are atomic under #29.

## Consequences

Positive:

- trusted code can authorize an operation batch without parsing prompts;
- read/disclosure, proposal, execution, approval, scope, and mutation risk are
  explicit and independently bounded;
- one Human Approval covers exactly one immutable proposal occurrence and
  authorization context;
- stale, changed, expired, revoked, and replayed approval fails closed;
- provider/model evolution remains replaceable; and
- semantic permission cannot silently become host-effect authority.

Costs:

- trusted code must derive disclosure and write footprints from typed semantic
  meaning and relevant base/candidate relationships;
- Delegated-origin or Delegated-authority publication needs live Grant checks
  and exact finite Human Approval;
- approval state and publication must be coordinated atomically; and
- minimum provenance must survive outside canonical semantic state.

## Required follow-up

- #29 implements proposal/Approval lifecycle integration, Approval state,
  atomic consume+publication, and execution receipts without changing these
  laws.
- #30 implements instruction/data separation, bypass prevention,
  external-effect enforcement, safe denials, and security tests.
- #93 supplies concrete session/revision/concurrency mechanics without changing
  proposal or authorization meaning.
- #11/#12 retain broader permissions, reusable policies, transaction/recovery,
  and history architecture.
- Issue #28 receives a Decision Capsule only after this authority package is
  reviewed and merged.

## Related

- Product Constitution §§2.5 through 2.7, 6, 7
- Design Principles §§7 through 10, 12
- ADR-0007
- ADR-0015
- ADR-0019
- ADR-0020
- ADR-0022
- ADR-0024
- Issues #11, #12, #28, #29, #30, #93
