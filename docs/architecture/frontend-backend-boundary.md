# Frontend and Backend Boundary

Decision state: Accepted direction. ADR-0020 makes the Headless Semantic API the mandatory first-party semantic product boundary. ADR-0024 defines immutable revision-pinned SemanticPatch proposal meaning. ADR-0026 defines trusted footprint derivation, scoped authorization, exact Human Approval, and semantic/external-effect separation. ADR-0022 accepts the resident shared Rust semantic/application runtime and host separation as the preferred interactive topology. Concrete authorization/proposal/revision/session/transport mechanics remain Deferred to #29/#30/#93 and later runtime work.

## Principle

The UI is a projection layer, not the owner of document meaning.

A frontend may own selection, viewport, interaction state, draft authoring buffers, presentation caches, and user workflow state. It must not create a second canonical semantic model or reimplement semantic validation/formula/mutation policy.

For an open interactive document, authoritative in-memory semantic state belongs to the shared Rust semantic/application runtime under ADR-0022. This does not replace ADR-0003/ADR-0017 durable representation authority; `.roproj/v1` is the canonical durable editable materialization, and Issue #123 implements its production pure codec plus native exact-tree materialize, canonical-only validate, and explicit bounded canonicalize workflow.

## Architecture

In this document, `Rust Runtime` means the shared Rust semantic/application runtime built around `workspace-engine` and the lower semantic engines, not the `semantic-core` crate alone.

The Accepted crate ownership and dependency direction are recorded in [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md). The first-class client contract is defined by [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md), the revision-pinned proposal contract by [ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md), and both are specified by [`semantic-api.md`](../specs/semantic-api.md). Authorization and Approval meaning are defined by [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md) and [`semantic-authorization.md`](../specs/semantic-authorization.md). Runtime ownership and host separation are defined by [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).

```text
React / Desktop / Web / future Mobile UI
        |
        | first-party Semantic API client
        v
First-class Semantic API
        |
        v
Trusted authorization/application boundary
        |
        v
Resident shared Rust semantic/application runtime
        |
        v
Semantic model + focused engines

Host / composition boundary beside the runtime:
filesystem / IndexedDB / dialogs / credentials / Git / process / network / persistence
```

The same semantic rule applies whether the physical call is an in-process Rust call, WASM invocation, IPC, FFI, or future network request. Transport does not grant permission to bypass semantic behavior.

## Frontend Responsibilities

A frontend may own:

- rendering and accessibility;
- selection, focus, viewport, panels, drag/drop, and other presentation state;
- revision-keyed semantic projections and query caches;
- pending-command, immutable proposal projection, preview, and review UI state;
- raw/draft authoring buffers where incomplete input is not yet semantic state under ADR-0019;
- presentation-local optimistic state that cannot redefine the authoritative semantic outcome; and
- projecting stable semantic identities into current human-readable labels, paths, ranges, or widgets.

Projection caches and optimistic UI state are disposable/derived from the standpoint of semantic authority. They do not become canonical merely because they render ahead of a confirmed runtime result.

## Shared Rust Runtime Responsibilities

The shared semantic/application runtime owns:

- authoritative interactive semantic document state;
- stable semantic identity and typed relationships;
- calculations and formula meaning;
- semantic validation and authoritative operation gates;
- ADR-0020 typed semantic Commands and Queries;
- Propose/Execute semantic behavior and ADR-0024 SemanticPatch binding;
- trusted derivation of ADR-0026 operation-family/disclosure-scope and
  associated operation-family/mutation-class/write-scope requirements;
- live scoped-Grant and exact-Approval checks at a trusted boundary that is not
  only UI or `ai-api` convention;
- semantic comparison/merge orchestration;
- all-or-nothing semantic publication for commands/batches; and
- presentation-neutral semantic results/diagnostics.

ADR-0022 prefers retaining this runtime across ordinary interactive operations instead of serializing/reconstructing the complete semantic document for each edit/query.

When SemanticPatch is implemented, the trusted boundary MUST enforce base
equality, same-ID immutability, rederived authorization footprints, live Grant
coverage, and exact Human Approval for Delegated-origin or Delegated-authority
publication. Approval is consumed only atomically with successful semantic
publication. Exact crate
placement, authorization/Approval DTOs and state, proposal/revision types,
resident session handle, concurrency algorithm, state commit/swap mechanism,
and projection/redaction delivery remain #29/#30/#93/#94 work.

## Snapshot boundaries

Full semantic snapshots are appropriate at explicit boundaries such as load/open, durable save/materialization, import/export, recovery/debug capture, and explicit branch/document exchange.

They are not the preferred normal per-edit client/runtime transport.

This does not prohibit internal cloning, full validation, or other whole-document implementation work inside Rust when required for correctness.

## Host Responsibilities

Persistence transformation and host effects remain composed outside `workspace-engine`.

Host/composition layers may own:

- filesystem and path access;
- IndexedDB/browser persistence;
- dialogs and OS/browser integration;
- credentials and authorization adapters;
- Git/process/network integration;
- Tauri host commands; and
- durable write/recovery mechanics.

Semantic publication, durable persistence, and external publication are distinct effects under ADR-0007/ADR-0022. Host authority does not redefine semantic meaning, and semantic edit authority does not implicitly grant filesystem/network/Git/deployment authority.

## Client rule

GUI/Web/mobile clients MUST use the Semantic API for product-semantic reads, validation/explanation, proposals, and execution.

A frontend MUST NOT:

- maintain an independently authoritative semantic `Document` as its edit model;
- mutate internal Rust `Document` fields as its durable edit protocol;
- target storage paths, JSON pointers, row/cell coordinates, or Rust field layout as semantic identity;
- mutate proposal contents under the same proposal identity or silently rebase
  a stale proposal;
- authoritatively declare its own disclosure/write footprint, Principal class,
  Grant coverage, or Approval state;
- reveal preview/diff/diagnostic evidence beyond live Query scope;
- derive operation permission from diagnostic severity/message rather than the authoritative gate/authorization boundary; or
- implement a host-specific version of formula, validation, mutation, diff, merge, or atomicity semantics.

## Native/WASM parity

Native and Web/WASM clients may use different host/transport implementations, but where capabilities overlap they must preserve the same Stable Semantic API meaning, operation gate decisions, diagnostics/formula facts, and atomicity for the same relevant semantic base/context and deterministic configuration.

A Worker, bridge, or transport may host, retain, cache, serialize, batch-deliver, or project Semantic API behavior. It may not redefine that behavior.

## Implementation status

The current workspace-engine operation surface remains substantially
snapshot-style and has no general SemanticPatch or AtomicBatch implementation.
ADR-0022/ADR-0024/ADR-0026 make the resident, proposal, and authorization laws
Accepted targets while allowing implementation to lag until #29/#30/#93–#95.

Separately, #123 implements `.roproj/v1` at the storage/native host boundary
without moving filesystem authority into workspace-engine or the interactive
runtime. #3 implements packaged `.ro` codec/pack/unpack/compare at that same
boundary, including atomic no-replace destination publication. Optional Git/CI
composition is implemented by #44 at the CLI/repository edge without moving Git
into workspace-engine; broader hostile source/path handling, full durability/
recovery, browser persistence, and host mechanisms remain Deferred.

No Web UI, resident session API, projection patch protocol, or browser persistence mechanism is introduced by this documentation decision.

## Why

A single semantic authority, first-class Semantic API, and resident shared runtime allow web, desktop, mobile, AI, CLI/automation, and future integrations to share one meaning while using different presentation and host mechanisms.

This avoids both expensive whole-document client/runtime traffic as the default topology and the more serious architectural failure of letting a client-side mirror become a second semantic source of truth.

## Related

- ADR-0016
- ADR-0019
- ADR-0020
- ADR-0022
- ADR-0024
- ADR-0026
- Issues #3, #26, #28, #29, #30, #44, #93, #94, #95, #123
- PR #91
