# Strict Rust Clean Code standard for #958

Audit snapshot at 2026-11-26 13:19 JST. This is step 1 of [#958](https://github.com/nurockplayer/tachiko-work/issues/958). The [Clean Architecture and Clean Code playbook](clean-architecture-and-code.md), especially §§6, 7 and 9, governs adoption. Each later PR preserves accepted behavior, error precedence, durable data, serialization, Rust and browser interfaces, and the [ADR-0016 crate DAG](../decisions/ADR-0016-milestone-02-rust-crate-layering.md). One refactor PR proceeds at a time, and 1.0 release work has priority.

## 1. Standard

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

## 2. Measured baseline

The audit branch started at `8e77b34eb4d5abb35a73d5fed5e482ebe02f7cde`, the revision cited by #958. Live `main` at the snapshot was `04b57d012e7fe50da78ff94738d5c0ac6fb6c21b`, after #956 merged. The scan covered **299 Rust files** at each SHA, including **70 production files** and **52 dedicated unit-test files** under `src/`. The remaining 177 were under `tests/`. The directive's headline lengths use a different counting convention. This baseline subtracts identified test-only spans, including from files that interleave tests and production definitions.

| Rule | Violating item | #958 SHA | Live main |
| --- | --- | ---: | ---: |
| File size | File | 29 | 29 |
| Function length | Function body | 210 | 211 |
| Control-flow complexity | Function body | 257 | 257 |
| Decision nesting | Function body | 91 | 91 |
| Parameter count | Signature | 116 | 119 |
| Lint suppression reasons | Suppression attribute | 173 | 172 |
| Inline tests | Outermost inline suite or standalone test | 52 | 52 |
| Unrestricted visibility | File | 13 | 13 |

### Ten worst violations per rule on live main

Files use line 1. Function and suite rows use the declaration line. Reason rows all have score 1 and use the attribute line. Ties sort by path, line, then label.

```text
file: 29
13723 packages/browser-client/runtime/src/lib.rs:1
 6780 crates/workspace-engine/src/patch_lifecycle.rs:1
 6697 packages/browser-client/runtime/src/interop_adapter.rs:1
 5678 packages/browser-client/runtime/src/interop_document.rs:1
 4009 crates/workspace-engine/src/lib.rs:1
 3298 crates/formula-engine/src/lib.rs:1
 2914 crates/storage/src/roproj/v3.rs:1
 1925 crates/semantic-core/src/validation.rs:1
 1811 crates/workspace-engine/src/resident_session.rs:1
 1722 crates/storage/src/roproj/v2.rs:1
body: 211
  568 packages/browser-client/runtime/src/interop_adapter.rs:3710 import_xlsx_profile
  565 packages/browser-client/runtime/src/lib.rs:4058 dispatch
  505 crates/workspace-engine/src/patch_lifecycle.rs:4362 plan_candidate
  336 packages/browser-client/runtime/src/interop_adapter.rs:504 native_tables_compatibility
  324 packages/browser-client/runtime/src/lib.rs:7970 publish_commands_with_imported
  307 packages/browser-client/runtime/src/interop_adapter.rs:6374 export_xlsx_for_profile
  293 packages/browser-client/runtime/src/save_size.rs:645 expression_bytes
  276 packages/browser-client/runtime/src/interop_document.rs:1420 promote_import_presentation
  249 crates/storage/src/roproj/v3.rs:2271 write_expression
  242 packages/browser-client/runtime/src/interop_adapter.rs:2236 parse_xml_profile
branches: 257
  230 packages/browser-client/runtime/src/interop_adapter.rs:3710 import_xlsx_profile
  143 packages/browser-client/runtime/src/interop_document.rs:2903 translate_a1_inner
  140 packages/browser-client/runtime/src/interop_adapter.rs:504 native_tables_compatibility
  129 packages/browser-client/runtime/src/interop_document.rs:1420 promote_import_presentation
  123 packages/browser-client/runtime/src/interop_adapter.rs:2236 parse_xml_profile
  107 packages/browser-client/runtime/src/interop_adapter.rs:5156 formula_admission
   93 crates/workspace-engine/src/patch_lifecycle.rs:4362 plan_candidate
   91 packages/browser-client/runtime/src/interop_adapter.rs:6200 validate_output_for_profile
   90 packages/browser-client/runtime/src/interop_adapter.rs:5929 unsupported_formula_function
   87 packages/browser-client/runtime/src/interop_adapter.rs:6374 export_xlsx_for_profile
depth: 91
    8 packages/browser-client/runtime/src/interop_adapter.rs:871 compatible_formula
    8 packages/browser-client/runtime/src/interop_adapter.rs:3710 import_xlsx_profile
    8 packages/browser-client/runtime/src/interop_adapter.rs:5156 formula_admission
    8 packages/browser-client/runtime/src/interop_document.rs:2903 translate_a1_inner
    7 crates/formula-engine/src/lib.rs:2995 evaluate_remaining_formulas
    7 crates/formula-engine/src/parser.rs:563 parse_comparison
    7 packages/browser-client/runtime/src/interop_adapter.rs:1881 import_csv_profile
    7 packages/browser-client/runtime/src/interop_adapter.rs:5929 unsupported_formula_function
    7 packages/browser-client/runtime/src/interop_number_format.rs:236 inspect
    6 crates/semantic-core/src/criterion.rs:141 parse
args: 119
   12 crates/workspace-engine/src/analysis_operations.rs:414 query_analysis_pair
   10 crates/workspace-engine/src/patch_lifecycle.rs:2593 preview_profile
   10 packages/browser-client/runtime/src/interop_document.rs:1861 build_import_sheet
    9 crates/semantic-core/src/validation.rs:1735 validate_value
    9 crates/workspace-engine/src/patch_lifecycle.rs:2198 propose_inner
    8 crates/semantic-core/src/validation.rs:779 validate_value
    8 packages/browser-client/runtime/src/interop_document.rs:2323 bind_import_formula
    8 packages/browser-client/runtime/src/lib.rs:6470 record_column_presentation
    7 crates/cli/src/commands.rs:370 analyze_query
    7 crates/merge-engine/src/lib.rs:773 merge_field_definition
reason: 172
    1 crates/cli/src/main.rs:287 allow(clippy::too_many_lines)
    1 crates/diff-engine/src/canonical_delta.rs:674 allow(clippy::too_many_lines)
    1 crates/formula-engine/src/evaluation.rs:551 allow(clippy::too_many_lines)
    1 crates/formula-engine/src/evaluation.rs:1206 allow(clippy::cast_possible_truncation)
    1 crates/formula-engine/src/evaluation.rs:1380 allow(clippy::float_cmp)
    1 crates/formula-engine/src/lib.rs:437 allow(clippy::too_many_lines)
    1 crates/formula-engine/src/lib.rs:609 allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)
    1 crates/formula-engine/src/lib.rs:645 allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)
    1 crates/formula-engine/src/lib.rs:859 allow(clippy::too_many_lines)
    1 crates/formula-engine/src/lib.rs:2751 allow(clippy::too_many_lines)
inline-test: 52
 2870 packages/browser-client/runtime/src/lib.rs:13929 tests
 1415 crates/workspace-engine/src/patch_lifecycle.rs:6993 tests
  988 packages/browser-client/runtime/src/interop_adapter.rs:7116 native_tables_output_tests
  450 crates/workspace-engine/src/keyed_grouped_sum_operations.rs:370 tests
  326 crates/workspace-engine/src/resident_session.rs:1914 tests
  315 packages/browser-client/runtime/src/interop_document.rs:5820 import_dependency_tests
  309 packages/browser-client/runtime/src/interop_adapter.rs:6806 capacity_proof_tests
  278 packages/browser-client/runtime/src/interop_number_format.rs:432 tests
  263 packages/browser-client/runtime/src/interop_adapter.rs:8431 ledger_cap_tests
  251 packages/browser-client/runtime/src/wasm.rs:1409 row_rank_parity_tests
pub: 13
  335 packages/browser-client/runtime/src/lib.rs:1
  186 crates/workspace-engine/src/patch_lifecycle.rs:1
  177 crates/workspace-engine/src/lib.rs:1
  112 crates/semantic-core/src/model.rs:1
   84 packages/browser-client/runtime/src/interop_adapter.rs:1
   79 crates/workspace-engine/src/resident_session.rs:1
   75 crates/formula-engine/src/lib.rs:1
   74 packages/browser-client/runtime/src/interop_document.rs:1
   69 crates/workspace-engine/src/formula_operations.rs:1
   62 crates/workspace-engine/src/analysis_operations.rs:1
```

### Commands

Run from the repository root. Save the Python block in the appendix as `measure.py` in a temporary directory and set `audit_dir` to that directory. The audit used the following commands. The fetch supplied the live commit object for the scan.

```sh
rtk git rev-parse HEAD
rtk gh api repos/nurockplayer/tachiko-work/branches/main --jq .commit.sha
rtk git fetch --no-tags --no-write-fetch-head origin 04b57d012e7fe50da78ff94738d5c0ac6fb6c21b
nice -n 15 uv run --no-project --python 3.12 \
  --with 'tree-sitter==0.25.2' --with 'tree-sitter-rust==0.24.2' \
  python "$audit_dir/measure.py" \
  8e77b34eb4d5abb35a73d5fed5e482ebe02f7cde \
  04b57d012e7fe50da78ff94738d5c0ac6fb6c21b > "$audit_dir/measure.txt"
```

The scan used no Cargo builds, tests or Clippy run. If the guard preflight compares compiler diagnostics with the source rules, use the proposed `clippy.toml` thresholds in the preflight checkout and run the existing lint machinery with explicit diagnostic levels:

```sh
tachi-run -- cargo clippy --workspace --all-targets --locked --message-format=json -- \
  --force-warn clippy::too_many_lines --force-warn clippy::cognitive_complexity \
  --force-warn clippy::excessive_nesting --force-warn clippy::too_many_arguments \
  --force-warn clippy::allow_attributes_without_reason
tachi-run -- cargo clippy --manifest-path packages/browser-client/runtime/Cargo.toml \
  --all-targets --locked --message-format=json -- \
  --force-warn clippy::too_many_lines --force-warn clippy::cognitive_complexity \
  --force-warn clippy::excessive_nesting --force-warn clippy::too_many_arguments \
  --force-warn clippy::allow_attributes_without_reason
```

## 3. Ratchet and CI

- In step 2, put the checker at `scripts/clean-code-check.py` and its sorted, generated ceilings at `scripts/clean-code-baseline.tsv`. Use the same source rules as this audit. Use the existing Python runner, with the two pinned prebuilt parsing packages isolated through `uv` and hash-locked for CI. The grammar handles Rust strings, comments, names and attributes without a regex approximation or a Cargo build. The §9 preflight reviews this audit-only dependency choice before adoption.
- Key rows by rule and repository-relative path, plus the qualified item for item-level rules. Include the enclosing module, type and trait impl where applicable, and normalized `cfg` predicates where they distinguish otherwise identical declarations. For a reason row, use the owning item and normalized suppression kind and lint set, with occurrence count as its value. For local attributes use the nearest named owner. Keep line numbers in diagnostics only. Treat ambiguous keys as a checker error rather than granting a shared ceiling to different functions.
- Measure the full candidate tree. Every current violation must have the same key in the base snapshot with an equal or greater value. Fail on a new key or an increased value, even when other entries disappear. Require the candidate TSV to exactly equal the sorted current violations. Improvements lower a ceiling or remove its row when it reaches the limit. Renaming or moving an over-limit item requires fixing the violation in the same change. Each dedicated refactor PR removes at least one row. Routine PRs may keep the baseline identical.
- Extend `quality` (**Rust quality and product workflows**) in [the existing CI workflow](../../.github/workflows/ci.yml). Add the source check near the ADR-0016 check. On pull requests use checkout depth 2 and compare the checked-out merge result with its first parent. On pushes to `main` compare with the event's `before` commit, fetching that exact object if necessary. Compare against Git's base-commit TSV so edits to the candidate TSV cannot grant themselves extra budget. Seed once from the approved step 2 base SHA after reconciling live main. Permit that specific baseline-free base only during bootstrap, measure its source directly, and require the seeded candidate to match it. Missing base evidence, duplicate keys, parse errors and empty input fail the step.
- In preflight, show that unchanged violations, shifted lines and genuine reductions pass. Show that raising a ceiling by one, adding a new violation while deleting another, keeping a stale row, and introducing an unreasoned suppression in inactive `cfg_attr` fail. Include a test-only module control and a file shared between test and production so exclusion cannot hide production code. Preserve the existing formatting, Clippy, ADR-0016, release, capacity, and SCD gates.

For example, an existing 97-line function may shrink to 80 and record 80. A later change to 81 fails. Once it reaches 60 its row disappears, and a later increase to 61 fails as a new violation.

## 4. Ranked refactor backlog

At 2026-11-26 13:19 JST, open PRs were #945, #947, #948, #957, #816, #654, #629 and #567. #956 merged at 2026-11-26 13:12 JST. The audit read the named diffs and every other open PR's file list. #957 advanced during the audit, so its file list was refreshed at head `8b5cd60d231462c5370f08a433af62aa6ab902bf`.

```sh
rtk gh pr list --state open --limit 100 --json number,title,baseRefName,headRefOid
rtk gh pr view 956 --json number,state,mergedAt,headRefOid,mergeCommit
for n in 945 947 948 956 957 816 654 629 567; do
  rtk gh pr diff "$n" --name-only
done
```

#945 touches `runtime/src/blank_workbook_tests.rs`, outside these slices. #947 touches the CI workflow, so the step 2 change to that file is **after PR merges (#947)**. #948 owns the adapter and document files and shares the runtime root with #957. #956 also touched the runtime root. The #816 scratch PR overlaps both workspace hotspots and the runtime root. Its title says **DO NOT MERGE**. Leave those slices parked until the overlap clears through closure or removal from its diff, and recheck the remaining owners. #654, #629 and #567 have no backlog overlap.

“Startable now” means the named source paths are clear of open PRs. Scope the work and map the existing tests now. Land source slices after the ratchet guard, with fresh ownership checks before each Ready handoff. Sizes are rough added plus deleted lines. Line numbers cite the live-main baseline.

| Rank and availability | Single-PR slice and estimated diff | Target baseline removals and risk |
| --- | --- | --- |
| 1 · Startable now | In `crates/semantic-core/src/criterion.rs`, use a private ordered operator-prefix table in `CriterionTest::parse`. Preserve longest-first matching, default equality and existing operand classification. **35–65 lines**, one file. | Remove branch **23** and depth **6** at `:141`. **Low.** Run the existing criterion tests for overlapping prefixes, empty operands and relational rejection. |
| 2 · Startable now | In `packages/browser-client/runtime/src/interop_number_format.rs`, separate bracket scanning and alphabetic token recognition from the `inspect` loop. Keep one cursor, existing validation order and section output. **140–230 lines**, one file. | Remove body **97** and depth **7** at `:236`. Lower branch **77** at the same key and keep new helpers within their caps. **Medium.** Existing `interop_number_format::tests` cover conditions, locale, contextual minutes, scientific notation and malformed patterns. |
| 3 · Startable now | Move `crates/storage/src/strict_json.rs`'s unit module to `crates/storage/src/strict_json/tests.rs` using `#[cfg(test)] mod tests;`. Keep its module and test names and all assertions. **230–245 mechanical lines**, two files. | Remove inline-test **118** at `:437`. **Low.** Preserve the duplicate-member, depth-budget and syntax-precedence cases. |
| 4 · Startable now | Move `crates/storage/src/portable_package/host.rs`'s private test module to `crates/storage/src/portable_package/host/tests.rs`. **145–165 mechanical lines**, two files. | Remove inline-test **76** at `:339`. **Low.** Keep the exclusive publication race and old-source dispatch tests intact. |
| 5 · Startable now | Split `crates/storage/src/presentation.rs`'s per-kind decoding into private style, table, rule and cell steps. Keep the admission loop's check order, canonical bytes and error messages. **225–295 lines**, one or two files. | Remove body **124** at `:116` and its obsolete reasonless `too_many_lines` at `:115`. Reduce branch **28** and depth **4** at `:116`. **Medium.** Use `presentation_codec` and `presentation_package` for ordering, round trips and refusal behavior. |
| 6 · After PR merges (#948) | Move the XML scope cases to `packages/browser-client/runtime/src/interop_adapter/xml_scope_tests.rs` from `interop_adapter.rs`. **75–95 mechanical lines**, two files. | Remove inline-test **42** at `:6715`. **Low.** Keep the namespace restore, profile cap and alias-order assertions before later parser refactors. |
| 7 · After PR merges (#948) | Move the A1 input-boundary test to `packages/browser-client/runtime/src/interop_document/a1_import_boundary_tests.rs` from `interop_document.rs`. **29–43 mechanical lines**, two files. | Remove inline-test **17** at `:5802`. **Low.** Keep exact original-byte-limit evidence before a later translation split. |
| 8 · After PR merges (#948, #957, #816 overlap; #956 already merged) | In `packages/browser-client/runtime/src/lib.rs`, group the before/after style and width as a private column-presentation value for `record_column_presentation`. Preserve revision checks, publication timing and the existing history payload. Keep the root's physical size flat or lower. **70–120 lines**, one or two files. | Remove arity **8** at `:6470` and its reasonless `too_many_arguments` at `:6469`. **Medium.** Reuse presentation history undo/redo and revision coverage. |
| 9 · After PR merges (#816 overlap) | In `crates/workspace-engine/src/patch_lifecycle.rs`, isolate append-collection collision detection from `duplicate_constraint_write` in a private child module. Keep first-conflict selection, ordering and target bookkeeping. **65–115 lines**, two files. | Remove depth **4** and lower branch **26** at `:6426`. Lower the file ceiling from **6,780**. **High.** Preserve constraint collision and patch lifecycle outcomes. |
| 10 · After PR merges (#816 overlap) | In `crates/workspace-engine/src/lib.rs`, move the per-field projection in `preflight_changed_formula_projections` to a private child module. Keep the ordered candidate walk, index fallback and test counters. **45–85 lines**, two files. | Remove depth **4** and lower branch **14** at `:3516`. Lower the file ceiling from **4,009**. **Medium.** Check existing incremental-vs-full projection evidence and first diagnostic order. |

## 5. Exact AGENTS.md addition

```markdown
## Rust Clean Code and continuous refactoring

Follow the [Rust Clean Code standard](docs/engineering/clean-code-standard-958-proposal.md#1-standard)
on every Tachiko Work Rust change. Keep new source within the limits, give each
lint suppression an inline reason, put new unit tests in dedicated test modules,
and use the narrowest visibility that serves the caller. Run the Clean Code check
for the candidate alongside the applicable existing validation. Keep the tracked
baseline equal to current violations. Every existing key must stay at or below
its recorded ceiling, and every resolved key must be removed.

Refactor one cohesive seam per Ready PR, with one refactor PR active at a time
under #958. A refactor PR removes at least one baseline entry. Recheck open PR
file overlap before starting and yield to 1.0 work. Reuse existing contract
coverage and keep the ADR-0016 dependency graph, behavior, error precedence,
durable data, serialization and public interfaces intact. Apply the Rust
ownership and unsafe rules above. Ask the lead for a DECISION if a split requires
changing a public interface, behavior, data or issue scope.
```

## Appendix. Reproducible read-only measurement

```python
import collections
import posixpath
import re
import subprocess
import sys
from pathlib import PurePosixPath

from tree_sitter import Language, Parser
import tree_sitter_rust

CAPS = {"file": 800, "body": 60, "branches": 12, "depth": 3,
        "args": 5, "reason": 0, "inline-test": 0, "pub": 40}
ATTR = {"attribute_item", "inner_attribute_item"}
COMMENT = {"line_comment", "block_comment"}
CONTROL = {"if_expression", "match_expression", "for_expression",
           "while_expression", "loop_expression"}
FN = {"function_item", "function_signature_item"}
SRC = re.compile(r"(?:crates/[^/]+|packages/browser-client/runtime)/(src|tests)/.*\.rs$")


def git(*args):
    return subprocess.check_output(("git", *args))


def text(node):
    return node.text.decode() if node else ""


def field(node, key):
    return node.child_by_field_name(key)


def row(node):
    return node.start_point.row + 1


def rows(node):
    return range(row(node), node.end_point.row + bool(node.end_point.column) + 1)


def groups(node):
    pending = []
    for child in node.named_children:
        if child.type == "attribute_item" or (
            child.type in COMMENT and text(child).startswith(("///", "/**"))
        ):
            pending.append(child)
        elif child.type not in COMMENT:
            yield pending, child
            pending = []


def parts(tree):
    group, result = [], []
    for token in tree.children[1:-1] if tree else []:
        if token.type == ",":
            if group:
                result.append(group)
            group = []
        elif token.type not in COMMENT:
            group.append(token)
    return result + ([group] if group else [])


def meta(tokens):
    return text(tokens[0]), tokens[1] if len(tokens) > 1 else None


def attrs(nodes):
    return [a.named_child(0) for a in nodes if a.type in ATTR]


def possible(tokens):
    name, args = meta(tokens)
    if tokens[-1].type == "token_tree" and name in {"all", "any", "not"}:
        values = [possible(part) for part in parts(args)]
        if name == "all":
            return all(v[0] for v in values), any(v[1] for v in values)
        if name == "any":
            return any(v[0] for v in values), all(v[1] for v in values)
        return values[0][1], values[0][0]
    return (False, True) if len(tokens) == 1 and name == "test" else (True, True)


def test_fn(nodes):
    return any(text(a.named_child(0)) == "test" for a in attrs(nodes))


def test_only(nodes):
    for a in attrs(nodes):
        name, args = meta(a.named_children)
        if name == "test" or (
            name == "cfg" and not possible(parts(args)[0])[0]
        ):
            return True
    return False


def production_view(node, excluded, omitted):
    for tags, child in groups(node):
        if test_only(tags):
            excluded.update([child, *tags])
            for skip in [child, *tags]:
                omitted.update(rows(skip))
            if test_fn(tags) or (child.type == "mod_item" and field(child, "body")):
                yield (max(rows(child)) - row(tags[0]) + 1, child)
        else:
            yield from production_view(child, excluded, omitted)


def descendants(node, excluded=frozenset()):
    if node in excluded:
        return
    yield node
    for child in node.named_children:
        yield from descendants(child, excluded)


def entries(node, scope=(), testing=False):
    testing |= test_only([n for n in node.named_children if n.type == "inner_attribute_item"])
    for tags, child in groups(node):
        child_test = testing or test_only(tags)
        yield tags, child, scope, child_test
        if child.type == "mod_item" and field(child, "body"):
            yield from entries(field(child, "body"), scope + (text(field(child, "name")),), child_test)


def links(path, root, inventory):
    p = PurePosixPath(path)
    for tags, node, scope, testing in entries(root):
        if node.type != "mod_item" or field(node, "body"):
            continue
        name = text(field(node, "name"))
        override = next((field(a, "value") for a in attrs(tags)
                         if text(a.named_child(0)) == "path"), None)
        folder = p.parent if p.name in {"lib.rs", "main.rs", "mod.rs"} else p.with_suffix("")
        folder = p.parent if p.parent.name == "bin" else folder
        normal = folder.joinpath(*scope)
        candidates = [normal / f"{name}.rs", normal / name / "mod.rs"]
        if override:
            base = normal if scope else p.parent
            candidates = [base / text(override.named_child(0))]
        found = [posixpath.normpath(c) for c in candidates if posixpath.normpath(c) in inventory]
        if len(found) != 1:
            raise ValueError((path, row(node), candidates, found))
        yield found[0], testing


def suppression(name, args):
    arguments = parts(args)
    if name == "cfg_attr":
        for branch in arguments[1:]:
            yield from suppression(*meta(branch))
    if name in {"allow", "expect"} and not any(
        len(part) == 3 and text(part[0]) == "reason" and text(part[1]) == "="
        and part[2].type in {"string_literal", "raw_string_literal"}
        and any(text(x).strip() for x in part[2].named_children if x.type == "string_content")
        for part in arguments
    ):
        yield name + "(" + ", ".join("".join(map(text, p)) for p in arguments) + ")"


def flow(node, excluded, depth=0):
    if node in excluded or node.type in FN or node.type == "token_tree":
        return 0, depth
    control = node.type in CONTROL or (
        node.type == "let_declaration" and field(node, "alternative") is not None
    )
    nested = depth + int(control)
    choices = nested if control else 0
    if node.type == "match_expression":
        choices += max(0, len([a for a in field(node, "body").named_children
                               if a.type == "match_arm"]) - 1)
    if node.type == "match_pattern" and field(node, "condition"):
        choices += 1
    if node.type in {"binary_expression", "let_chain"}:
        choices += sum(c.type in {"&&", "||"} for c in node.children)
    peak = nested
    for child in node.named_children:
        child_score, child_peak = flow(child, excluded, nested)
        choices += child_score
        peak = max(peak, child_peak)
    return choices, peak


def code_rows(node, lines, excluded):
    if node in excluded or node.type in COMMENT:
        return set()
    if not node.child_count:
        return {i for i in rows(node) if lines[i - 1].strip()}
    return set().union(*(code_rows(c, lines, excluded) for c in node.children))


def measure(rev):
    paths = sorted(p for p in git("ls-tree", "-r", "--name-only", rev).decode().splitlines()
                   if SRC.fullmatch(p))
    parser = Parser(Language(tree_sitter_rust.language()))
    data = {}
    query = "".join(f"{rev}:{p}\n" for p in paths).encode()
    raw = subprocess.run(("git", "cat-file", "--batch"), input=query,
                         stdout=subprocess.PIPE, check=True).stdout
    at = 0
    for p in paths:
        end = raw.index(b"\n", at)
        _, kind, size = raw[at:end].split()
        assert kind == b"blob"
        at = end + 1
        source = raw[at:at + int(size)]
        at += int(size) + 1
        lines = source.split(b"\n")
        if lines[-1] == b"":
            lines.pop()
        root = parser.parse(source).root_node
        if root.has_error:
            bad = [row(n) for n in descendants(root) if n.is_error or n.is_missing]
            raise ValueError((p, bad))
        data[p] = (root, lines)

    src = {p for p in paths if SRC.fullmatch(p)[1] == "src"}
    edges = {p: list(links(p, data[p][0], src)) for p in src}
    incoming = {target for targets in edges.values() for target, _ in targets}
    gated_files = {p for p in src if test_only(
        [n for n in data[p][0].named_children if n.type == "inner_attribute_item"])}
    production = src - incoming - gated_files
    queue = list(production)
    while queue:
        for target, testing in edges[queue.pop()]:
            if not testing and target not in production and target not in gated_files:
                production.add(target)
                queue.append(target)

    found = collections.defaultdict(list)
    for p, (root, lines) in data.items():
        def put(rule, value, line, label=""):
            if value > CAPS[rule]:
                found[rule].append((value, p, line, label))

        for node in descendants(root):
            if node.type in ATTR:
                a = node.named_child(0)
                for missing in suppression(*meta(a.named_children)):
                    put("reason", 1, row(node), missing)
        if p not in production:
            continue

        omitted, excluded = set(), set()
        for span, node in production_view(root, excluded, omitted):
            put("inline-test", span, row(node), text(field(node, "name")))
        put("file", len(lines) - len(omitted - code_rows(root, lines, excluded)), 1)
        nodes = list(descendants(root, excluded))
        put("pub", sum(n.type == "visibility_modifier" and text(n) == "pub" for n in nodes), 1)
        for node in nodes:
            if node.type in FN:
                name = text(field(node, "name"))
                parameters = field(node, "parameters")
                put("args", sum(n not in excluded and n.type in {"parameter", "self_parameter", "variadic_parameter"}
                                for n in parameters.named_children), row(node), name)
                if (body := field(node, "body")) is not None:
                    put("body", len(code_rows(body, lines, excluded)), row(node), name)
                    score, depth = flow(body, excluded)
                    put("branches", score, row(node), name)
                    put("depth", depth, row(node), name)

    print(rev, len(paths), len(production), len(src - production))
    for rule in CAPS:
        items = sorted(found[rule], key=lambda v: (-v[0], v[1], v[2], v[3]))
        print(f"{rule}: {len(items)}")
        for value, path, line, name in items[:10]:
            print(f"{value:5} {path}:{line} {name}")
    return found


if __name__ == "__main__":
    for revision in sys.argv[1:]:
        measure(revision)
```
