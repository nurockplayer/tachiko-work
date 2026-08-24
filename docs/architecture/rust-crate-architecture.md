# Rust Crate Architecture

Decision state: Milestone 02 layering is Accepted in ADR-0016. ADR-0020 now
accepts the transport-neutral Headless Semantic API as the first-class product
boundary implemented by the shared application layer; it does not stabilize the
current Rust source surface.

Implementation state: ADR-0016 boundary implemented by Issue #72; authoritative
validation/report composition implemented by Issue #89.

Architecture authority: ADR-0016 for crate ownership; ADR-0020 for the
first-class Semantic API product boundary.

## Purpose

This document records the live Rust workspace and implementation evidence for
the crate ownership accepted by
[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md).
The first-class client contract is specified separately in
[`semantic-api.md`](../specs/semantic-api.md).

The ADRs remain authority for dependency direction, forbidden edges,
portability, semantic-client rules, public-vs-internal stability, and future
amendments.

## Live workspace

```text
tachiko-work/
├── crates/
│   ├── semantic-core/
│   ├── formula-engine/
│   ├── diff-engine/
│   ├── merge-engine/
│   ├── storage/
│   ├── workspace-engine/
│   ├── ai-api/
│   └── cli/
```

Arrows point from a dependent crate toward the crate it uses:

```text
formula-engine ────────────────→ semantic-core
diff-engine ───────────────────→ formula-engine, semantic-core
merge-engine ──────────────────→ formula-engine, semantic-core
storage ───────────────────────→ semantic-core
workspace-engine ──────────────→ diff-engine, merge-engine,
                                  formula-engine, semantic-core
ai-api ────────────────────────→ workspace-engine
cli ───────────────────────────→ workspace-engine, storage
```

`scripts/workspace-dependency-check.mjs` reads locked Cargo metadata and fails
if a workspace package or local edge differs from this graph. CI and the local
release-equivalent gate execute that check. It includes development dependency
kinds, so a test-only workspace-engine-to-storage edge cannot silently weaken
the sibling boundary.

## Product contract versus Rust source boundary

ADR-0020 adds an explicit firewall:

> `workspace-engine` implements the first-class Semantic API contract, but its
> current Rust `pub` items, re-exports, modules, errors, result structs, and
> serde shapes are not automatically the public Semantic API.

A Rust surface becomes a stable downstream SDK only if a future explicit API
specification/version classifies it as such. A serialized Rust type becomes a
wire contract only when a transport specification says so.

This distinction allows first-party adapters to share one Rust implementation
without turning current source-level convenience into permanent ecosystem
compatibility debt.

Future native/WASM/IPC/network adapters are expected to conform to the semantic
contract, not to source-level Rust type equality.

## Responsibility evidence

### semantic-core

Semantic core owns opaque stable-ID types, mutable human-key types,
document/schema/entity/field models, typed values and bound relationships,
formula expression representation, derived address indexes, intrinsic semantic
diagnostics, and whole-document validation. It has no dependency on another
workspace crate and no UI, filesystem, network, UUID-generation, or host
capability.

ADR-0019 fixes the semantic validation/diagnostic meaning that this layer
participates in while keeping exact Rust diagnostic/report surfaces
Provisional.

Semantic types remain owned here. Workspace-engine re-exports the semantic
types required by current first-party adapters so those adapters need one
application dependency; this does not transfer semantic ownership or make the
exact Rust surface a stable external SDK.

### formula-engine

Formula engine owns bounded source parsing, human-address binding, stable-ID
bound projection, structural limits, deterministic finite-binary64 calculation,
dependency indexes, and formula failures. It depends only on semantic-core among
workspace crates. ADR-0018 remains authoritative for semantic and native/WASM
numeric behavior; ADR-0019 wraps those semantic facts into shared
validation/diagnostics without changing formula precedence or SCC meaning.

ADR-0020 makes those already-Accepted formula outcome facts observable through
the Semantic API where relevant; it does not promote current formula-engine Rust
errors or functions into public API.

### diff-engine and merge-engine

Diff owns typed semantic comparison and derived formula impact. Merge owns the
pure model-level three-way reconciliation algorithm and its typed conflicts.
Both remain below the application boundary, contain no host capability, and
retain their focused algorithm tests.

Workspace-engine is the first-party client entry point for semantic comparison
and merge orchestration. A successful merge result includes the base-to-merged
semantic impact; a conflicted result preserves the existing typed conflict data
without persistence or presentation policy.

The exact public effect/diff/merge projection shape remains Provisional under
ADR-0020 even where the underlying algorithmic meaning is Accepted elsewhere.

### storage

Storage owns strict version-specific DTOs/codecs, explicit migration, canonical
direct-ro materialization, and native filesystem load/save APIs. It depends on
semantic-core and remains a sibling of workspace-engine.

The CLI composition root performs `load → semantic operation → canonical
encode/write`. Workspace-engine does not depend on paths, files, storage DTOs,
or persistence. ADR-0003, ADR-0017, and the direct-ro specifications remain
unchanged by the first-class Semantic API decision.

Storage format/migration failures remain a representation-local family rather
than becoming universal semantic diagnostics.

### workspace-engine

`tachiko-workspace-engine` evolved in place from the former workflow crate; no
parallel orchestration crate or semantic workspace aggregate exists. Current
operations remain document-local and snapshot-style, preserving #26's
resident-runtime decision.

The engine owns real application behavior:

- host-supplied stable-ID creation and built-in starters;
- authoritative `ValidationReport` plus complete calculation orchestration;
- calculated values projected through current human addresses;
- overview, human-addressed field explanation, and stable formula analysis;
- scalar/formula edits and entity lifecycle candidate transitions;
- inert typed field-proposal validation shared with the AI adapter;
- semantic comparison and merge-plus-impact orchestration; and
- deterministic runtime-export projection independent of filesystem and
  terminal rendering.

All candidate operations are immutable. They validate/calculate before success,
and mutation previews include semantic impact where the current operation
contract requires it. The `IdGenerator` trait and `SemanticIdKind` preserve
ADR-0015's replaceable creation seam; UUIDv7 remains supplied by the native CLI
host rather than the portable engine.

ADR-0019 makes workspace-engine the first-party validation orchestration and
normalization boundary. Shared semantic validation is reused by queries,
mutations, and merge finalization; canonical authoring projection and
output-specific preflights remain explicit operation gates rather than alternate
definitions of semantic validity.

ADR-0020 promotes the **semantic operation boundary** to a first-class product
contract: Query/Command, Propose/Execute, operation gates, semantic atomicity,
capability-addressability, and compatibility laws. The complete operation
catalogue and exact current Rust functions/results remain Provisional.

### ai-api

Provider-free AI code depends only on workspace-engine among workspace crates.
It retains AI-facing descriptions, explanations, inert suggestions, and the
`requires_approval` adapter DTO. Formula analysis, semantic impact, typed
candidate cloning, schema checks, formula complexity/projection checks,
validation, and calculation delegate to workspace-engine.

Under ADR-0020 the AI crate is an adapter/projection over the same first-class
Semantic API behavior as other clients. `requires_approval` remains current v0.1
safety behavior, not the #27/#28 capability/approval/provenance protocol.

No current AI operation persists or mutates the supplied document.

### cli

CLI depends only on workspace-engine and storage among workspace crates. It
owns:

- Clap arguments and command dispatch;
- OS paths and default titles derived from host paths;
- storage load/canonical-encode composition;
- UUIDv7 generation supplied through the engine's creation seam;
- exclusive-create writes and no-overwrite timing; and
- terminal and JSON rendering.

Validation, calculation materialization, semantic diff, merge-plus-impact,
mutation rules, and runtime-export semantic projection are not implemented in
CLI command handlers.

ADR-0020 means future CLI semantic operations must continue to map the shared
Semantic API rather than grow independent semantic rules.

## Before/after ownership

| Concern | Before #72 | Current authority |
| --- | --- | --- |
| Candidate mutation policy | Workflow plus duplicated AI path | Workspace-engine / Semantic API implementation |
| Validation/calculation orchestration | Workflow, AI, CLI, command-specific paths | Workspace-engine for first-party clients |
| Semantic comparison | Direct CLI and AI calls | Workspace-engine |
| Merge plus base-to-result impact | CLI over merge and diff engines | Workspace-engine |
| Runtime export semantic projection | CLI | Workspace-engine |
| Host persistence and safe writes | CLI/storage | CLI/storage, unchanged |
| AI approval DTO | AI API | AI adapter, #27/#28 future authority |
| ID generation mechanism | CLI through workflow seam | CLI through workspace-engine seam |
| Product-semantic client contract | Provisional/internal | First-class transport-neutral Semantic API under ADR-0020 |

Low-level diff and merge algorithms still validate or calculate where their own
pure correctness contracts require it. That is algorithm ownership below the
application boundary, not a second client policy path.

## Internal bypass versus client bypass

ADR-0020's mandatory client rule does not force lower-level implementation code
to call a public facade recursively.

Allowed internal paths include:

- workspace-engine calling semantic-core/formula/diff/merge under this DAG;
- storage codecs/migrations operating at the representation boundary;
- host composition depending on workspace-engine plus storage;
- focused tests directly invoking their owner contract; and
- deterministic validators participating through ADR-0019.

Forbidden product paths include GUI/CLI/AI/native/WASM adapters implementing a
second semantic mutation, validation, formula, or gate policy simply because
they are in the same process or repository.

## Portability evidence

The capability-free portable set is:

- semantic-core;
- formula-engine;
- diff-engine;
- merge-engine;
- workspace-engine; and
- provider-free ai-api semantic adapter code.

`scripts/portable-conformance-check.sh` builds this set for native and
`wasm32-unknown-unknown`. Its shared production-API corpus executes on both
targets and compares stable observations for normalized Number bits, typed
failures, dependency/cycle behavior, binding/projection continuity, storage
numeric bytes, workspace-engine calculated queries, AI formula explanation, and
inert approval-required AI proposal validation.

Storage is also present in existing conformance coverage for portable codec
behavior, but the crate remains host-facing because it exposes native path/file
APIs. CLI is native-only.

This evidence supports semantic portability; it does not define a public WASM
ABI, Web Worker, resident runtime, browser persistence mechanism, or wire DTO.

## #26 mapping rule

#26 owns runtime/session state placement, revision/concurrency, Web Worker,
IPC/FFI, projection delivery, host capabilities, native/browser persistence, and
concrete serialization/ABI.

It must map the Accepted Semantic API rather than create a client-specific
semantic implementation. Runtime topology may host/cache/serialize/deliver the
contract but cannot redefine Query/Command/Propose/Execute meaning, validation
gates, stable diagnostics/formula facts, or semantic atomicity.

## Explicitly deferred seams

- ADR-0019 owns validation/diagnostic meaning and temporary-invalid candidate
  boundaries; exact Rust APIs remain Provisional.
- ADR-0020 owns external Semantic API semantic laws and compatibility; complete
  operation catalogue and exact Rust/wire shapes remain Provisional.
- #26 owns resident state, Web Worker placement, IPC/FFI, projection patches,
  diagnostic delivery, host capabilities, persistence composition, and
  concrete transport mappings.
- #27/#28 own AI capability IDs, principals, grants, approval, provenance, and
  execution authorization.
- #41 owns `.roproj` layout and materialization.
- A dedicated stable public Rust SDK/facade crate is Deferred until downstream
  pressure justifies it.

No new crate, semantic `Workspace`/`Project` aggregate, storage/formula
contract, or native/WASM feature-selected semantic behavior is introduced by
ADR-0020. Any future direct edge or crate split that changes the Accepted
ADR-0016 baseline must amend that ADR explicitly.

## Related authority

- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md)
- [Semantic API specification](../specs/semantic-api.md)
- [Semantic core rationale](semantic-core-rationale.md)
- [Knowledge authority](../governance/knowledge-authority.md)
- GitHub issues #10, #13, #17, #23, #26, #27, #28, #41, #72, #104
