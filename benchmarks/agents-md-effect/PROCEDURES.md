# Controlled Execution Procedures

Protocol: `tachiko-agents-effect-v1`. These procedures are frozen construction
output; they are not authorization to start an arm. A controlled run must not
begin while any blocking item in `READINESS.md` remains open.

## Wave registration and common preflight

Before an experiment wave, freeze one read-only controller bundle containing
the benchmark, schemas, validators, model catalog, environment lock, adapter
policy, and result validator. Record every byte hash in the wave registry.
Baseline and Variant workspaces must not be able to read this bundle.

Create the immutable pre-run registry and append-only ledger genesis exactly as
`wave-registration-v1.json` specifies. Generate `experiment_id`, `wave_id`,
`run_id`, `attempt_id`, `pair_id` where applicable, and
`blinded_candidate_id` as independent lowercase 128-bit CSPRNG hex values; IDs
may not encode case or arm and the generation receipt is registered. Commit the
sealed variant set, arm key, planned order, reviewer-allocation seed, and empty
ledger before provider contact. Register each attempt before workspace/task
exposure. Terminal ledger entries bind the completed record hash one-way; the
record binds only its earlier attempt-registration hash.

For every observation:

1. Create a new opaque run root on the locked host and a copy-on-write clone of
   the sealed Cargo cache. Use the neutral `codex-worker` account for the agent,
   `build-worker` for all candidate-controlled builds and tests, `probe-builder`
   for trusted probe compilation, `probe-runner` for candidate-linked probe
   execution, and `job-controller` for sealed inputs/receipts. Account names,
   home/tool paths, hostnames, and workspace paths visible to the agent must not
   contain benchmark/protocol, arm, variant, or case identifiers.
2. Require at least 10 GiB free; no other agent or validator process may run.
3. Materialize the model catalog once per wave:

   ```sh
   node <controller>/scripts/materialize-model-catalog.mjs \
     --output <sealed-wave>/models-0.149.0.json
   ```

   Verify its locked hash, then copy those exact read-only bytes to
   `/opt/isolated-runtime/model-catalog/models-0.149.0.json`; only that neutral
   path is passed to Codex and visible in its process arguments.

4. Launch the environment verifier with only the locked environment:

   ```sh
   /usr/bin/env -i \
     HOME=<opaque-run-root>/home \
     PATH=<environment-lock.controlled_runner.path> \
     LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
     CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true \
     CARGO_HOME=<opaque-run-root>/cargo-home \
     RUSTUP_HOME=/opt/isolated-runtime/rustup \
     PNPM_HOME=/opt/isolated-runtime/pnpm-11.13.0 \
     GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_ATTR_NOSYSTEM=1 \
     <locked-node> <controller>/scripts/verify-environment.mjs
   ```

5. With the exact Codex flags below, separately require: the effective feature
   list hash `3ee1728480bd8371db3bb68b095f711de9c38e4690b29260233e1824bb1095f5`;
   `mcp list --json` equal to `[]`; strict-config parse success up to the expected
   “no transport configured” error; and a neutral `debug prompt-input` containing
   no skill, app, collaboration, or environment-context additions. A sentinel
   checkout must also prove that the root `AGENTS.md` bytes are discovered.
   From the actual agent identity, capture `id`/`whoami`, hostname, `pwd`, `env`,
   command paths, mounts, process arguments, and representative denied-path
   errors; fail if any reveals benchmark/protocol, case, arm, or variant labels
   or an evaluator-only construction source path.
6. Provision a sealed per-run provider credential through a supervisor broker
   or equivalent channel that the agent shell/filesystem cannot read. Freeze
   hashed provider account and project identities plus organization, region,
   entitlement, and rate-limit context; both arms must match exactly. Fresh
   `CODEX_HOME`, disabled auth elicitation, and disabled secret storage do not
   themselves provide authentication and may not be bypassed with a task-visible
   token.
7. Abort as `invalid_run` on any mismatch. A task-quality failure or timeout is
   never retried. Permit one fresh `attempt_number=2` only if failure occurred
   before any model output, filesystem mutation, or candidate-dependent signal.
   Once output exists, never launch another agent: at most once re-execute only
   the failed validator/review stage against the identical immutable captured
   candidate when no protected content was exposed. Otherwise cancel the case;
   controlled A/B discards both sides. The matrix is automatic and cannot depend
   on candidate quality or operator preference.

Before exposing a case to Codex, create a separate ancestor-only clean-base
clone and execute the ordered de-duplicated union of every command in
`cases.json.validation.base` and every per-case validation command in
`core-score-lock.json` under the same validator environment. This includes
release, documentation, and metadata commands even when the shorter base list
omits them. Freeze the exact command-list hash, base commit/tree, and one receipt
per command. Any failure invalidates that case for the wave; it is not attributed
to a candidate. For controlled A/B, run this health control immediately before
the pair and use the same receipt for both arms. The base-control clone is never
an agent workspace and contributes no score.

## Timeout and termination policy

The agent deadline is exactly `cases.json.time_limit_minutes * 60` wall-clock
seconds, starting immediately before process spawn. At the deadline the
supervisor sends `SIGTERM` to the isolated process group, waits exactly 10
seconds, then sends `SIGKILL` to that process group. It must prove no descendant
survives before capture. A timeout is a task outcome, receives no retry, and the
remaining filesystem is captured and scored normally once processes are dead;
completion may be zero and ordinary hard-failure/cap rules still apply. Failure
to terminate the group or obtain a trustworthy capture is `invalid_run`, not a
candidate failure, and discards the controlled pair.

Every base-control, candidate-core, adapter, and oracle command has a 1,800-
second wall limit with the same 10-second TERM/KILL sequence. A base-control
timeout invalidates the case/pair. A timeout caused by candidate-controlled
build/test behavior is a failed candidate check or assertion; a supervisor,
host, or confinement failure is `infrastructure_error`. Record the deadline,
signals, timestamps, process-group extinction check, partial-capture status, and
classification in the result.

## Agent workspace and prompt

The controller runs:

```sh
node <controller>/scripts/prepare-case.mjs \
  --case <TW-ID> \
  --source-repo <unreadable-source-repository> \
  --variant-file <sealed-variant-file> \
  --workspace <opaque-run-root>/<random-neutral-name> \
  --trusted-dir <trusted-output>/preparation \
  --expected-variant-sha256 <registered-variant-sha256>
```

The path must not contain `benchmark`, `baseline`, `variant`, a case ID, or an
arm label. The script creates an ancestor-only bundle, removes remotes/reflogs
and alternates, verifies the exact base tree and absence of the target commit,
rejects the case if a root `AGENTS.md` is present anywhere in the base's
reachable history, and writes the registered root file as an ignored untracked
overlay without changing `HEAD`. The supervisor must then make that exact node
controller-owned and immutable to the agent through a qualified OS/sandbox
deny-write policy; merely setting mode `0444` is insufficient because a writable
parent permits replacement. Index flags or tracked-blob substitution are
forbidden. Record pre-launch `lstat` device/inode/type/owner/mode/flags, policy
receipt, bytes, and hash; require identical post-run identity and content. Raw
capture excludes that exact verified node only. No parent/user instruction files
are readable. The frozen task bytes stay outside the workspace and are streamed
to stdin. The current preparation script does not provision this protection and
is therefore construction-only until the runner supplies and negatively
qualifies it.

If any post-run overlay identity/content field differs or the node is missing,
do not capture or score candidate quality. Preserve available pre/post evidence
in the invalidation receipt, attribute the initiating action separately from the
control failure, set `invalid_run`, append the immutable terminal attempt record,
and cancel the controlled pair/case slot. Because model output already exists,
no replacement agent may be launched.

## Exact Codex invocation

Every controlled arm uses the following command and enumerated environment.
Only `<sealed-variant-file>` used during workspace preparation changes between
A and B.

```sh
/usr/bin/env -i \
  HOME=<opaque-run-root>/home \
  PATH=<environment-lock.controlled_runner.path> \
  CODEX_HOME=<opaque-run-root>/codex-home \
  TMPDIR=<opaque-run-root>/tmp \
  LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
  CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true \
  CARGO_HOME=<opaque-run-root>/cargo-home \
  RUSTUP_HOME=/opt/isolated-runtime/rustup \
  PNPM_HOME=/opt/isolated-runtime/pnpm-11.13.0 \
  GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_ATTR_NOSYSTEM=1 \
  /opt/homebrew/Caskroom/codex/0.149.0/bin/codex exec \
  --cd <agent-workspace> \
  --model gpt-5.6-sol \
  --sandbox workspace-write \
  --ephemeral \
  --ignore-user-config \
  --ignore-rules \
  --strict-config \
  --disable apps \
  --disable auth_elicitation \
  --disable browser_use \
  --disable browser_use_external \
  --disable browser_use_full_cdp_access \
  --enable code_mode_host \
  --disable computer_use \
  --enable enable_request_compression \
  --disable fast_mode \
  --disable goals \
  --disable guardian_approval \
  --disable hooks \
  --disable image_generation \
  --disable in_app_browser \
  --disable in_app_chat \
  --disable in_app_dictation \
  --disable in_app_updates \
  --disable memories \
  --disable mentions_v2 \
  --disable multi_agent \
  --disable multi_agent_v2 \
  --disable personality \
  --disable plugin_sharing \
  --disable plugins \
  --disable recommended_plugins \
  --disable remote_compaction_v2 \
  --disable remote_plugin \
  --disable secret_auth_storage \
  --disable shell_snapshot \
  --enable shell_tool \
  --disable skill_mcp_dependency_install \
  --disable skill_search \
  --disable tool_call_mcp_elicitation \
  --disable tool_suggest \
  --disable unbounded_connection_retries \
  --enable unified_exec \
  --disable view_image \
  --disable workspace_dependencies \
  -c 'model_reasoning_effort="high"' \
  -c 'model_reasoning_summary="none"' \
  -c 'model_verbosity="low"' \
  -c 'service_tier="default"' \
  -c 'web_search="disabled"' \
  -c 'agents.enabled=false' \
  -c 'tools.experimental_request_user_input.enabled=false' \
  -c 'tools.update_plan.enabled=false' \
  -c 'skills.bundled.enabled=false' \
  -c 'skills.include_instructions=false' \
  -c 'include_apps_instructions=false' \
  -c 'include_collaboration_mode_instructions=false' \
  -c 'include_environment_context=false' \
  -c 'orchestrator.skills.enabled=false' \
  -c 'orchestrator.mcp.enabled=false' \
  -c 'model_catalog_json="/opt/isolated-runtime/model-catalog/models-0.149.0.json"' \
  -c 'approval_policy="never"' \
  -c 'sandbox_workspace_write.network_access=false' \
  -c 'shell_environment_policy.inherit="all"' \
  -c 'shell_environment_policy.ignore_default_excludes=false' \
  -c 'shell_environment_policy.experimental_use_profile=false' \
  --json \
  -
```

The privileged supervisor writes the exact frozen task bytes to the child's
stdin through a one-way pipe; no controller path is mounted or readable by the
agent account. It captures stdout/stderr through pre-opened controller-owned
pipes and extracts the final response from immutable stdout JSONL; no
agent-writable `--output-last-message` path is used. The controller records exit
code, start/end timestamps, timeout reason, usage, task hash, all effective
locks, and the raw final-message hash. No resume, fork, steering, user reply,
retry for task quality, or sub-agent is allowed.

`HOME` is a newly created empty supervisor-owned read-only directory inside the
opaque run root. Freeze its empty-tree/path/owner/mode receipt before launch,
deny agent write/replace, recheck after exit, and delete it with the run; it is
never reused across attempts. `CODEX_HOME` and `TMPDIR` are likewise fresh per
attempt but writable only to the extent required by the locked client/sandbox.

## Candidate capture and validation order

After Codex exits, revoke the agent account and verify the `AGENTS.md` hash.
Capture must then read the raw filesystem, not trust the candidate repository's
index, ignore rules, attributes, hooks, filters, or local Git configuration.
It must include committed, staged, unstaged, ignored-untracked, ordinary
untracked, binary, executable-bit, and symlink changes while excluding only the
pre-registered root overlay, `.git`, and pre-registered reproducible cache/build
roots. Unsupported filesystem node types invalidate the run.

The current command is a construction prototype only:

```sh
node <controller>/scripts/capture-candidate.mjs \
  --case <TW-ID> \
  --workspace <agent-workspace> \
  --trusted-dir <trusted-output>/capture \
  --expected-agents-sha256 <registered-variant-sha256>
```

It is not authorized for a controlled run because it stages through the
agent-mutatable index/configuration. The production replacement must construct
the candidate in a separate trusted object database and temporary index using
raw path/type/mode/content observations and `hash-object --no-filters`; it must
disable hooks, attributes-driven filters, global/system configuration,
autocrlf conversion, and alternates. Every trusted Git subprocess uses the
locked Git binary through locked `rtk`, `GIT_CONFIG_NOSYSTEM=1`,
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ATTR_NOSYSTEM=1`, no alternates, and explicit
`core.hooksPath=/dev/null`, `core.attributesFile=/dev/null`, and
`core.autocrlf=false`. Qualification must prove that assume-unchanged, skip-worktree,
ignored files, hostile `.gitattributes`, configured clean filters, binary files,
symlinks, and executable-bit changes are captured byte-for-byte. A trusted
raw-tree digest, exclusion-list hash, generated candidate commit/tree, and
round-trip digest are required receipts.

After a qualified capture, prepare a clean candidate copy without evaluator
files:

```sh
node <controller>/scripts/prepare-validation.mjs \
  --case <TW-ID> \
  --source-repo <unreadable-source-repository> \
  --patch-file <trusted-output>/capture/candidate.patch \
  --capture-receipt <trusted-output>/capture/capture-receipt.json \
  --workspace <opaque-run-root>/<random-validation-name> \
  --trusted-dir <trusted-output>/candidate-preparation
```

Require capture-to-apply tree equality and a passing same-wave base-control
receipt, then run only the case's ten-point
`core-score-lock.json` commands in this clean candidate commit. Store one command
receipt per exact command. The following overlay command is a construction
prototype only; it is not authorized for a controlled run because it places
confidential evaluator source beside candidate-controlled build machinery:

```sh
node <controller>/scripts/prepare-oracle-overlay.mjs \
  --case <TW-ID> \
  --source-repo <unreadable-source-repository> \
  --workspace <validation-workspace> \
  --candidate-receipt <trusted-output>/candidate-preparation/validation-preparation-receipt.json \
  --trusted-dir <trusted-output>/oracle-overlay
```

The production replacement uses two stages. First, build candidate production
artifacts with no oracle, adapter, expected-value, controller, or receipt bytes
mounted and with candidate Cargo configuration policy frozen. Freeze the
candidate source and artifacts read-only and hash them. Second, compile each
trusted evaluator probe in a separate read-only capsule through direct,
controller-selected tool invocations against only the minimal prebuilt candidate
artifact/API surface; candidate hooks, wrappers, build scripts, proc macros, and
tests may not execute during probe compilation. Unmount probe source and
expected-value material before executing any candidate-linked binary. Pre/post
hashes must prove that candidate artifacts, probe binaries, and evaluator inputs
were not changed. If a contract cannot be evaluated through that separation,
the adapter family is unqualified and the observation cannot run.

The existing `oracle-lock.json` `commands` and `command_specs` are
construction/historical-ground-truth invocations, not controlled-run physical
commands: literal `cargo test` and the portable construction runner cannot meet
the confidentiality model above. Before a wave, freeze a production oracle
command manifest mapping every unchanged assertion ID to its candidate-only
artifact build receipt, trusted probe-build command/binary hash, and
expectation-free execution/validator command. The manifest and every adapter
family must pass positive ground truth, behavior-missing, oracle-read, and
oracle-mutation controls without changing any scenario, selector, expected
observation, or point.

Only then execute each `oracle-lock.json` assertion independently in that
qualified capsule. Exact Rust selectors must show exactly one matched passing
probe and the executed binary hash must equal the controller-built receipt.
JSON-pointer selectors consume the hash-locked validator's JSON output. Portable
selectors consume only selected record indexes from the qualified staged
runner. Never reuse an assertion ID or award points from an unselected record.

Where `adapter_allowed=true`, a variant-blind oracle custodian may adapt names
and types only. Save the complete adapter source/diff and hashes, obtain a
second independent adapter-integrity review, and apply it as a new evaluator
commit after the locked overlay. A compile failure is `oracle_adapter_required`,
not zero, until that process completes. An adapter that implements behavior or
emits expected values without exercising candidate production code invalidates
the observation.

Candidate core commands and candidate Cargo build scripts/tests execute as the
confined validator account in a disposable candidate-only copy. A distinct
probe-builder account creates the evaluator binaries without candidate-controlled
configuration or executables; a still-confined execution account runs those
binaries after probe source is unmounted. These identities can access only the
stage-specific workspace, copied dependency cache, temporary directory, and
explicit read-only inputs. None can access the full controller, source
repository, arm key, other runs, trusted receipt store, or an earlier/later
stage's confidential material; stdout/stderr and machine results leave through
pre-opened controller-owned channels. The controller never executes
candidate-controlled hooks, filters, build scripts, tests, proc macros, wrappers,
or binaries. Before a wave, hostile build.rs, `.cargo/config` rustc-wrapper,
proc-macro, and test controls must fail to read oracle/expected-value and
sentinel controller/source/other-run files, fail to modify evaluator inputs,
probe binaries, candidate artifacts, or trusted receipts, and fail to reach the
network, while an ordinary validation command still succeeds. Record the
stage-isolation and hostile-control receipt hashes in every result.

## Blinding, scoring, and record freeze

Run `redact-final-message.mjs`, use the attempt's pre-provider committed opaque
candidate label, and use the
precommitted arm-key-blind `reviewer-allocation-v1.json` algorithm for panels,
custodian/integrity roles, adjudicator priority, and non-adjacent review order.
Build the review packet specified in
`BLINDED_REVIEW.md`. Machine point totals, arm, model transcript, runtime, the
paired candidate, target history, and oracle implementation remain hidden.
Before packet release, obtain case-specific conflict/prior-exposure attestations
and reject constructors, historical case participants, oracle/adapter actors,
experiment operators, or anyone with implementation-specific prior knowledge.
Two reviewers score independently; controlled A/B arms use disjoint panels and
disjoint adjudicators, so no reviewer sees both candidates in a pair before all
pair reviews freeze. Adjudicate under the locked disagreement rules. A semantic
result validator must bind every ID, point, status, command, tree, receipt,
review-panel assignment, attestation, and total to the three locks and both JSON
schemas. Before unblinding, freeze a canonical anonymous score payload that
uses the exact wrapper, pointer removal list, unknown-field rule, and canonical
serialization in `pre-unblind-score-projection-v1.json`. After the arm-key join,
populate only those removed fields and bind the pre-unblind payload hash. Finally
validate the detached `semantic-result-projection-v1.json`, hash the completed
external receipt, insert it as `record_validation_receipt_sha256`, and prove both
projections reconstruct byte-for-byte with no score/evidence change or
self-hashing cycle.

## Baseline A procedure

1. Register exactly the current Baseline A bytes: source commit
   `30c001c42ee10c1d460ce0a690245107eedc4ac5`, Git blob
   `ee3eb018062129f5df8f7e990fef2721cf84f69a`, SHA-256
   `2179753f8e015f5c96e534ac633a3cdb2d10ffa7f98c3f608e351e929ade84d8`,
   2657 bytes.
2. After all readiness blockers are closed, run `TW-01` through `TW-09` as nine
   fresh, serialized single-agent processes in manifest order.
3. Apply all common capture, candidate-only validation, hidden evaluation,
   blinding, review, and record-freeze steps above.
4. Set `experiment_phase="baseline_a"` and `arm="A"` only after the anonymous
   score payload is frozen. Emit and publish all nine schema-valid final records,
   each bound to its pre-unblind payload; do not import any construction pilot or
   Ultra output.
5. These results are a standalone characterization. They may not be reused as
   the A side of a later B wave unless the full provider deployment and every
   other lock can be proven identical—which currently cannot be done.

## Variant B and future controlled A/B procedure

Construction does not create, edit, or recommend Variant B. A future owner must
register B outside the repository with immutable provenance, bytes, and SHA-256
before inspecting any B behavior. Before registration, the variant owner must
either prove that B is a pre-existing independently motivated immutable revision
or sign a conflict/access attestation showing no access to evaluator cases,
tasks, oracles, hidden modes, rubric, construction pilots, or Baseline results.
The sealed benchmark may be disclosed only after B freezes. No case, prompt,
oracle, rubric, or adapter contract may be changed in response to B.

For the actual controlled comparison:

1. Run fresh A and B observations; never pair B with the standalone Baseline A.
2. Pre-register order: A-first for odd cases and B-first for even cases; reverse
   every pair in a separately registered replicate wave, if used.
3. Run each pair consecutively on the same idle host and deployment, with fresh
   workspaces, Cargo homes, Codex homes, processes, and anonymous labels.
4. Use byte-identical model, effort, task, historical base, feature/tool set,
   prompt wrapper, validation environment, evaluator, reviewers, limits, and
   root exposure mechanism. Here “reviewers” means the identical eligibility
   pool, packet, rubric, randomization, and adjudication protocol; the actual
   quality reviewers, adjudicators, oracle custodians, and adapter-integrity
   reviewers are disjoint across the two arms of a case. Only the root
   `AGENTS.md` bytes differ.
5. If a provider, host, tool, cache, controller, evaluator, or review lock differs
   before any model output, use only the pre-registered `attempt_number=2` path.
   If output already exists, append the invalid/discarded terminal records and
   cancel both sides of the pair/case slot; never resample an agent or repair only
   one arm. The sole post-output recovery is the one permitted re-execution of a
   failed validator/review stage against the identical immutable candidate.
6. Freeze both anonymous results and all reviews before joining the arm key.
7. Bind the eligible independent variant provenance/access attestation, the
   variant-blind intrinsic benchmark audit, and the arm-label-blind comparison
   audit to the wave registry.
   Report paired B-minus-A raw and dimension differences under `PROTOCOL.md`.

A single first ordinary lookup, regardless of standard client class, denied with
zero protected bytes exposed and no control mutation remains valid and has no
automatic quality penalty. Deliberate boundary probing,
evasion, or repeated access after the task-visible notice or an explicit denial
is a valid candidate hard failure. Any actual protected-content
exposure—whether agent-caused or infrastructural—sets `invalid_run`, records
attribution, appends terminal records, and cancels both arms of the pair/case
slot without replacement.

## Optional Ultra procedure

Ultra uses new workspaces and the same task statements but may enable delegation
or different orchestration, including `multi_agent=true`. Register a separate
configuration and
`experiment_phase="ultra_optional"`; never substitute or pool Ultra observations
with Baseline A or controlled A/B.
