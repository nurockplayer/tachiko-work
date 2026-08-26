# AI-Native Architecture

Decision state: Accepted direction under
[ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md),
[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md), and
[ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-exact-approval.md).
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
trusted Machine principal
-> explicitly granted Query / Propose capability
-> typed Command or ordered AtomicBatch
-> revision-pinned immutable SemanticPatch / Propose
-> deterministic semantic review evidence
-> exact approval from a distinct Human principal
-> trusted authorization + current-base + authoritative-gate checks
-> single-use authorized Execute
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
- Query, Propose, Approve, and Execute are independent actions.
- Reusable grants are default-deny and scoped to one exact semantic DocumentId.
- Data, Formula, Schema, and Destructive authority are independently grantable.
- Every Machine Execute requires exact approval from a distinct Human principal.
- Approval binds the proposal occurrence, complete ADR-0024
  `ExactChangeBinding`, exact base, exact executor, mutation classes, approval
  profile, and grants relied upon.
- Approval has finite lifetime, is revocable, and is single-use.
- A changed proposal, stale base, changed executor, revoked/replaced grant,
  expiry, revocation, or consumption requires new approval.
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

A reviewable Machine proposal and successful execution retain machine-readable
provenance sufficient to identify:

- proposal and exact-change commitment;
- semantic base and resulting revision;
- proposer, Human approver, and Machine executor principals;
- grants and approval relied upon;
- mutation classes and final gate outcome;
- timestamps and execution result; and
- agent/provider/model/tool evidence when known.

Provenance is audit/history evidence, not command meaning, canonical Document
state, or privilege. Raw prompts, hidden reasoning, secrets, and complete chat
transcripts are not minimum provenance.

## Examples

An AI agent can:

- inspect and explain document structure;
- explain formula dependencies and calculated impact;
- detect inconsistencies;
- propose typed Data or Formula changes within granted classes;
- propose migrations or schema strengthening for Human review; and
- request Execute only when its exact scoped grants and Human approval permit it.

The document model itself becomes an API for intelligent operations, while the
trusted authorization boundary prevents a model, provider, document, or plugin
from becoming its own source of authority.
