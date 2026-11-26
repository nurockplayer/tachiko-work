# Rust Clean Code standard

The [Clean Architecture and Clean Code playbook](clean-architecture-and-code.md) governs this standard. Preserve accepted behavior, error precedence, durable data, serialization, public interfaces and the [ADR-0016 crate DAG](../decisions/ADR-0016-milestone-02-rust-crate-layering.md). Run one refactor PR at a time and give 1.0 release work priority. The ranked backlog lives in [#958](https://github.com/nurockplayer/tachiko-work/issues/958).

## Rules

Apply the production rules to tracked Rust under `crates/*/src/` and `packages/browser-client/runtime/src/`. Include all written production target and feature variants. Exclude dedicated test files and items gated exclusively to `test` from production measurements. Shared files such as the runtime's `wasm.rs` count once. Apply the reason rule to all of those source files and their crates' `tests/` trees, including dedicated tests. The limits are inclusive.

| Rule | Limit and reason | Clippy equivalent or script requirement |
| --- | --- | --- |
| Production file size | **800 physical lines per file**, including docs and blank lines outside test-only spans. This leaves space for a cohesive subsystem and makes growth in multi-responsibility files visible. | Script. |
| Function body length | **60 code-bearing body lines**, counting braces and literal lines, excluding blank/comment-only lines and test-only syntax. A use case should fit within a reviewable span. | Script for the exact count. Clippy has `too_many_lines` with `too-many-lines-threshold = 60`. |
| Control-flow complexity | **12 points per function**. Count each `if`, `match`, loop or `let … else` as one plus its enclosing decision depth. Add one per match arm after the first, match guard and `&&` or `\|\|` operator. This makes decision load and nesting measurable. | Script for this score. Clippy's `cognitive_complexity` and `cognitive-complexity-threshold` measure a different approximation. |
| Decision nesting | **3 levels per function**, starting at zero. Nested decisions include `else if` in Rust syntax. Match arms, closures and ordinary blocks add no level of their own. This caps the number of conditions a reader has to hold in mind. | Script for decision depth. Clippy has `excessive_nesting` with `excessive-nesting-threshold = 3` for block depth. |
| Parameter count | **5 per function or method**, including `self`, trait declarations and explicit variadics. Count each destructuring pattern once and exclude test-only parameters. Related parameters should form a meaningful type where the call sites warrant it. Preserve existing public and ABI signatures. | Script for all written signatures. Clippy has `too_many_arguments` with `too-many-arguments-threshold = 5`, with different compiler-side coverage. |
| Lint suppression reasons | **0 reasonless `allow` or `expect` attributes**, including inner attributes and branches of `cfg_attr`. Require a `reason = "…"` literal with nonblank explanatory text. Review whether the reason describes the specific invariant or trade-off. | Clippy has `allow_attributes_without_reason` for the compiled configuration. The script checks written source, including inactive branches. |
| Inline tests | **0 inline test modules or standalone `#[test]` functions in production files**. Use a separate file wired by a test-only `mod name;` so tests can retain private access. Existing test-only counters, assertions and instrumentation tied to production execution sites can stay in place. Record each misplaced suite or test with its physical span so it cannot grow. | Script. |
| Unrestricted visibility | **40 bare `pub` modifiers per production file**, including fields, methods and re-export declarations. Use the narrowest visibility that works for a new item. This checks the local visibility footprint. Compiler visibility and consumer checks govern the actual interface. | Script. |

For file metrics, a line shared by test-only and production syntax stays in the production count. Each function body has its own decision depth. A closure contributes to the containing function, while a nested named function has its own decision score. The scanner sees Rust syntax as written, including conditional items and explicit re-exports. Macro expansion, resolved reachability, aliasing and effective export names require compiler or consumer evidence. Macro tokens contribute to line measurements. Branch and visibility counts use syntax outside unexpanded macro token trees. The script determines test-only `cfg` by setting `test` false and treating other predicates as unknown. It excludes an item only when the test gate is proven.

Keep the existing Clippy gates. The workspace uses `[workspace.lints.clippy]` and the standalone runtime uses `[lints.clippy]`, both with `all = "deny"` and `pedantic = "warn"` in their manifests. The existing gate passes `-D warnings`. The threshold keys in the table belong in `clippy.toml` if a later lint is adopted. Lint levels belong under the corresponding manifest table. Use the script as the ratchet's single source of measurements. Tightening the Clippy settings alone would fail on existing debt, and suppressions and the selected compilation target can change the diagnostics. See the [Clippy configuration reference](https://doc.rust-lang.org/clippy/lint_configuration.html) and [lint reference](https://clippy.rust-lang.org/master/index.html).

## Ratchet

The [tracked TSV](../../scripts/clean-code-baseline.tsv) records the current
violations as ceilings. File rows are keyed by rule and repository-relative
path. Function and test placement rows also include their qualified item,
including the enclosing module, type or trait implementation and relevant
written `cfg` predicates. Suppressions with the same owner, condition, kind and
lint set share a row whose value is their occurrence count. Line numbers are
diagnostic locations and may move freely.

The checker measures the candidate's tracked source and compares it with the
TSV from the base commit. Each candidate key must exist in the base and its
value may only stay the same or decrease. A new violation fails even when
another violation was removed. An over-limit item must reach its cap before
its violation can move to a different path or qualified name. The candidate
TSV must match the measured violations exactly. Dedicated refactor PRs remove
at least one row.

An existing 97-line function may shrink to 80 and record 80. A later change to
81 fails. Reaching 60 deletes the row. An increase to 61 would then be a new
violation.

The existing CI `quality` job runs the check and its self-tests in one step.
Pull requests compare the checked-out merge result with its first parent.
Pushes to `main` use the event's previous commit. For the initial adoption,
the checker measures the explicitly pinned bootstrap base because it predates
the TSV.

## Run locally

Run from the repository root with uv-managed CPython 3.14 and the checked-in
script lock. Stage new or renamed Rust files before measuring them. The checker
reads the working contents of tracked files.

```sh
git fetch origin main
nice -n 15 uv run --locked --no-build --managed-python --script scripts/clean-code-check.py --self-test --base origin/main
```

After reducing or removing violations, use the same base to regenerate the TSV.
The write command first checks that every candidate violation fits its base ceiling.
Re-run the check and include the generated TSV change in the PR.

```sh
nice -n 15 uv run --locked --no-build --managed-python --script scripts/clean-code-check.py --base origin/main --write-baseline
```
