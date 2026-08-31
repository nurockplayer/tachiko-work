# Tachiko Work Product Roadmap

Status: Planning index

This page is the repository-wide entry point for product sequencing. It does
not create product, architecture, format, API, or release authority. The
Product Constitution, Accepted ADRs and policies, and applicable normative
specifications remain authoritative. GitHub Issues own unresolved decisions and
implementation work.

The roadmap uses one repository-level planning axis:

- **GitHub Milestone** = the earliest product maturity stage that genuinely
  requires a decision or implementation;
- **Epic and labels** = workstream, area, and priority inside that stage;
- **Issue and PR** = executable scope and implementation evidence.

Milestones are dependency and maturity horizons, not calendar promises. They do
not imply due dates or release commitments.

In this index, `Completed horizon` means a stage has no open Issues and is no
longer current; the underlying GitHub Milestone may remain open for historical
classification. It is a planning-state statement, not a claim that every
aspirational product outcome named for that stage has been implemented.

The milestone policy and original classification audit are recorded in
[Issue #56](https://github.com/nurockplayer/tachiko-work/issues/56). The live
[GitHub Milestones](https://github.com/nurockplayer/tachiko-work/milestones) and
Issues remain the operational planning surface.

## Current horizon

The current repository planning horizon is:

> **06 · Team Workspace Beta**

Designer MVP has no remaining open Issues and is therefore a completed horizon.
Team Workspace Beta is the active planning horizon for making semantic work
reviewable and collaborative through machine deltas, deterministic
merge/conflicts, history policy, permissions, and team review workflows.

- [Open work in the current horizon](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+is%3Aopen+milestone%3A%2206+%C2%B7+Team+Workspace+Beta%22)
- [All open Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+is%3Aopen)
- [Intentionally or provisionally unmilestoned Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+is%3Aopen+no%3Amilestone)

## Product stages

| Stage | Planning state | Product outcome | Live Issue view |
| --- | --- | --- | --- |
| `01 · Developer MVP` | Baseline established | Prove the CLI-first semantic game-balance workflow: typed data, deterministic formulas, validation, semantic diff/merge, Git review, and provider-free AI analysis. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2201+%C2%B7+Developer+MVP%22) |
| `02 · Core & Format Hardening` | Completed horizon | Stabilize the irreversible or expensive-to-change semantic, runtime, diagnostics, formula, storage, versioning, and portability contracts. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2202+%C2%B7+Core+%26+Format+Hardening%22) |
| `03 · Game Dev Alpha` | Completed horizon | Prove the standalone game-balance product workflow plus optional Git-native review/CI, production `.roproj` / `.ro`, safe SemanticPatch-based mutation, and useful semantic analysis for early adopters. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2203+%C2%B7+Game+Dev+Alpha%22) |
| `04 · Game Studio Beta` | Completed horizon | Add practical engine-integration paths, richer formula/scenario analysis, extension foundations, and pilot evidence from technical designers and tools teams. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2204+%C2%B7+Game+Studio+Beta%22) |
| `05 · Designer MVP` | Completed horizon | Enable non-CLI-first use through a graphical workspace, table/spreadsheet projections, visual authoring, progressive typing, and designer-grade diagnostics. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2205+%C2%B7+Designer+MVP%22) |
| `06 · Team Workspace Beta` | **Active** | Make semantic work reviewable and collaborative through machine deltas, deterministic merge/conflicts, history policy, permissions, and team review workflows. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2206+%C2%B7+Team+Workspace+Beta%22) |
| `07 · Migration & Enterprise Beta` | Future horizon | Support gradual adoption through evidence-backed Office/ODF/CSV interoperability, explicit migration, private extensions, and validated enterprise workflows. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2207+%C2%B7+Migration+%26+Enterprise+Beta%22) |
| `08 · Tachiko Work 1.0` | Future horizon | Establish the stable public platform promise: supported API/format commitments, conformance and security evidence, extension compatibility, licensing, distribution, and support boundaries. | [Issues](https://github.com/nurockplayer/tachiko-work/issues?q=is%3Aissue+milestone%3A%2208+%C2%B7+Tachiko+Work+1.0%22) |

## Workstream roadmaps

The product-stage milestones do not replace domain sequencing:

- [#35 — RO Protocol architecture and open format roadmap](https://github.com/nurockplayer/tachiko-work/issues/35)
  owns Protocol horizons and remains unmilestoned as a cross-stage Epic.
- [#42 — AI-native architecture roadmap](https://github.com/nurockplayer/tachiko-work/issues/42)
  owns AI sequencing and remains unmilestoned as a cross-stage Epic.
- [`game-dev-mvp-roadmap.md`](game-dev-mvp-roadmap.md) records the narrower
  CLI-first Developer MVP/game-balance slice.
- [`game-dev-market-entry.md`](game-dev-market-entry.md),
  [`developer-adoption-strategy.md`](developer-adoption-strategy.md), and
  [`engine-integration-strategy.md`](engine-integration-strategy.md) provide
  product and adoption context. They do not independently assign Issues to a
  competing milestone system.

## Unmilestoned exceptions

An open Issue should normally have one product milestone. It may remain
unmilestoned only when the reason is explicit, for example:

- a cross-stage Epic/index such as #35 or #42;
- deliberately post-roadmap work such as bounded autonomy;
- a deferred placeholder that has not met its activation condition;
- Research/Hypothesis work whose exit decision must first determine whether a
  concrete product-stage commitment is warranted.

When research produces implementable work, create or promote a concrete child
Issue and assign that child to the earliest stage that requires it.

## GitHub Project projection

A GitHub Projects v2 board may present this roadmap, but it is a projection over
Issues and Milestones, not another source of truth. Use exactly one project:

> [**Tachiko Work · Product Roadmap**](https://github.com/users/nurockplayer/projects/2)

Recommended views:

1. **Product Roadmap** — all open Issues, grouped by Milestone.
2. **Current · M06** — open Issues in `06 · Team Workspace Beta`.
3. **Workstreams** — grouped or filtered by area labels and Epic relationships.
4. **Future** — Milestones `07` through `08`.
5. **Unmilestoned** — explicit Epics, deferred placeholders, and research only.

Do not add speculative start dates, target dates, or iterations merely to fill a
timeline. Add calendar fields only when release evidence, a pilot commitment,
or another real delivery constraint makes them meaningful.

## Maintenance rules

1. Assign each concrete Issue to the earliest product stage that needs it.
2. Split later-stage implementation into child Issues instead of overloading one
   Issue across several milestones.
3. Keep priority separate from milestone.
4. Do not create parallel Protocol, AI, GUI, or business GitHub Milestone axes.
5. Update this page only when the stage model, active horizon, or navigation
   contract changes. Ordinary backlog movement belongs in GitHub, not duplicated
   prose.
