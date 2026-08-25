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
- [`READINESS.md`](READINESS.md) — normative go/no-go verdict, launch gates,
  and recorded limitations.
- [`environment-lock.json`](environment-lock.json) — pinned execution and
  validation controls.
- [`evaluator/oracle-lock.json`](evaluator/oracle-lock.json) — frozen independent
  semantic assertion/evidence lock.
- [`evaluator/production-oracles.json`](evaluator/production-oracles.json) —
  production-stage mapping for all nine cases and every frozen machine or
  subjective assertion.
- [`evaluator/qualifications/oracles.json`](evaluator/qualifications/oracles.json)
  — content-addressed construction qualification for positive, behavior-missing,
  and base-negative oracle controls.
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
- [`scripts/run-controller.mjs`](scripts/run-controller.mjs) — one-shot run
  controller with neutral preflight, same-wave base control, trusted capture,
  validation, and blinded-packet stages.
- [`scripts/preflight-run.mjs`](scripts/preflight-run.mjs) — fail-closed
  per-attempt environment, tool, artifact, and instruction-surface preflight.
- [`scripts/capture-candidate.mjs`](scripts/capture-candidate.mjs) — trusted
  raw-filesystem candidate/diff capture with no-filter round-trip proof.
- [`scripts/run-oracles.mjs`](scripts/run-oracles.mjs),
  [`scripts/qualify-oracles.mjs`](scripts/qualify-oracles.mjs), and
  [`scripts/run-tw05-offline.mjs`](scripts/run-tw05-offline.mjs) — production
  oracle execution, retained positive/negative qualification, and
  package-manager-neutral TW-05 execution.
- [`scripts/process-group-supervisor.mjs`](scripts/process-group-supervisor.mjs)
  — shared fixed-deadline TERM/KILL/extinction supervision for agent and
  validation command groups.
- [`scripts/build-review-packet.mjs`](scripts/build-review-packet.mjs) and
  [`scripts/scan-review-packet.mjs`](scripts/scan-review-packet.mjs) —
  deterministic blinded packet construction and standalone release scanning.

The current practical internal-experiment verdict is **READY for Baseline A**.
Every formal attempt still requires the controller's external authorization and
per-run preflight; readiness is not authorization and no formal Baseline A or
Variant B task has been executed during construction. See `READINESS.md` for the
remaining recorded limitations.

## Visibility boundary

The benchmark directory is evaluator-only during a controlled run. The agent
receives only:

- an ancestor-only clone whose `HEAD` is the fixed historical base commit;
- exactly one root `AGENTS.md` overlay;
- the exact bytes of one task file, supplied as the user prompt.

The agent must not receive this README, the case manifest, GitHub references,
target commits, historical tests added after the base, review threads, hidden
failure modes, scoring material, or another case's task.
