# Tachiko Obstacle Course v0

Status: implementation evidence; not protocol or conformance authority

Tracking issue: [Issue #213](https://github.com/nurockplayer/tachiko-work/issues/213)

## Purpose

The obstacle course is a compact release-mode check for a few load-bearing
repository journeys. It answers one engineering question:

> Does this exact checkout still preserve the selected end-to-end correctness
> invariants, and what informational performance/resource evidence did the same
> run observe?

It does not replace crate tests, the release gate, the deep performance research
in Issues #174/#175, or the future independent-implementation conformance work
owned by Issue #51.

Run the course from the repository root:

```sh
bash scripts/obstacle-course.sh
```

The command is offline by default. Cargo dependencies and a supported Rust
toolchain must already be available locally; the selected compiler identity is
recorded in the result. The runner resolves `RUSTC`, then
`CARGO_BUILD_RUSTC`, then the `rustc` on `PATH`; it exports that selection as
the single Cargo compiler and neutralizes competing compiler/wrapper selectors.
The reported identity and native target therefore come from the same compiler
used by Cargo. Native test execution also normalizes the exact target runner to
an `env` passthrough, overriding user or repository Cargo runner configuration.
`--list` prints the closed v0 stage registry without building or running it.

## v0 course

The course has exactly four correctness stages. All production behavior and
fixtures remain owned by their existing crates or journeys; the runner only
orchestrates them.

| Stage | Invariant | Existing owner |
| --- | --- | --- |
| `repository-dogfood` | The canonical Product Gaps `.roproj` admits successfully and repeated formula calculation is byte-identical with the pinned priority values. | `dogfood/product-gaps.roproj/`; its deterministic constructor and pin remain under `apps/designer/runtime/fixtures/` and `apps/designer/runtime/tests/`. |
| `git-review-roundtrip` | Direct `.ro` materializes to canonical `.roproj`, a scalar edit remains one Git-reviewable shard record, semantic review agrees inside/outside Git, package consistency is fail-closed, and invalid representation/semantics are rejected. | `examples/game-balance/game-balance.ro` and `scripts/git-ci-smoke.sh`. Storage fixtures remain under `crates/storage/tests/`. |
| `semantic-runtime` | Repeated bounded Query lineage is exact; an approved proposal previews, executes, verifies, and records provenance; a scalar publication invalidates the changed field and downstream formula projection at the new revision; failed-formula diagnostics retain stable subjects. | Focused existing tests in `crates/workspace-engine/tests/{analysis_operations,patch_lifecycle,resident_session}.rs` and their `tests/common/` fixture. |
| `retained-workspace` | Across deterministic 10/100/1,000-entity generated workspaces and 20 edits, retained calculation/validation equals the fresh full oracle and the exact recompute/reuse counters remain bounded. | `crates/workspace-engine/tests/retained_state_benchmark.rs`. |

The Git-review stage supplies durable intentional-failure proof through normal
product boundaries. It creates a noncanonical extra `.roproj` shard and a
division-by-zero edit, requires both operations to fail, checks the diagnostic,
and confirms the canonical source/package bytes are unchanged by those
read-only rejection checks. The runner has no fault-injection production path.

The authoritative semantics exercised here remain in the Accepted decisions and
their specifications:

- canonical storage and `.roproj/v1`:
  [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md),
  [ADR-0023](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md), and
  [`roproj-layout-v1.md`](../specs/roproj-layout-v1.md);
- formulas:
  [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
  and
  [`formula-engine-spec.md`](../specs/formula-engine-spec.md);
- validation and diagnostics:
  [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
  and
  [`diagnostics-contract.md`](../specs/diagnostics-contract.md);
- portable package consistency and fail-closed publication:
  [ADR-0025](../decisions/ADR-0025-portable-package-v1.md) and
  [`portable-package-v1.md`](../specs/portable-package-v1.md);
- headless Query/Propose/Execute and resident publication:
  [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md),
  [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md),
  [ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md),
  [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md),
  and [`semantic-api.md`](../specs/semantic-api.md);
- semantic review meaning:
  [ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md) and
  [`semantic-diff-spec.md`](../specs/semantic-diff-spec.md).

The current rendered diff is implementation evidence, not a new wire shape for
`tachiko.semantic-delta/v1`.

## Result and evidence contract

Correctness and performance/resource evidence are deliberately separate.

Each correctness stage prints `STAGE <id> PASS|FAIL` plus its invariant. The
runner attempts all four stages, prints `N/4 correctness stages passed`, and
returns nonzero when any correctness stage fails. A failure includes the stage,
sample, exit status, and captured stdout/stderr.

Before a named test-backed stage runs, the runner enumerates its exact test
target and requires the selected test to exist exactly once. Renaming or
removing a load-bearing test therefore fails the course instead of producing a
successful zero-test run. The exact invocation includes ignored tests, so a
change in ignore polarity cannot silently skip the selected test body.

Before running, the command reports:

- the full 40-character Git commit and `clean`/`dirty` state;
- release profile and offline network policy;
- OS/architecture and Rust compiler identity;
- a versioned workload ID and SHA-256 manifest digest for every stage.

A dirty run is provisional because the commit alone does not identify its
source. Final evidence must come from a clean exact HEAD. The runner rechecks
the commit, clean/dirty state, and a byte-sensitive fingerprint of tracked
changes plus relevant untracked file contents after setup and after every stage.
It also recomputes each exact workload-tree manifest at those checkpoints.
Regular files and every directory path, including empty directories, are bound
to that manifest. Ignored entries, symlinks, and special entries inside a
workload path are rejected before execution and at checkpoints, because a
stage could observe them even though Git excludes them from ordinary
source-state enumeration. The run is rejected if source or workload identity
changes while evidence is being collected. Repository-selecting and
command-scoped Git environment variables, including inherited author/committer
identity and dates, are cleared; every identity query is bound to the course
checkout; and the Git-review stage ignores user/system Git configuration such
as commit signing and hook paths. The runner also creates a fresh run-scoped
Cargo target directory, derives and pins the native host target from the
compiler Cargo uses, and uses
`<run-target>/<native-target>/release/`. Every CLI stage therefore uses the
platform-named release binary built by the same run rather than an inherited,
Cargo-config-selected, concurrently written, or stale artifact. The run target
is removed on exit.

The runner builds release artifacts before measurement. Build/setup time is
excluded. Each stage runs in a fresh process; OS cache state is explicitly
uncontrolled. The retained-workspace stage runs three samples and preserves the
raw per-size microsecond/counter rows. Where the platform exposes it, the runner
also records peak RSS for the complete stage process tree and normalizes it to
KiB. Unsupported resource collection is reported as `unavailable` and never
changes correctness.

All `PERF` and `PERF_WORKLOAD` lines are informational:

- there is no timing or RSS threshold in v0;
- samples are not called cold or warm when OS cache state is uncontrolled;
- stage-process-tree RSS is not a component-level memory attribution;
- absolute results must not be compared across OS/toolchain/hardware as if they
  were one population;
- deterministic oracle/work-counter assertion failures still fail the owning
  correctness stage, regardless of speed.

## Changing the course

Course changes are reviewable behavior changes, not incidental script cleanup.

To add a stage:

1. name the existing Accepted or explicitly Provisional repository authority;
2. justify why the journey is load-bearing and not redundant with a current
   stage;
3. use a public production boundary and an existing owned fixture/generator;
4. keep it deterministic, offline, release-mode, and inexpensive enough for
   ordinary local use;
5. define the exact correctness invariant and deterministic failure evidence;
6. keep timing/RSS observations separate from pass/fail; and
7. bump the course/workload version when the selected capability set or workload
   meaning changes.

To change a stage, preserve or strengthen its named invariant. Update the
workload identity, ownership table, and focused runner test in the same change.
Never make a regression disappear by weakening an assertion, silently swapping
fixtures, or filtering the failing case.

To remove a stage, record why the journey is no longer load-bearing or where an
equal-or-stronger gate now owns it, retain the historical rationale in the
change/Issue record, and bump the course version. Removal must not be used to
hide a current regression.

## Deliberate v0 exclusions

- No required hosted check is added in v0. Existing repository gates run the
  non-benchmark product/runtime surfaces, but the retained-state workload is
  intentionally ignored unless requested explicitly. This runner establishes
  the missing fresh stability evidence; hosted promotion requires a later
  measured case that the complete subset is stable and inexpensive.
- Native/WASM parity remains in `scripts/portable-conformance-check.sh` and the
  release gate. The course does not copy or subset its 55 fixed-oracle records,
  because conformance authority belongs to Issue #51.
- The Issue #175 Global-Spine matrix, sidecars, private research generators, and
  large RSS experiment are not promoted into this runner. v0 only reuses the
  already-owned 10/100/1,000-entity retained-state workload and measurement
  discipline.
- No CI/release refactor, benchmark framework, Dashboard dependency, network
  service, model call, or hostname-specific behavior is introduced.

The public registry/fail-closed option seam, exact-test execution, compiler/Git
environment isolation, ignored-workload rejection, and run-scoped native
artifact binding have a focused check:

```sh
bash scripts/obstacle-course-test.sh
```
