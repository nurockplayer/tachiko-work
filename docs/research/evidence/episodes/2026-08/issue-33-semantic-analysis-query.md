---
schema: research-evidence/v0
episode: issue-33-semantic-analysis-query
capture_mode: prospective
capture_status: active
captured_at: "2026-08-28T20:45:57Z"
repository: nurockplayer/tachiko-work
base_sha: be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9
issue: https://github.com/nurockplayer/tachiko-work/issues/33
authority_state: Open Question
agent:
  interface: unknown
  provider: unknown
  model: unknown
  configuration: unknown
context_manifest_status: partial
intervention_classes: [none]
failure_classes: []
links:
  issues:
    - https://github.com/nurockplayer/tachiko-work/issues/31
    - https://github.com/nurockplayer/tachiko-work/issues/32
    - https://github.com/nurockplayer/tachiko-work/issues/33
    - https://github.com/nurockplayer/tachiko-work/issues/144
    - https://github.com/nurockplayer/tachiko-work/issues/146
  prs:
    - https://github.com/nurockplayer/tachiko-work/pull/143
    - https://github.com/nurockplayer/tachiko-work/pull/145
  adrs:
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/decisions/ADR-0015-stable-semantic-identity.md
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/decisions/ADR-0020-first-class-headless-semantic-api.md
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/decisions/ADR-0026-scoped-semantic-authorization-and-approval.md
  specs:
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/specs/semantic-api.md
    - https://github.com/nurockplayer/tachiko-work/blob/be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9/docs/specs/semantic-authorization.md
  tests: []
  evidence:
    - https://github.com/nurockplayer/tachiko-work/issues/33#issuecomment-5456640622
    - https://github.com/nurockplayer/tachiko-work/issues/33#issuecomment-5456968918
    - https://github.com/nurockplayer/tachiko-work/issues/33#issuecomment-5457583307
---

# #33 — Semantic analysis query

This prospective capture preserves a research recommendation before an
authority/specification PR can promote any part of it into Accepted authority.
It does not amend #33's substantive decision scope.

## Question / material decision

Does M04 need a generic-enough Semantic Analysis Query family, or are #31 plus
#32/#144's domain-specific read-only, formula, and scenario Query surfaces
sufficient?

## Hypothesis

A small, typed, provider-neutral Analysis Query family may be needed for
authoritative population selection, typed predicate evaluation, grouping, and
bounded aggregate reduction, without promoting a general analytics or query
language.

## Baseline

The baseline is #31 plus #32/#144's domain-specific read-only, formula, and
scenario Query surfaces. The capture is anchored to live `main` at
`be18eb2a6ef44718ab4c18b7e69caf9424f7d1c9`.

## Alternatives

The retained decision space is:

1. no new generic family;
2. a bounded minimal family; or
3. a richer general analytics/query surface, explicitly outside the intended
   M04 boundary.

## Governing authority

The live governing authority at capture was ADR-0015, ADR-0018, ADR-0019,
ADR-0020, ADR-0026,
[the Semantic API specification](../../../../specs/semantic-api.md), and
[the Semantic Authorization specification](../../../../specs/semantic-authorization.md),
together with the relevant #31, #32/#143, and #144/#145 implementation
evidence. The #33 Decision Issue remains an Open Question until an authorized
authority/specification decision is accepted.

## Context / source manifest actually supplied

The exact source manifest supplied to the #33 research agent is only
**partial** in durable evidence. The #33 Issue and its research comments
confirm the material question, baseline, alternatives, governing authorities,
and recommendation. They do not prove that every listed authority or
implementation artifact was supplied to the agent, nor do they preserve a
complete prompt/context manifest.

Provider, model, interface, configuration, deployment revision, sampling, and
built-in system-prompt metadata are `unknown`. The governing-authority list is
not reconstructed as a fictional exact prompt manifest.

## Initial recommendation or result

The recorded research recommendation is **PROMOTE MINIMAL ANALYSIS QUERY
CONTRACT**. It says that #31 + #32/#144 do not own authoritative schema-wide
selection, typed predicates, grouping, or aggregate reduction, and recommends
a bounded logical family rather than a generic analytics/query language.

This is research/decision evidence, not a final Accepted outcome.

## Human intervention

No human override or correction was observed in durable evidence at capture.

## Failures / incorrect assumptions / authority drift

No failure was observed in the captured evidence. The pending-promotion state
must not be mistaken for Accepted product authority.

## Corrections

No correction was recorded at capture. Future corrections, if evidenced, are
appended chronologically rather than altering this snapshot.

## Final outcome

Pending. At capture, #33 remained an Open Question / Decision Issue pending
promotion. A research recommendation does not become the final Accepted
decision unless an authority/specification decision lands.

## Traceability

- [#33 Decision Issue](https://github.com/nurockplayer/tachiko-work/issues/33)
- [Project Steward research handoff](https://github.com/nurockplayer/tachiko-work/issues/33#issuecomment-5456640622)
- [Deep Research decision](https://github.com/nurockplayer/tachiko-work/issues/33#issuecomment-5456968918)
- [Prospective anchor](https://github.com/nurockplayer/tachiko-work/issues/33#issuecomment-5457583307)
- [#31 baseline](https://github.com/nurockplayer/tachiko-work/issues/31), [#32 decision](https://github.com/nurockplayer/tachiko-work/issues/32), [PR #143](https://github.com/nurockplayer/tachiko-work/pull/143), [#144 implementation](https://github.com/nurockplayer/tachiko-work/issues/144), and [PR #145](https://github.com/nurockplayer/tachiko-work/pull/145)
- [#146 capture Issue](https://github.com/nurockplayer/tachiko-work/issues/146)

## Downstream observations

- 2026-08-28: prospective anchor recorded before an authority/specification PR
  promoted the recommendation. No later decision outcome is recorded in this
  episode at capture.
