# Project Memory and Decision Provenance

Status: Discussion / Hypothesis

This document preserves a product and architecture discussion about using Tachiko Work itself to capture the semantic lineage of project decisions, evidence, implementation, and supersession history. It is historical context and a product hypothesis, not an Accepted ADR, normative specification, or implementation commitment.

## Context

Tachiko Work is currently being developed with heavy use of AI agents such as ChatGPT, Deep Research, and Codex. The repository already contains a canonical knowledge base, Product Constitution, Knowledge Authority policy, Accepted ADRs, specifications, architecture documents, GitHub Issues, PRs, tests, and `agent-handoff:v1` records.

The immediate concern was long-term project legibility: if future contributors with strong technical ability join after many AI-assisted decisions have accumulated, how can they quickly locate not only the current implementation, but also why it exists, which decision authorized it, what evidence supported it, what alternatives were rejected, and whether the decision was later superseded?

A documentation-only answer would be to strengthen project traceability conventions around research, Decision Issues, ADRs, specifications, PRs, tests, and supersession links. The discussion then raised a larger possibility: this capability may itself belong inside Tachiko Work.

## Core product hypothesis

Tachiko Work may be able to preserve not only the current state of work, but the semantic lineage of why that state exists.

In this model, Tachiko Work would provide general semantic primitives for meaning, relationships, provenance, authority, and history. A project-memory model would be one domain schema built on top of those primitives rather than a project-management subsystem hard-coded into the semantic core.

Tachiko Work itself would become the first dogfood project for this capability.

## Desired traceability

A future contributor should be able to move in both directions through a decision chain such as:

```text
research / evidence
    ↓
decision issue
    ↓
ADR / accepted policy
    ↓
specification / architecture
    ↓
implementation issue
    ↓
PR / commit
    ↓
tests / evidence
    ↓
current behavior
```

The reverse query is equally important:

```text
current behavior
    ↓
implementation evidence
    ↓
current specification
    ↓
authoritative decision
    ↓
original reasoning and alternatives
```

The goal is not merely to find documents. The system should be able to distinguish current authority, historical reasoning, implementation evidence, unresolved questions, and superseded material.

## Possible semantic model

The following names are illustrative domain concepts, not proposed semantic-core types:

- Decision
- Research
- Claim
- Evidence
- Issue
- ADR
- Specification
- Architecture artifact
- Implementation
- Pull Request
- Test
- Release
- Person or Agent

Illustrative relationships include:

- `motivated_by`
- `supported_by`
- `contradicts`
- `decides`
- `implements`
- `validates`
- `depends_on`
- `supersedes`
- `superseded_by`
- `derived_from`
- `blocked_by`

A possible dependency chain could therefore express that a research result supports an architectural decision, the decision governs a specification, an implementation issue implements that specification, a PR realizes the issue, and tests validate the resulting behavior.

## Product experiences enabled by the model

If the underlying semantic relationships prove useful, several interfaces could consume the same model.

### Explain why

A query such as:

```text
tachiko why diagnostics.formula_failure
```

could return the current authority, governing specification, rationale, rejected or deferred alternatives, implementation work, and validating tests.

### Inspect impact

```text
tachiko impact ADR-0018
```

could identify specifications, subsystems, APIs, tests, and implementation artifacts that depend on a decision.

### Inspect history

```text
tachiko history storage.canonicalization
```

could reconstruct the progression from research and decision work through implementation and later amendments.

### Detect knowledge gaps

```text
tachiko gaps
```

could eventually detect conditions such as:

- Accepted decisions with no implementation evidence
- implemented behavior with no recorded authority
- superseded specifications that are still referenced as current
- closed decision issues without a concise decision record
- semantic-contract changes whose PRs do not reference governing decisions or specs

### Prepare agent context

A query such as:

```text
tachiko context issue:89
```

could assemble only the relevant Product Constitution constraints, Accepted ADRs, current specs, superseded material, open questions, and explicit do-not-redesign boundaries for an AI or human contributor.

This would make semantic context selection a product capability rather than a growing prompt-maintenance exercise.

## Relationship to the Headless Semantic API

This hypothesis is a strong potential dogfood case for the decision under GitHub Issue #10, "Headless Semantic API as a first-class interface."

The same project-memory model could be consumed by:

- CLI queries
- desktop or web visualization
- AI-agent context loading
- CI authority checks
- GitHub integrations
- exports to Markdown or other portable forms

If those clients can use one semantic operation surface without leaking GitHub or Markdown storage details into the core, the use case would provide practical pressure on the first-class Semantic API design.

However, this document does not decide Issue #10 and should not be used to pre-commit its contract.

## Critical architectural boundary

Project-memory concepts such as `Decision`, `ADR`, and `GitHubIssue` should not be promoted into the stable semantic core merely because Tachiko Work uses them internally.

The intended direction to investigate is closer to:

```text
Tachiko semantic foundations
  ├─ typed entities
  ├─ stable identity
  ├─ references / relationships
  ├─ schemas
  ├─ validation
  ├─ queries
  ├─ provenance
  └─ semantic operations
          ↓
Project Memory domain schema / package
  ├─ Decision
  ├─ Evidence
  ├─ ADR
  ├─ Issue
  ├─ PR
  └─ project-specific relationships
```

This preserves the Product Constitution principle that the stable core should stay small and that reusable capabilities should retain replaceable seams and extension points.

## Dogfooding path to investigate

The lowest-risk progression discussed was:

1. Preserve the current GitHub + Markdown + ADR + tests workflow as the source material.
2. Define a project-memory schema that can represent existing Tachiko Work artifacts and relationships without changing their authority.
3. Build a read-only importer or projection from repository/GitHub evidence into Tachiko Work semantics.
4. Evaluate whether queries such as `why`, `impact`, `history`, `gaps`, and `context` are materially better than conventional repository search and hand-maintained indexes.
5. Only after the read model proves useful, evaluate write operations that can record or link decisions and export changes back to portable Git/Markdown/GitHub representations.

GitHub and plain-text artifacts should remain interoperability and ownership boundaries rather than becoming hidden implementation details that only Tachiko Work can understand.

## Success criterion

A useful first benchmark would be intentionally demanding:

> A technically capable contributor who has never participated in Tachiko Work development should be able, using the repository and the project-memory capability, to locate the current authority, original rationale, relevant alternatives, implementation evidence, and supersession history for a major architectural decision within a few hours, without requiring oral project history.

A second benchmark is AI-specific:

> A fresh AI-agent session should be able to obtain bounded, authority-aware context for a task without rediscovering or silently re-deciding established architecture.

## Relationship to existing documentation governance

This hypothesis complements rather than replaces the current knowledge system.

The existing documentation discipline remains valuable even if Tachiko Work eventually models the relationships semantically. In particular:

- Product Constitution and Foundational Principles remain the highest-level constraints.
- Research and discussions remain evidence/history rather than automatic authority.
- Decision Issues preserve unresolved questions and decision logs.
- ADRs preserve durable decisions and rejected/deferred alternatives.
- Specifications and architecture describe current contracts and structure.
- PRs, commits, and tests provide implementation evidence.
- `agent-handoff:v1` describes current working state but must not become the only durable source of a decision.
- superseded records should remain available and point to their replacements.

A semantic project-memory layer would make these relationships queryable; it should not erase their different authority roles.

## Open questions

The discussion deliberately leaves the following unresolved:

1. Is provenance/lineage a sufficiently general semantic primitive for Tachiko Work, or can the use case be satisfied with ordinary typed relations and metadata?
2. What is the minimal generic model required before Project Memory can be implemented as a domain package rather than core special cases?
3. Which Git/GitHub/Markdown artifacts are authoritative inputs versus merely discoverable evidence?
4. How should external identifiers such as issue numbers, PR numbers, commit SHAs, paths, and ADR identifiers map onto Tachiko Work identity without contaminating semantic identity?
5. Should the first experiment be purely read-only, and what evidence would justify write-back capabilities?
6. How should authority states such as Foundational, Accepted, Provisional, Hypothesis, Open Question, and Superseded be represented without duplicating or weakening `knowledge-authority.md`?
7. What should be derived automatically versus explicitly authored by humans or agents?
8. How should stale or contradictory links be diagnosed?
9. Does this belong inside the current Core & Format Hardening planning horizon, or should it remain a later dogfood/reference application while current identity, storage, validation, formula, and Semantic API contracts harden?
10. Which part of this hypothesis should inform Issue #10 without expanding #10 into a project-memory design exercise?

## Current recommendation

Treat Project Memory / Decision Provenance as a distinct product and architecture hypothesis with Tachiko Work as its first dogfood user.

Do not implement it immediately and do not hard-code project-management concepts into the semantic core. Preserve this document as discussion evidence, use it as a reference input to a focused future research/decision issue, and let the current foundational contracts mature enough that the experiment tests the architecture rather than distorting it.
