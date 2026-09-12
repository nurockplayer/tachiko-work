# Stewarded Continuous Delivery

Status: terminology and operating-mode note; no new delivery authority

Tracking issue: [#370](https://github.com/nurockplayer/tachiko-work/issues/370)

## Name

**Stewarded Continuous Delivery (SCD)** is the project name for the governed operating mode that continuously advances genuinely Ready repository work while preserving Project Steward authority, exact-head evidence, independent review, and live-state recalibration.

The name describes the existing repository delivery model. It does not create a second workflow, state machine, readiness rule, or merge policy.

The canonical authority remains the [Repository delivery workflow](project-governance.md#repository-delivery-workflow), together with the [Delivery Throughput Policy](delivery-throughput-policy.md), PR Decomposition Policy, Accepted ADRs/specifications, and each live Issue/PR.

## Provider-neutral role model

SCD separates responsibilities rather than binding them to a particular model, vendor, or tool:

- **Project Steward** — owns authority, roadmap/readiness, bounded acceptance, durable decisions, escalation, and post-delivery reconciliation.
- **Mission Lead / orchestrator** — coordinates live delivery lanes, delegates bounded work, preserves one-writer ownership, and repeatedly re-reads live authority.
- **Implementation worker** — implements one authorized bounded slice and adds implementation-owned tests/evidence.
- **Independent reviewer** — reviews the exact final material head according to the applicable risk class and remains independent of the relevant implementation/acceptance authorship.
- **GitHub** — remains the durable coordination, checkpoint, review, and audit surface.

A current deployment may map these roles to particular agents or models, but those assignments are operational choices beneath SCD rather than part of its durable definition.

## Operating loop

Conceptually:

```text
live authority + genuinely Ready work
        ↓
Steward acceptance / readiness
        ↓
Mission Lead selects a non-conflicting lane
        ↓
implementation worker delivers bounded change
        ↓
applicable validation + independent exact-head review
        ↓
merge / closeout under repository authority
        ↓
live-main recalibration
        ↓
select the next genuinely Ready lane, or stop/escalate
```

The Mission Lead may continue across multiple Issues only by completing the current bounded lane and recalibrating live state between them. An Issue being open, interesting, queued, deferred, or already specified does not make it Ready.

## Material GitHub checkpoints

Use the existing canonical `agent-handoff:v1` and, where applicable, `project-steward-watch:v1` surfaces. Do not add an SCD-specific machine schema.

At material delivery stages, record the exact HEAD/main identities, evidence actually executed, unresolved findings or blockers, and the next authorized action. Existing Issue-specific checkpoint names such as `TEST_CONTRACT_READY`, `IMPLEMENTATION_GREEN`, `INDEPENDENT_REVIEW_COMPLETE`, `DELIVERY_CLOSEOUT`, and `BLOCKED` remain narrative conventions where their owning Issue requires them; SCD does not replace the canonical handoff/watch headers.

## Stop conditions

SCD optimizes continuity, not perpetual agent activity. Stop or return to the Project Steward when live authority says work cannot safely continue, including when:

- no genuinely Ready, non-conflicting Issue remains;
- a durable architecture/product decision is unresolved;
- Accepted authority conflicts with the requested implementation;
- a human, legal, contractual, credential, billing, or external-service action is required;
- a valid blocking review or convergence HOLD cannot be resolved within the current authorized scope.

Never infer readiness, weaken acceptance, bypass a human/legal gate, or manufacture work merely to keep a delivery session active.

## Historical note

The operating pattern became especially visible while running a long autonomous delivery sequence in the Richman4 remake project. That project is historical provenance for the working style, not the name of the reusable Tachiko Work process. Historical Richman4 Issues/PRs that genuinely refer to that project should keep their original names.
