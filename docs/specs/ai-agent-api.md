# AI Agent API Specification

Decision state: Mixed. ADR-0007 establishes AI as a delegated semantic client
with no intrinsic authority, keeps every AI-originated canonical mutation
approval-gated at the current MVP stage, and separates semantic validity from
authorization. ADR-0020 establishes the first-class Headless Semantic API as
the semantic behavior boundary shared by AI and other first-party clients.
ADR-0024 establishes the provider-neutral immutable revision-pinned
SemanticPatch and exact-change binding used for reviewable proposals.
ADR-0021 permits AI-assisted progressive semantic strengthening while keeping
inference advisory rather than authoritative. ADR-0026 establishes
domain-scoped Human/Delegated principals, independent capabilities, stable-ID
semantic scopes, trusted `AuthorizationFootprint` derivation, exact finite
Human Approval, expiry/replay/revocation, minimum provenance, and
external-effect separation. ADR-0020's Issue #32 amendment accepts the logical
formula-reasoning Query, read-only scenario Query, and typed formula-update
Command shared by AI and every other first-party client. Exact identifiers,
DTOs, storage,
projection/redaction, lifecycle implementation, runtime placement, wire
formats, and promotion DTOs remain Provisional or Deferred as owned elsewhere.

Implementation state: the provider-free `tachiko-ai-api` crate implements a
v0.1 AI-facing read/explain/suggest adapter over `tachiko-workspace-engine`,
including structured read-only Semantic Analyst queries. Its provisional
`security_boundary` also accepts typed Semantic Query plus typed
proposal/execution intent, obtains
effective Principal and trusted time from host context rather than the request,
requires the lifecycle registry to prove that occurrence is active and
Delegated, delegates semantic enforcement to the #29 workspace lifecycle, and
returns stable disclosure-safe outcome codes. A Human session principal cannot
be reused as an AI credential. Raw semantic/storage mutation and host effects
are explicitly denied. The current Rust DTOs and code spellings are not the
public Semantic API/wire contract. No general schema-inference, freeform-
promotion, authentication/session/transport, or host-effect capability pipeline
is implemented. Issue #144 implements the first Provisional provider-neutral
workspace/CLI slice of the M04 formula-reasoning, scenario, and formula-update
Semantic API operations. Issue #150 implements the equivalent first
provider-neutral bounded Analysis Query workspace/CLI slice. AI-facing mappings
of those operations, public DTOs, and wire/transport behavior remain
unimplemented except for the bounded field capability discovery adapter added
under Issue #268.

## Principle

AI interacts with Tachiko Work through semantic operations and explicitly delegated capabilities.

AI has no intrinsic mutation authority. Provider, model, tool, prompt, confidence, or self-reported validation metadata is provenance/evidence, not privilege and not semantic truth.

AI must not simulate mouse/keyboard usage as the primary architecture and must not gain a second semantic mutation/validation policy merely because its provider-facing interface differs from GUI/CLI.

## Relationship to the Semantic API

[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md),
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md),
[ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md),
[`semantic-authorization.md`](semantic-authorization.md), and
[`semantic-api.md`](semantic-api.md) own the shared semantic behavior:

- Query semantics;
- typed semantic Commands;
- Propose versus Execute;
- immutable revision-pinned SemanticPatch occurrence and exact-change binding;
- authoritative validation/gating;
- formula outcome meaning;
- M04 structured formula reasoning, read-only scenario, and typed
  formula-update meaning;
- semantic atomicity;
- capability-addressability;
- trusted disclosure/write footprint, exact Human Approval, and provenance
  requirements; and
- compatibility/versioning laws.

ADR-0007 adds the AI-authority constraint: a first-party AI Execute path must
use the same shared semantic transition/gating behavior and must cross trusted
authorization/approval enforcement. ADR-0024 makes a reviewable AI proposal the
same immutable base-bound SemanticPatch available to any semantic client; it
does not make AI provenance or model output part of command meaning. The
provisional workspace-engine lifecycle/state implementation is present under
Issue `#29`. Issue #93 supplies the current internal resident session, revision-safe
publication, and composition evidence for this adapter through the same
`SemanticPublicationAuthority` seam. The #30 AI adapter enforces provider-facing
instruction/data, raw-bypass, trusted-context, and host-effect-denial boundaries
while leaving concrete authentication, public transport integrity, broader
cross-host concurrency, persistence, and actual external capability mechanisms
to their host/domain owners.
The AI adapter does not define Principal class, Grant scope, Approval, or
provenance from provider/model claims.

ADR-0021 adds a content-strengthening constraint: AI may analyze weak/freeform semantic content and propose stronger structure, but probabilistic inference is advisory evidence. Only an explicitly accepted typed semantic transition may change canonical semantic meaning.

The AI layer owns provider/agent-facing projections and review presentation. It does not become a parallel semantic API and must not be the sole enforcement point for canonical mutation authority.

Long term, AI-facing read/explain/suggest/execute experiences should map to Semantic API Query/Propose/Execute capabilities rather than depend on the exact internal `Document` field layout or current workspace-engine Rust signatures.

## Current operations

Implemented v0.1 convenience operations remain read, explain, and inert
suggest-only:

```text
describe_document(document)
explain_formula(document, field_ref)
explain_impact(before, after)
inspect_document(document, source_label)
analyze_field(document, source_label, field_ref)
analyze_changes(before, before_source_label, after, after_source_label)
analyze_validation(document, source_label)
suggest_field_change(document, field_ref, value)
```

No current AI API writes the document directly.

The provider-facing security seam adds three non-wire adapter operations:

```text
submit_semantic_proposal(trusted_host_context, typed_intent, inert_evidence)
execute_semantic_proposal(trusted_host_context, exact_proposal, optional_approval)
```

The same seam also exposes the bounded, read-only field capability Query:

```text
query_field_capabilities(trusted_host_context, exact_source_snapshot, FieldRef)
```

The request contributes only a stable target. Trusted host composition supplies
one paired document-snapshot/source-revision context, along with identity and
time; the workspace lifecycle authorizes the independent
discovery Query before returning the shared projection. The adapter does not
infer or rewrite the opaque revision encoding. The result does not grant any
listed family, does not reveal UI/presentation decisions, does not enumerate
Reference targets, and does not advertise conversions.

These operations do not add another mutation vocabulary. They reuse the #29
`SemanticPatch` Propose/Execute lifecycle, and the execution path can publish
semantic state only through its trusted `SemanticPublicationAuthority` seam.
The untrusted request contains neither effective Principal nor trusted time;
client/model validation or safety prose is retained only as inert evidence.

Suggestions are inert adapter objects. Formula analysis, semantic comparison,
typed candidate construction, validation, and calculation delegate to the
shared workspace-engine application authority.

The current `Suggestion { field, value, requires_approval }` has no proposal
occurrence identity, Semantic API compatibility binding, semantic base
reference, general Command body, or AtomicBatch body. It is therefore
implementation evidence for inert typed proposal validation, not an
implementation or wire precedent for ADR-0024 SemanticPatch.

`describe_document` currently builds an AI-facing projection from internal semantic structures. That is acceptable implementation evidence for the provider-free adapter, but it is not a precedent that future external AI clients may depend on Rust `Document` field layout. Stable long-lived AI consumers should use intentional Semantic API query projections as those operations are promoted.

The Semantic Analyst operations return deterministic workspace-engine facts:
document structure, formula source and value, transitive upstream dependencies,
transitive downstream impact, semantic changes, affected stable-ID areas, and
the current validation report. Their source labels are opaque caller-owned
evidence paired with semantic document identity; they do not define resident
revision, concurrency, authorization, provenance, or patch-lifecycle semantics.

## Proposal, authorization, and execution boundary

ADR-0020 requires semantic Propose and Execute to share the same command meaning
and authoritative gates. ADR-0024 wraps Propose in one immutable SemanticPatch
occurrence containing exactly one typed Command or ordered AtomicBatch, bound to
one Semantic API compatibility contract and semantic base. ADR-0007 additionally
requires semantic validity/gating and principal authorization/approval to
remain independent prerequisites.

For AI:

- query/read capability does not imply propose;
- propose does not imply execute;
- execute does not imply approve, one operation family does not imply another,
  and one mutation class does not imply another;
- the trusted semantic/application authority derives operation-family/
  disclosure-scope and operation-family/mutation-class/write-scope relations
  rather than accepting an agent-declared footprint;
- Propose authority does not grant arbitrary read access, so preview evidence
  outside Query scope is denied or safely reduced;
- an inert proposal does not publish semantic state;
- proposal identity is separate from semantic object identity and authority;
- the same proposal identity never names changed base, body, order, target,
  value, bound formula, generated ID, or immutable annotation content;
- a stale semantic base fails closed before candidate construction against the
  changed base and requires a newly identified proposal;
- a successful semantic gate does not grant permission to execute;
- delegated permission does not override a failed semantic gate;
- Delegated-origin or Delegated-authority publication requires one exact finite
  Human Approval covering the whole proposal/batch, originator, authorized
  executor, complete associated operation-family/mutation-class/scope write
  requirements, and the effective authorization-policy version, which must
  remain effective through publication, while separately recording the trusted
  Human approver and authorizing Approve Grant references;
- approval or a previous gate result does not authorize a changed proposal,
  write scope, mutation-class set, principal, policy, loss of required live
  Grant coverage, or semantic base;
- failure before publication does not consume Approval, while successful
  publication consumes it atomically; and
- an approved execution must use the same shared Semantic API command/gate semantics as the equivalent non-AI first-party operation.

The current `requires_approval` boolean preserves v0.1 safety behavior only. It
does not define or satisfy the ADR-0024 proposal/base contract and does not
define capability, principal, Grant, scope, footprint, Approval, provenance,
replay, revocation, or execution protocol.

For an AI-authored proposal, every semantic input that can affect candidate
construction is part of ADR-0024 `ExactChangeBinding`. Stable targets, typed
values, bound formulas, generated semantic IDs, body kind, and batch order are
fixed before proposal identity is issued. Prompt text, provider/model identity,
confidence, explanations, and rendered diff prose are not substitutes for that
binding.

## Capability-addressability

ADR-0020 accepts the principle that each semantic operation or family can be independently addressed as a capability.

AI adapters must therefore be able to expose a bounded subset such as read/query or propose without implicitly exposing arbitrary execute authority.

ADR-0026 fixes the minimum Query/Propose/Execute/Approve and
Value/Formula/Structure/Schema/Destructive dimensions, stable-ID document-local
scope atoms, trusted footprint derivation, default-deny Grants, exact Approval,
and minimum provenance. Exact identifiers, DTOs, storage, clocks, operation
catalogue, projection/redaction, and wire formats remain Provisional/Deferred.
ADR-0007's allowed current-MVP read, analysis, explanation, and Propose behavior
is preserved through explicit trusted-host Grant provisioning, not intrinsic AI
authority.

## Effect separation

Semantic publication, durable persistence, and external publication or host side effects are distinct authority domains.

A semantic edit capability must not imply filesystem, network, process, Git push, plugin, deployment, or other host authority. Provider-facing AI adapters must not create raw storage or host-effect paths that act as alternate semantic mutation authority.

Storage and host adapters may materialize or externally publish an authorized
semantic result under their own authority; they do not redefine semantic
meaning or grant semantic permission. The current `ai-api` operation classifier
admits only typed semantic Query/proposal/execution and returns stable denials for raw
semantic-state mutation, storage-representation mutation, durable persistence,
filesystem, network, process, Git, plugin, deployment, and credential requests.
It does not implement a generic external-effect capability.

## Instruction and data boundary

The current adapter distinguishes host-proven system, developer, and user
instructions from trusted semantic metadata and from untrusted document,
import, plugin, model, and client-request content. These are orchestration
treatment classes, not semantic authorization classes:

- instructions may guide orchestration but do not grant a Principal,
  capability, Approval, or host effect;
- trusted semantic metadata remains evidence/facts supplied by the trusted
  application boundary and cannot replace authorization;
- document/import/plugin/model/client content remains untrusted data even when
  it contains imperative language, structured capability claims, or assertions
  that a proposal is valid, approved, or safe; and
- an untrusted proposal request may carry only typed semantic intent plus inert
  untrusted evidence. It cannot select its trust class, effective Principal,
  Principal kind, trusted time, Grants, footprint, Approval, or gate result.

This is a deterministic platform boundary, not a claim to solve every model-
level prompt-injection problem. A concrete transport must preserve the same
separation and must not expose the trusted host-context trait as a client
credential or payload.

## Progressive semantic strengthening

ADR-0021 allows AI to assist a future weak-to-strong semantic transition, but it does not grant AI structure-authority.

AI may propose, for example:

- candidate entity/schema boundaries;
- candidate field types;
- relation targets;
- normalization/mapping plans;
- exact/lossy/unresolved conversion classifications;
- schema names or descriptions; and
- explanations of likely transition effects.

Those outputs are advisory proposal evidence. Probabilistic AI inference MUST NOT be represented as an authoritative Query fact merely because it is emitted in structured form.

AI MUST NOT:

- silently commit an inferred schema/type/relationship;
- decide ambiguous identity continuity without explicit semantic rules;
- retarget durable references by human label/name guessing;
- coerce weak values into stronger typed claims without exposing loss/ambiguity;
- weaken formula/reference semantics to accommodate untyped coordinates or labels;
- treat schema-valid model output as proof that the source-world interpretation is correct; or
- bypass the ADR-0020 Propose/Execute and ADR-0007 authorization/approval boundary.

Where a future strengthening operation is exposed, AI may use the ADR-0024
SemanticPatch envelope around `Propose(Command | AtomicBatch)` to obtain
reviewable mappings, diagnostics, semantic impact, and conversion evidence.
Changing a mapping, generated target identity, ambiguity/loss decision, body
order, or base creates a new proposal occurrence. Only an authorized `Execute`
may request semantic publication. Current MVP AI-originated strengthening
therefore remains explicitly approval-gated.

A source fragment that has not become a first-class stable-identity semantic object may be used as proposal/migration evidence but is not a durable reference or formula target. Exact source-selection and mapping mechanics remain Provisional.

## Safety

AI operations should be:

- typed where they claim typed semantic intent;
- validated;
- reviewable;
- capability-bounded;
- exact-Human-Approval-gated for a `SemanticPatch` originated by a Delegated
  principal or executed using Delegated authority;
- provider-neutral in semantic and authorization meaning;
- deterministic in authoritative semantic evaluation; and
- non-persistent unless an explicitly authorized Execute path performs the same shared semantic operation used by other first-party clients and the relevant host/persistence authority also permits the side effect.

Model-generated statements such as `validated=true`, `approved=true`, high confidence, or inferred schema conformance never substitute for deterministic Tachiko validation or trusted authorization/approval evidence.

## M04 formula operations

An AI adapter may consume the Accepted formula-reasoning and scenario Queries
only through the shared provider-neutral Semantic API. It receives structured
bound-expression, calculation, dependency/impact, validation, and scenario
provenance facts within live Query disclosure authority. Generated explanation
is an optional projection over those facts; it does not recompute or become
formula authority. A Query needs no mutation Approval merely because its caller
is Delegated.

An AI-originated formula update uses the normal typed formula-update Command,
ADR-0024 SemanticPatch, and ADR-0026 Formula-class Propose/Execute and exact
Approval rules. The adapter cannot introduce an AI-only scenario evaluator,
`FormulaPatch`, approval token, operation family, or mutation path. Successful
reasoning, scenario evaluation, or validation grants no proposal or execution
authority.

## Field capability discovery

AI and GUI clients may use the bounded `DescribeFieldCapabilities` Query from
the shared Semantic API to avoid duplicating field type matching, current
Formula edit restrictions, FormulaUpdate target checks, or Number scenario
applicability. The AI adapter delegates to the same workspace projection; it
does not maintain a second catalogue or infer capabilities from prompts,
schema text, or UI controls.

The projection is a semantic layer only: declared type/current value kind,
recognized family/input, applicability, and stable reason. Its
authorization/disclosure boundary is separate from that projection, and client
presentation is a third layer. A discovery Query grant does not imply the
listed family's Query, Propose, Execute, or Approval authority; those operations
re-evaluate live authority and semantic rules. v1 covers Number, Text, Boolean,
Reference, and Formula values already in the core model, with no conversions or
Reference valid-target enumeration (#254).

## Formula suggestions

Implemented formula suggestions:

- accept only typed `Value::Formula` proposals that target numeric fields;
- apply the shared complexity limit (`256` nodes, `64` post-desugar depth, `4096` canonical bytes);
- return an inert, validated workspace-engine candidate and mark the AI-facing proposal as requiring approval;
- require a separate approved host execution before semantic publication; and
- never replace formulas with scalars automatically (formula-to-scalar suggestions are rejected).

These behaviors are current implementation evidence constrained by the Accepted
formula/validation contracts. They do not provide ADR-0024 occurrence identity,
base or compatibility binding, generated-ID coverage, or batch semantics. The
Accepted but unimplemented formula-update Command binds the complete typed bound
formula and stable references before proposal identity; the current
`Suggestion` DTO is not stabilized as that contract.

## Project Memory

Issue #104 may later consume read/query/propose capabilities as a dogfood case. That does not promote Project Memory concepts, GitHub identifiers, or provenance workflow into the semantic core or into this AI adapter contract.

## Goal

AI is a native, capability-bounded semantic participant that uses the same meaning and operation authority as human-facing clients while exercising only explicitly delegated authority. It may help discover and propose stronger structure, but inference remains advisory until an accepted semantic transition changes meaning. Provider-specific interaction remains replaceable, and approval/security mechanisms remain enforced by trusted product boundaries rather than by model claims or adapter convention.
