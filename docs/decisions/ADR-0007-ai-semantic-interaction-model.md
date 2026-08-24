# ADR-0007: AI Semantic Interaction Model

## Status
Accepted

## Context

AI systems should understand and manipulate the underlying meaning of work rather than automate clicks against traditional interfaces.

The original MVP decision established AI as a semantic client and required explicit approval for direct mutation. Subsequent acceptance of ADR-0015 through ADR-0020 clarified stable semantic identity, storage boundaries, deterministic formula behavior, validation/gating, the shared workspace-engine application boundary, and the first-class transport-neutral Semantic API.

Issue #9 revisited the remaining authority question: an AI may originate or operate a change, but model output, provider identity, semantic validity, and authorization are different concepts and must not collapse into one source of authority.

## Decision

AI interacts through the Tachiko Work semantic layer and the Accepted laws of the first-class Semantic API.

### Authority

AI has no intrinsic authority and is never the canonical source of truth. An AI may act only as a principal exercising explicitly delegated authority.

Provider, model, tool, prompt, confidence, or self-reported validation metadata is provenance or evidence, not privilege and not semantic truth.

A canonical semantic mutation requires all of the following independently:

- the requested semantic operation is admissible;
- the deterministic purpose-specific semantic gate permits publication;
- the principal has sufficient delegated authority for the affected scope; and
- any approval required by policy is satisfied.

Semantic validity does not grant authorization. Authorization cannot override semantic failure.

First-party AI Execute paths must cross a trusted authorization/approval enforcement boundary and the same shared semantic transition/gating path used by equivalent non-AI first-party operations. The concrete placement, principal model, capability identifiers, grants, approval representation, provenance fields, and stale/replay mechanics remain owned by #27/#28 and runtime/host work where applicable.

### MVP permissions

During the current MVP stage:

- read: allowed;
- analysis: allowed;
- explanation: allowed;
- suggestions / Propose: allowed;
- direct canonical mutation / Execute: requires explicit approval.

Approval applies to the proposed semantic transition in its relevant authorization context. It must not silently carry over when the approved transition or relevant context has materially changed. Exact digest, revision, lifetime, replay, revocation, and token mechanics remain #28.

### Effect separation

Semantic publication, durable persistence, and external publication or host side effects are separate authority domains.

A semantic mutation capability does not implicitly grant filesystem, network, process, Git push, plugin, deployment, or other external-effect authority. Storage and host layers materialize or publish an already-authorized semantic result; they do not redefine semantic meaning or mint semantic authorization.

### Provider neutrality and future autonomy

Authorization, approval, semantic operations, validation, and canonical-state rules are provider-neutral.

Future bounded autonomous mutation may be permitted through explicit, scoped delegation after capability, review, recovery, and safety policy matures. Such delegation does not make the agent a canonical source of truth or allow it to bypass deterministic semantic gates.

## Consequences

The AI API should expose semantic operations, document structure, formulas, impact analysis, and reviewable proposals through the shared Semantic API rather than a provider-specific mutation path.

AI-generated content and operations remain untrusted inputs until admitted through the authoritative semantic and authorization boundaries.

The current `requires_approval` behavior is an MVP safety posture, not a permanent public approval protocol.

Autonomous agents, capability/grant vocabulary, approval/provenance mechanics, raw-host security boundaries, and unrestricted editing remain deferred to their narrower owning issues rather than being frozen here.
