# #678 Stage 5c XLSX qualification

This opt-in harness measures the Stage 5c decision after 5a/5b and #809. It
qualifies the Work producer, without claiming Sheet consumer acceptance.

## Method

The Rust generator uses the existing working-size XLSX writer, with no committed
binary fixture. One sheet has 10,000 data rows (plus a header) and 30 columns
(A–AD). Ten cells per data row are populated: A/B/G/AD Number, C/F Text, D Date,
E Boolean, and H/I same-row Number formulas (`A+B`, `A*2`). The other 20 cells
are blank. This is 100,000 populated data cells, including 20,000 formulas;
headers are excluded from the data-cell count. Values, headers, date and formulas
are deterministic. The small normal test uses the same generator at 200×30.

Each target runs three serial samples with no discarded warmup. Each sample
inspects the XLSX, imports it with explicit typed selection and installs a
working-size occurrence, then exports that opened occurrence using its returned
revision and metadata. Later imports replace the previous resident inside the
timer. Native uses `DesignerHost::spreadsheet`; Node drives the actual release
wasm32 ABI's request/project arenas and `tachiko_designer_spreadsheet_run`.

Timers include parsing, inspection, semantic admission, calculation, reply
serialization and installation, or workbook projection and XLSX encoding for
export. Generation, build/instantiation, request encoding/input copies, reply
copy/decoding and assertions are excluded. `open_ms` is the paired sum of XLSX
inspection and import (the import itself parses the source again, matching the
host workflow). Inspection and import are also reported separately. Each step
reports nearest-rank p50 and max. Every JSON reply and exported XLSX has a
recorded byte length in each sample.

Native open and export must each have max ≤3,000 ms. WASM p50 and max must each
be ≤2× the matching native statistic on the same runner. Import/export replies
must be ≤16 MiB. Inspection retains the decision's existing 48 MiB preview
exception. Reports are written before budget assertions, so a timing miss leaves
evidence for the lead's 5d decision. Refusals fail loudly with the exact wire
failure/limit; do not treat an admitted smaller shape as the target passing.

## Commands

Run these in order from the repository root, on the same GitHub runner and
revision, with Rust's wasm32 target and Node installed. Do not run the full
10,000-row measurements on the Owner's Mac. No instrumentation feature is needed.

Native:

```sh
cargo test --manifest-path packages/browser-client/runtime/Cargo.toml --locked --release --lib capacity_stage5_qualification -- --ignored --nocapture
```

Actual WASM (consumes the native run's identical generated XLSX and report):

```sh
cargo build --manifest-path packages/browser-client/runtime/Cargo.toml --locked --release --target wasm32-unknown-unknown && node packages/browser-client/runtime/tests/capacity_stage5_wire.mjs packages/browser-client/runtime/target/wasm32-unknown-unknown/release/tachiko_designer_runtime.wasm packages/browser-client/runtime/target/678-stage5-5c
```

Artifacts: `packages/browser-client/runtime/target/678-stage5-5c/{source.xlsx,native.json,wasm.json}`.
`STAGE5_ROWS=200` overrides native shape for a smoke run; `STAGE5_DIR` overrides
its output directory (pass the same directory to Node). A custom
`CARGO_TARGET_DIR` also requires adjusting the WASM artifact argument.

## Results (lead fills on GitHub runner)

Revision, runner/CPU, Rust/Node versions, run link, and timestamp with zone:

| Target | Shape / populated cells | n | Inspect p50 / max ms | Import p50 / max ms | Open p50 / max ms | Export p50 / max ms | Inspect / import / export reply bytes (each sample) | XLSX bytes (each sample) | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Native release | | | | | | | | | |
| Actual WASM release | | | | | | | | | |

If refused: exact limit, attempted/maximum values, and largest admitted shape
(record the next refused shape as evidence of the boundary):

5d required (only if 5c misses budget):
