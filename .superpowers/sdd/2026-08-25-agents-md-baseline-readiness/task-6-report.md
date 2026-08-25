# Task 6 report: one-shot Baseline controller

## Outcome

Implemented the one-shot benchmark controller and same-wave base-control capture required by the canonical Task 6 plan. Validation used only construction smoke fixtures and local fake agents. No formal authorization was supplied, and no Codex task was launched.

## Implemented behavior

- Atomically reserves one external `(wave_id, case_id, phase)` slot and installs an external terminal handler immediately, preventing resampling under a new attempt ID or artifact directory.
- Creates an opaque neutral run root, HOME, CODEX_HOME, temporary directories, staged runtime, and trusted controller bundle; records and binds all controlled source/runtime identities.
- Requires an external authorization file for formal phases, exact frozen Baseline A inputs and effective Codex arguments, the case timeout, ten-second termination grace, formal model catalog, complete Rustup home, Cargo cache, pnpm tree, code-mode host, and minimum free space.
- Runs same-wave historical-base controls before candidate preparation or agent launch, without exposing candidate/variant bytes, using the staged/hash-bound Bash.
- Launches one detached agent process group, captures raw JSONL/stdout/stderr/final message, and never retries after output, failure, or timeout.
- Uses one monotonic TERM deadline followed by KILL and proves process-group extinction. The shared supervisor also covers every base-control, candidate-core, oracle shell/adapter/portable command, Cargo metadata/build, and direct libtest execution.
- Captures the raw candidate and diff outside the workspace, reconstructs immutable validation input, runs core checks and production oracles, and verifies the overlay identity after the agent exits.
- Supports TW-05/TW-09 `awaiting_trusted_adapter` pause/resume against the same captured attempt without agent relaunch and with full chain/artifact revalidation.
- Builds and scans deterministic blinded-review packets and emits an explicit pending-score result skeleton. Authorized formal candidates remain result-eligible while `result_state` is `awaiting_score_freeze`; no score is claimed by the controller.
- Treats the append-only terminal ledger append as the commit and prevents a failed convenience-marker write from causing a second terminal append.
- Binds a trusted shell path/hash in the production oracle runner and qualification. Retained qualification uses the exact Bash identity in `environment-lock.json`.

## Verification

- Focused adversarial/runtime/controller tests: **7/7 passed** in the final reviewer pass.
- Full operational suite: **62/62 passed** in `151301.871584ms`.
- Full retained oracle qualification regenerated successfully:
  - payload SHA-256: `47cad14a2d985e2edd7dcd8dbcaf4cfa1fd4537531960bf339dd29e509869e2d`
  - run-receipt SHA-256: `d8663f74cf5d4c5710889769e813d5b6fe39c52a1b8a3cd3dd4e6cdb7f8cee51`
  - evidence commitment SHA-256: `a0dc816cc3373176f2be31a2f268174669353a99488ad8a96cc9ef147659b956`
- Oracle qualification verifier: passed.
- Nine-case benchmark verifier: passed.
- Syntax checks for every changed executable module: passed.
- `git diff --check`: passed.

## Practical limitations

- Provider-internal immutable deployment identity is not locally attestable.
- Current-user filesystem isolation and fail-closed post-run overlay identity checks cannot prove that a same-user process never modified and restored the instruction file in place; a dedicated account or stronger deny-write boundary would improve this.
- Multi-reviewer panel completion and additional independent neutrality audits remain outside the controller. The attempt is preserved as awaiting review rather than resampled.

These limitations are explicitly non-blocking under the approved practical internal-experiment standard.
