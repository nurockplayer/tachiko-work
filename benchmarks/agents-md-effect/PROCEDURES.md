# Controlled Execution Procedures

Protocol: `tachiko-agents-effect-v1`. These procedures do not authorize an arm.
The frozen task, case, historical-base, oracle-selector, and scoring contracts
remain unchanged. The operational profile below implements them for a practical
internal Baseline A; stronger provider/account/panel attestations described in
the original design are recorded limitations unless `READINESS.md` identifies a
locally actionable blocker.

Formal phases are fail-closed and require a separate external authorization
file. Construction smokes may exercise the controller with a local fake agent,
but may not supply a frozen task to Codex or be imported into a formal result.

## Wave registration and common preflight

Before an experiment wave, freeze one controller bundle containing the
benchmark, schemas, validators, environment lock, adapter policy, and result
validator. `run-controller.mjs` copies and hashes the complete executable bundle
before exposure, records its tree digest in every stage receipt, and invokes
helpers and contracts only from that copy. Agent workspaces are disjoint from
the artifact directory and receive none of its paths or bytes.

Create the immutable pre-run registry and append-only ledger genesis exactly as
`wave-registration-v1.json` specifies. The external controller registry reserves
the unique `(wave_id, case_id, phase)` slot atomically; a different attempt ID
cannot resample an occupied slot. Generate `experiment_id`, `wave_id`,
`run_id`, `attempt_id`, `pair_id` where applicable, and
`blinded_candidate_id` as independent lowercase 128-bit CSPRNG hex values; IDs
may not encode case or arm and the generation receipt is registered. Commit the
sealed variant set, arm key, planned order, reviewer-allocation seed, and empty
ledger before provider contact. Register each attempt before workspace/task
exposure. Terminal ledger entries bind the completed record hash one-way; the
record binds only its earlier attempt-registration hash.

For every observation:

1. Create a new opaque run root on an exclusive host and a copy-on-write clone
   of the sealed Cargo cache. The implementation uses disjoint neutral
   workspaces and stage-specific immutable copies/receipts. Dedicated
   `codex-worker`, `build-worker`, `probe-builder`, `probe-runner`, and
   `job-controller` accounts are additional deployment hardening, not an
   internal Baseline readiness prerequisite. Account names, home/tool paths,
   hostnames, and workspace paths visible to the agent must not contain
   benchmark/protocol, arm, variant, or case identifiers.
2. Require at least 10 GiB free; no other agent or validator process may run.
3. Materialize the model catalog once per wave outside the repository:

   ```sh
   node <controller>/scripts/materialize-model-catalog.mjs \
     --output <sealed-wave>/models-0.149.0.json
   ```

   The controller revalidates its raw, canonical catalog, model-record, and
   base-instruction hashes, then copies those exact read-only bytes to
   `<opaque-run-root>/runtime/model-catalog.json`. Only that neutral per-attempt
   path is substituted into the externally authorized Codex arguments. Both
   arms of a future paired wave must bind identical catalog bytes.

4. Launch the environment verifier with only the locked environment:

   ```sh
   /usr/bin/env -i \
     HOME=<opaque-run-root>/home \
     PATH=<opaque-run-root>/tool-bin:/usr/bin:/bin:/usr/sbin:/sbin \
     LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
     CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true \
     CARGO_HOME=<opaque-run-root>/cargo-home \
     RUSTUP_HOME=<opaque-run-root>/rustup-home \
     PNPM_HOME=<opaque-run-root>/pnpm-home \
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
6. Use the operator's authenticated provider context without placing credential
   bytes in the agent task, workspace, or recorded environment. Record the
   provider account/project/organization/region/entitlement context that the
   client exposes. A sealed supervisor broker and provider-signed identity
   receipt are stronger future controls but are not available repository-side.
7. Abort as `invalid_run` on any mismatch. The externally reserved
   `(wave_id, case_id, phase)` slot is never resampled, including failures before
   model output. Once launch is attempted, never launch another agent. At most
   once re-execute only the failed validator/review stage against the identical
   immutable captured candidate when no protected content was exposed; adapter
   pause/resume follows the same rule. Otherwise cancel the case; controlled A/B
   discards both sides. The matrix is automatic and cannot depend on candidate
   quality or operator preference.

Before exposing a case to Codex, create a separate ancestor-only clean-base
clone and execute the ordered de-duplicated union of every command in
`cases.json.validation.base` and every per-case validation command in
`core-score-lock.json` under the same validator environment. This includes
release, documentation, and metadata commands even when the shorter base list
omits them. Freeze the exact command-list hash, base commit/tree, and one receipt
per command. Any failure invalidates that case for the wave; it is not attributed
to a candidate. For controlled A/B, run this health control immediately before
the pair and use the same receipt for both arms. The base-control clone is never
an agent workspace and contributes no score. The controller first actively
probes the Darwin kernel network-denial profile, then runs every base command
inside that profile and binds the sandbox executable, profile, probe, and
process-group supervision receipts. Candidate-core and oracle execution use the
same deny-network boundary.

The controller entry point is:

```sh
node <controller>/scripts/run-controller.mjs \
  --case <TW-ID> \
  --source-repo <source-repository> \
  --variant-file <registered-variant> \
  --expected-variant-sha256 <registered-variant-sha256> \
  --phase baseline_a \
  --run-root <neutral-parent>/r-<128-bit-hex> \
  --artifact-dir <trusted-artifact-parent>/<new-opaque-id> \
  --attempt-registry-dir <trusted-external-registry> \
  --agent-executable <locked-codex> \
  --agent-args-file <registered-argument-template.json> \
  --model-catalog-file <sealed-wave>/models-0.149.0.json \
  --timeout-seconds <case-time-limit-minutes-times-60> \
  --wave-id <128-bit-hex> \
  --run-id <128-bit-hex> \
  --attempt-id <128-bit-hex> \
  --candidate-id <128-bit-hex> \
  --cargo-home-template <sealed-cargo-home-template> \
  --rustup-home-template <sealed-rustup-home-template> \
  --custodian-id <opaque-custodian-id> \
  --authorization-file <external-formal-authorization.json>
```

The registered argument template contains literal `<workspace>` and
`<model-catalog>` placeholders. For a formal attempt, the controller requires
the frozen case timeout, substitutes only the opaque per-run workspace and
staged catalog paths, and verifies authorization commitments for the exact
effective arguments and complete local runtime identity before reserving the
slot. The external registry and artifact directory must be disjoint from both
the source repository and run root.

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
overlay without changing `HEAD`. Index flags or tracked-blob substitution are
forbidden. Record pre-launch `lstat` device/inode/type/owner/mode, bytes, and
hash; require identical post-run identity and content before capture. Raw
capture excludes that exact verified node only. No parent/user instruction files
are injected, and the frozen task bytes stay outside the workspace and are
streamed to stdin. Under the practical same-user profile, an attempted
replacement invalidates the occupied slot and cannot be resampled. A separately
qualified OS deny-write policy and dedicated controller ownership would prevent
replacement rather than detect it and remain additional deployment hardening.

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
  PATH=<opaque-run-root>/tool-bin:/usr/bin:/bin:/usr/sbin:/sbin \
  CODEX_HOME=<opaque-run-root>/codex-home \
  TMPDIR=<opaque-run-root>/tmp \
  LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
  CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true \
  CARGO_HOME=<opaque-run-root>/cargo-home \
  RUSTUP_HOME=<opaque-run-root>/rustup-home \
  PNPM_HOME=<opaque-run-root>/pnpm-home \
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
  -c 'model_catalog_json="<opaque-run-root>/runtime/model-catalog.json"' \
  -c 'approval_policy="never"' \
  -c 'sandbox_workspace_write.network_access=false' \
  -c 'shell_environment_policy.inherit="all"' \
  -c 'shell_environment_policy.ignore_default_excludes=false' \
  -c 'shell_environment_policy.experimental_use_profile=false' \
  --json \
  -
```

The controller writes the exact frozen task bytes to the child's stdin through
a one-way pipe; no controller path is passed to the task or placed in the agent
workspace. It captures stdout/stderr through controller-owned pipes and extracts
the final response from the captured stdout JSONL; no
agent-writable `--output-last-message` path is used. The controller records exit
code, start/end timestamps, timeout reason, usage, task hash, all effective
locks, and the raw final-message hash. No resume, fork, steering, user reply,
retry for task quality, or sub-agent is allowed.

`HOME` is a newly created empty directory inside the opaque run root. Freeze its
empty-tree/path/owner/mode receipt before launch and delete it with the run; it
is never reused across attempts. `CODEX_HOME` and `TMPDIR` are likewise fresh
per attempt and disjoint from every pre-existing user or construction home. A
dedicated supervisor-owned read-only HOME is stronger deployment hardening for
a future multi-account runner, not a practical internal Baseline prerequisite.

## Candidate capture and validation order

After Codex exits and its process group is proven extinct, verify the
`AGENTS.md` identity and hash. Capture must then read the raw filesystem, not
trust the candidate repository's index, ignore rules, attributes, hooks,
filters, or local Git configuration.
It must include committed, staged, unstaged, ignored-untracked, ordinary
untracked, binary, executable-bit, and symlink changes while excluding only the
pre-registered root overlay, `.git`, and pre-registered reproducible cache/build
roots. Unsupported filesystem node types invalidate the run.

The production capture command is:

```sh
node <controller>/scripts/capture-candidate.mjs \
  --case <TW-ID> \
  --workspace <agent-workspace> \
  --source-repo <unreadable-source-repository> \
  --exclusions-file <trusted-output>/capture-exclusions.json \
  --expected-agents-identity-file <trusted-output>/preparation/overlay-identity.json \
  --trusted-dir <trusted-output>/capture \
  --expected-agents-sha256 <registered-variant-sha256>
```

It constructs the candidate in a separate trusted object database and temporary
index using raw path/type/mode/content observations and
`hash-object --no-filters`. It disables hooks, attributes-driven filters,
global/system configuration, autocrlf conversion, and alternates. Every trusted
Git subprocess uses the
locked Git binary through locked `rtk`, `GIT_CONFIG_NOSYSTEM=1`,
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_ATTR_NOSYSTEM=1`, no alternates, and explicit
`core.hooksPath=/dev/null`, `core.attributesFile=/dev/null`, and
`core.autocrlf=false`. Qualification proves that assume-unchanged,
skip-worktree, ignored files, hostile `.gitattributes`, configured clean filters,
binary files, symlinks, and executable-bit changes are captured byte-for-byte.
The receipt binds the source repository, expected overlay identity, normalized
exclusion list, raw-tree digest, generated candidate commit/tree, no-filter
patch, and round-trip digest.

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
receipt per exact command. A zero-byte/no-op patch is a valid captured candidate:
reconstruct the historical base, preserve the empty-patch identity in the
receipt, and continue through core, oracle, review, and score processing. The
following overlay command is a construction helper only and is not part of a
formal attempt:

```sh
node <controller>/scripts/prepare-oracle-overlay.mjs \
  --case <TW-ID> \
  --source-repo <unreadable-source-repository> \
  --workspace <validation-workspace> \
  --candidate-receipt <trusted-output>/candidate-preparation/validation-preparation-receipt.json \
  --trusted-dir <trusted-output>/oracle-overlay
```

`evaluator/production-oracles.json` is the production mapping for every frozen
command and assertion. The controller invokes its sealed copy through:

```sh
node <controller>/scripts/run-oracles.mjs \
  --case <TW-ID> \
  --candidate-root <validation-workspace> \
  --trusted-dir <trusted-output>/oracles \
  --expected-control-sha256 <registered-control-sha256> \
  --manifest <controller>/evaluator/production-oracles.json \
  --expected-manifest-sha256 <registered-manifest-sha256> \
  --oracle-lock <controller>/evaluator/oracle-lock.json \
  --expected-oracle-lock-sha256 <registered-oracle-lock-sha256> \
  --trusted-cargo <locked-cargo> \
  --expected-cargo-sha256 <registered-cargo-sha256> \
  --trusted-rustc <locked-rustc> \
  --expected-rustc-sha256 <registered-rustc-sha256> \
  --trusted-shell <locked-bash> \
  --expected-shell-sha256 <registered-bash-sha256>
```

TW-05 and TW-09 may additionally provide the content-addressed adapter and
contract files. If a correct candidate uses a different public seam, the
controller terminalizes as `awaiting_trusted_adapter`; the custodian may attach
only the operationally locked `candidate-adapter.mjs` scaffold, a disjoint
reviewer-authored config/probe pair, and a hash-bound eligible independent
adapter-integrity approval, then resume validation against the identical
captured patch. An arbitrary caller-supplied adapter is never formal-result
eligible. The agent is never relaunched.

```sh
node <controller>/scripts/run-controller.mjs \
  --resume-artifact-dir <trusted-attempt-artifacts> \
  --adapter-file <controller>/evaluator/adapters/candidate-adapter.mjs \
  --expected-adapter-sha256 <sealed-scaffold-sha256> \
  --adapter-config <external-reviewed-config.json> \
  --expected-adapter-config-sha256 <registered-config-sha256> \
  --adapter-integrity-receipt <external-integrity-review.json> \
  --expected-adapter-integrity-sha256 <registered-review-sha256> \
  --custodian-id <opaque-custodian-id>
```

Resume rehashes the sealed controller bundle, registry entry, prior stage chain,
frozen controls, process/capture/validation artifacts, variant/task, and source
repository identity, then reconstructs a fresh validation workspace from the
captured patch before oracle execution.

Formal adapter execution has one outer kernel sandbox. It denies network,
expected/control reads, and writes to the candidate, artifact/source/original
workspace, scaffold/config/probe/runtime, and other trusted roots. Only the
candidate subtree and exact trusted executable/config/probe inputs are readable;
only a newly created, initially empty adapter-specific TMP is writable. The
controller records pre/post candidate-tree and trusted-input bytes/types/modes,
requires process-group extinction, validates the scaffold's single stdout
receipt, and only then creates the normalized output with an exclusive trusted
write. The scaffold never receives expected values or the output path and may
not implement candidate behavior.

Candidate production work and trusted evaluator work are separated by fresh
copies and complete input/output identities. Exact Rust checks authenticate the
locked Cargo and rustc, every relevant Cargo/config/source input, the one built
test artifact, and direct libtest JSON execution. Candidate runners, wrappers,
hooks, filters, and harness substitutions are rejected. Portable and normalized
contract adapters record their trusted inputs and candidate-linked output hash.
Qualification covers positive ground truth, behavior-missing/base-negative, and
tamper controls without changing any scenario, selector, expected observation,
or point.

Only then execute each `oracle-lock.json` assertion independently in that
qualified capsule. Exact Rust selectors must show exactly one matched passing
probe and the executed binary hash must equal the controller-built receipt.
JSON-pointer selectors consume the hash-locked validator's JSON output. Portable
selectors consume only selected record indexes from the qualified staged
runner. Never reuse an assertion ID or award points from an unselected record.

The construction qualification is reproduced without Codex by:

```sh
node <controller>/scripts/qualify-oracles.mjs \
  --source-repo <source-repository> \
  --output <new-qualification-summary.json> \
  --mode full

node <controller>/scripts/verify-oracle-qualification.mjs \
  --receipt <new-qualification-summary.json>
```

For TW-05, `run-tw05-offline.mjs` invokes Cargo and Node entry points directly
under `CARGO_NET_OFFLINE=true` and an OS network-denial profile whose denial is
actively probed. npm, pnpm, and yarn are never part of the candidate execution
path. The full retained qualification uses a controlled runtime as the positive
for the frozen stale-revision contract, keeps the historical base as a negative,
and records the frozen historical target's two misses without weakening the
contract.

Where `adapter_allowed=true`, a variant-blind oracle custodian may adapt names
and types only. Save the complete config/probe source/diff and hashes and require
an eligible independent adapter-integrity review bound to the phase, attempt,
candidate capture, scaffold, config, and probe. A compile failure is
`oracle_adapter_required`,
not zero, until that process completes. An adapter that implements behavior or
emits expected values without exercising candidate production code invalidates
the observation.

Candidate core commands and candidate Cargo build scripts/tests execute as the
in a disposable candidate-only copy. Trusted probe construction and execution
use separate workspaces, explicit inputs, no candidate Git configuration, and
pre/post hashes. On a host with dedicated validator/probe accounts those stages
should use them; the practical internal profile permits the exclusive controller
account while preserving the same filesystem separation and receipts. Stdout,
stderr, and machine results enter the artifact store through controller-owned
paths. Hostile build/test/config controls, runtime binary substitution, network
access, and trusted-input mutation are construction-qualified and fail closed.

## Blinding, scoring, and record freeze

Use the attempt's pre-provider committed opaque candidate label. The controller
constructs a six-role hash-complete input manifest (`task`, `authority`,
`candidate_checkout`, `candidate_diff`, `candidate_validation`, and
`final_message`) and invokes:

```sh
node <controller>/scripts/build-review-packet.mjs \
  --case-id <TW-ID> \
  --candidate-id <opaque-candidate-id> \
  --input-root <trusted-packet-input-root> \
  --input-manifest <trusted-packet-input-manifest> \
  --variant <registered-variant-a> \
  --contract <controller>/evaluator/contracts/review-packet-blinding-v1.json \
  --output-dir <new-packet-directory> \
  --terminal-receipt <new-external-terminal-receipt> \
  --custodian-id <opaque-custodian-id> \
  --custodian-eligible true \
  --frozen-at <registered-rfc3339-time>
```

Register every frozen variant with a repeated `--variant`. The builder writes a
private match map outside the packet and a public manifest inside it. For a
formal attempt the controller supplies the same immutable
`--controller-context` and expected context hash to the builder and to a separate
`scan-review-packet.mjs` process, then binds the standalone scanner receipt into
its review-stage receipt. Direct builder/scanner invocations without that
context are labeled `construction_pilot_only` and cannot become formal evidence.
Release only when the external terminal scan receipt is `qualified` with zero
matches. Machine point totals, arm, model transcript, runtime, the
paired candidate, target history, and oracle implementation remain hidden.
Before packet release, obtain case-specific conflict/prior-exposure attestations
and reject constructors, historical case participants, oracle/adapter actors,
experiment operators, or anyone with implementation-specific prior knowledge.
For standalone internal Baseline A, one or more eligible packet-only reviewers
may score; record the actual reviewer count and eligibility/conflict statement.
If multiple reviewers are used, apply the locked independent/adjudication rules.
A future controlled A/B requires disjoint panels and adjudicators across each
pair, so no reviewer sees both candidates before pair reviews freeze. A semantic
result validator for the original fully governed publication profile must bind
every ID, point, status, command, tree, receipt,
review-panel assignment, attestation, and total to the three locks and both JSON
schemas. Before unblinding, freeze a canonical anonymous score payload that
uses the exact wrapper, pointer removal list, unknown-field rule, and canonical
serialization in `pre-unblind-score-projection-v1.json`. After the arm-key join,
populate only those removed fields and bind the pre-unblind payload hash. Finally
validate the detached `semantic-result-projection-v1.json`, hash the completed
external receipt, insert it as `record_validation_receipt_sha256`, and prove both
projections reconstruct byte-for-byte with no score/evidence change or
self-hashing cycle.

For the practical standalone internal Baseline, the controller terminates a
successful construction at a hash-bound `awaiting_score_freeze` result skeleton
and blinded packet. Apply the frozen rubric to the available eligible review
sheet or sheets, record the actual reviewer count, and preserve those artifacts
with the skeleton. Missing provider-signed deployment identity or a second
independent panel is a disclosed publication-profile limitation, not permission
to alter points, caps, thresholds, or evidence.

## Baseline A procedure

1. Register exactly the current Baseline A bytes: source commit
   `30c001c42ee10c1d460ce0a690245107eedc4ac5`, Git blob
   `ee3eb018062129f5df8f7e990fef2721cf84f69a`, SHA-256
   `2179753f8e015f5c96e534ac633a3cdb2d10ffa7f98c3f608e351e929ade84d8`,
   2657 bytes.
2. After the wave is externally authorized and each slot passes preflight, run
   `TW-01` through `TW-09` as nine fresh, serialized single-agent processes in
   manifest order.
3. Apply all common capture, candidate-only validation, hidden evaluation,
   blinding, review, and record-freeze steps above.
4. Set `experiment_phase="baseline_a"` and `arm="A"` only after the anonymous
   score payload is frozen. Preserve each controller result skeleton, blinded
   packet, review sheet, and score-freeze receipt; do not import any construction
   pilot or Ultra output. A publication claiming the stronger governed profile
   must additionally satisfy the full result schema and detached semantic
   validation contract.
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
5. If a provider, host, tool, cache, controller, evaluator, or review lock
   differs, append the invalid/discarded terminal record and cancel the pair/case
   slot; never resample an agent or repair only one arm. The sole post-launch
   recovery is the one permitted re-execution of a failed validator/review stage
   against the identical immutable candidate.
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
