# Knowledge Authority and Decision States

Status: Accepted policy when merged

## Purpose

Tachiko Work contains vision documents, ADRs, specifications, architecture notes, research, discussion history, implementation evidence, and a large GitHub backlog.

Those artifacts do not have equal authority.

This policy defines how humans and AI agents determine what is foundational, what has been decided, what is intentionally provisional, what still needs research, and what is only historical context.

The goal is to make project reasoning cumulative. A future ChatGPT, Deep Research session, Codex run, reviewer, or contributor should not need to reconstruct authority from chronology or writing confidence.

## Decision-state vocabulary

Every decision-bearing statement should be understood as one of the following states.

### Principle

A durable constraint about why Tachiko Work exists or what it must protect.

Examples include user ownership, semantic meaning over historical representation, progressive migration, and keeping the stable core small.

A Principle constrains lower-level decisions but normally does not prescribe a specific implementation.

### Accepted

A concrete direction or decision that has been explicitly adopted.

Accepted decisions remain authoritative until amended or superseded through an explicit decision record or policy change.

### Provisional

A practical choice adopted for the current implementation or milestone because it is useful and reversible, but not intended to become an irreversible ecosystem contract without additional evidence.

Provisional choices should preserve migration seams where practical.

### Hypothesis

A promising idea that still requires research, prototype evidence, user validation, standards analysis, or technical proof.

Hypotheses may guide investigation. They must not silently become implementation invariants.

### Open Question

A known unresolved decision.

An Open Question should be answered through focused research and a Decision Issue, ADR, specification amendment, or policy decision as appropriate.

Implementation convenience must not silently answer an Open Question when the choice would create a durable contract.

### Superseded

A previous decision or direction retained for historical understanding but no longer authoritative.

Superseded records should point to the record that replaced them whenever possible.

## Authority hierarchy

When artifacts appear to disagree, use this hierarchy and then reconcile the contradiction explicitly.

### 1. Product Constitution and foundational principles

`docs/vision/product-constitution.md`, together with the durable principles in Mission and Design Principles, defines the highest-level product constraints.

It answers why the project exists and what lower-level decisions must not accidentally sacrifice.

### 2. Accepted ADRs and accepted governance policies

Accepted ADRs are the canonical authority for concrete architectural decisions.

Accepted governance, licensing, security, and release policies are authoritative within their domains.

A newer implementation or Issue does not silently override an Accepted ADR.

### 3. Normative specifications

A specification may define an implementable contract only to the extent that it is consistent with Accepted ADRs and its own decision state.

Specifications must distinguish:

- implemented/current contract;
- accepted target direction;
- provisional draft;
- unresolved future design.

A file being located under `docs/specs/` does not automatically make every sentence Accepted.

### 4. Architecture and product documents

Architecture and product documents explain the current model, implemented baseline, strategic direction, and working design.

They are authoritative only to the decision state explicitly attached to the relevant statement or recorded in the reconciliation register.

They may not override a higher-level Accepted ADR by accident.

### 5. Shipped implementation, tests, and README

Code, tests, release artifacts, and README are strong evidence of what the current product actually does.

Implementation evidence is not automatically a permanent architecture decision.

If shipped behavior conflicts with an Accepted ADR, the project has a reconciliation problem. The correct response is to update the ADR/spec or migrate the implementation deliberately, not to pretend one side never existed.

### 6. GitHub Decision Issues

A Decision Issue preserves an unresolved problem, constraints, alternatives, evidence, Question Logs, and Decision Logs.

Until promoted into an Accepted ADR or policy, its unresolved content is an Open Question.

Imperative wording inside an Issue does not grant it authority to contradict an Accepted ADR.

### 7. Implementation Issues and Epics

Implementation Issues execute accepted or explicitly provisional work. They are not a legitimate place to invent expensive-to-reverse architectural contracts silently.

Epics are planning and indexing surfaces. Their ordering and scope are useful, but an Epic does not outrank the decisions it references.

### 8. Research and discussion history

Research records are normally Hypotheses, evidence, or comparisons until promoted.

Discussion history preserves origin, questions, rejected alternatives, and evolution. It is historical context, not current implementation authority unless it points to a still-accepted decision.

## Conflict rule

When two artifacts conflict:

1. Identify each artifact's authority level and decision state.
2. Distinguish a real semantic contradiction from implementation lag or stale wording.
3. Prefer an explicit Accepted decision over older exploratory prose.
4. Preserve historical records instead of deleting the evidence that a decision changed.
5. If existing Accepted records are genuinely insufficient, keep the matter an Open Question and resolve it through research and an ADR/policy decision.
6. Never let a code change, prompt, Issue body, or AI recommendation silently redefine a foundational invariant.

## Implementation state is separate from decision state

`Implemented`, `Not Implemented`, `Partially Implemented`, and `Future` describe product state, not decision authority.

For example:

- `.roproj` can be an **Accepted** architectural direction while **Not Implemented** in v0.1.
- canonical `.ro` JSON can be **Implemented** while some of its current identifier or encoding choices remain **Provisional** pending Core & Format Hardening.
- event sourcing can be **Not Implemented** and also remain a **Hypothesis/Open Question**.

Never infer one axis from the other.

## Freeze less, classify more

Before promoting a choice from Provisional or Open Question to Accepted, ask:

1. Will persisted documents depend on this?
2. Will Git history depend on this?
3. Will external tools, SDKs, plugins, or independent implementations depend on this?
4. Is the behavior visible as a public semantic/API contract?
5. Is migration later expensive, lossy, or organizationally disruptive?
6. Can the same requirement remain behind an adapter or replaceable boundary?

The more `yes` answers, the stronger the evidence and decision record required before freezing the choice.

## AI and Codex operating rule

A research or implementation agent should load, in order:

1. Product Constitution and Design Principles.
2. This knowledge-authority policy.
3. Relevant Accepted ADRs and policies.
4. Relevant current/provisional specifications and architecture docs.
5. The target Issue and its discussion history.
6. Current implementation/test evidence when the task depends on shipped behavior.

Then:

- Deep Research may investigate an Open Question and recommend a direction.
- A recommendation begins as Hypothesis/Provisional unless an authorized decision process promotes it.
- Codex may implement an Accepted contract or a clearly scoped Provisional implementation choice.
- Codex must not turn an Open Question into a durable public invariant merely because a coding task requires a local choice.
- If implementation discovers a contradiction with higher authority, return to reconciliation/ADR work rather than silently choosing a winner.

## Founder escalation rule

Do not escalate ordinary technical choices to the founder merely because they are difficult.

Choices such as identifier algorithms, serialization libraries, parser structures, crate boundaries, or cache representations should normally be resolved by focused research against project constraints.

Escalate only when the answer materially changes:

- product identity or mission;
- a foundational Principle;
- a difficult-to-reverse public ecosystem promise;
- material licensing/governance posture;
- a business/product tradeoff for which technical evidence cannot determine the desired outcome.

The founder owns intent. The repository and research process should carry the technical memory.