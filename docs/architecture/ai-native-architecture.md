# AI-Native Architecture

Decision state: Accepted direction under
[ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md),
[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md), and
[ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md).
Reviewable semantic proposals use the immutable revision-pinned SemanticPatch
contract Accepted by
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md).
Concrete lifecycle/registry implementation remains #29, trusted security
boundary enforcement remains #30, and revision/session mechanics remain #93.

## Principle

AI should operate on semantic objects, not simulate mouse and keyboard actions.

Traditional workflow:

```text
User interface -> file format -> AI workaround
```

Tachiko Work workflow:

```text
trusted Delegated principal
-> explicitly granted Query / Propose capability and stable-ID scope
-> typed Command or ordered AtomicBatch
-> revision-pinned immutable SemanticPatch / Propose
-> trusted AuthorizationFootprint + safely scoped review evidence
-> exact approval from one authorized Human principal
-> trusted authorization + current-base + authoritative-gate checks
-> authorized Execute
-> canonical semantic state + resulting revision/provenance
```

The proposal envelope reuses the Semantic API operation vocabulary. It is not a
second AI-only mutation API, does not itself become a Command, and does not
write `.roproj` or grant approval.

## Authorization shape

ADR-0026 defines the current MVP boundary:

- Principal identity is opaque and supplied by a trusted host/session boundary.
- Provider, model, tool, prompt, and confidence are provenance rather than
  privilege.
- Query, Propose, Approve, and Execute are independent actions, and each
  operation family is independently capability-addressable.
- Grants are default-deny and may contain a finite union of stable-ID Document,
  Schema, SchemaField, Entity, and EntityField scope atoms.
- ADR-0007's allowed current-MVP Query/Propose flows are preserved through
  explicit trusted-host Grant provisioning, not ambient AI authority.
- Value, Formula, Structure, Schema, and Destructive authority are independent.
- The trusted application derives operation-family/disclosure-scope and
  operation-family/mutation-class/write-scope relations; the agent cannot
  authoritatively declare its footprint.
- Propose does not grant arbitrary Query authority; preview evidence outside
  Query scope is denied or safely reduced.
- A patch originated by a Delegated principal or executed using Delegated
  authority requires one exact Human Approval.
- Approval binds the proposal occurrence, complete ADR-0024
  `ExactChangeBinding`, originator, exact executor, complete associated
  operation-family/mutation-class/scope write requirements, and the effective
  authorization-policy version, which must remain effective through
  publication. The trusted record also identifies the Human approver and
  authorizing Approve Grants.
- Approval has finite lifetime, is revocable, and can authorize at most one
  successful semantic publication.
- It is consumed atomically with successful semantic publication; failure
  before publication does not consume it.
- A changed proposal/relational authorization footprint, stale base, changed
  principal, changed effective authorization policy, invalid authorizing
  Approve Grant, Approval expiry/revocation, or consumption requires new
  approval. Insufficient live Execute authority denies the current execution
  attempt; a still-Active Approval may be retried after the bound executor
  obtains sufficient live Execute authority.
- Validation and operation gates remain independent authority and cannot be
  overridden by approval.

Current `Suggestion.requires_approval` remains an inert adapter safety marker,
not an implementation or wire precedent for this contract.

## Effect separation

Semantic publication, durable persistence, and host/external effects remain
separate authority domains.

Semantic Query/Propose/Approve/Execute does not authorize:

- `.roproj` or `.ro` materialization;
- filesystem or browser persistence;
- network access;
- process/shell execution;
- Git operations;
- plugin/connector execution;
- deployment/publication; or
- credentials/secrets access.

A host may materialize or externally publish an already-authorized semantic
result only under separate host authority. Provider-facing AI adapters must not
expose raw storage or host effects as alternate semantic mutation paths.

## Provenance

A reviewable Delegated proposal and successful execution retain machine-readable
provenance sufficient to identify:

- proposal and exact-binding reference;
- semantic base and resulting revision;
- originator, Human approver, and executor principals;
- grants and approval relied upon;
- trusted footprint, policy version, and final gate/report reference; and
- agent/provider/model/tool evidence when known.

Provenance is audit/history evidence, not command meaning, canonical Document
state, or privilege. Raw prompts, hidden reasoning, secrets, and complete chat
transcripts are not minimum provenance.

## Examples

An AI agent can:

- inspect and explain document structure;
- explain formula dependencies and calculated impact;
- detect inconsistencies;
- propose typed Value, Formula, Structure, or Schema changes within granted
  classes and scope;
- propose migrations or schema strengthening for Human review; and
- request Execute only when its exact scoped grants and Human approval permit it.

The document model itself becomes an API for intelligent operations, while the
trusted authorization boundary prevents a model, provider, document, or plugin
from becoming its own source of authority.
