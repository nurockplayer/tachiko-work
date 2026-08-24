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
| Is validation fitted to the historical patch? | Patch similarity is never scored. Oracle v2 uses unique assertion IDs, exact single-test selectors, disjoint JSON pointers, and selected portable records only. Metadata and normalized contracts allow different correct implementations. | Pass for locked assertions; adapter qualification remains blocking |
| Can every base/target be checked out and related unambiguously? | All nine full SHAs and base trees resolve. Eight targets have the declared base as direct parent. TW-09 is the only locked exception: base `7782114…` is an ancestor of implementation parent `c685fe7…`, with exact outcome-only first-parent sequence `22fc8eb…`, `b5b097c…`, `c685fe7…`. Ancestor-only preparation pilots could not resolve target commits. | Pass |
| Is historical `AGENTS.md` exposure eliminated? | The first root `AGENTS.md` entered history at `22fc8eb…`. Every selected base predates that commit, and the verifier scans each base's reachable root-path history and fails closed if any root `AGENTS.md` object is found. The overlay is always untracked; tracked-blob quarantine was rejected because tree object IDs disclose the treatment. | Pass |
| Can agent-visible runtime metadata reveal benchmark identity or prior cases? | The controlled design now uses neutral account/host/tool/catalog names, fresh opaque workspace/HOME/CODEX_HOME/TMPDIR paths, an empty read-only non-reused HOME, and a required identity/path leak-scan receipt. Construction-source/cache paths remain evaluator-only. | Design is neutral; staging and negative qualification remain blocker 2 |
| Do historical bases and outcomes build and satisfy their evidence? | Content-addressed construction-only receipts show all nine clean historical bases passing each case's ordered, deduplicated base/core command union. Separate retained target receipts cover TW-06's 140 workspace tests and structural gate, TW-08's 38 storage tests, and the rebased TW-09's diagnostic/validation/portable evidence; seven earlier interactive target observations were not retained and are not readiness evidence. None of these receipts substitutes for same-wave base control or all-nine target-positive/oracle-negative qualification. | Base construction pilot passes; target and formal wave qualification remain partial/blocking |
| Do cases rely on authority created after assignment? | Allowlisted authority is at or before each cutoff; later implementation/review is explicitly outcome-only. Unrecoverable edited text is excluded. | Pass |
| Are capabilities needlessly duplicated? | Nine cases cover identity authority, storage-contract remediation, numeric conversion, crate architecture, resident-runtime research, governance reconciliation, graph oracle, legacy persistence, and staged diagnostics. Native/WASM/release gates and formula/workspace semantics still recur because they dominate the repository's strong merged evidence. | Disclosed cluster limitation |
| Does scoring favor wording unique to one `AGENTS.md`? | Core tooling scores one explicit locked manager ecosystem rather than pnpm specifically; task text and subjective anchors do not cite either variant. Construction agents did see Baseline A, so this internal audit cannot substitute for both required independent roles: a variant-blind intrinsic audit and a post-freeze A-aware/no-edit comparison audit. | Internal review only; both independent signoffs remain blocking |
| Are external dependencies reproducible? | Five distinct Cargo.lock byte sets resolved from the sealed cache with `CARGO_NET_OFFLINE=true`; the three retained target/rebase receipts also used the offline lock. GNU Bash 5, clone-local Cargo targets, and ≥10 GiB free are locked. | Cargo/Rust construction evidence passes; general Node-manager neutrality is not yet proven |

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
- a happy-path candidate capture/apply pilot preserved committed, uncommitted,
  ordinary untracked, binary, and symlink changes, but later adversarial review
  showed that this implementation trusts agent-mutatable Git index flags,
  ignore rules, attributes, filters, and configuration; it is not qualified;
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

The replacement/rebase receipts, command stdout/stderr, hashes, selected
observations, and reconstructible TW-09 patch are frozen under
`evaluator/construction-pilots/` and indexed by
`evaluator/construction-pilot-index.json`. The index explicitly records which
release/workspace gates were not recaptured and permanently excludes every
artifact from formal scores.

Temporary pilot roots were deleted after receipts were recorded. Construction
never invoked Codex on a frozen benchmark task.

## Known cluster and carry-forward effects

The cases come from a short August 2026 development window and are not IID.
Later historical bases legitimately contain earlier selected outcomes because
that was live repository state. Each observation still uses only its own base
ancestors; no task receives future descendants. TW-09's earlier-ancestor replay
is separately declared and omits only hash-locked independent outcome-only
commits. Aggregate analysis must show all nine raw cases and capability
clusters, not claim nine independent samples or generalize to unrelated
repositories.

## Remaining blocking defects

1. **Provider deployment identity:** client binary, catalog, model record, base
   instructions, feature list, task, and tools are locked, but `gpt-5.6-sol`
   exposes no immutable dated backend snapshot/deployment fingerprint. “Exact
   same model” across future arms cannot currently be independently attested.
   The fresh `CODEX_HOME` invocation also lacks a provisioned sealed credential
   path and hashed provider account/project/entitlement context.
2. **Agent/validator/controller isolation:** the required dedicated accounts
   have not been provisioned or verified. In particular, candidate-controlled
   Cargo build scripts/tests must run in a validator identity that cannot read
   the full controller, source repository, arm key, other runs, or trusted
   receipts. Candidate build machinery also cannot be allowed to read or mutate
   hidden oracle source: production artifacts and evaluator probes require
   separate staged capsules, immutable pre/post hashes, and hostile build.rs,
   rustc-wrapper, proc-macro, and test negative controls. None has passed. The
   exact locked runtimes/caches also have not been staged and leak-scanned at
   their fixed neutral `/opt/isolated-runtime` paths; evaluator-only construction
   source paths must remain unreadable.
3. **Adapter execution:** TW-04, TW-05, TW-07, TW-08, and TW-09 require
   candidate-interface adapters for solution-neutral scoring. No complete
   adapter families have yet passed both historical-ground-truth and
   behavior-missing negative-control qualification, and no trusted insertion
   stage records their evaluator commit/review. The literal historical
   `cargo test`/portable commands in `oracle-lock.json` are explicitly
   construction-only; no production command manifest yet maps every unchanged
   semantic assertion to the separated artifact/probe/execution stages. The
   all-nine historical targets have not yet passed content-addressed positive
   qualification of every core/assertion path plus behavior-missing and
   base-negative discrimination controls.
4. **Semantic result controller:** scripts freeze inputs and trees, but no
   end-to-end controller yet substitutes locked commands, enforces exactly one
   Rust test match, resolves JSON pointers, applies caps, validates point/ID
   arithmetic against all locks, enforces valid/invalid stage semantics, passes
   positive and adversarial record fixtures, and emits the signed
   semantic-validation receipt required by the result schema.
5. **TW-05 tool neutrality:** the frozen task allows one reproducible locked
   Node manager, but only the pnpm runtime/cache path has been construction-
   qualified offline. Either equally lock and qualify every allowed workflow or
   freeze a historically justified manager requirement before a controlled run.
6. **Effective preflight/runtime integration:** the exact
   feature/MCP/prompt/catalog checks are specified and individually piloted, but
   are not yet one fail-closed controller preflight tied to each result. The
   construction-only all-nine base-union receipts now pass; the mandatory
   same-wave base control is still absent. The runtime verifier also does not yet
   bind hashes for the invoked Cargo/rustc/rustup/rustfmt/clippy binaries, the
   installed WASM target artifacts, and every locked Git/runtime executable.
7. **Trusted complete candidate capture:** the current prototype runs `git add
   -A` through the candidate repository. Agent-set `skip-worktree` or
   `assume-unchanged` flags and ignored files can evade it, while candidate
   `.gitattributes`, filters, hooks, or local configuration can transform or
   execute during privileged capture. A raw-filesystem, no-filter trusted
   object/index implementation and adversarial negative qualification are
   required.
8. **Reviewer eligibility and blindness:** conflict/prior-exposure attestations,
   an eligibility registry, and standalone-Baseline packet-only reviewer
   assignments are specified but not implemented or independently exercised.
   Constructors, historical PR participants, oracle/adapter custodians, and
   experiment operators must be excluded case by case. Future paired A/B also
   requires disjoint panels across arms. The frozen packet-wide variant-text
   scanner/redactor and receipt contract has not been implemented or passed its
   exact/near-copy, path, residual-match, and fail-closed qualification fixtures.
   The byte-exact reviewer allocator likewise lacks independent reference-vector
   replay and impossible-order negative qualification.
9. **Independent neutrality audits:** case constructors were exposed to Baseline
   A. Before standalone Baseline A, a variant-blind non-constructor must audit
   historical authenticity, leakage, capability balance, and generic rubric
   neutrality; a separate post-freeze A-aware/no-edit auditor must check A-specific
   overlap and favoritism without seeing outcomes. Neither has occurred. A future
   controlled A/B wave requires the second auditor to compare both frozen,
   anonymized variants while blind to arm labels, plus Variant B
   benchmark-firewall provenance/access attestation before evaluator internals
   are unsealed.

These are readiness defects, not low-score conditions. They must invalidate or
prevent a run rather than be charged to a candidate.

## Repository integrity

The construction did not modify `AGENTS.md`. Its frozen SHA-256 remains
`2179753f8e015f5c96e534ac633a3cdb2d10ffa7f98c3f608e351e929ade84d8`
(2657 bytes). No Variant B was created, edited, or recommended.
