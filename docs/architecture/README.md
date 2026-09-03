# Tachiko Work Architecture

Architecture documents explain the current model, implemented baseline, target direction, and working hypotheses. They do not all carry the same decision state.

Before treating an architecture detail as permanent, read [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and the [`canonical reconciliation register`](../governance/canonical-reconciliation-register.md).

## Architecture diagrams

![Detailed architecture diagram: React frontend, Tauri desktop shell, Rust semantic backend, formats, and integrations.](tachiko-work-architecture-detailed.png)

The diagrams show product architecture direction. Internal crate boundaries, runtime seams, and semantic-model details may evolve where the repository classifies them as Provisional or Open Questions.

For repository-derived, revision-pinned architecture maps and Architecture Delta use, read [`derived-architecture-maps.md`](derived-architecture-maps.md). Those maps are derived review/documentation evidence, not a replacement for Accepted ADR/spec authority.

## Read by subsystem

### Semantic core and Semantic API

- [`semantic-core-rationale.md`](semantic-core-rationale.md) — why the project is semantic-first; explanatory rationale, not a replacement for ADR authority.
- [`postgresql-like-engine-spreadsheet-interface.md`](postgresql-like-engine-spreadsheet-interface.md): explanatory founder framing that Tachiko is an extensible semantic work engine whose default human interface is spreadsheet-first; the spreadsheet is a client/projection, not semantic authority. Tracking strategy: #234.
- [`document-model.md`](document-model.md) — accepted semantic-document direction under ADR-0021 progressive strengthening; concrete future mixed-content graph mechanics remain Deferred.
- [`unified-semantic-model.md`](unified-semantic-model.md) — accepted unified-semantic direction across structured data, formulas, views, and AI operations.
- [`rust-crate-architecture.md`](rust-crate-architecture.md) — current implemented crate graph, ADR-0016 ownership, ADR-0020 first-class Semantic API mapping, ADR-0024 proposal ownership, ADR-0026 authorization boundary, and ADR-0022 runtime-host ownership without stabilizing the current Rust source surface.
- [`frontend-backend-boundary.md`](frontend-backend-boundary.md) — GUI/Web/mobile as projection layers over the mandatory Semantic API, trusted authorization boundary, and resident shared Rust runtime.

For the implementable transport-neutral client contract, read
[`../specs/semantic-api.md`](../specs/semantic-api.md). ADR-0020 is semantic
operation authority, ADR-0024 defines immutable revision-pinned proposal
meaning, ADR-0026 defines scoped Grants, trusted footprints, exact Human
Approval, and minimum provenance, and ADR-0022 defines runtime ownership/host
separation. Read [`../specs/semantic-authorization.md`](../specs/semantic-authorization.md)
for the normative authorization contract. Concrete proposal/revision encoding,
session, authorization DTO/state, lifecycle, and transport mechanisms remain
replaceable or separately owned; canonical approval bytes/digests/tokens are
Deferred.

### Storage, formats, and Git

- [`ro-and-roproj-format.md`](ro-and-roproj-format.md) — architecture-level `.roproj` / `.ro` representation direction, the implemented Issue #123 `.roproj/v1` codec/native host workflow, and the Accepted portable-package v1 envelope plus its #3 production implementation.
- [`git-native-workflow.md`](git-native-workflow.md) — how Git participates in authoring, review, and history without becoming the user interface.

For implementable format contracts, continue to [`../specs/README.md`](../specs/README.md); architecture prose must not silently override the normative/provisional specification state.

### Runtime and host boundaries

- [`wasm-strategy.md`](wasm-strategy.md) — Accepted ADR-0022 direction: WASM is an execution target for the same resident Rust semantic/application runtime and Semantic API, not a second semantic implementation.
- [`frontend-backend-boundary.md`](frontend-backend-boundary.md) — frontend projection/workflow state, shared runtime authority, explicit snapshot boundaries, and host composition.
- [`performance-model.md`](performance-model.md) — provisional performance guidance that should be refined by evidence.

ADR-0022 accepts resident Rust runtime ownership, the no-second-canonical-client-model rule, host separation, explicit snapshot boundaries, and native/WASM semantic parity. Issue #93 implements the current provisional in-process resident session, monotonic revision, and guarded installation seam; Issue #94 adds its internal selective entity/field projections and occurrence/revision-keyed invalidation facts; Issue #95 retains rebuildable, full-oracle-checked calculation and supporting query indexes across revisions. Public session handles, cross-host concurrency, Worker lifecycle, IPC/FFI/network mapping, persistence/recovery, and serialization/ABI remain Deferred to related host work.

### AI

- [`ai-native-architecture.md`](ai-native-architecture.md) — accepted direction that AI acts on the semantic model rather than imitating UI actions.

AI is a first-party semantic client under ADR-0007/ADR-0020. Its reviewable
semantic proposals use ADR-0024's immutable revision-pinned SemanticPatch rather
than an AI-only mutation vocabulary. ADR-0026 supplies the Human/Delegated,
capability/scope/Grant, trusted-footprint, exact-Approval, replay/revocation,
provenance, and external-effect boundary. Issue #29 implements the current
provisional workspace-engine lifecycle/state seam, and #30 adds the provisional
provider-facing instruction/data, trusted-context, raw-bypass, safe-denial, and
host-effect-denial adapter. Concrete authentication, public wire DTOs, broader
projection delivery, and transport remain later host work. Issue #93 supplies
the internal resident revision/session mechanics; public authentication and
transport remain deferred.

### Collaboration and future presentation

- [`distributed-collaboration.md`](distributed-collaboration.md) — Accepted semantic merge/history, ADR-0034 team-policy/recovery, and ADR-0035 causality/selective-convergence boundaries; concrete realtime, transport, clock, datatype, and runtime mechanics remain Deferred.
- [`rendering-system.md`](rendering-system.md) — future rendering and semantic-projection hypothesis, including the research → Git-reviewed knowledge → presentation use case tracked in #67, for later Designer MVP work.

## Maturity map

Use these broad cues together with the reconciliation register:

| Area | Current authority/maturity |
| --- | --- |
| Semantic-first platform direction | Accepted |
| First-class Headless Semantic API boundary | Accepted under ADR-0020; exact Rust API and transports remain Provisional/Deferred |
| Progressive semantic strengthening | Accepted under ADR-0021; concrete freeform object/runtime/UI mechanics Deferred |
| `.roproj` source / `.ro` portable-artifact relationship | Accepted under ADR-0003; exact `.roproj/v1` tree/DTO contract Accepted under ADR-0023 and production pure codec/native explicit host workflow implemented by #123; exact portable-package v1 contract Accepted under ADR-0025 with production codec/native host/CLI workflow implemented by #3; optional provider-neutral Git/CI composition implemented by #44 |
| Current `.ro` v0.1 encoding details | Provisional implemented baseline |
| Rust crate graph | Accepted Milestone 02 boundary implemented; exact Rust API remains Provisional |
| AI as delegated semantic client | Accepted under amended ADR-0007; scoped authorization and exact Human Approval Accepted under ADR-0026; #29 provider-neutral lifecycle, #30 provider-facing hostile boundary, and #93 resident publication composition implemented, while public transport/authentication remain Deferred |
| Revision-pinned SemanticPatch proposal | Accepted under ADR-0024; ADR-0026 consumes its structural binding without selecting canonical bytes/digest/token; #29 provisional Rust lifecycle and #93 internal resident revision mechanics implemented while public wire remains Deferred |
| Resident Native/WASM runtime and host separation | Accepted under ADR-0022; #93 implements the first production in-process resident session, #94 its internal selective projections/invalidation facts, and #95 rebuildable full-oracle-equivalent retained state with native/WASM evidence; public transport/persistence mechanics remain Deferred |
| Distributed collaboration beyond semantic merge | ADR-0034 team-policy/multi-effect recovery and ADR-0035 causality/selective-convergence boundaries Accepted; concrete realtime, transport, clock, datatype, and runtime mechanisms Deferred |
| Rendering/UI and cross-view projection architecture | Future hypothesis |

## Reading rule for architecture work

1. Start with Product Constitution and Knowledge Authority.
2. Read relevant Accepted ADRs.
3. Use this index to locate the architecture explanation.
4. Read the corresponding specification when an implementable contract matters.
5. Check the target Decision/implementation issue for unresolved details.
6. Check code/tests only when current shipped behavior matters.

If architecture prose conflicts with a higher-authority Accepted decision, reconcile the contradiction explicitly rather than choosing whichever file is newer.
