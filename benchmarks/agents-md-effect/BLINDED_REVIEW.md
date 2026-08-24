# Blinded Review Requirements

## What must be blinded

Quality reviewers receive an anonymized candidate checkout/diff, frozen task
statement, only the pre-cutoff `task_authority` sources and claim mappings
allowlisted in `authority-lock.json`, candidate-authored tests and
base/candidate gate outputs with machine point totals withheld, and a redacted
candidate final message. The raw final message is hash-locked for
completion/tooling scoring but is not shown. They must not receive:

- A/B/Ultra label or the `AGENTS.md` bytes/hash;
- target merge commit, historical implementation diff, or any target/outcome-only
  PR body, later test, or review thread; pre-cutoff PR/Issue text is visible only
  when explicitly allowlisted as `task_authority`;
- the other arm for the same case before submitting an independent score;
- agent identity, token count, wall time, or tool transcript unless adjudicating
  the tooling dimension after quality scoring.

Before review, an independent blinding custodian applies the hash-locked
`review-packet-blinding-v1.json` contract to every reviewer-visible path and text
artifact—not only the final message. The candidate checkout/diff, authored
tests/docs/comments, task/authority packet, validation excerpts, and final
message are scanned against the registered variant byte set with arm labels and
candidate mapping withheld. Exact/near variant text and explicit instruction or
arm references are replaced only in a detached review rendering with
`[instruction-reference redacted]`; the immutable captured candidate and scoring
evidence never change. The final-message raw/redacted hashes remain a required
subreceipt. Ordinary `TW-05` runtime-benchmark discussion is preserved unless
it independently matches a frozen rule.

Freeze the complete artifact manifest, rule/variant-set commitments, private
match-map hash, rendered packet hash, and a post-render zero-match scan. If any
reviewer-visible artifact cannot be rendered deterministically or a match
remains, record invalid/discarded terminal attempts and cancel the pair/case
slot without resampling the model. A visible placeholder preserves evidence of
candidate-authored instruction copying without revealing which variant matched.

Review is packet-only and offline. Each reviewer receives one anonymous packet
in a fresh isolated workspace with no network, GitHub remote, current
repository, controller/evaluator bundle, arm key, other run, or paired candidate
available. The reviewer may inspect only the allowlisted base authority and
candidate evidence in that packet and writes the score sheet to a single
controller-owned output channel. Freeze a reviewer-environment receipt proving
the packet hash, network denial, absent remotes/forbidden mounts, anonymous
identity, and process isolation. Before a wave, a negative qualification must
show that reviewer attempts to reach GitHub, current history, controller files,
or another candidate fail.

Allowlisted authority explains the frozen assignment; reviewers may not use it
to expand requirements beyond the task and the registered claim-to-contract
mapping.

## Cases requiring blinded review

All cases receive maintainability, authority, scope, and churn review. The
following require additional substantive review:

- `TW-01`: whether the ADR separates durable identity invariants from the
  provisional generator and reconciles conflicting authority.
- `TW-02`: whether the review remediation is complete without silently creating
  new storage decisions.
- `TW-05`: whether the spike methodology supports its conclusions and labels
  throwaway mechanisms honestly.
- `TW-06`: whether public pre-alpha status, licensing authority, contribution
  boundaries, and active guidance are reconciled without changing licenses or
  release/runtime machinery.
- `TW-07`: whether complete formula outcomes implement the accepted semantics
  when an alternative internal API needs an evaluator adapter.
- `TW-08`: whether the compatibility path remains a bounded legacy migration
  seam rather than leaking historical DTOs or permissive decoding into the
  canonical model.
- `TW-09`: whether diagnostic facts, suppression, and ownership follow ADR-0019
  without stabilizing a public wire contract.

## Reviewer eligibility

Before assignment, every quality reviewer and adjudicator signs a
case-specific conflict/prior-knowledge attestation. A person is ineligible for a
case if they:

- constructed, selected, relocked, or audited that benchmark case;
- authored or reviewed its historical Issue, PR, target patch, tests, or review
  findings;
- can access the oracle/controller bundle, historical outcome-only evidence,
  arm key, `AGENTS.md` variants, agent transcript, or paired candidate;
- ran either candidate, wrote or reviewed its oracle adapter, or performed its
  capture/integrity validation; or
- otherwise remembers implementation-specific facts that could identify the
  historical solution or one experiment arm.

The custodian records reviewer ID, case ID, attestation version, signed
yes/no answers, eligibility decision, and attestation hash before releasing a
packet. Undisclosed prior exposure invalidates every affected review. Benchmark
constructors, historical PR participants, oracle custodians, adapter-integrity
reviewers, and experiment operators may not serve as quality reviewers for the
same case.

## Pre-run neutrality audit roles

These are readiness roles, not candidate quality reviewers:

- A variant-blind non-constructor receives the case-selection ledger, frozen
  historical authority, tasks, rubric, capability clusters, and contamination
  report, but no instruction variant. They audit authenticity, solution leakage,
  later-knowledge dependence, duplication, and generic rubric neutrality.
- After benchmark and instruction bytes are immutable, a separate comparison
  auditor receives the frozen tasks/rubric plus instruction text with arm labels
  removed. For standalone Baseline A they see frozen A; for controlled A/B they
  see both variants in randomized order. They audit lexical/semantic overlap,
  authority cueing, tooling favoritism, and asymmetric scoring incentives.

Both roles sign conflict/no-edit/no-outcome attestations. Neither may propose or
perform changes to the benchmark or instruction text. A material finding cancels
the planned wave; it is not repaired after seeing an outcome. Their identities,
input-packet hashes, decisions, and signed receipt hashes enter the wave registry
and every valid result.

## Reviewer process

1. Two reviewers independently score the case's frozen
   `review_points_available` (57, 63, 69, or 81) and write one evidence sentence per
   reviewer-controlled dimension and functional group.
2. Reviewer IDs and score sheets are frozen before variant labels are revealed.
   Each sheet binds its reviewer-environment receipt.
3. Commit a 32-byte CSPRNG seed and each attempt's opaque candidate ID before
   provider/task launch. After candidate evidence freezes,
   `reviewer-allocation-v1.json` deterministically assigns
   exact two-person panels, adjudicator priority, and non-adjacent packet order
   from the frozen eligible pools without arm labels or operator choice. Record
   the allocation and independent replay receipts. In controlled A/B, no primary
   reviewer or adjudicator may receive both candidates from a pair. A standalone
   Baseline A uses one eligible panel per case.
4. A difference greater than 4 reviewer points, any mandatory-group status
   mismatch, any outcome-class mismatch, or any hard-failure disagreement
   triggers adjudication by a third reviewer.
5. Each adjudicator writes a separate schema-valid score sheet with
   `review_role="adjudicator"` and sees only one anonymous candidate and that candidate's two
   rationales; they remain blind to variant, the paired candidate, and historical
   implementation. The opposite arm uses a different eligible adjudicator if
   both require adjudication.
6. Final subjective points are the mean of the exact two primary score sheets,
   or the median of those two plus the bound adjudicator sheet after
   adjudication. The semantic controller recomputes every functional-group,
   dimension, and total component from the three content-addressed sheets.
7. Only after all case scores are frozen may the experiment owner join scores
   to variant labels, runtime, and token/cost data.

## Oracle adapter rule

When an evaluator-owned historical test assumes a non-normative API name or
private layout, a separate oracle custodian who is blind to variant may write
the smallest adapter needed to call the candidate's equivalent interface. The
quality reviewers remain historical-test-blind. The adapter:

- may translate names and types only;
- may not implement missing behavior;
- must be saved in full, with its byte hash and any evaluator-test diff; adapters
  may differ only where candidate interfaces differ, while the same normalized
  contract, custodian instructions, and validator apply to both arms;
- must exercise candidate-owned production behavior rather than emit or derive
  the locked expected observations itself;
- must receive an independent adapter-integrity review by someone other than
  the custodian and quality reviewers; the signed review hash is part of the
  result record;
- is recorded in `oracle_adapter` in the result schema.

The oracle custodian may see the locked oracle/contract and candidate interface,
but not `AGENTS.md`, arm labels, the paired candidate, scores, or historical
implementation code beyond the locked tests. The integrity reviewer sees only
the adapter, normalized contract, allowed interface surface, and candidate
production calls. Neither person awards quality points.

For controlled A/B, oracle custodians and adapter-integrity reviewers are also
disjoint across the two arms of a case. A pair-level blinding-role registry must
prove that no quality reviewer, adjudicator, custodian, or integrity reviewer
appears on both sides before either adapter outcome or quality score is joined
to an arm label.

Before a controlled wave, every adapter family must pass a construction-only
qualification on the historical ground truth and a negative control that lacks
the required behavior. For candidate-specific test-source adaptation, the
review must prove the diff changes only imports, names, constructors, or type
conversion—not assertions, expected values, scenarios, ordering, or failure
classification. An unqualified adapter family blocks the run.

The unadapted compile failure remains evidence about compatibility but is not
automatically functional failure unless the interface was required by the
frozen task or base authority.
