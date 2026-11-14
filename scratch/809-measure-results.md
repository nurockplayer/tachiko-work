# #809 step 1: formula-table measurements

**Scratch only — never merge.** Measured code: `748ee11f` (the results commit only adds this report).

Measurement correction: repeated native opens here exclude destruction of the
previous resident; `open_809` used a fresh slot and the caller replaced its slot
after timing. Reopen also measures admission into a fresh slot. The step-2
diagnosis records this limitation; the corrected benchmark measures repeated
opens through the retained slot. Historical timings below are unchanged.

Run: **2026-11-14 17:15:06–17:35:02 JST**. [Successful hosted run](https://github.com/nurockplayer/tachiko-work/actions/runs/43574909805), GitHub-hosted Ubuntu 24.04, Rust 1.99.0, Node 24, pnpm 12.11.2. Native ignored bench: 80.20 s. Entire remote command: 1,185 s (includes builds/install and WASM).

All four sizes completed 3 opens, 30 edits, 3 clears of each kind and 3 save/reopen pairs. **No repeat reduction.** Clears start from fresh opens, outside the measured interval. The first edit's column_7 result was asserted to equal twice the edited value; clear publications and the absence of cleared fields were asserted. The WASM path checks successful real ABI responses, row count and zero-row replies.

Fixture: 1 schema, 10 optional fields, no constraints or definitions; 3 same-entity cell formulas per row (7,500 / 15,000 / 22,500 / 30,000 formula cells). Built in Rust, encoded by `encode_roproj_v3`, framed as `TWDPROJ1`; the WASM script reads those exact bytes. Save uses **`export_project_v3`**, matching Sheet's `exportOpaque` Save a copy path for this fixture. That export includes its own admission preflight; explicit reopen is timed separately.

Native calls are typed runtime calls with phase timers and clone counters. WASM times include JSON serialization, arena copies and response parsing; its phase timer implementation is empty. These are hosted-runner measurements, not timings on Sheet's browser device.

## Native operation summary

Values are p50 ms. Open, clear, save and reopen use n=3; edits and their zero-row windows use n=30. Every operation has the same clone count at all sizes. Growth is the ratio of the 10,000-row and 5,000-row p50, not a claim about asymptotic complexity.

| step | 2,500 ms | 5,000 ms | 7,500 ms | 10,000 ms | 5k → 10k | Document clones / step |
|---|---:|---:|---:|---:|---:|---:|
| open combined | 221.078 | 482.672 | 836.312 | 1192.843 | 2.471× | 1 |
| edit | 39.108 | 92.884 | 152.088 | 225.216 | 2.425× | 4 |
| edit zero-row window | 1.611 | 4.092 | 6.838 | 10.366 | 2.533× | 0 |
| clear 1 | 97.907 | 240.635 | 394.979 | 558.623 | 2.321× | 2 |
| clear 1 zero-row window | 1.592 | 4.101 | 7.113 | 9.888 | 2.411× | 0 |
| clear 7 | 451.969 | 1059.475 | 1655.152 | 2329.428 | 2.199× | 2 |
| clear 7 zero-row window | 1.874 | 9.910 | 14.876 | 11.381 | 1.148× | 0 |
| save export_project_v3 | 247.502 | 567.689 | 911.135 | 1329.822 | 2.343× | 2 |
| reopen | 215.296 | 502.890 | 818.516 | 1201.933 | 2.390× | 1 |

## Phase facts

At 10,000 rows, the open baseline is 881.688 ms (73.91% of open): formula source projection 331.465 ms (27.79%), validation 264.601 ms (22.18%), and resident creation 259.762 ms (21.78%). Three full-calculation calls appear separately under those three parents; their times include node/graph construction, pregraph/cycle checks, initial values and formula evaluation. Bundle decode is 271.538 ms (22.76%); its storage parsing is 155.701 ms (13.05%), canonical re-encoding/comparison 65.281 ms (5.47%).

For one-cell edit, lifecycle proposal construction is 134.656 ms (59.79%), execute 47.056 ms (20.89%), and prepared evaluation 27.837 ms (12.36%). Its zero-row window is 10.366 ms, of which column totals are 10.324 ms (99.60%). For clear one, proposal construction is 200.626 ms (35.91%), prepared evaluation 120.465 ms (21.56%), and execute 84.992 ms (15.21%). For clear seven, they are 895.021 ms (38.42%), 520.797 ms (22.36%), and 440.141 ms (18.89%). These are inclusive per-phase medians; they need not sum exactly to the median of their parent.

The constructor's lifecycle phase registers a fixed catalogue of grants; it is 0.035 ms at 10,000 rows. Installing this fixture's empty presentation is 0.002 ms. There are no formula definitions or grouped-sum definitions to iterate. Formula-source and formula-calculation work shown here therefore applies to the row-local cell formulas. Bootstrap's definition-ID listing still exports a complete snapshot even though the definition list is empty: bootstrap is 15.783 ms and accounts for the open's one Document clone. The name `unframe` denotes the inclusive bundle decoder, including the storage child; its `[rest]` row contains framing/dispatch work outside those children.

The current edit path uses prepared retained-state validation (`prepare_evaluation/retained_update/validation_report`); it has no standalone `validate(candidate)` call. For scalar edits, formula preflight has a 46.528 ms maximum while its p50 rounds to 0.001 ms. All 30 edits, including the first, are retained.

## Reading the detailed tables

Durations are milliseconds from native nanosecond timers. Each indented phase is inclusive; do not add children to their parents. `[rest]` is computed per sample as parent minus direct children before taking percentiles. Share is phase p50 / whole-step p50. Different phases can have their median on different repetitions. Missing phases count as zero. p50 and p95 use nearest rank (30 samples: ranks 15 and 29). Values rounded to 0.000 are below the displayed precision, not omitted work.

Each original open is reported separately, followed by the n=3 combined table. The edit and its zero-row window, and each clear and its zero-row window, remain separate timed steps. Request construction and fixture/repeat setup are outside native timing. WASM bulk edit arrays are prepared outside timing; request-envelope construction inside the measured closures, serialization, arena copies and reply parsing are included. No warmup is discarded.

## Native 2,500 rows

### open 1 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 221.078 | 221.078 | 221.078 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.321 | 0.321 | 0.321 | 0.15% |
| &nbsp;&nbsp;`open` | 220.757 | 220.757 | 220.757 | 99.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 220.754 | 220.754 | 220.754 | 99.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 63.023 | 63.023 | 63.023 | 28.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 63.022 | 63.022 | 63.022 | 28.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.795 | 0.795 | 0.795 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 62.227 | 62.227 | 62.227 | 28.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 17.095 | 17.095 | 17.095 | 7.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 3.088 | 3.088 | 3.088 | 1.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.611 | 0.611 | 0.611 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.326 | 1.326 | 1.326 | 0.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.151 | 1.151 | 1.151 | 0.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 4.331 | 4.331 | 4.331 | 1.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 37.707 | 37.707 | 37.707 | 17.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 148.123 | 148.123 | 148.123 | 67.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 148.118 | 148.118 | 148.118 | 67.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 3.833 | 3.833 | 3.833 | 1.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.130 | 1.130 | 1.130 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 0.992 | 0.992 | 0.992 | 0.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.127 | 1.127 | 1.127 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.583 | 0.583 | 0.583 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 1.166 | 1.166 | 1.166 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 55.124 | 55.124 | 55.124 | 24.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 16.693 | 16.693 | 16.693 | 7.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.431 | 38.431 | 38.431 | 17.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.723 | 0.723 | 0.723 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.788 | 9.788 | 9.788 | 4.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.431 | 10.431 | 10.431 | 4.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.350 | 0.350 | 0.350 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.138 | 17.138 | 17.138 | 7.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.037 | 0.037 | 0.037 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 43.171 | 43.171 | 43.171 | 19.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 43.169 | 43.169 | 43.169 | 19.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.602 | 0.602 | 0.602 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.926 | 39.926 | 39.926 | 18.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.862 | 0.862 | 0.862 | 0.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.947 | 9.947 | 9.947 | 4.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.858 | 10.858 | 10.858 | 4.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.374 | 0.374 | 0.374 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.886 | 17.886 | 17.886 | 8.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 2.641 | 2.641 | 2.641 | 1.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 44.782 | 44.782 | 44.782 | 20.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.114 | 4.114 | 4.114 | 1.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 40.669 | 40.669 | 40.669 | 18.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.863 | 0.863 | 0.863 | 0.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.584 | 9.584 | 9.584 | 4.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.525 | 10.525 | 10.525 | 4.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.382 | 0.382 | 0.382 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 19.315 | 19.315 | 19.315 | 8.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 8.157 | 8.157 | 8.157 | 3.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.199 | 0.199 | 0.199 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.816 | 4.816 | 4.816 | 2.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 3.141 | 3.141 | 3.141 | 1.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.299 | 1.299 | 1.299 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.251 | 0.251 | 0.251 | 0.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.590 | 1.590 | 1.590 | 0.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 1.446 | 1.446 | 1.446 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.445 | 1.445 | 1.445 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 2 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 235.193 | 235.193 | 235.193 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.285 | 0.285 | 0.285 | 0.12% |
| &nbsp;&nbsp;`open` | 234.908 | 234.908 | 234.908 | 99.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 234.907 | 234.907 | 234.907 | 99.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 77.701 | 77.701 | 77.701 | 33.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 77.701 | 77.701 | 77.701 | 33.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.206 | 1.206 | 1.206 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 76.495 | 76.495 | 76.495 | 32.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 23.254 | 23.254 | 23.254 | 9.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 4.192 | 4.192 | 4.192 | 1.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.886 | 0.886 | 0.886 | 0.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.962 | 1.962 | 1.962 | 0.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.344 | 1.344 | 1.344 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 4.684 | 4.684 | 4.684 | 1.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 44.359 | 44.359 | 44.359 | 18.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 147.581 | 147.581 | 147.581 | 62.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 147.580 | 147.580 | 147.580 | 62.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 4.030 | 4.030 | 4.030 | 1.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.161 | 1.161 | 1.161 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.085 | 1.085 | 1.085 | 0.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.183 | 1.183 | 1.183 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.602 | 0.602 | 0.602 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 1.146 | 1.146 | 1.146 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 55.444 | 55.444 | 55.444 | 23.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 16.820 | 16.820 | 16.820 | 7.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.624 | 38.624 | 38.624 | 16.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.732 | 0.732 | 0.732 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.713 | 9.713 | 9.713 | 4.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.569 | 10.569 | 10.569 | 4.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.351 | 0.351 | 0.351 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.259 | 17.259 | 17.259 | 7.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.025 | 0.025 | 0.025 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 43.002 | 43.002 | 43.002 | 18.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 43.000 | 43.000 | 43.000 | 18.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.566 | 0.566 | 0.566 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 40.041 | 40.041 | 40.041 | 17.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.730 | 0.730 | 0.730 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.961 | 9.961 | 9.961 | 4.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.711 | 10.711 | 10.711 | 4.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.363 | 0.363 | 0.363 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 18.276 | 18.276 | 18.276 | 7.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 2.393 | 2.393 | 2.393 | 1.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 43.931 | 43.931 | 43.931 | 18.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.237 | 4.237 | 4.237 | 1.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.694 | 39.694 | 39.694 | 16.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.743 | 0.743 | 0.743 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.752 | 9.752 | 9.752 | 4.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.600 | 10.600 | 10.600 | 4.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.370 | 0.370 | 0.370 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 18.229 | 18.229 | 18.229 | 7.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 7.646 | 7.646 | 7.646 | 3.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.182 | 0.182 | 0.182 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.600 | 4.600 | 4.600 | 1.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 2.864 | 2.864 | 2.864 | 1.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.255 | 1.255 | 1.255 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.210 | 0.210 | 0.210 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.399 | 1.399 | 1.399 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 1.974 | 1.974 | 1.974 | 0.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.973 | 1.973 | 1.973 | 0.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 3 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 212.470 | 212.470 | 212.470 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.301 | 0.301 | 0.301 | 0.14% |
| &nbsp;&nbsp;`open` | 212.169 | 212.169 | 212.169 | 99.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 212.168 | 212.168 | 212.168 | 99.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 59.163 | 59.163 | 59.163 | 27.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 59.163 | 59.163 | 59.163 | 27.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.177 | 0.177 | 0.177 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 58.985 | 58.985 | 58.985 | 27.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 13.159 | 13.159 | 13.159 | 6.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 3.285 | 3.285 | 3.285 | 1.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.651 | 0.651 | 0.651 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.431 | 1.431 | 1.431 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.202 | 1.202 | 1.202 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 4.360 | 4.360 | 4.360 | 2.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 38.175 | 38.175 | 38.175 | 17.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 145.656 | 145.656 | 145.656 | 68.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 145.656 | 145.656 | 145.656 | 68.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 3.959 | 3.959 | 3.959 | 1.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.164 | 1.164 | 1.164 | 0.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.040 | 1.040 | 1.040 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.169 | 1.169 | 1.169 | 0.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.586 | 0.586 | 0.586 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 1.217 | 1.217 | 1.217 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 55.690 | 55.690 | 55.690 | 26.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 16.806 | 16.806 | 16.806 | 7.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.884 | 38.884 | 38.884 | 18.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.633 | 0.633 | 0.633 | 0.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 10.060 | 10.060 | 10.060 | 4.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.526 | 10.526 | 10.526 | 4.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.371 | 0.371 | 0.371 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.294 | 17.294 | 17.294 | 8.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.026 | 0.026 | 0.026 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 42.032 | 42.032 | 42.032 | 19.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 42.030 | 42.030 | 42.030 | 19.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.574 | 0.574 | 0.574 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.060 | 39.060 | 39.060 | 18.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.703 | 0.703 | 0.703 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.962 | 9.962 | 9.962 | 4.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.603 | 10.603 | 10.603 | 4.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.362 | 0.362 | 0.362 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.429 | 17.429 | 17.429 | 8.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 2.396 | 2.396 | 2.396 | 1.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 42.730 | 42.730 | 42.730 | 20.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.135 | 4.135 | 4.135 | 1.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.595 | 38.595 | 38.595 | 18.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.622 | 0.622 | 0.622 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.753 | 9.753 | 9.753 | 4.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.544 | 10.544 | 10.544 | 4.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.374 | 0.374 | 0.374 | 0.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.302 | 17.302 | 17.302 | 8.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 5.875 | 5.875 | 5.875 | 2.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.181 | 0.181 | 0.181 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 2.839 | 2.839 | 2.839 | 1.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 2.854 | 2.854 | 2.854 | 1.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.248 | 1.248 | 1.248 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.209 | 0.209 | 0.209 | 0.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.397 | 1.397 | 1.397 | 0.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 1.470 | 1.470 | 1.470 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.469 | 1.469 | 1.469 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open combined (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 221.078 | 235.193 | 235.193 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.301 | 0.321 | 0.321 | 0.14% |
| &nbsp;&nbsp;`open` | 220.757 | 234.908 | 234.908 | 99.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 220.754 | 234.907 | 234.907 | 99.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 63.023 | 77.701 | 77.701 | 28.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 63.022 | 77.701 | 77.701 | 28.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.795 | 1.206 | 1.206 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 62.227 | 76.495 | 76.495 | 28.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 17.095 | 23.254 | 23.254 | 7.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 3.285 | 4.192 | 4.192 | 1.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.651 | 0.886 | 0.886 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.431 | 1.962 | 1.962 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.202 | 1.344 | 1.344 | 0.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 4.360 | 4.684 | 4.684 | 1.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 38.175 | 44.359 | 44.359 | 17.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 147.581 | 148.123 | 148.123 | 66.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 147.580 | 148.118 | 148.118 | 66.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 3.959 | 4.030 | 4.030 | 1.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.161 | 1.164 | 1.164 | 0.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.040 | 1.085 | 1.085 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.169 | 1.183 | 1.183 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.586 | 0.602 | 0.602 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 1.166 | 1.217 | 1.217 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 55.444 | 55.690 | 55.690 | 25.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 16.806 | 16.820 | 16.820 | 7.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.624 | 38.884 | 38.884 | 17.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.723 | 0.732 | 0.732 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.788 | 10.060 | 10.060 | 4.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.526 | 10.569 | 10.569 | 4.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.351 | 0.371 | 0.371 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.259 | 17.294 | 17.294 | 7.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.026 | 0.037 | 0.037 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 43.002 | 43.171 | 43.171 | 19.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 43.000 | 43.169 | 43.169 | 19.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.574 | 0.602 | 0.602 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.926 | 40.041 | 40.041 | 18.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.730 | 0.862 | 0.862 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.961 | 9.962 | 9.962 | 4.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.711 | 10.858 | 10.858 | 4.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.363 | 0.374 | 0.374 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.886 | 18.276 | 18.276 | 8.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 2.396 | 2.641 | 2.641 | 1.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 43.931 | 44.782 | 44.782 | 19.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.135 | 4.237 | 4.237 | 1.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.694 | 40.669 | 40.669 | 17.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.743 | 0.863 | 0.863 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.752 | 9.753 | 9.753 | 4.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.544 | 10.600 | 10.600 | 4.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.374 | 0.382 | 0.382 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 18.229 | 19.315 | 19.315 | 8.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 7.646 | 8.157 | 8.157 | 3.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.182 | 0.199 | 0.199 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.600 | 4.816 | 4.816 | 2.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 2.864 | 3.141 | 3.141 | 1.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.255 | 1.299 | 1.299 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.210 | 0.251 | 0.251 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.399 | 1.590 | 1.590 | 0.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 1.470 | 1.974 | 1.974 | 0.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.469 | 1.973 | 1.973 | 0.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### edit (n=30; Document clones per step: 4)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 39.108 | 43.683 | 55.924 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 39.108 | 43.683 | 55.923 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 39.062 | 43.617 | 55.760 | 99.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.004 | 0.004 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 39.059 | 43.614 | 55.757 | 99.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.291 | 0.693 | 0.764 | 0.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 38.774 | 43.324 | 55.202 | 99.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.360 | 0.391 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 0.010 | 0.013 | 0.013 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 6.881 | 7.940 | 8.553 | 17.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.777 | 2.097 | 2.133 | 4.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 5.061 | 6.114 | 6.470 | 12.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 25.699 | 29.625 | 29.880 | 65.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 0.008 | 0.010 | 0.010 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 4.496 | 5.118 | 15.763 | 11.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.005 | 0.014 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 0.001 | 0.001 | 9.877 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 0.914 | 1.182 | 1.366 | 2.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 2.354 | 2.491 | 3.463 | 6.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 1.197 | 1.454 | 1.714 | 3.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 1.190 | 1.447 | 1.705 | 3.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 0.006 | 0.006 | 0.008 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 0.010 | 0.012 | 0.013 | 0.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.004 | 0.006 | 0.013 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 0.004 | 0.005 | 0.005 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 1.333 | 1.674 | 2.700 | 3.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.056 | 0.163 | 0.271 | 0.14% |

### edit zero-row window (n=30; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1.611 | 1.807 | 1.821 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.01% |
| &nbsp;&nbsp;`handle` | 1.611 | 1.806 | 1.821 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 1.610 | 1.806 | 1.820 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.002 | 0.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 1.609 | 1.804 | 1.819 | 99.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.015 | 0.018 | 0.024 | 0.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.007 | 0.008 | 0.009 | 0.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.585 | 1.778 | 1.794 | 98.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.001 | 0.01% |

### clear 1 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 97.907 | 103.223 | 103.223 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 97.907 | 103.223 | 103.223 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 97.906 | 103.222 | 103.222 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.147 | 0.150 | 0.150 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 97.755 | 103.075 | 103.075 | 99.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.928 | 1.961 | 1.961 | 1.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 95.831 | 101.114 | 101.114 | 97.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.223 | 1.251 | 1.251 | 1.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 0.364 | 0.365 | 0.365 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 2.222 | 2.226 | 2.226 | 2.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.166 | 1.184 | 1.184 | 1.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.045 | 1.056 | 1.056 | 1.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 13.515 | 13.812 | 13.812 | 13.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.217 | 7.237 | 7.237 | 7.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 6.298 | 6.576 | 6.576 | 6.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 36.052 | 41.394 | 41.394 | 36.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 15.481 | 15.488 | 15.488 | 15.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 0.604 | 0.610 | 0.610 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.603 | 0.609 | 0.609 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 22.267 | 22.365 | 22.365 | 22.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 9.769 | 9.821 | 9.821 | 9.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 3.271 | 3.281 | 3.281 | 3.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 2.442 | 2.452 | 2.452 | 2.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 6.808 | 6.828 | 6.828 | 6.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 5.905 | 5.927 | 5.927 | 6.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 0.899 | 0.901 | 0.901 | 0.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 1.900 | 2.078 | 2.078 | 1.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.015 | 0.015 | 0.015 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 0.792 | 0.821 | 0.821 | 0.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 1.081 | 1.082 | 1.082 | 1.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 1 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1.592 | 1.658 | 1.658 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.02% |
| &nbsp;&nbsp;`handle` | 1.592 | 1.658 | 1.658 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 1.591 | 1.658 | 1.658 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 1.590 | 1.657 | 1.657 | 99.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.020 | 0.020 | 0.020 | 1.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.063 | 0.126 | 0.126 | 3.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.508 | 1.511 | 1.511 | 94.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.01% |

### clear 7 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 451.969 | 476.599 | 476.599 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 451.968 | 476.598 | 476.598 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 451.967 | 476.597 | 476.597 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.804 | 0.805 | 0.805 | 0.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 451.172 | 475.792 | 475.792 | 99.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 11.358 | 14.201 | 14.201 | 2.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 439.867 | 461.592 | 461.592 | 97.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 9.811 | 10.100 | 10.100 | 2.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 1.948 | 2.252 | 2.252 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 1.870 | 2.500 | 2.500 | 0.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.650 | 0.841 | 0.841 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.220 | 1.659 | 1.659 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 88.721 | 96.404 | 96.404 | 19.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 74.216 | 82.320 | 82.320 | 16.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 14.505 | 14.595 | 14.595 | 3.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 177.539 | 203.849 | 203.849 | 39.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 15.857 | 15.923 | 15.923 | 3.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 0.620 | 0.639 | 0.639 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.619 | 0.638 | 0.638 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 99.078 | 99.187 | 99.187 | 21.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 9.965 | 10.030 | 10.030 | 2.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 7.219 | 7.322 | 7.322 | 1.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 2.904 | 2.932 | 2.932 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 78.948 | 79.031 | 79.031 | 17.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 70.333 | 70.350 | 70.350 | 15.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 8.635 | 8.696 | 8.696 | 1.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 33.531 | 36.852 | 36.852 | 7.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.016 | 0.016 | 0.016 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 3.162 | 3.178 | 3.178 | 0.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 1.838 | 1.874 | 1.874 | 0.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.001 | 0.001 | 0.001 | 0.00% |

### clear 7 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1.874 | 1.891 | 1.891 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.02% |
| &nbsp;&nbsp;`handle` | 1.874 | 1.891 | 1.891 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 1.874 | 1.890 | 1.890 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 1.873 | 1.889 | 1.889 | 99.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.022 | 0.022 | 0.022 | 1.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.253 | 0.264 | 0.264 | 13.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.603 | 1.611 | 1.611 | 85.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### save export_project_v3 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 247.502 | 249.467 | 249.467 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`export_project_v3` | 247.501 | 249.466 | 249.466 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 29.870 | 30.053 | 30.053 | 12.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 214.984 | 216.851 | 216.851 | 86.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 59.647 | 61.125 | 61.125 | 24.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 59.647 | 61.125 | 61.125 | 24.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.134 | 0.150 | 0.150 | 0.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 59.531 | 60.975 | 60.975 | 24.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 14.036 | 15.494 | 15.494 | 5.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 3.138 | 3.209 | 3.209 | 1.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.624 | 0.640 | 0.640 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.357 | 1.404 | 1.404 | 0.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.162 | 1.165 | 1.165 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 4.316 | 4.343 | 4.343 | 1.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 38.022 | 38.072 | 38.072 | 15.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 146.391 | 146.426 | 146.426 | 59.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 146.391 | 146.425 | 146.425 | 59.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 3.931 | 3.962 | 3.962 | 1.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.149 | 1.151 | 1.151 | 0.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.031 | 1.049 | 1.049 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.156 | 1.193 | 1.193 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.589 | 0.589 | 0.589 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 1.153 | 1.164 | 1.164 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 56.140 | 56.279 | 56.279 | 22.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 17.028 | 17.601 | 17.601 | 6.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.789 | 39.152 | 39.152 | 15.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.721 | 0.835 | 0.835 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.713 | 9.717 | 9.717 | 3.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.766 | 10.793 | 10.793 | 4.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.392 | 0.410 | 0.410 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.237 | 17.726 | 17.726 | 6.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.026 | 0.027 | 0.027 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 42.149 | 42.327 | 42.327 | 17.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 42.148 | 42.326 | 42.326 | 17.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.555 | 0.574 | 0.574 | 0.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.240 | 39.407 | 39.407 | 15.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.728 | 0.730 | 0.730 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.992 | 10.216 | 10.216 | 4.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.516 | 10.517 | 10.517 | 4.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.367 | 0.385 | 0.385 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.507 | 17.831 | 17.831 | 7.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 2.364 | 2.384 | 2.384 | 0.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 42.816 | 42.840 | 42.840 | 17.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.031 | 4.046 | 4.046 | 1.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 38.784 | 38.793 | 38.793 | 15.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.665 | 0.698 | 0.698 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.741 | 9.906 | 9.906 | 3.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.509 | 10.676 | 10.676 | 4.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.358 | 0.361 | 0.361 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.353 | 17.465 | 17.465 | 7.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 7.410 | 7.611 | 7.611 | 2.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.181 | 0.182 | 0.182 | 0.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.392 | 4.505 | 4.505 | 1.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 2.869 | 2.925 | 2.925 | 1.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.235 | 1.279 | 1.279 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.213 | 0.280 | 0.280 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.402 | 1.409 | 1.409 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 1.498 | 1.719 | 1.719 | 0.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.497 | 1.718 | 1.718 | 0.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.316 | 1.326 | 1.326 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.298 | 1.320 | 1.320 | 0.52% |

### reopen (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 215.296 | 219.138 | 219.138 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.093 | 0.094 | 0.094 | 0.04% |
| &nbsp;&nbsp;`open` | 215.202 | 219.046 | 219.046 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 215.201 | 219.045 | 219.045 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 60.697 | 61.892 | 61.892 | 28.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 60.696 | 61.891 | 61.891 | 28.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.183 | 0.186 | 0.186 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 60.548 | 61.705 | 61.705 | 28.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.008 | 0.008 | 0.008 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 13.634 | 13.878 | 13.878 | 6.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 3.830 | 4.477 | 4.477 | 1.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.787 | 0.884 | 0.884 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.685 | 2.157 | 2.157 | 0.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.358 | 1.437 | 1.437 | 0.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 4.476 | 4.515 | 4.515 | 2.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 38.675 | 38.867 | 38.867 | 17.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 148.000 | 150.035 | 150.035 | 68.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 147.999 | 150.034 | 150.034 | 68.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 4.276 | 4.424 | 4.424 | 1.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.237 | 1.253 | 1.253 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 1.160 | 1.210 | 1.210 | 0.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.231 | 1.263 | 1.263 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 0.632 | 0.714 | 0.714 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 1.198 | 1.245 | 1.245 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 56.258 | 57.079 | 57.079 | 26.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 17.171 | 17.897 | 17.897 | 7.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.182 | 39.189 | 39.189 | 18.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.687 | 0.710 | 0.710 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.882 | 9.917 | 9.917 | 4.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.713 | 10.835 | 10.835 | 4.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.375 | 0.377 | 0.377 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.408 | 17.466 | 17.466 | 8.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.027 | 0.027 | 0.027 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 42.587 | 43.400 | 43.400 | 19.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 42.585 | 43.399 | 43.399 | 19.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.582 | 0.604 | 0.604 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.536 | 40.167 | 40.167 | 18.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.663 | 0.675 | 0.675 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 10.098 | 10.270 | 10.270 | 4.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.859 | 10.883 | 10.883 | 5.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.381 | 0.443 | 0.443 | 0.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.528 | 17.920 | 17.920 | 8.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 2.574 | 2.628 | 2.628 | 1.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 43.513 | 44.005 | 44.005 | 20.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.300 | 4.372 | 4.372 | 2.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 39.233 | 39.633 | 39.633 | 18.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.647 | 0.658 | 0.658 | 0.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 9.934 | 10.134 | 10.134 | 4.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 10.698 | 10.750 | 10.750 | 4.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 0.388 | 0.428 | 0.428 | 0.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 17.576 | 17.662 | 17.662 | 8.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 4.878 | 4.954 | 4.954 | 2.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.181 | 0.194 | 0.194 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 1.750 | 1.875 | 1.875 | 0.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 2.898 | 2.967 | 2.967 | 1.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.255 | 1.260 | 1.260 | 0.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.238 | 0.243 | 0.243 | 0.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 1.409 | 1.463 | 1.463 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 1.750 | 2.160 | 2.160 | 0.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 1.750 | 2.159 | 2.159 | 0.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

Actual counts: `809_COUNTS rows=2500 opens=3 edits=30 saves=3 reduced=false`. Clear counts: 3 for each kind, as in the tables.

## Native 5,000 rows

### open 1 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 482.672 | 482.672 | 482.672 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.047 | 0.047 | 0.047 | 0.01% |
| &nbsp;&nbsp;`open` | 482.625 | 482.625 | 482.625 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 482.624 | 482.624 | 482.624 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 122.586 | 122.586 | 122.586 | 25.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 122.586 | 122.586 | 122.586 | 25.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.074 | 2.074 | 2.074 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 120.512 | 120.512 | 120.512 | 24.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 28.183 | 28.183 | 28.183 | 5.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 7.050 | 7.050 | 7.050 | 1.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.408 | 1.408 | 1.408 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.238 | 3.238 | 3.238 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.403 | 2.403 | 2.403 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 9.057 | 9.057 | 9.057 | 1.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 76.214 | 76.214 | 76.214 | 15.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 346.114 | 346.114 | 346.114 | 71.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 346.113 | 346.113 | 346.113 | 71.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 8.777 | 8.777 | 8.777 | 1.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.402 | 2.402 | 2.402 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 2.674 | 2.674 | 2.674 | 0.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.382 | 2.382 | 2.382 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.320 | 1.320 | 1.320 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 2.862 | 2.862 | 2.862 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 130.961 | 130.961 | 130.961 | 27.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 38.319 | 38.319 | 38.319 | 7.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 92.643 | 92.643 | 92.643 | 19.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.651 | 1.651 | 1.651 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 23.024 | 23.024 | 23.024 | 4.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.761 | 24.761 | 24.761 | 5.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.144 | 1.144 | 1.144 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 42.062 | 42.062 | 42.062 | 8.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.028 | 0.028 | 0.028 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 105.134 | 105.134 | 105.134 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 105.132 | 105.132 | 105.132 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.357 | 1.357 | 1.357 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 98.048 | 98.048 | 98.048 | 20.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.707 | 1.707 | 1.707 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 23.114 | 23.114 | 23.114 | 4.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 26.406 | 26.406 | 26.406 | 5.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.323 | 1.323 | 1.323 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 45.498 | 45.498 | 45.498 | 9.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 5.726 | 5.726 | 5.726 | 1.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 98.347 | 98.347 | 98.347 | 20.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 10.544 | 10.544 | 10.544 | 2.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 87.802 | 87.802 | 87.802 | 18.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.543 | 1.543 | 1.543 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.093 | 22.093 | 22.093 | 4.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 23.800 | 23.800 | 23.800 | 4.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.039 | 1.039 | 1.039 | 0.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 39.327 | 39.327 | 39.327 | 8.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 9.560 | 9.560 | 9.560 | 1.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.183 | 0.183 | 0.183 | 0.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.133 | 4.133 | 4.133 | 0.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 5.245 | 5.245 | 5.245 | 1.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.281 | 1.281 | 1.281 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.559 | 0.559 | 0.559 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.405 | 3.405 | 3.405 | 0.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 4.358 | 4.358 | 4.358 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.357 | 4.357 | 4.357 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 2 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 477.834 | 477.834 | 477.834 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.048 | 0.048 | 0.048 | 0.01% |
| &nbsp;&nbsp;`open` | 477.786 | 477.786 | 477.786 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 477.785 | 477.785 | 477.785 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 124.095 | 124.095 | 124.095 | 25.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 124.095 | 124.095 | 124.095 | 25.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.320 | 0.320 | 0.320 | 0.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 123.775 | 123.775 | 123.775 | 25.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 28.042 | 28.042 | 28.042 | 5.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 7.755 | 7.755 | 7.755 | 1.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.571 | 1.571 | 1.571 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.662 | 3.662 | 3.662 | 0.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.522 | 2.522 | 2.522 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 9.313 | 9.313 | 9.313 | 1.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 78.658 | 78.658 | 78.658 | 16.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 339.480 | 339.480 | 339.480 | 71.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 339.479 | 339.479 | 339.479 | 71.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 8.888 | 8.888 | 8.888 | 1.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.379 | 2.379 | 2.379 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 2.766 | 2.766 | 2.766 | 0.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.416 | 2.416 | 2.416 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.327 | 1.327 | 1.327 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 2.907 | 2.907 | 2.907 | 0.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 127.190 | 127.190 | 127.190 | 26.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 38.555 | 38.555 | 38.555 | 8.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 88.635 | 88.635 | 88.635 | 18.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.691 | 1.691 | 1.691 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.442 | 22.442 | 22.442 | 4.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.085 | 24.085 | 24.085 | 5.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.063 | 1.063 | 1.063 | 0.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 39.352 | 39.352 | 39.352 | 8.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.026 | 0.026 | 0.026 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 97.115 | 97.115 | 97.115 | 20.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 97.114 | 97.114 | 97.114 | 20.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.329 | 1.329 | 1.329 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 90.025 | 90.025 | 90.025 | 18.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.658 | 1.658 | 1.658 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 23.240 | 23.240 | 23.240 | 4.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.360 | 24.360 | 24.360 | 5.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.075 | 1.075 | 1.075 | 0.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 39.691 | 39.691 | 39.691 | 8.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 5.760 | 5.760 | 5.760 | 1.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 103.349 | 103.349 | 103.349 | 21.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 11.370 | 11.370 | 11.370 | 2.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 91.978 | 91.978 | 91.978 | 19.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.644 | 1.644 | 1.644 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 21.960 | 21.960 | 21.960 | 4.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.586 | 24.586 | 24.586 | 5.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.245 | 1.245 | 1.245 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 42.543 | 42.543 | 42.543 | 8.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 9.906 | 9.906 | 9.906 | 2.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.194 | 0.194 | 0.194 | 0.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.312 | 4.312 | 4.312 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 5.400 | 5.400 | 5.400 | 1.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.283 | 1.283 | 1.283 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.595 | 0.595 | 0.595 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.521 | 3.521 | 3.521 | 0.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 4.299 | 4.299 | 4.299 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.298 | 4.298 | 4.298 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 3 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 509.389 | 509.389 | 509.389 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.047 | 0.047 | 0.047 | 0.01% |
| &nbsp;&nbsp;`open` | 509.341 | 509.341 | 509.341 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 509.340 | 509.340 | 509.340 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 125.075 | 125.075 | 125.075 | 24.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 125.074 | 125.074 | 125.074 | 24.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.451 | 0.451 | 0.451 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 124.623 | 124.623 | 124.623 | 24.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.008 | 0.008 | 0.008 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 28.009 | 28.009 | 28.009 | 5.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 7.553 | 7.553 | 7.553 | 1.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.498 | 1.498 | 1.498 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.500 | 3.500 | 3.500 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.555 | 2.555 | 2.555 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 9.400 | 9.400 | 9.400 | 1.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 79.654 | 79.654 | 79.654 | 15.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 368.527 | 368.527 | 368.527 | 72.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 368.526 | 368.526 | 368.526 | 72.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 9.215 | 9.215 | 9.215 | 1.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.471 | 2.471 | 2.471 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 2.851 | 2.851 | 2.851 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.480 | 2.480 | 2.480 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.413 | 1.413 | 1.413 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 3.052 | 3.052 | 3.052 | 0.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 131.784 | 131.784 | 131.784 | 25.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 40.970 | 40.970 | 40.970 | 8.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 90.813 | 90.813 | 90.813 | 17.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.647 | 1.647 | 1.647 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.738 | 22.738 | 22.738 | 4.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.678 | 24.678 | 24.678 | 4.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.141 | 1.141 | 1.141 | 0.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 40.610 | 40.610 | 40.610 | 7.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.028 | 0.028 | 0.028 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 123.995 | 123.995 | 123.995 | 24.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 123.994 | 123.994 | 123.994 | 24.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.514 | 1.514 | 1.514 | 0.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 116.043 | 116.043 | 116.043 | 22.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.197 | 2.197 | 2.197 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 28.622 | 28.622 | 28.622 | 5.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 28.044 | 28.044 | 28.044 | 5.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.356 | 1.356 | 1.356 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 55.824 | 55.824 | 55.824 | 10.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 6.436 | 6.436 | 6.436 | 1.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 100.448 | 100.448 | 100.448 | 19.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 11.534 | 11.534 | 11.534 | 2.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 88.914 | 88.914 | 88.914 | 17.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.620 | 1.620 | 1.620 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.181 | 22.181 | 22.181 | 4.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.234 | 24.234 | 24.234 | 4.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.089 | 1.089 | 1.089 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 39.791 | 39.791 | 39.791 | 7.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 10.969 | 10.969 | 10.969 | 2.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.184 | 0.184 | 0.184 | 0.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 5.049 | 5.049 | 5.049 | 0.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 5.736 | 5.736 | 5.736 | 1.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.304 | 1.304 | 1.304 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.624 | 0.624 | 0.624 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.808 | 3.808 | 3.808 | 0.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 4.764 | 4.764 | 4.764 | 0.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.763 | 4.763 | 4.763 | 0.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open combined (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 482.672 | 509.389 | 509.389 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.047 | 0.048 | 0.048 | 0.01% |
| &nbsp;&nbsp;`open` | 482.625 | 509.341 | 509.341 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 482.624 | 509.340 | 509.340 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 124.095 | 125.075 | 125.075 | 25.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 124.095 | 125.074 | 125.074 | 25.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.451 | 2.074 | 2.074 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 123.775 | 124.623 | 124.623 | 25.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.008 | 0.008 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 28.042 | 28.183 | 28.183 | 5.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 7.553 | 7.755 | 7.755 | 1.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.498 | 1.571 | 1.571 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.500 | 3.662 | 3.662 | 0.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.522 | 2.555 | 2.555 | 0.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 9.313 | 9.400 | 9.400 | 1.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 78.658 | 79.654 | 79.654 | 16.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 346.114 | 368.527 | 368.527 | 71.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 346.113 | 368.526 | 368.526 | 71.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 8.888 | 9.215 | 9.215 | 1.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.402 | 2.471 | 2.471 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 2.766 | 2.851 | 2.851 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.416 | 2.480 | 2.480 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.327 | 1.413 | 1.413 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 2.907 | 3.052 | 3.052 | 0.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 130.961 | 131.784 | 131.784 | 27.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 38.555 | 40.970 | 40.970 | 7.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 90.813 | 92.643 | 92.643 | 18.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.651 | 1.691 | 1.691 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.738 | 23.024 | 23.024 | 4.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.678 | 24.761 | 24.761 | 5.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.141 | 1.144 | 1.144 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 40.610 | 42.062 | 42.062 | 8.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.028 | 0.028 | 0.028 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 105.134 | 123.995 | 123.995 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 105.132 | 123.994 | 123.994 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.357 | 1.514 | 1.514 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 98.048 | 116.043 | 116.043 | 20.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.707 | 2.197 | 2.197 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 23.240 | 28.622 | 28.622 | 4.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 26.406 | 28.044 | 28.044 | 5.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.323 | 1.356 | 1.356 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 45.498 | 55.824 | 55.824 | 9.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 5.760 | 6.436 | 6.436 | 1.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 100.448 | 103.349 | 103.349 | 20.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 11.370 | 11.534 | 11.534 | 2.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 88.914 | 91.978 | 91.978 | 18.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.620 | 1.644 | 1.644 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.093 | 22.181 | 22.181 | 4.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.234 | 24.586 | 24.586 | 5.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.089 | 1.245 | 1.245 | 0.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 39.791 | 42.543 | 42.543 | 8.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 9.906 | 10.969 | 10.969 | 2.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.184 | 0.194 | 0.194 | 0.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.312 | 5.049 | 5.049 | 0.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 5.400 | 5.736 | 5.736 | 1.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.283 | 1.304 | 1.304 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.595 | 0.624 | 0.624 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.521 | 3.808 | 3.808 | 0.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 4.358 | 4.764 | 4.764 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.357 | 4.763 | 4.763 | 0.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### edit (n=30; Document clones per step: 4)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 92.884 | 101.856 | 117.186 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 92.884 | 101.855 | 117.185 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 92.795 | 101.792 | 116.917 | 99.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 92.792 | 101.789 | 116.914 | 99.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.738 | 2.012 | 2.087 | 0.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 91.125 | 101.047 | 115.661 | 98.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.008 | 1.300 | 1.706 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 0.011 | 0.013 | 0.013 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 19.227 | 21.370 | 22.503 | 20.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.994 | 8.448 | 8.594 | 6.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 13.522 | 14.469 | 14.709 | 14.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 56.409 | 63.683 | 63.971 | 60.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 0.010 | 0.014 | 0.735 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 10.878 | 12.854 | 35.768 | 11.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.006 | 0.007 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 0.001 | 0.001 | 22.593 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 2.736 | 3.220 | 3.563 | 2.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 5.495 | 6.600 | 7.384 | 5.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 2.580 | 2.939 | 2.993 | 2.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 2.572 | 2.929 | 2.985 | 2.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 0.006 | 0.008 | 0.008 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 0.013 | 0.021 | 0.023 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.006 | 0.006 | 0.007 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 0.004 | 0.005 | 0.008 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 4.283 | 5.432 | 5.799 | 4.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.076 | 0.099 | 0.268 | 0.08% |

### edit zero-row window (n=30; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 4.092 | 5.122 | 6.233 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;`handle` | 4.092 | 5.121 | 6.233 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 4.091 | 5.121 | 6.232 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 4.090 | 5.119 | 6.230 | 99.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.018 | 0.024 | 0.025 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.010 | 0.012 | 0.128 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 4.062 | 5.087 | 6.193 | 99.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 1 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 240.635 | 241.137 | 241.137 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 240.635 | 241.137 | 241.137 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 240.634 | 241.136 | 241.136 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.331 | 0.373 | 0.373 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 240.303 | 240.763 | 240.763 | 99.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.393 | 4.720 | 4.720 | 1.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 235.910 | 236.043 | 236.043 | 98.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.272 | 3.467 | 3.467 | 1.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 1.177 | 1.426 | 1.426 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 6.197 | 7.541 | 7.541 | 2.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.040 | 3.662 | 3.662 | 1.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.157 | 3.879 | 3.879 | 1.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 35.122 | 35.606 | 35.606 | 14.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 19.908 | 20.764 | 20.764 | 8.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 14.842 | 15.214 | 15.214 | 6.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 88.588 | 91.237 | 91.237 | 36.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 35.926 | 36.115 | 36.115 | 14.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 1.530 | 1.697 | 1.697 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.529 | 1.696 | 1.696 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 53.039 | 53.758 | 53.758 | 22.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 21.495 | 21.740 | 21.740 | 8.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 7.990 | 8.689 | 8.689 | 3.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 6.795 | 7.037 | 7.037 | 2.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 16.534 | 16.626 | 16.626 | 6.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 14.393 | 14.426 | 14.426 | 5.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 2.139 | 2.197 | 2.197 | 0.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 4.570 | 4.723 | 4.723 | 1.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.017 | 0.018 | 0.018 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 2.243 | 2.288 | 2.288 | 0.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 3.377 | 4.084 | 4.084 | 1.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.001 | 0.001 | 0.001 | 0.00% |

### clear 1 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 4.101 | 4.375 | 4.375 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.01% |
| &nbsp;&nbsp;`handle` | 4.100 | 4.375 | 4.375 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 4.100 | 4.374 | 4.374 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 4.099 | 4.373 | 4.373 | 99.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.023 | 0.026 | 0.026 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.113 | 0.371 | 0.371 | 2.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.970 | 3.975 | 3.975 | 96.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 7 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1059.475 | 1121.764 | 1121.764 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 1059.475 | 1121.764 | 1121.764 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 1059.474 | 1121.762 | 1121.762 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.662 | 1.672 | 1.672 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 1057.812 | 1120.113 | 1120.113 | 99.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 26.953 | 30.372 | 30.372 | 2.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 1032.255 | 1089.741 | 1089.741 | 97.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 23.282 | 23.465 | 23.465 | 2.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 5.481 | 5.706 | 5.706 | 0.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 5.943 | 6.401 | 6.401 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.985 | 2.164 | 2.164 | 0.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.958 | 4.238 | 4.238 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 198.171 | 199.691 | 199.691 | 18.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 168.442 | 170.975 | 170.975 | 15.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 28.787 | 32.010 | 32.010 | 2.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 420.494 | 475.426 | 475.426 | 39.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 36.968 | 39.313 | 39.313 | 3.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 2.006 | 2.146 | 2.146 | 0.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 2.004 | 2.144 | 2.144 | 0.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 236.227 | 237.274 | 237.274 | 22.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.013 | 0.013 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 23.183 | 23.708 | 23.708 | 2.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 17.379 | 17.509 | 17.509 | 1.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 7.295 | 7.504 | 7.504 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 188.236 | 188.678 | 188.678 | 17.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 167.354 | 167.526 | 167.526 | 15.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 20.705 | 22.658 | 22.658 | 1.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 83.787 | 90.836 | 90.836 | 7.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.021 | 0.023 | 0.023 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 9.689 | 11.621 | 11.621 | 0.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 5.234 | 5.255 | 5.255 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.001 | 0.001 | 0.001 | 0.00% |

### clear 7 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 9.910 | 10.524 | 10.524 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;`handle` | 9.909 | 10.523 | 10.523 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 9.908 | 10.523 | 10.523 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 9.907 | 10.521 | 10.521 | 99.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.318 | 0.321 | 0.321 | 3.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 5.998 | 6.572 | 6.572 | 60.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.591 | 3.628 | 3.628 | 36.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### save export_project_v3 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 567.689 | 598.634 | 598.634 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;`export_project_v3` | 567.688 | 598.632 | 598.632 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 69.508 | 72.702 | 72.702 | 12.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 488.513 | 517.844 | 517.844 | 86.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 123.794 | 126.551 | 126.551 | 21.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 123.794 | 126.551 | 126.551 | 21.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.267 | 0.313 | 0.313 | 0.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 123.481 | 126.283 | 126.283 | 21.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.008 | 0.008 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 28.900 | 29.688 | 29.688 | 5.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 8.741 | 10.078 | 10.078 | 1.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.829 | 2.261 | 2.261 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 4.229 | 4.599 | 4.599 | 0.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.683 | 3.218 | 3.218 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 9.431 | 10.084 | 10.084 | 1.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 76.427 | 76.919 | 76.919 | 13.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 352.114 | 376.085 | 376.085 | 62.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 352.113 | 376.085 | 376.085 | 62.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.010 | 0.016 | 0.016 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 8.828 | 9.603 | 9.603 | 1.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.381 | 2.401 | 2.401 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 2.702 | 3.186 | 3.186 | 0.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.394 | 2.511 | 2.511 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.351 | 1.506 | 1.506 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 3.109 | 3.435 | 3.435 | 0.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 136.081 | 136.395 | 136.395 | 23.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 40.751 | 41.108 | 41.108 | 7.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 94.973 | 95.644 | 95.644 | 16.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.938 | 2.984 | 2.984 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 24.044 | 24.510 | 24.510 | 4.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 25.535 | 25.571 | 25.571 | 4.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.215 | 1.252 | 1.252 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 41.972 | 42.875 | 42.875 | 7.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.028 | 0.041 | 0.041 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 98.315 | 99.670 | 99.670 | 17.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 98.313 | 99.668 | 99.668 | 17.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.366 | 1.381 | 1.381 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 91.061 | 92.549 | 92.549 | 16.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.670 | 1.691 | 1.691 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 24.088 | 24.473 | 24.473 | 4.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 24.208 | 24.495 | 24.495 | 4.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.112 | 1.167 | 1.167 | 0.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 40.119 | 40.743 | 40.743 | 7.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 5.753 | 5.871 | 5.871 | 1.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 110.115 | 128.280 | 128.280 | 19.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 13.638 | 15.717 | 15.717 | 2.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 96.477 | 112.562 | 112.562 | 16.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.254 | 3.679 | 3.679 | 0.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.866 | 24.422 | 24.422 | 4.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 25.882 | 31.213 | 31.213 | 4.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.203 | 1.894 | 1.894 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 44.272 | 51.355 | 51.355 | 7.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 10.196 | 10.331 | 10.331 | 1.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.183 | 0.183 | 0.183 | 0.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 4.632 | 4.810 | 4.810 | 0.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 5.378 | 5.381 | 5.381 | 0.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.297 | 1.305 | 1.305 | 0.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.539 | 0.559 | 0.559 | 0.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.525 | 3.545 | 3.545 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 4.540 | 5.224 | 5.224 | 0.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.539 | 5.223 | 5.223 | 0.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 4.048 | 4.712 | 4.712 | 0.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.038 | 4.954 | 4.954 | 0.71% |

### reopen (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 502.890 | 510.018 | 510.018 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.050 | 0.057 | 0.057 | 0.01% |
| &nbsp;&nbsp;`open` | 502.840 | 509.961 | 509.961 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 502.839 | 509.960 | 509.960 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 125.678 | 126.205 | 126.205 | 24.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 125.677 | 126.205 | 126.205 | 24.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.380 | 0.393 | 0.393 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 125.305 | 125.812 | 125.812 | 24.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.009 | 0.009 | 0.009 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 29.436 | 29.653 | 29.653 | 5.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 8.596 | 8.781 | 8.781 | 1.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.749 | 1.774 | 1.774 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.949 | 4.062 | 4.062 | 0.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.873 | 2.971 | 2.971 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 9.474 | 9.931 | 9.931 | 1.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 77.437 | 77.791 | 77.791 | 15.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 360.712 | 366.200 | 366.200 | 71.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 360.711 | 366.200 | 366.200 | 71.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 10.399 | 12.037 | 12.037 | 2.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.566 | 2.631 | 2.631 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 3.313 | 3.750 | 3.750 | 0.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 2.804 | 3.313 | 3.313 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 1.792 | 2.343 | 2.343 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 2.999 | 3.187 | 3.187 | 0.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 135.438 | 135.794 | 135.794 | 26.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 41.025 | 42.026 | 42.026 | 8.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 93.412 | 94.769 | 94.769 | 18.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.882 | 1.915 | 1.915 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 23.259 | 23.475 | 23.475 | 4.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 25.917 | 26.329 | 26.329 | 5.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.259 | 1.266 | 1.266 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 41.309 | 42.236 | 42.236 | 8.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.029 | 0.029 | 0.029 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 108.402 | 110.065 | 110.065 | 21.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 108.400 | 110.064 | 110.064 | 21.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.484 | 1.565 | 1.565 | 0.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 100.237 | 101.504 | 101.504 | 19.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.790 | 2.001 | 2.001 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 25.225 | 25.927 | 25.927 | 5.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 25.292 | 28.915 | 28.915 | 5.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.272 | 1.430 | 1.430 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 44.556 | 45.956 | 45.956 | 8.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 6.680 | 6.995 | 6.995 | 1.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 105.188 | 107.014 | 107.014 | 20.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 12.877 | 13.013 | 13.013 | 2.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 93.105 | 94.001 | 94.001 | 18.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.863 | 1.875 | 1.875 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 22.780 | 23.183 | 23.183 | 4.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 25.239 | 25.872 | 25.872 | 5.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.246 | 1.281 | 1.281 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 41.802 | 43.304 | 43.304 | 8.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 11.934 | 12.285 | 12.285 | 2.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.183 | 0.193 | 0.193 | 0.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 5.745 | 6.291 | 6.291 | 1.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 5.812 | 5.996 | 5.996 | 1.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.326 | 1.328 | 1.328 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.581 | 0.635 | 0.635 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 3.905 | 4.033 | 4.033 | 0.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 5.790 | 5.987 | 5.987 | 1.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 5.789 | 5.986 | 5.986 | 1.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

Actual counts: `809_COUNTS rows=5000 opens=3 edits=30 saves=3 reduced=false`. Clear counts: 3 for each kind, as in the tables.

## Native 7,500 rows

### open 1 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 872.352 | 872.352 | 872.352 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.114 | 0.114 | 0.114 | 0.01% |
| &nbsp;&nbsp;`open` | 872.238 | 872.238 | 872.238 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 872.237 | 872.237 | 872.237 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 201.164 | 201.164 | 201.164 | 23.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 201.164 | 201.164 | 201.164 | 23.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.964 | 3.964 | 3.964 | 0.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 197.200 | 197.200 | 197.200 | 22.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.011 | 0.011 | 0.011 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 47.102 | 47.102 | 47.102 | 5.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 17.089 | 17.089 | 17.089 | 1.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.019 | 4.019 | 4.019 | 0.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 7.033 | 7.033 | 7.033 | 0.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 6.037 | 6.037 | 6.037 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 18.050 | 18.050 | 18.050 | 2.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 114.947 | 114.947 | 114.947 | 13.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 644.238 | 644.238 | 644.238 | 73.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 644.238 | 644.238 | 644.238 | 73.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 16.353 | 16.353 | 16.353 | 1.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.057 | 4.057 | 4.057 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.484 | 5.484 | 5.484 | 0.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.206 | 4.206 | 4.206 | 0.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 2.606 | 2.606 | 2.606 | 0.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 5.612 | 5.612 | 5.612 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 241.575 | 241.575 | 241.575 | 27.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 66.760 | 66.760 | 66.760 | 7.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 174.815 | 174.815 | 174.815 | 20.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.676 | 3.676 | 3.676 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 41.982 | 41.982 | 41.982 | 4.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 47.932 | 47.932 | 47.932 | 5.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.208 | 2.208 | 2.208 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 79.017 | 79.017 | 79.017 | 9.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.029 | 0.029 | 0.029 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 181.570 | 181.570 | 181.570 | 20.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 181.568 | 181.568 | 181.568 | 20.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.372 | 2.372 | 2.372 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 169.073 | 169.073 | 169.073 | 19.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.137 | 3.137 | 3.137 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 43.499 | 43.499 | 43.499 | 4.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 45.200 | 45.200 | 45.200 | 5.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.137 | 2.137 | 2.137 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 75.099 | 75.099 | 75.099 | 8.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 10.124 | 10.124 | 10.124 | 1.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 199.093 | 199.093 | 199.093 | 22.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 22.394 | 22.394 | 22.394 | 2.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 176.699 | 176.699 | 176.699 | 20.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.519 | 3.519 | 3.519 | 0.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 40.973 | 40.973 | 40.973 | 4.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 46.399 | 46.399 | 46.399 | 5.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.112 | 2.112 | 2.112 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 83.696 | 83.696 | 83.696 | 9.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 18.761 | 18.761 | 18.761 | 2.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.187 | 0.187 | 0.187 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 9.270 | 9.270 | 9.270 | 1.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 9.305 | 9.305 | 9.305 | 1.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.371 | 1.371 | 1.371 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 1.301 | 1.301 | 1.301 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 6.633 | 6.633 | 6.633 | 0.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 8.065 | 8.065 | 8.065 | 0.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.064 | 8.064 | 8.064 | 0.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 2 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 831.050 | 831.050 | 831.050 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.086 | 0.086 | 0.086 | 0.01% |
| &nbsp;&nbsp;`open` | 830.964 | 830.964 | 830.964 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 830.963 | 830.963 | 830.963 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 194.299 | 194.299 | 194.299 | 23.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 194.299 | 194.299 | 194.299 | 23.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.456 | 0.456 | 0.456 | 0.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 193.843 | 193.843 | 193.843 | 23.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.010 | 0.010 | 0.010 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 46.602 | 46.602 | 46.602 | 5.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 16.553 | 16.553 | 16.553 | 1.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.982 | 3.982 | 3.982 | 0.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 6.915 | 6.915 | 6.915 | 0.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 5.656 | 5.656 | 5.656 | 0.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 17.095 | 17.095 | 17.095 | 2.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 113.583 | 113.583 | 113.583 | 13.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 608.317 | 608.317 | 608.317 | 73.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 608.317 | 608.317 | 608.317 | 73.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 17.555 | 17.555 | 17.555 | 2.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.950 | 3.950 | 3.950 | 0.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 6.139 | 6.139 | 6.139 | 0.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.442 | 4.442 | 4.442 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 3.024 | 3.024 | 3.024 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 5.434 | 5.434 | 5.434 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 230.888 | 230.888 | 230.888 | 27.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 67.899 | 67.899 | 67.899 | 8.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 162.989 | 162.989 | 162.989 | 19.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.481 | 4.481 | 4.481 | 0.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 39.072 | 39.072 | 39.072 | 4.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 45.644 | 45.644 | 45.644 | 5.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.092 | 2.092 | 2.092 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 71.701 | 71.701 | 71.701 | 8.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.031 | 0.031 | 0.031 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 177.722 | 177.722 | 177.722 | 21.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 177.720 | 177.720 | 177.720 | 21.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.057 | 2.057 | 2.057 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 165.466 | 165.466 | 165.466 | 19.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.856 | 2.856 | 2.856 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 44.439 | 44.439 | 44.439 | 5.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 42.166 | 42.166 | 42.166 | 5.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.035 | 2.035 | 2.035 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 73.969 | 73.969 | 73.969 | 8.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 10.198 | 10.198 | 10.198 | 1.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 176.682 | 176.682 | 176.682 | 21.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 22.405 | 22.405 | 22.405 | 2.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 154.276 | 154.276 | 154.276 | 18.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.201 | 4.201 | 4.201 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 36.910 | 36.910 | 36.910 | 4.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 43.807 | 43.807 | 43.807 | 5.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.288 | 2.288 | 2.288 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 67.070 | 67.070 | 67.070 | 8.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 20.083 | 20.083 | 20.083 | 2.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.180 | 0.180 | 0.180 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 9.717 | 9.717 | 9.717 | 1.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 10.186 | 10.186 | 10.186 | 1.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.331 | 1.331 | 1.331 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 1.528 | 1.528 | 1.528 | 0.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 7.327 | 7.327 | 7.327 | 0.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 8.257 | 8.257 | 8.257 | 0.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.256 | 8.256 | 8.256 | 0.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 3 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 836.312 | 836.312 | 836.312 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.053 | 0.053 | 0.053 | 0.01% |
| &nbsp;&nbsp;`open` | 836.259 | 836.259 | 836.259 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 836.258 | 836.258 | 836.258 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 199.032 | 199.032 | 199.032 | 23.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 199.031 | 199.031 | 199.031 | 23.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.661 | 0.661 | 0.661 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 198.370 | 198.370 | 198.370 | 23.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.012 | 0.012 | 0.012 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 48.029 | 48.029 | 48.029 | 5.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 15.093 | 15.093 | 15.093 | 1.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.701 | 3.701 | 3.701 | 0.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 6.539 | 6.539 | 6.539 | 0.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.854 | 4.854 | 4.854 | 0.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 15.131 | 15.131 | 15.131 | 1.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 120.105 | 120.105 | 120.105 | 14.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 605.446 | 605.446 | 605.446 | 72.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 605.446 | 605.446 | 605.446 | 72.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 18.712 | 18.712 | 18.712 | 2.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.708 | 4.708 | 4.708 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.959 | 5.959 | 5.959 | 0.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.902 | 4.902 | 4.902 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 3.144 | 3.144 | 3.144 | 0.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 5.750 | 5.750 | 5.750 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 231.749 | 231.749 | 231.749 | 27.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 68.125 | 68.125 | 68.125 | 8.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 163.623 | 163.623 | 163.623 | 19.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.636 | 3.636 | 3.636 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 39.391 | 39.391 | 39.391 | 4.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 46.743 | 46.743 | 46.743 | 5.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.216 | 2.216 | 2.216 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 71.636 | 71.636 | 71.636 | 8.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.033 | 0.033 | 0.033 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 170.858 | 170.858 | 170.858 | 20.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 170.856 | 170.856 | 170.856 | 20.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.269 | 2.269 | 2.269 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 158.619 | 158.619 | 158.619 | 18.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.853 | 2.853 | 2.853 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 42.593 | 42.593 | 42.593 | 5.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 41.932 | 41.932 | 41.932 | 5.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.042 | 2.042 | 2.042 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 69.198 | 69.198 | 69.198 | 8.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 9.968 | 9.968 | 9.968 | 1.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 178.338 | 178.338 | 178.338 | 21.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 20.785 | 20.785 | 20.785 | 2.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 157.554 | 157.554 | 157.554 | 18.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.953 | 2.953 | 2.953 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 41.287 | 41.287 | 41.287 | 4.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 42.132 | 42.132 | 42.132 | 5.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.245 | 2.245 | 2.245 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 68.936 | 68.936 | 68.936 | 8.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 23.339 | 23.339 | 23.339 | 2.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.193 | 0.193 | 0.193 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 11.250 | 11.250 | 11.250 | 1.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 11.896 | 11.896 | 11.896 | 1.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.415 | 1.415 | 1.415 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 2.564 | 2.564 | 2.564 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 7.917 | 7.917 | 7.917 | 0.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 8.435 | 8.435 | 8.435 | 1.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.433 | 8.433 | 8.433 | 1.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open combined (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 836.312 | 872.352 | 872.352 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.086 | 0.114 | 0.114 | 0.01% |
| &nbsp;&nbsp;`open` | 836.259 | 872.238 | 872.238 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 836.258 | 872.237 | 872.237 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 199.032 | 201.164 | 201.164 | 23.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 199.031 | 201.164 | 201.164 | 23.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.661 | 3.964 | 3.964 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 197.200 | 198.370 | 198.370 | 23.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.011 | 0.012 | 0.012 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 47.102 | 48.029 | 48.029 | 5.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 16.553 | 17.089 | 17.089 | 1.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.982 | 4.019 | 4.019 | 0.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 6.915 | 7.033 | 7.033 | 0.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 5.656 | 6.037 | 6.037 | 0.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 17.095 | 18.050 | 18.050 | 2.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 114.947 | 120.105 | 120.105 | 13.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 608.317 | 644.238 | 644.238 | 72.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 608.317 | 644.238 | 644.238 | 72.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 17.555 | 18.712 | 18.712 | 2.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.057 | 4.708 | 4.708 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.959 | 6.139 | 6.139 | 0.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.442 | 4.902 | 4.902 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 3.024 | 3.144 | 3.144 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 5.612 | 5.750 | 5.750 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 231.749 | 241.575 | 241.575 | 27.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 67.899 | 68.125 | 68.125 | 8.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 163.623 | 174.815 | 174.815 | 19.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.676 | 4.481 | 4.481 | 0.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 39.391 | 41.982 | 41.982 | 4.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 46.743 | 47.932 | 47.932 | 5.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.208 | 2.216 | 2.216 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 71.701 | 79.017 | 79.017 | 8.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.031 | 0.033 | 0.033 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 177.722 | 181.570 | 181.570 | 21.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 177.720 | 181.568 | 181.568 | 21.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.269 | 2.372 | 2.372 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 165.466 | 169.073 | 169.073 | 19.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.856 | 3.137 | 3.137 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 43.499 | 44.439 | 44.439 | 5.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 42.166 | 45.200 | 45.200 | 5.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.042 | 2.137 | 2.137 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 73.969 | 75.099 | 75.099 | 8.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 10.124 | 10.198 | 10.198 | 1.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 178.338 | 199.093 | 199.093 | 21.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 22.394 | 22.405 | 22.405 | 2.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 157.554 | 176.699 | 176.699 | 18.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.519 | 4.201 | 4.201 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 40.973 | 41.287 | 41.287 | 4.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 43.807 | 46.399 | 46.399 | 5.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.245 | 2.288 | 2.288 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 68.936 | 83.696 | 83.696 | 8.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 20.083 | 23.339 | 23.339 | 2.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.187 | 0.193 | 0.193 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 9.717 | 11.250 | 11.250 | 1.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 10.186 | 11.896 | 11.896 | 1.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.371 | 1.415 | 1.415 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 1.528 | 2.564 | 2.564 | 0.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 7.327 | 7.917 | 7.917 | 0.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 8.257 | 8.435 | 8.435 | 0.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.256 | 8.433 | 8.433 | 0.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### edit (n=30; Document clones per step: 4)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 152.088 | 171.087 | 190.785 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 152.087 | 171.087 | 190.785 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 152.029 | 170.966 | 190.291 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 152.026 | 170.963 | 190.286 | 99.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.241 | 3.936 | 3.996 | 0.82% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 150.611 | 169.482 | 186.929 | 99.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.009 | 2.657 | 2.808 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 0.002 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 0.011 | 0.013 | 0.013 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 31.819 | 35.298 | 37.235 | 20.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 9.634 | 12.070 | 13.954 | 6.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 21.710 | 24.193 | 26.786 | 14.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 91.834 | 101.425 | 102.779 | 60.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 0.011 | 0.014 | 2.280 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 18.488 | 22.401 | 54.373 | 12.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.007 | 0.010 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 0.001 | 0.001 | 33.654 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 4.710 | 5.743 | 5.776 | 3.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 9.063 | 11.079 | 11.509 | 5.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 4.728 | 5.279 | 5.541 | 3.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 4.718 | 5.269 | 5.530 | 3.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 0.008 | 0.009 | 0.009 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 0.014 | 0.015 | 0.020 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.006 | 0.008 | 0.009 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 0.004 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 7.399 | 9.048 | 9.516 | 4.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.090 | 0.469 | 0.493 | 0.06% |

### edit zero-row window (n=30; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 6.838 | 8.227 | 9.003 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 6.838 | 8.226 | 9.003 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 6.837 | 8.225 | 9.002 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.010 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 6.836 | 8.223 | 9.000 | 99.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.020 | 0.027 | 0.030 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.012 | 0.030 | 0.732 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 6.805 | 8.185 | 8.959 | 99.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 1 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 394.979 | 396.875 | 396.875 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 394.979 | 396.874 | 396.874 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 394.977 | 396.873 | 396.873 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.556 | 0.560 | 0.560 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 394.421 | 396.313 | 396.313 | 99.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.109 | 7.616 | 7.616 | 1.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 386.805 | 389.336 | 389.336 | 97.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.822 | 6.475 | 6.475 | 1.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 1.991 | 2.105 | 2.105 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 10.672 | 11.223 | 11.223 | 2.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.168 | 5.447 | 5.447 | 1.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.504 | 5.776 | 5.776 | 1.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 58.598 | 59.310 | 59.310 | 14.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 34.921 | 36.091 | 36.091 | 8.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 23.208 | 24.389 | 24.389 | 5.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 147.613 | 149.944 | 149.944 | 37.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 56.864 | 57.527 | 57.527 | 14.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 2.435 | 2.882 | 2.882 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 2.433 | 2.880 | 2.880 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 83.579 | 83.668 | 83.668 | 21.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 33.373 | 33.532 | 33.532 | 8.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 13.210 | 13.267 | 13.267 | 3.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 10.915 | 11.076 | 11.076 | 2.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 26.006 | 26.609 | 26.609 | 6.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 22.632 | 23.228 | 23.228 | 5.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 3.371 | 3.378 | 3.378 | 0.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 7.612 | 7.829 | 7.829 | 1.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.022 | 0.022 | 0.022 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 3.965 | 4.140 | 4.140 | 1.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 5.932 | 6.115 | 6.115 | 1.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.001 | 0.001 | 0.001 | 0.00% |

### clear 1 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 7.113 | 7.672 | 7.672 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;`handle` | 7.113 | 7.672 | 7.672 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 7.113 | 7.671 | 7.671 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 7.111 | 7.670 | 7.670 | 99.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.025 | 0.032 | 0.032 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.319 | 0.411 | 0.411 | 4.48% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 6.676 | 7.319 | 7.319 | 93.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 7 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1655.152 | 1666.758 | 1666.758 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 1655.151 | 1666.757 | 1666.757 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 1655.149 | 1666.756 | 1666.756 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.457 | 2.468 | 2.468 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 1652.692 | 1664.288 | 1664.288 | 99.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 38.616 | 39.410 | 39.410 | 2.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 1614.076 | 1624.878 | 1624.878 | 97.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 37.250 | 39.488 | 39.488 | 2.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 8.537 | 10.040 | 10.040 | 0.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 11.128 | 14.066 | 14.066 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.609 | 6.172 | 6.172 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 6.648 | 7.894 | 7.894 | 0.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 308.881 | 330.126 | 330.126 | 18.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 265.879 | 279.009 | 279.009 | 16.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 43.002 | 51.118 | 51.118 | 2.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 636.330 | 683.040 | 683.040 | 38.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 57.784 | 67.511 | 67.511 | 3.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 2.649 | 3.082 | 3.082 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 2.648 | 3.080 | 3.080 | 0.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 361.018 | 364.229 | 364.229 | 21.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 34.603 | 35.769 | 35.769 | 2.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 27.572 | 28.501 | 28.501 | 1.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 11.477 | 11.533 | 11.533 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 287.314 | 288.477 | 288.477 | 17.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 256.611 | 257.122 | 257.122 | 15.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 31.150 | 31.351 | 31.351 | 1.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 124.757 | 137.368 | 137.368 | 7.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.020 | 0.024 | 0.024 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 14.053 | 16.889 | 16.889 | 0.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 7.600 | 7.711 | 7.711 | 0.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.001 | 0.001 | 0.00% |

### clear 7 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 14.876 | 15.690 | 15.690 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 14.875 | 15.689 | 15.689 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 14.874 | 15.689 | 15.689 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 14.873 | 15.687 | 15.687 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.091 | 1.479 | 1.479 | 7.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 7.028 | 8.806 | 8.806 | 47.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 5.790 | 6.475 | 6.475 | 38.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### save export_project_v3 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 911.135 | 916.866 | 916.866 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;`export_project_v3` | 911.133 | 916.864 | 916.864 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 109.838 | 113.595 | 113.595 | 12.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 789.517 | 789.772 | 789.772 | 86.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 186.741 | 187.192 | 187.192 | 20.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 186.740 | 187.192 | 187.192 | 20.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.441 | 0.531 | 0.531 | 0.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 186.404 | 186.660 | 186.660 | 20.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.010 | 0.011 | 0.011 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 44.954 | 45.450 | 45.450 | 4.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 12.811 | 12.817 | 12.817 | 1.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.845 | 2.921 | 2.921 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.841 | 5.931 | 5.931 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.040 | 4.048 | 4.048 | 0.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 14.223 | 14.275 | 14.275 | 1.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 114.714 | 114.731 | 114.731 | 12.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 575.438 | 576.872 | 576.872 | 63.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 575.437 | 576.871 | 576.871 | 63.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.010 | 0.017 | 0.017 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 14.960 | 15.671 | 15.671 | 1.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.764 | 3.834 | 3.834 | 0.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.064 | 5.127 | 5.127 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 3.815 | 3.990 | 3.990 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 2.317 | 2.719 | 2.719 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 5.283 | 5.387 | 5.387 | 0.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 213.603 | 216.248 | 216.248 | 23.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 62.933 | 63.162 | 63.162 | 6.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 150.670 | 153.087 | 153.087 | 16.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.962 | 3.074 | 3.074 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 38.035 | 38.311 | 38.311 | 4.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 42.028 | 42.741 | 42.741 | 4.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.955 | 2.034 | 2.034 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 65.420 | 67.203 | 67.203 | 7.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.029 | 0.030 | 0.030 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 169.681 | 172.599 | 172.599 | 18.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 169.679 | 172.597 | 172.597 | 18.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.384 | 2.486 | 2.486 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 157.544 | 159.529 | 159.529 | 17.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.183 | 3.502 | 3.502 | 0.35% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 39.033 | 39.284 | 39.284 | 4.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 45.800 | 46.341 | 46.341 | 5.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.158 | 2.197 | 2.197 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 67.866 | 68.501 | 68.501 | 7.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 9.751 | 10.582 | 10.582 | 1.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 170.464 | 173.196 | 173.196 | 18.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 20.761 | 20.973 | 20.973 | 2.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 150.467 | 152.436 | 152.436 | 16.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.922 | 2.997 | 2.997 | 0.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 36.963 | 37.612 | 37.612 | 4.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 41.622 | 43.214 | 43.214 | 4.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 1.956 | 2.038 | 2.038 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 66.432 | 67.224 | 67.224 | 7.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 18.323 | 19.519 | 19.519 | 2.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.185 | 0.186 | 0.186 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 8.870 | 9.587 | 9.587 | 0.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 9.360 | 9.747 | 9.747 | 1.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.356 | 1.365 | 1.365 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 1.380 | 1.673 | 1.673 | 0.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 6.623 | 6.710 | 6.710 | 0.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 7.577 | 7.618 | 7.618 | 0.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 7.576 | 7.617 | 7.617 | 0.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.934 | 6.473 | 6.473 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 5.958 | 7.279 | 7.279 | 0.65% |

### reopen (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 818.516 | 820.167 | 820.167 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.052 | 0.057 | 0.057 | 0.01% |
| &nbsp;&nbsp;`open` | 818.464 | 820.110 | 820.110 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 818.463 | 820.109 | 820.109 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 192.651 | 193.726 | 193.726 | 23.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 192.651 | 193.726 | 193.726 | 23.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.633 | 0.710 | 0.710 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 192.040 | 193.016 | 193.016 | 23.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.012 | 0.014 | 0.014 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 45.638 | 46.000 | 46.000 | 5.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 15.005 | 15.308 | 15.308 | 1.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.187 | 3.343 | 3.343 | 0.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 6.466 | 6.819 | 6.819 | 0.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 5.147 | 5.353 | 5.353 | 0.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 15.257 | 15.523 | 15.523 | 1.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 116.465 | 116.476 | 116.476 | 14.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 596.565 | 597.711 | 597.711 | 72.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 596.565 | 597.710 | 597.710 | 72.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 16.868 | 17.140 | 17.140 | 2.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.234 | 4.302 | 4.302 | 0.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 5.504 | 5.567 | 5.567 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 4.525 | 4.526 | 4.526 | 0.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 2.747 | 2.940 | 2.940 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 5.450 | 5.557 | 5.557 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 221.123 | 226.119 | 226.119 | 27.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 65.104 | 65.629 | 65.629 | 7.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 156.019 | 161.695 | 161.695 | 19.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.969 | 2.975 | 2.975 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 38.573 | 38.582 | 38.582 | 4.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 43.272 | 43.995 | 43.995 | 5.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.060 | 2.113 | 2.113 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 68.429 | 74.753 | 74.753 | 8.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.031 | 0.032 | 0.032 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 171.884 | 177.904 | 177.904 | 21.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 171.882 | 177.903 | 177.903 | 21.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.394 | 2.521 | 2.521 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 158.714 | 165.202 | 165.202 | 19.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.876 | 3.910 | 3.910 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 39.497 | 39.591 | 39.591 | 4.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 44.030 | 45.471 | 45.471 | 5.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.332 | 2.475 | 2.475 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 69.913 | 73.848 | 73.848 | 8.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 10.356 | 10.647 | 10.647 | 1.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 176.056 | 176.377 | 176.377 | 21.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 21.567 | 22.494 | 22.494 | 2.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 154.489 | 154.927 | 154.927 | 18.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 2.967 | 3.042 | 3.042 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 37.081 | 38.510 | 38.510 | 4.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 43.386 | 44.755 | 44.755 | 5.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.102 | 2.146 | 2.146 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 67.614 | 68.412 | 68.412 | 8.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 20.386 | 20.688 | 20.688 | 2.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.187 | 0.189 | 0.189 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 10.077 | 10.083 | 10.083 | 1.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 10.373 | 10.415 | 10.415 | 1.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.368 | 1.376 | 1.376 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 2.079 | 2.262 | 2.262 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 6.930 | 6.969 | 6.969 | 0.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 8.856 | 9.012 | 9.012 | 1.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.855 | 9.011 | 9.011 | 1.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

Actual counts: `809_COUNTS rows=7500 opens=3 edits=30 saves=3 reduced=false`. Clear counts: 3 for each kind, as in the tables.

## Native 10,000 rows

### open 1 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1192.843 | 1192.843 | 1192.843 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.057 | 0.057 | 0.057 | 0.00% |
| &nbsp;&nbsp;`open` | 1192.787 | 1192.787 | 1192.787 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 1192.786 | 1192.786 | 1192.786 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.009 | 0.009 | 0.009 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 271.538 | 271.538 | 271.538 | 22.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 271.537 | 271.537 | 271.537 | 22.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.652 | 5.652 | 5.652 | 0.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 265.885 | 265.885 | 265.885 | 22.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.013 | 0.013 | 0.013 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 66.294 | 66.294 | 66.294 | 5.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 22.109 | 22.109 | 22.109 | 1.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.297 | 5.297 | 5.297 | 0.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.054 | 9.054 | 9.054 | 0.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 7.757 | 7.757 | 7.757 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 24.327 | 24.327 | 24.327 | 2.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 153.143 | 153.143 | 153.143 | 12.84% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 880.314 | 880.314 | 880.314 | 73.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 880.313 | 880.313 | 880.313 | 73.80% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 25.419 | 25.419 | 25.419 | 2.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 6.125 | 6.125 | 6.125 | 0.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.226 | 8.226 | 8.226 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 6.652 | 6.652 | 6.652 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 4.415 | 4.415 | 4.415 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 8.872 | 8.872 | 8.872 | 0.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 321.618 | 321.618 | 321.618 | 26.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 91.902 | 91.902 | 91.902 | 7.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 229.716 | 229.716 | 229.716 | 19.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.082 | 7.082 | 7.082 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 55.943 | 55.943 | 55.943 | 4.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.884 | 64.884 | 64.884 | 5.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.946 | 2.946 | 2.946 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 98.860 | 98.860 | 98.860 | 8.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.035 | 0.035 | 0.035 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 259.762 | 259.762 | 259.762 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 259.759 | 259.759 | 259.759 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.595 | 3.595 | 3.595 | 0.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 241.089 | 241.089 | 241.089 | 20.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.149 | 5.149 | 5.149 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 63.959 | 63.959 | 63.959 | 5.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 67.380 | 67.380 | 67.380 | 5.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.932 | 2.932 | 2.932 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 101.669 | 101.669 | 101.669 | 8.52% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 15.075 | 15.075 | 15.075 | 1.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 264.601 | 264.601 | 264.601 | 22.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 36.881 | 36.881 | 36.881 | 3.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 227.720 | 227.720 | 227.720 | 19.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.974 | 5.974 | 5.974 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 53.737 | 53.737 | 53.737 | 4.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 65.959 | 65.959 | 65.959 | 5.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.843 | 2.843 | 2.843 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 99.206 | 99.206 | 99.206 | 8.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 30.313 | 30.313 | 30.313 | 2.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.208 | 0.208 | 0.208 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 15.363 | 15.363 | 15.363 | 1.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 14.742 | 14.742 | 14.742 | 1.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.406 | 1.406 | 1.406 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 3.047 | 3.047 | 3.047 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.289 | 10.289 | 10.289 | 0.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 10.610 | 10.610 | 10.610 | 0.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 10.609 | 10.609 | 10.609 | 0.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 2 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1229.197 | 1229.197 | 1229.197 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.143 | 0.143 | 0.143 | 0.01% |
| &nbsp;&nbsp;`open` | 1229.054 | 1229.054 | 1229.054 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 1229.053 | 1229.053 | 1229.053 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 273.098 | 273.098 | 273.098 | 22.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 273.098 | 273.098 | 273.098 | 22.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.694 | 0.694 | 0.694 | 0.06% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 272.404 | 272.404 | 272.404 | 22.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.012 | 0.012 | 0.012 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 65.281 | 65.281 | 65.281 | 5.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 25.899 | 25.899 | 25.899 | 2.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 6.136 | 6.136 | 6.136 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.768 | 9.768 | 9.768 | 0.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 9.995 | 9.995 | 9.995 | 0.81% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 25.511 | 25.511 | 25.511 | 2.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 155.701 | 155.701 | 155.701 | 12.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 912.114 | 912.114 | 912.114 | 74.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 912.114 | 912.114 | 912.114 | 74.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 29.163 | 29.163 | 29.163 | 2.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.998 | 7.998 | 7.998 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.067 | 8.067 | 8.067 | 0.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.042 | 8.042 | 8.042 | 0.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 5.057 | 5.057 | 5.057 | 0.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 8.103 | 8.103 | 8.103 | 0.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 331.465 | 331.465 | 331.465 | 26.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 92.570 | 92.570 | 92.570 | 7.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 238.895 | 238.895 | 238.895 | 19.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.774 | 4.774 | 4.774 | 0.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 55.222 | 55.222 | 55.222 | 4.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.838 | 64.838 | 64.838 | 5.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 4.621 | 4.621 | 4.621 | 0.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 109.440 | 109.440 | 109.440 | 8.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.045 | 0.045 | 0.045 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 264.807 | 264.807 | 264.807 | 21.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 264.804 | 264.804 | 264.804 | 21.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.437 | 3.437 | 3.437 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 246.213 | 246.213 | 246.213 | 20.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.106 | 5.106 | 5.106 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 60.532 | 60.532 | 60.532 | 4.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 65.285 | 65.285 | 65.285 | 5.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.864 | 2.864 | 2.864 | 0.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 112.426 | 112.426 | 112.426 | 9.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 15.154 | 15.154 | 15.154 | 1.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 278.523 | 278.523 | 278.523 | 22.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 33.163 | 33.163 | 33.163 | 2.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 245.361 | 245.361 | 245.361 | 19.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.811 | 7.811 | 7.811 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 56.066 | 56.066 | 56.066 | 4.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 68.641 | 68.641 | 68.641 | 5.58% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.985 | 2.985 | 2.985 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 109.859 | 109.859 | 109.859 | 8.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 32.672 | 32.672 | 32.672 | 2.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.208 | 0.208 | 0.208 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 16.114 | 16.114 | 16.114 | 1.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 16.350 | 16.350 | 16.350 | 1.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.501 | 1.501 | 1.501 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 4.202 | 4.202 | 4.202 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.647 | 10.647 | 10.647 | 0.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 11.162 | 11.162 | 11.162 | 0.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 11.161 | 11.161 | 11.161 | 0.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open 3 (n=1; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1191.910 | 1191.910 | 1191.910 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.131 | 0.131 | 0.131 | 0.01% |
| &nbsp;&nbsp;`open` | 1191.779 | 1191.779 | 1191.779 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 1191.778 | 1191.778 | 1191.778 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 266.721 | 266.721 | 266.721 | 22.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 266.721 | 266.721 | 266.721 | 22.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.007 | 1.007 | 1.007 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 265.714 | 265.714 | 265.714 | 22.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.012 | 0.012 | 0.012 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 64.057 | 64.057 | 64.057 | 5.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 22.402 | 22.402 | 22.402 | 1.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.163 | 5.163 | 5.163 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.301 | 9.301 | 9.301 | 0.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 7.938 | 7.938 | 7.938 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 23.178 | 23.178 | 23.178 | 1.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 156.065 | 156.065 | 156.065 | 13.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 881.689 | 881.689 | 881.689 | 73.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 881.688 | 881.688 | 881.688 | 73.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 26.468 | 26.468 | 26.468 | 2.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 6.758 | 6.758 | 6.758 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.293 | 8.293 | 8.293 | 0.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 6.998 | 6.998 | 6.998 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 4.419 | 4.419 | 4.419 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 8.095 | 8.095 | 8.095 | 0.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 342.796 | 342.796 | 342.796 | 28.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 94.328 | 94.328 | 94.328 | 7.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 248.469 | 248.469 | 248.469 | 20.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.070 | 5.070 | 5.070 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 56.961 | 56.961 | 56.961 | 4.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 76.410 | 76.410 | 76.410 | 6.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 3.056 | 3.056 | 3.056 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 106.971 | 106.971 | 106.971 | 8.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.032 | 0.032 | 0.032 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 252.390 | 252.390 | 252.390 | 21.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 252.387 | 252.387 | 252.387 | 21.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.427 | 3.427 | 3.427 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 234.267 | 234.267 | 234.267 | 19.65% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.053 | 5.053 | 5.053 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 57.913 | 57.913 | 57.913 | 4.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.631 | 64.631 | 64.631 | 5.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 3.149 | 3.149 | 3.149 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 103.521 | 103.521 | 103.521 | 8.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 14.694 | 14.694 | 14.694 | 1.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 251.902 | 251.902 | 251.902 | 21.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 32.236 | 32.236 | 32.236 | 2.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 219.666 | 219.666 | 219.666 | 18.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.565 | 4.565 | 4.565 | 0.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 53.269 | 53.269 | 53.269 | 4.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.187 | 64.187 | 64.187 | 5.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.840 | 2.840 | 2.840 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 94.806 | 94.806 | 94.806 | 7.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 30.133 | 30.133 | 30.133 | 2.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.197 | 0.197 | 0.197 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 15.783 | 15.783 | 15.783 | 1.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 14.153 | 14.153 | 14.153 | 1.19% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.414 | 1.414 | 1.414 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 2.493 | 2.493 | 2.493 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.246 | 10.246 | 10.246 | 0.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 13.229 | 13.229 | 13.229 | 1.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 13.227 | 13.227 | 13.227 | 1.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### open combined (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1192.843 | 1229.197 | 1229.197 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.131 | 0.143 | 0.143 | 0.01% |
| &nbsp;&nbsp;`open` | 1192.787 | 1229.054 | 1229.054 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 1192.786 | 1229.053 | 1229.053 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.009 | 0.009 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 271.538 | 273.098 | 273.098 | 22.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 271.537 | 273.098 | 273.098 | 22.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.007 | 5.652 | 5.652 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 265.885 | 272.404 | 272.404 | 22.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.012 | 0.013 | 0.013 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 65.281 | 66.294 | 66.294 | 5.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 22.402 | 25.899 | 25.899 | 1.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.297 | 6.136 | 6.136 | 0.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.301 | 9.768 | 9.768 | 0.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 7.938 | 9.995 | 9.995 | 0.67% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 24.327 | 25.511 | 25.511 | 2.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 155.701 | 156.065 | 156.065 | 13.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 881.689 | 912.114 | 912.114 | 73.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 881.688 | 912.114 | 912.114 | 73.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 26.468 | 29.163 | 29.163 | 2.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 6.758 | 7.998 | 7.998 | 0.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.226 | 8.293 | 8.293 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 6.998 | 8.042 | 8.042 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 4.419 | 5.057 | 5.057 | 0.37% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 8.103 | 8.872 | 8.872 | 0.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 331.465 | 342.796 | 342.796 | 27.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 92.570 | 94.328 | 94.328 | 7.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 238.895 | 248.469 | 248.469 | 20.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.070 | 7.082 | 7.082 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 55.943 | 56.961 | 56.961 | 4.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.884 | 76.410 | 76.410 | 5.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 3.056 | 4.621 | 4.621 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 106.971 | 109.440 | 109.440 | 8.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.035 | 0.045 | 0.045 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 259.762 | 264.807 | 264.807 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 259.759 | 264.804 | 264.804 | 21.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.437 | 3.595 | 3.595 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 241.089 | 246.213 | 246.213 | 20.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.106 | 5.149 | 5.149 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 60.532 | 63.959 | 63.959 | 5.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 65.285 | 67.380 | 67.380 | 5.47% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.932 | 3.149 | 3.149 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 103.521 | 112.426 | 112.426 | 8.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 15.075 | 15.154 | 15.154 | 1.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 264.601 | 278.523 | 278.523 | 22.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 33.163 | 36.881 | 36.881 | 2.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 227.720 | 245.361 | 245.361 | 19.09% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.974 | 7.811 | 7.811 | 0.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 53.737 | 56.066 | 56.066 | 4.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 65.959 | 68.641 | 68.641 | 5.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.843 | 2.985 | 2.985 | 0.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 99.206 | 109.859 | 109.859 | 8.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 30.313 | 32.672 | 32.672 | 2.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.208 | 0.208 | 0.208 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 15.783 | 16.114 | 16.114 | 1.32% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 14.742 | 16.350 | 16.350 | 1.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.414 | 1.501 | 1.501 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 3.047 | 4.202 | 4.202 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.289 | 10.647 | 10.647 | 0.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 11.162 | 13.229 | 13.229 | 0.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 11.161 | 13.227 | 13.227 | 0.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

### edit (n=30; Document clones per step: 4)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 225.216 | 237.261 | 270.897 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 225.215 | 237.260 | 270.897 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 224.999 | 236.213 | 269.678 | 99.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.004 | 0.004 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 224.995 | 236.210 | 269.674 | 99.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.915 | 5.538 | 5.811 | 0.85% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 222.044 | 234.464 | 266.926 | 98.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.009 | 4.626 | 4.757 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 0.002 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 0.012 | 0.014 | 0.014 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 47.056 | 51.572 | 53.000 | 20.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 15.520 | 17.714 | 18.090 | 6.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 31.863 | 34.910 | 34.913 | 14.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 134.656 | 141.003 | 141.829 | 59.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 0.013 | 0.015 | 0.015 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 27.837 | 31.835 | 75.113 | 12.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.005 | 0.006 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 0.001 | 0.001 | 46.528 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 7.120 | 7.818 | 8.397 | 3.16% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 13.843 | 15.990 | 16.055 | 6.15% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 6.836 | 7.314 | 7.442 | 3.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 6.827 | 7.303 | 7.430 | 3.03% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 0.008 | 0.009 | 0.009 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 0.014 | 0.023 | 0.023 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.007 | 0.008 | 0.008 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 0.004 | 0.005 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 11.566 | 13.431 | 13.597 | 5.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.145 | 1.251 | 1.279 | 0.06% |

### edit zero-row window (n=30; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 10.366 | 11.016 | 11.343 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;`handle` | 10.365 | 11.015 | 11.343 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 10.365 | 11.014 | 11.342 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.004 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 10.363 | 11.012 | 11.340 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.021 | 0.025 | 0.029 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.014 | 0.017 | 0.023 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.324 | 10.977 | 11.302 | 99.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 1 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 558.623 | 592.018 | 592.018 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 558.623 | 592.017 | 592.017 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 558.622 | 592.016 | 592.016 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.718 | 0.729 | 0.729 | 0.13% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 557.903 | 591.307 | 591.307 | 99.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 11.123 | 12.958 | 12.958 | 1.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 546.780 | 578.349 | 578.349 | 97.88% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.845 | 8.295 | 8.295 | 1.40% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 3.386 | 3.713 | 3.713 | 0.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 18.449 | 20.824 | 20.824 | 3.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 9.561 | 10.931 | 10.931 | 1.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.888 | 9.893 | 9.893 | 1.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 84.992 | 90.211 | 90.211 | 15.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 52.760 | 54.796 | 54.796 | 9.44% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 33.007 | 35.415 | 35.415 | 5.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 200.626 | 209.756 | 209.756 | 35.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 81.906 | 86.900 | 86.900 | 14.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 3.441 | 3.442 | 3.442 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 3.439 | 3.440 | 3.440 | 0.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 120.465 | 126.128 | 126.128 | 21.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.006 | 0.006 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 46.441 | 47.040 | 47.040 | 8.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 19.622 | 21.241 | 21.241 | 3.51% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 16.395 | 18.207 | 18.207 | 2.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 38.152 | 39.634 | 39.634 | 6.83% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.003 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 33.243 | 34.641 | 34.641 | 5.95% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 4.964 | 4.990 | 4.990 | 0.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 11.311 | 11.555 | 11.555 | 2.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.024 | 0.025 | 0.025 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 6.366 | 6.546 | 6.546 | 1.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 9.371 | 12.059 | 12.059 | 1.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.001 | 0.001 | 0.001 | 0.00% |

### clear 1 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 9.888 | 10.056 | 10.056 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;`handle` | 9.888 | 10.055 | 10.055 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 9.887 | 10.055 | 10.055 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 9.886 | 10.053 | 10.053 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.030 | 0.034 | 0.034 | 0.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 0.251 | 0.416 | 0.416 | 2.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 9.603 | 9.604 | 9.604 | 97.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### clear 7 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 2329.428 | 2400.721 | 2400.721 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;`handle` | 2329.428 | 2400.720 | 2400.720 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 2329.426 | 2400.719 | 2400.719 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.354 | 3.540 | 3.540 | 0.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`edit_cells` | 2326.072 | 2397.372 | 2397.372 | 99.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 53.700 | 54.870 | 54.870 | 2.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_commands` | 2272.372 | 2344.436 | 2344.436 | 97.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 52.259 | 52.573 | 52.573 | 2.24% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`apply_commands` | 12.295 | 12.723 | 12.723 | 0.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 17.682 | 17.742 | 17.742 | 0.76% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.621 | 7.994 | 7.994 | 0.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.748 | 10.061 | 10.061 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`execute` | 440.141 | 453.262 | 453.262 | 18.89% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 376.814 | 389.899 | 389.899 | 16.18% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`publish_if_current` | 63.363 | 63.616 | 63.616 | 2.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle_propose` | 895.021 | 1007.100 | 1007.100 | 38.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`post_publication` | 82.775 | 85.836 | 85.836 | 3.55% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preflight` | 3.854 | 4.015 | 4.015 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 3.852 | 4.013 | 4.013 | 0.17% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`prepare_evaluation` | 520.797 | 526.645 | 526.645 | 22.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.006 | 0.014 | 0.014 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_preflight` | 48.547 | 49.050 | 49.050 | 2.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`projection_changes` | 38.571 | 39.999 | 39.999 | 1.66% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_clone` | 16.280 | 17.072 | 17.072 | 0.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`retained_update` | 417.416 | 422.739 | 422.739 | 17.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.004 | 0.005 | 0.005 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`calculation_update` | 370.720 | 373.223 | 373.223 | 15.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validation_report` | 44.189 | 52.013 | 52.013 | 1.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`preview` | 185.565 | 197.182 | 197.182 | 7.97% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`proposal_setup` | 0.022 | 0.024 | 0.024 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`published_projection` | 21.600 | 21.643 | 21.643 | 0.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`snapshot_clone` | 10.427 | 10.476 | 10.476 | 0.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.001 | 0.001 | 0.001 | 0.00% |

### clear 7 zero-row window (n=3; Document clones per step: 0)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 11.381 | 20.460 | 20.460 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.000 | 0.000 | 0.000 | 0.00% |
| &nbsp;&nbsp;`handle` | 11.381 | 20.460 | 20.460 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;`dispatch` | 11.380 | 20.459 | 20.459 | 99.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 11.378 | 20.457 | 20.457 | 99.98% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.844 | 2.200 | 2.200 | 16.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 1.398 | 9.461 | 9.461 | 12.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 8.137 | 8.796 | 8.796 | 71.50% |
| &nbsp;&nbsp;&nbsp;&nbsp;`response_building` | 0.000 | 0.000 | 0.000 | 0.00% |

### save export_project_v3 (n=3; Document clones per step: 2)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1329.822 | 1343.756 | 1343.756 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;`export_project_v3` | 1329.820 | 1343.754 | 1343.754 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 166.651 | 169.107 | 169.107 | 12.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 1144.775 | 1153.943 | 1153.943 | 86.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 262.954 | 264.001 | 264.001 | 19.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 262.954 | 264.000 | 264.000 | 19.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.675 | 0.741 | 0.741 | 0.05% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 262.331 | 263.259 | 263.259 | 19.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.010 | 0.010 | 0.010 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 62.561 | 62.785 | 62.785 | 4.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 19.472 | 20.229 | 20.229 | 1.46% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.456 | 4.717 | 4.717 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.513 | 8.569 | 8.569 | 0.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 6.503 | 6.942 | 6.942 | 0.49% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 21.185 | 24.177 | 24.177 | 1.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 156.534 | 159.458 | 159.458 | 11.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 847.542 | 850.164 | 850.164 | 63.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.000 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 847.541 | 850.163 | 850.163 | 63.73% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.025 | 0.027 | 0.027 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 22.832 | 25.150 | 25.150 | 1.72% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.779 | 6.102 | 6.102 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 7.392 | 7.499 | 7.499 | 0.56% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 5.938 | 6.578 | 6.578 | 0.45% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 3.751 | 5.311 | 5.311 | 0.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 8.020 | 9.410 | 9.410 | 0.60% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 317.468 | 323.384 | 323.384 | 23.87% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 90.049 | 91.577 | 91.577 | 6.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 227.419 | 231.807 | 231.807 | 17.10% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.531 | 6.409 | 6.409 | 0.34% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 57.368 | 57.887 | 57.887 | 4.31% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 60.768 | 63.444 | 63.444 | 4.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.795 | 2.917 | 2.917 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 100.118 | 103.029 | 103.029 | 7.53% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.030 | 0.032 | 0.032 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 243.020 | 246.443 | 246.443 | 18.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.003 | 0.003 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 243.018 | 246.441 | 246.441 | 18.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.416 | 3.423 | 3.423 | 0.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 225.295 | 228.832 | 228.832 | 16.94% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.161 | 5.818 | 5.818 | 0.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 56.670 | 57.294 | 57.294 | 4.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 62.281 | 66.296 | 66.296 | 4.68% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.828 | 2.831 | 2.831 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 97.825 | 98.355 | 98.355 | 7.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 14.218 | 14.307 | 14.307 | 1.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 248.844 | 253.022 | 253.022 | 18.71% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 30.352 | 32.750 | 32.750 | 2.28% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 217.592 | 222.671 | 222.671 | 16.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.819 | 6.403 | 6.403 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 54.065 | 54.174 | 54.174 | 4.07% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 60.379 | 64.284 | 64.284 | 4.54% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.784 | 2.858 | 2.858 | 0.21% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 96.434 | 96.535 | 96.535 | 7.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 29.200 | 30.505 | 30.505 | 2.20% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.193 | 0.276 | 0.276 | 0.01% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 13.855 | 14.363 | 14.363 | 1.04% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 15.152 | 16.492 | 16.492 | 1.14% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.429 | 1.903 | 1.903 | 0.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 3.112 | 3.298 | 3.298 | 0.23% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.425 | 11.777 | 11.777 | 0.78% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 10.485 | 10.744 | 10.744 | 0.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 10.484 | 10.742 | 10.742 | 0.79% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.112 | 10.414 | 10.414 | 0.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 9.283 | 10.289 | 10.289 | 0.70% |

### reopen (n=3; Document clones per step: 1)

| phase (inclusive) | p50 ms | p95 ms | max ms | share of whole step |
|---|---:|---:|---:|---:|
| `step` | 1201.933 | 1202.131 | 1202.131 | 100.00% |
| &nbsp;&nbsp;`[rest]` | 0.141 | 0.151 | 0.151 | 0.01% |
| &nbsp;&nbsp;`open` | 1201.879 | 1201.980 | 1201.980 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;`admit` | 1201.878 | 1201.979 | 1201.979 | 100.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.008 | 0.012 | 0.012 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`decode_bundle` | 273.705 | 275.086 | 275.086 | 22.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`unframe` | 273.704 | 275.085 | 275.085 | 22.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.925 | 1.068 | 1.068 | 0.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_admit` | 272.779 | 274.017 | 274.017 | 22.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.013 | 0.015 | 0.015 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`canonical_encode_compare` | 64.774 | 65.874 | 65.874 | 5.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`profile_callback` | 23.593 | 24.806 | 24.806 | 1.96% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.200 | 6.094 | 6.094 | 0.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 9.242 | 9.876 | 9.876 | 0.77% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 8.836 | 9.602 | 9.602 | 0.74% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_after_decode` | 23.868 | 24.641 | 24.641 | 1.99% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`storage_decode` | 159.888 | 160.245 | 160.245 | 13.30% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`from_document_working_size` | 881.318 | 881.494 | 881.494 | 73.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.001 | 0.001 | 0.001 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`baseline` | 881.318 | 881.494 | 881.494 | 73.33% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.007 | 0.007 | 0.007 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`capacity_checks` | 27.082 | 27.913 | 27.913 | 2.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 7.084 | 7.233 | 7.233 | 0.59% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`document_capacity_check` | 8.365 | 8.431 | 8.431 | 0.70% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 7.345 | 7.746 | 7.746 | 0.61% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`working_strings_check` | 4.383 | 4.504 | 4.504 | 0.36% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collection_and_static` | 9.019 | 9.542 | 9.542 | 0.75% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`formula_sources` | 325.894 | 329.551 | 329.551 | 27.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 95.094 | 100.163 | 100.163 | 7.91% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 229.645 | 232.880 | 232.880 | 19.11% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.710 | 5.192 | 5.192 | 0.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 55.574 | 60.031 | 60.031 | 4.62% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.673 | 65.799 | 65.799 | 5.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.967 | 3.029 | 3.029 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 101.304 | 101.719 | 101.719 | 8.43% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`lifecycle` | 0.035 | 0.053 | 0.053 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_session_new` | 255.110 | 256.356 | 256.356 | 21.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`resident_rebuild` | 255.108 | 256.354 | 256.354 | 21.22% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 3.469 | 3.947 | 3.947 | 0.29% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 236.025 | 236.426 | 236.426 | 19.64% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 4.924 | 5.836 | 5.836 | 0.41% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 58.919 | 60.359 | 60.359 | 4.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.644 | 65.236 | 65.236 | 5.38% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.987 | 4.792 | 4.792 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 103.058 | 103.147 | 103.147 | 8.57% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_validation` | 15.322 | 16.381 | 16.381 | 1.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`validate` | 263.507 | 264.801 | 264.801 | 21.92% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 35.169 | 36.373 | 36.373 | 2.93% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`full_calculation` | 227.134 | 229.632 | 229.632 | 18.90% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 5.064 | 5.713 | 5.713 | 0.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`collect_nodes_graph` | 55.630 | 56.395 | 56.395 | 4.63% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`evaluate_formulas` | 64.743 | 64.878 | 64.878 | 5.39% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`initial_values` | 2.990 | 3.139 | 3.139 | 0.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`pregraph_cycle_checks` | 99.253 | 100.670 | 100.670 | 8.26% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`opened_window` | 32.303 | 32.782 | 32.782 | 2.69% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.194 | 0.194 | 0.194 | 0.02% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`bootstrap_projection` | 17.124 | 17.467 | 17.467 | 1.42% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`query_table_window` | 14.986 | 15.122 | 15.122 | 1.25% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 1.424 | 1.497 | 1.497 | 0.12% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`view_order` | 3.238 | 3.253 | 3.253 | 0.27% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`window_column_totals` | 10.309 | 10.492 | 10.492 | 0.86% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes` | 12.985 | 14.160 | 14.160 | 1.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`[rest]` | 0.002 | 0.002 | 0.002 | 0.00% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`saved_bytes_check` | 12.983 | 14.158 | 14.158 | 1.08% |
| &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;`set_presentation` | 0.002 | 0.002 | 0.002 | 0.00% |

Actual counts: `809_COUNTS rows=10000 opens=3 edits=30 saves=3 reduced=false`. Clear counts: 3 for each kind, as in the tables.

## Native growth: 5,000 → 10,000 rows

| step | p50 growth |
|---|---:|
| clear 1 | 2.321× |
| clear 1 zero-row window | 2.411× |
| clear 7 | 2.199× |
| clear 7 zero-row window | 1.148× |
| edit | 2.425× |
| edit zero-row window | 2.533× |
| open | 2.471× |
| reopen | 2.390× |
| save export_project_v3 | 2.343× |

## WASM real ABI

Open n=3; edits n=30; clears n=3 at each size. No phases or clone counters.

| rows | step | n | p50 ms | p95 ms | max ms |
|---:|---|---:|---:|---:|---:|
| 5000 | open | 3 | 545.780 | 738.279 | 738.279 |
| 5000 | edit | 30 | 62.997 | 73.755 | 149.683 |
| 5000 | edit zero-row window | 30 | 3.704 | 4.246 | 5.012 |
| 5000 | edit + window | 30 | 66.724 | 77.879 | 154.695 |
| 5000 | clear 1 | 3 | 275.348 | 358.981 | 358.981 |
| 5000 | clear 1 zero-row window | 3 | 5.017 | 5.050 | 5.050 |
| 5000 | clear 1 + window | 3 | 280.364 | 364.032 | 364.032 |
| 5000 | clear 7 | 3 | 1192.792 | 1986.524 | 1986.524 |
| 5000 | clear 7 zero-row window | 3 | 3.951 | 4.901 | 4.901 |
| 5000 | clear 7 + window | 3 | 1196.742 | 1990.342 | 1990.342 |
| 10000 | open | 3 | 1275.718 | 1286.069 | 1286.069 |
| 10000 | edit | 30 | 175.407 | 181.648 | 247.536 |
| 10000 | edit zero-row window | 30 | 9.898 | 10.277 | 10.369 |
| 10000 | edit + window | 30 | 185.245 | 191.514 | 257.813 |
| 10000 | clear 1 | 3 | 670.616 | 672.680 | 672.680 |
| 10000 | clear 1 zero-row window | 3 | 12.142 | 13.011 | 13.011 |
| 10000 | clear 1 + window | 3 | 683.627 | 684.823 | 684.823 |
| 10000 | clear 7 | 3 | 2721.149 | 3980.268 | 3980.268 |
| 10000 | clear 7 zero-row window | 3 | 9.196 | 9.374 | 9.374 |
| 10000 | clear 7 + window | 3 | 2730.522 | 3989.442 | 3989.442 |


### WASM growth: 5,000 → 10,000 rows

| step | p50 growth |
|---|---:|
| open | 2.337× |
| edit | 2.784× |
| edit zero-row window | 2.672× |
| edit + window | 2.776× |
| clear 1 | 2.436× |
| clear 1 zero-row window | 2.420× |
| clear 1 + window | 2.438× |
| clear 7 | 2.281× |
| clear 7 zero-row window | 2.328× |
| clear 7 + window | 2.282× |

## Run and result

```sh
job-run --id 809-measure-r2 -- remote-test -- 'cargo test --release --manifest-path packages/browser-client/runtime/Cargo.toml --locked --features capacity-bench capacity_809_formula_table -- --ignored --test-threads=1 --nocapture && pnpm --dir packages/browser-client install --frozen-lockfile && pnpm --dir packages/browser-client build && node packages/browser-client/runtime/tests/capacity_809_wire.mjs packages/browser-client/.build/designer_runtime.wasm'
```

Native bench result:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 310 filtered out; finished in 80.20s
```

The subsequent frozen pnpm install, WASM build, and ABI measurements all completed with exit 0. Last remote result line:

```text
remote-test: success after 1185s; step log /Users/tachikoma/.local/state/remote-test/tachiko-work-20261114-171506-54968.log, whole run /Users/tachikoma/.local/state/remote-test/tachiko-work-20261114-171506-54968.full.log
```

The first run stopped at compilation on a misplaced storage timer (`E0425`); `748ee11f` corrected it before this successful measurement. No host cargo builds, product fixes, or capacity-gate changes were made.

