# Rust Crate Architecture

Decision state: Milestone 02 layering is Accepted in ADR-0016. ADR-0020 accepts the transport-neutral Headless Semantic API as the first-class product boundary implemented by the shared application layer. ADR-0022 accepts the resident shared Rust semantic/application runtime and host-separation direction without stabilizing current source/session/transport mechanisms.

Implementation state: ADR-0016 boundary implemented by Issue #72; authoritative validation/report composition implemented by Issue #89. Current workspace-engine operations remain substantially snapshot-style; ADR-0022 resident-runtime topology is Accepted architecture with later implementation in #93–#95.

Architecture authority: ADR-0016 for crate ownership; ADR-0020 for the first-class Semantic API product boundary; ADR-0022 for runtime ownership, resident topology, native/WASM parity, and host separation.

## Purpose

This document records the live Rust workspace and implementation evidence for the crate ownership accepted by [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md), the first-class client contract accepted by [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md), and the runtime/host topology accepted by [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).

The transport-neutral client contract is specified separately in [`semantic-api.md`](../specs/semantic-api.md).

These ADRs remain authority for dependency direction, semantic-client behavior, runtime ownership, portability, host separation, and public-vs-internal stability. Current Rust APIs, session shapes, serde layouts, and transport DTOs remain implementation details unless explicitly stabilized elsewhere.

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

`scripts/workspace-dependency-check.mjs` reads locked Cargo metadata and fails if a workspace package or local edge differs from this graph. CI and the local release-equivalent gate execute that check, including development dependency kinds.

ADR-0022 does not change this DAG and does not add a runtime/host abstraction crate.

## Product contract versus Rust source boundary

ADR-0020 establishes this firewall:

> `workspace-engine` implements the first-class Semantic API contract, but its current Rust `pub` items, re-exports, modules, errors, result structs, and serde shapes are not automatically the public Semantic API.

ADR-0022 extends the same discipline to runtime mechanics:

> A resident Rust runtime is the Accepted interactive topology, but a current or future session handle, revision type, Worker DTO, IPC/FFI request, WASM ABI, persistence adapter, or state-installation mechanism is not automatically a stable product contract.

A Rust surface becomes a stable downstream SDK only if an explicit API specification/version classifies it as such. A serialized Rust type becomes a wire contract only when a transport specification says so.

Future native/WASM/IPC/network adapters conform to semantic/runtime ownership laws, not source-level type equality.

## Responsibility evidence

### semantic-core

Semantic core owns opaque stable-ID types, mutable human-key types, document/schema/entity/field models, typed values and bound relationships, formula expression representation, derived address indexes, intrinsic semantic diagnostics, and whole-document validation. It has no dependency on another workspace crate and no UI, filesystem, network, UUID-generation, or host capability.

ADR-0019 fixes semantic validation/diagnostic meaning while exact Rust diagnostic/report surfaces remain Provisional.

Semantic types remain owned here. Workspace-engine re-exports types required by current first-party adapters for convenience; that does not transfer semantic ownership or make the exact Rust surface a stable external SDK.

### formula-engine

Formula engine owns bounded source parsing, human-address binding, stable-ID bound projection, structural limits, deterministic finite-binary64 calculation, dependency indexes, and formula failures. It depends only on semantic-core among workspace crates.

ADR-0018 remains authoritative for semantic and native/WASM formula/numeric behavior. ADR-0019 maps those facts into shared validation/diagnostics, and ADR-0020 makes relevant stable outcome meaning observable through the Semantic API without promoting current Rust errors/functions to public API.

### diff-engine and merge-engine

Diff owns typed semantic comparison and derived formula impact. Merge owns pure model-level three-way reconciliation and typed conflicts. Both remain below the application boundary, contain no host capability, and retain focused algorithm tests.

Workspace-engine is the first-party application entry point for semantic comparison and merge orchestration. Exact public effect/diff/merge projection shape remains Provisional under ADR-0020.

### storage

Storage owns strict version-specific DTOs/codecs, explicit migration, canonical direct-ro materialization, and native filesystem load/save APIs. It depends on semantic-core and remains a sibling of workspace-engine.

The CLI composition root performs `load -> semantic operation -> canonical encode/write`. Workspace-engine does not depend on paths, files, storage DTOs, or persistence.

ADR-0022 keeps durable persistence outside the semantic runtime authority. Host/storage may materialize an authorized semantic result, but they do not redefine semantic meaning or semantic authorization. Browser/native persistence/recovery and concrete durable commit mechanics remain replaceable host concerns.

### workspace-engine

`tachiko-workspace-engine` evolved in place from the former workflow crate; no parallel orchestration crate or semantic `Workspace`/`Project` aggregate exists.

The engine owns application behavior such as:

- host-supplied stable-ID creation and built-in starters;
- authoritative `ValidationReport` plus complete calculation orchestration;
- calculated values projected through current human addresses;
- overview, field explanation, and stable formula analysis;
- scalar/formula edits and entity lifecycle candidate transitions;
- inert typed field-proposal validation shared with the AI adapter;
- semantic comparison and merge-plus-impact orchestration; and
- deterministic runtime-export projection independent of filesystem and terminal rendering.

Candidate operations are immutable and validate/calculate before success. Mutation previews expose semantic impact where current operation contracts require it.

ADR-0020 promotes the **semantic operation boundary** to a first-class product contract: Query/Command, Propose/Execute, operation gates, semantic atomicity, capability-addressability, and compatibility laws. The complete operation catalogue and exact current Rust functions/results remain Provisional.

ADR-0022 now accepts a resident shared Rust semantic/application runtime as the preferred interactive topology. For an open interactive document, authoritative in-memory semantic state belongs to that runtime rather than to a frontend mirror. Normal interaction should use Semantic API intent/results without repeatedly reconstructing the full document across the client/runtime boundary.

Current workspace-engine functions remain substantially snapshot-style. That is implementation state, not competing architecture authority. #93–#95 own later resident-session, selective-query/invalidation, and retained-incremental implementation.

ADR-0022 requires one authoritative runtime state and ADR-0020 semantic atomicity, but it does **not** define an exact state commit/swap/locking/cloning algorithm, revision token, cancellation policy, or concurrency model.

### ai-api

Provider-free AI code depends only on workspace-engine among workspace crates. It retains AI-facing descriptions, explanations, inert suggestions, and the current `requires_approval` adapter DTO.

Formula analysis, semantic impact, typed candidate cloning, schema checks, formula complexity/projection checks, validation, and calculation delegate to workspace-engine.

Under ADR-0007/ADR-0020, AI is a delegated first-party semantic client over the same Semantic API behavior as other clients. No current AI operation persists or mutates the supplied document. Exact principal/capability/grant/approval/provenance mechanics remain #27/#28.

### cli

CLI depends only on workspace-engine and storage among workspace crates. It owns arguments/dispatch, OS paths, storage load/canonical-encode composition, UUIDv7 generation supplied through the engine creation seam, exclusive host writes, and terminal/JSON rendering.

Validation, calculation materialization, semantic diff, merge-plus-impact, mutation rules, and runtime-export semantic projection are not independently implemented in CLI handlers.

ADR-0020 requires future CLI semantic behavior to map the shared Semantic API. ADR-0022 does not require the native CLI to become a long-lived resident UI; snapshot-style command-line composition remains appropriate for explicit one-shot CLI operations.

## Runtime ownership under ADR-0022

For interactive native/Tauri/Web/WASM clients:

```text
frontend / shell
      |
      | Semantic API intent/results
      v
resident shared Rust semantic/application runtime
      |
      v
workspace-engine + focused semantic engines

host/storage beside the runtime:
filesystem / IndexedDB / dialogs / credentials / Git / process / network
```

The frontend may own revision-keyed projections/query caches, selection/viewport/panels/focus, pending-command/review state, and raw/unbound authoring buffers before semantic admission. Those states are not a competing semantic source of truth.

Full semantic snapshots are explicit boundary tools for load/open, durable materialization/save, import/export, recovery/debug capture, or explicit branch/document exchange. They are not the preferred ordinary per-edit client/runtime protocol.

This topology does not prohibit whole-document clones, snapshots, or full validation **inside** the Rust runtime as implementation mechanisms.

## Before/after ownership

| Concern | Before #72/#26 promotion | Current authority |
| --- | --- | --- |
| Candidate mutation policy | Workflow plus duplicated AI path | Workspace-engine / ADR-0020 Semantic API implementation |
| Validation/calculation orchestration | Workflow, AI, CLI, command-specific paths | Workspace-engine for first-party clients |
| Semantic comparison | Direct CLI and AI calls | Workspace-engine |
| Merge plus base-to-result impact | CLI over merge and diff engines | Workspace-engine |
| Runtime export semantic projection | CLI | Workspace-engine |
| Host persistence and safe writes | CLI/storage | Host/storage, unchanged |
| AI approval DTO | AI API | AI adapter implementation; ADR-0007 authority and #27/#28 future protocol |
| ID generation mechanism | CLI through workflow seam | Host through workspace-engine seam |
| Product-semantic client contract | Provisional/internal | First-class transport-neutral Semantic API under ADR-0020 |
| Interactive authoritative state ownership | Open under #26 | Shared Rust semantic/application runtime under ADR-0022 |
| Resident topology | Spike evidence | Accepted preferred interactive topology under ADR-0022; implementation pending #93–#95 |
| Concrete session/revision/transport | Open | Deferred; not frozen by ADR-0022 |

## Internal bypass versus client bypass

ADR-0020's mandatory client rule does not force lower-level implementation code to call a public facade recursively.

Allowed internal paths include:

- workspace-engine calling semantic-core/formula/diff/merge under ADR-0016;
- storage codecs/migrations operating at the ADR-0017 representation boundary;
- host composition depending on workspace-engine/runtime plus storage;
- focused tests directly invoking their owner contract; and
- deterministic validators participating through ADR-0019.

Forbidden product paths include GUI/CLI/AI/native/WASM adapters implementing a second semantic mutation, validation, formula, gate, identity, diff/merge, or atomicity policy simply because they share a process or repository.

## Portability and runtime evidence

The capability-free portable set is:

- semantic-core;
- formula-engine;
- diff-engine;
- merge-engine;
- workspace-engine; and
- provider-free ai-api semantic adapter code.

`scripts/portable-conformance-check.sh` builds the supported portable set for native and `wasm32-unknown-unknown` and compares stable observations where the production corpus overlaps.

PR #91 adds executable topology evidence that a TypeScript -> Worker -> WASM -> workspace-engine path can retain one Rust-owned semantic document and preserve equivalent exercised native/WASM semantic outcomes while avoiding repeated whole-document boundary traffic.

This evidence supports ADR-0022's runtime ownership/topology. It does not define a public WASM ABI, Worker lifecycle, JS DTO, browser persistence mechanism, memory budget, or latency SLA.

## Native/WASM semantic parity

WASM is an execution target, not a second semantic implementation.

Where native and WASM expose the same semantic capability over the same relevant semantic base/context and deterministic configuration, they must preserve equivalent Stable semantic meaning, including applicable Semantic API operation outcomes, gate decisions, diagnostics/formula facts, and semantic atomicity.

Transport bytes, memory layout, batching, Worker placement, and host error wrappers may differ when they preserve the contract.

A runtime/transport may host, retain, cache, serialize, batch-deliver, or project the Semantic API. It may not redefine it.

## Host capability boundary

Filesystem, IndexedDB/browser persistence, dialogs, credentials, Git/process integration, network access, Tauri host commands, and durable replacement/recovery remain outside workspace-engine.

A composition root may combine runtime, storage, and host adapters. This mechanical composition is not alternate semantic logic.

Semantic publication, durable persistence, and external publication remain distinct effects. ADR-0022 does not make a semantic Execute capability imply filesystem/network/Git/deployment authority.

## Explicitly deferred seams

- exact Rust `ValidationReport`/result APIs remain Provisional under ADR-0019/ADR-0020;
- complete externally Stable Semantic API operation catalogue remains Provisional;
- resident session/handle representation is Deferred;
- revision/precondition and concurrency/conflict mechanics are Deferred to #93 and related runtime work;
- cancellation is Deferred;
- exact runtime state commit/swap/locking/cloning mechanism is Deferred;
- Web Worker lifecycle/loading/startup/memory behavior is Deferred;
- IPC/FFI/network/JS/WASM serialization and ABI are Deferred;
- projection patch/delivery/invalidation mechanics are Deferred to #94;
- retained indexes/caches/incremental calculation/validation are Deferred to #95 subject to full-oracle equivalence;
- browser/native persistence/recovery and IndexedDB schema are Deferred;
- multi-document/project/branch/history residency is Deferred;
- #27/#28 own capability IDs, principals, grants, approval, provenance, and execution authorization;
- #41 owns `.roproj` physical layout/materialization details; and
- a dedicated stable public Rust SDK/facade crate remains Deferred.

No new crate, semantic `Workspace`/`Project` aggregate, storage/formula contract, or native/WASM feature-selected semantic behavior is introduced by ADR-0022. Any future direct crate edge that changes the Accepted ADR-0016 baseline must amend that ADR explicitly.

## Related authority

- [ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md)
- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md)
- [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md)
- [Semantic API specification](../specs/semantic-api.md)
- [Knowledge authority](../governance/knowledge-authority.md)
- Issues #26, #27, #28, #41, #93, #94, #95
- PR #91
