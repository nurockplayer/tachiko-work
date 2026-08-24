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

## Compatibility / migration

<!-- If applicable. -->

## Out of scope

<!-- Prevent implementation convenience from expanding the contract. -->

## Decision discovery rule

If implementation exposes a new expensive-to-reverse public, persistence, compatibility, or semantic contract that is not already authorized, stop that part of the work and return it to focused decision/research work rather than silently freezing it in code.
