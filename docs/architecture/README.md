# Tachiko Work Architecture

Architecture documents explain the current model, implemented baseline, target direction, and working hypotheses. They do not all carry the same decision state.

Before treating an architecture detail as permanent, read [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and the [`canonical reconciliation register`](../governance/canonical-reconciliation-register.md).

## Architecture diagrams

![Detailed architecture diagram: React frontend, Tauri desktop shell, Rust semantic backend, formats, and integrations.](tachiko-work-architecture-detailed.png)

The diagrams show product architecture direction. Internal crate boundaries, runtime seams, and semantic-model details may evolve where the repository classifies them as Provisional or Open Questions.

## Read by subsystem

### Semantic core

- [`semantic-core-rationale.md`](semantic-core-rationale.md) — why the project is semantic-first; explanatory rationale, not a replacement for ADR authority.
- [`document-model.md`](document-model.md) — accepted semantic-document direction; detailed graph mechanics remain subject to Core & Format Hardening.
- [`unified-semantic-model.md`](unified-semantic-model.md) — accepted unified-semantic direction across structured data, formulas, views, and AI operations.

### Storage, formats, and Git

- [`ro-and-roproj-format.md`](ro-and-roproj-format.md) — architecture-level `.roproj` / `.ro` representation direction under ADR-0003.
- [`git-native-workflow.md`](git-native-workflow.md) — how Git participates in authoring, review, and history without becoming the user interface.

For implementable format contracts, continue to [`../specs/README.md`](../specs/README.md); architecture prose must not silently override the normative/provisional specification state.

### Rust core and runtime boundaries

- [`rust-crate-architecture.md`](rust-crate-architecture.md) — current implemented crate graph plus the Accepted Milestone 02 layering baseline recorded by ADR-0016.
- [`frontend-backend-boundary.md`](frontend-backend-boundary.md) — accepted frontend/semantic-backend direction with detailed runtime seam still evolvable.
- [`wasm-strategy.md`](wasm-strategy.md) — WASM strategy hypothesis/Open Question; do not treat it as a selected product-runtime contract.
- [`performance-model.md`](performance-model.md) — provisional performance guidance that should be refined by evidence.

### AI

- [`ai-native-architecture.md`](ai-native-architecture.md) — accepted direction that AI acts on the semantic model rather than imitating UI actions.

Concrete mutation, permissions, approval, provenance, and security contracts remain narrower protocol/security work.

### Collaboration and future presentation

- [`distributed-collaboration.md`](distributed-collaboration.md) — future collaboration hypothesis/Open Question beyond the implemented semantic merge baseline.
- [`rendering-system.md`](rendering-system.md) — future rendering and semantic-projection hypothesis, including the research → Git-reviewed knowledge → presentation use case tracked in #67, for later Designer MVP work.

## Maturity map

Use these broad cues together with the reconciliation register:

| Area | Current authority/maturity |
| --- | --- |
| Semantic-first platform direction | Accepted |
| `.roproj` source / `.ro` portable-artifact relationship | Accepted direction |
| Current `.ro` v0.1 encoding details | Provisional implemented baseline |
| Rust crate graph | Provisional implemented v0.1 baseline; Accepted Milestone 02 target in ADR-0016 |
| AI as semantic client | Accepted direction |
| Native/WASM runtime seam | Provisional / Open Question |
| Distributed collaboration beyond semantic merge | Hypothesis / Open Question |
| Rendering/UI and cross-view projection architecture | Future hypothesis |

## Reading rule for architecture work

1. Start with Product Constitution and Knowledge Authority.
2. Read relevant Accepted ADRs.
3. Use this index to locate the architecture explanation.
4. Read the corresponding specification when an implementable contract matters.
5. Check the target Decision Issue for unresolved details.
6. Check code/tests only when current shipped behavior matters.

If architecture prose conflicts with a higher-authority Accepted decision, reconcile the contradiction explicitly rather than choosing whichever file is newer.
