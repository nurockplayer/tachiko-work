# Issue #26 resident semantic runtime and host boundary promotion

Decision state: Research / executable-evidence synthesis. ADR-0022 is the architecture authority.

## Evidence baseline

Merged PR #91 (`16289f8`) provides the executable evidence used by Issue #26:

- the ADR-0016 portable semantic/application crate set compiles for `wasm32-unknown-unknown`;
- the audited portable production sources do not require filesystem/path, clock, ambient randomness, locale, environment, threads, sockets, or network clients;
- a TypeScript -> Node Worker -> WASM -> workspace-engine spike retained one Rust-owned authoritative semantic `Document` across requests;
- native and WASM produced equivalent deterministic semantic observations over the exercised overview/calculation/mutation/diff/snapshot/merge corpus;
- resident ownership avoids whole-document request/result traffic for ordinary interaction; and
- at 1,000 synthetic entities/formulas, the recorded release-WASM mutation remained roughly O(document)-like, locating the remaining exercised pressure inside Rust-side clone/validation/calculation/diff work rather than boundary serialization.

The benchmark is topology/performance-pressure evidence, not a browser/device performance promise.

## Promoted direction

ADR-0022 promotes only these durable rules:

- authoritative interactive semantic state belongs to the shared Rust semantic/application runtime;
- first-party clients do not own a second canonical semantic document model;
- a resident Rust runtime is the preferred interactive topology;
- ordinary interaction uses ADR-0020 Semantic API intent/results rather than full-document round trips as the default edit protocol;
- full semantic snapshots remain explicit load/save/export/recovery/debug/branch-exchange style boundaries;
- frontend projection/cache/selection/viewport/pending-command/raw-authoring state is allowed but is not semantic truth;
- filesystem/browser persistence, dialogs, credentials, Git/process/network, Tauri, and durable replacement/recovery remain host/composition concerns outside `workspace-engine`;
- native and WASM use the same semantic implementation/contracts and preserve equivalent Stable semantic observations where capabilities overlap; and
- a runtime/transport may host, retain, cache, serialize, batch-deliver, or project the Semantic API but may not redefine semantic behavior.

## Deliberately not promoted

The decision does not freeze:

- PR #91's JSON DTOs or raw WASM ABI;
- a Worker lifecycle/loading/startup/memory policy;
- a resident session/handle type;
- revision/precondition tokens;
- concurrency/conflict/cancellation algorithms;
- exact state commit/swap/locking/cloning mechanics;
- projection patch/invalidation protocol;
- IPC/FFI/network mappings;
- browser/native persistence/recovery implementation;
- IndexedDB schema;
- multi-document/branch/history residency; or
- a latency/memory/throughput SLA.

In particular, ADR-0020's all-or-nothing semantic publication does not imply an Accepted runtime `swap` implementation. #93 and related runtime work remain free to choose the concrete state-transition mechanism while preserving one authoritative runtime state and semantic atomicity.

## Implementation separation

Current workspace-engine operations remain substantially snapshot-style. That is implementation lag relative to the accepted resident topology, not an M02 blocker.

Later implementation remains split deliberately:

- #93 resident workspace session + revision-safe commands;
- #94 selective semantic queries + projection invalidation; and
- #95 retained incremental engine state + full-oracle equivalence.

No production code is required to resolve #26.

## Relationship to ADR-0020

ADR-0020 owns **what** Semantic API operations mean. ADR-0022 owns the durable runtime/host topology in which those laws execute.

Concrete transport, session, revision, serialization, persistence, and state-transition mechanisms remain replaceable. A host topology can change without changing Semantic API meaning.
