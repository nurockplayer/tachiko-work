# Rust Crate Architecture

Decision state: Milestone 02 layering is Accepted in ADR-0016

Implementation state: Implemented by the Issue #72 workspace-engine migration

Architecture authority: ADR-0016

## Purpose

This document records the live Rust workspace and implementation evidence for
the crate ownership accepted by
[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md).
The ADR remains the authority for dependency direction, forbidden edges,
portability, Provisional seams, and future amendments.

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

## Responsibility evidence

### semantic-core

Semantic core owns opaque stable-ID types, mutable human-key types,
document/schema/entity/field models, typed values and bound relationships,
formula expression representation, derived address indexes, intrinsic semantic
diagnostics, and whole-document validation. It has no dependency on another
workspace crate and no UI, filesystem, network, UUID-generation, or host
capability.

Semantic types remain owned here. Workspace-engine re-exports the semantic
types required by first-party adapters so those adapters need one application
dependency; this does not transfer semantic ownership or make the exact Rust
surface a stable external SDK.

### formula-engine

Formula engine owns bounded source parsing, human-address binding, stable-ID
bound projection, structural limits, deterministic finite-binary64
calculation, dependency indexes, and formula failures. It depends only on
semantic-core among workspace crates. ADR-0018 remains authoritative for
semantic and native/WASM numeric behavior.

### diff-engine and merge-engine

Diff owns typed semantic comparison and derived formula impact. Merge owns the
pure model-level three-way reconciliation algorithm and its typed conflicts.
Both remain below the application boundary, contain no host capability, and
retain their focused algorithm tests.

Workspace-engine is the only first-party client entry point for semantic
comparison and merge orchestration. A successful merge result includes the
base-to-merged semantic impact; a conflicted result preserves the existing
typed conflict data without persistence or presentation policy.

### storage

Storage owns strict version-specific DTOs/codecs, explicit migration,
canonical direct-ro materialization, and native filesystem load/save APIs. It
depends on semantic-core and remains a sibling of workspace-engine.

The CLI composition root performs `load → workspace operation → canonical
encode/write`. Workspace-engine does not depend on paths, files, storage DTOs,
or persistence. ADR-0003, ADR-0017, and the current direct-ro specifications
remain unchanged by the application-layer migration.

### workspace-engine

`tachiko-workspace-engine` evolved in place from the former workflow crate; no
parallel orchestration crate or semantic workspace aggregate exists. Current
operations remain document-local and snapshot-style, preserving #26's
resident-runtime decision.

The engine owns real application behavior:

- host-supplied stable-ID creation and built-in starters;
- intrinsic validation plus complete calculation orchestration;
- calculated values projected through current human addresses;
- overview, human-addressed field explanation, and stable formula analysis;
- scalar/formula edits and entity lifecycle candidate transitions;
- inert typed field-proposal validation shared with the AI adapter;
- semantic comparison and merge-plus-impact orchestration;
- deterministic runtime-export projection independent of filesystem and
  terminal rendering.

All candidate operations are immutable. They validate/calculate before success,
and mutation previews include semantic impact where the existing operation
contract requires it. The `IdGenerator` trait and `SemanticIdKind` preserve
ADR-0015's replaceable creation seam; UUIDv7 remains supplied by the native CLI
host rather than the portable engine.

These Rust functions and result structures are the first-party internal
boundary. Their external stability and versioning remain Provisional under
#10.

### ai-api

Provider-free AI code depends only on workspace-engine among workspace crates.
It retains AI-facing descriptions, explanations, inert suggestions, and the
`requires_approval` adapter DTO. Formula analysis, semantic impact, typed
candidate cloning, schema checks, formula complexity/projection checks,
validation, and calculation delegate to workspace-engine.

This migration does not define the #27/#28 capability, approval, provenance, or
execution protocol. No AI operation persists or mutates the supplied document.

### cli

CLI depends only on workspace-engine and storage among workspace crates. It
owns:

- Clap arguments and command dispatch;
- OS paths and default titles derived from host paths;
- storage load/canonical-encode composition;
- UUIDv7 generation supplied through the engine's creation seam;
- exclusive-create writes and no-overwrite timing;
- terminal and JSON rendering.

Validation, calculation materialization, semantic diff, merge-plus-impact,
mutation rules, and runtime-export semantic projection are no longer implemented
in CLI command handlers.

## Before/after ownership

| Concern | Before #72 | After #72 |
| --- | --- | --- |
| Candidate mutation policy | Workflow plus duplicated AI path | Workspace-engine |
| Validation/calculation orchestration | Workflow, AI, CLI, and command-specific paths | Workspace-engine for first-party clients |
| Semantic comparison | Direct CLI and AI calls | Workspace-engine |
| Merge plus base-to-result impact | CLI over merge and diff engines | Workspace-engine |
| Runtime export semantic projection | CLI | Workspace-engine |
| Host persistence and safe writes | CLI/storage | CLI/storage, unchanged |
| AI approval DTO | AI API | AI API, unchanged |
| ID generation mechanism | CLI through workflow seam | CLI through workspace-engine seam |

Low-level diff and merge algorithms still validate or calculate where their own
pure correctness contracts require it. That is algorithm ownership below the
application boundary, not a second client policy path.

## Portability evidence

The capability-free portable set is:

- semantic-core;
- formula-engine;
- diff-engine;
- merge-engine;
- workspace-engine;
- provider-free ai-api.

`scripts/portable-conformance-check.sh` builds this set for native and
`wasm32-unknown-unknown`. Its shared production-API corpus executes on both
targets and compares exact records for normalized Number bits, typed failures,
dependency/cycle behavior, binding/projection continuity, storage numeric
bytes, workspace-engine calculated queries, AI formula explanation, and inert
approval-required AI proposal validation.

Storage is also present in the existing conformance corpus for its portable
codec behavior, but the crate remains host-facing because it exposes native
path/file APIs. CLI is native-only. This evidence does not define a public WASM
ABI, Web Worker, resident runtime, or browser persistence mechanism.

## Explicitly deferred seams

- #10 owns external Semantic API stability, versioning, batch, transaction, and
  bypass policy.
- #23 owns the general validation/diagnostic envelope and temporary-invalid
  editing policy.
- #26 owns resident state, Web Worker placement, IPC/FFI, projection patches,
  host capabilities, and native/browser persistence composition.
- #27/#28 own AI capability and approval architecture.
- #41 owns `.roproj` layout and materialization.

No new crate, semantic `Workspace`/`Project` aggregate, storage/formula
contract, or native/WASM feature-selected semantic behavior was introduced by
#72. Any future direct edge or crate split that changes the Accepted baseline
must amend ADR-0016 explicitly.

## Related authority

- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [Semantic core rationale](semantic-core-rationale.md)
- [Knowledge authority](../governance/knowledge-authority.md)
- GitHub issues #10, #23, #26, #27, #28, #41, #72
