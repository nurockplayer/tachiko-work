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
| container | OrbStack, image `tachiko-work-rust:1.99.0`, `orb-test --memory 1g --cpus 1` |
| build | `cargo test --release --features capacity-bench`, Rust 1.99.0 |
| allocator | system allocator wrapped by a counting `GlobalAlloc` in the bench binary only |
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

Median of 3–5 iterations per op; peak column is the maximum peak-allocation
delta across iterations (counting allocator; bytes requested by `alloc`,
`alloc_zeroed` and `realloc`).

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

Latency, median of 3–5 iterations (milliseconds). The final run was
`2026-10-26 05:44 JST` in `orb-test-tachiko-work-70443`, with the counting
allocator delegating `realloc`/`alloc_zeroed` to the system allocator.

| operation | 1k cells | 10k cells | 100k cells |
| --- | ---: | ---: | ---: |
| session build | 0.326 | 4.134 | 40.344 |
| scalar edit + publish | 0.447 | 6.451 | 55.485 |
| snapshot export | 0.062 | 0.643 | 12.619 |
| rate edit, 100 dependent formulas | 1.069 | 4.931 | 50.642 |
| full table projection build | 0.915 | 11.590 | 120.856 |
| table projection JSON | 0.183 | 1.706 | 14.907 |
| v3 encode | 0.558 | 3.927 | 32.192 |
| v3 bundle encode | 0.003 | 0.027 | 0.141 |
| v3 decode | 0.900 | 11.057 | 85.480 |
| v3 bundle decode | 1.104 | refused | refused |
| runtime `EditScalar` (admitted, 64 × 5 = 320 cells) | 3.029 | — | — |

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
| fixture document live bytes | 113,890 | 1,105,604 | 11,023,800 |
| bytes per populated cell | 124 | 121 | 121 |
| table projection JSON | 177,602 | 1,754,558 | 17,511,498 |
| v3 tree bytes | 45,090 | 427,582 | 4,249,423 |
| v3 bundle bytes | 45,513 | 428,005 | 4,249,846 |
| projection build peak delta | 0 MiB | 10 MiB | 100 MiB |

Runtime request path, the largest fixture the current admission profile accepts
(64 rows × 5 columns = 320 cells): 3.029 ms, **18** full `Document` clones,
64 entities and 591 fields inspected, 1 structural equality check, and 308,709
bytes of JSON serialized — per edit. The runtime re-admits and re-serializes the
whole document on every request.

Admission refusals observed while measuring (not errors — the profile doing its
job): `decode_project_bundle` refused the 1k- and 10k-row fixtures
("a collection exceeds the bounded maximum of 128 entities"; "the project
contains 10000 entities; the bounded maximum is 1024"), so the runtime save/open
path cannot carry Stage A sizes at all today.

Timing varies with host load: three runs of the same benchmark measured the
100k-cell scalar edit at 92.0 ms, 47.2 ms and 55.5 ms, and the projection build
at 166.9 ms, 120.4 ms and 120.9 ms. Call counts and byte counts were identical
across runs. Treat latencies as ±2× and use the counters for conclusions.

## Priority 0 — does one-cell edit cost scale with document size?

**Confirmed.** A one-cell edit costs 0.447 ms at 1k cells, 6.451 ms at 10k and
55.485 ms at 100k — linear in document size (×14.4, then ×8.6 for a tenfold
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
per edit, independent of how small the edit is. `snapshot export` alone is 12.619 ms at
100k cells and is itself a clone, which is the floor for the current interface.

The runtime layer is the same story plus admission: one admitted `EditScalar` at
320 cells clones the document 18 times, inspects 591 fields, compares once, and
serializes 308,709 bytes of JSON, because every request re-runs whole-document
preflight admission (`from_document_profile(PREFLIGHT)`) and every publication
re-serializes the table projection. If that JSON grows with the table as the
projection measurement suggests, it would be ~17.5 MB per edit at 100k cells —
not measured directly, because the admission profile refuses that size.

Projection memory is the other Stage A risk: building the 100k-cell table
projection peaks 100 MiB above the resident document (10 KB per row) while the
wire JSON is 17.5 MB. That already consumes 40% of the 256 MiB Stage A Worker
budget in a native build, before any WASM overhead.

Priority 0 therefore stands as written in the plan: the target is touched-field
work plus affected dependencies, not a comparison of every row and field after
every publication.

## Priority 1 — does undo/redo retain whole documents?

**No full `Document` copies, but history memory is not bounded either.**
`HistoryEntry` stores a forward and an inverse `HistoryAction`, and every action
is semantic (`Vec<SemanticCommand>`, a formula source update, or a formula value
restore) — `packages/browser-client/runtime/src/lib.rs:1310-1320`. There is no
`Document` field and no clone on the undo path.

Measurement, 64 admitted scalar edits on the 320-cell fixture (document live
bytes 71,588): retained memory grew by 705,241 bytes, 11,019 bytes per edit. A
full document copy per step would cost at least 71,588 bytes per edit, so the
history entries are clearly not documents. Undoing all 64 steps took 229.122 ms
and left 1,245,582 bytes retained — that is 540,341 bytes more than after the
edits, i.e. ~8.4 KB per undo.

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
feared; it does grow linearly with the number of operations.

## Limitations

- Native-in-OrbStack only; WASM + browser still to be measured.
- The runtime admission profile (128 rows, 1,024 entities, 64 KiB projection)
  refuses Stage A sizes outright, so full runtime request-path edits could only
  be measured at 64 × 5. Stage A sizes were measured at the engine and storage
  layers, and the runtime projection through a test-only unbounded build.
- The counting allocator reports requested bytes, not RSS, and the counters are
  relaxed atomics: use them for shape and call counts, not for accounting.
- The history measurement counts every live byte the runtime retains across 64
  edits, so it attributes nothing to one specific structure; the split between
  undo entries and lifecycle evidence is read from the code, not measured
  separately.
- Latencies are indicative only: three runs of the same binary on a shared host
  differed by up to 2×. Counts and byte totals were stable.

## Reproducing

```text
orb-test --image tachiko-work-rust:1.99.0 --memory 1g --cpus 1 -- \
  bash -c 'cargo test --release \
    --manifest-path packages/browser-client/runtime/Cargo.toml \
    --features capacity-bench --lib -- --ignored --test-threads=1 --nocapture'
```

`--lib` keeps the run to the benchmark binary; without it Cargo also runs the
other test targets, whose output pushes the table out of view.

CI stays fast: the benchmark test is `#[ignore]`d and the counters are
feature-gated.
