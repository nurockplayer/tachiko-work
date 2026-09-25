# Tachiko Work documentation

## Start with the question, not the archive

[What is Tachiko Work?](../README.md) →
[Three-minute engineering map](../ARCHITECTURE.md) → the relevant detail below.
The map explains the whole system; this page routes deeper questions. Neither
replaces task-relevant authority or the instructions in
[AGENTS](../AGENTS.md) and [CONTRIBUTING](../CONTRIBUTING.md).

| Your question | Canonical starting point |
| --- | --- |
| Why does this exist, and what must it not become? | [Product Constitution](vision/product-constitution.md), [design principles](vision/design-principles.md), [mission](vision/mission.md) |
| Which statement wins, and is it accepted or only proposed? | [Knowledge authority](governance/knowledge-authority.md), [decision traceability](governance/decision-traceability.md), [reconciliation register](governance/canonical-reconciliation-register.md), [ADR index](decisions/README.md) |
| Where does a change belong in the code? | [Engineering map](../ARCHITECTURE.md), [crate ownership/DAG](architecture/rust-crate-architecture.md), [clean architecture guidance](engineering/clean-architecture-and-code.md) |
| What are data, identity, formulas and validation? | [Document model](architecture/document-model.md), [semantic data model](specs/semantic-data-model.md), [formula specification](specs/formula-engine-spec.md), [validation](specs/validation-engine.md), [diagnostics](specs/diagnostics-contract.md) |
| What may a client query, propose or execute? | [Semantic API](specs/semantic-api.md), [authorization](specs/semantic-authorization.md), [AI adapter contract](specs/ai-agent-api.md) |
| How do runtime ownership, clients and hosts fit together? | [Frontend/backend boundary](architecture/frontend-backend-boundary.md), [WASM strategy](architecture/wasm-strategy.md), [experimental producer](../packages/browser-client/README.md) |
| Which bytes are compatible, and how do I migrate them? | [Format guide](architecture/ro-and-roproj-format.md), [storage versioning](specs/storage-versioning-and-migration.md), [version-specific spec index](specs/README.md) |
| How do comparison, merge, history and collaboration differ? | [Semantic diff](specs/semantic-diff-spec.md), [collaboration boundaries](architecture/distributed-collaboration.md), [Git workflow](architecture/git-native-workflow.md) |
| What works, what should I run, and what may I work on next? | [Runnable example](../examples/game-balance/README.md), [contribution/validation guide](../CONTRIBUTING.md), [delivery governance](governance/project-governance.md), [live campaign handoff](https://github.com/nurockplayer/tachiko-work/issues/374) |
| Where is the product going? | [Product roadmap](product/product-roadmap.md), then its live owning Issues; stage names are not release claims |

## Read status as well as content

The [knowledge-authority policy](governance/knowledge-authority.md) owns the full
precedence and AI-loading rules. In particular, an Accepted semantic law does
not stabilize every current Rust/serde shape; implementation evidence does not
turn a proposal into an accepted contract. A historical Designer journey does
not make this repository the current spreadsheet UI owner.

The [source-pinned orientation audit](engineering/repository-orientation-audit.md)
records corrections and remaining drift found while rebuilding these entry
points. Use the register as an authority map, not a live implementation queue;
check linked source and current Issue/PR handoffs for delivery and qualification.
If accepted authorities genuinely conflict, surface the conflict through the
existing decision process rather than silently choosing a new contract.

## Browse the library

| Area | What belongs here |
| --- | --- |
| [Vision](vision/) | Foundational purpose and principles |
| [Decisions](decisions/README.md) | Accepted, superseded and other explicitly classified ADRs |
| [Specifications](specs/README.md) | Detailed contracts with their declared decision/implementation states |
| [Architecture](architecture/README.md) | Explanatory subsystem designs and routes to contract authority |
| [Product](product/) | User outcomes, adoption and planning horizons |
| [Governance](governance/) | Authority, delivery, contribution and release policies |
| [Engineering](engineering/) | Code-quality, implementation and agent-workflow guidance |
| [Security](security/) | Threat and trust-boundary documentation |
| [Research](research/) and [business](business/) | Evidence, hypotheses and business analysis—not automatic product commitments |
| [Discussions](discussions/) and [superpowers](superpowers/) | Historical reasoning and scoped plans; verify their status before reuse |

## Keep the entry points small

README owns the human introduction; root ARCHITECTURE owns the engineering
orientation; this index owns task navigation. Keep detailed contracts in their
existing authoritative documents and operational state in live handoffs. Link
rather than reproduce them. Preserve decision history and explicit supersession.

When changing a subsystem, refresh its affected source/status links here or in
the engineering map as needed. Check the relevant authority and follow
[the documentation consistency gate](../scripts/docs-consistency-check.sh);
that gate checks indexed Markdown files/paths, not every anchor or the truth of
prose, so those still need review.
