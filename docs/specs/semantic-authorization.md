# Semantic Authorization and Approval Specification

Decision state: Accepted under
[ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md).

Implementation state: Not implemented. Current provider-free AI operations are
read/explain/analyze/suggest-only. The current `Suggestion` DTO is not a
SemanticPatch, Grant, Approval, execution credential, or public protocol.
Lifecycle, registry, atomic publication/consumption, enforcement, revision,
and transport work remains owned by #29, #30, and #93.

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

Define the smallest provider-neutral contract needed to authorize semantic
Query, Propose, Approve, and Execute behavior and to require exact Human
approval for a proposal originated by or executed with Delegated authority.

This specification defines:

- opaque principals within one trusted authorization domain;
- independently grantable semantic actions, operation families, and mutation
  classes;
- a closed set of document-local stable-ID scope atoms;
- trusted relational derivation of operation-family, disclosure, and
  canonical-write requirements;
- exact approval binding without selecting digest or wire bytes;
- finite expiry, revocation, retry, and at-most-once successful publication;
- minimum proposal, approval, and execution provenance; and
- the boundary between semantic authorization and host/external effects.

It consumes ADR-0024 `SemanticPatch`, `ExactChangeBinding`, semantic-base, and
stale laws exactly. It does not restate or amend those laws, define another
mutation vocabulary, introduce a policy DSL, or freeze a public Rust/Serde/wire
DTO.

## Contract boundary

```text
trusted identity / authorization domain
              |
              v
authenticated Principal + live Grants
              |
              v
trusted derivation of operation family + disclosure/write scope + mutation classes
              |
              v
Query / Propose / Approve / Execute authorization
              |
              v
direct Human Execute OR immutable ADR-0024 SemanticPatch
                         + exact Human Approval when required
              |
              v
current evaluated context + authoritative semantic gate
              |
              v
common publication-boundary condition
              |
              v
atomic semantic publication
plus Approval consumption only for Approval-gated Execute
              |
              v
result revision + minimum provenance

separate host-effect domains beside this path:
storage / filesystem / browser persistence / network / Git / process / plugin
```

Semantic admissibility, operation gating, authorization, approval, and host
authority are independent. Passing one never satisfies another.

## Terminology

### Authorization domain

The trusted context in which Principal, Grant, and Approval identifiers are
resolved and registry state is authoritative. The exact host/session/account
mechanism and domain identifier encoding are Provisional.

### Principal

One non-reusable, non-reassignable occurrence of an accountable authorization
subject, resolved by the trusted identity or host boundary within one
authorization domain.

```text
PrincipalKind = Human | Delegated
```

`Delegated` includes agents, automation, services, plugins, and similar
non-human authorities. Provider, model, tool, prompt, confidence, and
self-reported metadata are provenance, not principal class or privilege.

### Grant

An immutable authorization issuance occurrence created through a trusted host
authorization authority for one subject principal. One or more live Grants may
combine to cover one complete semantic authorization requirement. Revocation
is terminal trusted registry state associated with the occurrence, not mutable
Grant content.

### Authorization footprint

The complete operation-family/disclosure-scope requirements and associated
operation-family/mutation-class/scope write requirements derived by the trusted
semantic/application authority for an operation and its relevant base/candidate
relationships. The requested action is authorization-check context, not a
member of that bound relation. Flattened operation-family, write-scope, and
mutation-class sets are summaries of the relation.

### Approval

An immutable Human authorization occurrence for one exact proposal occurrence,
exact ADR-0024 change/base, exact authorization context, and one authorized
executor. Mutable use/revocation state belongs to a trusted registry.

## Principal contract

1. A trusted identity/host/session boundary MUST supply the effective
   PrincipalId for every authorization-relevant request.
2. `(AuthorizationDomain, PrincipalId)` MUST identify exactly one principal
   occurrence and MUST NOT be reused or reassigned to a different authorization
   subject.
3. Deleting, disabling, transferring, recreating, or replacing an account MUST
   NOT reassign its PrincipalId. A replacement subject MUST receive a new
   PrincipalId.
4. Login names, email addresses, provider identifiers, aliases, and similar
   account attributes MUST NOT define principal equality or retarget existing
   Grants, Approval originator/executor bindings, or provenance when reused.
   Those references remain attached only to the original principal occurrence.
5. PrincipalId is meaningful within one authorization domain. A client MUST
   NOT substitute an identifier from another domain.
6. A request payload, prompt, model response, document, import, or plugin result
   MUST NOT select or upgrade the effective principal or principal kind.
7. Proposer/originator, Human approver, and executor are separate recorded
   roles. A Human may occupy more than one role when policy permits; no
   mandatory four-eyes rule is introduced.
8. The Human approval required by this specification MUST be issued by a
   trusted Human principal. A Delegated principal cannot satisfy that role.
9. Disabled, missing, unauthenticated, or unresolvable principal occurrences
   fail closed; resolving a replacement subject under a reused account
   attribute does not reactivate the original occurrence.
10. Accounts, login providers, directories, groups, organizations, and
   enterprise identity administration are outside this contract.

## Semantic capability contract

The minimum action dimensions are:

```text
Query(OperationFamily)
Propose(OperationFamily, MutationClass)
Execute(OperationFamily, MutationClass)
Approve(OperationFamily, MutationClass)
```

`OperationFamily` is an independent capability dimension derived by the
trusted Semantic API authority. Authority for one family MUST NOT authorize a
different family merely because action, mutation class, and semantic scope are
otherwise equal. Unknown or unclassified operation-family meaning fails
closed. Exact family identifiers and the complete family catalogue remain
Provisional.

### Action meaning

- `Query` authorizes deterministic, non-publishing Semantic API reads for the
  covered disclosure scope.
- `Propose` authorizes non-publishing evaluation of typed Command or
  AtomicBatch meaning for the covered canonical-write scope and mutation
  classes. It may issue an immutable SemanticPatch.
- `Approve` authorizes a Human to issue or revoke one exact Approval for the
  covered canonical-write scope and mutation classes.
- `Execute` authorizes the named executor to request authoritative semantic
  publication for the covered canonical-write scope and mutation classes.

No action implies another. Query does not imply Propose. Propose does not imply
Execute. Approve does not imply Execute. Execute does not imply Approve.
Unknown actions fail closed.

ADR-0020 remains Execute meaning. This specification requires an exact
SemanticPatch for a Delegated-origin or Delegated-authority approval path; it
does not require an ordinary directly authenticated Human editing operation to
introduce a reviewable proposal when no policy requires one.

## Mutation-class contract

The closed MVP classes are:

```text
Value
Formula
Structure
Schema
Destructive
```

- `Value` covers stored non-formula typed values, including references.
- `Formula` covers formula definitions and formula-bearing transitions.
- `Structure` covers non-schema document/entity lifecycle, creation,
  duplication, rename, membership, and similar structure.
- `Schema` covers schema/field definition, type, requiredness, reference-target,
  and other schema meaning.
- `Destructive` covers removal of an existing semantic object or fact without
  a same-kind replacement, or another Stable command meaning explicitly
  classified as data-losing. It combines with another class.

Ordinary in-place replacement at one stable target is not automatically
Destructive merely because the previous value or definition changes.

Classification laws:

1. Every Stable mutation Command family MUST have a deterministic required
   class set under its Semantic API compatibility contract.
2. Classification follows typed command meaning, never prompt prose,
   provider/model metadata, confidence, rendered diff, or UI labels.
3. Classes are additive and non-hierarchical.
4. Scalar/reference update requires at least Value.
5. Formula update requires at least Formula.
6. Entity creation/rename/duplicate requires at least Structure.
7. Entity deletion requires at least Structure + Destructive.
8. Formula removal requires at least Formula + Destructive.
9. Schema/field deletion requires at least Schema + Destructive.
10. AtomicBatch requires the union of every member Command's classes.
11. Unknown, unsupported, or unclassified mutation meaning fails closed for
    Propose, Approve, and Execute.

The complete operation-to-class catalogue remains Provisional. Published
Stable mappings cannot change silently.

## Semantic scope contract

The closed MVP scope atoms are:

```text
Document(DocumentId)
Schema(SchemaId)
SchemaField(SchemaId, FieldId)
Entity(EntityId)
EntityField(EntityId, FieldId)
```

Every atom is interpreted within one document-local semantic context. ADR-0015
keeps semantic relationships document-local; a matching subordinate ID in
another document does not match scope. Exact source/wire representation of
that document qualification remains Provisional.

Containment meaning:

- Document covers every semantic subject in that document.
- Schema covers its definition, fields, and entities/instances belonging to it
  in the relevant base or candidate.
- SchemaField covers the field definition and its entity-field instances.
- Entity covers the entity and its field instances, not its schema definition.
- EntityField covers one exact field instance.

A Grant MAY contain a finite union of atoms. Coverage is evaluated over the
relevant base and candidate. An operation moving or retargeting meaning across
containers requires coverage of both old and new sides.

Project, workspace, organization, tenant, branch, path, JSON Pointer, wildcard
string, tag, UI/storage/Git coordinate, natural-language predicate, and generic
scope expression are not supported.

## Trusted authorization-footprint derivation

Conceptually:

```text
AuthorizationFootprint
- DisclosureRequirements: (OperationFamily, DisclosureScopeAtom)
- AssociatedWriteRequirements: (OperationFamily, MutationClass, ScopeAtom)
- CanonicalWriteScope
- RequiredMutationClasses
```

`DisclosureRequirements` retains every required `(operation family,
disclosure scope)` pair. `AssociatedWriteRequirements` retains every required
`(operation family, mutation class, scope)` tuple. At a Query, Propose,
Approve, or Execute check, the requested action is combined with every
applicable tuple for coverage. Flattened operation-family sets,
`CanonicalWriteScope`, and `RequiredMutationClasses` are review/provenance
summaries of those relations, not independently unionable permission sets.
Exact representation remains Provisional.

Normative laws:

1. The trusted semantic/application authority MUST derive the footprint from
   typed operation meaning and relevant base/candidate relationships.
2. A client, model, prompt, or request-supplied footprint is untrusted and MUST
   NOT reduce the derived requirement.
3. Query authorization MUST retain each associated `(operation family,
   disclosure scope)` requirement and check Query with every pair.
4. Mutation authorization MUST retain each associated `(operation family,
   mutation class, scope)` requirement. The requested Propose, Approve, or
   Execute action MUST be checked with every tuple. Flattened operation-family,
   CanonicalWriteScope, and RequiredMutationClasses summaries are not coverage
   proof and not permission to form their Cartesian product.
5. AtomicBatch uses the union of every member Command's associated tuples and
   preserves each member's operation-family association.
6. CanonicalWriteScope includes direct targets, generated IDs, created/deleted
   objects and their owning containers, explicit retargeting, and
   command-defined canonical side effects.
7. Purely derived recalculation, FormulaImpact, validation findings, and review
   projections are not canonical writes.
8. DisclosureScope includes semantic subjects revealed by Query results,
   preview, diff, dependencies, impact, diagnostics, explanations, and result
   projections.
9. If the complete requirement cannot be derived safely, authorization fails
   closed or requires broader explicit scope.

## Grant contract

Conceptually, without fixing source or wire fields:

```text
Grant
- GrantId
- AuthorizationDomain
- IssuerPrincipal
- SubjectPrincipal
- Capabilities
- SemanticScope
- Validity
```

Normative laws:

1. `(AuthorizationDomain, GrantId)` MUST identify exactly one non-reusable
   issuance occurrence. Issuer, subject, capabilities, semantic scope, and
   validity are immutable; changed content creates another occurrence with a
   new GrantId.
2. Revocation state belongs to the trusted Grant registry. Revocation is
   terminal for that occurrence; restored or equivalent authority requires a
   new Grant occurrence and MUST NOT reactivate or reuse the old GrantId.
3. Grants may be issued only through a trusted host authorization authority,
   which may act on an explicitly authorized Human provisioning action or
   trusted host policy. Human principal class alone grants no issuance power.
4. A Delegated principal MUST NOT self-grant, expand, or transitively delegate
   authority.
5. Authorization is allow-only and default-deny.
6. Each Query requirement MUST be covered as one complete `(Query, operation
   family, disclosure scope)` tuple by one same live Grant. For a requested
   mutation action, each derived `(action, operation family, mutation class,
   scope)` tuple MUST be covered by one same live Grant. Different Grants MAY
   cover different associated requirements, but independently unioning their
   actions, operation families, classes, and scopes MUST NOT create crossed
   authority.
7. A Grant covers only its fixed subject in its AuthorizationDomain. A
   same-spelled GrantId from another domain or a Grant for another subject
   grants nothing to the effective principal.
8. Grant validity/revocation and subject state are checked whenever that Grant
   occurrence is relied upon. Immediately before Execute, authorizing Approve
   Grant references and sufficient current Execute Grants are rechecked.
9. Missing, expired, revoked, disabled, unsupported, or unresolvable authority
   grants nothing.
10. Approval cannot create, extend, restore, or replace Grant authority.
11. ADR-0007's current-MVP allowed read, analysis, explanation, and Propose
    behavior MUST be preserved through explicit trusted-host provisioning of
    sufficient Query and Propose Grants for supported flows. Those product
    defaults are not ambient AI authority or exceptions to default deny.

Exact Grant DTOs, identifiers, storage, administration, expiry/clock encoding,
and bootstrap remain Provisional.

## Disclosure and review contract

1. Propose authority does not imply Query authority.
2. Preview, diff, dependencies, impact, diagnostics, and explanations MUST NOT
   disclose subjects outside sufficient Query coverage; the trusted boundary
   denies or safely reduces the projection.
3. Approve authority permits inspection of the exact immutable proposal body
   needed to identify what is being approved. It does not grant arbitrary
   Query over the base or candidate.
4. Base values, candidate projections, diffs, dependencies, impact, and
   diagnostics shown during review still require Query coverage or a safely
   reduced approval projection.
5. Execute outcomes and denials MUST NOT disclose unauthorized semantic facts.
6. Exact redaction/projection DTOs and diagnostics belong to #29/#30; their
   replaceability does not weaken these disclosure laws.

## Approval policy for Delegated origin or authority

Exact Human Approval is required when either condition holds:

```text
proposal originator is Delegated
OR Execute uses Delegated authority
```

Query and Propose require Grants but no Approval.

For one proposal:

1. One Human approver must have live Approve authority covering every
   associated operation-family/mutation-class/scope write requirement.
2. Approval binds the named executor exactly. The trusted boundary checks that
   executor's live Execute authority immediately before publication, not as an
   issuance-time prerequisite.
3. Approval covers the exact whole Command or AtomicBatch.
4. Partial-batch approval and combining partial approvers are forbidden.
5. Quorum, approval chains, standing/reusable policies, and autonomous
   approval are not part of the MVP.
6. Approval cannot override stale base, semantic inapplicability, validation,
   or the authoritative gate.
7. A Human approver may also be the Human executor; no mandatory four-eyes rule
   is introduced.

## Approval binding contract

The Accepted logical binding is:

```text
ApprovalBinding(A) =
    AuthorizationDomain
  + ProposalId
  + ADR-0024 ExactChangeBinding(P)
  + OriginatorPrincipalId
  + ExecutorPrincipalId
  + complete AssociatedWriteRequirements
  + AuthorizationPolicyVersion
```

Approval additionally records:

```text
ApprovalId
HumanApproverPrincipalId
IssuedAt
finite ExpiresAt
Approve Grant references used at issuance
```

Normative laws:

1. ApprovalId identifies one occurrence and MUST NOT be reused.
2. Immutable Approval content MUST NOT change after issuance.
3. Approval binds one ProposalId. An identical ExactChangeBinding under another
   proposal occurrence requires another Approval.
4. The trusted boundary MUST structurally verify the retained immutable
   proposal and the complete ExactChangeBinding defined by ADR-0024.
5. Approval binds exact originator, executor, complete associated
   operation-family/mutation-class/scope write requirements, and authorization-
   policy version. Flattened operation-family, CanonicalWriteScope, and
   RequiredMutationClasses summaries MAY be retained for review or provenance
   but MUST NOT replace the bound relation. The Approval record separately
   identifies the trusted Human approver.
6. Disclosure scope is authorized independently and is not ApprovalBinding.
   Presentation or diagnostic projection changes do not redefine the approved
   semantic publication.
7. Rendered diff/prose, prompts, confidence, model/provider identity, UI
   coordinates, storage bytes/paths, and Git objects MUST NOT substitute for
   the binding.
8. A client-supplied Approval is untrusted. The authoritative boundary MUST
   verify it against trusted Approval and lifecycle state.
9. Approval MUST NOT become a transferable bearer credential.

The trusted authorization domain determines the effective authorization-policy
version governing each execution. Approval issuance MUST bind the version that
is effective at issuance. Approval-gated Execute MUST require the bound version
to equal the effective version governing execution, and that equality MUST
still hold at the publication boundary. A historically readable or supported
version is not execution authority after another version becomes effective;
the proposal requires a new Approval. Direct Human Execute uses the effective
current policy without fabricating an Approval or historical policy binding.
Exact version representation, policy selection mechanism, migration, and
support-window behavior remain Provisional.

### Authorizing Grant references

Approve Grant references used at issuance are immutable Approval evidence. A
reference resolves one exact non-reusable `(AuthorizationDomain, GrantId)`
occurrence and its fixed subject/content.

Immediately before Execute, the trusted boundary rederives the exact
authorization requirement and requires sufficient live Approve and Execute
Grants. Therefore:

- loss of effective Approve or Execute authority denies;
- each authorizing Approve Grant reference needed for issuance must remain
  valid and covering;
- an equivalent newly issued Approve Grant does not revive Approval after a
  referenced authorizing Grant expires or is revoked;
- the Execute Grant references used at publication are retained as provenance;
  and
- explicit Approval revocation permanently cancels the occurrence even if
  Grant authority later exists.

## Exact-change integrity boundary

Trusted structural equality with one retained immutable proposal record is
sufficient for this MVP. ProposalId alone is not content-integrity proof.

This specification does not select:

- canonical proposal or Approval bytes;
- a digest/hash algorithm or transcript;
- a commitment profile identifier;
- a signature, MAC, PKI, or key lifecycle;
- JSON/Serde, IPC, WASM, network, or SDK DTOs; or
- a portable/offline Approval token.

An Approval that crosses an untrusted boundary requires a separately Accepted
integrity-protected profile. An implementation MAY use an internal digest as a
Provisional optimization, but conformance rests on the logical structural
binding and trusted registry, not on any particular bytes or algorithm.

## Expiry, revocation, replay, retry, and stale behavior

The logical lifecycle is:

```text
Active -> Consumed | Revoked | Expired
```

### Expiry

- Every Approval MUST have finite expiry.
- Permanent Approval is invalid.
- Exact duration, maximum duration, time encoding, and trusted clock are
  Provisional host/security-profile choices.
- This specification recommends no concrete TTL.
- Unavailable or untrusted time fails closed.

### Revocation

- Active Approval MUST be revocable by its approver, a Grant issuer whose
  authority supported issuance, or the trusted authorization authority.
- Revocation is terminal and does not undo an already published transition.
- Missing revocation/use state fails closed.
- An ephemeral/session-bound Approval becomes unusable when its verifier state
  no longer exists.

### At-most-once successful publication

- Successful semantic publication and transition to Consumed MUST be atomic.
- One Approval can authorize at most one successful semantic publication.
- Failure before semantic publication MUST NOT consume Approval.
- A retry while Approval remains Active MUST repeat every current-base,
  identity, structural-binding, associated-write-requirement, principal, Grant,
  expiry, revocation, effective-policy-version, semantic-precondition,
  validation, and gate check.
- A consumed ApprovalId MUST fail replay without publication.
- Concurrent attempts MUST NOT both publish successfully.
- Approval-gated Execute MUST satisfy the common Execute publication condition
  below and additionally condition semantic publication and Approval
  consumption on the bound originator and approver occurrences, authorizing
  Approve Grant references, Approval state, and exact proposal/Approval/base
  binding still being valid.
- Revocation, expiry, principal disablement, Approval consumption, effective-
  policy change, or proposal-base invalidation racing with Approval-gated
  Execute MUST prevent publication.
- If the trusted boundary cannot prove the complete common and Approval-specific
  conjunction at the publication boundary, it MUST fail closed and publish
  nothing.
- Approval reservation, locking, and atomic-consumption coordination remain #29
  implementation work. Concrete revision concurrency and state installation
  remain #93 work; broader transaction/recovery and history protocols remain
  with #11/#12.
- If a failure leaves the trusted boundary unable to prove whether publication
  occurred, it MUST fail closed and MUST NOT permit retry until authoritative
  state is reconciled.

### Stale behavior

Stale behavior is exactly ADR-0024. A base mismatch is detected before
candidate construction against the changed base, publishes nothing, performs
no implicit rebase/merge/retarget/replay, and leaves the immutable proposal
unchanged. A stale or re-proposed patch requires another Approval. Later
semantic content equivalence does not revive the old base occurrence.

## Authorization algorithms

The steps below specify required logical checks, not a Rust API, DTO, database
transaction, or transport sequence.

### Authorize Query

```text
allow iff:
  authenticated Principal is active
  AND trusted authority derives complete operation-family/disclosure-scope
      requirements
  AND live Query Grants cover every complete requirement
```

### Authorize Propose

```text
allow iff:
  authenticated Principal is active
  AND trusted authority derives associated operation-family + write-scope +
      mutation requirements
  AND live Propose Grants cover the complete requirement
  AND command/base are admissible under the Semantic API
```

Returned evidence is separately filtered by the disclosure contract. Propose
publishes nothing and does not imply later Execute authority.

### Issue Approval

```text
allow iff:
  immutable proposal identity/content are structurally consistent
  AND proposal base is current
  AND trusted authority rederives associated operation-family + write-scope +
      mutation requirements
  AND approver is authenticated Human
  AND live Approve Grants cover the complete requirement
  AND named executor is active
  AND trusted authorization domain selects the effective policy version
  AND finite expiry + that effective policy version are recorded
```

The trusted record captures ApprovalBinding and the authorizing Approve Grant
references used at issuance.

### Common Execute publication rule

Every Execute path, including directly authenticated Human Execute without a
SemanticPatch or Approval, MUST satisfy this common publication-boundary law:

```text
1. Authenticate an active effective executor.
2. Determine the effective authorization policy selected by the trusted
   authorization domain and rederive every associated operation-family/
   mutation-class/scope requirement from trusted typed meaning and relevant
   semantic relationships under that policy.
3. Require sufficient live Execute Grants to cover every complete relational
   requirement.
4. Evaluate the candidate and authoritative gate against a known semantic
   context.
5. At the publication boundary, publish only while the executor occurrence is
   still active and authenticated, sufficient live relational Execute Grant
   coverage still exists, the authorization policy used for evaluation remains
   effective, the evaluated semantic context is still current (or an
   equivalent revision-safe condition holds), the authoritative gate result is
   still valid, and no unauthorized host/external effect is performed.
```

Execute-Grant revocation or expiry, executor disablement, effective-policy
change, relevant semantic state advance, gate invalidation, or inability to
prove the complete conjunction MUST prevent publication. Direct Human Execute
uses the effective current policy without an Approval or historical policy
binding. Approval-gated Execute adds its proposal/Approval conditions to this
common law; it does not replace or weaken it. Concrete reservation, locking,
transaction, revision, retry, and state-installation mechanics remain #29/#93
work.

### Authorize approval-gated Execute

```text
1. Load the trusted immutable proposal and Approval internally by opaque ID.
   Before re-evaluation, authorization, execution, or candidate construction
   against a changed base, compare the current semantic revision with the
   proposal base under ADR-0024. Retain the result without disclosing it.
2. Authenticate the caller and require caller == Approval-bound executor. An
   unauthenticated or unbound caller receives only a disclosure-safe
   authorization denial, regardless of the retained base result.
3. Reject unsupported Semantic API versions. Determine the effective
   authorization-policy version selected by the trusted authorization domain
   and require the Approval-bound version to equal it; historical readability
   or support is insufficient. Verify trusted immutable proposal
   identity/content, complete ADR-0024 ExactChangeBinding, retained relational
   AuthorizationFootprint, and exact ApprovalBinding equality, including
   authorization domain, ProposalId, originator, executor, complete associated
   operation-family/mutation-class/scope requirements, and policy version. A
   mismatch receives only a disclosure-safe binding denial.
4. Only after steps 2-3, expose the retained Stale outcome when the base did not
   match, before any candidate construction against the changed base. Stale
   details require sufficient Query authority.
5. For a current base, rederive associated operation-family,
   canonical-write-scope, and mutation-class requirements from typed meaning
   and require relational equality with the bound trusted footprint.
6. Recheck originator, approver, authorizing Approve Grant references,
   executor, sufficient current live Execute Grants, Approval expiry,
   revocation, and use state.
7. Re-run authoritative semantic preconditions, validation/calculation, and
   operation gate.
8. At the publication boundary, satisfy the common Execute publication rule.
   Additionally require the Approval-bound authorization-policy version still
   to equal the effective policy governing execution, and condition semantic
   publication and Approval consumption on the bound originator and approver
   occurrences, authorizing Approve Grant references, Approval state, and exact
   proposal/Approval/base binding still being valid. If any common or Approval-
   specific condition raced or cannot be proven valid, publish nothing.
9. When the complete condition holds, atomically publish all semantic state and
   mark Approval Consumed.
10. Return a disclosure-safe outcome, resulting revision on success, and
    minimum provenance.
```

The ADR-0024 comparison in step 1 is an internal semantic precondition check,
not authorization to disclose proposal state. After the executor and complete
ApprovalBinding checks, `Stale` identifies only the bound proposal's status; it
MUST NOT reveal the current revision or other semantic facts without sufficient
Query authority. Exact failure precedence and side-channel hardening remain #30
implementation work. An earlier preview, rendered diff, client gate result, or
model claim is not authority for step 7 or publication under step 8.

## Minimum provenance contract

Provenance is machine-readable evidence outside canonical semantic Document
state.

### Proposal provenance

At minimum:

```text
AuthorizationDomain
ProposalId + ExactChangeBinding reference
OriginatorPrincipalId
Propose Grant references
AuthorizationFootprint
AuthorizationPolicyVersion
```

When available, retain structured agent/provider/model/tool/orchestrator
identity/version and correlation facts as opaque provenance. They never grant
privilege. Full prompts/conversations are not required.

### Approval provenance

At minimum:

```text
ApprovalId + ApprovalBinding
HumanApproverPrincipalId
Approve Grant references used at issuance
IssuedAt + ExpiresAt
revocation/use evidence
```

### Successful approval-gated execution provenance

At minimum:

```text
ProposalId + exact-binding reference
Originator, Approver, and Executor PrincipalIds
ApprovalId
Approve/Execute Grant references used at execution
issuance Grant references or durable reference to Approval
CanonicalWriteScope + RequiredMutationClasses
AuthorizationPolicyVersion
base semantic revision + resulting semantic revision
authoritative gate/report reference
Approval terminal state Consumed
agent/provider/model/tool facts at execution when known
```

### Direct Human execution provenance

A directly authenticated Human Execute that legitimately requires no proposal
or Approval MUST NOT fabricate:

- ProposalId or ExactChangeBinding;
- originator or approver roles that do not exist;
- ApprovalId or Approve Grant references; or
- Approval terminal state Consumed.

Such a receipt MAY retain executor identity, effective Execute Grant
references, the trusted AuthorizationFootprint and policy version, relevant
input and resulting revisions, and gate/result evidence. This permission does
not require a receipt or freeze its shape. Exact receipt/history DTO, storage,
retention, and broader history architecture remain #29/#12 work.

Additional Approval-gated provenance laws:

1. The immutable proposal or a durable lossless reference MUST survive; a
   digest alone would not explain what was approved.
2. Provider/model change does not alter authority when trusted principals and
   ApprovalBinding are unchanged, but changed execution facts are recorded when
   known.
3. Full prompts, hidden reasoning, credentials, secrets, and complete chat
   transcripts are not minimum provenance.
4. Provenance MUST NOT be written into `.roproj` merely to make it durable.
5. Event sourcing, a universal operation log, CRDT, or tamper-evident audit
   ledger is not required.
6. Storage, retention, redaction, history UI, and receipt DTOs remain
   Provisional/Deferred to #29/#12.

## Semantic and external-effect separation

Semantic Grants and Approvals authorize only the Semantic API domain. They do
not authorize:

- filesystem or browser persistence access;
- `.roproj` or `.ro` materialization;
- network access;
- process or shell execution;
- Git commit, push, merge, or repository administration;
- plugin/connector invocation;
- deployment/publication;
- credentials/secrets access; or
- another host effect.

A host may materialize or externally publish an already-authorized semantic
result only under separate authority. External-effect capability vocabulary is
Deferred and is not invented by this specification.

A denied request MAY produce separately authorized security/audit evidence.
That recording is not semantic publication, does not consume Approval, and does
not authorize any requested host effect; concrete denial logging remains #30 or
later audit-policy work.

## Failure meanings

A conforming client can distinguish, where applicable:

- principal unavailable/disabled;
- capability missing;
- disclosure or write scope denied;
- mutation class denied/unknown;
- approval required/missing;
- approver not trusted Human;
- proposal occurrence or exact binding mismatch;
- originator, approver, or executor mismatch;
- authorization-policy version unsupported or not the effective version;
- approval expired, revoked, consumed, or state unavailable;
- live Approve or Execute authority lost;
- stale proposal under ADR-0024;
- semantic admission/precondition/gate failure; and
- external effect denied.

Authorization outcomes remain separate from semantic `ValidationReport`,
representation failure, and host failure. Stable machine meaning is required;
exact code strings, Rust enums, transport tags, and messages remain
Provisional under #30. A denial MUST NOT disclose semantic content outside
authorized disclosure scope.

## Required conformance scenarios

1. Query-only principal cannot Propose, Approve, or Execute.
2. Propose-only principal can issue an inert covered proposal but cannot
   Execute.
3. Preview evidence outside Query scope is denied or safely reduced.
4. Fine-grained scope does not match a same-spelled subordinate ID in another
   DocumentId.
5. Moving/retargeting across containers requires old- and new-side coverage.
6. Covered ordinary Value proposal with exact Human Approval publishes once.
7. Value-only authority denies Formula mutation.
8. Non-destructive authority denies entity/schema/formula removal.
9. Changed target/value/formula/generated ID/base/body/order/contract requires
   another proposal and Approval under ADR-0024.
10. Identical ExactChangeBinding under another ProposalId cannot reuse Approval.
11. Any intervening semantic publication makes proposal/Approval stale.
12. Expired or explicitly revoked Approval denies without publication.
13. Expiry/revocation of an authorizing Approve Grant or loss of sufficient
    live Execute authority denies; an equivalent new Approve Grant does not
    revive Approval, while a different live Execute Grant set may satisfy the
    fresh executor-authority recheck and is retained as execution provenance.
14. A failed authoritative semantic gate publishes nothing and leaves Approval
    Active when no publication occurred.
15. A retry repeats every live authorization and semantic check.
16. Concurrent attempts cannot both publish; consumed Approval denies replay.
17. An uncertain publication outcome fails closed until reconciled.
18. Provider/model change with unchanged principals/binding affects provenance,
    not privilege.
19. Delegated self-approval denies.
20. Semantic authorization cannot grant filesystem/network/process/Git/plugin/
    deployment/persistence effects.
21. Mixed AtomicBatch requires the union of associated write requirements, one
    whole-batch Approval, and all-or-nothing publication.
22. Cross-pairing Formula authority for one field with Value authority for
    another cannot authorize either class on the other field, and exact
    Approval cannot flatten those pairs into independent scope/class sets.
23. A revoked Grant occurrence cannot be reactivated or have its GrantId
    reused; a newly issued equivalent Grant does not revive an Approval that
    references the revoked occurrence.
24. A same-spelled GrantId from another authorization domain, or a Grant for
    another subject, provides no coverage.
25. Delegated origin with a Human executor still requires Human Approval.
26. Human origin with a Delegated executor still requires Human Approval.
27. Substituting either the bound originator or bound executor denies.
28. Deleting, transferring, recreating, or replacing an account never
    reassigns its PrincipalId; a replacement subject receives a new PrincipalId
    and cannot inherit the original occurrence's Grants, Approval
    originator/executor bindings, or provenance through a reused login, email,
    provider identifier, or alias.
29. Revoking a required authorizing Approve Grant or relied-upon Execute Grant
    after the ordinary gate check but before publication prevents publication
    and Approval consumption.
30. Disabling any required principal after the ordinary gate check but before
    publication prevents publication and Approval consumption.
31. Concurrent Approval revocation, expiry, or consumption prevents another
    publication.
32. Any base revision change before publication prevents candidate installation
    and leaves the proposal stale.
33. An unauthenticated or wrong executor cannot distinguish current, stale,
    missing, or mismatched proposal state and receives only a disclosure-safe
    authorization denial.
34. A bound executor with a mismatched ProposalId or ApprovalBinding cannot
    probe another proposal through Stale; binding mismatch denies before stale
    disclosure.
35. Revoking or expiring relied-upon Execute authority after ordinary
    authorization/gating but before direct Human publication publishes nothing.
36. Disabling the directly authenticated Human executor before publication
    publishes nothing.
37. Relevant semantic state advance before direct Human publication prevents
    installation of a candidate evaluated against obsolete state.
38. Successful direct Human Execute does not manufacture proposal, originator,
    approver, Approval, Approve Grant, or Consumed provenance.
39. Successful Approval-gated Execute retains the complete proposal/Approval
    provenance above and consumes Approval atomically with publication.
40. Query authority for operation family A over one disclosure scope does not
    authorize Query family B over that same scope.
41. Execute authority for operation family A over one mutation class and scope
    does not authorize Execute family B over that same class and scope.
42. AtomicBatch retains each member's operation-family/mutation-class/scope
    associations; flattened family, class, or scope unions cannot synthesize a
    covered member tuple.
43. An Approval bound to policy V1 denies and requires a new Approval when V2
    is the effective execution policy, even if V1 remains readable or
    historically supported.
44. A change in the effective authorization policy between authorization/gate
    evaluation and publication prevents publication on every Execute path and,
    for Approval-gated Execute, also prevents Approval consumption.
45. Direct Human Execute uses the effective current policy without fabricating
    an Approval or historical policy binding.

## Stability classification

| Concept | State |
| --- | --- |
| Non-reusable, non-reassignable Principal occurrences within one trusted authorization domain | Accepted |
| Human versus Delegated distinction for MVP policy | Accepted |
| Principal/domain encoding and authentication mechanism | Provisional host concern |
| Query, Propose, Approve, Execute non-implication | Accepted |
| Operation-family identity as an independent checked capability dimension | Accepted under ADR-0020/ADR-0026 |
| Exact operation-family identifiers and catalogue | Provisional |
| Capability identifier strings and public representation | Provisional |
| Value, Formula, Structure, Schema, Destructive meanings | Accepted MVP contract |
| Complete Stable command-family mapping | Provisional; published mappings cannot change silently |
| Closed document-local stable-ID scope concepts and containment | Accepted MVP contract |
| Project/workspace/org/tenant/predicate scope | Deferred |
| Trusted AuthorizationFootprint derivation | Accepted |
| Associated operation-family/mutation-class/scope coverage, combined with the requested action, without crossed-Grant or Approval unions | Accepted |
| Immutable, non-reusable, default-deny Grant occurrences with terminal revocation | Accepted |
| Grant registry/admin/DTO/clock representation | Provisional |
| Exact Human Approval for Delegated-origin or Delegated-authority publication | Accepted current MVP policy |
| ApprovalBinding fields in this specification | Accepted |
| Authorizing Approve Grant references remain valid and covering | Accepted |
| Fresh executor-authority recheck before Execute | Accepted |
| Approval-bound policy version equals the effective execution policy through publication | Accepted |
| Policy-version representation and effective-policy selection mechanism | Provisional |
| Structural equality with trusted immutable proposal | Accepted MVP profile |
| Canonical bytes, digest/hash/transcript/signature/MAC | Deferred |
| Approval is not transferable bearer authority | Accepted MVP profile |
| Portable/offline Approval protocol | Deferred |
| Finite expiry with no fixed TTL | Accepted law / Provisional value |
| Explicit revocation and fail-closed unverifiable state | Accepted |
| Common conditional publication-boundary safety for every Execute path | Accepted |
| At-most-once conditional publication and consumption while the complete publication-boundary authorization condition remains valid, plus replay denial | Accepted |
| Approval reservation/locking/atomic-consumption mechanics | #29 Provisional implementation |
| Concrete revision concurrency/state-installation mechanics | #93 Provisional implementation |
| Minimum proposal/approval/Approval-gated execution provenance without fabricated Approval facts on direct Human Execute | Accepted |
| Provenance store/retention/redaction/tamper evidence/UI | Provisional/Deferred |
| Provider/model as provenance rather than privilege | Accepted under ADR-0007 |
| Semantic/host-effect separation | Accepted under ADR-0007/ADR-0022 |
| External-effect capability vocabulary | Deferred |
| Roles/groups/ABAC/policy DSL/SSO/SCIM/tenancy | Deferred |
| Auto-approval, autonomous mutation, quorum/multi-party approval | Deferred |
| Broader transaction/recovery and event sourcing/operation log/undo/history protocol | Deferred to #11/#12 |
| Public Rust/Serde/wire authorization DTO | Deferred |

## Ownership boundaries

- ADR-0026 owns the authorization and Approval decision; this specification
  normatively elaborates that decision without amending it.
- ADR-0024/#27 own proposal occurrence, immutable contents,
  ExactChangeBinding, exact base, and stale behavior unchanged.
- ADR-0020 owns Query/Command/Propose/Execute and semantic publication
  atomicity.
- The trusted identity/host boundary owns Principal occurrence issuance and
  resolution and MUST preserve non-reassignment across its account lifecycle;
  exact account/provider mechanisms remain Provisional.
- #29 owns the proposal/Approval lifecycle registry,
  reservation/consumption implementation, atomic apply/verify, receipts, and
  provenance persistence.
- #30 owns trusted enforcement, instruction/data separation, bypass prevention,
  disclosure-safe denials, host-effect denial, and security diagnostics/tests.
- #93 owns concrete resident session/revision/concurrency/state-installation
  mechanics.
- #11 owns broader team/enterprise permissions, reusable policy questions, and
  transaction/recovery architecture.
- #12/history work owns persisted history, event sourcing, undo, and retention.
- `workspace-engine` remains shared semantic transition/gate authority;
  authorization must not exist only in `ai-api`, UI, or client convention.
- host/storage adapters remain separate effect authorities.

## Explicit non-goals

- enterprise RBAC administration, groups, organizations, tenants, SSO, SCIM;
- generic authorization, policy, or scope-expression language;
- reusable automatic approval or autonomous mutation;
- multi-party approval, quorum, escalation, or workflow chains;
- canonical approval bytes, SHA-256 profile, signature/MAC, or portable token;
- public Rust/Serde/JSON/IPC/WASM/network DTOs;
- external-effect capability design or plugin/network sandboxing;
- revision/session/concurrency implementation;
- approval UI or redaction DTO design;
- event sourcing, universal operation log, CRDT, undo, or rollback; or
- production implementation in this documentation decision.
