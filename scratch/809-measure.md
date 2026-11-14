# #809 formula-table measurement (never merge)

From main `2da87c09c7f7d4fca3ac89c7459ecceb86ad4c0d`.

The ignored native test `capacity_809_formula_table` builds Sheet #375's exact
10-column shape at 2,500, 5,000, 7,500 and 10,000 rows, using 3 cell formulas
per row and no definitions. The codec frames v3 files as TWDPROJ1. It writes
identical fixtures to `$RUNNER_TEMP/809-<rows>.twd` for the real WASM ABI run.

Native output includes each of 3 opens, phase p50/p95/max, share of the whole
step, inclusive paths, per-sample residuals and Document clone counts. It runs
30 evenly spread edits, separates each zero-row window, clears 1 and 7 columns
3 times each from fresh opens, and saves/reopens 3 times. No warmup is discarded.
The 10,000-row size reduces remaining repeats to 1 and edits to 10 after
15 minutes, and prints the actual counts. Timers use nanoseconds; tables use ms.

Save uses `export_project_v3`, matching Sheet's `exportOpaque` route for Save a
copy for this fixture (`src/App.tsx`'s portableSave=false branch). It includes
fresh admission of the exported bytes before returning; the explicit reopen is
a separate measurement. Input and saved projects reopen through
`open_project_working_size`.

WASM measures open, edit, clear 1 and clear 7 at 5,000 and 10,000 rows with
separate zero-row window requests and combined durations. It includes JSON
serialization, arena writes and response parsing. Native measures typed calls.
The phase timer API is unconditional; its WASM implementation is empty because
this optional ABI measurement does not collect phases (and wasm32 lacks native
Instant support). The existing capacity-bench feature counts Document clones
only for the native benchmark.

All phase paths are inclusive; children must not be added to their parents.
`[rest]` subtracts direct child times before computing percentiles. Shares use
the whole operation, so inclusive nested shares are not additive. Missing phases
in a sample count as zero. Median is nearest rank (for 30 samples, rank 15),
p95 rank 29. Open timings retain cold/first-open differences. Fixture creation,
framing, request construction and clear-repeat rebuilds are outside the measured
native operations. Resident replacement/drop during open is included.

The scratch diff exceeds 300 lines because it contains the complete fixture,
native reporting bench, ABI harness and timers across the existing call paths.
No product behavior or capacity gates have been changed and no fixes are in scope.

Run remotely (no host cargo builds):

```sh
job-run --id 809-measure -- orb-test -- bash -lc 'CAPACITY_809_ALL_SIZES=1 cargo test --release --manifest-path packages/browser-client/runtime/Cargo.toml --locked --features capacity-bench capacity_809_formula_table -- --ignored --test-threads=1 --nocapture && pnpm --dir packages/browser-client install --frozen-lockfile && pnpm --dir packages/browser-client build && node packages/browser-client/runtime/tests/capacity_809_wire.mjs packages/browser-client/.build/designer_runtime.wasm'
```

The full remote command-step log contains all tables. Do not merge this branch,
change production behavior, or recommend fixes from this measurement.

Completed measurements and all phase tables: [809-measure-results.md](809-measure-results.md).

Step 2 defaults the bench to 10,000 rows only; use `CAPACITY_809_ALL_SIZES=1` for the original four-size run. See [809-diagnosis.md](809-diagnosis.md).
