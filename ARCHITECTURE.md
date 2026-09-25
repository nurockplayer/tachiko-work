# Tachiko Work: engineering map

A compact orientation, not a new architecture decision or dispatch queue.
Follow the linked authority before changing a contract; start with
[AGENTS](AGENTS.md) and [CONTRIBUTING](CONTRIBUTING.md) before editing.

## 1. What you are looking at

**Tachiko Work** is the platform/repository. **Tachikore** names its composed Rust
semantic/application engine and resident runtime, not a ninth crate or released
SDK. **[Tachiko Sheet](https://github.com/nurockplayer/tachiko-sheet)** owns the
first-party spreadsheet UX. The legacy Work Designer product UI is retired;
[the browser-client package](packages/browser-client/README.md) remains an
experimental producer and qualification boundary.

The [Constitution](docs/vision/product-constitution.md) and
[design principles](docs/vision/design-principles.md) seek portable, user-owned
work with shared meaning across tools. Game balance is the first proving ground;
Office interoperability is a boundary requirement, not the core's ontology.
Progressive strengthening is an accepted direction, not permission to invent a
universal schema, database, or application platform.

## 2. The mental model

A **Document** contains schemas, fields, entities, typed values, references, and
bound formulas. Opaque stable IDs identify meaning; human keys, display names,
paths, and cell positions do not. Formulas bind references to those IDs, not
screen coordinates. Calculated values, indexes, and projections are derived.

Each interactive document occurrence has one authoritative shared Rust runtime
owner. Clients request queries or changes and render projections; they do not run a
second authoritative evaluator. Native and WASM must agree on accepted portable
semantics. Current state and complete snapshots are authoritative; optional
history and Git do not redefine them.

## 3. Find the owning code

These are the eight workspace crates; names link to their source entry points.
The [crate architecture](docs/architecture/rust-crate-architecture.md) gives the
full dependency DAG and forbidden edges.

| Source | Owns |
| --- | --- |
| [semantic-core](crates/semantic-core/src/lib.rs) | IDs, document/schema/value model, references, bound expressions, intrinsic validation |
| [formula-engine](crates/formula-engine/src/lib.rs) | Parsing/binding, deterministic calculation, dependencies and retained derived state |
| [diff-engine](crates/diff-engine/src/lib.rs) | Semantic comparison, direct-state delta and derived impact |
| [merge-engine](crates/merge-engine/src/lib.rs) | Three-way reconciliation and typed conflicts; candidates still require final validation |
| [workspace-engine](crates/workspace-engine/src/lib.rs) | Shared Semantic API application behavior, validation/calculation composition and operation gates |
| [storage](crates/storage/src/lib.rs) | Version-owned codecs, explicit migrations and native file publication; a sibling of workspace-engine |
| [ai-api](crates/ai-api/src/lib.rs) | Provider-neutral semantic queries and hostile-client security composition over workspace-engine |
| [cli](crates/cli/src/main.rs) | Host composition: load, invoke shared operations, present results, explicitly write outputs |

Within workspace-engine, start with [resident_session](crates/workspace-engine/src/resident_session.rs)
for occurrence/revision ownership, [patch_lifecycle](crates/workspace-engine/src/patch_lifecycle.rs)
for proposal/authorization/publication, and [analysis_operations](crates/workspace-engine/src/analysis_operations.rs)
for bounded queries. Saved keyed grouped sums have their own
[keyed_grouped_sum_operations](crates/workspace-engine/src/keyed_grouped_sum_operations.rs),
not a general formula-language extension.

The separate [browser runtime adapter](packages/browser-client/runtime/src/lib.rs)
composes storage and the same workspace engine. Its `Designer` symbols, Worker
DTOs and WASM ABI are implementation/compatibility details, not a restored UI or
stable public SDK. Import/export mappings stay in its adapter boundary.

## 4. Follow a change across the boundaries

A host admits versioned input through storage and supplies trusted context. The
shared application evaluates a proposed `Command` or ordered `AtomicBatch`
against its exact semantic base. Validation and authorization are separate gates;
delegated publication also needs the exact required Human Approval. Successful
publication installs semantic state atomically; stale, invalid or unauthorized
attempts do not become canonical state. Proposing is not publishing.

The host then handles explicit persistence and external effects. **Semantic
publication is not a filesystem/cloud transaction.** Keep paths, credentials,
network access and persistence outside workspace-engine. CLI new-output commands
refuse overwrites; do not generalize that into a durability guarantee for every
host. In ADR-0039's selected desktop approval profile, native owns the entire
occurrence, including ordinary Human operations—not just delegated calls.

Canonical detail: [Semantic API](docs/specs/semantic-api.md),
[authorization](docs/specs/semantic-authorization.md),
[runtime/host topology](docs/decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md),
[desktop ownership](docs/decisions/ADR-0039-native-trusted-host-delegated-human-approval.md).

## 5. Do not collapse the format names

| Representation | Meaning and implementation boundary |
| --- | --- |
| Direct `.ro` JSON | Legacy v1 compatibility input; current v2 writer. Not the portable ZIP container. |
| `.roproj` tree | Versioned editable source. v1 is frozen; v2 adds the bounded saved grouped-sum definition; v3 adds two closed durable field constraints. Storage v3 codecs/migration exist; that alone does not qualify client save/reopen. |
| Portable `.ro` package v1 | Derived deterministic ZIP32 envelope over exactly `.roproj/v1`. Not a universal latest-version container or live incremental database. |

Use the [format guide](docs/architecture/ro-and-roproj-format.md) and
[versioned specifications](docs/specs/README.md), not file extensions alone.
`TWDPROJ2` in the adapter is an app-host envelope, not `.roproj/v2`.
The inspected browser adapter uses v1/v2 codecs; native v3 selection/conversion
and consumer qualification remain with
[#452](https://github.com/nurockplayer/tachiko-work/issues/452).

## 6. Authority, evidence and next action

[Knowledge authority](docs/governance/knowledge-authority.md) orders principles,
Accepted decisions/policies, applicable normative specifications, explanatory
docs, implementation evidence, and open proposals/research. **Implemented is not
automatically Accepted; Accepted is not automatically shipped.** Internal Rust
`pub` items and serialized DTOs are not automatically stable external contracts.

Use [docs by task](docs/README.md) to load only the relevant canonical detail.
[The dated orientation audit](docs/engineering/repository-orientation-audit.md)
records stale annotations and unresolved boundaries. Read the live
[#374 handoff](https://github.com/nurockplayer/tachiko-work/issues/374) and owning
Issue/PR before choosing work or a producer pin. A roadmap horizon or closed
implementation issue is not readiness for another client.

For evidence, start with [the fixture journey](examples/game-balance/README.md),
[executable smoke](scripts/first-user-smoke.sh), and
[CI](.github/workflows/ci.yml). Contribution guidance owns exact setup, docs,
Rust/MSRV, native/WASM, browser-consumer and release checks. This map grants no
exception to scope, independent review or merge gates.
