# ADR-0026: Scoped semantic authorization, exact approval, and provenance

## Status

Accepted

Decision issue: [#28](https://github.com/nurockplayer/tachiko-work/issues/28)

Specified by: [`semantic-authorization.md`](../specs/semantic-authorization.md)

Related authority: ADR-0007, ADR-0015, ADR-0019, ADR-0020, ADR-0022, ADR-0024

Related implementation: [#29](https://github.com/nurockplayer/tachiko-work/issues/29), [#30](https://github.com/nurockplayer/tachiko-work/issues/30), [#93](https://github.com/nurockplayer/tachiko-work/issues/93)

## Context

ADR-0007 establishes that AI is a delegated semantic client with no intrinsic
authority. Semantic admissibility, deterministic operation gating, delegated
authorization, and required approval are independent prerequisites for
canonical semantic publication. Provider, model, tool, prompt, confidence, and
self-reported validation are provenance or evidence rather than privilege.

ADR-0020 establishes one transport-neutral Semantic API with typed `Query`,
`Command`, `Propose`, `Execute`, and ordered all-or-nothing `AtomicBatch`
semantics. Every operation or operation family is capability-addressable, but
ADR-0020 deliberately does not define principals, grants, approval, provenance,
or integrity tokens.

ADR-0024 establishes one immutable revision-pinned `SemanticPatch` occurrence
around `Propose(Command | AtomicBatch)`. It fixes proposal immutability,
Semantic API compatibility binding, exact semantic-base binding,
`ExactChangeBinding`, and fail-closed stale behavior without selecting a digest,
approval protocol, or runtime implementation.

The current implementation exposes provider-free read/explain/analyze behavior
and inert validated one-field suggestions. It has no general SemanticPatch,
AtomicBatch, principal registry, capability grants, exact approval object,
replay protection, or durable provenance contract. The current
`requires_approval` boolean is implementation evidence for the MVP safety
posture, not an authorization protocol.

Issue #28 therefore needs the smallest provider-neutral authorization contract
that lets implementation decide who may read, propose, approve, or execute;
what semantic authority they hold; what exact transition a human approved; and
when that approval is no longer usable. It must not turn the MVP into an
enterprise IAM or generic policy-language product.

## Decision

### 1. Principals are opaque authorization subjects

Every authorization-relevant action is attributed to one trusted principal:

```text
Principal
- opaque PrincipalId
- PrincipalKind: Human | Machine
```

`PrincipalId` is resolved by a trusted host, authentication, or session
boundary. A request body, model output, prompt, document, plugin result, or
self-reported identity cannot mint or replace the authenticated principal.

`Machine` includes AI agents, automation, and service integrations. Provider,
model, agent framework, tool, prompt, and confidence metadata do not determine
principal privilege. They may be recorded as provenance.

Proposal originator, approver, and executor are separate principal roles even
when a future workflow allows the same human principal to occupy more than one
role. During the current MVP, approval satisfying an AI/Machine execution must
come from a `Human` principal. A Machine principal cannot self-approve.

Principal identifier encoding, authentication mechanism, account lifecycle,
and finer principal-kind taxonomy remain Provisional host concerns.

### 2. Use four independent semantic actions

The closed MVP action vocabulary is:

```text
Query
Propose
Approve
Execute
```

- `Query` performs non-publishing Semantic API reads, including inspection,
  validation/report inspection, explanation, calculation, comparison, and
  impact analysis.
- `Propose` evaluates typed semantic intent and may issue an immutable
  SemanticPatch without publishing semantic state.
- `Approve` issues or revokes exact human approval for one proposed semantic
  execution.
- `Execute` requests authoritative semantic publication of one exact
  SemanticPatch.

No action implies another. Query does not imply Propose. Propose does not imply
Execute. Approve does not imply Execute. Execute does not bypass approval.

Exact capability identifier spelling and public DTO representation remain
Provisional. The action meanings and non-implication laws are Accepted.

### 3. Reusable grants are document-scoped and default-deny

The MVP grant is an immutable, revocable authorization occurrence:

```text
Grant
- opaque GrantId
- PrincipalId
- semantic action
- exact DocumentId scope
- allowed mutation classes where applicable
- optional finite expiry
```

The only reusable semantic scope accepted for the current MVP is one exact
semantic `DocumentId`. A principal needing access to multiple documents needs
separate grants. Scope is semantic identity, not a path, filename, UI tab,
`.roproj` location, Git branch, or project/workspace name.

`Query` grants require only the document scope. `Propose`, `Approve`, and
`Execute` grants additionally declare the mutation classes they cover.

Authorization is allow-only and default-deny. All required grants must be
active and unexpired when authorization is checked. A revoked, expired,
disabled, missing, or unresolvable grant grants no authority.

The MVP does not introduce roles, groups, inheritance, deny rules, organization
policy, tag/path predicates, conditions, arbitrary expressions, or a generic
policy language. Entity-, schema-, field-, project-, workspace-, branch-, and
organization-scoped reusable grants remain Deferred. Exact approval still binds
the concrete stable-ID targets of one proposal, so document-level reusable
authority does not make approval broad.

Grant ID encoding, registry persistence, administration UI, and trusted local
bootstrap remain Provisional.

### 4. Mutation authority uses four additive classes

Every Stable semantic Command family that may appear in a reviewable proposal
must have a deterministic mutation classification derived from typed command
meaning:

```text
Data
Formula
Schema
Destructive
```

- `Data` covers ordinary non-formula semantic data creation or update,
  reference-value update, non-destructive entity metadata/key change, and other
  changes that do not alter formula definitions or schema meaning.
- `Formula` covers creation, replacement, or removal of a bound formula
  definition or formula-bearing semantic value.
- `Schema` covers creation, replacement, or removal of schema or field
  declaration meaning, including type, requiredness, or reference-target
  semantics.
- `Destructive` covers deletion or irreversible discard/replacement of
  established semantic objects or data.

Classes are additive rather than hierarchical. Entity removal requires at least
`Data + Destructive`; schema/field removal requires at least
`Schema + Destructive`; formula removal requires at least
`Formula + Destructive`.

For an AtomicBatch:

```text
RequiredClasses(batch) = union(RequiredClasses(command_i))
```

Every required class must be covered independently. Unknown, unsupported, or
unclassified mutation commands fail closed for Propose, Approve, and Execute.
Classification cannot depend on natural-language prompts, provider/model
identity, confidence, rendered diff prose, or UI labels.

A Data-only proposal is the MVP ordinary/routine class. Any proposal containing
Formula, Schema, or Destructive is elevated for review presentation. This risk
projection does not replace the exact mutation-class set or exact approval.

The complete future command catalogue remains Provisional under ADR-0020.
Published Stable command-to-class meaning cannot be changed silently.

### 5. Machine execution always requires exact Human approval in the MVP

Every `Execute` requested by a Machine principal requires one active exact
approval issued by a distinct Human principal.

The Machine executor must hold active `Execute` grants covering the proposal's
DocumentId and every required mutation class. The Human approver must hold
active `Approve` grants covering the same DocumentId and classes. Approval
cannot add missing capability, widen scope, or override a semantic gate.

All Machine executions remain approval-gated, including ordinary Data changes.
Formula, Schema, and Destructive operations receive no autonomous exception.
Reusable auto-approval rules and bounded autonomous mutation are Deferred.

This ADR does not require an additional approval for a directly authenticated
Human principal using an ordinary first-party human editing workflow. Host or
product policy may require one, but that is not part of the current AI/Machine
MVP contract.

### 6. Approval binds one occurrence, one exact change, and one authorization context

The logical approval object is:

```text
Approval
- opaque ApprovalId
- ProposalId
- ExactChangeCommitment
- Semantic API compatibility contract
- exact semantic base reference
- DocumentId
- required mutation-class set
- Human approver PrincipalId
- exact Approve GrantIds relied upon
- Machine executor PrincipalId
- exact Execute GrantIds relied upon
- authorization/approval profile identifier
- issued_at
- finite expires_at
```

Approval content is immutable after issuance. Mutable lifecycle state is held by
a trusted approval registry and is at least distinguishable as active, revoked,
and consumed; expiry may be derived from trusted time.

Approval binds both proposal occurrence identity and ADR-0024
`ExactChangeBinding`. Two independently issued proposal occurrences with
identical exact semantic contents cannot share approval because their ProposalId
is different.

Approval additionally binds the exact executor, approver, DocumentId, required
mutation classes, approval profile, and GrantIds used to establish approval and
execution authority. Revoking or replacing any bound grant makes the approval
unusable. Issuing a semantically equivalent new grant does not revive the old
approval.

Rendered diff text, intent prose, prompt, confidence, provider/model identity,
UI coordinates, `.roproj` bytes, storage paths, and Git objects are not approval
identity.

Exact Approval/Grant/Principal Rust types, field names, Serde layout, transport,
and registry storage remain Provisional.

### 7. ExactChangeCommitment is structural authority with a replaceable digest profile

The Accepted integrity law is:

```text
ApprovalExactChange(A) == ExactChangeBinding(P)
```

A trusted implementation may satisfy this law through structural comparison
with the retained immutable proposal record. Proposal occurrence identity alone
is insufficient.

When proposal and approval are persisted or transported as separately decoded
records, the implementation must also use a versioned, domain-separated,
collision-resistant commitment over the complete logical
`ExactChangeBinding`. The commitment covers the Semantic API compatibility
contract, exact base, body kind, complete typed command semantics, generated
IDs, bound formulas, command-owned semantic preconditions, and AtomicBatch
order.

It must not hash rendered diff prose, incidental Rust/Serde/transport layout,
`.roproj` bytes, a Git object, or only the ProposalId. Unsupported commitment
profiles fail closed.

For the first implementation, SHA-256 over a tagged, length-delimited,
versioned internal transcript is the preferred Provisional mechanism. The exact
transcript bytes, profile spelling, algorithm migration, and public wire form
are not Accepted ecosystem contracts. A digest detects exact-content mismatch;
it does not authenticate the approver or grant authority by itself.

The current MVP therefore treats ApprovalId as a reference into a trusted
registry rather than a portable bearer credential. Portable/offline approvals,
MACs, signatures, PKI, key rotation, and cross-service verification remain
Deferred.

### 8. Approval is finite, revocable, stale-sensitive, and single-use

Every approval has finite `expires_at`. Permanent approval is forbidden. Exact
TTL and maximum lifetime are Provisional host/security-profile choices; a
15-minute default is recommended for the first implementation.

An approval is unusable when any of the following is true:

- proposal occurrence identity differs;
- ExactChangeCommitment differs;
- current semantic base does not exactly equal the proposal base;
- Semantic API or approval profile is unsupported;
- approver or executor differs, is disabled, or cannot be authenticated;
- any bound grant is absent, revoked, expired, replaced, or no longer covers the
  exact document/classes;
- the approval is expired;
- the approval is explicitly revoked; or
- the approval is already consumed.

Base mismatch follows ADR-0024 exactly: return `Stale` before constructing a
candidate against the changed base, publish nothing, perform no implicit
rebase/merge/retarget/replay, and leave the immutable proposal unchanged. Later
semantic content equivalence does not revive the old proposal or approval.

Approval is single-use. After all pre-publication authorization, base, identity,
integrity, and gate checks pass, the trusted boundary must claim/consume the
approval before or atomically with entry into the semantic publication path.
Only one concurrent attempt may claim it. A failed attempt after claim does not
restore the approval; a new approval is required. Exact reservation and commit
mechanics belong to #29.

Approval state, trusted time, grant state, or principal state that cannot be
verified causes fail-closed denial.

### 9. Minimum provenance survives outside canonical semantic state

Every reviewable proposal retains at least:

- ProposalId;
- exact-change commitment/profile;
- Semantic API compatibility contract;
- exact semantic base;
- proposer PrincipalId and principal kind;
- proposal timestamp; and
- when available, structured agent/provider/model/tool identity as evidence.

Every approval retains at least:

- ApprovalId;
- Human approver PrincipalId;
- Machine executor PrincipalId;
- bound GrantIds;
- approval profile;
- DocumentId and mutation classes;
- issue and expiry timestamps; and
- revocation/consumption evidence.

Every successful Machine Execute retains at least:

- proposal and exact-change identifiers;
- proposer, approver, and executor principals;
- bound grant and approval identifiers;
- base and resulting semantic revision;
- required mutation classes;
- final authoritative gate outcome or durable reference to its machine-readable
  evidence;
- execution timestamp and result; and
- provider/model/tool snapshot actually used when known.

Provider/model changes do not alter authority when the trusted executor
PrincipalId remains the same; they are recorded as changed provenance. If the
host resolves a new PrincipalId, existing approval fails executor binding.

Provenance is audit/history evidence, not semantic Document state, semantic
truth, or privilege. It must not be written into `.roproj` merely to make it
durable. Full prompts, hidden reasoning, credentials, secrets, and complete chat
transcripts are not minimum provenance.

The exact provenance DTO, retention policy, history store, tamper-evidence,
redaction, and UI remain Provisional/Deferred. This ADR does not require event
sourcing or a universal operation log.

### 10. Semantic authority and host/external effects remain separate

Semantic publication, durable materialization, and external/host effects are
separate authority domains.

A Query, Propose, Approve, or Execute grant under this ADR does not authorize:

- filesystem or browser persistence reads/writes;
- `.roproj` or `.ro` materialization;
- network access;
- process or shell execution;
- Git commit, push, or repository administration;
- plugin invocation;
- deployment or publication;
- credentials or secrets access; or
- any other host capability.

A host may materialize or externally publish an already-authorized semantic
result only under separate host authority. Storage and host layers cannot mint
semantic approval or redefine semantic meaning. Cross-domain transactions and
external-effect capability vocabulary remain Deferred.

### 11. Preserve current crate and runtime boundaries

Authorization/approval policy belongs at the trusted application/runtime and
host composition boundary. It is not added to `semantic-core`, formula, diff,
merge, or storage semantics.

`workspace-engine` remains the shared semantic transition/gating authority.
`ai-api` remains a provider-facing adapter/projection and must not become the
sole enforcement point. Trusted principal resolution, grants, time, approval
registry, and external host effects remain outside the capability-free portable
semantic engines.

#29 owns concrete preview/approve/apply/verify lifecycle, approval registry and
claim/consumption mechanics, atomic publication integration, and provenance
persistence. #30 owns instruction/data separation, untrusted-input treatment,
raw-mutation bypass prevention, external-effect enforcement, and stable
security-denial observability. #93 owns concrete resident session and semantic
revision mechanics needed to realize exact base equality and state advancement.

## Required conformance scenarios

Future implementation must preserve at least these representation-neutral
outcomes:

1. **Read-only principal** — Query succeeds within the granted DocumentId;
   Propose, Approve, and Execute are denied.
2. **Proposal-only principal** — Propose for covered classes succeeds and
   publishes nothing; Execute is denied.
3. **Approved ordinary Data update** — covered Machine Execute grant, covered
   Human Approve grant, current base, exact approval, and successful gate publish
   exactly once and return the resulting revision/provenance.
4. **Denied Formula operation** — Data-only grants or approval cannot authorize
   Formula mutation.
5. **Denied Destructive operation** — non-destructive grants cannot authorize
   deletion.
6. **Changed proposal** — changed target, typed value, formula, generated ID,
   body kind, command, base, compatibility contract, or batch order requires a
   new proposal and approval.
7. **Identical change, different occurrence** — a different ProposalId cannot
   reuse approval even when its ExactChangeBinding is identical.
8. **Stale base after approval** — returns Stale before candidate construction
   and publishes nothing.
9. **Expired or revoked approval** — denies without publication.
10. **Revoked/replaced bound grant** — invalidates approval; an equivalent new
    grant does not revive it.
11. **Replay** — consumed ApprovalId cannot authorize a second attempt;
    concurrent claims cannot both succeed.
12. **Provider/model change** — same trusted executor principal preserves
    authority but records new provenance; a different principal fails executor
    binding.
13. **Machine self-approval** — denied.
14. **Gate failure despite approval** — semantic gate remains authoritative and
    publishes nothing.
15. **External side effect under semantic authority** — filesystem, network,
    process, Git, plugin, deployment, or persistence request is denied in the
    separate effect domain.
16. **Mixed AtomicBatch** — every mutation class in the union must be granted
    and approved; missing one denies the complete batch.

## Deliberately Provisional or Deferred

The following remain replaceable or unresolved:

- exact PrincipalId, GrantId, ApprovalId, capability, class, and profile string
  encodings;
- authentication/session mechanism and principal registry;
- grant and approval storage APIs;
- exact Rust/Serde/wire DTOs;
- exact approval TTL and trusted clock implementation;
- exact digest transcript bytes and algorithm migration;
- public approval token or bearer protocol;
- signatures, MACs, PKI, and offline/cross-service approvals;
- entity/schema/field/project/workspace/organization reusable grant scopes;
- roles, groups, inheritance, ABAC, policy DSLs, and enterprise IAM;
- reusable auto-approval policies and autonomous mutation;
- multi-party approval, quorum, escalation, and workflow chains;
- external/host capability vocabulary and plugin/network sandboxing;
- provenance store, retention, tamper evidence, redaction, and history UI;
- operation log, event sourcing, undo, rollback, and recovery protocol;
- concrete revision/session/concurrency mechanics under #93; and
- lifecycle/result DTOs and atomic approval-claim implementation under #29.

## Rejected alternatives

### A broad `editor` or `write:*` grant

Rejected. It hides Formula, Schema, Destructive, and future external authority
inside one flag and prevents meaningful least privilege.

### Provider- or model-based privilege

Rejected. Provider/model identity is provenance and cannot be semantic or
authorization authority.

### Approve rendered diff text or chat prose

Rejected. Diff/prose is derived presentation and cannot replace ADR-0024 exact
command/base binding.

### Use ProposalId alone as integrity proof

Rejected. Proposal occurrence identity is not a content digest and ADR-0024
explicitly forbids treating it as cryptographic proof.

### Make approval a client-supplied bearer object now

Rejected for the MVP. It would require authenticated signing/MAC, key lifecycle,
and portable verification machinery without current product pressure. A trusted
registry reference is smaller and safer.

### Generic policy or precondition language

Rejected. Typed commands, closed mutation classes, document grants, and exact
approval are sufficient for the MVP. Authorization conditions do not become a
second semantic precondition vocabulary.

### Persist approval/provenance inside the semantic Document

Rejected. Authorization and audit evidence are neighboring application/history
concerns, not canonical product-semantic state.

### Let approval override validation or gates

Rejected. Authorization and semantic correctness remain independent mandatory
prerequisites.

## Consequences

Positive:

- implementation can authorize Query, Propose, Approve, and Execute without
  parsing natural language;
- ordinary Data, Formula, Schema, and Destructive authority remain visibly
  separate;
- exact human review is bound to the immutable proposal/base rather than UI
  prose or provider claims;
- stale, modified, expired, revoked, and replayed approval fails closed;
- provider/model evolution remains replaceable without changing authorization;
- semantic and host/external authority stay separate; and
- the MVP gains a safe implementation target without an enterprise IAM or
  generic policy platform.

Costs:

- every Machine mutation requires explicit approval in the MVP;
- document-scoped reusable grants are intentionally coarser than future
  least-privilege scopes;
- approvals need a trusted registry, finite lifetime, revocation, and atomic
  consumption;
- every Stable mutation command family needs explicit class mapping; and
- provenance must survive independently from canonical semantic state.

## Required follow-up

- Reconcile `semantic-api.md`, `ai-agent-api.md`, AI/runtime architecture,
  security documentation, indexes, and the canonical reconciliation register.
- #29 implements proposal preview, approval issuance/registry,
  claim/consumption, atomic Execute, verification, and provenance attachment.
- #30 enforces trusted principal/data boundaries, no raw mutation bypass,
  external-effect separation, and machine-readable denials.
- #93 supplies concrete exact semantic revision/session behavior without
  changing this authorization contract.
- Close #28 with a Decision Capsule after the authority/documentation PR merges.

## Related

- Product Constitution §§2.5 through 2.7, 6, 7
- Design Principles §§7 through 10, 12
- ADR-0007
- ADR-0015
- ADR-0019
- ADR-0020
- ADR-0022
- ADR-0024
- Issues #11, #28, #29, #30, #93
