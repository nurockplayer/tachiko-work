# Tachiko Work AGENTS.md Effect Benchmark

This directory defines a replay benchmark for measuring how an `AGENTS.md`
variant affects single-agent repository work in Tachiko Work.

The benchmark has four strictly separate phases:

1. benchmark construction and construction-only pilots;
2. controlled single-agent Baseline A execution;
3. a future controlled single-agent A/B experiment using a frozen Variant B;
4. an optional, separately reported Ultra autonomous-performance evaluation.

Nothing produced by construction or an Ultra run is a Baseline A or Variant B
result. This benchmark does not propose or optimize an `AGENTS.md` variant.

## Artifacts

- [`PROTOCOL.md`](PROTOCOL.md) — experimental design and isolation rules.
- [`CASE_MANIFEST.md`](CASE_MANIFEST.md) — final nine-case overview.
- [`tasks/`](tasks/) — the only case-specific text visible to an agent.
- [`evaluator/cases.json`](evaluator/cases.json) — evaluator-only provenance,
  hidden risks, and validation details.
- [`SCORING.md`](SCORING.md) — variant-neutral 100-point rubric and outcome
  thresholds.
- [`PROCEDURES.md`](PROCEDURES.md) — exact Baseline A, future A/B, and optional
  Ultra procedures.
- [`BLINDED_REVIEW.md`](BLINDED_REVIEW.md) — reviewer blinding and adjudication.
- [`AUDIT.md`](AUDIT.md) — contamination and reproducibility audit.
- [`READINESS.md`](READINESS.md) — normative go/no-go verdict and blockers.
- [`environment-lock.json`](environment-lock.json) — pinned execution and
  validation controls.
- [`evaluator/oracle-lock.json`](evaluator/oracle-lock.json) — independent
  semantic assertion/evidence lock; its historical command strings are
  construction-only until a qualified staged production manifest exists.
- [`evaluator/core-score-lock.json`](evaluator/core-score-lock.json) — fixed
  19-point candidate-tree machine core.
- [`evaluator/authority-lock.json`](evaluator/authority-lock.json) — assignment
  cutoffs, allowed authority, and outcome-only separation.
- [`schemas/result-record.schema.json`](schemas/result-record.schema.json) —
  machine-readable result schema.
- [`schemas/blinded-review-score.schema.json`](schemas/blinded-review-score.schema.json)
  — independent reviewer score-sheet schema.
- [`RESULT_RECORDING.md`](RESULT_RECORDING.md) — artifact linkage, arithmetic,
  phase/arm, and unblinding rules for result records.
- [`evaluator/construction-pilot-index.json`](evaluator/construction-pilot-index.json)
  — content-addressed, permanently excluded construction evidence.
- [`scripts/verify-benchmark.mjs`](scripts/verify-benchmark.mjs) — construction
  integrity checks. It never runs a benchmark agent.

The current verdict is **NOT READY for Baseline A**. Static integrity passing is
necessary but does not override the blockers in `READINESS.md`.

## Visibility boundary

The benchmark directory is evaluator-only during a controlled run. The agent
receives only:

- an ancestor-only clone whose `HEAD` is the fixed historical base commit;
- exactly one root `AGENTS.md` overlay;
- the exact bytes of one task file, supplied as the user prompt.

The agent must not receive this README, the case manifest, GitHub references,
target commits, historical tests added after the base, review threads, hidden
failure modes, scoring material, or another case's task.
