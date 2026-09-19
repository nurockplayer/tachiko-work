# ADR-0007: AI Semantic Interaction Model

## Status
Accepted

Amendment decision issue: [#421](https://github.com/nurockplayer/tachiko-work/issues/421)

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

First-party AI Execute paths must cross a trusted authorization/approval enforcement boundary and the same shared semantic transition/gating path used by equivalent non-AI first-party operations. ADR-0024 owns the immutable revision-pinned proposal and exact-change binding laws. The concrete enforcement placement, principal model, capability identifiers, grants, approval representation, provenance fields, digest/integrity, and authorization replay mechanics remain owned by #28 and runtime/host work where applicable.

### MVP permissions

During the current MVP stage:

- read: allowed;
- analysis: allowed;
- explanation: allowed;
- suggestions / Propose: allowed;
- direct canonical mutation / Execute: requires explicit approval.

Approval applies to the proposed semantic transition in its relevant authorization context. It must not silently carry over when the approved transition or relevant context has materially changed. ADR-0024 defines the representation-neutral exact proposal/base binding; digest, lifetime, replay, revocation, token, and concrete revision mechanics remain #28/#93.

### Effect separation

Semantic publication, durable persistence, and external publication or host side effects are separate authority domains.

A semantic mutation capability does not implicitly grant filesystem, network, process, Git push, plugin, deployment, or other external-effect authority. Storage and host layers materialize or publish an already-authorized semantic result; they do not redefine semantic meaning or mint semantic authorization.

### Provider and deployment neutrality

Authorization, approval, semantic operations, validation, and canonical-state rules are provider- and deployment-neutral.

An AI client may be hosted by a model vendor, hosted by the user's organization, self-hosted, local/on-device, or absent entirely. Canonical semantic meaning, deterministic calculation and validation, the open ownership path, and equivalent non-AI first-party workflows MUST NOT require one AI provider, one model family, or a remote AI service.

Changing AI provider or deployment location MUST NOT change semantic command meaning, authoritative gates, authorization, approval, or canonical-state rules. Local and self-hosted agents remain subject to the same trusted principal, scope, proposal, validation, approval, and Execute boundaries as hosted agents.

Provider-specific SDKs, hosted APIs, MCP, A2A, local-runtime bridges, and similar integration protocols are replaceable boundary adapters unless a narrower Accepted decision explicitly gives one a durable role. They do not become canonical document semantics or a second semantic API merely because they are convenient integration surfaces.

This decision does not require equal capability across models, bundle model weights, select a local inference runtime, promise particular hardware support, or stabilize a provider protocol.

Future bounded autonomous mutation may be permitted through explicit, scoped delegation after capability, review, recovery, and safety policy matures. Such delegation does not make the agent a canonical source of truth or allow it to bypass deterministic semantic gates.

## Consequences

The AI API should expose semantic operations, document structure, formulas, impact analysis, and reviewable proposals through the shared Semantic API rather than a provider-specific mutation path.

AI-generated content and operations remain untrusted inputs until admitted through the authoritative semantic and authorization boundaries.

Hosted, organization-hosted, self-hosted, and local model clients can evolve independently of the canonical semantic substrate. A remote AI subscription or provider account is not a prerequisite for semantic ownership, deterministic correctness, or the non-AI operation of legitimately held Tachiko work.

The current `requires_approval` behavior is an MVP safety posture, not a permanent public approval protocol.

Autonomous agents, capability/grant vocabulary, approval/provenance and digest mechanics, raw-host security boundaries, and unrestricted editing remain deferred to their narrower owning issues rather than being frozen here. ADR-0024's proposal contract does not grant any of those authorities.
