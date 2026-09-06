# Delivery Throughput Policy

Status: Accepted governance policy when merged

Decision issue: [#307](https://github.com/nurockplayer/tachiko-work/issues/307)

## Purpose

This policy increases delivery throughput without weakening proof where failure is expensive. It supplements the canonical [Repository delivery workflow](project-governance.md#repository-delivery-workflow) and keeps the existing one-Ready-Issue -> one-PR decomposition rule.

The governing principle is:

> Low-risk implementation optimizes for flow; high-risk contracts optimize for proof; milestones re-establish integrated confidence.

This policy does not bypass repository-required checks, branch protection, exact-head validation, Accepted authority, or the Project Steward's stop/escalation rules.

## Delivery risk class

Before an implementation Issue becomes Ready, the Project Steward records one delivery risk class. Classify by the highest applicable risk, not by diff size alone.

### Fast

Use **Fast** when the work is isolated, reversible, and does not change a durable contract. Typical examples are bounded UI behavior, copy/docs, test-fixture cleanup, mechanical refactors, or isolated bugs with stable acceptance and no guarded trigger.

### Standard

Use **Standard** for ordinary product work that may cross several modules but implements a stable Accepted or explicitly bounded Provisional contract and has no guarded trigger.

### Guarded

Use **Guarded** when a mistake can create durable incompatibility, silent corruption, difficult recovery, security exposure, or governance drift. Guarded triggers include material changes to:

- persistence, storage, serialization, file formats, or import/export correctness;
- schema or migration behavior;
- core semantic, identity, validation, formula, or calculation contracts;
- revision, history, merge, recovery, or conflict semantics;
- authentication, authorization, security boundaries, destructive operations, or data-loss behavior;
- public APIs, wire formats, backward compatibility, SDK/plugin contracts, or release compatibility;
- repository governance, CI/release gates, branch policy, or other rules that control what may ship.

A material authority contradiction or unresolved expensive-to-reverse choice is not merely Guarded implementation. Route it back to Research/Decision work until production readiness can be established.

When classification is genuinely uncertain, use Guarded until the uncertainty is resolved.

## Review route

Every final PR head still receives independent review appropriate to its risk. Independence is about authorship and evidence, not a particular provider name.

- **Fast:** the orchestrator or another reviewer who did not author the relevant change may perform final independent review after targeted validation. A separate high-cost deep-review agent is not mandatory.
- **Standard:** independent final-head review is required. The orchestrator may satisfy it when it remained outside both implementation and acceptance/evidence authorship and can inspect the complete diff and evidence. Escalate to a deep reviewer when uncertainty or substantive findings justify it.
- **Guarded:** require a fresh independent deep review of the exact final material head. Review must cover implementation, acceptance adequacy, relevant unit tests, and durable-contract risk.

A valid P0/P1/P2-equivalent finding, authority contradiction, unexplained data-integrity risk, or repeated non-convergence escalates the lane regardless of its initial class. Reclassification never makes a known blocker mergeable.

At a milestone or release boundary, perform a fresh independent integrated review across the merged milestone/release surface even when constituent Fast or Standard PRs used lighter review routes.

## Parallel Ready lanes

The Project Steward or designated orchestrator may run up to **three** Ready implementation lanes concurrently when parallelism is real rather than nominal.

Parallel lanes require:

- distinct owning Issues, branches, PRs, and canonical handoffs;
- one active writer per branch/PR;
- explicit, non-overlapping primary write surfaces or an integration order that prevents competing mutation;
- no shared unresolved durable decision or unstable semantic contract;
- a clear owner for integration/rebase conflict resolution.

Do not parallelize two mutation lanes merely because separate agents are available. If they begin competing for the same state model, schema, semantic boundary, generated artifact, or hotspot, pause and repartition or serialize them.

The orchestrator coordinates ownership, dependency order, and integration. Sub-agents should spend their time implementing bounded work rather than repeatedly reconstructing global scheduling state.

## Material checkpoints

GitHub remains the shared coordination and audit surface, but checkpointing is for **material stage transitions**, not every local edit or test invocation.

For a normal implementation lane, checkpoint when applicable at:

1. **START / ownership established** — exact Issue, branch/PR, base/head, scope boundary, risk class, and next stage;
2. **IMPLEMENTATION COMPLETE** — bounded implementation is complete enough for the planned validation surface;
3. **VALIDATION COMPLETE** — applicable acceptance/unit/document/repository evidence for the current head is recorded;
4. **REVIEW FINDINGS RESOLVED** — substantive review findings or Steward challenges are dispositioned on an exact head;
5. **READY TO MERGE / STOP** — terminal exact-head gate results establish merge readiness, or a canonical escalation/stop reason is recorded. Pending CI, hosted review, or other non-terminal asynchronous validation is recorded while the lane remains active and continues waiting.

A stage that does not exist for the task does not need an artificial checkpoint. Multiple local edit-test-fix loops inside one material stage do not require a GitHub write and authority re-read after each iteration. At each required material checkpoint, PATCH the canonical handoff/watch comments in place with the then-current exact head and checked live `main`; a local commit inside an unchanged stage has no standalone GitHub-write requirement.

Re-read live Issue/PR authority and current Steward guidance at material boundaries, after an external authority change, before resuming mutation from HOLD, and before declaring merge readiness. Quiet local iteration inside an unchanged authorized stage does not require busy-wait synchronization.

Keep the existing `agent-handoff:v1` and `project-steward-watch:v1` schemas unchanged and update their canonical comments in place.

## Validation layering

Validation should shorten the inner implementation loop while preserving exact-head confidence.

### Iteration

Run the smallest targeted unit, acceptance, integration, lint, build, or document checks that can disprove the current change quickly. Do not repeatedly run an expensive whole-workspace suite after every local edit unless the risk surface requires it.

### PR final head

Before merge, run the applicable Steward-authored acceptance surface, applicable delivery-agent unit tests, and all repository-required or risk-applicable checks against the exact final head. For each non-applicable acceptance or unit-test class, the owning Issue must contain a separate explicit bounded applicability exception decided by the Steward and identify the document/refactor checks and review evidence that replace that class; an exception never waives the other applicable class. Fast/Standard classification never waives a required hosted check.

### Guarded, milestone, and release gates

Run the broader regression/compatibility surface appropriate to the affected subsystem. At milestone or release boundaries, run the repository's full integrated regression/release gate where available, or explicitly record the bounded substitute when an external requirement cannot be executed in the current environment.

Never present an unexecuted check as PASS and never weaken required CI merely to reduce cycle time.

## Stable-contract execution freeze

Once a relevant contract is Accepted and sufficient for the Ready Issue, implementation agents should treat it as frozen for that delivery lane. Do not reopen foundational semantics merely because another design is attractive or implementation is difficult.

Reopen the contract only when concrete evidence shows a contradiction, missing required behavior, unacceptable migration/compatibility risk, or another existing Steward escalation condition. Otherwise finish the implementation against the current authority and capture optional redesign ideas separately.

This freeze is an execution rule, not a claim that Accepted decisions can never be amended through the normal decision process.

## Acceptance-first ownership

The Project Steward owns the bounded specification and executable acceptance handoff. The delivery agent owns production implementation and its unit tests. Evidence-based challenges follow the canonical reconciliation path in `project-governance.md`.

This split is intended to reduce implementation search space: the delivery agent should usually be able to drive from a concrete failing/target acceptance boundary to a passing exact head without inventing the product contract during implementation.

## Rollout

Apply this policy prospectively to newly Readied work and at the next material checkpoint of already active lanes. Do not restart sound active work solely to rename its risk class.

Existing repository authority remains in force. Where an older Issue prompt hard-codes a stronger review or validation route, obey that Issue until the Project Steward explicitly reconciles it; do not silently downgrade an already-dispatched contract.

After rollout, prefer measuring product throughput by completed user-visible capability and milestone convergence, not by raw Issue count or agent activity.
