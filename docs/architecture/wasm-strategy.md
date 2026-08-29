# WASM Strategy

Decision state: Accepted runtime direction under ADR-0022; concrete browser/Worker/session/transport mechanics remain Deferred.

Executable evidence: PR #91.

## Principle

The semantic/application runtime is portable across supported native and `wasm32-unknown-unknown` environments.

WASM is an execution target and host mapping for the same Semantic API and Rust semantic/application authority. It is not a second semantic implementation or the semantic foundation itself.

## Runtime ownership

ADR-0022 accepts a resident shared Rust runtime as the preferred interactive topology.

```text
Native / Tauri client      Web client
        |                      |
        | Semantic API         | Semantic API
        v                      v
shared Rust semantic/application runtime
        |
        +-- native execution target
        +-- WASM execution target
```

Interactive clients do not own a second canonical semantic document model. Frontend projections, caches, selection/viewport state, and raw authoring buffers may live outside Rust where ADR-0019/ADR-0022 permit them, but semantic meaning and operation gates remain in the shared runtime.

Ordinary edits/queries should cross the host/runtime boundary as Semantic API intent/results rather than repeated whole-document serialization. Full snapshots remain appropriate at explicit load/save/export/recovery/debug/branch-exchange style boundaries.

## Native/WASM parity

Where capabilities overlap, native and WASM must preserve equivalent Stable semantic observations for the same relevant semantic base/context and deterministic configuration, including Semantic API operation meaning, operation gates, ADR-0019 diagnostics, ADR-0018 formula facts, and ADR-0020 atomicity.

The concrete transport bytes, memory layout, request batching, Worker placement, and host error wrappers may differ.

PR #91 is executable evidence that this topology is viable and that the shared production semantic/application path can produce matching native/WASM observations. Its JSON DTOs and raw WASM ABI are deliberately non-authoritative.

## Host boundary

Filesystem, IndexedDB/browser persistence, dialogs, credentials, Git/process integration, network access, Tauri commands, and durable replacement/recovery remain host/composition concerns outside `workspace-engine`.

A runtime/host may retain, cache, serialize, project, or deliver the ADR-0020 Semantic API. It may not redefine its semantic behavior.

## Performance evidence

The PR #91 benchmark shows topology pressure: resident Rust ownership avoids O(document)-sized request/result traffic for ordinary interaction and exposes remaining exercised whole-document cost inside Rust-side work.

It is not a browser/device latency guarantee, memory budget, throughput SLA, Worker startup requirement, or mandate for a specific incremental algorithm.

## Deferred mechanics

ADR-0022 does not freeze:

- browser Worker lifecycle/loading/startup/memory behavior;
- resident session/handle representation;
- revision/concurrency/conflict algorithms;
- cancellation;
- exact state commit/swap/locking/cloning mechanics;
- projection patch/invalidation protocol;
- IPC/FFI/network mapping;
- WASM ABI or JS DTO spelling;
- browser/native persistence and recovery; or
- multi-document/branch/history residency.

Issue `#93` implements the current internal resident session/revision mechanics,
and Issue `#94` adds revision-pinned selective projections plus fresh
dependency-derived invalidation facts with production native/WASM semantic
parity evidence. Issue `#95` retains incremental performance work. Public
Worker/ABI/persistence mechanics may evolve while preserving
ADR-0020/ADR-0022 semantic and runtime-ownership laws.

## Related

- ADR-0016
- ADR-0018
- ADR-0019
- ADR-0020
- ADR-0022
- Issue #26
- Issues #93, #94, #95
- PR #91
