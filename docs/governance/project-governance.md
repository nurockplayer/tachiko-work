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

## Decision traceability

`docs/governance/decision-traceability.md` defines the minimum cross-linking protocol between research/discussion, Decision Issues, ADRs/policies/specifications, implementation issues, pull requests, and executable evidence.

The project follows a one-hop traceability rule for material decisions: a contributor should not need repository-wide archaeology merely to discover the next relevant authority or evidence link. This traceability records provenance and relationships; it does not alter the authority hierarchy above.

## Repository delivery workflow

This section is the canonical operating contract for issue-driven repository
delivery. The role names describe responsibilities and do not depend on a
particular person, AI provider, or implementation tool.

Origin: [Issue #141](https://github.com/nurockplayer/tachiko-work/issues/141)

### Responsibilities

- The **Project Steward** owns roadmap state, Issue readiness, durable
  architecture and product decisions, GitHub governance, escalation, and
  cross-PR drift review.
- The **delivery agent** owns repository mutation after one concrete Issue is
  genuinely Ready: focused implementation, tests, necessary documentation,
  pull-request creation, review fixes, exact-head validation, merge, and
  post-merge recalibration.
- GitHub Issues, pull requests, reviews, and Projects remain the shared
  coordination and audit surface.

### Ready gate

A delivery agent may start repository mutation only when all of these
conditions hold:

1. current live repository authority and the Product Roadmap have been checked;
2. durable architecture or product decisions required for production are
   Accepted; any Steward-supplied Provisional choice is limited to a non-durable
   implementation detail permitted by current Accepted authority;
3. scope and acceptance criteria are sufficient for focused delivery; and
4. no conflicting open implementation PR owns the same work.

A Decision or Research Issue with unresolved durable choices is not Ready for
production implementation. **Decision-Ready** is an Issue workflow state, not
Accepted authority. When the Project Steward explicitly marks a resolution
Decision-Ready, it authorizes only a focused authority or specification PR.
Production implementation must wait for that authority to be Accepted and for a
separate implementation Issue to become Ready.

### One-Issue delivery loop

Each Ready Issue uses one independently reviewable PR:

```text
live main + Ready Issue
  -> focused branch
  -> implement / test
  -> local review + repository gates
  -> one PR
  -> hosted review / CI
  -> fix actionable findings
  -> exact-head validation
  -> merge
  -> live-state recalibration
```

Do not bundle unrelated Issues merely to continue a delivery run.

### Canonical PR handoff

Every agent-owned PR must have exactly one top-level comment containing this
marker:

```text
<!-- agent-handoff:v1 -->
```

Create the comment immediately after opening the PR. Thereafter, locate that
comment and PATCH it in place; do not add replacement status comments. The
handoff records, at minimum:

- Issue;
- current status;
- exact head commit;
- last checked `main` commit;
- scope boundary;
- validation evidence;
- unresolved review state;
- next action; and
- escalation or human-decision requirement.

The handoff is operational state. It is not architectural or product authority,
and durable rationale must remain in the appropriate Issue, policy, ADR,
specification, or repository documentation.

### Review and merge discipline

Verify and fix valid correctness, security, data-integrity, Accepted-authority,
and scope findings. P0/P1/P2-equivalent substantive findings block merge.
P3, nitpick, and pure-maintainability suggestions do not create an endless
review loop unless they expose a real repository-gate failure, correctness
risk, or durable-contract problem.

Do not change Accepted authority merely to satisfy reviewer preference.
Escalate genuine authority contradictions to the Project Steward. Never force
push. Merge only the exact reviewed and validated head under repository policy.

### Review-fix convergence circuit breaker

A sequence of individually valid review findings can still show that a delivery
loop is no longer converging. The **Project Steward** owns a qualitative,
root-cause-aware convergence circuit breaker. Review count, commit count, test
count, or elapsed time may support the diagnosis, but none is sufficient by
itself to trip or clear the breaker and none can make a known blocker mergeable.

Classify the active review-fix loop as:

- **GREEN — converging:** substantive findings are materially independent or a
  finite cluster around one invariant; each fix removes a concrete defect;
  scope and Accepted authority remain stable; and new evidence is shrinking
  the blocker set rather than only changing its wording.
- **AMBER — convergence at risk:** successive substantive findings cluster on
  one root-cause seam, implementation/test/evidence volume grows without
  corresponding acceptance-capability growth, fixes drift toward aliases,
  synonyms, exception lists, or other local enumeration, or exact-head review
  repeatedly discovers adjacent variants of the same abstraction weakness.
  The delivery agent may finish the current bounded batch, but the next repair
  should prefer the root cause over another local patch.
- **HOLD — structurally non-convergent:** another autonomous mutation batch is
  more likely to enlarge or relocate the problem than close it. Evidence
  includes the same seam recurring after a structural repair attempt, proposed
  fixes undoing or contradicting previous valid fixes, a valid blocker requiring
  new durable authority or scope expansion, materially conflicting valid repair
  directions that need Steward reconciliation, or an implementation becoming an
  open-ended grammar/exception system rather than a bounded representation.

HOLD blocks new speculative mutation. It does not block read-only CI, hosted
review, evidence collection, or reconciliation. A pending non-terminal gate is
not itself a convergence failure. Genuine P0/P1/P2-equivalent correctness,
security, data-integrity, ownership, scope, GitHub-API, or Accepted-authority
findings remain blocking under every convergence state. Conversely, P3,
nitpick, or pure-maintainability churn does not keep a delivery loop alive.

After HOLD, the Project Steward must reconcile the durable evidence and choose
one bounded action:

1. reject or reclassify a finding when repository authority shows that it is
   invalid or outside the current acceptance criteria;
2. authorize exactly one bounded structural repair when the current Issue can
   still be satisfied without changing durable authority; or
3. freeze further mutation and route the design problem to focused
   Research/Decision work.

The bounded structural repair is a controlled recovery probe, not permission to
resume the previous loop. After the repair, and before any further autonomous
mutation, the Project Steward must update the same Steward watch with the repair
result, exact observed PR head and checked live `main`, supporting evidence, and
a fresh GREEN, AMBER, or HOLD verdict. A successful repair does not implicitly
clear HOLD. If the same root-cause seam materially recurs after that repair,
HOLD applies again and autonomous mutation stops. Moving design debt to
follow-up work never relaxes an unresolved valid blocker on the current PR; if
current acceptance cannot be met without the new decision, the current PR
remains unmergeable until the proper authority resolves or explicitly re-scopes
it.

For autonomous implementation PRs under Project Steward monitoring, maintain
exactly one separate top-level comment containing:

```text
<!-- project-steward-watch:v1 -->
```

Create this comment when Project Steward monitoring begins. Thereafter, PATCH
that same comment in place when the verdict, supporting evidence, blocker
disposition, or bounded authorized next action changes; do not add replacement
Steward-watch comments.

The Steward watch records, at minimum:

- GREEN / AMBER / HOLD convergence verdict;
- exact observed PR head and last checked live `main`;
- scope / authority verdict;
- root-cause seam or `none` and concise supporting evidence;
- current blocker disposition;
- bounded authorized next action; and
- whether human or founder input is required and, when it is required, why.

The Steward watch is operational evidence, not product or architecture
authority. It is advisory in GREEN and AMBER and blocks new mutation in HOLD.
The delivery agent must re-read it when reconciling Git state, before a new
review-fix batch, before declaring merge-ready, and while waiting on hosted
CI/review. The single `agent-handoff:v1` remains the implementation ownership
and status handoff; do not turn either narrative body into an open-ended machine
grammar.

### Post-merge recalibration and stop conditions

After every merge, refresh live `main` and re-read the Product Roadmap, open
Issues and PRs, dependencies, and relevant Accepted authority before selecting
successor work. Continue only with the next genuinely Ready Issue.

Stop and return to the Project Steward instead of speculating when:

- no genuinely Ready Issue remains;
- a durable architecture or product decision is unresolved;
- Accepted authority conflicts; or
- an external permission or service requires human action.

Do not silently activate a future roadmap horizon merely because the previous
implementation work finished.

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
