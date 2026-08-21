# Architecture Decision Records

This directory contains Tachiko Work Architecture Decision Records (ADRs).

ADRs preserve both current authority and decision history. A Superseded ADR remains in the repository so future readers can understand why the project changed direction.

For project-wide authority rules, read [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md). For the current reconciliation map, read [`../governance/canonical-reconciliation-register.md`](../governance/canonical-reconciliation-register.md).

## Current ADR index

| ADR | Decision | State | Current authority note |
| --- | --- | --- | --- |
| [ADR-0001](ADR-0001-semantic-platform-not-office-clone.md) | Semantic platform, not Office clone | Accepted | Foundational architectural direction |
| [ADR-0002](ADR-0002-game-dev-first-wedge.md) | Game-development first wedge | Superseded | Replaced by ADR-0005; retained for history |
| [ADR-0003](ADR-0003-ro-and-roproj-representation.md) | `.roproj` source / `.ro` portable representation | Accepted | Long-term representation relationship; current direct `.ro` persistence is implementation state |
| [ADR-0004](ADR-0004-mvp-boundary.md) | Developer MVP boundary | Accepted, historical milestone boundary | Defines the completed Developer MVP scope |
| [ADR-0005](ADR-0005-game-development-first-wedge.md) | Game-development first commercial wedge | Accepted | Current first-wedge authority |
| [ADR-0006](ADR-0006-mvp-interface-strategy.md) | CLI-first MVP interface | Accepted | GUI remains a later projection |
| [ADR-0007](ADR-0007-ai-semantic-interaction-model.md) | AI semantic interaction model | Accepted | AI is a semantic client; mutation remains approval-gated at this stage |
| [ADR-0008](ADR-0008-developer-mvp-completion-and-next-phase.md) | Developer MVP completion / next phase | Superseded | ADR-0009 is the surviving authority |
| [ADR-0009](ADR-0009-developer-mvp-validation-and-next-phase.md) | Developer MVP validation / next phase | Accepted, historical milestone boundary | Confirms Developer MVP as the completed validation point |
| [ADR-0010](ADR-0010-first-usable-product-workflow.md) | First usable product workflow | Accepted | Current CLI-first usable workflow |
| [ADR-0011](ADR-0011-semantic-three-way-merge.md) | Semantic three-way merge | Accepted for implemented merge contract | Broader collaboration/conflict semantics remain separate Open Questions |
| [ADR-0012](ADR-0012-release-distribution-contract.md) | Release distribution contract | Accepted | Tag-gated release/distribution contract |

## How to use ADRs

- Prefer an explicit Accepted ADR over older exploratory architecture, roadmap, research, or Issue prose.
- A newer implementation does not silently supersede an Accepted ADR.
- A Superseded ADR is historical context, not current implementation authority.
- If an ADR defines an Accepted direction but the implementation has not caught up, classify that as implementation lag rather than silently rewriting the decision.
- New expensive-to-reverse public contracts should be promoted through explicit decision work, not hidden inside implementation Issues.

The canonical reconciliation register is the source for cross-document status when a narrower architecture or specification file has mixed decision states.
