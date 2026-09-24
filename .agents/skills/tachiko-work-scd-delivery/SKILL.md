---
name: tachiko-work-scd-delivery
description: Run or resume an active Tachiko Work Stewarded Continuous Delivery loop for a Ready Issue, including async continuity, durable handoff recovery, and canonical stop conditions. Use when implementing, resuming, or supervising an active Ready Issue/PR under SCD.
---

# Tachiko Work SCD Delivery

Use this Skill only for an active or resumed SCD delivery lane. It is an operating procedure, not an authority source.

Before acting, read the repository `AGENTS.md`, the canonical [Repository delivery workflow](../../../docs/governance/project-governance.md#repository-delivery-workflow), the owning Issue/PR, and the canonical `agent-handoff:v1` when present. Higher repository governance, accepted specs/ADRs, and explicit Issue scope always win.

## Continue the active lane

For a Ready Issue with an active agent-owned PR, continue the bounded one-Issue delivery loop autonomously until a canonical stop or escalation condition is actually reached.

A pending non-terminal sub-agent result, CI job, hosted review, or other asynchronous validation is not a completion, handoff, or stop condition. When the execution environment can remain active, wait or poll for the result, consume it when available, and continue the same review-fix / exact-head-validation loop.

Runtime liveness is separate from delivery state. If an agent run may terminate before a canonical stop, make continuation recoverable outside ephemeral local state first:

- keep the single canonical `agent-handoff:v1` exact;
- persist meaningful active work to Git or another repository-approved durable artifact when necessary;
- never leave the only copy of progress as an uncommitted local diff.

If a run terminates before a canonical stop, the scheduler or orchestrator is responsible for re-entering the same Issue/PR from that durable handoff/checkpoint. A human copying and pasting a `continue` prompt is not part of the intended workflow.

Intermediate progress may update the single canonical `agent-handoff:v1`, but do not present intermediate progress as task completion merely because an asynchronous gate is still running or one agent runtime ended.

## Stop conditions

Return control only when one of the canonical repository delivery workflow conditions applies:

- no genuinely Ready Issue remains after live-state recalibration;
- an unresolved durable architecture or product decision exists;
- Accepted authority conflicts;
- an external permission or service requires human action.

Do not manufacture work to avoid a legitimate stop condition.

## Discipline

Preserve one Issue / one PR ownership and the repository's Ready, validation, review, and merge gates. This Skill does not grant readiness, architecture authority, merge authority, or permission to weaken evidence.
