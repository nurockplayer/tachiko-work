# AI Agent API Specification

Decision state: Mixed. ADR-0007 establishes AI as a delegated semantic client with no intrinsic authority, keeps every AI-originated canonical mutation approval-gated at the current MVP stage, and separates semantic validity from authorization. ADR-0020 establishes the first-class Headless Semantic API as the semantic behavior boundary shared by AI and other first-party clients. Exact capability identifiers, principals, grants, approval tokens, execution authorization, provenance, stale/replay mechanics, and runtime placement remain #27/#28/#26 as applicable.

Implementation state: the provider-free `tachiko-ai-api` crate implements a v0.1 AI-facing read/explain/suggest adapter over `tachiko-workspace-engine`. The current Rust DTOs are not the public Semantic API contract.

## Principle

AI interacts with Tachiko Work through semantic operations and explicitly delegated capabilities.

AI has no intrinsic mutation authority. Provider, model, tool, prompt, confidence, or self-reported validation metadata is provenance/evidence, not privilege and not semantic truth.

AI must not simulate mouse/keyboard usage as the primary architecture and must not gain a second semantic mutation/validation policy merely because its provider-facing interface differs from GUI/CLI.

## Relationship to the Semantic API

[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md) and [`semantic-api.md`](semantic-api.md) own the shared semantic behavior:

- Query semantics;
- typed semantic Commands;
- Propose versus Execute;
- authoritative validation/gating;
- formula outcome meaning;
- semantic atomicity;
- capability-addressability; and
- compatibility/versioning laws.

ADR-0007 adds the AI-authority constraint: a first-party AI Execute path must use the same shared semantic transition/gating behavior and must cross trusted authorization/approval enforcement. That enforcement is mandatory as an architecture invariant, but its concrete principal model, grant protocol, approval representation, provenance shape, and runtime placement are not defined here.

The AI layer owns provider/agent-facing projections and review presentation. It does not become a parallel semantic API and must not be the sole enforcement point for canonical mutation authority.

Long term, AI-facing read/explain/suggest/execute experiences should map to Semantic API Query/Propose/Execute capabilities rather than depend on the exact internal `Document` field layout or current workspace-engine Rust signatures.

## Current operations

Implemented v0.1 operations are read, explain, and suggest-only:

```text
describe_document(document)
explain_formula(document, field_ref)
explain_impact(before, after)
suggest_field_change(document, field_ref, value)
```

No current AI API writes the document directly.

Suggestions are inert proposal objects. Formula analysis, semantic comparison, typed proposal construction, validation, and calculation delegate to the shared workspace-engine application authority.

`describe_document` currently builds an AI-facing projection from internal semantic structures. That is acceptable implementation evidence for the provider-free adapter, but it is not a precedent that future external AI clients may depend on Rust `Document` field layout. Stable long-lived AI consumers should use intentional Semantic API query projections as those operations are promoted.

## Proposal, authorization, and execution boundary

ADR-0020 requires semantic Propose and Execute to share the same command meaning and authoritative gates. ADR-0007 additionally requires semantic validity/gating and principal authorization/approval to remain independent prerequisites.

For AI:

- query/read capability does not imply propose;
- propose does not imply execute;
- an inert proposal does not publish semantic state;
- a successful semantic gate does not grant permission to execute;
- delegated permission does not override a failed semantic gate;
- approval or a previous gate result does not authorize a materially changed transition or materially changed relevant authorization context automatically; and
- an approved execution must use the same shared Semantic API command/gate semantics as the equivalent non-AI first-party operation.

The current `requires_approval` boolean preserves v0.1 safety behavior only. It does not define the future capability, principal, grant, approval token, provenance, base/revision binding, stale-proposal, replay, revocation, or execution protocol.

## Capability-addressability

ADR-0020 accepts the principle that each semantic operation or family can be independently addressed as a capability.

AI adapters must therefore be able to expose a bounded subset such as read/query or propose without implicitly exposing arbitrary execute authority.

Exact capability identifiers, scope grammar, delegation, and authorization semantics remain #27/#28.

## Effect separation

Semantic publication, durable persistence, and external publication or host side effects are distinct authority domains.

A semantic edit capability must not imply filesystem, network, process, Git push, plugin, deployment, or other host authority. Provider-facing AI adapters must not create raw storage or host-effect paths that act as alternate semantic mutation authority.

Storage and host adapters may materialize or externally publish an authorized semantic result under their own authority; they do not redefine semantic meaning or grant semantic permission.

## Safety

AI operations should be:

- typed;
- validated;
- reviewable;
- capability-bounded;
- approval-gated where mutation authority requires it;
- provider-neutral in semantic and authorization meaning;
- deterministic in semantic evaluation; and
- non-persistent unless an explicitly authorized Execute path performs the same shared semantic operation used by other first-party clients and the relevant host/persistence authority also permits the side effect.

Machine-generated statements such as `validated=true`, `approved=true`, or high confidence never substitute for deterministic Tachiko validation or trusted authorization/approval evidence.

## Formula suggestions

Implemented formula suggestions:

- accept only typed `Value::Formula` proposals that target numeric fields;
- apply the shared complexity limit (`256` nodes, `64` post-desugar depth, `4096` canonical bytes);
- return an inert, validated workspace-engine candidate and mark the AI-facing proposal as requiring approval;
- require a separate approved host execution before semantic publication; and
- never replace formulas with scalars automatically (formula-to-scalar suggestions are rejected).

These behaviors are current implementation evidence constrained by the Accepted formula/validation contracts. The exact proposal DTO is not stabilized by ADR-0007 or ADR-0020.

## Project Memory

Issue #104 may later consume read/query/propose capabilities as a dogfood case. That does not promote Project Memory concepts, GitHub identifiers, or provenance workflow into the semantic core or into this AI adapter contract.

## Goal

AI is a native, capability-bounded semantic participant that uses the same meaning and operation authority as human-facing clients while exercising only explicitly delegated authority. Provider-specific interaction remains replaceable, and approval/security mechanisms remain enforced by trusted product boundaries rather than by model claims or adapter convention.
