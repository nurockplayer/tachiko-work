---
name: Implementation
about: Implement an Accepted or explicitly Provisional contract without silently redesigning it
title: ""
labels: ""
assignees: ""
---

## Purpose

<!-- What bounded behavior should be implemented? -->

## Traceability

- Authority: <!-- governing ADR / policy / accepted spec / explicit Provisional choice -->
- Implements: <!-- decision/spec/parent issue -->
- Related: <!-- neighboring work -->

## Current behavior

<!-- What exists now? -->

## Desired behavior

<!-- Observable target behavior. -->

## Locked boundaries

<!-- Identity/storage/formula/validation/API/etc. decisions that must not be reopened here. -->

## Required behavior

<!-- Concrete contract requirements. -->

## Failure behavior

<!-- Expected diagnostics/errors/atomicity/fail-closed behavior where relevant. -->

## Acceptance criteria

<!-- Observable completion conditions. -->

## Validation / evidence

<!-- Tests, fixtures, CI, benchmarks, compatibility evidence. -->

## Acceptance-test handoff

<!-- Complete before production Ready; follow docs/governance/project-governance.md#acceptance-first-preparation-and-handoff. Link evidence instead of copying the policy. -->

- Steward / acceptance author: <!-- owner -->
- Baseline / seed: <!-- full commits, test/fixture paths, existing branch/PR -->
- Criterion-to-case mapping: <!-- concrete test cases and independent expected outcomes -->
- Commands / baseline evidence: <!-- actual results; distinguish behavioral failure from unexecuted/setup failure -->
- Remaining checks / applicability: <!-- manual/external evidence and owner, or explicit bounded Steward exception with rationale -->

<!-- The delivery agent writes implementation and unit tests. Evidence-based challenges use the existing handoff narrative; material acceptance changes need a linked Steward decision. -->

## Compatibility / migration

<!-- If applicable. -->

## Out of scope

<!-- Prevent implementation convenience from expanding the contract. -->

## Decision discovery rule

If implementation exposes a new expensive-to-reverse public, persistence, compatibility, or semantic contract that is not already authorized, stop that part of the work and return it to focused decision/research work rather than silently freezing it in code.
