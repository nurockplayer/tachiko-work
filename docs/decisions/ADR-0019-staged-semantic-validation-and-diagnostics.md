# ADR-0019: Staged semantic validation and diagnostics contract

## Status

Accepted

Decision issue: #23

## Context

Tachiko Work already has Accepted contracts for stable semantic identity
(ADR-0015), Rust crate layering (ADR-0016), versioned storage and canonical
representation (ADR-0017), and bound deterministic formulas (ADR-0018).
Implementation through #40, #70, #72, and PR #85 establishes a shared
`workspace-engine` application boundary and native/WASM conformance without yet
settling the general validation and diagnostics contract.

The current implementation is useful evidence but is not itself the durable
contract:

- `semantic-core::validate_document` accumulates and deterministically sorts a
  small set of semantic diagnostics;
- address/index construction and many formula/application operations still fail
  fast;
- `workspace-engine` finalization performs stricter checks than some current
  `validate` entry points;
- formula evaluation still exposes fail-first `CalculationError` behavior even
  though ADR-0018 accepts a stronger full-recompute failure oracle;
- current diagnostics center on a string path and human message, while future
  CLI, CI, AI, Web, and Tauri clients need shared machine-readable meaning; and
- storage owns representation-local failures that must not become the universal
  semantic diagnostics model merely because they already exist.

Research compared compiler-style diagnostics, rustc/rust-analyzer, LSP,
SARIF, and JSON Schema validation/output models. The common useful pattern is
to separate rule identity, semantic subject, related evidence, presentation,
and execution policy. Tachiko Work additionally has stable semantic identity
that must outrank source paths, JSON pointers, or UI coordinates.

The goal is therefore to accept a small semantic validation contract, not a
large validation framework.

## Decision

### 1. Schema declaration owns durable semantic constraints

A constraint belongs to durable schema/semantic meaning only when conforming
clients must interpret the same semantic snapshot the same way regardless of
UI, host, validation schedule, or presentation.

The current Milestone 02 durable declaration surface includes the semantic
facts already represented and constrained by Accepted ADRs, including stable
schema/field identities, field types, requiredness, entity-to-schema
membership, reference target schema identity, stable typed relationships, and
ADR-0018 formula meaning.

Validation execution does not invent new schema meaning. Runtime scheduling,
debounce, cache/index design, full versus incremental scheduling, diagnostic
rendering, CI policy, source-span projection, and transport are not persisted
schema semantics.

Generic constraint DSLs, enum/range/pattern/default semantics, nominal type
systems, and general validation-rule languages remain future work until
concrete schema pressure justifies them.

### 2. Raw authoring, semantic candidates, and finalized snapshots are distinct

Tachiko Work distinguishes three conceptual states:

```text
raw authoring / representation state
        |
        | admission / parse / bind / construction
        v
structurally admissible semantic candidate
        |
        | authoritative validation / operation gate
        v
validated / finalized semantic snapshot
```

This is a semantic distinction, not a requirement to introduce a Rust type-state
framework now.

Raw authoring state may be incomplete or malformed and is not semantic truth.
Examples include malformed JSON, an unfinished formula source buffer, or source
text that fails parsing/binding.

A semantic candidate must satisfy the intrinsic representability invariants
already required by Accepted contracts. States that cannot form coherent stable
semantic identity, non-finite Numbers, bound expressions outside Accepted
structural limits, or a newly authored formula that failed parse/bind/type
construction do not become a new semantic candidate.

Once a candidate is structurally admissible, higher-level semantic failures may
be diagnosable without destroying its stable meaning. Examples include missing
required fields, schema-instance mismatch, dangling typed relationships, stale
existing bound formula targets, formula cycles, failed dependencies, and
formula evaluation failures.

Permitting such a candidate for an interactive/runtime editing workflow does
not require existing strict workspace commands to start accepting invalid
inputs. A typed operation may continue to reject a candidate atomically.
Resident interactive state and recovery persistence remain owned by #26, #13,
and representation-specific work.

### 3. Validation has one staged authority model

The authoritative prerequisite order is:

```text
0. admission / representation prerequisites
1. intrinsic semantic declaration invariants
2. schema-instance conformance
3. semantic relationship validation
4. formula static / graph validation
5. formula evaluation
6. deterministic domain validation
7. deterministic extension validation
```

Stage 0 is representation/parser/type-construction admission and may fail before
there is a semantic candidate. Storage keeps its ADR-0017 failure precedence and
representation-local error family.

Stages 1 through 3 validate semantic facts without redefining ADR-0015 identity
or ADR-0017 representation semantics.

Stages 4 and 5 follow ADR-0018. In particular, formula semantic failure
precedence remains:

```text
structural
  -> binding / type / stale target
  -> cycle
  -> failed dependency
  -> local evaluation
```

#23 does not freeze the current fail-first formula implementation as semantic
authority. ADR-0018's full-recompute outcome and SCC membership remain the
contract when current implementation behavior is weaker.

Stages 6 and 7 may add findings over valid-enough semantic facts. They may not
reinterpret an earlier authoritative stage.

This order defines authority and prerequisites. It does not require a naive
full scan at every runtime call.

### 4. Accumulate independent failures and suppress cascades

Validation is neither globally fail-fast nor blindly exhaustive.

When a prerequisite for one semantic subject is unavailable, dependent checks
for that subject stop rather than generating secondary noise. Independent
subjects continue to be validated and their findings accumulate.

For example, an entity whose schema cannot be resolved may receive a schema
failure without also receiving speculative field-type failures. Another
independent entity is still validated.

Diagnostics produced by a conforming full validation pass are deterministic for
the same semantic snapshot and deterministic validator configuration.

### 5. Full validation is the correctness oracle

A complete deterministic validation over a semantic snapshot is the correctness
oracle.

Incremental validation is permitted only as an optimization. Its stable
observations must be equivalent to the full oracle for the same semantic
snapshot and validator configuration. If an implementation cannot safely prove
the affected set, it falls back to the full oracle.

Stable observations include the semantic meaning of diagnostic codes,
semantic subjects, semantically relevant related evidence/facts, diagnostic
classification/severity, validator provenance, and any formula outcomes already
made semantic by ADR-0018.

Localized prose, rendered human paths, source spans, UI ordering, terminal
formatting, or a particular cycle witness are not part of incremental semantic
equivalence.

The specific invalidation indexes, caching model, parallel scheduler, and sort
implementation remain replaceable mechanisms.

### 6. Diagnostics are semantic-first machine-readable facts

A shared diagnostic has a small presentation-neutral semantic contract.
Conceptually it carries:

- a published symbolic code whose meaning is machine-readable;
- machine-readable classification/severity distinct from operation gating;
- one or more semantic subjects identified by stable semantic identity;
- semantically meaningful related subjects or stable machine facts when the
  failure involves more than one object or a missing target;
- validator/provider provenance sufficient to identify the authority that
  produced the finding; and
- optional human message/help presentation.

Exact Rust structs, enum shapes, JSON encoding, transport versioning, and the
complete initial code catalog are not Accepted by this ADR.

A published diagnostic code is not a Rust enum ordinal. Once a code is declared
stable, its semantic meaning must not be silently reused for a different rule.
Wording changes do not require a code change; an incompatible semantic meaning
requires a new code or a separately Accepted versioning decision.

Stable semantic subjects outrank mutable human keys, storage paths, JSON
pointers, source ranges, and UI coordinates. Those are derived projections that
may be attached by an adapter when available.

A diagnostic is not required to invent one arbitrary authoritative textual
"primary" location for a genuinely multi-subject failure. The exact distinction
between primary subject, subject set, related subjects, and code-specific facts
remains Provisional as long as all semantically relevant stable identities are
preserved.

Human message/help text, localized wording, human-key paths, text spans,
line/column coordinates, rendered locations, and selected cycle witness paths
are presentation-level and may evolve without changing diagnostic meaning.

### 7. Severity and operation gates are separate

A diagnostic's machine-readable classification/severity is not itself the
complete policy for every operation.

A strict finalization, runtime export, CI profile, or approved mutation may
block on a defined class of findings. An interactive editor may retain the same
semantic candidate and display those findings while the user repairs it.
Changing the operation gate does not rewrite the diagnostic's semantic meaning.

The exact severity vocabulary and external wire representation remain
Provisional until #10. A future CI profile may choose to treat warnings as
blocking without mutating the underlying rule identity.

Raw Git repositories are user-owned external systems. This ADR does not make
physical `git commit` rejection a semantic invariant; hooks and protected-branch
CI are workflow policy.

Likewise, this ADR does not impose a blanket rule that every semantic Error must
be rejected by every storage save API. ADR-0017 requires semantic validation as
required by the operation, and storage remains a sibling boundary to
workspace-engine. Representation-specific save semantics and future invalid
editor-draft persistence remain separately owned.

### 8. Deterministic domain and extension validators are read-only

A domain or extension validator that participates in authoritative semantic
validation must be deterministic and read-only for the same semantic snapshot
and deterministic configuration.

It must not depend implicitly on filesystem state, network responses, wall
clock, locale, process environment, ambient randomness, or thread completion
order. It may add findings but may not mutate semantic state or redefine
Accepted core identity, type, reference, Number, formula, or storage semantics.

External or nondeterministic checks may exist as separate host/workflow
services, but their results do not silently become canonical semantic validity.

Plugin runtime, ABI, sandboxing, signing, capability grants, distribution, and
compatibility remain Deferred. ADR-0028 later resolves only the M04
game-engine host-adapter classification; its #134/#135 follow-ups own narrower
private-enterprise and public-ecosystem policy, not these mechanics.

### 9. Keep the ADR-0016 crate boundary

Current evidence does not justify a new `validation`, `diagnostics`, or generic
foundation crate.

The intended ownership remains:

- `semantic-core`: intrinsic semantic model/invariants and minimal shared
  semantic diagnostic/location primitives;
- `formula-engine`: formula-specific parsing, binding, graph, and evaluation
  failure authority under ADR-0018;
- `workspace-engine`: first-party validation orchestration, normalization,
  finalization/gating, and shared application behavior;
- domain/extension code: its own deterministic rule definitions and facts; and
- CLI, CI, AI, Web, Tauri, LSP, SARIF, and transport adapters: rendering,
  projection, serialization, and host-specific policy as separately Accepted.

Storage failures remain representation-local and may be wrapped or projected by
higher layers without losing their ADR-0017 meaning.

## Provisional details

The following remain intentionally replaceable:

- exact Rust diagnostic/report types;
- exact symbolic code namespace spelling and initial catalog;
- exact severity vocabulary;
- exact semantic-location/facet enum shape;
- exact primary/related/facts representation;
- exact deterministic sort comparator;
- source-span and human-path projection APIs;
- hint/remediation representation;
- validator registration traits or registries;
- domain-validator incremental invalidation mechanisms;
- validation profile / warnings-as-errors configuration; and
- provider version encoding.

Current `Diagnostic.path: String`, exact-vector tests, fail-first
`CalculationError`, and concrete DFS cycle witness behavior are implementation
evidence and must not be promoted accidentally through this ADR.

## Deferred decisions

This ADR does not decide:

- progressive/freeform or mixed typed/untyped content semantics (#13);
- public Semantic API stability, wire compatibility, batch/transaction shape,
  or bypass policy (#10);
- native/WASM/IPC/Tauri serialization, resident runtime, Web Worker placement,
  or diagnostic delivery protocol (#26);
- plugin runtime/ABI/sandbox/registry/distribution (Deferred; ADR-0028 resolves
  only the game-engine host-adapter classification);
- `.roproj` physical layout, source spans, or invalid-draft persistence (#41);
- generic schema constraint DSLs or complete enum/range/pattern/default type
  semantics;
- structured fix-it/autofix protocol;
- diagnostic suppression, baselines, occurrence fingerprints, or GUIDs;
- nondeterministic remote-validator authority;
- cross-document/project validation; or
- a particular incremental-validation performance algorithm.

## Consequences

Positive:

- interactive clients can eventually represent useful temporary invalidity
  without weakening stable semantic primitives;
- CLI, CI, AI, Web, and Tauri can consume one semantic validation meaning rather
  than parsing prose or reimplementing rules;
- stable semantic identity remains the diagnostic authority across renames and
  projections;
- formula diagnostics can converge on ADR-0018's stronger oracle without
  freezing current fail-first behavior;
- later LSP and SARIF adapters can reuse mature interchange formats without
  making those formats the semantic model; and
- future domain validators have an extension boundary without forcing a plugin
  runtime or new crate now.

Costs:

- current validation/finalization entry points are not yet fully symmetric;
- some tests currently freeze presentation paths or witness details that must be
  relaxed during implementation;
- formula-engine still needs implementation work to reach ADR-0018's complete
  failure-oracle contract; and
- structured CLI/AI/external diagnostic transport remains later work rather than
  being solved implicitly here.

## Required follow-up

- Reconcile `validation-engine.md`, `schema-system.md`, diagnostics
  documentation, and the canonical authority register with this ADR.
- Follow-up implementation should create one authoritative validation report
  path through workspace-engine, remove duplicated validation/preflight rules
  where possible, close current `validate` versus finalization asymmetries, and
  converge formula failure reporting on ADR-0018 without changing its semantics.
- #10 decides the public Semantic API and externally versioned diagnostic
  transport.
- #26 decides native/WASM/IPC runtime transport and resident interactive state.
- #13 decides progressive/freeform authoring semantics.
- Plugin validator runtime mechanics remain Deferred; ADR-0028 and its
  #134/#135 follow-ups do not stabilize them.
- #41 decides `.roproj` physical materialization and any durable draft profile.

## Related

- ADR-0015
- ADR-0016
- ADR-0017
- ADR-0018
- Issues #10, #13, #17, #23, #26, #41
