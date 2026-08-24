# ADR-0022: Resident semantic runtime and host boundary

## Status

Accepted

Decision issue: [#26](https://github.com/nurockplayer/tachiko-work/issues/26)

Executable evidence: merged PR #91 (`16289f8`)

Related authority: ADR-0016, ADR-0019, ADR-0020

## Context

ADR-0016 establishes `tachiko-workspace-engine` as the shared first-party application boundary over the semantic and focused domain engines while keeping storage and host capabilities outside that portable boundary. ADR-0020 subsequently establishes one transport-neutral Semantic API whose semantic laws are mandatory for first-party clients.

Issue #26 asks how those accepted semantic/application boundaries should be hosted for interactive native and Web/WASM clients without creating a second client-owned canonical document model or turning an implementation spike's DTO/ABI into public contract.

Merged PR #91 provides executable topology evidence:

- the portable semantic/application crate set compiles for `wasm32-unknown-unknown`;
- audited portable production code does not require filesystem/path, wall clock, ambient randomness, locale, environment, threads, sockets, or network clients;
- a TypeScript -> Node Worker -> WASM -> workspace-engine spike can retain one Rust-owned authoritative semantic `Document` across requests;
- native and WASM executions produced equivalent deterministic semantic observations over the exercised overview/calculation/mutation/diff/snapshot/merge corpus;
- retaining semantic state on the Rust side avoids whole-document request/result traffic for ordinary interaction; and
- the recorded 1,000-entity/formula benchmark exposed remaining whole-document work inside Rust rather than proving a browser/device latency target.

This evidence is sufficient to promote runtime ownership and host-separation laws. It is not sufficient to freeze a particular Worker lifecycle, session handle, revision token, IPC/FFI schema, serialization, state-swap algorithm, persistence/recovery mechanism, or performance promise.

## Decision

### 1. Interactive authoritative semantic state belongs to the shared Rust semantic/application runtime

For an interactive Tachiko Work client, the authoritative in-memory semantic state is owned by the shared Rust semantic/application runtime built around `workspace-engine` and the lower semantic engines.

A GUI, Web/WASM frontend, Tauri shell, native adapter, or future client MUST NOT maintain a second independently authoritative semantic document model.

Frontend projections, optimistic UI state, caches, and authoring buffers do not become semantic truth merely because they are more recent visually than the last rendered runtime response.

This runtime-state decision does not create a second source of truth beside `.roproj`. ADR-0003/ADR-0017 continue to govern canonical durable representation; this ADR governs where authoritative interactive semantic state and semantic execution live while a project is open.

### 2. A resident runtime is the preferred interactive topology

Normal interactive clients SHOULD retain the authoritative semantic/application runtime across semantic operations rather than serialize and reconstruct the entire semantic document for each edit/query.

Ordinary interaction should cross the boundary as ADR-0020 Semantic API intent/results such as Query, Propose, Execute, and their bounded projections, not as repeated whole-document round trips.

This is a topology rule, not a public session protocol. The exact resident handle, object lifetime, memory ownership ABI, revision identifier, cancellation model, concurrency policy, and transport framing remain Deferred.

### 3. Full semantic snapshots are explicit boundaries, not the normal edit protocol

Full semantic snapshots remain appropriate at explicit boundaries such as:

- open/load;
- durable save/materialization;
- import/export;
- recovery/diagnostic capture;
- explicit branch/document exchange; and
- other intentionally snapshot-oriented operations defined by future contracts.

A first-party interactive edit path SHOULD NOT require whole-document serialization across the client/runtime boundary merely as its ordinary mutation protocol.

This rule does not prohibit an implementation from taking internal snapshots, cloning state, or using full validation for correctness. Those are internal mechanisms and performance concerns.

### 4. Frontend state is projection/workflow state

A frontend MAY own:

- revision-keyed semantic projections/query caches;
- selection, focus, viewport, panels, drag/drop, and other presentation state;
- pending-command and review UI state;
- presentation-local optimistic state that cannot redefine semantic outcome; and
- raw/unbound/incomplete authoring buffers before semantic admission under ADR-0019.

A frontend MUST NOT use those states to create alternate formula, validation, identity, mutation, merge, or operation-gate semantics.

Projection cache invalidation/delivery mechanics, patch shapes, and optimistic-conflict handling remain Deferred to later runtime implementation work.

### 5. Host capabilities remain outside `workspace-engine`

Filesystem access, native/browser persistence, IndexedDB, file dialogs, credentials, Git/process integration, network access, Tauri commands, OS/browser APIs, and durable replacement mechanics remain host/composition responsibilities outside `workspace-engine`.

A composition root MAY combine the semantic runtime with storage/host adapters, but host capability does not grant permission to redefine ADR-0020 semantic behavior or ADR-0007 authorization rules.

Durable persistence is a separate effect from semantic publication. Exact durable commit/recovery mechanics remain owned by storage/host work and are not frozen here.

### 6. Native and WASM use the same semantic implementation and contract

WASM is an execution target, not a second semantic implementation.

Where native and WASM environments expose the same semantic capability over the same relevant semantic base/context and deterministic configuration, they MUST preserve equivalent Stable semantic meaning, including where applicable:

- Query/Command/Propose/Execute semantics;
- operation gate decisions;
- ADR-0019 stable diagnostic observations;
- ADR-0018 formula/calculation facts; and
- ADR-0020 single-command/batch atomicity.

Transport bytes, memory layout, Worker placement, request batching, ABI representation, and host-specific error wrappers may differ when they preserve the semantic contract.

The existing native/WASM conformance corpus is implementation evidence for this rule, not a permanent test-file/API-shape contract.

### 7. Runtime/transport may host the Semantic API, never redefine it

Any native, WASM, Worker, IPC, FFI, or future network runtime is an adapter/host for ADR-0020.

It may retain, cache, serialize, batch-deliver, or project Semantic API requests/results. It MUST NOT introduce host-specific semantic mutation, validation, formula, diff/merge, gate, identity, or atomicity rules.

The exact external serialization/ABI remains Deferred. The JSON DTOs and raw WASM ABI used by PR #91 are explicitly non-authoritative spike artifacts.

### 8. Semantic atomicity does not freeze a runtime commit/swap algorithm

ADR-0020 remains authoritative for all-or-nothing semantic publication.

An authorized Execute transition reaches authoritative runtime state only through the runtime/state boundary while preserving that semantic atomicity. This ADR does **not** prescribe how the runtime commits, swaps, versions, locks, clones, rolls back, or otherwise installs the resulting state.

Revision/concurrency/conflict behavior, stale execution mechanics, and the concrete state-transition implementation remain Deferred to #93 and related runtime work.

### 9. Performance evidence establishes topology pressure, not a product SLA

PR #91 demonstrates that resident ownership removes avoidable O(document)-sized client/WASM request/result traffic and that remaining exercised mutation cost can reside inside Rust-side whole-document work.

It does not establish a browser/device latency guarantee, memory budget, throughput SLA, Worker startup requirement, incremental algorithm, or caching mandate.

Performance optimization may introduce retained indexes/caches or selective projections only if observable semantic behavior remains equivalent to the Accepted full-oracle contracts.

### 10. Current snapshot-style implementation may lag this accepted topology

Current workspace-engine APIs may remain snapshot-style while later implementation work introduces the resident session/runtime shape.

This is implementation lag, not permission for clients to create a second semantic authority.

The production follow-ups remain intentionally separate:

- #93: resident workspace session and revision-safe commands;
- #94: selective semantic queries and projection invalidation; and
- #95: retained incremental engine state with full-oracle equivalence.

Those later implementation tasks MUST NOT retroactively block this Milestone 02 architecture promotion.

## Deliberately Deferred

This ADR does not freeze:

- public session/handle types;
- revision/precondition token representation;
- concurrency/conflict algorithms;
- cancellation policy;
- exact runtime commit/swap/locking/cloning mechanism;
- actual browser Worker lifecycle/loading/startup/memory behavior;
- IPC/FFI/network request/response schemas;
- WASM ABI or JS DTO spelling;
- projection patch/delivery/invalidation protocol;
- native/browser persistence and recovery implementation;
- IndexedDB schema;
- multi-document/project/branch/history residency;
- host credential/Git/process APIs;
- concrete capability/approval protocol (#27/#28);
- operation log/event sourcing/undo history; or
- a stable embedded Rust SDK.

## Explicit non-goals

This ADR does not:

- change ADR-0015 identity semantics;
- change ADR-0017 storage/representation semantics;
- change ADR-0018 formula semantics;
- change ADR-0019 validation/diagnostic semantics;
- reopen ADR-0020 Semantic API meaning/versioning;
- modify ADR-0016's crate DAG;
- add a production crate;
- implement Web/Tauri UI;
- implement #93/#94/#95;
- select a concrete transport or serialization; or
- promise browser/device performance from the research benchmark.

## Alternatives considered

### Let each frontend own a canonical JavaScript/native document model

Rejected. It would create a second semantic authority and allow client-specific mutation/validation behavior to drift from the Rust engines.

### Serialize the full document through the client/runtime boundary for every edit

Rejected as the preferred interactive architecture. It is viable as a simple spike/compatibility mechanism but creates avoidable O(document) boundary traffic and makes frontend state ownership easier to confuse with semantic authority.

### Freeze PR #91's Worker/WASM JSON ABI

Rejected. The spike demonstrates topology viability, not a stable transport contract.

### Put filesystem/browser/Git persistence inside workspace-engine

Rejected. It would violate ADR-0016's portable capability-free application boundary and entangle semantic behavior with replaceable host concerns.

### Require the M02 architecture ticket to implement resident sessions and incremental caches

Rejected. #93 through #95 deliberately separate later implementation/performance work from the architecture promotion.

## Consequences

Positive:

- native and Web/WASM clients share one semantic authority rather than mirror documents in client code;
- ADR-0020 gains an explicit runtime/host mapping without transport lock-in;
- frontend engineering can optimize presentation and authoring state without becoming semantic truth;
- storage, Git, credentials, browser APIs, and persistence remain replaceable host capabilities;
- native/WASM parity can be tested on semantic observations rather than identical ABI bytes; and
- future resident/incremental performance work has a stable correctness boundary.

Costs:

- current snapshot-style APIs do not yet implement the preferred resident interactive topology;
- later runtime work must define revision/concurrency and lifecycle mechanics explicitly;
- hosts must compose semantic runtime and persistence rather than hiding durability inside workspace-engine; and
- client projection/cache logic must respect runtime authority and invalidation instead of treating local state as canonical.

## Required follow-up

- Reconcile `rust-crate-architecture.md`, `frontend-backend-boundary.md`, and `wasm-strategy.md` to this Accepted runtime/host boundary.
- Keep ADR-0020/`semantic-api.md` as semantic contract authority; do not duplicate it here.
- Keep PR #91 as executable topology/conformance evidence.
- Leave #93/#94/#95 as later implementation tasks.
- Close #26 with a Decision Capsule after documentation reconciliation is merged.

## Related

- ADR-0003
- ADR-0016
- ADR-0017
- ADR-0018
- ADR-0019
- ADR-0020
- Issues #26, #27, #28, #41, #93, #94, #95
- PR #91
