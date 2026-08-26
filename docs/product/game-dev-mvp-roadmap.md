# Game Dev MVP Roadmap

> **Scope note:** This file records the narrower CLI-first Developer MVP and
> game-balance slice. For the repository-wide product stages, current planning
> horizon, live Milestone views, and roadmap maintenance rules, see
> [`product-roadmap.md`](product-roadmap.md). Accepted ADRs, specifications, and
> current implementation evidence take precedence over older checkpoint wording
> below.

## Product Thesis

The first product is not an Excel replacement.

It is a CLI-first, Git-native computational game-balance workflow.

## MVP: Semantic Game-Balance Workflow

- semantic document model
- versioned `.ro` foundation
- schema validation
- formula computation
- deterministic serialization
- semantic diff
- CLI workflow
- game balance example
- AI semantic read/query capability
- immutable workflow previews with explicit output paths
- deterministic formula authoring and explainability

## Implemented behavior at checkpoint `8f214c3`

- schema-based scalar edit (`set`)
- entity lifecycle (`duplicate`, `rename`, `remove`)
- validated formula authoring (`formula set`) with bounded parser complexity
- typed three-way merge (`merge`)
- CLI-first onboarding, explain, review, validate, calculate, and export flows
- AI query/suggestion model with explicit approval-required write boundary

## Deferred roadmap layers

These capabilities are intentionally not implemented yet and remain future work:

- GUI/React or spreadsheet-style designer interface
- `.roproj` production workflow and Git-driver adapters
- realtime/online collaboration
- cloud-hosted project orchestration and enterprise permissions
- Office/Excel compatibility
- engine-level plugins (Unity, Unreal, Godot)
- engine-specific live syncing or runtime hot-reload pipelines
- operation logs, event sourcing, and CRDT-style sync

## Success Metric

A developer or technical designer can create, calculate, validate, review,
and merge game balance data through a version-controlled semantic workflow, with
AI able to explain the model and impact.
