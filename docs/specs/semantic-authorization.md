# Semantic Authorization, Exact Approval, and Provenance Specification

Decision state: Accepted under
[ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-exact-approval.md).

Implementation state: Not implemented. Current provider-free AI operations are
read/explain/analyze/suggest-only, and the current `Suggestion` DTO is not a
SemanticPatch, grant, approval, or execution credential. Concrete lifecycle,
registry, consumption, runtime revision, enforcement, and transport work remain
owned by #29, #30, and #93.

Decision issue: [#28](https://github.com/nurockplayer/tachiko-work/issues/28)

Related authority:
[ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md),
[ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md),
[ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md),
[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md),
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md),
and
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md).

## Purpose

Define the smallest provider-neutral authorization and approval contract needed
for safe Machine/AI-originated SemanticPatch execution in the current MVP.

This specification lets an implementation decide, without interpreting natural
language:

- which principal may Query, Propose, Approve, or Execute;
- which semantic document and mutation classes that authority covers;
- whether explicit approval is required;
- the exact proposal, semantic base, executor, and authorization context an
  approval covers;
- what makes approval stale, expired, revoked, or consumed;
- what minimum provenance must survive; and
- where semantic authority stops before persistence or another host/external
  effect begins.

It consumes ADR-0024 `SemanticPatch` and `ExactChangeBinding` exactly. It does
not define another command vocabulary, generic policy language, public wire
DTO, or runtime/session protocol.

## Contract boundary

Conceptually:

```text
trusted principal/session resolution
              |
              v
principal + grants
              |
              v
Query / Propose / Approve / Execute authorization
              |
              v
immutable revision-pinned SemanticPatch
              |
              v
exact Human approval when required
              |
              v
authoritative base check + semantic gate
              |
              v
single-use authorized semantic publication
              |
              v
result revision + minimum provenance

separate host domains beside this path:
storage / filesystem / browser persistence / network / Git / process / plugin
```

Semantic admissibility, deterministic operation gating, authorization, and
approval are separate mandatory checks. Passing one does not satisfy another.

## Terminology

### Principal

An accountable authorization subject resolved by a trusted boundary.

```text
Principal
- PrincipalId
- PrincipalKind: Human | Machine
```

The exact type names and encodings are conceptual and Provisional.

### Grant

An immutable, revocable occurrence that grants one principal one semantic
action over one exact DocumentId, with mutation classes where the action can
address mutation.

### Approval

An immutable Human authorization occurrence for one exact proposal occurrence,
exact semantic change/base, exact Machine executor, exact mutation classes, and
exact grant context.

### Direct semantic mutation

The canonical semantic subjects and definitions a Command or AtomicBatch
actually creates, changes, or removes.

Derived formula results, dependency impacts, validation changes, and rendered
review evidence are not additional direct writes, though they remain required
review/provenance evidence where the owning Semantic API operation exposes them.

## Principal contract

1. A trusted host/authentication/session boundary MUST supply the effective
   PrincipalId for every authorization-relevant request.
2. Request payloads, model output, prompts, document text, imported content, or
   plugin results MUST NOT select or upgrade the effective principal.
3. A Machine principal includes an AI agent, automation, or service integration.
4. Provider, model, tool, framework, prompt, confidence, and self-reported
   validation are provenance/evidence only.
5. Proposer, approver, and executor MUST be recorded separately.
6. A Machine principal MUST NOT issue the Human approval required for its own
   execution.
7. Disabled, missing, or unresolvable principals fail closed.
8. Human/Machine subtype expansion, identity proof, accounts, directories,
   groups, organizations, and enterprise identity management are outside this
   contract.

## Semantic action contract

The closed MVP actions are:

```text
Query
Propose
Approve
Execute
```

### Query

Query authorizes deterministic non-publishing Semantic API reads for the scoped
document. This includes inspection, validation/report inspection, explanation,
calculation, comparison, semantic diff/impact, and other operations that do not
publish semantic state.

### Propose

Propose authorizes `Propose(Command | AtomicBatch)` for covered mutation
classes. It may issue an immutable ADR-0024 SemanticPatch and review evidence.
It publishes no semantic state.

### Approve

Approve authorizes a Human principal to issue or revoke exact approval for
covered mutation classes in the scoped document. It does not grant Execute.

### Execute

Execute authorizes a principal to request publication of an exact
SemanticPatch for covered mutation classes. It does not bypass required Human
approval, base equality, semantic preconditions, or the authoritative operation
gate.

### Non-implication laws

- Query MUST NOT imply Propose, Approve, or Execute.
- Propose MUST NOT imply Execute.
- Approve MUST NOT imply Execute.
- Execute MUST NOT imply Approve.
- Provider/model identity MUST NOT imply any action.
- Unknown actions fail closed.

## Mutation-class contract

The closed MVP classes are:

```text
Data
Formula
Schema
Destructive
```

### Data

Ordinary non-formula semantic data creation/update, typed reference-value
update, non-destructive entity metadata or human-key change, and other mutation
that does not alter formula-definition or schema-declaration meaning.

### Formula

Creation, replacement, or removal of a bound formula definition or
formula-bearing semantic value.

### Schema

Creation, replacement, or removal of schema/field declaration meaning,
including declared type, requiredness, or reference-target semantics.

### Destructive

Deletion or irreversible discard/replacement of established semantic objects or
canonical data.

### Classification laws

1. Every Stable mutation Command family MUST publish one deterministic required
   class set as part of its Semantic API compatibility contract.
2. Classification MUST follow typed command meaning rather than prompt text,
   provider/model metadata, confidence, rendered diff, or UI presentation.
3. Classes are additive and non-hierarchical.
4. Entity removal requires at least `Data + Destructive`.
5. Formula removal requires at least `Formula + Destructive`.
6. Schema/field removal requires at least `Schema + Destructive`.
7. AtomicBatch requires the union of every member command's classes.
8. Every required class must be independently granted and approved.
9. Unknown, unsupported, or unclassified mutation commands fail closed.
10. A Data-only class set is ordinary/routine review presentation. A set
    containing Formula, Schema, or Destructive is elevated presentation. Risk
    presentation does not replace the exact class set.

## Scope and Grant contract

The only reusable semantic scope in this MVP profile is one exact `DocumentId`.

Conceptually:

```text
Grant
- GrantId
- PrincipalId
- SemanticAction
- DocumentId
- allowed MutationClass set for Propose/Approve/Execute
- issued_by
- issued_at
- optional expires_at
```

Normative laws:

1. A Grant is immutable after issuance.
2. Changing principal, action, document, or class set creates a new GrantId.
3. Grant state is held by a trusted authorization boundary and is at least
   revocable.
4. Authorization is default-deny.
5. Query requires one active Query grant for the exact DocumentId.
6. Propose, Approve, and Execute require active grants covering the exact
   DocumentId and every required mutation class.
7. A grant for one class does not imply another class.
8. A grant for one document does not authorize another document.
9. Document scope is semantic identity and MUST NOT be inferred from a path,
   filename, project name, UI tab, `.roproj` location, Git branch, commit, or
   repository.
10. A revoked, expired, disabled, missing, or unresolvable grant grants no
    authority.
11. Grant state MUST be rechecked when approval is issued and immediately
    before Execute.
12. Roles, groups, inheritance, deny rules, conditions, wildcard expressions,
    tags, path predicates, arbitrary scripts, and generic policy DSLs are not
    supported.
13. Entity-, schema-, field-, project-, workspace-, branch-, organization-, and
    tenant-scoped reusable grants remain Deferred.

Exact proposal approval narrows document-level reusable authority to one
occurrence and the complete stable-ID-targeted command semantics in
ExactChangeBinding.

## Machine execution approval policy

The current MVP rule is:

```text
Machine Execute => exact Human approval required
```

An approval may be issued only when:

1. the immutable proposal occurrence and ExactChangeBinding are internally
   consistent;
2. the proposal base equals the current semantic context revision;
3. the approver is an active Human principal;
4. the approver is not the Machine executor;
5. active Approve grants cover the proposal DocumentId and every required class;
6. the named executor is active;
7. active Execute grants cover the same DocumentId and every required class;
8. the approval has finite expiry; and
9. the selected authorization/approval profile is supported.

All Machine Execute requests require approval, including ordinary Data writes.
There is no Formula, Schema, Destructive, provider, model, confidence, or
validation-success bypass.

This profile does not add an approval requirement to ordinary directly
authenticated Human editing paths. A host may impose a stricter policy without
weakening this Machine rule.

## Approval object contract

Conceptually:

```text
Approval
- ApprovalId
- ProposalId
- ExactChangeCommitment
- SemanticApiCompatibilityContract
- BaseReference
- DocumentId
- RequiredMutationClasses
- ApproverPrincipalId
- ApproverGrantIds
- ExecutorPrincipalId
- ExecutorGrantIds
- AuthorizationProfileId
- IssuedAt
- ExpiresAt
```

The exact source/wire names are not stable.

Normative laws:

1. ApprovalId identifies one approval occurrence and MUST NOT be reused.
2. Approval contents are immutable after issuance.
3. Approval binds one ProposalId and cannot transfer to another occurrence even
   when its exact semantic contents are identical.
4. Approval binds the complete ADR-0024 ExactChangeBinding, including Semantic
   API contract, exact base, body kind, complete typed command semantics,
   generated IDs, bound formulas, command-owned semantic preconditions, and
   AtomicBatch order.
5. Approval binds one exact executor principal.
6. Approval binds one exact Human approver principal.
7. Approval binds the exact GrantIds relied upon by the approver and executor.
8. Approval binds the exact DocumentId, mutation-class set, and authorization
   profile.
9. Approval has finite ExpiresAt.
10. Mutable status is maintained by a trusted registry and is at least
    distinguishable as active, revoked, and consumed.
11. Rendered diff prose, intent text, prompt, confidence, provider/model
    identity, UI coordinates, `.roproj` bytes, storage paths, and Git objects
    MUST NOT substitute for exact binding.
12. A client-supplied approval record is untrusted. The authoritative boundary
    MUST reload the trusted Approval by ApprovalId.
13. ApprovalId is a registry reference, not a portable bearer capability.

## Exact-change integrity contract

The Accepted logical law is:

```text
Approval.ExactChange == ExactChangeBinding(SemanticPatch)
```

A trusted implementation MAY establish this through structural comparison with
one retained immutable proposal record.

When proposal and approval are separately persisted/decoded, the implementation
MUST additionally use a versioned, domain-separated, collision-resistant
commitment over the complete logical ExactChangeBinding.

The commitment profile:

- MUST cover every identity-defining ADR-0024 field;
- MUST preserve AtomicBatch order;
- MUST encode typed values, normalized Numbers, bound ASTs, generated stable
  IDs, and command-owned semantic preconditions without relying on
  presentation;
- MUST identify its profile and algorithm;
- MUST reject unsupported profiles;
- MUST NOT be a hash of only ProposalId, rendered diff, Rust memory,
  Serde/transport bytes, `.roproj` bytes, Git objects, or package digests; and
- MUST NOT be treated as proof of approver identity or authorization.

Preferred first implementation mechanism, classified Provisional:

```text
profile: tachiko.semantic-exact-change/v1
algorithm: SHA-256
topology: tagged, length-delimited, versioned internal transcript
```

Exact transcript bytes and future algorithm migration remain replaceable. A
portable/offline approval format would require a separately Accepted MAC,
signature, or authenticated protocol.

## Validity, expiry, revocation, replay, and stale behavior

An approval is usable only if every condition below remains true:

```text
proposal occurrence matches
AND exact-change commitment matches
AND current semantic base exactly matches proposal base
AND Semantic API / authorization profiles are supported
AND approver and executor principals are active
AND executor identity matches
AND every bound approver/executor GrantId is active and covering
AND approval is not expired
AND approval is not revoked
AND approval is not consumed
```

### Expiry

- Every approval MUST have finite ExpiresAt.
- Permanent approval is invalid.
- Exact TTL and maximum TTL are Provisional host policy.
- A 15-minute default is recommended for the first implementation.
- Trusted time belongs to the host/authorization boundary, not workspace-engine.
- If trusted time or approval status cannot be determined, authorization fails
  closed.

### Revocation and grant changes

- Approval MUST be explicitly revocable before consumption.
- Revoking, expiring, replacing, or disabling any bound grant invalidates the
  approval.
- A newly issued semantically equivalent GrantId does not revive it.
- Disabling the approver or executor invalidates the approval.
- Revocation does not undo an already published transition.

### Stale interaction

Stale behavior remains exactly ADR-0024:

- compare the current semantic revision with the proposal base before
  re-authorizing or executing;
- return Stale on mismatch before candidate construction against the changed
  base;
- publish no semantic state;
- perform no implicit rebase, merge, retarget, or best-effort replay;
- leave the proposal unchanged; and
- require a new proposal and new approval.

Later semantic content equivalence does not revive the old revision occurrence.

### Replay and consumption

- Approval is single-use.
- After all identity, integrity, base, principal, grant, expiry, revocation, and
  semantic-gate checks pass, the trusted boundary MUST claim/consume the
  approval before or atomically with entry into the semantic publication path.
- At most one concurrent attempt may claim an ApprovalId.
- A claimed/consumed approval MUST NOT be restored after a later execution or
  host failure.
- A second attempt with the same ApprovalId MUST fail without publication.
- Exact reservation, locking, transaction, and state-installation mechanics are
  owned by #29/#93.

## Authorization algorithms

### Authorize Query

```text
allow iff:
  principal is active
  AND active Query grant covers exact DocumentId
```

### Authorize Propose

```text
allow iff:
  principal is active
  AND active Propose grants cover exact DocumentId
  AND active Propose grants cover every RequiredMutationClass
  AND command/base are otherwise admissible under the Semantic API
```

Propose publishes nothing and does not imply later Execute authorization.

### Issue Approval

```text
allow iff:
  immutable proposal identity/content are consistent
  AND proposal base is current
  AND approver is active Human
  AND approver != Machine executor
  AND exact active Approve GrantIds cover document + classes
  AND exact active Execute GrantIds for the named executor cover document + classes
  AND finite expiry and supported profiles are recorded
```

### Authorize Execute

A conforming trusted boundary preserves this logical order:

```text
1. Load immutable proposal and trusted Approval by ID.
2. Reject unsupported Semantic API, commitment, or authorization profiles.
3. Verify proposal identity/content consistency and exact-change commitment.
4. Compare current semantic revision with proposal base; return Stale on mismatch.
5. Recompute required mutation classes from typed command semantics.
6. Require authenticated actor == bound executor principal.
7. Recheck bound principals and exact GrantIds.
8. Recheck expiry, revocation, and consumption.
9. Require exact Approval binding equality.
10. Re-run authoritative semantic preconditions, validation/calculation, and gate.
11. Atomically claim/consume approval and enter all-or-nothing publication.
12. Return outcome, resulting revision when successful, and minimum provenance.
```

An earlier preview, rendered diff, validation result, or client-side allow/deny
calculation is not authority for step 10.

## Minimum provenance contract

Provenance is machine-readable audit/history evidence outside canonical
semantic Document state.

### Proposal provenance

At minimum:

```text
ProposalId
ExactChangeCommitment + profile
Semantic API compatibility contract
BaseReference
ProposerPrincipalId + PrincipalKind
ProposedAt
```

When available at the adapter boundary, structured agent instance, provider,
model, tool/orchestrator identity/version, and correlation ID are also retained.
They remain non-authoritative.

### Approval provenance

At minimum:

```text
ApprovalId
ApproverPrincipalId
ApproverGrantIds
ExecutorPrincipalId
ExecutorGrantIds
AuthorizationProfileId
DocumentId
RequiredMutationClasses
IssuedAt
ExpiresAt
Revocation/consumption evidence
```

### Successful Machine execution provenance

At minimum:

```text
ProposalId
ExactChangeCommitment/profile
BaseReference
ProposerPrincipalId
ApprovalId
ApproverPrincipalId
ExecutorPrincipalId
bound GrantIds
AuthorizationProfileId
DocumentId
RequiredMutationClasses
ExecutedAt
final authoritative gate outcome or durable evidence reference
resulting semantic revision
execution outcome
actual agent/provider/model/tool snapshot when known
```

Additional laws:

1. The complete immutable proposal or a durable lossless reference to it MUST
   survive; a digest alone is insufficient to explain what was approved.
2. Provider/model changes with the same trusted executor PrincipalId do not
   invalidate authority but MUST be reflected in provenance.
3. A different resolved executor PrincipalId invalidates approval.
4. Full prompts, hidden reasoning, credentials, secrets, and complete chat
   transcripts are not minimum provenance.
5. Provenance MUST NOT be written into `.roproj` merely to make it durable.
6. This specification does not require event sourcing, a universal operation
   log, or tamper-evident ledger.
7. Provenance DTO, retention, storage, redaction, and UI remain
   Provisional/Deferred.

## Semantic and external effect separation

The action and grant vocabulary in this specification authorizes only the
Semantic API domain.

It does not authorize:

- filesystem or browser persistence reads/writes;
- `.roproj` or `.ro` materialization;
- network requests;
- shell or process execution;
- Git commit/push/repository administration;
- plugin execution;
- deployment/publication;
- credentials/secrets access; or
- another host effect.

A host may materialize or publish an already-authorized semantic result only
under separate host authority. Storage/host code does not grant semantic
permission, and semantic approval does not grant host authority.

## Failure meanings

A conforming client must be able to distinguish these authorization/security
meanings where applicable:

- principal unavailable or disabled;
- action capability missing;
- document scope mismatch;
- mutation class denied or unknown;
- approval required/missing;
- Machine/self approval rejected;
- proposal occurrence mismatch;
- exact-change commitment mismatch;
- executor mismatch;
- authorization profile unsupported;
- approval expired;
- approval revoked;
- approval consumed/replayed;
- bound grant unavailable/revoked/expired/replaced; and
- approval registry/time/authentication state unavailable.

These remain distinct from ADR-0024 Stale, admission/construction failure,
semantic precondition failure, authoritative gate rejection, representation
failure, and host failure.

Stable machine meaning is required; exact diagnostic code strings, Rust enums,
transport tags, and message wording remain Provisional and are enforced under
#30.

## Required conformance scenarios

1. Query-only principal cannot Propose, Approve, or Execute.
2. Propose-only principal can issue an inert covered proposal but cannot Execute.
3. Covered ordinary Data proposal with exact Human approval publishes once.
4. Data-only authority denies Formula mutation.
5. Non-destructive authority denies entity/schema/formula removal.
6. Changed target/value/formula/generated ID/base/body/order/contract requires a
   new proposal and approval.
7. Identical ExactChangeBinding under a different ProposalId cannot reuse
   approval.
8. Any intervening semantic publication makes proposal/approval stale.
9. Expired approval denies.
10. Explicitly revoked approval denies.
11. Revoked/replaced bound grant denies; equivalent new GrantId does not revive.
12. Consumed ApprovalId denies replay and concurrent double claim.
13. Same executor principal with changed provider/model retains authority and
    records new provenance.
14. Different executor PrincipalId denies.
15. Machine self-approval denies.
16. Valid approval cannot override failed semantic gate.
17. Semantic grants cannot authorize filesystem/network/process/Git/plugin/
    deployment/persistence effects.
18. Mixed AtomicBatch requires the union of every mutation class and publishes
    no prefix.

## Stability classification

| Concept | State |
| --- | --- |
| Opaque trusted Principal and Human/Machine distinction | Accepted |
| PrincipalId encoding/authentication mechanism | Provisional host concern |
| Query, Propose, Approve, Execute meanings and non-implication | Accepted |
| Exact capability identifier spelling/wire form | Provisional |
| Data, Formula, Schema, Destructive class meanings | Accepted MVP contract |
| Stable command-family class mapping | Semantic API profile-specific; published meaning must not change silently |
| DocumentId-only reusable semantic grant scope | Accepted MVP profile |
| Entity/schema/field/project/workspace/org reusable scopes | Deferred |
| Immutable, revocable, default-deny Grant occurrences | Accepted |
| Grant registry/admin/DTO | Provisional |
| Exact Human approval for every Machine Execute | Accepted current MVP policy |
| Approval binds occurrence, ExactChangeBinding/base, executor, classes, profile, and exact GrantIds | Accepted |
| Structural equality as approval authority | Accepted |
| Versioned domain-separated collision-resistant commitment when separately stored/decoded | Accepted |
| SHA-256 tagged transcript first profile | Provisional recommended mechanism |
| Exact transcript bytes and public digest/wire format | Provisional |
| ApprovalId as trusted registry reference, not bearer credential | Accepted MVP profile |
| Portable signed/MAC/offline approvals | Deferred |
| Finite expiry | Accepted |
| Exact TTL/clock implementation | Provisional |
| Explicit revocation and fail-closed unverifiable state | Accepted |
| Single-use approval / replay denial | Accepted |
| Reservation/locking/atomic consumption mechanism | #29/#93 Provisional implementation |
| Minimum proposal/approval/execution provenance content | Accepted |
| Provenance store/retention/redaction/tamper evidence/UI | Provisional/Deferred |
| Provider/model as provenance rather than privilege | Accepted under ADR-0007 |
| Semantic and host/external effect separation | Accepted under ADR-0007/ADR-0022 |
| Host/external capability vocabulary | Deferred |
| Roles/groups/ABAC/policy DSL/SSO/SCIM/tenancy | Deferred |
| Auto-approval/autonomous mutation/multi-party approval | Deferred |
| Event sourcing/operation log/undo/recovery protocol | Deferred |
| Public Rust/Serde/wire authorization DTO | Deferred |

## Ownership boundaries

- ADR-0026 / this specification own authorization and exact approval semantics.
- ADR-0024 owns proposal occurrence, ExactChangeBinding, base, and stale laws.
- ADR-0020 owns Query/Command/Propose/Execute and atomic semantic publication.
- #29 owns lifecycle, registry, approval claim/consumption, apply/verify, and
  provenance persistence.
- #30 owns trusted enforcement, instruction/data separation, bypass prevention,
  host-effect denial, and machine-readable security diagnostics.
- #93 owns concrete resident session/revision/concurrency mechanics.
- workspace-engine remains the shared semantic transition/gate authority.
- host/storage adapters remain separate effect authorities.

## Explicit non-goals

- enterprise RBAC administration;
- groups, organizations, tenants, SSO, or SCIM;
- generic authorization or policy-expression language;
- reusable automatic approval rules;
- autonomous mutation without Human approval;
- multi-party approval chains;
- portable bearer approval tokens or signing infrastructure;
- plugin/network/filesystem/process/Git capability design;
- public Rust/Serde/JSON/IPC/network DTO freeze;
- revision/session/concurrency implementation;
- approval UI;
- event sourcing, universal operation log, undo, or rollback; or
- production implementation in this documentation decision.
