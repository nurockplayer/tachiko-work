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
validation/report composition implemented by Issue #89. Issue #29 implements
the provisional SemanticPatch lifecycle, Issue #30 its `ai-api` security
composition, and Issue #93 the first production resident session with internal
monotonic revision, explicit snapshots, and guarded state installation. Issue
#94 adds internal occurrence-and-revision-pinned selective entity/field
projections and fresh full-oracle invalidation facts; #95 retains incremental-
state work. Issue #123
implements the storage-owned `.roproj/v1` pure codec, native exact-tree host
workflow, and CLI composition without changing the Accepted crate DAG.

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
direct-ro and `.roproj/v1` materialization, and native filesystem load/save
APIs. It depends on semantic-core and remains a sibling of workspace-engine.

Issue #123 adds production pure `.roproj/v1` encode/decode over the exact
18-file tree plus native canonical-only load, bounded canonicalization, and
staged absent-destination publication. The CLI composes these as explicit
materialize, canonical-only validate, and bounded canonicalize operations. The
workflow operates without Git and does not move filesystem authority into
workspace-engine.

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
parallel orchestration crate or semantic workspace aggregate exists. Semantic
operations remain document-local, with snapshot evaluation retained inside one
production resident state owner.

The engine owns real application behavior:

- host-supplied stable-ID creation and built-in starters;
- authoritative `ValidationReport` plus complete calculation orchestration;
- calculated values projected through current human addresses;
- overview, human-addressed field explanation, and stable formula analysis;
- bounded typed semantic Analysis Query selection, grouping, Count/Number
  Min/Max, per-member observations, exact-context pairing, lineage, and
  complete-result Query authorization;
- scalar/formula edits and entity lifecycle candidate transitions;
- inert typed field-proposal validation shared with the AI adapter;
- a provisional snapshot-style SemanticPatch lifecycle for stable-ID typed
  field-value Commands and ordered AtomicBatch evaluation, including scoped
  Grants/preview, exact Human Approval, atomic publication/consumption,
  verification, and receipts through a host-supplied revision/publication seam;
- a provisional resident session owning one authoritative `Document`
  occurrence, an internal monotonic `SemanticRevision`, revision-pinned
  validation/calculation queries, explicit detached snapshots, and guarded
  compare-and-publish through that same publication seam;
- occurrence-and-revision-pinned selective entity/field projections that keep
  stable subjects, stored literals, bound formula definitions, calculated
  outcomes, diagnostics, and mutable human addresses distinct, plus fresh
  deterministic downstream projection invalidation without retained caches;
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
proposal/revision Rust types, ID generation, and transport remain Provisional.
Issue #29 supplies the provisional snapshot/publication lifecycle. Issue #93
composes its unchanged command, patch, stale/conflict, authorization, Approval,
and publication meanings with the concrete resident revision/state owner.

ADR-0026 adds the **authorization law** beside that application boundary. The
trusted semantic/application authority derives operation-family/disclosure-
scope and associated operation-family/mutation-class/canonical-write-scope
requirements from typed meaning and relevant base/candidate relationships.
Trusted composition combines the requested action with each associated tuple,
then enforces live scoped Grants
and exact Human Approval for Delegated-origin or Delegated-authority
publication. Issue #29 places the current provisional trusted lifecycle and
authorization-state implementation in workspace-engine. Public DTO/module
stability remains Provisional; enforcement must not live only in `ai-api`, UI,
or client convention, and authorization state must not become `semantic-core`
Document meaning.

ADR-0022 now accepts a resident shared Rust semantic/application runtime as the
preferred interactive topology. For an open interactive document, authoritative
in-memory semantic state belongs to that runtime rather than to a frontend
mirror. Normal interactive clients should use Semantic API intent/results
without repeatedly reconstructing the complete document across the
client/runtime boundary.

The current resident session remains an internal Provisional Rust surface, not
a public session/transport contract. Issue #94 implements the current internal
selective-query/projection-invalidation slice; #95 owns retained-incremental
implementation.

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

Issue #30's provisional `security_boundary` accepts typed Propose/Execute only
after the workspace lifecycle proves an active Delegated occurrence. It may
publish only through that lifecycle's exact Approval and guarded publication
path; it never directly mutates a supplied snapshot or persists one. Raw
semantic/storage mutation and host effects are denied independently.

### cli

CLI depends only on workspace-engine and storage among workspace crates. It
owns:

- Clap arguments and command dispatch;
- OS paths and default titles derived from host paths;
- direct `.ro` and explicit `.roproj/v1` storage/host composition;
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
| AI proposal envelope | One-field inert `Suggestion` only | Issue #29 implements a provider-neutral provisional SemanticPatch/AtomicBatch lifecycle in workspace-engine; #30 adds typed `ai-api` proposal/execution delegation with inert untrusted evidence; #93 supplies resident revision/session mechanics |
| Semantic authorization/Approval | Not implemented | Issue #29 implements provisional trusted in-process relational Grants, scoped preview, exact finite Approval state, atomic consumption/publication, and receipts; #30 adds hostile-client admission and safe denials; #93 supplies guarded resident publication while public wire/authentication remains Deferred |
| ID generation mechanism | CLI through workflow seam | CLI through workspace-engine seam |
| Product-semantic client contract | Provisional/internal | First-class transport-neutral Semantic API under ADR-0020 |
| Interactive authoritative state ownership | Open under #26 | Shared Rust semantic/application runtime under ADR-0022 |
| Resident interactive topology | PR #91 spike evidence | Accepted under ADR-0022; #93 implements the resident session and #94 its current selective projection/invalidation surface, while #95 retains incremental work |
| Concrete session/revision/transport mechanics | Open under #26 | Internal in-process session/revision mechanics implemented by #93; public transport shapes remain Deferred |

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
numeric bytes, the production `.roproj/v1` exact 18-path encode/decode/re-encode
record, workspace-engine calculated queries, AI formula explanation, and inert
approval-required AI proposal validation. Issue #150 adds fixed bounded
Analysis Query success, structured-failure, authorization, and paired-context
records through the same native/WASM corpus.

Storage is also present in existing conformance coverage for portable codec
behavior. Its fixed `.roproj/v1` record proves native/WASM exact-tree parity for
the pure codec, while the crate remains host-facing because it also exposes
native path/file APIs. CLI is native-only.

PR #91 adds executable topology evidence that a TypeScript → Node Worker → WASM
→ workspace-engine path can retain one Rust-owned authoritative semantic
`Document`, preserve equivalent exercised native/WASM semantic outcomes, and
avoid repeated whole-document request/result traffic.

Issue #93 adds production evidence through the same corpus: the resident
workspace-engine session retains one `Document`, pins queries to its current
opaque revision, installs an existing typed FormulaUpdate through the guarded
publication seam, advances once, and rejects a stale precondition identically
on native and WASM.

This evidence supports semantic portability and ADR-0022 runtime ownership. It
does not define a public WASM ABI, Web Worker lifecycle, public resident session type,
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
  compatibility binding, semantic-base pinning, and stale meaning; Issue #29
  supplies the current provisional Rust lifecycle and #93 the internal resident
  revision mechanics, while public wire encodings remain Provisional.
- ADR-0026 owns Principal, capability, stable-ID scope, Grant, trusted
  AuthorizationFootprint, exact Approval, expiry/replay/revocation, provenance,
  and external-effect separation. Exact crate/module placement, DTOs, storage,
  clocks and wire formats remain Provisional; Issue #29 supplies the
  current replaceable in-process implementation, while #30 supplies provisional
  provider-facing context/error shapes and stable internal code meanings;
  canonical bytes/digest/signature/MAC/portable tokens remain Deferred.
- ADR-0022 owns resident runtime/state and host-separation laws. Issue #93
  supplies current Provisional evidence for the internal resident workspace
  session, opaque monotonic revision, exact in-process comparison, and guarded
  state installation. Public session-handle shape, broader cross-host
  concurrency, cancellation, public commit/swap/locking/cloning contracts, Web
  Worker lifecycle, IPC/FFI/network mapping, projection delivery, and
  persistence/recovery implementations remain Deferred.
- #94 implements the current internal selective semantic query and fresh
  full-oracle projection-invalidation surface without stabilizing transport.
- #95 owns later retained incremental engine state with full-oracle equivalence.
- ADR-0023 and the `.roproj/v1` specifications own the Accepted layout and
  version-owned wire contract; #123 implements the production pure codec plus
  current native exact-tree materialize/canonical-only-validate/explicit-
  canonicalize host workflow. Storage also owns #3's packaged `.ro` pure codec
  and native pack/unpack/compare boundary; #44 composes those standalone
  boundaries into optional provider-neutral Git/CI review and consistency
  checks at the CLI/repository edge. Broader hostile source/path races, full
  durability/recovery, and host work remain Deferred.
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
- GitHub issues #3, #26, #27, #28, #29, #30, #41, #44, #93, #94, #95, #123
- PR #91
