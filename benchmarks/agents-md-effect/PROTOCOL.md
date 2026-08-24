# Benchmark Protocol

Protocol ID: `tachiko-agents-effect-v1`

## Research question

For authentic Tachiko Work repository tasks, how does changing only the root
`AGENTS.md` variant affect the quality of a controlled, single-agent Codex run?

Historical patch similarity is not the outcome. A different implementation can
score `strong` when it satisfies the accepted repository contract, preserves
compatibility, validates its behavior, and remains maintainable and scoped.

`READINESS.md` is normative. Static construction integrity does not authorize a
controlled run while environment identity, isolation, adapters, or result
binding remain unresolved.

## Experimental phases

### Phase 1 — construction

This phase reconstructs repository authority, selects cases, freezes prompts,
validates historical bases and available ground-truth evidence, records any
unqualified evaluator command/adapter infrastructure, and audits contamination.
Any end-to-end execution during this phase is labeled `construction_pilot` and
is permanently excluded from A/B results.

### Phase 2 — Baseline A

Run one fresh single-agent task for each case with the frozen Baseline A
`AGENTS.md`. These nine results are a standalone baseline characterization.
They are not automatically the A arm of a later experiment if the model
deployment, Codex CLI, toolchain, system prompt, or environment cannot be shown
to be identical.

### Phase 3 — future controlled A/B

After Variant B exists and is frozen without using these results as a hidden
test answer key, run fresh A and B arms. Pair by case, counterbalance arm order,
and serialize runs on the same validation host. Re-run A; do not compare a new
B run to an old A result across an unverifiable model or environment change.

Variant B must cross a benchmark firewall. Either it is a pre-existing,
independently motivated immutable revision, or its authors sign attestations
that they had no access to evaluator-only cases, frozen tasks, oracles, hidden
failure modes, rubric, construction pilots, or Baseline results. Freeze B's
provenance, bytes, and hash before those materials are unsealed. A variant
written or edited with benchmark-specific knowledge is ineligible even if its
author never executes a case.

### Phase 4 — optional Ultra

Ultra may use multi-agent delegation or autonomous orchestration under its own
separately registered configuration. Report it in a separate dataset with
`experiment_phase = "ultra_optional"`. It is neither an A nor B observation and
cannot be pooled into the controlled estimate. The single-agent
`multi_agent=false` constraint applies only to controlled Baseline/A/B phases.

## Unit of observation

One controlled Baseline/A/B observation is one fresh Codex process working on
one case, with no resumed thread, imported memory, prior case context, or
sub-agent. The primary A/B unit is the paired case difference. There are nine
case blocks and eighteen primary controlled A/B observations. Optional Ultra
uses its separately registered orchestration and observation definition.

## Wave registration and attempt ledger

`evaluator/contracts/wave-registration-v1.json` separates an immutable pre-run
wave registration from an append-only attempt ledger. Before the first provider
contact it commits the planned cases/order, controller and every control hash,
sealed variant-set and arm-key documents, audit receipts, provider context,
reviewer allocator/pools, and ledger genesis. Every attempted observation is
registered first with opaque 128-bit IDs and a hash-chain predecessor. Terminal
ledger entries later bind completed valid, invalid, or not-started records; the
record binds the pre-run attempt registration, avoiding a mutual hash cycle.

No attempt may be omitted or rewritten. A task-quality failure or timeout is a
final scored observation. One fresh agent replacement is allowed only for a
registered failure before any model output or candidate-dependent signal; a
second such failure cancels the case. After output exists, never resample the
model: re-run a failed validator/review stage at most once on the identical
immutable candidate when safe, otherwise cancel the case/pair. Protected-content
exposure or untrustworthy capture always cancels instead of resampling. Publish
the full ledger, final chain head, seed/arm-key openings, and invalid records
after the wave freezes.

## Locked controlled variables

Both arms use identical:

- model ID and effective model-catalog record;
- reasoning effort and reasoning mode;
- Codex CLI binary and base/system prompt;
- enabled tools and feature flags;
- frozen task bytes and prompt wrapper;
- historical base commit and tree;
- `AGENTS.md` location, discovery mechanism, and overlay procedure;
- sandbox, network policy, approval policy, environment variables, time limit,
  CPU/memory allocation, and validation image;
- evaluator tests, scoring rubric, reviewer eligibility pool, packet format,
  arm-blind allocation algorithm/seed commitment, randomization, and adjudication rules. Individual reviewer/custodian identities
  are disjoint across paired arms as required by the common blinding protocol;
  identical identities are not a controlled variable.

The only intended variable is the bytes of the root `AGENTS.md` overlay.

## Case-selection rules

Cases were drawn from real GitHub Issues, merged PRs, first-parent history,
accepted ADRs, shipped tests, CI evidence, and review findings. Selection
favored:

- a fixed pre-change base with all required authority already present;
- a merged outcome and deterministic evidence;
- public behavior or accepted semantic invariants rather than patch shape;
- a task a maintainer could plausibly assign at that point in history;
- complementary capabilities and independently scoreable outcomes.

Early Issue #4–#8 work was excluded because PR #16 bundled 101 files and 16,183
additions into one foundation with no clean independent replay boundary. PR #81
was excluded because its 73-file identity migration duplicated several selected
capabilities at much greater runtime and contamination risk. PR #102 was
excluded because a one-line configuration change had insufficient behavioral
ground truth for a primary case.

The set is repository-specific and clustered in the August 2026 M02 history.
It is not an IID sample of software engineering, and its aggregate must not be
generalized beyond Tachiko Work without replication. Report raw cases and
capability-cluster summaries so repeated native/WASM, formula/workspace, and
release-gate evidence is not mistaken for nine independent capabilities.

Eight targets are replayed from their direct parent. `TW-09` alone uses a
declared earlier-ancestor replay: that base contains its completed formula
prerequisite, predates the first root `AGENTS.md`, and omits three exact
first-parent outcome-only commits before the implementation parent. The
intervening sequence and the constructed case-local portable contract are
hash-locked. This exception is permitted only because construction pilots show
that the required behavior can be replayed without importing the independent
intervening storage work.

## Historical workspace isolation

Checking out a base in the full repository is forbidden for controlled runs:
descendant refs and unreachable Git objects can reveal the answer.

The trusted harness must:

1. create a temporary bare clone unavailable to the agent;
2. create one temporary ref at the case's historical base;
3. create a Git bundle containing only that ref and its ancestors;
4. clone the bundle with the temporary base branch checked out;
5. verify `HEAD` equals the manifest base SHA;
6. verify the target commit is absent with `git cat-file -e <target>` returning
   non-zero;
7. verify neither the base tree nor any reachable ancestor contains a root
   `AGENTS.md` path or blob;
8. remove GitHub remotes and deny network access;
9. expose no evaluator or current-repository directory to the agent OS user.

Ancestors are retained because they are legitimate historical repository
context. Descendants, PR branches, tags containing descendants, reflogs,
alternates, object pools, and local-reference paths are forbidden.

## AGENTS.md exposure

For every arm and case, the harness writes the selected variant to the checkout
root as `AGENTS.md` while leaving the historical `HEAD` unchanged.

- Every selected base and its reachable history is frozen as root-`AGENTS.md`-
  free. A case that violates this invariant is rejected; index flags or blob
  replacement are not permitted as a workaround.
- The harness adds `/AGENTS.md` to `.git/info/exclude` and provisions the
  registered bytes as a supervisor-owned, read-only, non-symlink root overlay
  protected by an OS/sandbox deny-write policy. It verifies that the path is
  absent from `HEAD` and all reachable objects before process launch.

The overlay is therefore discoverable through the same root-file mechanism in
both arms and does not appear as task work in normal status output. Its SHA-256
is checked before and after the run. An actual modification, deletion, or
replacement means the promised isolation failed and the instruction treatment
is no longer controlled: record agent/infrastructure attribution, mark
`invalid_run`, append terminal records, and cancel the pair/case slot without
replacement. A deliberate or repeated mutation attempt
that the control fully denies without changing/exposing bytes is a valid
candidate hard failure. Record and compare pre/post path, device, inode,
regular-file type, owner, mode, flags/policy receipt, byte count, and hash.
Before each wave, the agent identity must fail qualified write, truncate,
rename, unlink, and symlink-swap attempts. No parent-directory or user-level
instruction file may be readable to the agent. Both arms use this identical
untracked-overlay mechanism; only overlay bytes may differ.

## Information separation

Agent-visible task/experiment information is limited to the historical checkout,
the selected `AGENTS.md`, one frozen task statement, and generic runtime metadata
required to execute it. Account names, hostnames, home/tool/cache paths, and
workspace names are neutral and may not encode the protocol, benchmark, case,
arm, or variant; evaluator-only construction source paths are not mounted. Every
task begins with the same plain-language notice that external services, current
remote state, and descendant history are unavailable. The task files
intentionally omit:

- benchmark identity and scoring criteria;
- Issue/PR URLs and target commit IDs;
- merged implementation details and patch topology;
- tests introduced after the base;
- review findings that were not part of the maintainer assignment;
- known hidden failure modes.

Evaluator-only information lives under `evaluator/` and must be stored outside
the agent's readable filesystem during execution.

Construction agents necessarily saw the existing Baseline A instructions while
selecting cases. Two no-edit audits are therefore required. A variant-blind
non-constructor first checks historical authenticity, solution leakage,
capability balance, and generic rubric neutrality. After all compared instruction
bytes and the benchmark are immutable—but before any run—a separate comparison
auditor sees arm-label-free variant texts plus the frozen tasks and rubric and
checks variant-specific lexical/semantic overlap, authority cues, and tooling
favoritism. For standalone Baseline A this second review sees only frozen A; for
controlled A/B it sees both anonymized variants. Neither auditor may change the
benchmark or variants, propose instruction text, or see outcomes. A material
finding cancels the planned wave.

For each case, `authority-lock.json` freezes an assignment cutoff and the exact
unedited Issue/PR text that may justify the task. Later implementation, review,
and merge evidence is `outcome_only`: it may validate the evaluator after the
candidate is frozen but may not be supplied to the agent or quality reviewers.

## Validation

Validation has four ordered layers:

1. **candidate core:** exact public regression/repository commands on the clean
   captured candidate commit, before any evaluator file is added;
2. **oracle capsule:** hash-locked evaluator files compiled separately against
   immutable prebuilt candidate artifacts, with candidate-controlled build
   machinery unable to read or mutate the oracle;
3. **case assertions:** independent exact-test, JSON-pointer, or selected
   portable-record assertions from `oracle-lock.json`, executed only after probe
   source and expected values are unmounted;
4. **blinded review:** authority, test quality, semantic completeness, scope,
   maintainability, and churn that cannot be scored reliably from bytes.

Historical target tests are evidence and may be used when they exercise public
or explicitly required contracts. They are never scored by whole-suite exit
reuse: one point-bearing assertion belongs to one group, and selected portable
records exclude inherited records. Historical tests may not make private helper
names, file layout, or patch structure the outcome. When an alternative
interface makes a probe fail to compile, record `oracle_adapter_required`; a
variant-blind custodian may adapt names/types only, save the complete source or
diff, and obtain independent integrity review. Compile failure alone is not a
functional failure. A behavior-implementing adapter invalidates the run.

## Analysis

Report per-case raw dimension scores, outcome class, wall time, token use,
validation results, and reviewer variance. For the future A/B experiment report:

- paired B-minus-A total score and each dimension;
- median and mean across every valid paired difference, while retaining the
  planned denominator of nine and showing every missing/cancelled case block
  explicitly without imputation;
- exact sign count across cases;
- bootstrap confidence intervals across case blocks, labeled exploratory at
  this sample size;
- hard-failure and major-regression counts;
- runtime and token deltas separately from quality;
- the disposition of every pre-registered attempt (`valid`, `invalid`,
  `not_started`, `discarded`, `replacement`, or `cancelled`), including the
  replacement chain and cancelled pair/case slot; and
- invalidation and same-candidate stage-reexecution rates by arm, stage,
  attribution, and frozen reason code.

Do not collapse quality and cost into one undocumented composite. Do not tune
the rubric after variant labels are revealed. Invalid/discarded attempts remain
visible safety and reliability outcomes with their registered denominators; they
are never pooled into candidate quality scores or silently removed from arm
reporting.
