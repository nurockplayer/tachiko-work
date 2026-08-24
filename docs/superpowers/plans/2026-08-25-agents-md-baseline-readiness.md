# AGENTS.md Baseline Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the frozen nine-case `benchmarks/agents-md-effect` benchmark operationally ready for a practical internal Baseline A wave without executing that wave.

**Architecture:** A dependency-free Node controller owns neutral run-root construction, preflight, same-wave base controls, one-shot agent execution, trusted raw-filesystem capture, validation/oracle receipts, and deterministic review packets. Frozen case/task/scoring/oracle semantics remain inputs; new operational manifests map them to executable commands and construction qualifications.

**Tech Stack:** Node.js 24 built-ins and `node:test`, existing Bash/Rust/Cargo/Git/rtk workflow, JSON receipts.

**Spec:** `benchmarks/agents-md-effect/PROTOCOL.md`, `PROCEDURES.md`, `SCORING.md`, `BLINDED_REVIEW.md`, and the user-approved practical internal-experiment readiness requirements.

## Global Constraints

- Do not change either `AGENTS.md` variant, task wording, case selection, historical bases, assertion/point allocation, or scoring semantics.
- Do not execute formal Baseline A or Variant B; every automated exercise in this change is `construction_pilot_only`.
- Never resample after model output or candidate mutation; the controller launches at most one agent process per registered construction/formal attempt.
- Treat provider-side immutable deployment identity, external reviewer panels, and further independent neutrality audits as recorded limitations, not local implementation blockers.
- Preserve the repository Cargo/pnpm toolchain and introduce no runtime dependency.

---

### Task 1: Operational contract and regression harness

**Files:**
- Create: `benchmarks/agents-md-effect/evaluator/production-oracles.json`
- Create: `benchmarks/agents-md-effect/tests/operational.test.mjs`
- Modify: `benchmarks/agents-md-effect/scripts/verify-benchmark.mjs`

**Interfaces:**
- Consumes: frozen `cases.json`, `oracle-lock.json`, and `core-score-lock.json`.
- Produces: a one-to-one mapping from every frozen command/assertion to a production command stage and a `node --test` entry point.

- [ ] Write a failing test that loads all four locks and proves all nine cases, every oracle command ID, every assertion ID, and every subjective group are represented exactly once without changing points/selectors.
- [ ] Run `node --test benchmarks/agents-md-effect/tests/operational.test.mjs` and confirm it fails because the production manifest is absent.
- [ ] Add the minimal manifest and integrity verification needed to make the mapping pass.
- [ ] Re-run the focused test and `node benchmarks/agents-md-effect/scripts/verify-benchmark.mjs`.

### Task 2: Neutral run root and per-attempt preflight

**Files:**
- Create: `benchmarks/agents-md-effect/scripts/preflight-run.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`

**Interfaces:**
- Consumes: `--workspace`, `--home`, `--codex-home`, `--artifact-dir`, `--receipt`, environment lock, and production-oracle manifest.
- Produces: a fail-closed JSON receipt containing instruction/skill exposure scans, environment values, binary/version/hash observations, filesystem identity, free space, and artifact/control hashes.

- [ ] Write failing tests proving an empty neutral HOME passes while ancestor `AGENTS.md`, `.codex/skills`, unexpected HOME content, semantic run-root labels, or a changed locked control fail.
- [ ] Run the focused tests and confirm each failure is for the missing preflight.
- [ ] Implement deterministic scans and receipt recording using only Node built-ins and direct executable hashing/version probes.
- [ ] Re-run the focused tests.

### Task 3: Trusted raw-filesystem candidate capture

**Files:**
- Replace: `benchmarks/agents-md-effect/scripts/capture-candidate.mjs`
- Modify: `benchmarks/agents-md-effect/scripts/prepare-validation.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`

**Interfaces:**
- Consumes: a frozen base commit, raw workspace tree, exact overlay identity/hash, and explicit exclusion list.
- Produces: a no-filter patch, raw tree manifest/digest, separate trusted object database/index tree, round-trip digest, and capture receipt.

- [ ] Write a failing adversarial test containing ignored/untracked files, assume-unchanged and skip-worktree paths, hostile attributes/clean filter/config/hook, binary content, symlink, and executable-bit changes.
- [ ] Confirm the current index-based capture misses or transforms at least one fixture.
- [ ] Build a lexical raw walker that rejects unsupported nodes, excludes only `.git`, the exact overlay, and registered cache roots, then hashes blobs with `git hash-object --no-filters` under a separate temporary object database/index.
- [ ] Generate the patch from the trusted tree, reconstruct it in a clean bundle clone, and require raw digest/tree equality.
- [ ] Re-run adversarial and validation-preparation tests.

### Task 4: Oracle execution, qualification, and TW-05 offline neutrality

**Files:**
- Create: `benchmarks/agents-md-effect/scripts/run-oracles.mjs`
- Create: `benchmarks/agents-md-effect/scripts/qualify-oracles.mjs`
- Create: `benchmarks/agents-md-effect/scripts/run-tw05-offline.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`
- Create: `benchmarks/agents-md-effect/evaluator/qualifications/oracles.json`

**Interfaces:**
- `run-oracles.mjs` consumes a case/candidate/manifest and emits command plus assertion receipts with exact-test match counts, JSON-pointer values, and selected portable-record results.
- `qualify-oracles.mjs` materializes target and base/behavior-missing workspaces, records content-addressed positive and negative outcomes, and never launches Codex.
- `run-tw05-offline.mjs` executes Rust build/tests and Node `--test`/benchmark directly with network-denied environment, independent of npm/pnpm/yarn command availability.

- [ ] Write failing manifest/runner tests for exact one-test matching, nonzero command behavior, JSON-pointer mismatch, portable-record mismatch, subjective-only cases, and TW-05 with package-manager shims that fail if invoked.
- [ ] Implement the minimal production runner and construction qualifier while preserving every frozen assertion selector and point.
- [ ] Run all-nine positive and base/behavior-missing qualification; store only compact content-addressed receipts and classify subjective cases as packet/gate qualification rather than fabricated machine discrimination.
- [ ] Re-run focused tests and static verification.

### Task 5: Deterministic blinded-review packets

**Files:**
- Create: `benchmarks/agents-md-effect/scripts/build-review-packet.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`

**Interfaces:**
- Consumes: case, candidate raw-tree/diff/final-message artifacts, frozen task/authority, validation excerpts, unlabeled variant set, opaque candidate ID, and output directory.
- Produces: sorted detached packet artifacts, deterministic redactions, private match map, packet manifest/hash, post-render scan receipt, and `safe_to_release` decision.

- [ ] Write failing fixtures for exact copy, case/whitespace rewrite, one-edit near-copy, explicit identifiers in paths/content, unchanged domain text, invalid UTF-8/binary fail-closed behavior, and residual-match rejection.
- [ ] Implement the frozen R1-R4 scanner, path aliasing, deterministic rendering, sorted manifest, and post-render scan.
- [ ] Re-run qualification fixtures twice and assert byte-identical semantic packet content/manifest after excluding the factual freeze timestamp.

### Task 6: One-shot controller and same-wave controls

**Files:**
- Create: `benchmarks/agents-md-effect/scripts/run-controller.mjs`
- Modify: `benchmarks/agents-md-effect/scripts/capture-base-control-evidence.mjs`
- Modify: `benchmarks/agents-md-effect/tests/operational.test.mjs`

**Interfaces:**
- Consumes: case, source repository, variant file/hash, phase, opaque run root, agent executable/arguments, and controller artifact directory.
- Produces: pre-registered attempt/append-only terminal ledger, preflight/base-control/process/capture/validation/oracle/review receipts, stdout/stderr/final-message artifacts, and a construction/formal result skeleton.

- [ ] Write a failing construction smoke using a local fake agent that mutates one file and emits one JSONL final response; assert one spawn, no retry on failure/timeout, base control precedes launch, all stage receipts bind the same attempt, and formal phases require an explicit authorization token.
- [ ] Implement ordered fail-closed stages and process-group timeout/termination with no agent resampling.
- [ ] Run the construction smoke only; do not provide the formal authorization token.

### Task 7: Readiness integration and final verification

**Files:**
- Modify: `benchmarks/agents-md-effect/README.md`
- Modify: `benchmarks/agents-md-effect/PROCEDURES.md`
- Modify: `benchmarks/agents-md-effect/AUDIT.md`
- Modify: `benchmarks/agents-md-effect/READINESS.md`

**Interfaces:**
- Consumes: fresh test, qualification, preflight, and smoke receipts.
- Produces: the practical internal-experiment verdict with only genuine operational blockers and explicit external limitations.

- [ ] Update procedures to name the operational commands and construction qualification evidence without changing the frozen experiment contract.
- [ ] Record provider deployment identity, unavailable independent panels/audits, and same-user host hardening as limitations where outside local control.
- [ ] Run `node --test benchmarks/agents-md-effect/tests/operational.test.mjs`, `node benchmarks/agents-md-effect/scripts/verify-benchmark.mjs`, benchmark construction qualifications, and the repository fast gate relevant to changed executable files.
- [ ] Review the complete diff against the user requirements and repository standards, fix Critical/Important findings, commit the focused change, and state exactly `READY for Baseline A` or `NOT READY for Baseline A` with only genuine blockers.
