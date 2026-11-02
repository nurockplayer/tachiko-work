# Capacity baseline — Stage A (2026-10-26)

This note records the first measured capacity baseline for the 1.0 growth plan
(`.claude/briefs/capacity-plan-2026-10-26.md`, task A-01). It is engineering
evidence, not a specification, a limit change, or a release claim: no admission
limit was raised by the change set that produced it.

Scope of this baseline:

- sizes: 1,000 / 10,000 / 100,000 grid cells at ten columns (the Stage A band:
  10k rows × 10 columns = 100k grid cells, of which ~91% are populated, the rest
  deliberately blank);
- layers: workspace-engine resident publication, the runtime table projection,
  and v3 persistence;
- question: does the cost of one cell edit grow with total document size, and
  does undo/redo history retain whole `Document` copies?

## Environment

| item | value |
| --- | --- |
| machine | Apple Silicon Mac (macOS 27, `darwin`), container-native run |
| container | OrbStack, image `tachiko-work-rust:1.99.0`, `orb-test --memory 1g --cpus 2` |
| build | `cargo test --release --features capacity-bench`, Rust 1.99.0 |
| memory | system allocator; peak RSS from Linux procfs (`/proc/self/clear_refs` + `VmHWM`/`VmRSS`), no allocator shim |
| date | 2026-10-26 (JST) |

These numbers are **native in OrbStack**, not WebAssembly in a browser. WASM and
browser numbers are still to be measured (Plan Stage A gate); a native run only
establishes the shape of the curve and the call-count behavior, not the client
budget.

## Fixtures

`packages/browser-client/runtime/src/capacity_fixtures.rs` builds documents
directly through `Document`, bypassing the 128-row admission profile, so the
same generator can produce every size. It is deterministic: one seeded
`SplitMix64` stream fills cells in row-major order, so identical arguments always
produce an identical document.

Realistic value mix per column (`column % 3`): number, short text, date; roughly
one cell in ten is blank (column `c0` is always populated so formulas can
reference it). A formula variant adds a shared `rate` driver cell in row 0, 99
same-row `rate × c0` formula cells, and one balanced column-total formula over
them — 100 dependent formula cells for one `rate` edit, which is the plan's
"≤100 dependents" scenario. The engine reports 101 formula evaluations per
`rate` edit.

## Operations measured

| operation | layer | what the timer covers |
| --- | --- | --- |
| `session build` | workspace-engine | `ResidentWorkspaceSession::new` derived-state rebuild |
| `scalar edit + publish` | workspace-engine | `export_snapshot` → mutate one cell → `publish_if_current` (equality check, invalidation scan, install) |
| `rate edit, 100 dependent formulas` | workspace-engine | same, plus recomputation of 100 dependent formula nodes |
| `snapshot export` | workspace-engine | `export_snapshot` alone |
| `full table projection build` | runtime | the projection build over the whole schema, through the test-only unbounded entry point (the product entry point refuses these sizes) |
| `table projection JSON` | runtime | projection serialization to wire JSON |
| `v3 encode` / `v3 decode` | storage | `encode_roproj_v3` / `decode_roproj_v3` |
| `v3 bundle encode` / `v3 bundle decode` | runtime | zip bundle around the v3 files; decode only where the admission profile still accepts the size |
| `runtime EditScalar (admitted)` | runtime | full request path at 64 × 5, the largest fixture the current admission profile still accepts |

Median of 3–5 iterations per op; the peak column is the largest resident-memory
delta across iterations. It is measured entirely through safe Linux procfs:
`/proc/self/clear_refs` (`5`) resets the high-water mark `VmHWM` before each
operation, and the delta is `VmHWM` after minus `VmRSS` before. These numbers are
**RSS — 4 KiB page granularity, allocator retention included — not live heap**,
so they read higher and coarser than the bytes a program asked for. On non-Linux
targets the column says `unavailable` instead of failing. An earlier draft used
a test-only counting `GlobalAlloc`; it was removed because the repository
ownership policy in `AGENTS.md` forbids adding a global-allocator shim to the
runtime crate for a measurement harness without explicit Issue scope and Guarded
review.

Instrumentation counters (`capacity-instrumentation` / `capacity-bench`
features) are inert by default: without the feature the hooks compile to empty
inline `const fn` bodies, which an optimized build should drop entirely (this is
by construction, not yet verified by comparing emitted WASM). Enabling
`capacity-bench` for a `wasm32` build is a compile error. With the feature they count
`Document` clones, entities/fields inspected during publication invalidation,
structural equality checks, and serialized JSON bytes.

## Results

Grid cells per fixture, and populated cells in brackets: 1,000 (920), 10,000
(9,119), 100,000 (91,038). The blank tenth is deliberate fixture realism, so the
"cells" in this note are grid slots and the counters below are the exact
measurements.

Latency, median of 3–5 iterations (milliseconds). The numbers below are from
`2026-10-26 06:18 JST` in `orb-test-tachiko-work-12727`, with the system
allocator and peak RSS read from procfs. A re-run at `2026-10-26 06:48 JST`
after a review-only change to the failure path reproduced the counters and the
large peaks (100k projection build 88.7 MiB, 111.898 ms).

| operation | 1k cells | 10k cells | 100k cells |
| --- | ---: | ---: | ---: |
| session build | 0.232 | 2.687 | 25.373 |
| scalar edit + publish | 0.387 | 3.951 | 47.015 |
| snapshot export | 0.045 | 0.439 | 5.048 |
| rate edit, 100 dependent formulas | 0.948 | 4.498 | 50.091 |
| full table projection build | 1.047 | 9.711 | 128.888 |
| table projection JSON | 0.175 | 1.461 | 15.698 |
| v3 encode | 0.457 | 2.577 | 26.051 |
| v3 bundle encode | 0.003 | 0.020 | 0.143 |
| v3 decode | 0.901 | 7.147 | 85.016 |
| v3 bundle decode | 1.080 | refused | refused |
| runtime `EditScalar` (admitted, 64 × 5 = 320 cells) | 2.814 | — | — |

Work counters for one scalar edit + publish — these do not depend on timing
noise:

| counter | 1k cells | 10k cells | 100k cells |
| --- | ---: | ---: | ---: |
| full `Document` clones | 2 | 2 | 2 |
| entities inspected by invalidation | 100 | 1,000 | 10,000 |
| fields inspected by invalidation | 1,850 | 18,248 | 182,086 |
| structural equality checks | 1 | 1 | 1 |
| formula nodes recomputed (rate edit) | 101 | 101 | 101 |

Bytes:

| measurement | 1k cells | 10k cells | 100k cells |
| --- | ---: | ---: | ---: |
| fixture document RSS increase | 475,136 | 0 (reused) | 6,275,072 |
| table projection JSON | 177,602 | 1,754,558 | 17,511,498 |
| v3 tree bytes | 45,090 | 427,582 | 4,249,423 |
| v3 bundle bytes | 45,513 | 428,005 | 4,249,846 |
| projection build peak RSS delta | 0.6 MiB | 8.4 MiB | 88.8 MiB |

The fixture RSS row is not a document-size measurement: the benchmark is one
long-lived process, so glibc can serve a new fixture from pages freed by an
earlier, larger one — hence the 10k reading of 0. The deterministic size
proxies are the projection JSON, v3 tree and bundle byte totals.

Runtime request path, the largest fixture the current admission profile accepts
(64 rows × 5 columns = 320 cells): 2.814 ms, **18** full `Document` clones,
64 entities and 591 fields inspected, 1 structural equality check, and 308,697
bytes of JSON serialized — per edit. The runtime re-admits and re-serializes the
whole document on every request. (The runtime-path JSON total moved by a few
tens of bytes between runs — 308,689 / 308,697 observed today — while the
table-projection JSON totals above were identical across runs.)

Admission refusals observed while measuring (not errors — the profile doing its
job): `decode_project_bundle` refused the 1k- and 10k-row fixtures
("a collection exceeds the bounded maximum of 128 entities"; "the project
contains 10000 entities; the bounded maximum is 1024"), so the runtime save/open
path cannot carry Stage A sizes at all today.

Timing varies with host load: across benchmark runs the 100k-cell scalar edit
measured 47.0–92.0 ms and the projection build 106.9–166.9 ms. Call counts and
deterministic byte totals (table projection JSON, v3 tree, v3 bundle) were
identical across runs. Treat latencies as ±2× and use the counters for
conclusions.

## Priority 0 — does one-cell edit cost scale with document size?

**Confirmed.** A one-cell edit costs 0.387 ms at 1k cells, 3.951 ms at 10k and
47.015 ms at 100k — linear in document size (×10.2, then ×11.9 for a tenfold
growth each time, on a noisy shared host). The counters explain the curve
exactly; every step of the publication path walks everything:

1. `export_snapshot()` clones the whole document (`resident_session.rs:216`).
2. `publish_if_current` compares the candidate with the resident document by
   structural equality (`resident_session.rs:623-626`). The benchmark edits the
   last rows, so the ordered `BTreeMap` comparison has to walk the whole
   document; the counter records one equality check per publication either way.
3. `projection_invalidation` scans every entity and every field
   (`collect_entity_projection_changes`, and the schema and diagnostic
   collectors) — 10,000 entities and 182,086 fields inspected at 100k cells.
4. The successful publication returns another full document clone
   (`resident_session.rs:596-655`).

So exactly two document clones, one structural equality check, and one full scan
per edit, independent of how small the edit is. `snapshot export` alone is 5.048 ms at
100k cells and is itself a clone, which is the floor for the current interface.

The runtime layer is the same story plus admission: one admitted `EditScalar` at
320 cells clones the document 18 times, inspects 591 fields, compares once, and
serializes ~308.7 KB of JSON, because every request re-runs whole-document
preflight admission (`from_document_profile(PREFLIGHT)`) and every publication
re-serializes the table projection. If that JSON grows with the table as the
projection measurement suggests, it would be ~17.5 MB per edit at 100k cells —
not measured directly, because the admission profile refuses that size.

Projection memory is the other Stage A risk: building the 100k-cell table
projection adds 88.8 MiB of peak RSS (≈9.1 KB per row; allocator retention
included) while the wire JSON is 17.5 MB. That already consumes ~35% of the
256 MiB Stage A Worker budget in a native build, before any WASM overhead.

Priority 0 therefore stands as written in the plan: the target is touched-field
work plus affected dependencies, not a comparison of every row and field after
every publication.

## Priority 1 — does undo/redo retain whole documents?

**No full `Document` copies, but history memory is not bounded either.**
`HistoryEntry` stores a forward and an inverse `HistoryAction`, and every action
is semantic (`Vec<SemanticCommand>`, a formula source update, or a formula value
restore) — `packages/browser-client/runtime/src/lib.rs:1310-1320`. There is no
`Document` field and no clone on the undo path.

Measurement, 64 admitted scalar edits on the 320-cell fixture: RSS grew by 0
bytes (0 bytes/entry) over the edits and was 4,096 bytes above the pre-edit
level after undoing all 64 steps; the edits took 192.785 ms and the undos
200.903 ms. That reading is inconclusive rather than reassuring: RSS is
page-granular, and this long-lived process had already released 100k-cell
fixtures, so the allocator could serve the fixture and any history growth from
pages it still held. The 4,096-byte step at least shows the undo pass touched
one more page.

The reason to reject whole-document history is the code, not this number:
`HistoryEntry` has no `Document` field and the undo path clones none. An earlier
revision of this note used a counting `GlobalAlloc` and measured 705,241 bytes
retained over the 64 edits (11,019 bytes/entry) and ~8.4 KB per undo on the same
fixture (document live bytes 71,588). That instrument was removed under the
repository ownership policy (`AGENTS.md`); those figures are historical and
cannot be reproduced with the current procfs harness.

The per-edit residue is therefore the semantic command entries plus the patch
lifecycle's retained evidence: `PatchLifecycle` keeps
`proposals: BTreeMap<ProposalId, ProposalRecord>` and
`execution_receipts_v2: Vec<ExecutionReceiptV2>`
(`crates/workspace-engine/src/patch_lifecycle.rs:1088-1091`) and never prunes
them. Each retained `ProposalRecord` carries a `SemanticPatch` plus lifecycle
history, and each v2 receipt carries evidence, formula impacts and a validation
report. Two consequences for the plan's Priority 1 work:

- the 64-step cap (`lib.rs:3716-3718`, oldest entry evicted) bounds the undo
  stack only; a long session accumulates lifecycle evidence without limit,
  regardless of the cap;
- the plan's per-stage history budget (32 / 64 / 128 MiB) can be applied to the
  undo stack, but a real bound needs a retention or accounting policy for
  proposals and receipts as well.

Undo does not multiply workbook memory by document size, which is what the plan
feared; the retained lifecycle evidence grows with the number of operations,
which the RSS harness can no longer quantify.

## Limitations

- Native-in-OrbStack only; WASM + browser still to be measured.
- The runtime admission profile (128 rows, 1,024 entities, 64 KiB projection)
  refuses Stage A sizes outright, so full runtime request-path edits could only
  be measured at 64 × 5. Stage A sizes were measured at the engine and storage
  layers, and the runtime projection through a test-only unbounded build.
- Peak memory is RSS read from procfs, not live heap: 4 KiB page granularity,
  allocator retention included, and — in one long-lived benchmark process — freed
  pages from an earlier fixture can be reused, so a 0 MiB delta does not mean a
  zero-byte allocation. Use it for the shape of large allocations only; use the
  JSON and v3 byte totals for deterministic size comparisons. The instrumentation
  counters are relaxed atomics: use them for shape and call counts.
- The history measurement reads RSS across 64 edits on a small fixture, so it
  does not resolve retained bytes per entry; see Priority 1. The split between
  undo entries and lifecycle evidence is read from the code, not measured
  separately.
- Latencies are indicative only: runs of the same binary on a shared host
  differed by up to 2×. Counts and deterministic byte totals were stable.

## Reproducing

```text
orb-test --image tachiko-work-rust:1.99.0 --memory 1g --cpus 2 -- \
  bash -c 'cargo test --release \
    --manifest-path packages/browser-client/runtime/Cargo.toml \
    --features capacity-bench --lib -- --ignored --test-threads=1 --nocapture'
```

`--lib` keeps the run to the benchmark binary; without it Cargo also runs the
other test targets, whose output pushes the table out of view. The benchmark
must run inside the Linux OrbStack container for memory numbers; on a non-Linux
host the peak column prints `unavailable` and the run still passes.

CI stays fast: the benchmark test is `#[ignore]`d and the counters are
feature-gated.
