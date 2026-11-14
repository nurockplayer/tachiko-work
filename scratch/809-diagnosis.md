# #809 step 2: edit / clear / zero-row-window diagnosis

Scratch branch `scratch/809-measure`, PR #816. Never merge.

## Measurement

The single native rerun is pending. This report will receive the 10,000-row phase
tables, source inventory, savings estimates and alternative change sets when the
background job returns. No product optimization or design selection is included.

The existing bench now defaults to 10,000 rows only. Set
`CAPACITY_809_ALL_SIZES=1` to reproduce all four step-1 sizes. The standard command
needs no extra environment variable for this diagnostic rerun:

```sh
job-run --id 809-diagnose -- remote-test -- 'cargo test --release --manifest-path packages/browser-client/runtime/Cargo.toml --locked --features capacity-bench capacity_809_formula_table -- --ignored --test-threads=1 --nocapture'
```

Repetitions, fixture, assertions and native typed-call boundaries are unchanged.
There is no WASM run in step 2. Step-1 results remain in `809-measure-results.md`.

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
