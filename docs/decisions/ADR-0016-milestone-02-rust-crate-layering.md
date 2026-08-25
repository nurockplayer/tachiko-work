# ADR-0016: Milestone 02 Rust crate layering

## Status

Accepted

Decision issue: [#20](https://github.com/nurockplayer/tachiko-work/issues/20)

## Context

Tachiko Work's v0.1 workspace proves that the semantic model, formula
calculation, semantic diff, semantic merge, storage, AI queries, reusable
workflows, and CLI can remain in an acyclic Rust graph. It does not yet provide
one complete application boundary for future CLI, Tauri, Web/WASM, AI, and
server clients:

- `tachiko-workflow` already coordinates semantic-core, formula, validation,
  and diff behavior without filesystem or UI dependencies;
- the CLI still coordinates storage, calculation, diff, merge, workflow, and
  rendering directly;
- `tachiko-ai-api` calls the low-level engines directly and repeats candidate
  mutation, validation, and calculation behavior that also exists in workflow;
- the current public Rust structures and `serde` derives are implementation
  evidence, not a stable wire-format or external SDK contract.

The Product Constitution requires a small stable core and replaceable hosts.
ADR-0003 requires storage to depend on semantic contracts rather than define
them. ADR-0007 makes AI a semantic client. ADR-0015 places opaque typed IDs and
typed relationships in the lowest semantic layer while keeping generation and
persisted encoding replaceable.

Issues #23, #24, and #25 still own detailed schema/validation, formula, and
storage decisions. This ADR accepts the current macro layering baseline without
freezing speculative sub-crates or public data layouts. A later Accepted
decision may amend the crate set when concrete dependency, testing,
portability, lifecycle, or public-contract pressure justifies it.

## Decision

### 1. Use eight crates as the current Milestone 02 baseline

The current target workspace is:

```text
crates/
├── semantic-core/
├── formula-engine/
├── diff-engine/
├── merge-engine/
├── storage/
├── workspace-engine/   # evolves from the current workflow crate
├── ai-api/
└── cli/
```

`workspace-engine` replaces rather than sits beside `workflow`. The current
workflow crate is the implementation seed of the shared application/runtime
boundary. Adding a second orchestration crate would create two places for
business behavior without adding an ownership or portability boundary.

The name does not create a `Workspace` or `Project` semantic aggregate.
ADR-0015 keeps v1 semantic authority document-local; a broader aggregate needs
separate product and dependency evidence.

No `schema`, `validation`, `diagnostics`, foundational `types`, plugin,
collaboration, or host-abstraction crate is added by this decision. A later
split requires concrete dependency, testing, portability, independent-lifecycle,
or public-contract pressure and must amend this ADR explicitly rather than
emerge through implementation drift. The number eight is therefore the Accepted
Milestone 02 baseline, not a permanent cardinality constraint on the repository.

### 2. Adopt this direct dependency DAG

Arrows point from a dependent crate to the crate it uses:

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

The target graph is acyclic. `storage` and `workspace-engine` are sibling
boundaries: the host composition root loads or decodes semantic state, invokes
the shared engine, and saves or encodes the resulting state. The engine does
not own filesystem, browser storage, network, Git, or UI capabilities.

### 3. Keep semantic state and application orchestration distinct

`semantic-core` owns the authoritative domain representation and intrinsic
semantic invariants. It contains the minimum shared meanings every client must
agree on, including:

- typed opaque identities and typed relationships;
- document, schema declaration, field, entity, value, and reference meaning;
- the minimum bound formula/reference contract required by semantic state;
- intrinsic structural validation and the minimum diagnostic vocabulary needed
  to state semantic failures.

A running client has one authoritative semantic aggregate. `workspace-engine`
is the application boundary for operations over that aggregate and may own the
derived indexes required by those operations. Whether a host retains a resident
engine-owned aggregate across calls, uses snapshot-style operations, places the
runtime in a Web Worker, or chooses another stateful execution shape remains
owned by #26. UI projections, AI descriptions, storage DTOs, and caches are not
competing semantic state.

`workspace-engine` owns application behavior that coordinates multiple domain
services:

- shared command/query operations for first-party clients;
- operation preconditions and atomic candidate transitions;
- validation and calculation orchestration;
- semantic impact and merge orchestration;
- operation results, projections, and diagnostics independent of presentation;
- the replaceable creation boundary through which a host supplies new IDs.

It must own behavior, not merely re-export lower-level crates.

### 4. Keep engines pure and capability-free

`formula-engine` owns parsing/binding/evaluation behavior and derived dependency
indexes, subject to #24. `diff-engine` owns semantic comparison and derived
impact. `merge-engine` owns model-level three-way reconciliation. These crates
accept semantic values and return semantic results; they do not load files,
render UI, call networks, consult clocks or environment variables, or persist
runtime caches as semantic state.

`storage` owns versioned DTOs/codecs, canonical materialization, migration, and
host persistence behavior subject to #25. Storage types do not become semantic
types merely because both are Rust structures or use `serde`.

### 5. Make clients adapters over one shared application boundary

The CLI and `ai-api` become adapters over `workspace-engine`:

- the CLI owns arguments, filesystem paths, exclusive/atomic host writes, and
  terminal or machine rendering;
- `ai-api` owns AI-facing capability/approval DTOs and presentation, while
  delegating semantic queries and candidate operations to the engine;
- future Tauri, Web/WASM, and server adapters expose the same shared operations
  through host-appropriate IPC/FFI/serialization.

First-party clients must not reproduce validation, formula, diff, merge, or
operation rules. Mechanical host sequencing such as `load → operate → save` is
allowed at a composition root and is not alternate business logic.

This ADR does not settle #10's external Semantic API stability, versioning,
batch, transaction, or bypass policy. During Milestone 02,
`workspace-engine` is the supported internal boundary for first-party clients;
promotion to a stable external SDK/API requires the separate #10 decision.

### 6. Define public and workspace-internal boundaries explicitly

Rust `pub` visibility does not by itself promise a stable downstream API.

- The semantic invariants accepted by existing ADRs are stable; the exact Rust
  field layout, constructors, module layout, and `serde` representation are
  workspace-internal until deliberately versioned.
- `workspace-engine` command/query concepts are the target client boundary. The
  transport-neutral semantic laws were subsequently Accepted by ADR-0020 and
  the proposal envelope laws by ADR-0024, while the exact Rust external API and
  capability/approval mapping remain Provisional/#28.
- formula, diff, and merge engine Rust APIs are workspace-internal. Where an
  Accepted ADR governs behavior, it remains authoritative; otherwise current
  detailed behavior remains an implemented Provisional contract.
- storage's durable on-disk contract is owned by #25/#37/#38; storage DTOs and
  codecs are not public semantic APIs.
- CLI commands and AI capability results are adapter contracts, not permission
  to bypass the shared engine or expose internal engine types accidentally.

Shared types and errors remain with the layer that owns their meaning:

- typed IDs, values, relationships, and the minimum bound-reference contract
  live in `semantic-core`, not a foundational micro-crate;
- each domain engine owns its internal typed failures and maps them into
  structured `workspace-engine` operation results at the application boundary;
- storage owns format/migration/I/O failures and the host maps those alongside
  application failures without turning them into semantic state;
- minimal semantic diagnostic meaning remains low-level, while #23 owns the
  stable cross-client diagnostic envelope, locations, severities, and policy.

### 7. Require a portable semantic/application set

The following crates must compile for supported native targets and
`wasm32-unknown-unknown` and must have equivalent deterministic behavior where
host capabilities overlap:

- `semantic-core`;
- `formula-engine`;
- `diff-engine`;
- `merge-engine`;
- `workspace-engine`;
- provider-free `ai-api` semantic adapter code.

These crates may not depend implicitly on filesystems, OS paths, browser
globals, network clients, wall clocks, locales, environment variables, process
state, or ambient randomness. Host-dependent creation mechanisms are injected;
UUIDv7 remains a Provisional generator, not semantic meaning.

`cli` is native-only. The current `storage` crate is host-facing because it
exposes filesystem paths and file I/O even though some codecs may compile on
WASM. #25 and #26 may later justify separating portable codecs from native and
browser persistence hosts. This ADR does not create those crates in advance.

### 8. Forbid reverse and bypass dependencies

The following edges are forbidden:

- `semantic-core` to any other workspace crate;
- `semantic-core` or a domain engine to storage DTOs/codecs, filesystem, UI,
  AI, network, Git-host, plugin-host, or client types;
- `formula-engine`, `diff-engine`, or `merge-engine` to
  `workspace-engine`, storage, or client/host crates;
- storage to formula, diff, merge, workspace-engine, or client crates;
- `workspace-engine` to storage, CLI, AI, Tauri, Web, filesystem, browser, or
  provider-specific crates;
- AI, CLI, Tauri, Web, or future server adapters implementing alternate
  semantic mutation/validation/calculation/diff/merge rules;
- any workspace crate depending on `cli`;
- cycles hidden behind feature flags;
- features that select different semantic behavior for native and WASM.

Host composition roots may depend on both `workspace-engine` and storage or a
host adapter. Import/export adapters may depend downward on semantic contracts
for conversion, but semantic crates never depend on adapters.

## Deliberately Provisional seams

This ADR accepts the macro layering baseline while leaving these narrower
decisions open:

- #23 may keep schema declaration and validation together in modules inside
  `semantic-core` or justify a `validation` crate that depends downward on core
  and possibly formula services. It may not introduce a reverse dependency
  into core.
- #24 may refine source, unbound, bound, and typed formula representations. It
  must preserve the acyclic direction; a new low-level formula-contract crate
  requires demonstrated pressure rather than convenience.
- #23 owns the stable diagnostic envelope and phase/policy model. Minimal
  semantic diagnostic meaning may stay low-level while emitters remain with the
  engines that detect failures.
- #25 owns storage DTO, codec, migration, package, and host-I/O sub-boundaries.
  `storage → semantic-core` is not Provisional; its internal crate split is.
- #26 owns the stateful runtime, IPC/FFI, Web Worker, projection-patch, and host
  capability details. It must build on this layering rather than introduce a
  client-specific semantic core.
- #10 owns whether and how the engine surface becomes a stable external API.

These issues do not invalidate the Accepted baseline or its forbidden dependency
directions. If later evidence requires a new crate or direct edge that changes
the baseline DAG, the responsible decision must amend or supersede this ADR
explicitly rather than allowing implementation drift.

## Unresolved implementation risks

- Wire-authored expression trees can currently reach recursive semantic
  validation and calculation without the formula authoring/AI complexity gate.
  #24 must establish one shared structural limit before untrusted native or
  WASM evaluation.
- Workflow and AI result types are not intentional IPC/FFI DTO contracts. #10
  and #26 must define explicit boundary/versioning types rather than stabilize
  internal Rust structures accidentally.
- Storage currently mixes portable string codecs with native filesystem APIs.
  #25/#26 must preserve codec reuse without introducing a storage dependency
  into the portable engine.

## Alternatives considered

### Keep the CLI as the broad composition layer

Rejected. It would require each future client to coordinate validation,
formula, diff, merge, and operations independently, making semantic behavior
drift likely.

### Add `workspace-engine` beside `workflow`

Rejected. Current workflow already has the application-layer dependency shape.
A second crate would duplicate ownership and add glue without a distinct
lifecycle.

### Keep the `workflow` name indefinitely

Rejected as the target name. Its responsibility is broader than guided product
workflows once it becomes the shared command/query and runtime boundary. The
existing crate should evolve in place and be renamed during migration.

### Put orchestration or storage in `semantic-core`

Rejected. This creates a God core, introduces host concerns, and makes semantic
invariants harder to separate from replaceable mechanisms.

### Make `workspace-engine` own storage

Rejected for Milestone 02. It would force filesystem/browser capability and
async durability questions into the portable application layer. Host
composition can combine the two siblings without duplicating semantic rules.

### Split `schema`, `validation`, `diagnostics`, and common `types` now

Rejected. Current schema declarations, typed values, relationships,
diagnostics, and validation traverse one small document aggregate. Splitting
them before #23/#24 settle the contract would create glue, possible cycles, and
micro-crate debt.

### Collapse formula, diff, and merge into the engine

Rejected. They have distinct pure algorithms, tests, implemented behavior, and
reuse pressure below application orchestration. Accepted ADRs remain
authoritative where they exist; other detailed contracts remain Provisional.
Keeping the engines separate makes their dependency and portability constraints
enforceable.

### Pre-build plugin, collaboration, or generic host ports

Rejected. No current dependency requires them. Future implementations must
attach above or beside the shared semantic/application layers without becoming
mandatory core dependencies.

## Migration

1. Record this decision without a broad code refactor.
2. Complete the ADR-0015 identity migration inside the low-level semantic
   boundary while preserving the replaceable ID-generation seam.
3. Resolve #23/#24 enough to name the validation/formula contracts. If either
   decision produces concrete pressure for a crate split or new direct edge,
   amend ADR-0016 explicitly before changing the baseline DAG.
4. Rename `tachiko-workflow` to `tachiko-workspace-engine` and expand it to own
   the existing shared operations plus calculation, diff, and merge
   orchestration.
5. Move CLI business orchestration behind the engine, leaving the CLI with
   host I/O, arguments, and rendering; reduce its direct workspace dependencies
   to engine and storage.
6. Rebase `tachiko-ai-api` on engine commands/queries and remove duplicate
   candidate mutation/validation/calculation paths.
7. Add CI checks for the portable crate set on native and
   `wasm32-unknown-unknown`, including shared deterministic fixtures where
   applicable.
8. Let #25/#26 determine whether portable codecs and native/browser hosts need
   separate storage crates; let #10 determine any external API stabilization.

Each step must reconcile the live dependency graph and downstream tests before
changing a shared contract.

## Required follow-up documentation

This crate-ownership decision does not require a new normative specification.
The implementation contracts remain owned by existing work:

- #10: external command/query/API stability and versioning;
- #23: schema/validation stages and diagnostic contract;
- #24: formula compilation, binding, numeric, and resource-limit contract;
- #25/#37/#38: storage DTO, version, and canonical-encoding specifications;
- #26: native/WASM host and bridge contract.

A focused implementation issue should own the staged workflow-to-engine
migration. The refactor must not be folded into this decision PR.

## Consequences

Positive:

- every first-party client has one place to obtain semantic application
  behavior;
- the semantic kernel remains small and host-independent;
- storage and native/browser capability evolution cannot redefine meaning;
- native and WASM clients can share the deterministic domain/application set;
- unresolved formula, validation, storage, plugin, and collaboration details
  retain explicit escape hatches;
- the migration removes direct client fan-out without creating an extra
  orchestration crate.

Negative:

- the current CLI and AI API require follow-up dependency reduction;
- renaming workflow creates short-term crate/package churn;
- storage remains a mixed codec/host crate until #25/#26 justify a split;
- the supported first-party engine surface remains Provisional until #10 and
  adjacent contracts are resolved.

## Verification evidence

At the time of acceptance:

- the current eight-crate dependency graph is acyclic;
- `semantic-core` has only `serde` as a non-dev dependency;
- current workflow has no storage or host dependency and already coordinates
  semantic-core, formula, and diff operations;
- current CLI directly depends on six workspace crates, demonstrating the
  application-boundary gap;
- current AI API directly depends on core, formula, and diff and independently
  validates candidate changes;
- semantic-core, formula-engine, diff-engine, merge-engine, workflow, and
  provider-free ai-api compile together for `wasm32-unknown-unknown`.

## Implementation status (2026-08-23)

Issue #72 implements this decision without amending its Accepted semantics:

- the former workflow crate has evolved in place into
  `tachiko-workspace-engine`; no parallel workflow ownership layer remains;
- workspace-engine depends on diff, merge, formula, and semantic core, while
  storage remains a sibling;
- provider-free AI depends only on workspace-engine among workspace crates;
- CLI depends only on workspace-engine and storage among workspace crates and
  retains host parsing, persistence composition, safe writes, and rendering;
- exact Cargo-metadata dependency checking is part of CI and the local
  release-equivalent gate;
- the native/WASM corpus executes workspace-engine calculation and AI semantic
  proposal/query behavior on both targets in addition to existing portable
  semantic evidence.

The implementation remains document-local and snapshot-style, keeps the
host-supplied stable-ID generator seam, and introduces no storage dependency or
host capability into the portable application set. Subsequent ADR-0019,
ADR-0020, ADR-0022, ADR-0023, and ADR-0024 accept the validation, Semantic API,
runtime, `.roproj/v1`, and proposal laws without making the current Rust/wire
surfaces stable. Capability/approval/digest protocol remains #28, proposal
lifecycle implementation #29, and resident revision/session implementation
#93.

## Related

- Product Constitution §§2.2, 2.3, 2.6, 2.7, 6
- Design Principles §§3, 6, 8, 9, 12
- ADR-0001, ADR-0003, ADR-0007, ADR-0015
- `docs/architecture/semantic-core-rationale.md`
- `docs/architecture/rust-crate-architecture.md`
- Issues #10, #20, #23, #24, #25, #26
