# PR Decomposition Policy

This policy refines the issue-driven repository delivery workflow in
[`project-governance.md`](project-governance.md#repository-delivery-workflow).
It does not replace that workflow or its one-Ready-Issue -> one-PR rule.

Origin: [Issue #294](https://github.com/nurockplayer/tachiko-work/issues/294)

## Purpose

A Ready implementation Issue must be small enough to remain one coherent,
independently reviewable pull request. Large product goals may therefore need to
be decomposed into multiple child implementation Issues **before** any child is
marked Ready.

The governing principle is:

> **Decompose before Ready, not halfway through implementation.**

A parent/product Issue may coordinate several child implementation Issues. Each
Ready child still owns exactly one independently reviewable PR under the
canonical delivery workflow.

## Pre-Ready decomposition gate

Before marking an implementation Issue Ready, the Project Steward checks whether
its likely PR is one coherent review unit. Decompose the work when independently
valuable concerns can be separated without leaving `main` in a knowingly broken
or misleading state.

Prefer **vertical slices**: each child should establish a useful behavior or
contract that can be tested and reviewed on its own. Do not split work merely by
file, layer, helper, type, or other horizontal scaffolding when that would leave
half-features on `main`.

The Steward should consider:

- whether the Issue contains multiple independently testable outcomes;
- whether reviewers would need materially different mental models for distinct
  parts of the change;
- whether import, persistence, semantic behavior, export, UI, migration,
  compatibility, release evidence, or other concerns can be validated as
  separate coherent slices;
- whether failure or rollback of one concern should be isolated from the others;
- the projected review surface, including changed files and non-generated diff.

## Size signals

Size is a reviewability heuristic, not a correctness rule.

| Projected review surface | Steward action |
| --- | --- |
| More than about 25 changed files | Re-evaluate decomposition before Ready. |
| More than about 2,000-3,000 non-generated changed LOC | Re-evaluate decomposition before Ready. |
| Multiple independently testable concerns or distinct review mental models | Prefer decomposition even when the diff is smaller. |
| More than about 50 changed files | Strong decomposition signal; keeping one Issue/PR requires explicit justification. |
| More than about 5,000 non-generated changed LOC | Strong decomposition signal; keeping one Issue/PR requires explicit justification. |

For this policy, changed LOC means additions plus deletions that a reviewer must
meaningfully inspect. Generated outputs, vendored snapshots, lockfile churn,
fixtures, mechanical migrations, and similar material may be excluded when the
Issue or PR clearly identifies why they do not add equivalent review complexity.
Their correctness and repository gates still remain required.

A small diff may still require decomposition when it mixes separable concerns.
A large mechanical change may remain one PR when splitting it would reduce
correctness or reviewability rather than improve it.

## Exceptions

Keeping a change above a strong size signal in one Issue/PR is acceptable only
when the work is still one coherent review unit and the Issue or PR records the
reason. Typical bounded exceptions include:

- generated or machine-maintained artifacts;
- fixture or golden-data expansion tied to one contract;
- mechanical migrations with one transformation rule;
- repository-wide renames or equivalent repetitive edits; and
- inseparable atomic changes where intermediate merges would knowingly break a
  supported contract or required gate.

"The agent can implement it in one run" is not a justification.

## Unexpected growth after Ready

A Ready Issue may reveal more work than expected. When implementation starts to
cross into an adjacent independently reviewable concern, the delivery agent must
not silently absorb that scope merely to finish the original product goal.

Scope reconciliation here is a **non-terminal step inside the active delivery
loop**. It does not by itself satisfy a canonical stop condition, end the agent
run, or transfer ownership of the active Issue/PR.

Instead:

1. stop speculative expansion into the adjacent concern;
2. preserve the current coherent work and evidence in repository-approved
   durable state;
3. ask the Project Steward to reconcile the scope boundary while the active
   delivery loop remains live; and
4. resume the bounded delivery loop only after the Steward records one of these
   outcomes:
   - the remaining work is inseparable and still bounded, so it remains in the
     active Issue/PR; or
   - the adjacent concern is outside the active Issue's current acceptance
     criteria, so it moves to follow-up/child Issue(s); or
   - the adjacent concern is currently required by the active Issue, and the
     Steward explicitly re-scopes that Issue and its acceptance criteria before
     moving the concern to follow-up/child Issue(s).

A follow-up Issue never, by itself, removes an existing acceptance requirement
from the active Issue. Re-scoping must be explicit and must not be used to evade
a valid correctness, security, data-integrity, Accepted-authority, or other
blocking requirement.

Only if reconciliation exposes an existing canonical stop condition -- for
example an unresolved durable architecture/product decision, Accepted-authority
conflict, or external permission requirement -- does the normal stop/escalation
contract apply. Creating a follow-up Issue for separable work is not itself a
reason to abandon or prematurely stop an otherwise valid active PR.

Do not split an already-active PR into arbitrary fragments solely to satisfy a
number. The purpose of this policy is to prevent oversized review units through
early scope design, not to create churn after the fact.

## Relationship to the delivery workflow

This policy preserves these invariants:

- one Ready implementation Issue owns one PR;
- one PR does not bundle unrelated Issues;
- parent/product tracking Issues may coordinate several child implementation
  Issues and do not themselves need an implementation PR;
- all Ready, review, validation, handoff, merge, and post-merge rules in the
  canonical repository delivery workflow continue to apply independently to
  each child Issue/PR.

When a decomposition choice would require a new durable product or architecture
decision, resolve that authority first; this policy does not authorize delivery
agents to invent new product scope or architectural contracts.
