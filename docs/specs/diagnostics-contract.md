# Diagnostics Contract

Decision state: Mixed. The semantic diagnostic meaning and stability rules are
Accepted under [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md).
Exact Rust data structures, code catalog, external wire format, transport, and
presentation adapters remain Provisional or Deferred as noted below.

## Purpose

Define the smallest shared machine-readable diagnostic contract that can be
consumed consistently by CLI, CI, AI, and future graphical clients without
making human prose, storage layout, or source spans semantic authority.

This is not an LSP or SARIF wire specification. Those formats may be used by
future adapters.

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

The exact namespace and initial code catalog remain Provisional until the
existing diagnostic families are audited during implementation.

## Classification and severity

Diagnostics carry machine-readable classification/severity independent from
operation gating.

For example, an interactive editor may retain a candidate that contains a
blocking semantic finding while a strict export operation rejects the same
candidate. The diagnostic does not change identity merely because the consumer
uses a different gate.

The exact severity vocabulary is Provisional. External versioning belongs to
#10.

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
understand a failure.

### Native/WASM/IPC

Transport serialization, version negotiation, resident delivery, push/pull
updates, and projection patches remain #26 and #10.

## Storage failures

ADR-0017 storage/version/migration failures remain a representation-local
failure family. Higher layers may wrap or project them into client-facing
results, but this specification does not reclassify storage errors as universal
semantic diagnostics or erase their version-specific failure meaning.

## Stability classification

| Element | State |
| --- | --- |
| Stable symbolic code identity and meaning | Accepted |
| Semantic stable-ID subject authority | Accepted |
| Preservation of semantically relevant related subjects/facts | Accepted |
| Validator/provider provenance concept | Accepted |
| Separation of machine severity from operation gate | Accepted |
| Human message/help wording | Presentation / unstable |
| Human-key path and source span | Projection / unstable |
| Exact code namespace/catalog | Provisional |
| Exact severity enum | Provisional |
| Exact semantic-location Rust type | Provisional |
| Exact facts/related encoding | Provisional |
| Exact sort tuple | Provisional |
| External JSON/API wire format | Deferred to #10 |
| IPC/WASM transport | Deferred to #26 |
| Diagnostic fingerprint/GUID/baseline | Deferred |
| Suppression/fix-it protocol | Deferred |

## Related

- ADR-0015
- ADR-0018
- ADR-0019
- `validation-engine.md`
- Issues #10, #17, #23, #26
