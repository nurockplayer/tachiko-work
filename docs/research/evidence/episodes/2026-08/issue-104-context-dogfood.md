---
schema: research-evidence/v0
episode: issue-104-context-dogfood
capture_mode: retrospective
capture_status: closed
captured_at: "2026-08-28T21:04:34Z"
repository: nurockplayer/tachiko-work
base_sha: 4b9c29d0245a518629c2cdb5e9628a2c6c1c37be
issue: https://github.com/nurockplayer/tachiko-work/issues/104
authority_state: Hypothesis
agent:
  interface: codex-cli 0.149.0
  provider: unknown
  model: gpt-5.6-luna
  configuration: reasoning low; fixed disabled features; fresh --ephemeral process/thread per run
context_manifest_status: exact
intervention_classes: [correction]
failure_classes: [authority_miss, authority_drift, reasoning_error, tooling_failure]
links:
  issues:
    - https://github.com/nurockplayer/tachiko-work/issues/104
    - https://github.com/nurockplayer/tachiko-work/issues/146
  prs: []
  adrs: []
  specs: []
  tests: []
  evidence:
    - https://gist.github.com/nurockplayer/7400afc71bb30989781800bf178b68a2
    - https://github.com/nurockplayer/tachiko-work/issues/104#issuecomment-5454232508
    - https://github.com/nurockplayer/tachiko-work/issues/104#issuecomment-5455440106
---

# #104 — Context dogfood benchmark

This retrospective index preserves the completed benchmark without changing
its Research / Hypothesis authority state or rewriting its result.

## Question / material decision

Could current manual authority loading be safely replaced by authority-aware
retrieval only, or by retrieval plus protected context compression, for the
frozen #104 corpus?

## Hypothesis

Retrieval might reduce task context while preserving every preregistered
authority and operational hard gate. Compression could be considered only if
it used the same retrieved canonical source set as retrieval and preserved the
protected facts.

## Baseline

Candidate A was current/manual authority loading. Candidate B was
authority-aware retrieval only. Candidate C was retrieval plus compression,
using the same retrieved canonical source set as B.

The frozen repository commit was
`4b9c29d0245a518629c2cdb5e9628a2c6c1c37be`; its Git tree was
`4bcb77c424550c5d84b6f92dbf85b874495830c0`. The Issue cutoff was
`2026-08-28T15:11:18Z`.

## Alternatives

The benchmark compared A, B, and C only. It did not authorize a production
Project Memory, RAG/vector store, or context-compression pipeline.

## Governing authority

#104 remains a Research / Hypothesis record. The repository
[knowledge-authority policy](../../../../governance/knowledge-authority.md)
governs how this research evidence is interpreted: the result cannot replace
Accepted authority without an authorized promotion.

## Context / source manifest actually supplied

The sealed run view was exact: 291 Git-tracked files, one structured sanitized
target Issue, and the result schema (293 files). Commands, configuration,
prompts, sanitized Issue, and that run view matched the recorded seal; the
evidence bundle links the reproducibility record.

This exact source manifest does not make unavailable instrumentation available.
The provider deployment revision, sampling seed, exact built-in system-prompt
content/hash, actual billing/monetary cost, model-tokenized initial task-context
subset, follow-up source-content tokens/reacquisition ratio, and instrumented
source-read coverage/provenance precision remain `unknown`.

## Initial recommendation or result

All 36 planned runs completed in fresh isolated threads. Blind scoring measured
26/36 per-run passes, 10/36 failures, and 14 critical violations. B failed the
zero-critical gate. A also had critical failures, so it was not an empirical
winner.

C's fail-closed eligibility classifier found zero eligible spans. C therefore
performed no compression and was byte-identical to B; it did not test
compression efficacy.

## Human intervention

The initial proposed run view failed the capability gate because it would have
exposed eight non-Git Issue archives. It was corrected to a Git-allowlisted
construction, resealed, and independently audited before scored runs began.

## Failures / incorrect assumptions / authority drift

The scored failures included weakened #104 Research/Hypothesis dispositions,
mis-stated Accepted authority, missed action boundaries, and one fabricated
decisive blocker inference. Thirteen worker shell commands failed across twelve
runs (mostly Git attempts in the intentionally non-Git archive); these are
recorded worker behavior, not infrastructure failures.

The scoring record also discloses one sanitized-#104 protocol omission: a
same-source-set permission probe depended on a preregistration rule excluded
from worker views to avoid gold-answer leakage. That omission was non-decisive:
removing the probe still left B with three #104 status/disposition failures.

## Corrections

The Git-allowlisted run-view correction occurred before scoring. The protocol
omission was disclosed during interpretation rather than filled with a proxy or
used to change the final verdict.

## Final outcome

**INSUFFICIENT EVIDENCE.** The manual authority-loading rule remained
unchanged because no safe replacement was proven, not because A empirically
won. Independent methodology and Project Steward reviews accepted that bounded
interpretation.

No ADR, production Project Memory implementation, RAG/vector system,
compression pipeline, or other productionization was authorized by this result.

## Traceability

- [#104 research Issue](https://github.com/nurockplayer/tachiko-work/issues/104)
- [Definitive benchmark report](https://github.com/nurockplayer/tachiko-work/issues/104#issuecomment-5454232508)
- [Project Steward review](https://github.com/nurockplayer/tachiko-work/issues/104#issuecomment-5455440106)
- [Evidence bundle](https://gist.github.com/nurockplayer/7400afc71bb30989781800bf178b68a2), evidence-index SHA-256 `e81b1182e483b09b535455c69b9a413ed3f311823d83b37e5f2f41e1b3219598`
- [#146 retrospective calibration index](https://github.com/nurockplayer/tachiko-work/issues/104#issuecomment-5457585707)

## Downstream observations

- 2026-08-28: the Project Steward accepted the bounded `INSUFFICIENT EVIDENCE`
  disposition and kept #104 Research / Hypothesis, read-only-first.
- 2026-08-28: #104 closed as completed research; no follow-on benchmark or
  productionization was created merely to force a winner.
