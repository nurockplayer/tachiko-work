# ADR-0038: Tachikore semantic/application engine name

## Status

Accepted

Decision issue: [#405](https://github.com/nurockplayer/tachiko-work/issues/405)

## Context

Tachiko Work now has a clear architectural separation between semantic meaning,
domain engines, the shared application/runtime boundary, and client/host
projections. ADR-0016 fixes the current Rust crate layering, ADR-0020 makes the
transport-neutral Semantic API the first-class product boundary, and ADR-0022
places interactive authoritative state in the resident shared Rust
semantic/application runtime.

Those internal names are intentionally precise, but there has not been a stable
human-facing name for the composed engine beneath Tachiko clients. Calling the
whole system `semantic-core` would be inaccurate because that crate owns only the
lowest semantic-model layer. Calling it `workspace-engine` would also be too
narrow because the composed engine includes lower semantic, formula, diff, and
merge behavior that the application/runtime boundary orchestrates.

Renaming the repository to `tachiko-core` would weaken the existing **Tachiko
Work** product identity, while introducing a new facade crate only for naming
would amend the Accepted ADR-0016 graph without concrete SDK, dependency, or
lifecycle pressure.

## Decision

### 1. Adopt the name **Tachikore**

The naming hierarchy is:

- **Tachiko** — umbrella brand.
- **Tachiko Work** — the semantic work platform/product vision and this
  repository. The repository remains `tachiko-work`.
- **Tachikore** — the composed shared semantic/application engine and resident
  runtime beneath first-party semantic clients.
- **Tachiko Sheet** — the spreadsheet client/product. Its repository remains
  `tachiko-sheet`.

The name combines the Tachiko identity with “core”, while remaining distinct
from the implementation-specific `semantic-core` crate name.

### 2. Tachikore names one existing authority, not a new layer

Tachikore is an architecture/product name for the existing Rust-owned semantic
and application authority. It does not introduce a second semantic model,
second orchestration layer, new runtime, or alternate API.

In the current implementation:

- `semantic-core` owns typed semantic meaning and intrinsic invariants;
- `formula-engine`, `diff-engine`, and `merge-engine` own focused deterministic
  semantic behavior;
- `workspace-engine` composes the shared application behavior and resident
  semantic runtime exposed through the Semantic API;
- storage, CLI, browser/native hosts, network transports, and other external
  effects remain explicit boundaries rather than becoming semantic authority.

When documentation says a client is “powered by Tachikore”, it means the client
uses this composed semantic/application authority rather than implementing a
parallel document model or business-rule engine.

### 3. Preserve the current Rust crate DAG

This naming decision does **not** rename `semantic-core` to `tachikore`, does not
rename `workspace-engine`, and does not add a ninth facade/orchestration crate.
ADR-0016 remains authoritative for the current eight-crate workspace and direct
dependency graph.

This distinction is deliberate: `semantic-core` is only one layer inside
Tachikore, and a facade crate without demonstrated public-SDK pressure would add
indirection without a new ownership boundary.

### 4. Reserve `tachikore` for a future deliberate SDK/facade if justified

If Tachiko later exposes a deliberately versioned public Rust SDK or facade over
the semantic/application engine, `tachikore` is the preferred package/crate
name. Doing so still requires separate evidence and an explicit Accepted
amendment to ADR-0016 (and any API/versioning authority that applies at that
time).

This ADR does not itself stabilize current Rust `pub` items, serde layouts,
transport DTOs, ABI shapes, or package names as public compatibility contracts.
ADR-0020 and ADR-0022 continue to govern those boundaries.

## Consequences

### Positive

- Product language can distinguish the platform (**Tachiko Work**) from the
  engine (**Tachikore**) and client products such as **Tachiko Sheet**.
- Documentation can talk about one semantic/application engine without leaking
  internal crate names into product architecture.
- `semantic-core` keeps its precise low-level meaning and `workspace-engine`
  keeps its precise application/runtime meaning.
- The existing crate graph, CI dependency checks, and implementation boundaries
  remain unchanged.
- A future public SDK has a natural name without prematurely creating that SDK.

### Trade-offs

- Tachikore is initially a conceptual/composed engine name rather than a
  one-to-one Cargo package name.
- Contributors must not treat the name as permission to collapse existing
  boundaries or add an umbrella crate without new architectural evidence.

## Relationship to prior decisions

- **ADR-0016** remains authoritative for Rust crate ownership and the current
  dependency DAG.
- **ADR-0020** remains authoritative for the first-class Headless Semantic API
  boundary.
- **ADR-0022** remains authoritative for resident semantic runtime ownership and
  host separation.

This ADR names the composed engine described by those decisions; it does not
supersede or weaken them.
