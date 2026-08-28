# Research Evidence Capture schema v0

`research-evidence/v0` is a small Git-versioned Markdown convention. Its YAML
frontmatter is machine-parseable, while the body remains the human-readable,
diffable reconstruction record. It is repository metadata only, not a Tachiko
product schema or a compatibility promise.

## Frontmatter

Each episode uses this metadata surface:

```yaml
---
schema: research-evidence/v0
episode: stable-repository-local-slug
capture_mode: prospective
capture_status: active
captured_at: "2026-08-28T20:45:57Z"
repository: nurockplayer/tachiko-work
base_sha: exact-repository-commit-or-unknown
issue: https://github.com/nurockplayer/tachiko-work/issues/0
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
  issues: []
  prs: []
  adrs: []
  specs: []
  tests: []
  evidence: []
---
```

`episode` is a stable repository-local slug. `capture_mode` is `prospective`
or `retrospective`; `capture_status` is `active` or `closed`; and
`authority_state` uses the existing repository decision-state vocabulary.
`base_sha` is the exact repository commit when known, otherwise the literal
`unknown`.

`context_manifest_status` is `exact`, `partial`, or `unknown`. It describes
how completely the episode can establish the context actually supplied, not
how many governing sources happen to be known now. `links` is an index to
durable evidence, not a second authority graph.

## Unknown and empty values

`unknown` is a real literal value. Do not replace unavailable provider,
backend, model, deployment, sampling, system-prompt, billing, configuration,
or historical-context information with inference, an empty value, or `null`.
An empty `failure_classes` list means no failure was observed in the captured
evidence. Use `[unknown]` when evidence records a failure but cannot support a
more specific class. For a known no-intervention episode, use
`intervention_classes: [none]`.

## Required sections

Every episode contains these sections, in this order, so its evidence can be
understood without private chat history:

1. Question / material decision
2. Hypothesis
3. Baseline
4. Alternatives
5. Governing authority
6. Context / source manifest actually supplied
7. Initial recommendation or result
8. Human intervention
9. Failures / incorrect assumptions / authority drift
10. Corrections
11. Final outcome
12. Traceability
13. Downstream observations

The context section distinguishes confirmed supplied sources from durable
records that are only reconstructed or uncertain. Later observations are
appended chronologically; they do not overwrite the initial recommendation or
outcome.

## Provisional taxonomy

Use a class only when captured evidence supports it. The taxonomy is
provisional and should grow only when real episodes require it.

Intervention classes:

- `none`
- `clarification`
- `constraint_addition`
- `decision`
- `override`
- `correction`
- `rollback`

Failure classes:

- `authority_miss`
- `authority_drift`
- `context_omission`
- `dependency_miss`
- `scope_expansion`
- `premature_abstraction`
- `implementation_assumption`
- `security_boundary_error`
- `reasoning_error`
- `tooling_failure`
- `unknown`
