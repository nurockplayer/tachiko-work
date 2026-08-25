# Task 6 report: one-shot Baseline controller

## Outcome

Implemented the one-shot benchmark controller and same-wave base-control capture required by the canonical Task 6 plan. Validation used only construction smoke fixtures and local fake agents. No formal authorization was supplied, and no Codex task was launched.

## Implemented behavior

- Atomically reserves one external `(wave_id, case_id, phase)` slot and installs an external terminal handler immediately, preventing resampling under a new attempt ID or artifact directory.
- Creates an opaque neutral run root, HOME, CODEX_HOME, temporary directories, staged runtime, and trusted controller bundle; records and binds all controlled source/runtime identities.
- Requires an external authorization file for formal phases, exact frozen Baseline A inputs and effective Codex arguments, the case timeout, ten-second termination grace, formal model catalog, complete Rustup home, Cargo cache, pnpm tree, code-mode host, and minimum free space.
- Runs same-wave historical-base controls before candidate preparation or agent launch, without exposing candidate/variant bytes, using the staged/hash-bound Bash.
- Launches one agent attempt inside a unique launchd resource coalition, captures raw JSONL/stdout/stderr/final message, and never retries after output, failure, or timeout.
- Uses one monotonic TERM deadline followed by KILL, enumerates the entire resource coalition through a sealed native helper with stable, complete, growing PID scans, unloads the launchd job, and proves coalition extinction. The kernel profile denies direct session/process-group changes, `launchctl` execution, and Mach bootstrap; active tests also prove extinction of native `POSIX_SPAWN_SETPGROUP` and `POSIX_SPAWN_SETSID` escapes. The shared supervisor covers every base-control, candidate-core, oracle shell/adapter/portable command, Cargo metadata/build, and direct libtest execution.
- Keeps coalition control files and the HMAC key outside child environment/arguments and read closure, denies child writes to exact control/status/output paths, captures target output through controller pipes, and authenticates preliminary and final runner records before accepting terminal evidence.
- Runs the candidate under a kernel profile that denies the controller bundle, artifact directory, source repository, attempt registry, historical control workspace, and external variant/authorization/catalog inputs while permitting only the neutral workspace and staged run-root runtime/cache/HOME/CODEX_HOME/TMP roots. Candidate core commands retain kernel network denial and cannot write trusted capture, process, control, or bundle roots.
- Re-hashes the complete controller bundle path/type/mode/content manifest after agent and candidate-process extinction, immediately before and after every bundled helper, and before every stage receipt. Trusted helpers inherit bundle/control write denial, formal context verification independently re-hashes the live bundle tree, and every authorized stage binds its current verification.
- Captures the raw candidate and diff outside the workspace, reconstructs immutable validation input, runs core checks and production oracles, and verifies the overlay identity after the agent exits.
- Supports TW-05/TW-09 `awaiting_trusted_adapter` pause/resume against the same captured attempt without agent relaunch and with full chain/artifact revalidation.
- Requires formal adapter probes to use the sealed scaffold and a complete hash-closed input closure. TW-09 probes are built by the controller's content-addressed bundle from the complete captured raw manifest and an independently reviewed external Rust source, using exact Cargo/rustc/rust-lld identities and a fixed offline command inside a fresh disjoint deny-network/read/write sandbox. The resulting binary and build receipt are committed by an authorized post-issuance stage; production oracles verify and consume that exact package without accepting declarative external build identities.
- Applies deny-by-default reads and writes to formal adapters, with only exact immutable system runtime paths, sealed adapter inputs, the reconstructed candidate, and one fresh controller-owned temporary directory allowed. Candidate/control inputs are re-hashed after process-group extinction.
- Treats a zero-byte trusted patch as explicit candidate evidence, so no-op and timeout-before-edit remain scored task outcomes rather than infrastructure failures.
- Builds and scans deterministic blinded-review packets and emits an explicit pending-score result skeleton. Authorized formal candidates remain result-eligible while `result_state` is `awaiting_score_freeze`; no score is claimed by the controller.
- Treats the append-only terminal ledger append as the commit and prevents a failed convenience-marker write from causing a second terminal append.
- Binds a trusted shell path/hash in the production oracle runner and qualification. Retained qualification uses the exact Bash identity in `environment-lock.json`.

## Verification

- Focused coalition/control-plane/network gate: **9/9 passed**, including direct `setsid`, native `posix_spawn` group/session escapes, launchctl/bootstrap denial, child control-file forgery, complete growing PID scans, and base/core/oracle kernel network denial.
- Focused qualifier/TW-09/controller gate: **5/5 passed**, including the real sealed TW-09 positive path and hostile-builder rejection.
- Focused controller-bundle isolation/drift gate: **2/2 passed**, including direct/symlink candidate access denial and same-UID malicious self-restoring helper injection before capture.
- Focused formal issuance/context/resume gate: **6/6 passed**, including independent live bundle-tree verification by oracle/review helpers and same-attempt adapter resume.
- Full operational suite: **102/102 passed** in `378.6s`.
- Full retained oracle qualification regenerated successfully:
  - payload SHA-256: `3234ed6d3c150c9fdb60840347d5b1e2bd82806430135b54a3fbb4169e2e0b38`
  - run-receipt SHA-256: `ee59d2f100f7beb83aa3aef5913f454e5f0dc6301fef07ebdc71bdffc72cd162`
  - evidence commitment SHA-256: `ac01d9623425b82c183efa74133badddb5e1b8d2f0784217e027f9a3c37fac8d`
- Oracle qualification verifier: passed.
- Nine-case benchmark verifier: passed.
- Syntax checks for every changed executable module: passed.
- `git diff --check`: passed.

## Practical limitations

- Provider-internal immutable deployment identity is not locally attestable.
- Current-user filesystem isolation and fail-closed post-run overlay identity checks cannot prove that a same-user process never modified and restored the instruction file in place; a dedicated account or stronger deny-write boundary would improve this.
- Multi-reviewer panel completion and additional independent neutrality audits remain outside the controller. The attempt is preserved as awaiting review rather than resampled.
- The retained construction qualification uses a padded, bounded, non-formal top-level trusted-orchestrator deadline because macOS launchd jobs cannot bootstrap the per-command launchd coalition. Every candidate core/oracle command remains independently sandboxed and coalition-supervised; formal controller evidence does not use the construction-only wrapper.

These limitations are explicitly non-blocking under the approved practical internal-experiment standard.
