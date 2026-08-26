# Rust Crate Architecture

Decision state: Milestone 02 layering is Accepted in ADR-0016. ADR-0020 accepts
the transport-neutral Headless Semantic API as the first-class product boundary
implemented by the shared application layer; it does not stabilize the current
Rust source surface. ADR-0024 accepts the immutable revision-pinned
SemanticPatch and representation-neutral exact-change/base-binding laws without
stabilizing a Rust or wire DTO. ADR-0022 accepts the resident shared Rust
semantic/application runtime and host-separation direction without stabilizing
current session/transport mechanisms. ADR-0026 accepts the provider-neutral
Principal/capability/scope/Grant/footprint/Approval/provenance laws without
selecting crate placement or public Rust/wire types.

Implementation state: ADR-0016 boundary implemented by Issue #72; authoritative
validation/report composition implemented by Issue #89. Current workspace-engine
operations remain substantially snapshot-style; resident runtime implementation
is deferred to #93–#95. Current one-field inert proposal validation does not
implement ADR-0024 proposal occurrence identity, base/compatibility binding, or
AtomicBatch.

Architecture authority: ADR-0016 for crate ownership; ADR-0020 for the
first-class Semantic API product boundary; ADR-0024 for SemanticPatch proposal
meaning; ADR-0022 for runtime ownership, resident topology, native/WASM parity,
and host separation; ADR-0026 for authorization and exact Approval meaning.

## Purpose

This document records the live Rust workspace and implementation evidence for
the crate ownership accepted by
[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md).
The first-class client contract is specified separately in
[`semantic-api.md`](../specs/semantic-api.md), and the runtime/host topology is
Accepted in
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).

The ADRs remain authority for dependency direction, forbidden edges,
portability, semantic-client rules, runtime ownership, public-vs-internal
stability, and future amendments.

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

ADR-0022 does not change this DAG or add a runtime/host abstraction crate.

## Product contract versus Rust source boundary

ADR-0020 adds an explicit firewall:

> `workspace-engine` implements the first-class Semantic API contract, but its
> current Rust `pub` items, re-exports, modules, errors, result structs, and
> serde shapes are not automatically the public Semantic API.

ADR-0022 applies the same discipline to runtime mechanics: the resident Rust
runtime is the Accepted interactive topology, but a session handle, revision
type, Worker DTO, IPC/FFI request, WASM ABI, persistence adapter, or runtime
state-installation mechanism is not automatically a stable product contract.

A Rust surface becomes a stable downstream SDK only if a future explicit API
specification/version classifies it as such. A serialized Rust type becomes a
wire contract only when a transport specification says so.

This distinction allows first-party adapters to share one Rust implementation
without turning current source-level convenience into permanent ecosystem
compatibility debt.

Future native/WASM/IPC/network adapters are expected to conform to the semantic
and runtime-ownership contracts, not to source-level Rust type equality.

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
unchanged by the first-class Semantic API and resident-runtime decisions.

ADR-0019 explicitly preserves storage format/migration failures as a
representation-local family rather than promoting them into universal semantic
diagnostics.

ADR-0022 keeps durable persistence outside workspace-engine. Storage/host layers
may materialize an authorized semantic result, but they do not redefine
semantic meaning or semantic authorization. Browser/native persistence,
recovery, and concrete durable commit mechanisms remain host concerns.

### workspace-engine

`tachiko-workspace-engine` evolved in place from the former workflow crate; no
parallel orchestration crate or semantic workspace aggregate exists. Current
operations remain document-local and substantially snapshot-style.

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
and mutation previews include semantic impact where the existing operation
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

ADR-0024 adds the **proposal envelope law** at that same application boundary:
one immutable proposal occurrence binds a Semantic API compatibility contract,
one exact semantic base, and exactly one typed Command or ordered AtomicBatch.
It does not add another engine, crate, mutation primitive, operation vocabulary,
or storage dependency. ADR-0026 consumes its exact binding structurally and
deliberately selects no canonical bytes, digest, public DTO, or crate. Exact
proposal/revision Rust types, ID generation, transport, and lifecycle mechanics
remain Provisional or owned by #29/#93.

ADR-0026 adds the **authorization law** beside that application boundary. The
trusted semantic/application authority derives disclosure scope and associated
mutation-class/canonical-write-scope requirements from typed meaning and
relevant base/candidate relationships. Trusted composition combines the
requested action with each associated pair, then enforces live scoped Grants
and exact Human Approval for Delegated-origin or Delegated-authority
publication. Exact module/crate
placement remains Provisional, but enforcement must not live only in `ai-api`,
UI, or client convention, and authorization state must not become
`semantic-core` Document meaning.

ADR-0022 now accepts a resident shared Rust semantic/application runtime as the
preferred interactive topology. For an open interactive document, authoritative
in-memory semantic state belongs to that runtime rather than to a frontend
mirror. Normal interactive clients should use Semantic API intent/results
without repeatedly reconstructing the complete document across the
client/runtime boundary.

The current snapshot-style surface is implementation state, not competing
architecture authority. #93–#95 own later resident-session, selective-query/
projection-invalidation, and retained-incremental implementation.

ADR-0022 requires one authoritative runtime state and ADR-0020 semantic
atomicity, but does not define an exact runtime commit/swap/locking/cloning
algorithm, revision token, cancellation policy, or concurrency model.

### ai-api

Provider-free AI code depends only on workspace-engine among workspace crates.
It retains AI-facing descriptions, explanations, inert suggestions, and the
`requires_approval` adapter DTO. Formula analysis, semantic impact, typed
candidate cloning, schema checks, formula complexity/projection checks,
validation, and calculation delegate to workspace-engine.

Under ADR-0007/ADR-0020/ADR-0024/ADR-0026 the AI crate is an adapter/projection over the
same first-class Semantic API behavior and immutable revision-bound proposal
contract as other clients. Its current
`Suggestion { field, value, requires_approval }` has no proposal occurrence ID,
Semantic API compatibility contract, semantic base, general Command, or
AtomicBatch. It is implementation evidence only, not the SemanticPatch wire or
source contract. `requires_approval` remains current v0.1 safety behavior, not
the ADR-0026 scoped Grant, footprint, exact Approval, or provenance contract.

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
Semantic API rather than grow independent semantic rules. ADR-0022 does not
require one-shot CLI invocations to become long-lived resident UI sessions.

## Resident runtime ownership

For interactive native/Tauri/Web/WASM clients, ADR-0022 accepts this topology:

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

The frontend may own revision-keyed semantic projections/query caches,
selection/viewport/panels/focus, pending-command/review state, and raw/unbound
authoring buffers before semantic admission. These states are not a competing
semantic source of truth.

Full semantic snapshots remain explicit boundaries for load/open, durable
materialization/save, import/export, recovery/debug capture, or explicit
branch/document exchange. They are not the preferred ordinary per-edit
client/runtime protocol.

This topology does not prohibit whole-document clones, snapshots, or full
validation inside Rust as implementation mechanisms.

## Before/after ownership

| Concern | Before #72/#26 | Current authority |
| --- | --- | --- |
| Candidate mutation policy | Workflow plus duplicated AI path | Workspace-engine / Semantic API implementation |
| Validation/calculation orchestration | Workflow, AI, CLI, command-specific paths | Workspace-engine for first-party clients |
| Semantic comparison | Direct CLI and AI calls | Workspace-engine |
| Merge plus base-to-result impact | CLI over merge and diff engines | Workspace-engine |
| Runtime export semantic projection | CLI | Workspace-engine |
| Host persistence and safe writes | CLI/storage | CLI/storage/host composition, unchanged |
| AI proposal envelope | One-field inert `Suggestion` only | ADR-0024 SemanticPatch accepted at the Semantic API boundary; implementation pending #29/#93 |
| Semantic authorization/Approval | Not implemented | ADR-0026 representation-neutral contract; exact placement/DTO/state remains #29/#30/#93 |
| ID generation mechanism | CLI through workflow seam | CLI through workspace-engine seam |
| Product-semantic client contract | Provisional/internal | First-class transport-neutral Semantic API under ADR-0020 |
| Interactive authoritative state ownership | Open under #26 | Shared Rust semantic/application runtime under ADR-0022 |
| Resident interactive topology | PR #91 spike evidence | Accepted under ADR-0022; implementation pending #93–#95 |
| Concrete session/revision/transport mechanics | Open under #26 | Deferred; not frozen by ADR-0022 |

Low-level diff and merge algorithms still validate or calculate where their own
pure correctness contracts require it. That is algorithm ownership below the
application boundary, not a second client policy path.

## Internal bypass versus client bypass

ADR-0020's mandatory client rule does not force lower-level implementation code
to call a public facade recursively.

Allowed internal paths include:

- workspace-engine calling semantic-core/formula/diff/merge under this DAG;
- storage codecs/migrations operating at the representation boundary;
- host composition depending on workspace-engine/runtime plus storage;
- focused tests directly invoking their owner contract; and
- deterministic validators participating through ADR-0019.

Forbidden product paths include GUI/CLI/AI/native/WASM adapters implementing a
second semantic mutation, validation, formula, gate, identity, diff/merge, or
atomicity policy simply because they are in the same process or repository.

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

PR #91 adds executable topology evidence that a TypeScript → Node Worker → WASM
→ workspace-engine path can retain one Rust-owned authoritative semantic
`Document`, preserve equivalent exercised native/WASM semantic outcomes, and
avoid repeated whole-document request/result traffic.

This evidence supports semantic portability and ADR-0022 runtime ownership. It
does not define a public WASM ABI, Web Worker lifecycle, resident session type,
browser persistence mechanism, wire DTO, memory budget, or browser/device
latency SLA.

## Native/WASM semantic parity

WASM is an execution target, not a second semantic implementation.

Where native and WASM expose the same semantic capability over the same relevant
semantic base/context and deterministic configuration, they must preserve
equivalent Stable semantic meaning, including applicable Semantic API operation
outcomes, gate decisions, diagnostic/formula facts, and semantic atomicity.

Transport bytes, memory layout, request batching, Worker placement, and host
error wrappers may differ when they preserve the contract.

A runtime/transport may host, retain, cache, serialize, batch-deliver, or project
the Semantic API. It may not redefine it.

## Host capability boundary

Filesystem, IndexedDB/browser persistence, dialogs, credentials, Git/process
integration, network access, Tauri host commands, and durable replacement/
recovery remain outside workspace-engine.

A composition root may combine runtime, storage, and host adapters. This
mechanical composition is not alternate semantic logic.

Semantic publication, durable persistence, and external publication remain
distinct effects under ADR-0007/ADR-0022/ADR-0026. Semantic Execute authority
does not implicitly grant filesystem/network/Git/plugin/deployment authority.

## Explicitly deferred seams

- ADR-0019 owns validation/diagnostic meaning and temporary-invalid candidate
  boundaries; exact Rust APIs remain Provisional.
- ADR-0020 owns external Semantic API semantic laws and compatibility; complete
  operation catalogue and exact Rust/wire shapes remain Provisional.
- ADR-0024 owns proposal occurrence immutability, exact-change and Semantic API
  compatibility binding, semantic-base pinning, and stale meaning; Rust/wire
  encodings and implementation remain Provisional/#29/#93.
- ADR-0026 owns Principal, capability, stable-ID scope, Grant, trusted
  AuthorizationFootprint, exact Approval, expiry/replay/revocation, provenance,
  and external-effect separation. Exact crate/module placement, DTOs, storage,
  clocks, result codes, and wire formats remain Provisional/#29/#30/#93;
  canonical bytes/digest/signature/MAC/portable tokens remain Deferred.
- ADR-0022 owns resident runtime/state and host-separation laws, while session
  handle shape, revision/concurrency, cancellation, state commit/swap/locking/
  cloning mechanics, Web Worker lifecycle, IPC/FFI/network mapping, projection
  delivery, and persistence/recovery implementations remain Deferred.
- #93 owns later resident workspace session and revision-safe command
  implementation.
- #94 owns later selective semantic queries and projection invalidation.
- #95 owns later retained incremental engine state with full-oracle equivalence.
- ADR-0023 and the `.roproj/v1` specifications own the Accepted layout and
  version-owned wire contract; production materialization remains later
  storage/host implementation work.
- A dedicated stable public Rust SDK/facade crate is Deferred until downstream
  pressure justifies it.

No new crate, semantic `Workspace`/`Project` aggregate, storage/formula
contract, or native/WASM feature-selected semantic behavior is introduced by
ADR-0022. Any future direct edge or crate split that changes the Accepted
ADR-0016 baseline must amend that ADR explicitly.

## Related authority

- [ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md)
- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md)
- [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md)
- [ADR-0023](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md)
- [ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md)
- [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md)
- [Semantic API specification](../specs/semantic-api.md)
- [Semantic authorization specification](../specs/semantic-authorization.md)
- [Semantic core rationale](semantic-core-rationale.md)
- [Knowledge authority](../governance/knowledge-authority.md)
- GitHub issues #26, #27, #28, #29, #30, #41, #93, #94, #95
- PR #91
