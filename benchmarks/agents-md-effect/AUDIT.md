# Contamination and Reproducibility Audit

Audit date: 2026-08-25. Scope: benchmark construction only. No Codex benchmark
task was run, and no construction or Ultra observation is eligible for Baseline
A or Variant B.

## Methods and evidence boundary

Construction used first-parent Git history, frozen GitHub Issue/PR snapshots,
review comments, accepted repository authority, target tests, and local
construction pilots. `authority-lock.json` records, per case, the assignment
cutoff, exact allowlisted text hashes, claims, and outcome-only material.
`history-snapshots.json` and `authority-snapshots.json` are evaluator-only and
byte-locked; the agent receives none of them.

GitHub captures were made through authenticated read-only `rtk gh issue view`,
`rtk gh pr view`, GraphQL body-edit-history queries, and `rtk gh api` review
comment queries. Stable node IDs, created/edited timestamps, bodies, URLs, and
capture time are frozen in the snapshot files. Current GitHub state is not a
runtime dependency of an experiment wave.

## Audit matrix

| Question | Evidence | Verdict |
| --- | --- | --- |
| Does task wording reveal a target commit, PR URL, benchmark identity, score, historical helper/path, or hidden trap? | Static leak patterns plus manual review of all nine task files. TW-01's overly specific ADR hint was neutralized; TW-05 no longer mandates the historical path or one manager; TW-06 asks for the contemporaneous governance outcome without copying PR wording; TW-08 states the accepted behavior but not the historical DTO seam or review defect. | Pass |
| Is validation fitted to the historical patch? | Patch similarity is never scored. Oracle v2 uses unique assertion IDs, exact single-test selectors, disjoint JSON pointers, and selected portable records only. Production adapters normalize names and types without implementing behavior, and each executable family passed positive and behavior-missing controls. | Pass |
| Can every base/target be checked out and related unambiguously? | All nine full SHAs and base trees resolve. Eight targets have the declared base as direct parent. TW-09 is the only locked exception: base `7782114…` is an ancestor of implementation parent `c685fe7…`, with exact outcome-only first-parent sequence `22fc8eb…`, `b5b097c…`, `c685fe7…`. Ancestor-only preparation pilots could not resolve target commits. | Pass |
| Is historical `AGENTS.md` exposure eliminated? | The first root `AGENTS.md` entered history at `22fc8eb…`. Every selected base predates that commit, and the verifier scans each base's reachable root-path history and fails closed if any root `AGENTS.md` object is found. The overlay is always untracked; tracked-blob quarantine was rejected because tree object IDs disclose the treatment. | Pass |
| Can agent-visible runtime metadata reveal benchmark identity or prior cases? | The controller uses fresh opaque run roots and workspace/HOME/CODEX_HOME/TMPDIR paths, a newly empty HOME, a closed environment allowlist, staged neutral tool/catalog paths, and a recursive ancestor/tree scan for instruction files and skill directories. Symlinks and semantic path labels fail closed; the exact exposed root `AGENTS.md` identity is recorded before and after execution. | Pass for the practical internal runner; dedicated-account attestation remains a limitation |
| Do historical bases and outcomes build and satisfy their evidence? | The full construction qualification materialized all nine targets and behavior-missing bases, executed every core command, and exercised every applicable machine assertion. Machine cases have positive and negative discrimination evidence; subjective-only TW-01, TW-02, and TW-06 have deterministic packet gates. The controller independently executes the exact ordered base/core union in a fresh same-wave base checkout before agent launch. | Pass, with the disclosed TW-05 historical-target calibration miss |
| Do cases rely on authority created after assignment? | Allowlisted authority is at or before each cutoff; later implementation/review is explicitly outcome-only. Unrecoverable edited text is excluded. | Pass |
| Are capabilities needlessly duplicated? | Nine cases cover identity authority, storage-contract remediation, numeric conversion, crate architecture, resident-runtime research, governance reconciliation, graph oracle, legacy persistence, and staged diagnostics. Native/WASM/release gates and formula/workspace semantics still recur because they dominate the repository's strong merged evidence. | Disclosed cluster limitation |
| Does scoring favor wording unique to one `AGENTS.md`? | Core tooling scores one explicit locked manager ecosystem rather than pnpm specifically; task text and subjective anchors do not cite either variant. Construction agents did see Baseline A. | Internal review passes; further independent neutrality audits are a recorded limitation |
| Are external dependencies reproducible? | Five distinct Cargo.lock byte sets resolved from the sealed cache with `CARGO_NET_OFFLINE=true`. TW-05 builds and exercises Rust, native Node tests, the Worker/parity path, benchmark, and portability directly under enforced network denial without invoking npm, pnpm, or yarn. GNU Bash 5, clone-local targets, binary hashes, WASM target artifacts, and free-space checks are recorded per attempt. | Pass |

## Construction pilot results

All pilots below are labeled `construction_pilot_only` and excluded permanently
from formal results:

- all nine clean historical bases passed the exact ordered, deduplicated union
  of their base and candidate-core command lists; the receipts are
  content-addressed but do not claim same-wave health or candidate correctness;
- seven historical targets were interactively observed passing focused checks,
  but no content-addressed receipts were retained, so those observations are
  contextual only and are not used for readiness;
- replacement TW-06 passed 140 workspace tests and the positive structural/link
  validator, while replacement TW-08 passed all 38 storage tests at its
  historical target;
- the rebased TW-09 pilot applied the feature behavior to base `7782114…`
  without the independent storage-envelope commits, passed 6 diagnostic and 19
  validation-report tests, and produced exact native/WASM records 27–30;
- all five distinct Cargo lock sets fetched/resolved with network disabled;
- raw candidate capture qualification preserved committed, staged, unstaged,
  ignored and ordinary untracked files, binary bytes, symlinks, executable mode,
  `assume-unchanged`, and `skip-worktree` changes while defeating hostile hooks,
  attributes, filters, and candidate Git configuration; a separate trusted
  object database/index and round-trip tree digest bind the result;
- oracle materialization rejected a symlink escape and did not write outside the
  validation root;
- a no-task Codex configuration smoke in an empty directory returned exactly
  `CONTROL_OK` with zero tool calls;
- untouched-base oracle discrimination showed semantic red tests where stable
  public seams existed, while TW-04/TW-05/TW-07/TW-08/TW-09 exposed
  candidate-interface coupling; this directly motivated assertion granularity
  and the adapter gate;
- retained portable captures and the rebased TW-09 pilot proved exact selected
  observations and native/WASM equality without scoring inherited records.
- the all-nine production-oracle qualification executed every core command and
  applicable frozen assertion on positive and behavior-missing/base-negative
  workspaces; selector-family negative fixtures rejected wrong Rust match
  counts, JSON-pointer values, and portable record sets;
- TW-05's direct offline runner completed without a package-manager command and
  with an independently probed OS network denial; its controlled reference
  runtime passes the frozen normalized contract, while the historical target's
  two stale-revision misses are retained as calibration evidence rather than
  rewritten away;
- deterministic blinded-packet qualification covered exact, whitespace/case,
  one-edit, identifier/path, residual-match, binary, symlink, invalid UTF-8,
  manifest-tamper, and independent-rescan failures, plus byte-identical repeat
  construction;
- one-shot controller smokes used local fake agents only. They qualified stage
  ordering, same-wave base controls before candidate exposure, no retry after
  failure or timeout, process-group extinction, raw output/final-message
  capture, empty-patch scoring, adapter pause/resume within the same attempt,
  external slot uniqueness, and append-only terminalization. No frozen benchmark
  task was sent to Codex;
- adversarial formal-adapter fixtures rejected caller-chosen scaffolds,
  expectation tokens and reads, config/probe overlap, candidate/trusted-input
  writes, reuse of an earlier agent TMP, and pre-extinction output. The qualified
  path uses the sealed scaffold, disjoint probe/config and independent approval,
  one outer network/read/write sandbox, a fresh empty adapter TMP, pre/post tree
  identities, and controller materialization after process-group extinction;
- same-wave base, candidate-core, and oracle stages actively proved kernel
  network denial, and formal review-builder/scanner fixtures proved immutable
  phase/attempt/candidate context binding. Standalone review helpers remain
  construction-only when that context is absent.

The replacement/rebase receipts, command stdout/stderr, hashes, selected
observations, and reconstructible TW-09 patch are frozen under
`evaluator/construction-pilots/` and indexed by
`evaluator/construction-pilot-index.json`. The index explicitly records which
release/workspace gates were not recaptured and permanently excludes every
artifact from formal scores.

Temporary pilot roots were deleted after receipts were recorded. Construction
never invoked Codex on a frozen benchmark task. The compact oracle qualification
summary has payload SHA-256
`a2613a91bb4907df2706e84088130727aefd2fa1f240a3b7c93f9013769da53a`
and run-receipt SHA-256
`f1a569c7c2cc2fd9d1a71fe15d670bf6f5d0babca58e178d06867c94e09b6fa7`,
and evidence commitment SHA-256
`e527df8c8edad212078976ed9b0dc27a4126ae42d6d2f11b4e5b2628c07382ac`.

## Known cluster and carry-forward effects

The cases come from a short August 2026 development window and are not IID.
Later historical bases legitimately contain earlier selected outcomes because
that was live repository state. Each observation still uses only its own base
ancestors; no task receives future descendants. TW-09's earlier-ancestor replay
is separately declared and omits only hash-locked independent outcome-only
commits. Aggregate analysis must show all nine raw cases and capability
clusters, not claim nine independent samples or generalize to unrelated
repositories.

## Operational closure

The practical internal-experiment implementation closes the locally actionable
readiness defects:

1. `production-oracles.json` maps all 9 cases, 27 frozen core commands, 58
   frozen oracle commands, 74 assertions, and 16 subjective groups without
   changing a selector or point.
2. `qualifications/oracles.json` binds all-nine positive and negative execution,
   selector-family discrimination, adapters, and TW-05 package-manager-neutral
   offline evidence. The verifier recomputes both its semantic payload and its
   separate content-addressed run receipt.
3. `preflight-run.mjs` and the controller record a closed environment, neutral
   instruction surface, overlay identity, free space, local runtime binary
   versions/hashes, WASM artifacts, model catalog, and the sealed controller
   bundle for each attempt.
4. `capture-candidate.mjs` performs raw lexical capture into a separate trusted
   Git object database/index and proves candidate patch/tree round-trip equality.
5. `run-controller.mjs` externally registers a unique wave/case/phase slot,
   executes its base control before exposure/launch, launches at most one agent
   process group, captures immutable output and candidate state only after group
   extinction, and commits one hash-chained terminal disposition. Adapter work
   can resume only the identical captured attempt; it cannot launch another
   agent. Empty patches remain valid scoreable candidates.
6. `build-review-packet.mjs` creates deterministic path-aliased packets from a
   hash-complete role manifest. The independent scanner validates the frozen
   R1–R4 contract, registered variants, public packet manifest, and zero residual
   matches before a terminal `qualified` release receipt. Formal receipts bind
   the controller phase/IDs/capture context, and the controller separately
   invokes and binds the standalone scanner receipt.
7. Formal adapters are restricted to the operationally locked scaffold plus
   disjoint candidate-exercising inputs and an independently approved integrity
   receipt. Kernel read/write/network confinement, fresh adapter-only TMP,
   pre/post candidate/input hashes, group extinction, and trusted output
   materialization prevent an adapter from manufacturing candidate behavior.
8. Base controls, candidate-core commands, and oracle commands execute only
   after an active kernel network-denial probe and bind the sandbox identity and
   supervision receipt.

An individual Baseline attempt remains fail-closed: missing external
authorization, catalog/tool/hash drift, insufficient disk, base-control failure,
instruction leakage, overlay drift, surviving descendants, capture mismatch, or
invalid review material prevents or invalidates that observation. Those are
per-run gates implemented by the controller, not missing infrastructure and not
candidate score deductions.

## Recorded limitations

- OpenAI does not expose an immutable dated deployment fingerprint for
  `gpt-5.6-sol`. The client, model ID, bundled catalog/model/base-instruction
  hashes, exact arguments, feature set, and locally controlled runtime are
  recorded instead. Provider account/project/entitlement details are recorded
  when available but cannot be independently attested by this repository.
- The practical internal runner relies on opaque paths, closed environments,
  stage-separated copies, content hashes, and an exclusive host. Dedicated OS
  account provisioning and an additional independent confinement audit would
  strengthen the boundary but are not required for the internal Baseline. The
  same-user runner fails closed on an observed root-overlay identity/content
  change but cannot attest against a mutate-and-restore-in-place sequence.
- Multi-reviewer panels are not required for Baseline readiness. Any available
  reviewer count and eligibility/conflict statement must be recorded; packet
  generation and release scanning remain mandatory.
- Constructors saw Baseline A. Further variant-blind and A-aware neutrality
  audits are desirable and remain mandatory for claims that depend on stronger
  independent governance, but their absence does not block this internal
  Baseline characterization.
- TW-05's frozen historical target does not satisfy two frozen stale-revision
  expectations. The contract was not weakened: a controlled real runtime is the
  qualified positive, the historical base is negative, and the target miss is
  retained in the qualification receipt.
- A future Variant B or paired A/B wave still requires Variant B provenance,
  both arms in the same newly registered wave, byte-identical controls, and the
  stronger pair cancellation/blinding rules. Standalone Baseline A output may
  not be recycled as a future A arm.

## Repository integrity

The construction did not modify `AGENTS.md`. Its frozen SHA-256 remains
`2179753f8e015f5c96e534ac633a3cdb2d10ffa7f98c3f608e351e929ade84d8`
(2657 bytes). No Variant B was created, edited, or recommended.
