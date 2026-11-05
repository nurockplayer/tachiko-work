# NSH feasibility prototype — spike report

Issue #648. Throwaway spike under `prototypes/nsh-spike/`, branch `ds/nsh-spike`.
**Do not merge.** Times are JST (2026-11-06).

## Summary

| Scope | Verdict |
| --- | --- |
| 1. Work runtime client in one module, cargo unit tests | **Works.** 3/3 unit tests pass against the embedded fixture: list, edit → export → reopen, stale revision refused. |
| 2. Screen A — DataTable bound to the opened Work table, edit through the client, save/reopen the exported project | **Works** end-to-end in the headless runner. |
| 3. Screen B — 100,000 × 50 computed grid, frozen header/first column, selection, keyboard nav, in-cell/overlay edit | **Partial.** The 100k × 50 DataTable mounts (3718 mount items, 1174 nodes), but the tab switch to reach it never takes effect headless, so no grid interaction is verified (`Gaps`). |
| 4. `nsh test` headless checks | **Written and run.** Work screen: pass. Grid screen: fails at the tab switch (see `Gaps`). |
| 5. Web-target measurements | **Not run** (`nsh package --target web` left to the lead, see `Lead-run commands`). |

NSH 0.4.0 as shipped can host a Rust app that embeds the Work runtime directly and drives a
virtualized 100k × 50 table. The single blocker found is a tab-visibility problem in the
headless runner, not in DataTable itself.

## What was built

`prototypes/nsh-spike/` — the NSH crate template (`nsh.json`, `src/main.rs`, `App.test.tsx`)
plus:

- **`src/work.rs`** — the Work client, no NSH types. It wraps the public runtime functions
  `open_local_document`, `open_project`, `process_wire_request` and
  `DesignerRuntime::export_project` (the `host_bridge::DesignerHost` façade is
  `#[cfg(not(target_arch = "wasm32"))]`, so calling the functions it wraps keeps the module
  web-compatible). Types: `WorkClient`, `Opened`, `Table`, `Row`, `Cell`, `Column`,
  `Collection`, `EditKind`. It speaks the same private wire protocol as the browser host:
  `query_table` and `edit_scalar` with `expected_revision`. The fixture
  `examples/game-balance/game-balance.ro` is embedded with `include_bytes!`, so the app does
  not depend on the working directory.
- **`src/lib.rs`** — `App` root with two screens:
  - *Work table*: opens the fixture, renders the default collection in a `DataTable`
    (column headers from the runtime projection, row header, cell selection), commits edits
    through `work.rs`, exports the resident revision to `/tmp/nsh-spike-roundtrip.ro`
    (path is an editable input), reopens that file as an exported project, and shows the
    table count / size and Work's 128-rows-per-table limit.
  - *Grid stress*: `DataTable` with `row_count(100_000)`, 50 columns (first one `fixed`),
    `row_height(24)`, windowed `DataTableRows` (40 rows around the visible range) with cell
    text computed on demand (`R<row>C<col>`), frozen row header, cell selection, keyboard
    navigation from the component, double-press to edit in place, a 1×1 element-ref marker
    in the editing cell with an `overlay()` editor anchored to it, and a "Jump to
    R99999C49" button that moves the window, selects and scrolls to the last cell through
    `DataTableApi`.
- **`App.test.tsx`** — two `nsh test` cases driven by app-level test ids (the DataTable hit
  layer is its own element, so presses target the app's own controls, not table cells).

## Measurements

All numbers from `orb-test --local --host` runs on this Mac, `CARGO_TARGET_DIR` on
`/Volumes/KIOXIA/tachiko-nsh-spike-target`.

| What | Command | Result |
| --- | --- | --- |
| Work client unit tests | `cargo test` (via `orb-test --local --host`) | 3/3 pass, test time 0.08 s, build 8 m 43 s |
| Crate check | `nsh check` | crate 7.9 s; first headless-runner build 12 m 17 s |
| Headless suite, cold | `nsh test --trace` (2026-11-06 02:17 JST) | 4 passed, 1 failed — 54 m 57 s (first runner + deps build) |
| Headless suite, warm | `nsh test --trace` (2026-11-06 03:21 JST) | 4 passed, 1 failed — 2 m 39 s; crate build 2 m 05 s, runner build 1 m 36 s |
| Headless suite, warm, with a debug readout added | `nsh test --trace` (2026-11-06 03:28 JST) | 3 passed, 2 failed — 4 m 35 s; both TSX tests fail at their first press (see `Gaps`) |
| Screen A flow (open → edit → save → reopen) | in the warm run | 1.5 s; export 6442 bytes |
| Screen A test result | | pass |
| Grid mount (100k × 50, 40-row window) | press commit after tab switch | 3718 mount items, 1174 nodes, 538 ms commit |
| `--fps` frame measurement | — | not available headless (target-host only) |
| Web target | `nsh package --target web` | not run (see `Lead-run commands`) |

## Gaps

### 1. Press-driven state does not reach the tree reliably in the headless runner (blocking Screen B)

Both screens are mounted; a `style_when(move || tab.get() != 0/1, |s| s.hidden())` binding on
each screen's container is supposed to show one and hide the other. The trace shows:

```
perf event Press on view 13            <- id:tab-grid's view; Events { id: 13, EventMask([1,0]) } is registered
perf commit: 0 ops, 538162µs (list js 18493µs), 3718 mount items, 1174 nodes
✗ grid stress …: no pressable view matches `id:jump-last`;
  on screen: ["Work table", "Grid stress", "Open fixture", "Save", "Reopen",
              "Work capacity: 128 rows per table (Work limit, not NSH)", "no project open", "Commit cell"]
```

- The press is delivered to the right node and the grid's listener is registered.
- After the press the grid's whole subtree is (re)mounted — 3718 items, 1174 nodes — but
  every grid text stays `content: 0x0 @ 0,0`, the Work screen's texts stay the only on-screen
  ones, and `id:jump-last` is not pressable.
- The same failure occurred with the first implementation, a `when(..)` region over the same
  signal (`3917 mount items, 1133 nodes`, Work screen still on screen), so the symptom is not
  specific to `style_when` vs `when`.
- A second run added a `tab N` text bound to that same signal (a text binding, not a style
  binding) and asserted it right after the press. The text still read `tab 0`, and the app
  rebuilt itself from the press (3719 mount items). In that same run the **Work** test also
  failed at its first press — `opened Moonfall: starter balance` never appeared — although it
  passed in the previous run. The press-triggered rebuild leaves signals created in the root
  component's body at their initial values (`no project open`, `tab 0`).
- So this is not a tab or DataTable problem: in the headless runner a press sometimes
  triggers a full root rebuild in which root-owned signals are recreated, and a state write
  from the press handler is lost (or applied in a commit that has not settled when the test
  reads `app.texts()`). Whether it happens varies between runs of the same suite; the only
  code difference between the two runs above was one extra text readout.
- Suspected NSH cause: the runner's press → commit path re-runs/remounts the root component
  (and so re-creates the `signal()` values in its body) instead of applying the platform ops
  of the state change, or reads the tree before the state-change commit lands. Either way it
  is in NSH 0.4.0's headless runner, not in this app.

Attempts made, per the lead's two-attempt budget: (1) replace the `when(..)` region with both
screens mounted plus `style_when(.., |s| s.hidden())` (the pattern `nsh_base::tabs` uses);
(2) add the `tab N` readout described above. Stopped after these, as instructed; the grid
screen's interaction (jump to R99999C49, overlay editor, commit) is unverified headless. The
committed tree is the one from the first run (4/5); the readout experiment was reverted.

### 1b. The native window was not run

Neither `nsh run` nor `cargo run` was started on the native backend (the spike only had the
headless runner time budget). Whether the tab switch and the 100k × 50 grid behave in a real
window is unverified; that is the first thing to check if the runner issue is not the cause.

### 2. Smaller findings

- `DataTableElement` has no `test_id`; ids must go on a wrapping view or on app controls.
- `nsh_base::data_table` is a private module — the DataTable types are only reachable at the
  crate root.
- `input` is not in `nsh_core::prelude`.
- No file dialog component in NSH 0.4.0; the spike uses a path input plus `std::fs` (and the
  web target returns "the web target has no file access").
- Work itself caps a table at 128 rows, so Screen A can only show that limit, not stress it.

## Lead-run commands

```bash
# Web target packaging and size (not run here; first build may take a while):
cd prototypes/nsh-spike
CARGO_TARGET_DIR=/Volumes/KIOXIA/tachiko-nsh-spike-target nsh package --target web

# Optional: re-run the headless suite with the trace dump for the tab-switch gap:
CARGO_TARGET_DIR=/Volumes/KIOXIA/tachiko-nsh-spike-target nsh test --trace

# After the lead-run work, delete the shared spike target dir:
rm -rf /Volumes/KIOXIA/tachiko-nsh-spike-target
```

## Recommendation

The prototype answers the feasibility question for the parts that matter most: the Work
runtime can be embedded in a Rust NSH app without the JS/WASM kit, its private wire protocol
is usable from Rust, and an edit → export → reopen round trip survives. A virtualized
100,000 × 50 computed DataTable mounts and lays out without trouble, which is the scale
question; NSH's DataTable is not the risk.

The risk is NSH's headless runner: press-driven state is not applied reliably (one screen's
visibility never flips, and a later run lost the first press of the other screen too), which
blocked every grid interaction check. Before committing to NSH for Work, spend one
investigation on that runner bug — or check the same app in a native window with `nsh run`,
where the runtime path is different — then re-run `nsh test`. The app and its tests are
already written; only the switch and the runner need to work. Until then, treat the grid
screen as "mounts at scale, interaction unproven".
