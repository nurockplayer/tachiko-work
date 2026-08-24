# Frontend and Backend Boundary

Decision state: Accepted direction. ADR-0020 now makes the Headless Semantic API
the mandatory first-party semantic product boundary. Detailed runtime/state and
transport mechanics remain #26.

## Principle

The UI is a projection layer, not the owner of document meaning.

A frontend may own selection, viewport, interaction state, draft authoring
buffers, presentation caches, and user workflow state. It must not create a
second canonical semantic model or reimplement semantic validation/formula/
mutation policy.

## Architecture

In this document, `Rust Core` means the shared Rust semantic/application
runtime, not the `semantic-core` crate alone. The Accepted crate ownership and
dependency direction are recorded in
[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md).
The first-class client contract is defined by
[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md) and
[`semantic-api.md`](../specs/semantic-api.md).
Detailed resident-state, native/WASM host, revision/concurrency, and bridge
behavior remains owned by #26.

```text
React / Desktop / Web / future Mobile UI
        |
        | first-party semantic client
        v
First-class Semantic API
        |
        v
Shared Rust semantic/application runtime
        |
        v
Semantic model + focused engines
```

The same rule applies even when the physical call is an in-process Rust call, a
WASM invocation, IPC, FFI, or a future network request. Transport does not grant
permission to bypass semantic behavior.

## Frontend Responsibilities

- rendering;
- interaction and accessibility;
- selection, focus, viewport, drag/drop, and other presentation state;
- raw/draft authoring buffers where incomplete input is not yet semantic state;
- user workflows and review presentation;
- projecting stable semantic identities into current human-readable labels,
  paths, ranges, or widgets; and
- mapping Semantic API results into UI state without redefining their semantic
  meaning.

## Shared Semantic/Application Responsibilities

- authoritative semantic document state;
- stable semantic identity and typed relationships;
- calculations and formula meaning;
- semantic validation and authoritative operation gates;
- typed semantic commands and queries;
- Propose/Execute behavior;
- semantic comparison/merge orchestration;
- all-or-nothing semantic publication for commands/batches; and
- presentation-neutral semantic results/diagnostics.

Persistence transformation remains composed through explicit storage/host
boundaries rather than being owned by the UI or workspace-engine.

## Client rule

GUI/Web/mobile clients MUST use the Semantic API for product-semantic reads,
validation/explanation, proposals, and execution.

A frontend MUST NOT:

- mutate internal `Document` fields as its durable edit protocol;
- target storage paths, JSON pointers, row/cell coordinates, or Rust field layout
  as semantic identity;
- derive operation permission from diagnostic severity/message rather than the
  authoritative gate; or
- implement a host-specific version of formula, validation, mutation, diff, or
  merge semantics.

## Why

A single semantic authority and first-class Semantic API allow:

- web application;
- desktop application;
- mobile clients;
- AI agents;
- CLI/automation; and
- future integrations

to share the same meaning while using different presentations and transports.

This is the boundary #26 must host/map rather than redesign.
