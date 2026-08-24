# Decision Traceability Protocol

Status: Accepted policy when merged

## Purpose

Tachiko Work preserves decisions across documents, GitHub Issues, pull requests, tests, and implementation history. This protocol defines the minimum cross-linking discipline needed so a future human or AI contributor can trace a material behavior to its current authority, original reasoning, implementation evidence, and supersession history without relying on oral history or raw chat transcripts.

This policy complements `knowledge-authority.md`. It does not change the authority hierarchy. It defines how artifacts point to one another.

## Core rule: one-hop traceability

For material decisions and contract changes, each artifact should link directly to the nearest relevant predecessor and successor when known.

A healthy chain normally looks like:

```text
research / discussion
    ↓
decision or research issue
    ↓
ADR / accepted policy / normative spec
    ↓
implementation issue
    ↓
pull request / commit
    ↓
tests / executable evidence
    ↓
current behavior
```

The chain must also be navigable in reverse. A contributor starting from current behavior should be able to locate the governing spec or decision, then the reasoning and alternatives that produced it.

Do not require a reader to search the entire repository merely to discover the next link in the chain.

## Provenance is not authority

A link records provenance or relationship. It does not promote the linked artifact's authority.

Examples:

- a discussion document may explain where a hypothesis came from but remains historical context;
- an implementation PR may prove shipped behavior but does not silently supersede an Accepted ADR;
- a Decision Issue may contain a strong recommendation but remains unresolved until the authorized decision process promotes it;
- an `agent-handoff:v1` record may explain working state but must not become the only durable source of a decision.

Always interpret links using `knowledge-authority.md` and the canonical reconciliation register.

## Stable relationship labels

When practical, use the following labels verbatim. They are intentionally small and human-readable so future tooling can parse them without making Markdown itself a semantic-core contract.

- `Origin:` — where the question, hypothesis, or requirement came from
- `Related:` — relevant but non-authoritative neighboring work
- `Decision issue:` — issue that owns the unresolved decision
- `Authority:` — Accepted ADR, policy, or other governing authority
- `Implements:` — decision/spec/issue realized by implementation work
- `Specified by:` — current normative specification
- `Validated by:` — tests, fixtures, CI, or other executable evidence
- `Supersedes:` — older record replaced by this record
- `Superseded by:` — newer record that replaced this record
- `Tracking issue:` — issue that owns follow-up research or implementation

These labels describe relationships. They do not define new semantic-core entity types.

## Artifact expectations

### Research and discussion records

A durable research or discussion record should include, when applicable:

- `Status:` such as Hypothesis, Discussion, or Research;
- `Origin:` when the originating conversation, incident, user evidence, or previous artifact is known;
- `Tracking issue:` or `Decision issue:` once focused work exists;
- explicit unresolved questions;
- a statement that the record is evidence/history rather than Accepted authority unless promoted elsewhere.

Raw chat transcripts are not required. Preserve the materially important question, reasoning, alternatives, and conclusions.

### Decision and research issues

A focused issue should link to the source material that motivated it and to the authority it eventually produces.

When closing a material Decision Issue, leave a concise **Decision Capsule** containing:

```text
Decision
<what was decided>

Why
<principal reasons and evidence>

Rejected
<important alternatives rejected and why>

Deferred
<important alternatives deliberately left open>

Authority produced
<ADR / policy / spec links>

Still open
<remaining questions>

Implementation follows
<implementation issues / PRs when known>
```

`Rejected` and `Deferred` are deliberately separate. A deferred option may become valid when constraints change.

Research issues that do not produce an Accepted decision should instead close with the recommendation, evidence, unresolved questions, and explicit next owner.

### ADRs and accepted policies

An ADR or accepted policy should link to:

- the `Decision issue:` that produced it when one exists;
- important `Supersedes:` / `Superseded by:` relationships;
- the current normative specification or architecture document when the decision is implemented through a separate contract;
- follow-up implementation work when useful for navigation.

Do not rewrite old ADR history to make the past look cleaner. Supersession should remain explicit.

### Specifications and architecture documents

A normative or mixed-state document should make its governing authority discoverable.

Where practical, include:

- `Authority:` for the Accepted decisions it implements;
- decision-state notes for Provisional or Open Question sections;
- `Supersedes:` / `Superseded by:` when the document replaces another contract;
- implementation evidence when current behavior materially matters.

Do not duplicate full rationale from the ADR. Link back to it.

### Implementation issues

An implementation issue should state:

- `Authority:` — the Accepted or explicitly Provisional contract it is implementing;
- `Implements:` — the narrower spec/decision/work item;
- boundaries that must not be redesigned;
- expected validation or evidence;
- any new durable decision discovered during implementation.

If implementation exposes a new expensive-to-reverse contract, return to focused decision work rather than silently freezing it in code.

### Pull requests

Every non-trivial PR should explicitly declare:

- `Implements:` or `Related:` issue(s);
- `Authority:` governing ADR/spec/policy when applicable;
- `Decision impact:`;
- `Authority impact:`;
- validation/evidence;
- documentation impact.

Use one of these decision-impact values:

- `None` — no durable decision is introduced or changed;
- `Implements existing decision` — implementation realizes already-authorized behavior;
- `Introduces or amends durable decision` — the PR changes an expensive-to-reverse contract and must point to the corresponding decision work.

Use one or more authority-impact categories as appropriate:

- `None`
- `Discussion / Research`
- `ADR / Policy`
- `Specification`
- `Architecture`
- `Governance`

A PR description is implementation context, not a substitute for promoting a durable decision into the correct authority artifact.

### Tests and executable evidence

Tests do not need prose-heavy metadata. The surrounding implementation Issue, PR, spec, or ADR should make the important evidence discoverable.

For compatibility, persistence, public API, migration, validation, or other expensive-to-reverse contracts, prefer explicit named fixtures/tests that make the contract observable and link them from the PR or specification.

### Agent handoffs

`agent-handoff:v1` records working state:

- what is done;
- what is in progress;
- what remains;
- blockers and boundaries;
- relevant authority.

A handoff must not be the only surviving location for an architectural rationale or accepted decision. Before work closes, durable conclusions must be promoted to the appropriate Issue, ADR, policy, spec, architecture document, or discussion record.

## Closure rule

Before closing or merging material work, ask:

1. Did this work create, modify, implement, or supersede a durable decision?
2. Can the current artifact reach its governing authority in one hop?
3. Can the authority reach the implementation/evidence in one hop when that evidence materially matters?
4. Are rejected and deferred alternatives preserved where future maintainers are likely to ask why?
5. Is any important rationale trapped only in chat, an agent handoff, or a PR description?
6. Did a Provisional or Open Question accidentally become a public invariant through implementation convenience?

If the answer exposes a gap, repair the links or promote the missing durable record before treating the work as complete.

## Relationship to Project Memory

This protocol is intentionally useful without any Tachiko Work product feature. It is the manual baseline that future Project Memory / semantic decision-provenance research may attempt to ingest, validate, and automate.

Project Memory must not be considered successful merely because it can parse these labels. The larger product hypothesis remains whether semantic modeling provides materially better authority-aware `why`, `impact`, `history`, `gaps`, and `context` capabilities than conventional repository navigation.

See GitHub Issue #104 and `docs/discussions/project-memory-and-decision-provenance.md` for that separate Hypothesis/Research track.
