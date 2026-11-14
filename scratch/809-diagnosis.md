# #809 step 2: edit / clear / zero-row-window diagnosis

Scratch branch `scratch/809-measure`, PR #816. Never merge.

## Measurement

The native rerun passed at measured commit
`cf71ae685c280bf2edde63a1f0ce0ae1394e132e`:
[hosted run](https://github.com/nurockplayer/tachiko-work/actions/runs/43577430723),
GitHub Ubuntu 24.04. Whole job: **2026-11-14 17:46:50–17:59:57 JST**;
native bench: 29.75 s; remote-test: success after 772 s.
All 3 opens, 30 edits, 3 clears of each width and 3 save/reopen pairs ran:
`809_COUNTS rows=10000 opens=3 edits=30 saves=3 reduced=false`.

No product optimization or design selection is included. The scalar edit uses
column_1; clear one removes column_0 (Text), and clear seven removes stored
columns 0–6. The latter leaves all three formula columns in place, including
their missing-input outcomes. These are different affected-calculation workloads.

[Complete edit/clear/window phase tables](809-diagnosis-phases.md) retain every
inclusive path, residual, p50/p95/max, share and Document clone count. The compact
tables below use the same values. With n=3, p95 is the maximum; it is not a
statistically established tail bound. All durations are milliseconds.

| operation | n | p50 ms | p95 ms | max ms | Document clones |
|---|---:|---:|---:|---:|---:|
| open combined | 3 | 996.600 | 1035.249 | 1035.249 | 1 |
| edit | 30 | 187.000 | 212.285 | 252.190 | 4 |
| edit zero-row window | 30 | 8.061 | 8.898 | 9.195 | 0 |
| save export_project_v3 | 3 | 1135.426 | 1143.973 | 1143.973 | 2 |
| reopen | 3 | 1025.222 | 1060.286 | 1060.286 | 1 |
| clear 1 | 3 | 546.578 | 549.877 | 549.877 | 2 |
| clear 1 zero-row window | 3 | 7.938 | 8.174 | 8.174 | 0 |
| clear 7 | 3 | 2031.394 | 2069.104 | 2069.104 | 2 |
| clear 7 zero-row window | 3 | 10.804 | 12.948 | 12.948 | 0 |

The edit and its window have separate samples: 187.000 + 8.061 = 195.061 ms is
the **sum of medians**, not the measured median of a combined request pair.
The corresponding clear sums are 554.516 and 2042.198 ms. No combined p95 is
inferred from separate percentiles. These runs do not establish a browser-worker
budget or isolate the cause of Sheet's slower VM results.

**Open-timer correction:** at the measured commit repeated native opens used a
fresh local slot; replacement/destruction of the previous resident happened
after timing. The individual opens were 1035.249, 996.600 and 935.558 ms.
Reopen also admits into a fresh slot, unlike replacing the WASM resident.
Historical tables remain unchanged and have this limitation. The follow-up
benchmark correction passes the retained slot into the measured open operation;
its result is pending. Edit/clear/window instrumentation is unchanged.

## Mutation phase summary

| phase | edit p50 / p95 ms | clear 1 p50 / p95 ms | clear 7 p50 / p95 ms |
|---|---:|---:|---:|
| publish snapshot clone | 8.689 / 10.681 | 6.433 / 7.050 | 10.237 / 10.463 |
| prepare: compare documents | 5.663 / 6.672 | 14.682 / 14.722 | 30.786 / 31.963 |
| prepare: retained state clone | 13.221 / 14.199 | 16.387 / 16.647 | 17.341 / 18.480 |
| prepare: calculation update | 6.645 / 7.055 | 37.468 / 37.530 | 298.605 / 333.652 |
| prepare: formula preflight | 0.001 / 0.002 | 43.253 / 44.217 | 41.948 / 43.167 |
| propose: plan commands | 11.934 / 14.002 | 28.706 / 30.095 | 109.408 / 124.844 |
| propose: finalize evaluation | 97.085 / 108.536 | 164.148 / 164.717 | 551.167 / 571.021 |
| propose: store memo | 4.839 / 6.176 | 6.548 / 6.866 | 46.507 / 54.061 |
| execute: context | 0.005 / 0.006 | 5.485 / 5.649 | 62.320 / 66.977 |
| execute: authorize + publish | 24.390 / 29.539 | 38.139 / 41.874 | 88.377 / 89.814 |
| execute: receipt building | 3.483 / 4.170 | 10.528 / 10.746 | 48.258 / 48.365 |
| execute: receipt store/redact | 0.569 / 0.884 | 8.310 / 8.328 | 97.982 / 98.808 |
| execute: drop evaluated document | 4.272 / 6.678 | 5.019 / 5.778 | 4.504 / 5.161 |
| execute: verify publication | 3.257 / 4.083 | 3.523 / 4.296 | 3.074 / 3.141 |
| post-publication refresh | 0.013 / 1.865 | 69.028 / 70.160 | 84.089 / 86.479 |
| zero-row totals fold | 7.998 / 8.833 | 7.871 / 8.097 | 8.979 / 10.329 |

Rows are inclusive and may overlap. In particular, retained-clone time includes
the calculation and dependency graph copies, and authorize/publish includes
resident replacement and destruction. Do not add parent and child costs.

## What the finer timers establish

For scalar edit, proposal construction is 113.302 ms (60.59% of 187.000 ms).
Of its 97.085 ms finalization, candidate equality is 3.139 ms, formula projection
preflight 39.886 ms, V1 diff construction 42.843 ms, and destruction of that
diff's snapshots 10.718 ms. Preflight spends 36.828 ms scanning/projecting all
formulas and 2.826 ms constructing the address index. The diff includes two
Document copies (9.682 and 8.375 ms entity copies), all-entity/descriptor comparison
(7.819 ms), and whole-formula impact work (15.636 ms). Four Document clones are
counted: runtime snapshot, planner candidate, and these two V1 snapshots.

Preparation already computes incrementally: dirty-root discovery is 0.027 ms
and dirty recomputation 0.047 ms for the scalar edit. Yet before/after formula
set enumeration costs 2.036 + 4.029 ms and retained-state copying costs 13.221 ms.
The latter includes a 10.132 ms graph copy; cell readers and reference uses alone
cost 4.265 and 4.296 ms. The calculation is not spending 187 ms evaluating the
two changed same-row formulas.

Execute is 36.608 ms. It spends 3.657 ms comparing the installation candidate,
13.054 ms replacing/dropping retained state, 5.733 ms replacing/dropping the old
Document, 4.272 ms dropping the separate evaluated Document, and 3.257 ms
verifying publication. Candidate-vs-current equality is 1.379 ms p50 but
3.615 ms p95. Moving cloning off the hot path without accounting for last-owner
destruction would hide much of this work rather than remove it.

Clear uses V2 evidence, so it has only the runtime and planner Document clones.
For clear one, candidate preflight is 43.253 ms during preparation; V2 evidence
also performs a 44.588 ms full preflight. The resident begins with
`preflight_succeeded=false`; fresh clear fixtures do not carry a successful
base preflight witness, so the V2 base check runs. In contrast, most scalar
samples follow successful earlier publications. Initial admission does not pass
its existing formula-projection success into the resident constructor.

Clear-one V2 evidence costs 144.184 ms: canonical scoped delta 14.511 ms,
scoped formula-document projections 26.128 ms, and scoped formula diff
33.356 ms, in addition to the preflight. All 10,000 entities are in scope;
"scoped" is not small here. Post-publication costs 69.028 ms: projecting all
formula sources 52.969 ms and rebuilding structure/capacity 14.733 ms.

Clear seven does substantially more necessary work: dirty discovery 120.042 ms,
dirty recomputation 147.094 ms, disclosures 218.837 ms, preview 156.305 ms,
receipt build 48.258 ms and receipt store/redact 97.982 ms. Those costs scale with
70,000 commands and the affected calculations/evidence; they cannot all be
classified as avoidable whole-table scans. Scoped projections still copy
10,000 entities (24.266 ms), and source refresh still projects 30,000 formulas
(63.491 ms).

Zero-row windows do not materialize rows, but totals fold members and numeric
fields: 7.998 ms after edit, 7.871 ms after clear one, and 8.979 ms after clear
seven. Default view ordering is already cheap after edit (0.024 ms), so caching
that order cannot supply the missing ~145 ms in the edit budget.

The existing bench now defaults to 10,000 rows only. Set
`CAPACITY_809_ALL_SIZES=1` to reproduce all four step-1 sizes. The standard command
needs no extra environment variable for this diagnostic rerun:

```sh
job-run --id 809-diagnose-formatted -- orb-test --image rust:1.99.0 -- bash -c 'rustup component add rustfmt && cargo fmt --all -- --check && cargo fmt --manifest-path packages/browser-client/runtime/Cargo.toml -- --check && cargo test --release --manifest-path packages/browser-client/runtime/Cargo.toml --locked --features capacity-bench capacity_809_formula_table -- --ignored --test-threads=1 --nocapture'
```

Repetitions, fixture, assertions and native typed-call boundaries are unchanged.
There is no WASM run in step 2. Step-1 results remain in `809-measure-results.md`.
The completed run above used the previously handed-off `remote-test` command;
the correction runs through `orb-test`. Its measurement does not retroactively
validate the later benchmark correction.

Correction validation attempt `809-diagnose-corrected`, **2026-11-14
18:06:34–18:10:28 JST**, exited 127 before formatting or tests. Dot reported
`dot-ci: dot-run exited 5 without a result`; orb-test then fell back to
`orb-test-node:24`, where the command failed with
`bash: line 1: cargo: command not found`. No correction measurement was collected.
The single infrastructure retry above explicitly uses a Rust image and a
non-login shell to preserve that image's toolchain PATH. Its native timings,
if collected, run on the Mac's OrbStack VM and are a different environment from
the hosted-runner diagnosis; they must not be treated as optimization deltas.

The Rust-image attempt `809-diagnose-rust`, **2026-11-14
18:12:22–18:13:54 JST**, reached rustfmt and exited 1 on the repeated-open call's
line wrapping at `capacity_bench.rs:1950`. The benchmark did not run. The exact
rustfmt change is applied; the same checks and benchmark above are pending at
that corrected source.

## Instrumentation assumptions

- Phase durations are inclusive, with the existing per-sample `[rest]` calculation.
- Derived Clone implementations for resident/calculation/graph state are expanded
  field for field solely to measure their constituent copies. Document clone
  counters retain their existing meaning.
- Explicitly timed destruction of V1 diff snapshots, the evaluated candidate and
  its remaining footprint occurs at the end of their existing use; values and
  lifecycle operations are unchanged. Replaced resident state/document destruction
  remains inside its original assignment.
- Batch loops have one outer timer, avoiding a timer allocation for every cell.
  Named full scans, complete container clones, whole comparisons, destruction and
  formula projection loops remain meaningful leaf operations even above 3 ms.
- Threshold estimates must separate reusable/equal candidates from atomic failure
  fallbacks, preserve exact authorization footprints and current-time authorization,
  and preserve ordered binary64 totals. Estimates will be hypotheses, not measured
  implementations.

## Source-level cost inventory and possible remedies

Sources and line numbers below refer to the measured `cf71ae6` commit. Let N be
all populated fields/entities, F all formulas, K explicitly changed fields and
D affected calculations. BTree collections add lookup/union costs; these are
work-size descriptions, not claims of exact linear complexity. No change below
has been implemented or authorized by this diagnosis.

| operation and source | current work / measured cost | how it could follow the change size | boundary classification |
|---|---|---|---|
| Runtime snapshot, `runtime/src/lib.rs:7308`; lifecycle planner, `patch_lifecycle.rs:3975` | Two O(N) deep copies; edit 8.689 + 8.525 ms | Borrow the base while building one prepared candidate, then consume that exact candidate in planning/publication; truly O(K) candidate creation also needs shared storage or a transactional overlay | Borrow/consume plumbing changes crate Rust APIs/lifetimes; persistent Document storage changes the public Rust representation. Neither inherently needs wire/storage changes. |
| `resident_session.rs:1376`, projection changes | Union and compare all entities/fields; edit 5.663 ms, clear one 14.682 ms | Derive exact invalidation, calculation roots and diff scope from admitted commands plus old/new values, O(K + D); structural commands retain complete discovery | Internal engine work, possibly a new Rust preparation entry. Must prove completeness rather than trust an adapter-supplied list. |
| `resident_session.rs:470,576`, retained clone; `formula-engine/src/lib.rs:1115,1155,2070` | Full graph, values, nodes, diagnostics and address-index copies, 13.221 ms on edit | Isolate a candidate's changed state, preserving untouched graph/value partitions; commit only after successful authorization | Internal retained-state representation change. Whole-map copy-on-write alone still copies the complete map on first write and does not deliver O(K). |
| `formula-engine/src/lib.rs:1252` | Full before/after formula sets, 6.065 ms on edit despite 0.047 ms dirty recompute | Derive formula membership changes and outcomes from dirty roots/old+new dependencies, O(K + D) | Internal algorithm; retain full changed-projection reporting when global failure availability changes. |
| `workspace-engine/src/lib.rs:2519,3242,3280`; `resident_session.rs:539` | V1 repeats a 39.886 ms full projection preflight; fresh clear repeats base/candidate preflight | Carry a successful admitted-base witness and reuse the exact prepared verdict. Resolve changed formula fields directly with the retained address index; current "changed" helper still scans every field | Internal evidence reuse plus Rust constructor/preparation plumbing. No semantic/wire change if exact errors, base-first failure order and invalid-path fallback remain. |
| `diff-engine/src/lib.rs:291`; `patch_lifecycle.rs:4547` | V1 full comparisons, all-formula impacts, two snapshot copies and their 10.718 ms destruction | Lifecycle can consume scoped change evidence without constructing a renderable `SemanticDiff`; use exact K/D scope while keeping public snapshot-backed diff rendering intact | A crate-private helper or crate-shared Rust evidence API. Replacing public `SemanticDiff` snapshots itself is a separate Rust API change; changing evidence content/order would change contract. |
| `patch_lifecycle.rs:4594,6145`, V2 evidence | Clear-one scoped projection copies 26.128 ms; formula diff 33.356 ms, including all-formula enumeration/comparison | Read formula projections through borrowed data and restrict calculated impacts to proven affected formulas; direct canonical delta still covers every changed entity/field | Internal algorithm/Rust seam, no required storage or public semantic change. O(K + D) is still large for these clears. |
| `workspace-engine/src/lib.rs:3326`; lifecycle `plan_commands` | Whole formula-structure check (3.364 ms on edit), plus command application | Reuse a valid-base structure proof and inspect changed expressions; keep real command validation and associated writes | Internal only if every accepted/rejected command and authorization footprint remains exact. |
| `resident_session.rs:1151`; lifecycle `execute_profile:2551` and `verify_publication:3175` | Repeated complete candidate comparisons; installation comparison 3.657 ms, verification 3.257 ms on edit | Bind a prepared candidate to its base occurrence/revision and trusted ownership identity, then consume it once; keep complete comparison for untrusted/general candidates | Changes Rust publication/preparation seam. Cannot replace equality with revision alone: distinct candidates can share the same base revision. |
| Same publication/execute sources | Edit drops old retained 13.054 ms, old Document 5.733 ms, evaluated Document 4.272 ms | Reuse untouched owned partitions and avoid duplicate candidate owners; unchanged data must survive publication, last-owner release is still O(owned size) | Internal retained-state or public Document representation change. No deferred drop queue, background task or extra thread is proposed. |
| `runtime/src/lib.rs:7513,7561,7774,7860` | Clear rechecks capacity/strings, collection specs, and all formula sources; post-work 69.028 / 84.089 ms | Derive exact population/string/save-size deltas from old/new cells; preserve membership caches if schema/entity membership is unchanged; update source strings only for changed expressions | Runtime/internal Rust work. Removing stored cells still changes capacity; caches must preserve refusals, returned metadata and sources. |
| `resident_session.rs:382`; `runtime/src/lib.rs:5059` | Full ordered totals fold, 7.998 ms after edit | Reuse unaffected column totals and recompute affected numeric columns in the original member order; a per-revision cache only helps repeated reads, not the first post-edit request | Internal query cache if bit-identical. Running sum ± delta or a differently grouped reduction can change binary64 results and needs a semantic decision; it is not an equivalent optimization. |

All-O(table) elimination is not promised: an operation that changes 10,000 or
70,000 cells must still admit, authorize, disclose and return its actual changes.
Memo storage, preview, footprints, receipts, redaction and their destruction
contain O(K + D) work even after scans and snapshots are removed. The phase
tables keep residuals visible; no timer residual is automatically classified as
removable work.

## Savings hypotheses and alternative change sets

These are source-based estimates for this runner and fixture, not measured
post-change results. They use disjoint leaves where costs are combined; parent
and child timings are never counted together. Real replacements have nonzero
cost, and summing phase medians is only budget arithmetic.

| possible change set | measured opportunity / estimate | expected limit and authority needed |
|---|---|---|
| A. Keep public Document and diff representations; avoid redundant preflight, build lifecycle evidence without render snapshots, scope formula work and command-derived invalidation | Edit opportunity: preflight 39.886; V1 snapshot entity copies 18.057 + their drop 10.718; entity comparison 7.819 + formula work 15.636; retained formula sets 6.065; projection scan 5.663; structure scan 3.364. About 107 ms of disjoint work before replacement cost; plausible saving 80–105 ms | Leaves roughly 90–115 ms for edit+window by median arithmetic. Insufficient by itself to claim ≤50 ms. Internal changes and explicit Rust seam review; public SemanticDiff behavior stays intact. |
| B. A plus consume one candidate and share/partition Document and retained state | Additional edit opportunity: runtime/planner copies ~17.2; retained copy + old-retained drop ~26.3; old/evaluated Document drops ~10.0 ms. Plausible additional saving 30–50 ms, less any overlap with the chosen A implementation | Combined saving ~110–155 ms leaves ~40–85 ms. It might reach 50 ms but gives no p95 guarantee; public Document storage/type changes need work-lead's decision before implementation. Whole-map COW can erase these savings. |
| C. Clear-specific metadata/source refresh plus witnessed preflight reuse | Clear one has 52.969 ms source refresh, 14.733 ms structure/capacity rebuild and 44.588 ms redundant base preflight. Saving 70–100 ms after replacement work would put clear near 447–477 ms, plus its window if budgeted together | A candidate to test against the 500 ms clear budget without selecting a Document representation. Carry exact capacity/string deltas and admission proof; validate the fresh-open case. Three samples cannot establish p95. |
| D. Totals-only work | Full post-edit fold is 7.998 ms; reusing unchanged results while folding the three changed numeric columns out of six can save only part of it (roughly 0–4 ms; unmeasured) | Does not solve edit latency. Unchanged numeric totals may all be reused for clear-one Text removal. New-revision caching alone has zero first-read saving. |

Clear seven retains large K/D authorization and evidence costs; even removing
the repeated 47.107 ms base preflight and ~84 ms post-refresh cannot turn its
~2.0 s into a small scalar-edit cost. These alternatives do not propose dropping
receipt/disclosure work. Open/save remain outside this optimization diagnosis;
reusing their repeated full calculations is a separate change set and needs
exact admission/projection evidence.

The engine lead can choose A/C first, or request a Document representation design
for B. Neither route is selected here. Any implementation needs its own bounded
PR and native/browser measurements; this scratch branch must never merge.

## Contract and decision boundaries

Preserve ADR-0018's exact binary64 outcomes and typed formula identity,
ADR-0024's revision-pinned exact change, ADR-0026's complete authorization and
current trusted time at publication, ADR-0030's direct canonical evidence, and
ADR-0033's standalone authoritative snapshots. Implementation sharing does not
authorize changing canonical snapshot bytes, history guarantees or public data.

In particular, a reusable valid prepared candidate must be bound to the exact
occurrence, base revision and candidate; failed/unsupported/invalid preparations
keep atomic failure and the existing full checks. Both proposed write scopes and
derived disclosure scopes must remain complete. The two execution-time
authorization checks are not redundant simply because proposal authorization
succeeded earlier: grants or policy may have changed. Structural/schema/formula
changes and globally changing failure availability cannot inherit scalar-only
proofs.

No alternative here requires changing JSON, WASM ABI, `.ro`/`.roproj` formats or
the semantic contract. Some require new Rust borrowing/consumption APIs, and B
may change the public `Document` containers; those are explicit decisions for
the engine/work lead rather than permission granted by a performance estimate.
No async, Pin, unsafe or concurrency change is included.

## Code facts identified before the run

Scalar edits use V1 evidence in this fixture. `diff_with_calculations` retains
both a before and an after Document copy. Clear uses V2 evidence because it emits
UnsetField commands. Both paths still construct a planner candidate and compare
prepared state against it. Preparation clones retained calculation/graph/address
state and discovers changes by comparing the two full documents. Publication
checks candidate equality and replaces/drops old resident state. Clear refreshes
structure and all formula-source projections. A default zero-row window still
folds every Number field across every collection member.

Accepted contract references for the eventual alternatives: ADR-0018 (bound
formulas and deterministic binary64), ADR-0024 (revision-pinned SemanticPatch),
ADR-0026 (scoped semantic authorization/approval), ADR-0030 (canonical semantic
delta), ADR-0033 (snapshot-first semantic history), and the semantic diff,
Semantic API and authorization specifications. The diagnosis will distinguish
internal Rust implementation/API changes from ABI/wire/storage or contract changes.
