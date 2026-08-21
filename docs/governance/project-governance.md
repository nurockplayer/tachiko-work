# Project Governance

## Purpose

Tachiko Work is intended to become an open platform, not only a software repository.

Governance must preserve the founding principles:

- semantic model over legacy formats
- open interoperability
- Git-native workflows
- AI-native architecture
- community participation

## Decision Making

Major architectural decisions should be recorded as ADRs.

Changes affecting:

- file formats
- core semantic model
- compatibility rules
- licensing
- security

require explicit design discussion.

## Knowledge authority and decision state

Repository artifacts do not all have the same authority.

`docs/governance/knowledge-authority.md` defines the canonical authority hierarchy and the decision-state vocabulary used by human and AI contributors:

- Principle
- Accepted
- Provisional
- Hypothesis
- Open Question
- Superseded

`docs/governance/canonical-reconciliation-register.md` applies that policy to the current knowledge base and records known supersession/staleness relationships.

A file under `docs/specs/`, a strongly worded Issue, or an implemented code path is not automatically an Accepted long-term invariant. Implementation state and decision state are separate axes.

When artifacts conflict, reconcile them explicitly. Do not let chronology, implementation convenience, or confident prose silently choose the winner.

## Decision Discussion Workflow

Major unresolved architecture, product-foundation, security, format, or governance questions should begin as a dedicated GitHub **Decision Issue** before becoming an ADR.

The normal workflow is:

```text
Decision Issue
    |
    v
Focused discussion / research
    |
    v
Question Logs + Decision Logs
    |
    v
Final decision
    |
    v
ADR or policy document
    |
    v
Implementation issue(s)
    |
    v
Pull request(s)
```

### Decision Issue

A Decision Issue preserves the problem and the reasoning history. It should contain:

- canonical constraints from existing Vision, Mission, Architecture, Specs, and Accepted ADRs;
- the unresolved core question;
- important alternatives and tradeoffs;
- risks and affected areas;
- explicit MVP boundary where relevant;
- exit criteria for reaching a mature decision.

A Decision Issue is not itself an Accepted architecture decision.

### Question Logs

Important questions raised during discussion are part of the architectural record and should be preserved when they materially shape the problem, expose an assumption, introduce a useful analogy, challenge an existing direction, or may remain valuable if technology and ecosystem conditions change later.

Question Logs should preserve, as closely as practical:

- the substantive question or problem framing that was raised;
- why the question mattered;
- assumptions it challenged;
- evidence or alternatives it caused the project to investigate;
- the current answer or status, including whether the question remains open.

Questions should not be discarded merely because a current answer has been reached. A good question can outlive its present answer and may need to be revisited when standards, libraries, markets, product constraints, or implementation evidence change later.

The goal is not to archive every conversational sentence. Routine clarification, repetition, and chat scaffolding can be omitted. The goal is to preserve all materially important questions and problem framings without requiring future maintainers to reconstruct them from raw chat history.

### Decision Logs

Important discussion milestones should be recorded as concise issue comments rather than relying on chat-session history.

Decision Logs should preserve:

- changes in assumptions;
- newly discovered evidence;
- alternatives considered and rejected;
- founder/architecture-owner decisions;
- unresolved questions;
- consequences for existing specs, ADRs, and implementation issues.

Question Logs explain **what was worth asking and why**. Decision Logs explain **how the answer evolved and what was decided**. Both belong in the related Decision Issue.

The goal is to preserve the reasoning chain without requiring future maintainers to read raw conversation transcripts.

### ADR

An ADR records the final canonical architectural decision after the discussion has matured.

The ADR should summarize the decision, rationale, consequences, and rejected alternatives. Detailed conversational history, Question Logs, and Decision Logs belong in the related Decision Issue.

Where practical, ADRs should reference their Decision Issue, and Decision Issues should reference the resulting ADR.

### Implementation Issues

Implementation issues should implement accepted decisions or clearly scoped Provisional choices rather than silently make new architectural decisions.

If implementation exposes a contradiction or a materially new expensive-to-reverse architectural choice, open or reopen a Decision Issue and resolve the decision before allowing implementation details to redefine the canonical design accidentally.

Routine technical choices should not be escalated to the founder merely because they require expertise. Focused research should resolve them against accepted constraints unless the result changes product identity, a foundational principle, a difficult-to-reverse public ecosystem promise, or material business/governance posture.

## Principles

Technical elegance alone is not sufficient. Decisions should optimize for:

1. Long-term ecosystem growth
2. Developer adoption
3. User freedom
4. Sustainable maintenance
5. Clear ownership
