# Diagnostics Contract

Decision state: Mixed. The semantic diagnostic meaning and stability rules are
Accepted under [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md).
ADR-0020 additionally makes those stable semantic observations and operation-gate
relationships part of the first-class Semantic API result meaning where
applicable. ADR-0022 fixes runtime ownership, host separation, and native/WASM
semantic parity while leaving exact diagnostic delivery/wire mechanisms
Deferred. Exact Rust data structures, complete code catalog, external wire
encoding, transport, severity vocabulary, and presentation adapters remain
Provisional or Deferred as noted below.

Implementation state: the Milestone 02 internal envelope is implemented in
`semantic-core` with symbolic codes, machine severity, stable semantic subject
sets, related subjects/facts, opaque provider identity, and explicitly
presentation-only location/message fields. `workspace-engine` owns the
authoritative first-party `ValidationReport` and stable-observation ordering.
The exact Rust representation, severity vocabulary, external namespace/wire,
and transport remain Provisional or Deferred.

Specified with: [Semantic API](semantic-api.md)

## Purpose

Define the smallest shared machine-readable diagnostic contract that can be
consumed consistently by CLI, CI, AI, and future graphical clients without
making human prose, storage layout, or source spans semantic authority.

This is not an LSP, SARIF, JSON, or IPC wire specification. Those formats may be
used by future adapters.

## Semantic diagnostic model

Conceptually, a diagnostic carries:

```text
Diagnostic
├── code
├── classification / severity
├── semantic subject(s)
├── related semantic evidence
├── code-specific machine facts
├── validator provenance
├── message          # presentation
└── help / hint       # presentation
```

The exact Rust struct or enum shape is Provisional. The semantic requirements
below are authoritative.

## Stable code identity

A published diagnostic code is a stable symbolic identifier for one semantic
rule meaning.

Requirements:

- codes are machine-readable and must not depend on a Rust enum ordinal;
- a code's semantic meaning is not changed silently after publication;
- wording changes do not require a new code;
- a meaning that changes incompatibly receives a new code or a separately
  Accepted versioning decision;
- a retired code may stop being emitted but must not later be reused for an
  unrelated meaning; and
- code spelling must not embed mutable human keys, source paths, severity, or
  localized prose.

The implemented internal code families have been audited for this milestone.
Their exact external namespace, complete versioned catalog, and wire
representation remain Provisional.

### Unknown-code compatibility

A conforming Semantic API client must be able to preserve/represent an unknown
published diagnostic code as an opaque machine finding according to the active
transport mapping.

A client must not require an exhaustive known-code switch to derive whether an
operation is allowed. Operation eligibility is carried by the authoritative
gate outcome, not reconstructed from a locally known diagnostic catalog.

Adding a new code is not automatically a harmless additive semantic change. If
the new code represents a newly blocking semantic rule that changes Accepted
gate behavior rather than implementing an already Accepted rule, the semantic
behavior change follows ADR-0020's decision/version process.

## Classification and severity

Diagnostics carry machine-readable classification/severity independent from
operation gating.

For example, an interactive editor may retain a candidate that contains a
blocking semantic finding while a strict export operation rejects the same
candidate. The diagnostic does not change identity merely because the consumer
uses a different gate.

The exact severity vocabulary is Provisional.

### Gate control-flow rule

A client must use the authoritative operation gate outcome when deciding whether
a semantic operation may publish. It must not infer universal allow/deny from:

- a severity ordinal;
- localized message wording;
- a report being empty/non-empty; or
- a client-maintained copy of validation rules.

This rule is part of ADR-0020's first-class Semantic API contract.

## Semantic subjects and locations

Stable semantic identity is the authoritative location mechanism.

A diagnostic subject may identify concepts such as:

```text
Document(DocumentId)
Schema(SchemaId)
SchemaField(SchemaId, FieldId)
Entity(EntityId)
EntityField(EntityId, FieldId)
```

This vocabulary is illustrative rather than a frozen Rust enum.

Human keys, JSON paths, `.roproj` paths, line/column ranges, spreadsheet cells,
UI widgets, and formula-source byte spans are derived projection locations.
They may be attached by an adapter but do not become the diagnostic's semantic
identity.

A rename or physical materialization change must not create a different
semantic diagnostic merely because a presentation path changed.

## Multi-subject failures and related evidence

Some failures are inherently about more than one semantic object. Examples
include duplicate human addresses, dependency cycles, and failed-dependency
sets.

The contract therefore requires preserving all semantically relevant stable
subjects or facts. An implementation may expose a primary subject plus related
subjects, a subject set, or another equivalent representation, provided it does
not discard semantic identity or invent a textual path as authority.

Examples of code-specific stable machine facts include:

- a missing stable target ID that cannot itself be represented as an existing
  semantic location;
- expected and actual semantic type/kind;
- ADR-0018 formula SCC membership;
- directly failed dependency subjects; and
- unresolved stable targets for authoring projection.

The exact `facts` encoding and related-location role vocabulary are
Provisional.

## Validator provenance

Consumers must be able to distinguish which semantic authority produced a
finding.

At minimum, provenance identifies the validator/provider namespace. Exact
provider-version representation is Provisional.

Core, formula, domain, and future extension diagnostics may share the envelope
without sharing one giant enum or making extension rules part of semantic-core.

This validator provenance is not the broader user/agent decision provenance
protocol owned by #27/#28/#104.

## Presentation-only fields

The following are not stable machine meaning:

- human message wording;
- localization;
- help/hint prose;
- rendered human-key paths;
- source line/column/span;
- terminal formatting;
- selected cycle witness/path when ADR-0018 defines SCC membership as the
  semantic fact; and
- UI grouping or ordering.

Presentation may evolve without changing the underlying diagnostic code and
semantic subject.

## Determinism

For the same semantic snapshot and deterministic validator configuration,
authoritative validation must produce the same stable diagnostic observations
across supported native/WASM targets.

The report must have deterministic externally observable ordering when an
ordered representation is required for testing, canonical machine output, or
transport. The exact internal comparator is Provisional and must not redefine
semantic meaning.

## Adapter guidance

### CLI

Human output may render current keys and concise messages. Machine output should
consume structured diagnostics rather than parse `Display` strings.

Exact CLI JSON/output and exit-code policy are not fixed here.

### CI / SARIF

A future SARIF adapter may map stable diagnostic codes to `ruleId`, semantic
subjects to logical locations, and representation-specific spans to physical
locations when available. SARIF is an interchange adapter, not the semantic
model.

### LSP / editors

A future editor adapter may project a semantic subject into the currently
visible text/document range and preserve the stable code/semantic identity in
adapter data. LSP `Range` is not Tachiko's canonical diagnostic location.

### AI

AI consumers should receive stable codes, semantic subjects, related evidence,
and machine facts. They must not need to parse localized human messages to
understand a failure and must not infer execute authority from a report alone.

### Native/WASM/IPC

The semantic diagnostic observations and gate relationship follow ADR-0019 and
ADR-0020. ADR-0022 requires equivalent Stable semantic diagnostic observations
where native and WASM expose the same semantic capability, and places
interactive authoritative state in the shared Rust runtime. Exact transport
serialization/version negotiation, resident session delivery, push/pull
updates, and projection patches remain Deferred to #93–#95 and future
host/transport implementation as applicable.

## Storage failures

ADR-0017 storage/version/migration failures remain a representation-local
failure family. Higher layers may wrap or project them into client-facing
results, but this specification does not reclassify storage errors as universal
semantic diagnostics or erase their version-specific failure meaning.

## Stability classification

| Element | State |
| --- | --- |
| Stable symbolic code identity and meaning | Accepted |
| Unknown code remains representable; code meaning not silently reused | Accepted under ADR-0019/ADR-0020 |
| Semantic stable-ID subject authority | Accepted |
| Preservation of semantically relevant related subjects/facts | Accepted |
| Validator/provider provenance concept | Accepted |
| Separation of machine severity from operation gate | Accepted |
| Authoritative GateOutcome, not severity/report emptiness, controls semantic publication | Accepted under ADR-0020 |
| Native/WASM equivalent Stable diagnostic observations where capabilities overlap | Accepted under ADR-0022 |
| Human message/help wording | Presentation / unstable |
| Human-key path and source span | Projection / unstable |
| Exact code namespace/catalog spelling | Provisional; published meanings stable |
| Exact severity enum | Provisional |
| Exact semantic-location Rust type | Provisional |
| Exact facts/related encoding | Provisional |
| Exact sort tuple | Provisional |
| External JSON/API wire format | Deferred; must preserve ADR-0019/ADR-0020/ADR-0022 laws |
| IPC/WASM transport and resident diagnostic delivery mechanics | Deferred to future runtime/transport implementation |
| Diagnostic fingerprint/GUID/baseline | Deferred |
| Suppression/fix-it protocol | Deferred |

## Related

- ADR-0015
- ADR-0018
- ADR-0019
- ADR-0020
- ADR-0022
- `semantic-api.md`
- `validation-engine.md`
- Issues #10, #17, #23, #27, #28, #93, #94, #95
