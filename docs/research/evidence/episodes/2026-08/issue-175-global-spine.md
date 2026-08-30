---
schema: research-evidence/v0
episode: issue-175-global-spine
capture_mode: prospective
capture_status: active
captured_at: "2026-08-30T18:52:40Z"
repository: nurockplayer/tachiko-work
base_sha: 022a14d18503477aa7e20f6fca102f9e85dce740
issue: https://github.com/nurockplayer/tachiko-work/issues/175
authority_state: Hypothesis
agent:
  interface: codex-desktop
  provider: unknown
  model: unknown
  configuration: coordinating writer with three bounded read-only Luna audit roles; release benchmark harness
context_manifest_status: partial
intervention_classes: [correction, constraint_addition]
failure_classes: [implementation_assumption, reasoning_error, tooling_failure]
links:
  issues:
    - https://github.com/nurockplayer/tachiko-work/issues/175
    - https://github.com/nurockplayer/tachiko-work/issues/174
  prs:
    - https://github.com/nurockplayer/tachiko-work/pull/195
  adrs:
    - ../../../../decisions/ADR-0017-versioned-storage-and-canonical-representation.md
    - ../../../../decisions/ADR-0018-formula-semantics-and-complete-calculation.md
    - ../../../../decisions/ADR-0019-validation-authority-and-reporting.md
    - ../../../../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md
  specs:
    - ../../../../specs/roproj-format.md
  tests:
    - ../../../../../crates/storage/src/roproj/v1/issue_175_research.rs
    - ../../../../../crates/workspace-engine/tests/issue_175_residency.rs
  evidence:
    - ../../../2026-08-31-exact-global-spine.md
    - ../../issue-175/manifest.json
---

# #175 — Exact Global Spine and progressive `.roproj/v1` open

This prospective record preserves the falsification experiment without
promoting its recommendation into Accepted architecture.

## Question / material decision

After removing avoidable cost from native `.roproj/v1` eager admission, does an
exact Global Directory / Structural Index plus bounded materialization provide
enough latency or memory benefit to justify additional runtime and
completeness complexity?

## Hypothesis

A compact exact Global Spine might materially improve time to a requested
exact bounded result or peak memory on at least two realistic large workload
classes, without weakening full-oracle correctness, degrading foreground work,
or hiding eventual full-residency and sidecar-validation cost.

The preregistered materiality threshold was approximately `>=2x` p95 latency
or `>=40%` peak-RSS improvement over optimized exact A1, on at least two
realistic large classes, with no `>10%` foreground regression.

## Baseline

A0 was the current eager native host implementation: exact source read,
canonical admission that decodes and re-encodes the tree, followed by ordinary
load's second semantic decode.

A1 was a research-only optimized exact eager implementation preserving
manifest-first dispatch, the exact 18-file tree, strict JSON and canonical
spelling, shard/order/identity and uniqueness laws, semantic validation, and
formula limits while constructing one complete `Document` without whole-tree
re-encode or second decode.

## Alternatives

- B: non-authoritative progressive source preview over A1.
- C: exact Global Directory and exact Global Structural Index.
- D: verified pinned spine plus bounded payload materialization.
- E1: disposable sidecar for arbitrary/dirty filesystem source.
- E2: disposable sidecar bound to immutable Git identity.
- F: spine working set and eventual full residency.

No alternative authorized partial semantic authority, a canonical sidecar,
`.roproj/v2`, a public readiness/cache protocol, or production implementation.

## Governing authority

Issue #175 body and its canonical handoff governed scope, arms, correctness
pressure, measurements, and gates. Parent #174 owns the unresolved architecture
decision. ADR-0017, ADR-0018, ADR-0019, and ADR-0023 governed versioning,
canonical representation, complete formula calculation, validation authority,
and exact `.roproj/v1` layout.

Research evidence cannot amend those Accepted records.

## Context / source manifest actually supplied

Confirmed supplied context included #175 and its canonical handoff, parent
#174, live `main@022a14d18503477aa7e20f6fca102f9e85dce740`, open PR overlap
for #188 and #193, current storage/formula/workspace code and tests, Accepted
ADRs/specifications, prior #41/#91/#95/#96 harness patterns, and three bounded
read-only audits covering benchmark method, full-oracle/adversarial behavior,
and spine/sidecar/RSS accounting.

The exact hidden system prompt, provider deployment, sampling state, and model
revision are `unknown`. The committed benchmark manifest records the observable
machine, toolchain, commands, exact measurement commit, cache state, and
artifact hashes.

## Initial recommendation or result

No architecture winner was selected before measurement. Early A0/A1 smoke
evidence suggested A1 removed substantial duplicate work, while early C size
ratios varied sharply by workload.

The first 2,400-row matrix attempt was not promoted: A0 included teardown in
its timestamp, C included instrumentation-only traversal and size
serialization, and Structural retained a duplicate Directory. Only its A1
rows were provisionally usable, and all rows were replaced for the final
paired conclusion.

## Human intervention

The Project Steward's hourly AMBER checkpoint required the canonical handoff
and review inventory to catch up before further authoritative measurement. It
emphasized exact-layer rejection parity, sidecar source binding, and symmetric
timer/RSS boundaries.

Independent reviews additionally identified A1-only accounting traversal,
UTF-8 fixture truncation, percentile definition, codec-versus-host labels,
parser-pass accounting, late-invalid physical placement, dirty-sidecar TOCTOU,
duplicate spine residency, and full-oracle distinctions. These corrections
were accepted where they matched Issue and Accepted authority.

## Failures / incorrect assumptions / authority drift

- Initial A0 matrix timing included `Document` destruction after open.
- Initial C timing included counter-only AST work and size-only serialization.
- Initial Structural RSS retained two Directory copies.
- The first A1 fixture truncation treated a byte count as a character count.
- The first percentile helper used a lower-rank index rather than explicit
  nearest-rank `ceil(n*p)-1`.
- Dirty-sidecar open initially trusted a cached expected fingerprint without
  re-reading the live root.
- One exploratory E2 launch accidentally ran three scales concurrently. Those
  files were discarded and all E2 scales were rerun sequentially.
- The first background-cancellation implementation checked cancellation only
  after an unbounded record read, and dependency accounting mixed target
  occurrences with unique graph edges. Both were corrected before the final
  capture: cancellation now polls between bounded 64 KiB chunks and every arm
  reports the same unique-edge definition.
- Self-hashed sidecar payloads initially proved representation integrity but
  not that every cached fact came from the bound source. The first correction
  re-derived the index, and a second audit caught a live-tree race after that
  scan. Final E reuse pins the exact 18-file snapshot, independently derives
  the index, and requires complete equality while retaining the pinned source.
- The first B interference capture always ran baseline first, warming the
  deterministic resident navigation path before the background arm. Final B
  alternates order and records it per row; its p95 paired ratio is `1.103` and
  therefore fails the 10% foreground-regression gate.
- The first corrected E2 still derived trusted facts and pinned bytes from the
  mutable worktree. Final E2 binds object format, commit, tree, modes, paths,
  blob IDs, and reads the exact blob objects before independently re-deriving
  every Structural Index fact. A dirty-worktree test proves reuse is sourced
  from the immutable objects.
- Progressive preview originally used an unbounded, uncancellable record read,
  and final semantic validation could publish after a late cancellation. Final
  preview is exact-layout checked, cancellable, and capped at 64 KiB; admission
  checks cancellation before, between, and after final validation passes.
- Sidecar A1 comparison originally timestamped after dropping the complete
  `Document`, while D added one shared pin duration to every warmed lookup.
  Final E retains A1 through the timestamp; final D resamples pinning and the
  first materialization in every repetition.
- Initial raw A0 rows doubled parser-byte work but not decoded record, AST,
  reference, or dependency counters for the second logical decode. Final rows
  aggregate every decode-work counter symmetrically. A fixture manifest now
  binds explicit seed `175`, revision, and source hash to every workload cell.
- Controlled cold-cache evidence was unavailable because macOS denied
  `/usr/sbin/purge`.

No failed or cancelled measurement was silently removed from the committed
authoritative captures.

## Corrections

The harness moved accounting outside asymmetric timers, timestamped A0 while
the `Document` remained alive, collected C counters during required structural
traversal, moved size serialization outside scan time, retained only one
Directory, separated nesting and parser/deserializer byte work, used
nearest-rank percentiles, added equivalent host-open measurements, and labeled
all cache/process boundaries.

Exact-layer validation was extended for global/scoped key laws, schema targets,
required fields, types, reference coverage, empty titles, late-invalid source,
and complete formula limits. Sidecars gained explicit algorithm/source binding,
whole-envelope integrity, live dirty-source revalidation, Git blob-derived
source identity, mismatch tests, and fail-closed fallback. D moved to a pinned
source snapshot and returns `RequiresFullAdmission` for unsupported semantic
proof.

A0/A1/C/F captures were regenerated at `4ed7f994` after the cancellation and
edge-accounting corrections. Final B was regenerated at `be4d801`; D, E1, and
E2 were regenerated at `8ce55419` after per-observation pinning, A1 lifetime,
and immutable-object trust corrections. The earlier
`1b3d75de`, `bd115c8`, and interrupted exploratory E2 files were discarded.
Cold-cache cells remain absent and explicitly unavailable rather than
relabeled.

The final exactness correction at `607e920` proves exact A0/A1 `Document`
equality across a valid chain, cold numeric mutation, cross-cold SCC, and
division-by-zero pressure. The same shapes independently preserve complete
calculation and workspace stable observations. C/D still publish no semantic
result and return `RequiresFullAdmission` where the full oracle is required.

## Final outcome

**Outcome A — reject/defer Global Spine**, with **B — progressive UX only** as
an optional bounded shell/source-preview technique over optimized exact A1.
The measured background-admission schedule is rejected because it fails the
10% foreground-regression gate.

A1 materially improves current A0, but C does not beat A1 at the `>=2x` gate.
Structural size is compact only in payload/constant-AST shapes and expands
mixed, reference, field, chain, and cycle classes. Fresh-process RSS shows no
40% reduction and exposes higher spine-plus-eventual-full peaks. D is useful
for exact source payload navigation but cannot publish complete formula or
validation truth. Exact E1 is 1.87x slower than A1 at p95 on 16k mixed data;
exact E2 is 2.33x slower. The faster E2 identity-plus-decode cell is explicitly
non-authoritative because it does not prove source-derived facts.

No ADR, `.roproj/v2`, public protocol, canonical sidecar, or production
successor is authorized by this outcome.

## Traceability

- [#175 research Issue](https://github.com/nurockplayer/tachiko-work/issues/175)
- [#174 parent Decision](https://github.com/nurockplayer/tachiko-work/issues/174)
- [PR #195](https://github.com/nurockplayer/tachiko-work/pull/195)
- [Definitive research report](../../../2026-08-31-exact-global-spine.md)
- [Evidence manifest](../../issue-175/manifest.json)
- [Matrix raw samples](../../issue-175/matrix-raw.csv)
- [Matrix derived summary](../../issue-175/matrix-summary.csv)

## Downstream observations

- 2026-08-31: authoritative measurements and the A/B/C/D recommendation were
  prepared for return to #174. Steward acceptance, merge disposition, and any
  separately authorized production follow-up were not yet observed.
