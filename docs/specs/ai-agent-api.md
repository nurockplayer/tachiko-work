# AI Agent API Specification

Decision state: Mixed. ADR-0007 establishes AI as a semantic client and keeps
mutation approval-gated at this stage. ADR-0020 establishes the first-class
Headless Semantic API as the semantic behavior boundary shared by AI and other
first-party clients. Exact capability identifiers, principals, grants,
approval tokens, execution authorization, and provenance remain #27/#28.

Implementation state: the provider-free `tachiko-ai-api` crate implements a v0.1
AI-facing read/explain/suggest adapter over `tachiko-workspace-engine`. The
current Rust DTOs are not the public Semantic API contract.

## Principle

AI interacts with Tachiko Work through semantic operations and capabilities.

It must not simulate mouse/keyboard usage as the primary architecture and must
not gain a second semantic mutation/validation policy merely because its
provider-facing interface differs from GUI/CLI.

## Relationship to the Semantic API

[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md) and
[`semantic-api.md`](semantic-api.md) own the shared semantic behavior:

- Query semantics;
- typed semantic Commands;
- Propose versus Execute;
- authoritative validation/gating;
- formula outcome meaning;
- semantic atomicity;
- capability-addressability; and
- compatibility/versioning laws.

The AI layer owns provider/agent-facing projections and the security/approval
experience that later #27/#28 decisions authorize. It does not become a parallel
semantic API.

Long term, AI-facing read/explain/suggest/execute experiences should map to
Semantic API Query/Propose/Execute capabilities rather than depend on the exact
internal `Document` field layout or current workspace-engine Rust signatures.

## Current operations

Implemented v0.1 operations are read, explain, and suggest-only:

```text
describe_document(document)
explain_formula(document, field_ref)
explain_impact(before, after)
suggest_field_change(document, field_ref, value)
```

No current AI API writes the document directly.

Suggestions are inert proposal objects. Formula analysis, semantic comparison,
typed proposal construction, validation, and calculation delegate to the shared
workspace-engine application authority.

`describe_document` currently builds an AI-facing projection from internal
semantic structures. That is acceptable implementation evidence for the
provider-free adapter, but it is not a precedent that future external AI clients
may depend on Rust `Document` field layout. Stable long-lived AI consumers should
use intentional Semantic API query projections as those operations are promoted.

## Proposal and execution boundary

ADR-0020 requires semantic Propose and Execute to share the same command meaning
and authoritative gates.

For AI:

- query/read capability does not imply propose;
- propose does not imply execute;
- an inert proposal does not publish semantic state;
- approval UX or a previous gate result does not authorize a later stale
  execution automatically; and
- an approved execution must use the same shared Semantic API command/gate
  semantics as the equivalent non-AI first-party operation.

The current `requires_approval` boolean preserves v0.1 safety behavior only. It
does not define the future capability, principal, grant, approval token,
provenance, stale-proposal, or execution protocol.

## Capability-addressability

ADR-0020 accepts the principle that each semantic operation or family can be
independently addressed as a capability.

AI adapters must therefore be able to expose a bounded subset such as read/query
or propose without implicitly exposing arbitrary execute authority.

Exact capability identifiers and authorization semantics remain #27/#28.

## Safety

AI operations should be:

- typed;
- validated;
- reviewable;
- capability-bounded;
- approval-gated where mutation authority requires it;
- deterministic in their semantic evaluation; and
- non-persistent unless an explicitly authorized Execute path performs the same
  shared semantic operation used by other first-party clients.

## Formula suggestions

Implemented formula suggestions:

- accept only typed `Value::Formula` proposals that target numeric fields;
- apply the shared complexity limit (`256` nodes, `64` post-desugar depth,
  `4096` canonical bytes);
- return an inert, validated workspace-engine candidate and mark the AI-facing
  proposal as requiring approval;
- require a separate approved host execution before publication; and
- never replace formulas with scalars automatically (formula-to-scalar
  suggestions are rejected).

These behaviors are current implementation evidence constrained by the Accepted
formula/validation contracts. The exact proposal DTO is not stabilized by
ADR-0020.

## Project Memory

Issue #104 may later consume read/query/propose capabilities as a dogfood case.
That does not promote Project Memory concepts, GitHub identifiers, or provenance
workflow into the semantic core or into this AI adapter contract.

## Goal

AI is a native, capability-bounded semantic participant that uses the same
meaning and operation authority as human-facing clients, while provider-specific
interaction and approval/security remain replaceable adapter layers.
