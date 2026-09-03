# Codex Worktree Housekeeping

Status: Provisional operational policy and implementation evidence for Issue
[#271](https://github.com/nurockplayer/tachiko-work/issues/271).

Authority: the repository [delivery workflow](../governance/project-governance.md#repository-delivery-workflow),
`AGENTS.md`, `CONTRIBUTING.md`, and the existing release/native/WASM
correctness gates. This document describes replaceable repository tooling; it
does not introduce a semantic, runtime, persistence, or Conductor contract.

## Safe GC command

Run the audit from a repository checkout:

~~~sh
bash scripts/codex-worktree-gc.sh
~~~

The default is a dry run. An explicit apply is required before any registered
worktree can be removed:

~~~sh
bash scripts/codex-worktree-gc.sh --apply
~~~

The command considers only registered Git worktrees below the Codex root. It
does not scan arbitrary directories and does not use directory age as deletion
authority. The root is selected in this order:

1. `--codex-root PATH`;
2. `CODEX_WORKTREE_ROOT`; or
3. `$HOME/.codex/worktrees`.

`--repository PATH` selects the repository checkout being inspected and
protects it as the current checkout. The primary worktree (the first worktree
reported by Git) is protected independently. A path that resolves through a
different filesystem path or escapes the configured root is not eligible.

Each row reports a classification, registered relative path, reason, and
measured disk usage:

| Classification | Meaning |
| --- | --- |
| `PROTECTED` | The primary or selected current checkout. Never removed. |
| `KEEP` | Known local state that is not terminal, such as an open PR, a closed-unmerged PR, or a locked worktree. |
| `DIRTY` | `git status --porcelain=v1 --untracked-files=all` found tracked or untracked changes. |
| `UNKNOWN` | Local, GitHub, repository, path, or identity state could not be proved. Never removed. |
| `DELETE` | Local state is clean and one live merged PR exactly matches the worktree identity. Eligible only in apply mode. |

For `DELETE`, the command proves all of the following immediately before
removal: the worktree is registered and present; the registered and observed
HEAD agree; the registered and observed branch agree; status is clean; exactly
one live PR matches; the PR head branch, head SHA, base branch, and head
repository match; and the PR is `MERGED` with a non-empty live merge timestamp.
Open PRs are always kept. Closed-unmerged or abandoned work remains kept until
an explicit bounded policy exists.

GitHub lookup is read-only through `gh`. A missing CLI, unavailable API,
ambiguous PR result, incomplete response, or identity mismatch produces
`UNKNOWN`. A completed audit containing any `UNKNOWN` exits with status 2, so
an external caller cannot mistake an unresolved audit for a safe complete
result. Other safe classifications do not authorize deletion.

Dry-run mode does not invoke `git worktree remove` or `git worktree prune`.
Apply mode rechecks each candidate against live local/GitHub state, removes it
without `--force`, and then invokes `git worktree prune --verbose` only after a
fresh guard proves that no prunable registration exists anywhere in the
repository. An in-scope or out-of-scope prunable registration blocks apply
before candidate removal and skips prune after a race; it reports a blocked
result rather than discarding an unresolved registration. Prune is part of the
post-terminal registration lifecycle; it does not authorize removal of a
worktree. The command protects both the selected `--repository` checkout and
the actual worktree containing its invoking process, so it never deletes the
worktree in which it runs.

The summary reports the number of registered Codex worktrees, each
classification, and estimated/proven reclaimable space from `du`. An
unavailable disk measurement is itself unresolved and cannot produce an
eligible deletion.

The external lifecycle owner remains the scheduler/orchestrator. It may invoke
this command after a durable terminal delivery state has been established
outside the ephemeral worktree. This repository command only decides whether a
specific worktree is eligible and reports the outcome; it does not implement a
delivery state machine, retries, persistence, or Conductor integration.

## Rust build artifacts

Active worktrees retain independent Cargo target directories. Use the thin
agent helper for ephemeral validation:

~~~sh
bash scripts/codex-cargo.sh test --workspace --all-targets --locked
~~~

The helper sets `CARGO_TARGET_DIR` to the current worktree's `target/` when the
caller has not supplied one, and rejects a target that resolves outside the
current worktree or is a symlink, and rejects Cargo's `--target-dir` override.
This prevents concurrent agents from sharing one mutable `CARGO_TARGET_DIR`. The
existing `scripts/release-check.sh`, CI commands, and
native/WASM conformance commands remain the correctness and reproducibility
authority; the helper is not a replacement for them.

`CARGO_INCREMENTAL=0` is the default for the helper only. It is an
ephemeral-agent optimization selected for measurement, not a global developer
default. Set `CARGO_INCREMENTAL=1` explicitly when reproducing the comparison.

`sccache` is not required and is disabled by default. It can be evaluated or
used explicitly:

~~~sh
TACHIKO_CODEX_SCCACHE=1 \
  SCCACHE_CACHE_SIZE=5G \
  bash scripts/codex-cargo.sh check --workspace --locked
~~~

When enabled, each worktree still has its own `target/`; only compiler outputs
may be shared through the bounded cache. `scripts/codex-rustc-wrapper.sh`
delegates each compiler invocation exactly once. A missing cache binary is
bypassed, and the helper enables sccache's `SCCACHE_IGNORE_SERVER_IO_ERROR=1`
fallback so a server-I/O outage uses direct `rustc`; a real compiler failure is
never retried as a cache failure. A cache miss or outage therefore cannot change
semantic results or make the repository correctness gates unavailable. The
cache is an optimization, not a correctness dependency. The portable Cargo
fixture probes propagation of the server-I/O fallback contract, while a
separate missing-binary integration path proves an uncached build succeeds.
Cache hit/miss and incremental on/off timing and disk evidence belong in the
Issue/PR evidence record.

## Issue #271 evidence

Measured on 2026-09-04 at commit `b37960b` with Rust 1.97.1, sccache 0.17.0,
and fresh linked worktrees. The incremental comparison used
`cargo check --workspace --all-targets --locked --quiet` through the helper,
with the cache disabled and a private `target/` in each worktree. The
worktree-size column includes source plus Cargo artifacts; the target-size
column isolates the mutable build directory.

| Fresh worktree policy | Before (KiB) | Real time | After (KiB) | Target after (KiB) |
| --- | ---: | ---: | ---: | ---: |
| `CARGO_INCREMENTAL=1` | 6,936 | 7.74 s | 224,612 | 217,676 |
| `CARGO_INCREMENTAL=0` | 6,936 | 5.51 s | 84,220 | 77,284 |

The measurement therefore selects `CARGO_INCREMENTAL=0` for ephemeral helper
invocations only; it does not change developer or release defaults.

For the explicit sccache evaluation, two different fresh worktrees used the
same temporary local cache and server socket while retaining separate private
targets. Worktree A recorded 58 Rust misses, 0 hits, and 58 writes. Worktree B
recorded 58 additional Rust misses, 0 additional hits, and 58 additional
writes. The different absolute worktree paths produced no cross-worktree hit
in this repository setup, so sccache remains opt-in rather than a default
policy. Both builds completed; the cache is not required for correctness.

The GC fixture records the safety boundary without mutating a live repository:
dry-run classified an open PR as `KEEP`, a clean merged PR as `DELETE`, tracked
and untracked changes as `DIRTY`, and ambiguous, unavailable, detached, or
identity-mismatched state as `UNKNOWN`; primary and selected current were
`PROTECTED`. It also proved that a stale developer registration outside the
Codex root blocks apply before removal/prune. Dry-run performed no remove/prune.
After that stale registration was cleared in the temporary fixture, apply
removed only the proven merged fixture, ran `git worktree prune`, and a repeated
apply found no delete candidate while remaining idempotent. The fixture exits 2
while any unknown state remains.

## Local checks and recovery

The repository-owned fixtures exercise the safety boundary without requiring a
live GitHub mutation:

~~~sh
bash scripts/codex-worktree-gc-check.sh
bash scripts/codex-cargo-check.sh
~~~

If an audit reports `DIRTY`, preserve or review the local changes before
rerunning the audit. If it reports `KEEP`, leave the worktree until the PR
reaches an explicitly terminal state. If it reports `UNKNOWN`, do not override
the result with age or a force flag; resolve the local/GitHub identity or
service ambiguity, rerun the dry run, and apply only after a `DELETE` proof is
visible. A removed clean worktree can be re-created from its branch or commit
through ordinary Git worktree commands; the command does not delete branches
or rewrite history.
