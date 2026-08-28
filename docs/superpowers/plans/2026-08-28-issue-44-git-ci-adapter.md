# Issue 44 Git and CI Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the optional, provider-neutral Git-facing adapter for canonical `.roproj/v1` projects so ordinary raw diffs, deterministic semantic review, CI validation, and tracked-package consistency compose without making Git part of semantic correctness.

**Architecture:** The existing storage and workspace engines remain authoritative. The CLI composition root will admit an exact canonical `.roproj/v1` directory for existing read-only semantic commands while preserving direct and packaged `.ro` file handling. Repository attributes will force canonical project members to text with LF endings without installing a driver or changing Git configuration. An executable ordinary-Git smoke journey will prove localized raw diffs and compose the same standalone validation, structured semantic analysis, and package/root comparison commands used outside Git.

**Tech Stack:** Rust 2024 (MSRV 1.85.0), the existing `tachiko-storage` and `tachiko-workspace-engine` APIs, Clap, ordinary Git CLI plumbing in a test-only smoke journey, Bash, Cargo, and the existing native/WASM/release gates.

**Authority:** Issue #44; ADR-0003, ADR-0011, ADR-0015, ADR-0023, and ADR-0025. [`git-native-not-git-shaped.md`](../../discussions/2026-08-26-git-native-not-git-shaped.md) is rationale only and does not amend Accepted authority.

## Global constraints

- Git, a repository, Git configuration, and a Git host remain optional. Every semantic command composed by the adapter must retain equivalent standalone behavior.
- Do not add Git commit, branch, repository, path, shard, or line identity to semantic types, representation DTOs, revision authority, or validation outcomes.
- Admit working and historical snapshots only through exact canonical `.roproj/v1` decoding plus the existing workspace validation gate.
- Keep raw Git diff and Tachiko semantic diff complementary. Raw text is useful review evidence, not semantic authority.
- Keep tracked `.roproj` authoritative when a generated portable `.ro` disagrees. Comparison is read-only and never regenerates, overwrites, merges, or selects by time/path.
- Do not install a diff/merge driver, mutate Git configuration, add GitHub APIs, or define semantic merge/history/provenance contracts.
- Do not change `.roproj/v1`, portable-package/v1, direct-JSON bytes, format dispatch, or the current writer.
- Preserve canonical LF bytes across Git checkouts without assigning a blanket attribute to the mixed direct/package `.ro` extension.

---

### Task 1: Exact `.roproj` inputs for standalone semantic review

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/tests/cli.rs`

**Interfaces:**
- Existing read-only `validate`, `calculate`, `show`, `explain`, `analyze`, `diff`, and `export` commands accept either a supported `.ro` file or an exact canonical `.roproj/v1` directory.
- No new public semantic/storage DTO or output protocol.

- [ ] Add failing CLI tests proving direct/package `.ro` behavior remains unchanged while exact `.roproj` sources produce the same validation, human semantic diff, and fixed-label structured analysis.
- [ ] Add one CLI-host source loader that dispatches only on filesystem node kind, calls the existing `load` or `load_roproj` boundary, and performs no repository discovery.
- [ ] Route only read-only semantic consumers through that helper; keep mutation/materialization inputs and output/no-overwrite behavior unchanged.
- [ ] Prove a noncanonical `.roproj` directory fails as a representation error without direct-file fallback or mutation.

### Task 2: Portable raw-diff attributes without Git configuration

**Files:**
- Create: `.gitattributes`
- Add coverage in: `scripts/git-ci-smoke.sh`

- [ ] Pin only canonical `.roproj/v1` JSON/JSONL members as text with LF endings and ordinary text diff behavior.
- [ ] Do not apply a blanket text or binary policy to `*.ro`, because direct JSON and packaged artifacts share that provisional extension.
- [ ] Prove `git check-attr` resolves the expected attributes and a representative scalar edit produces exactly one added and one removed JSONL record in one canonical entity shard.
- [ ] Prove raw diff evidence exposes the changed record while the independently authoritative semantic diff exposes the field change and derived formula impact.

### Task 3: Provider-neutral Git and CI acceptance journey

**Files:**
- Create: `scripts/git-ci-smoke.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/release-check.sh`

- [ ] Add a failing executable journey that first produces fixed-label structured validation/review evidence outside Git, then repeats it inside an ordinary temporary Git repository and requires byte-identical semantic output.
- [ ] Create the Git branch change only through supported semantic edit and canonical materialization commands; never hand-edit the successful project state.
- [ ] Exercise exact canonical/workspace validation on the tracked tree and fail closed on a noncanonical and a semantically invalid candidate.
- [ ] Track a verified package beside its source, prove consistency before the project change, then prove the stale package/source root mismatch fails explicitly without mutating either side.
- [ ] Use ordinary local Git only; do not require credentials, a remote, GitHub APIs, or persistent user/repository configuration for semantic checks.
- [ ] Add the journey to both CI and the release-equivalent local gate.

### Task 4: User workflow and implementation-status documentation

**Files:**
- Modify: `README.md`
- Modify: `examples/game-balance/README.md`
- Modify: `docs/architecture/git-native-workflow.md`
- Modify: `docs/product/game-dev-wedge.md`
- Modify: `docs/governance/canonical-reconciliation-register.md`
- Modify: `CHANGELOG.md`

- [ ] Document the copy-paste Git/CI command sequence: canonical validation, ordinary raw diff, semantic diff/structured analysis, and optional package/source comparison.
- [ ] Document the exact `.gitattributes` snippet and why generated packaged `.ro` paths need an explicit path-specific binary rule when tracked.
- [ ] State that Git refs may be caller-owned analysis labels/evidence only, not semantic revision or identity.
- [ ] Reconcile only implementation-status prose; do not amend Accepted ADR/spec contracts or mark Issue #119 complete.

### Task 5: Repository gates, review, and delivery

- [ ] Run focused CLI tests and the Git/CI smoke while iterating, including direct/package regressions and negative canonicality/validity/drift cases.
- [ ] Run formatting, warnings-as-errors Clippy, all workspace/all-target tests, exact Rust 1.85, native/WASM conformance, documentation checks, packaging, and `scripts/release-check.sh` from a clean commit.
- [ ] Review the complete diff against Issue #44 and Accepted authority, then request independent review and address every actionable finding with focused regression coverage.
- [ ] Open one Issue #44 PR, monitor all required checks and review threads, merge with head-match protection, and verify live `main`, Issue closure, and Project status.
- [ ] Recalibrate live `main` and the Product Roadmap before selecting the next genuinely Ready critical-path Issue.
