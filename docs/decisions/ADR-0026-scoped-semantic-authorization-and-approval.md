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
operation families, semantic subjects, and mutation classes that authority
covers; what one human approved; and when that approval is no longer usable. It
must not become enterprise IAM, a role system, a generic policy language, or a
portable bearer-token protocol.

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

`PrincipalKind` is an immutable authorization attribute of one `(authorization
domain, PrincipalId)` occurrence. The same PrincipalId can never be
reclassified between `Human` and `Delegated`; a change of kind creates a new
principal occurrence with a new PrincipalId.

Every authorization-relevant principal comparison uses the complete
`(authorization domain, PrincipalId)` occurrence. A bare PrincipalId from one
domain never matches the same-spelled PrincipalId in another domain.

`(authorization domain, PrincipalId)` identifies one non-reusable,
non-reassignable principal occurrence. Deleting, disabling, transferring,
recreating, or replacing an account cannot assign that PrincipalId to a
different authorization subject; a replacement subject receives a new
PrincipalId. Login names, email addresses, provider identifiers, aliases, and
similar account attributes are not authorization identity and their reuse
cannot retarget an existing Grant, Approval originator/executor binding,
Approval approver reference, or provenance record. Those references remain
attached only to the original principal occurrence and do not transfer to a
replacement occurrence created for a different subject or kind. A disabled
occurrence may be re-enabled only when the trusted boundary proves continuity
of the same authorization subject and the same immutable PrincipalKind.
Otherwise it must issue a new PrincipalId. Resolution fails closed when the
required occurrence is disabled, missing, unauthenticated, or unresolvable, or
when subject continuity or immutable kind cannot be proven.

The authorization domain does not create a semantic `ProjectId` or
`WorkspaceId`. Exact account/login/identity-provider behavior, principal ID
encoding, and Rust/wire representations remain Provisional.

### 2. Capabilities preserve operation-family, action, and mutation dimensions

Every Semantic API operation or family remains independently
capability-addressable under ADR-0020. The minimum authority dimensions are:

```text
Query(OperationFamily)
Propose(OperationFamily, MutationClass)
Execute(OperationFamily, MutationClass)
Approve(OperationFamily, MutationClass)
```

`OperationFamily` is an independent checked dimension selected by trusted
Semantic API classification. Authority for one operation family does not cover
another family with the same action, mutation class, or semantic scope.
Unknown or unclassified operation-family meaning fails closed.

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

No capability implies another operation family, action, or mutation class.
Query does not imply Propose, Propose does not imply Execute, Approve does not
imply Execute, and Execute does not imply Approve. Exact operation-family and
capability identifiers, the operation-family catalogue, and the complete
operation-to-class catalogue remain Provisional.

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
- disclosure_requirements: (OperationFamily, DisclosureScopeAtom)
- associated_write_requirements: (OperationFamily, MutationClass, ScopeAtom)
- canonical_write_scope
- mutation_classes
```

The client or agent cannot authoritatively declare its own footprint. Each
disclosure requirement retains one operation-family/disclosure-scope pair.
Each associated write requirement retains one operation-family,
mutation-class, and canonical-write-scope tuple. At a Query, Propose, Approve,
or Execute check, the trusted boundary combines the requested action with each
applicable tuple and requires same-Grant coverage for that complete
association. The action is check context, not part of the footprint bound into
Approval. `AtomicBatch` preserves the union of those associated per-command
tuples. Flattened operation-family, canonical-write-scope, and mutation-class
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
requirement. Each Query requirement is a complete `(Query, operation family,
disclosure scope)` tuple. For a requested mutation action, each derived
`(action, operation family, mutation class, scope)` tuple must be covered by
one same live Grant. Multiple Grants may combine across distinct associated
requirements, but independently unioning their actions, operation families,
classes, and scopes must not create crossed authority that no Grant actually
contains. A Grant from another authorization domain or for another subject
grants nothing to the effective principal.

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
  associated operation-family/mutation-class/scope write requirement;
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
  + complete associated operation-family/mutation-class/write-scope requirements
  + authorization-policy version
```

Approval additionally records an approval ID, the Human approver, issued time,
finite expiry, and the authorizing Approve Grant references.

The trusted boundary structurally verifies one immutable proposal occurrence
and its complete `ExactChangeBinding`, then binds the exact originator,
executor, complete associated operation-family/mutation-class/write-scope
requirements, and the effective authorization-policy version selected by the
trusted authorization domain at issuance.
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
`ExactChangeBinding`, associated operation-family/mutation-class/scope write
requirements, the Approval-bound policy version against the effective policy
governing execution, required principal occurrences and their immutable kinds,
and the authoritative semantic gate.

For Approval-gated Execute, the common publication-boundary condition in
section 10 is necessary but not sufficient. Semantic publication and Approval
consumption are additionally conditional on the bound originator and approver
occurrences and their immutable PrincipalKinds still being proven, authorizing
Approve Grant references, Approval state, and exact proposal/Approval/base
binding still being valid. Revocation, expiry, disablement, consumption,
attempted same-ID kind reclassification, loss of required occurrence or kind
proof, or proposal-base invalidation racing with execution prevents both
publication and Approval consumption. If the trusted boundary cannot prove the
complete conjunction, it fails closed and publishes nothing.

Before use, approval can be revoked by its approver, a referenced Grant issuer,
or the trusted authorization authority. Loss of verifier use/revocation state
fails closed. An ephemeral or session-bound approval becomes invalid when its
verifier state no longer exists. A stale or re-proposed patch requires a new
Approval. Replay of a consumed Approval returns a machine-distinguishable
denial.

#29/#93 own concrete reservation, locking, transaction, revision, retry, and
state-installation mechanics. This ADR fixes only the observable conditional
publication guarantee.

### 9. Approval becomes unusable without publication

Execute fails without publication when any of these holds:

1. `ProposalId` differs;
2. `ExactChangeBinding` differs;
3. the semantic base is stale;
4. the originator differs or its occurrence/immutable kind cannot be proven;
5. the executor differs or its occurrence/immutable kind cannot be proven;
6. the rederived associated operation-family/mutation-class/scope write
   requirement differs;
7. the Approval-bound authorization-policy version is unsupported or differs
   from the effective policy governing execution;
8. Approval expired;
9. Approval was revoked;
10. Approval was already consumed;
11. the approver is not a trusted Human or its occurrence/immutable kind cannot
    be proven;
12. an authorizing Approve Grant reference is no longer valid or covering;
13. the executor's Execute Grants are no longer sufficient; or
14. the authoritative semantic gate rejects publication.

Approval may remain as historical evidence after becoming stale or unusable,
but cannot authorize another proposal or base.

### 10. Execute is a conjunction, not a shortcut

Every semantic Execute path has the same publication-boundary safety core,
including a directly authenticated Human Execute that legitimately has no
SemanticPatch or Approval. At the publication boundary, publication remains
conditional on all of these facts still holding:

```text
complete effective executor occurrence, including authorization domain, is
    active and authenticated
AND that occurrence's immutable PrincipalKind remains proven and matches the
    selected authorization path; direct Execute without Approval requires Human
AND live Execute Grants cover every rederived associated
    operation-family/mutation-class/scope requirement
AND authorization is evaluated under the effective policy selected by the
    trusted authorization domain, and that policy remains effective
AND the semantic context used to evaluate the candidate and gate is current,
    or an equivalent revision-safe condition holds
AND the authoritative semantic gate still allows publication
AND no unauthorized host or external effect is performed
```

Execute-Grant revocation or expiry, executor disablement, relevant semantic
state advance, effective-policy change, gate invalidation, attempted same-ID
kind reclassification, loss of kind proof, or inability to prove this
conjunction prevents publication. This common condition applies whether or not
the Execute path uses a proposal or Approval. Direct Human Execute uses the
effective current policy and does not invent a historical or Approval-bound
policy choice.

ADR-0024 requires the trusted application/runtime boundary to compare the
current semantic context with the proposal base before re-evaluating,
authorizing, or executing an existing proposal and before candidate
construction against a changed base. Approval-gated Execute performs that
comparison internally first and retains the result without disclosure.

The boundary may also detect Semantic API version support internally, but
neither version detection nor stale detection grants a right to learn proposal
or revision state. The boundary next authenticates and matches the
complete authenticated caller occurrence, including authorization domain,
against the Approval-bound executor occurrence and verifies the complete
trusted proposal/Approval binding. An unauthenticated, cross-domain, or unbound
caller receives only a disclosure-safe authorization denial. A missing,
unrelated, mismatched, or unverifiable proposal receives the same disclosure-
safe binding denial regardless of the retained version or base result. Only
after exact binding is proven may the boundary expose an unsupported-version
result and then the retained `Stale` outcome; stale details remain limited by
Query authority. If the complete binding cannot be proven for an unsupported
version, the result is binding denial rather than version disclosure. This
detect-versus-disclose distinction preserves ADR-0024 without amending it.

Approval-gated Execute adds all of these independent requirements to the common
publication condition:

```text
supported immutable SemanticPatch
AND exact proposal/base + complete Approval binding
AND complete authenticated executor occurrence, including authorization domain,
    matches the Approval-bound executor occurrence and remains equal at
    publication
AND bound relational AuthorizationFootprint remains exact
AND Approval-bound authorization-policy version equals the effective policy
    governing execution and still equals it at publication
AND valid active Human Approval
AND active bound originator and Human approver occurrences with each immutable
    PrincipalKind still proven
AND live authorizing Approve Grant references
AND atomic Approval consumption with successful publication
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

Successful Approval-gated Execute provenance preserves at least:

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

A directly authenticated Human Execute that requires no proposal or Approval
must not fabricate a proposal ID, `ExactChangeBinding`, originator or approver
role, Approval ID, Approve Grant reference, or `Consumed` Approval state. A
direct-Human receipt may retain the executor, effective Execute Grant
references, trusted authorization footprint and policy version, relevant input
and resulting revision, and gate/result evidence. Exact receipt/history DTO,
storage, and retention remain #29/#12 work.

Approval-gated provenance and any optional direct-Human receipt facts may
attach to an execution receipt or history record and cross adapter boundaries.
They are not semantic Document data. Exact durable storage, receipt DTO,
retention, and recovery links remain #29/#12 follow-up. Event sourcing, CRDT,
and a general operation-log protocol are not required. Denials need
machine-readable outcomes; durable denial logging remains #30/audit-policy
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
  resolution and must preserve subject non-reassignment and immutable
  PrincipalKind continuity across its account lifecycle; exact
  account/provider and re-enable mechanisms remain Provisional.
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
20. **Grant revocation races publication** — revoking a required authorizing
    Approve Grant or relied-upon Execute Grant after the ordinary gate check but
    before publication prevents publication and Approval consumption.
21. **Principal disablement races publication** — disabling any required
    principal occurrence after the ordinary gate check but before publication
    prevents publication and Approval consumption.
22. **Approval state races publication** — concurrent revocation, expiry, or
    consumption of Approval prevents another publication.
23. **Base advance races publication** — any intervening semantic revision
    change before publication prevents candidate installation and leaves the
    proposal stale.
24. **Unauthorized stale probing** — an unauthenticated or wrong executor
    cannot distinguish current, stale, missing, or mismatched proposal state
    and receives only a disclosure-safe authorization denial.
25. **Cross-proposal stale probing** — even the bound executor cannot use a
    mismatched ProposalId or ApprovalBinding to probe another proposal through
    `Stale`; binding mismatch denies before stale disclosure.
26. **Direct-Human Grant revocation races publication** — revoking or expiring
    relied-upon Execute authority after ordinary authorization/gating but before
    direct Human publication prevents publication.
27. **Direct-Human executor disablement races publication** — disabling the
    directly authenticated Human executor before publication prevents
    publication.
28. **Direct-Human context advances before publication** — relevant semantic
    state advance prevents installation of a candidate evaluated against the
    obsolete context.
29. **Direct-Human provenance stays truthful** — successful direct Human
    Execute does not manufacture proposal, originator, approver, Approval,
    Approve Grant, or `Consumed` facts.
30. **Approval-gated provenance and consumption remain complete** — successful
    Approval-gated Execute retains its full proposal/Approval provenance and
    consumes Approval atomically with publication.
31. **Query families do not cross-authorize** — Query authority for one
    operation family and disclosure scope does not authorize another Query
    family over the same scope.
32. **Execute families do not cross-authorize** — Execute authority for one
    operation family, mutation class, and write scope does not authorize a
    different Execute family with the same class and scope.
33. **AtomicBatch preserves operation-family associations** — every batch
    member retains its own operation-family/mutation-class/scope association;
    flattened unions cannot synthesize coverage for a member tuple.
34. **Readable historical policy does not remain authority** — an Approval
    bound under V1 denies and requires a new Approval when V2 is the effective
    execution policy, even if V1 remains readable or supported historically.
35. **Effective-policy change races publication** — changing the effective
    authorization policy after authorization/gating but before publication
    prevents publication on every Execute path and, for Approval-gated Execute,
    also prevents Approval consumption.
36. **Direct Human uses current policy** — direct Human Execute is authorized
    under the effective current policy and does not fabricate an Approval or
    historical policy binding.
37. **Unsupported-version probing** — a bound executor supplying a missing,
    unrelated, mismatched, or unverifiable proposal receives the same
    disclosure-safe binding denial regardless of internally detected version
    support; only a completely verified proposal/Approval binding may expose
    an unsupported Semantic API version.
38. **Human-to-Delegated race** — after a direct Human Execute is authorized
    and gated, attempting to reclassify that same PrincipalId as Delegated
    before publication cannot satisfy the immutable-kind proof and publishes
    nothing; a Delegated replacement is a new occurrence requiring a new ID
    and the applicable Approval path.
39. **Delegated-to-Human bypass** — a Delegated originator or executor cannot
    be reclassified under the same PrincipalId as Human to bypass Approval; a
    Human replacement is a distinct occurrence and does not retarget the
    proposal or Approval.
40. **Kind-changing replacement has no inherited authority** — every valid
    Human/Delegated reclassification creates a new PrincipalId whose occurrence
    inherits no Grant, Approval originator/executor binding, Approval approver
    reference, or provenance from the old occurrence.
41. **Same-occurrence re-enable** — a disabled occurrence may be re-enabled
    under the same PrincipalId only when the trusted boundary proves the same
    authorization subject and unchanged PrincipalKind; all independent Grant,
    Approval, and publication checks still apply.
42. **Cross-domain same-ID executor** — an authenticated caller from one
    authorization domain cannot satisfy an Approval-bound executor occurrence
    in another domain even when both domains contain the same-spelled
    PrincipalId; the complete occurrence equality remains required through
    publication, and Grants from one domain cannot cover the other.

## Stability classification

Accepted:

- opaque, non-reusable, non-reassignable Principal occurrences within an
  authorization domain;
- immutable PrincipalKind per occurrence, with a new occurrence and
  PrincipalId required for Human/Delegated reclassification;
- trusted Human versus Delegated distinction for MVP policy;
- default deny and explicit Grants;
- non-reusable immutable Grant occurrences with terminal revocation;
- Query/Propose/Execute/Approve non-implication;
- operation-family identity as an independent checked capability dimension;
- Value/Formula/Structure/Schema/Destructive distinctions;
- stable-ID, document-local scope concepts and finite-union Grants;
- trusted `AuthorizationFootprint` derivation;
- associated operation-family/mutation-class/scope coverage, combined with the
  requested action, without crossed-Grant unions;
- Human Approval for Delegated-origin or Delegated-authority publication;
- exact originator/executor/proposal/associated-write-requirement/policy
  binding;
- Approval-bound policy-version equality with the effective execution policy
  through the publication boundary;
- authorizing Approve Grant references remain live, with fresh executor
  authority rechecks before Execute;
- the common publication-boundary condition for every Execute path;
- finite Approval consumed atomically with at most one successful publication,
  and only while the complete publication-boundary authorization condition
  remains valid;
- revocation, fail-closed lost state, and replay denial;
- trusted structural exact binding for the MVP;
- minimum proposal/Approval-gated execution provenance without fabricated
  Approval facts on direct Human Execute; and
- semantic/external-effect and authorization/validation separation.

Provisional:

- exact type, field, capability, and denial-code names;
- exact operation-family identifiers and catalogue;
- ID encodings and generators;
- Grant/Approval/provenance DTOs and storage;
- exact duration values and clock representation;
- policy-version representation and effective-policy selection mechanisms;
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
operation-family/mutation-class/scope write requirements, and policy version
are the smaller stable authorization context. Authorizing Approve Grant
references remain immutable issuance and revocation dependencies, while
Execute authority is freshly rederived and may be satisfied by another live
Grant set. Propose authority is checked when the proposal is issued and
retained as provenance; it is not later execution authority.

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
