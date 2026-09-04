# Tachiko Work Architecture

> This is an explanatory and navigation document. It does **not** independently
> establish normative product or architecture contracts. When it conflicts with
> higher-authority material, the existing authority hierarchy wins: the Product
> Constitution, Accepted ADRs and governance policies, and normative
> specifications take precedence. Start with the
> [Knowledge Authority policy](docs/governance/knowledge-authority.md) and its
> [canonical reconciliation register](docs/governance/canonical-reconciliation-register.md)
> when the maturity or authority of a statement matters.

## The system in one minute

Tachiko Work is a semantic workspace for structured data and computation. Its
core meaning is expressed through typed schemas, entities, references, formulas,
validation, and semantic changes. Files, cells, paths, and UI state are
representations or projections; they are not automatically the source of that
meaning.

The high-level flow is:

```text
Human / AI / CLI / graphical clients
                │
                ▼
        transport-neutral Semantic API
                │
                ▼
   trusted application and authorization boundary
                │
                ▼
 resident shared Rust semantic/application runtime
                │
                ▼
 semantic model · formulas · validation · diff/merge
                │
                ▼
 current semantic state · complete snapshots · projections
                │
       explicit snapshot/materialization boundary
                │
                ▼
 host and composition concerns: filesystem, browser persistence,
 Git, process, credentials, network, and other external effects
                │
                ▼
 canonical .roproj/v1 · portable .ro · optional Git/CI workflows
```

The [Semantic API](docs/specs/semantic-api.md) is the shared semantic boundary
for first-party clients. The resident runtime owns interactive semantic state;
frontends own presentation and workflow state. Native and Web/WASM hosts may
use different transport or host mechanisms, but overlapping capabilities must
preserve the same semantic meaning. The [frontend/backend boundary](docs/architecture/frontend-backend-boundary.md)
defines these ownership rules in detail.

Tachiko is spreadsheet-first for the current human workflow, but not
spreadsheet-shaped at its semantic core. The
[PostgreSQL-like engine / spreadsheet-first explanation](docs/architecture/postgresql-like-engine-spreadsheet-interface.md)
is an explanatory founder framing, not a new authority. Likewise, the
[product-surface overview](docs/architecture/README.md) illustrates independent
frontends sharing semantic authority; its Project, CRM, Inventory, and Finance
surfaces are not claims that those products are implemented. A shared frontend
toolkit remains optional, not a platform requirement.

## Core invariants

These are short pointers to established authority, not a replacement for the
underlying contracts:

- **Meaning outranks representation.** A file format, cell coordinate, path,
  label, or renderer must not silently become semantic identity. See the
  [Product Constitution](docs/vision/product-constitution.md),
  [ADR-0001](docs/decisions/ADR-0001-semantic-platform-not-office-clone.md),
  and [ADR-0015](docs/decisions/ADR-0015-stable-semantic-identity.md).
- **One semantic authority, many clients.** CLI, AI, graphical clients, and
  integrations use the Semantic API where they perform semantic work. A
  frontend must not create a second authoritative document model or duplicate
  Rust formula, validation, mutation, diff, or merge policy. See
  [ADR-0020](docs/decisions/ADR-0020-first-class-headless-semantic-api.md) and
  [ADR-0022](docs/decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).
- **Views are projections over shared meaning.** Spreadsheet, document,
  dashboard, and future product views can differ in architecture, visual
  language, and interaction model without owning separate semantic truth. See
  [Design Principles](docs/vision/design-principles.md) and the
  [frontend/backend boundary](docs/architecture/frontend-backend-boundary.md).
- **Equivalent clients preserve semantic behavior.** Native and Web/WASM are
  execution targets for the same Rust-owned semantic behavior where their
  capabilities overlap; a host or transport cannot bypass semantic gates. See
  [WASM strategy](docs/architecture/wasm-strategy.md).
- **Storage and host effects stay outside semantic authority.** `.roproj/v1`,
  portable `.ro`, Git, browser persistence, credentials, and network/process
  effects are explicit boundaries. Semantic publication does not implicitly
  authorize a filesystem or external effect. See
  [.ro and .roproj architecture](docs/architecture/ro-and-roproj-format.md)
  and [Git-native workflow](docs/architecture/git-native-workflow.md).
- **Keep the stable core small.** Generalize from real pressure and keep
  replaceable mechanisms behind boundaries. The current product is not a
  completed spreadsheet UI, Office/OOXML implementation, realtime collaboration
  system, or cloud service. The amended [ADR-0027](docs/decisions/ADR-0027-open-format-and-interoperability-policy.md)
  makes established spreadsheet workflows, especially Excel, a first-class
  interoperability target without making Excel/OOXML semantic authority.
  Historical Tachiko implementation choices do not receive permanent
  compatibility protection merely because they already exist; when they
  obstruct an Accepted interoperability requirement, prefer an explicit
  migration or supersession path while protecting user data and durable
  external contracts.

## Major subsystem map

| Area | Boundary and current reading | Read next |
| --- | --- | --- |
| Semantic model and core | Typed meaning, stable identity, references, and progressive strengthening belong to the semantic foundation. | [Semantic core rationale](docs/architecture/semantic-core-rationale.md), [document model](docs/architecture/document-model.md), [semantic data model](docs/specs/semantic-data-model.md), [ADR-0021](docs/decisions/ADR-0021-progressive-semantic-strengthening.md) |
| Semantic API and resident runtime | `workspace-engine` and the lower Rust engines provide the shared application authority. The transport-neutral API and resident topology are Accepted; current Rust source, session, and transport shapes remain replaceable where stated. | [Rust crate architecture](docs/architecture/rust-crate-architecture.md), [Semantic API specification](docs/specs/semantic-api.md), [ADR-0020](docs/decisions/ADR-0020-first-class-headless-semantic-api.md), [ADR-0022](docs/decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md) |
| Formulas and validation | Formula meaning, finite deterministic calculation, staged validation, diagnostics, and operation gates are semantic/runtime responsibilities rather than frontend conventions. | [Formula engine specification](docs/specs/formula-engine-spec.md), [validation engine](docs/specs/validation-engine.md), [diagnostics contract](docs/specs/diagnostics-contract.md), [ADR-0018](docs/decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md), [ADR-0019](docs/decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md) |
| Persistence and formats | `.roproj/v1` is the canonical editable representation. The implemented `direct-ro/v2` path is the current direct JSON writer, while portable-package/v1 is a derived single-file `.ro` artifact; legacy direct `.ro` v1 input is an explicit migration path. Storage codecs and host publication do not redefine semantic meaning. | [.ro and .roproj architecture](docs/architecture/ro-and-roproj-format.md), [`.roproj/v1` specification](docs/specs/roproj-format.md), [portable package specification](docs/specs/portable-package-v1.md), [ADR-0003](docs/decisions/ADR-0003-ro-and-roproj-representation.md), [ADR-0023](docs/decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md), [ADR-0025](docs/decisions/ADR-0025-portable-package-v1.md) |
| Git-native workflow | Git is an optional storage and collaboration protocol for reviewable semantic work, not the semantic model or the end-user UI. Semantic delta and conflict evidence remain distinct from raw text merging. | [Git-native workflow](docs/architecture/git-native-workflow.md), [semantic delta](docs/specs/semantic-diff-spec.md), [conflict resolution](docs/specs/conflict-resolution.md), [ADR-0030](docs/decisions/ADR-0030-canonical-semantic-delta.md), [ADR-0031](docs/decisions/ADR-0031-semantic-merge-conflict-protocol.md) |
| Designer, frontend, and host | Web, desktop, and future mobile surfaces are projections and host compositions over the Semantic API. The current Designer slice is implementation evidence, not a second semantic authority or a general frontend contract. | [Frontend/backend boundary](docs/architecture/frontend-backend-boundary.md), [WASM strategy](docs/architecture/wasm-strategy.md), [architecture index](docs/architecture/README.md), [ADR-0022](docs/decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md) |
| AI | AI is a delegated semantic client. It queries and proposes typed operations through the same semantic boundary; approval, authorization, validation, and external effects remain separate gates. | [AI-native architecture](docs/architecture/ai-native-architecture.md), [AI agent API](docs/specs/ai-agent-api.md), [semantic authorization](docs/specs/semantic-authorization.md), [ADR-0007](docs/decisions/ADR-0007-ai-semantic-interaction-model.md), [ADR-0024](docs/decisions/ADR-0024-revision-pinned-semantic-patch.md), [ADR-0026](docs/decisions/ADR-0026-scoped-semantic-authorization-and-approval.md) |
| Collaboration and team workspace | Semantic merge, current-state/history boundaries, team-policy recovery, and causality boundaries are Accepted in their named ADRs. Realtime transport, clocks, collaboration DTOs, and broad runtime mechanics remain Deferred. | [Distributed collaboration](docs/architecture/distributed-collaboration.md), [collaboration model](docs/specs/collaboration-model.md), [ADR-0029](docs/decisions/ADR-0029-current-state-authority-and-optional-history.md), [ADR-0034](docs/decisions/ADR-0034-team-workspace-policy-and-recovery-boundary.md), [ADR-0035](docs/decisions/ADR-0035-collaboration-causality-and-selective-convergence-boundary.md) |
| Presentation and rendering | Rendering is a future projection hypothesis. Renderer-specific layout, typography, animation, and host capabilities must not be promoted into the semantic core without further authority. | [Rendering system direction](docs/architecture/rendering-system.md) (Hypothesis; #67) |

## Stability and maturity cues

Decision state and implementation state are separate. A behavior can be
implemented but still Provisional, and an Accepted direction can remain
unimplemented. Use the [Knowledge Authority policy](docs/governance/knowledge-authority.md)
and [reconciliation register](docs/governance/canonical-reconciliation-register.md)
to classify a boundary before relying on it.

| State | How to read it |
| --- | --- |
| Principle | A durable product constraint; it guides lower-level decisions without necessarily selecting an implementation. |
| Accepted | An adopted decision or policy; it remains authoritative until explicitly amended or superseded. |
| Provisional | A useful, reversible current choice; do not infer a permanent public contract from it. |
| Hypothesis | A promising direction that still needs research, evidence, or validation; it may guide investigation but must not silently become an implementation invariant. |
| Open Question | A known unresolved decision that requires focused decision work; implementation convenience must not answer it when the choice would create a durable contract. |
| Deferred | Work or mechanics intentionally left for later; its existence does not authorize implementation or promote it to current architecture. |
| Superseded | Historical context retained for traceability; follow the linked replacement or current authority. |

The detailed [architecture index](docs/architecture/README.md) also separates
Accepted architecture from implementation evidence. In particular, the
collaboration documents preserve Accepted semantic boundaries while marking
concrete realtime mechanics Deferred, and the rendering document remains a
Hypothesis. Do not flatten those distinctions into a single stable architecture
picture.

## Change-routing guide

When planning a change, use the smallest relevant route before exploring the
rest of the repository:

| If you are changing... | Read this next |
| --- | --- |
| A schema, entity, reference, or semantic identity rule | [Semantic data model](docs/specs/semantic-data-model.md), [ADR-0015](docs/decisions/ADR-0015-stable-semantic-identity.md), and the [semantic core rationale](docs/architecture/semantic-core-rationale.md) |
| A formula, calculation, validation, or diagnostic | [Formula engine](docs/specs/formula-engine-spec.md), [validation engine](docs/specs/validation-engine.md), [diagnostics contract](docs/specs/diagnostics-contract.md), and ADR-0018/ADR-0019 |
| A client, UI, runtime, WASM, or host boundary | [Semantic API](docs/specs/semantic-api.md), [frontend/backend boundary](docs/architecture/frontend-backend-boundary.md), and ADR-0020/ADR-0022 |
| `.ro`, `.roproj`, migration, or persistence | [.ro/.roproj architecture](docs/architecture/ro-and-roproj-format.md), [storage versioning](docs/specs/storage-versioning-and-migration.md), and ADR-0003/ADR-0017/ADR-0023/ADR-0025 |
| Git review, semantic delta, merge, history, or checkpoints | [Git-native workflow](docs/architecture/git-native-workflow.md), [conflict resolution](docs/specs/conflict-resolution.md), and ADR-0029/ADR-0030/ADR-0031/ADR-0033 |
| AI proposals, approval, authorization, or external effects | [AI-native architecture](docs/architecture/ai-native-architecture.md), [semantic authorization](docs/specs/semantic-authorization.md), and ADR-0007/ADR-0024/ADR-0026 |
| Excel, Office, or another legacy format boundary | [ADR-0027](docs/decisions/ADR-0027-open-format-and-interoperability-policy.md), [migration framework](docs/specs/migration-framework.md), and the relevant implementation Issue; compatibility mappings remain separately scoped |
| Team collaboration, recovery, causality, or convergence | [Distributed collaboration](docs/architecture/distributed-collaboration.md), [collaboration model](docs/specs/collaboration-model.md), and ADR-0034/ADR-0035 |
| Rendering, presentation, or a new visual projection | [Rendering system direction](docs/architecture/rendering-system.md), then check its Hypothesis/Open Question status before introducing a contract |
| Architecture documentation or a possible authority conflict | [Knowledge Authority](docs/governance/knowledge-authority.md), the [reconciliation register](docs/governance/canonical-reconciliation-register.md), and the [architecture index](docs/architecture/README.md) |

## How the documentation fits together

```text
README.md                 → what Tachiko Work is / how to start
ARCHITECTURE.md           → top-level system mental model and navigation
docs/architecture/*       → subsystem architecture explanations
docs/decisions/*          → why durable decisions were made
docs/specs/*              → precise implementable contracts
docs/governance/*         → authority and delivery rules
```

The [knowledge-base front door](docs/README.md) gives the complete reading
order, authority hierarchy, ADR index, specification map, and governance entry
points. This document is a fast route into that material; it does not replace
the front door, the [architecture index](docs/architecture/README.md), or any
canonical ADR or specification.
