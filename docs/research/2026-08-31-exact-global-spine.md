# Issue #175 exact Global Spine and progressive `.roproj/v1` open

Decision state: Research evidence for [#174](https://github.com/nurockplayer/tachiko-work/issues/174). This report does not change Accepted architecture.
The recommendation below remains withdrawn under the Project Steward HOLD and
is not republished to #174 by this correction.

Measurement implementations: A0/A1/C/F at
`4ed7f994825edb8a3bb6f1ac4a5cc5d940f74387`; final counterbalanced and
progress-synchronized B, fresh-process E1/E2 comparators, and chunk-cancellable
background admission with post-read fallback behavior at
`01ef8dc0ffd69a5b5314854ae621b7bb5706ed67`;
resampled D at
`8ce554191e36496dedb57322fc1ab059a205ab07`; aggregate two-pass counter
corrections at `607e9208da2fdf71e0741d7ff7efceec890ac6fc`; and cross-crate full-oracle
wiring plus cancellable metadata loading at
`d3edb03b0e2d1bd1fb7690aa09d5a23f92926666`; cancellation-polled accepted
semantic validation at `b31d1429b96d98427631a1eaab8a39f79232bdb1`; and its
research-feature isolation plus host-path formula-node regression at
`878e62c836cb06fd3cf58061567b989c64451213`; and the typed large-record
fast-path closure plus exact-A1 fallback proof at
`3faece0fcbc8c7ef7e2734f404afb8d00c2a2ed4`. The final B/E1/E2 captures are
mechanically reconciled with `main@c3b5ad2aad04e6b79594dbc7f79199591997bdc4`;
the unaffected A0/A1/C/D/F artifacts retain their exact pre-reconciliation
measurement/base heads in the evidence manifest because #193 changed only the
disjoint Designer lane. This final evidence disposition is mechanically
reconciled with `main@2c76be0c61a8e751efe76b02dc66d6a40cb6031b`; no measurement
artifact or measurement provenance was rewritten.

## Outcome

**Research result: outcome A — reject/defer Global Spine, with B only as a
progressive-UX observation over optimized exact eager admission; do not
advance C or D.** B does not satisfy #175's bounded and memory-pressure-aware
background-work requirement for an adversarial arbitrarily large single
record. This does not change Accepted authority or publish an architecture
decision for #174.

The experiment falsified a universal Global Spine benefit:

- optimized exact A1 removes roughly half of current A0 open latency while
  preserving exact `.roproj/v1` admission;
- neither exact Directory nor exact Structural Index beats A1 by the
  preregistered `>=2x` p95 gate on any large realistic class;
- a Structural Index is compact for payload-heavy and constant-AST-heavy
  shapes, but is `1.38x` source for mixed data, `1.78x` for wide fields,
  `2.69x` for chains/cycles, and `2.83x` for reference-heavy data;
- at 16,000 mixed entities, Structural Index peak RSS is higher than A0 and
  more than twice A1; spine plus eventual `Document` raises peak further;
- exact dirty-source sidecar open is `2.13x` slower than A1 at p95,
  while Git validation is `2.20x` slower than an exact A1 that independently
  pays the same Git identity and object-pinning costs;
- background-admission interference is inconclusive: all five ratios above
  `1.10` occur in `background_then_baseline`, whose p95 is `1.683`, while
  `baseline_then_background` has p95 `1.039`, so the combined `1.638` cannot
  be attributed to concurrent background work rather than order/carryover;
- the research reader polls cancellation while reading chunks, but its record
  buffer grows until newline. Its `RequiresForegroundExactAdmission` result is
  therefore a post-read typed decline, not an early allocation-bounded
  fallback for an arbitrarily large single record;
- exact bounded source payload access is fast, but formula meaning and
  validation still require complete admission. The prototype correctly
  returns `requires_full_admission` rather than guessing.

Progressive source preview and exact resident ID/field navigation remain useful
UX techniques. They do not publish partial semantic authority and do not
justify a Global Spine.

## Decision-gate disposition

| Gate | Result | Evidence |
| --- | --- | --- |
| Full-oracle equivalence | Pass for the research paths tested | The actual A0 and A1 admitted outputs under success, cold numeric mutation, cross-cold SCC, and division-by-zero pressure exactly match the source `Document`, `calculate_complete` outcome, and workspace stable observations; late-invalid A0/A1/C rejection parity; formula oracle 10/10 and workspace validation 19/19 |
| `>=2x` p95 benefit over A1 in two realistic large classes | Fail | At 64k, C Structural is only `1.07x` faster than A1 for payload and is `1.41x` slower for references and `1.43x` slower for mixed |
| Benefit not limited to payload-heavy data | Fail | Payload Structural Index is `0.07x` source, but mixed is `1.38x` and references `2.83x` |
| No `>10%` foreground regression | Inconclusive / not attributable | All five ratios above `1.10` are in `background_then_baseline` (p95 `1.683`), while `baseline_then_background` has p95 `1.039`; the combined p95 `1.638` is confounded by arm order/carryover and cannot establish background-attributable regression |
| Background work is bounded, cancellable, foreground-aware, and memory-pressure-aware | Fail for an adversarial single record | Reads poll cancellation by chunk, but the record buffer grows until newline and only then emits `RequiresForegroundExactAdmission`; exact A1 proves correctness recovery for the same source, not early allocation-bounded background fallback |
| `>=40%` peak-RSS reduction, without hidden eventual peak | Fail | A1 p50 peak is `30.4 MB`; Structural is `71.8 MB`; Structural + `Document` is `79.4 MB`; pinned Structural + `Document` is `91.2 MB` |
| Sidecar validation preserves material reuse benefit | Fail | E1 full-open p95 is `719 ms` versus A1 `338 ms`; E2 full-open p95 is `1,042 ms` versus independently Git-pinned A1 `473 ms` |

## Method

All latency measurements used release builds on macOS 15.7.4, Apple M2 Pro,
16 GiB RAM, and an Apple AP0512Z NVMe SSD. Samples are OS-cache-warm. A
privileged cold-cache run was unavailable because `/usr/sbin/purge` returned
`Operation not permitted`; this report makes no cold-cache claim.

The primary matrix contains 2,400 samples: 10 deterministic shapes, 1k/4k/16k
entities, four arms, and 20 repetitions. Payload-heavy, reference-heavy, and
mixed shapes extend to 64k with another 240 samples. Formula-heavy stress
reached 4.08 million AST nodes and 129 MB canonical source at 16k; the 64k
extension selected the three realistic large classes used by the decision
gate. No matrix sample failed or was removed.

Per-run percentiles use the nearest-rank definition
`sorted[ceil(n * p) - 1]`. Host-open, D, and sidecar cells use 30 repetitions.
Fresh-process RSS uses five repetitions per arm and reports macOS byte units;
these RSS values are environment-specific high-water evidence, not a product
SLA.

Every deterministic fixture uses explicit seed `175` and source revision `A`.
The fixture manifest joins `(experiment, workload, entities, source_sha256)`
to that seed/revision; the adversarial revision-race row is `A_to_B`. Raw rows
inherit `schema_version`, experiment, exact base/measurement HEAD, outcome,
and oracle status through the bundle manifest's declared row-context mapping.
Successful rows have `outcome=success`; B cancellation rows retain their
explicit `cancelled` outcome. No missing metadata is inferred.

Fixture generation and initial filesystem materialization are outside admission
timing. E1/E2 setup, exact A1, and reuse each run in separate fresh child
processes; child-internal admission/reuse timing excludes process startup, and
the raw rows separately report process wall time. E2 validated reuse and exact
A1 each include their own independent Git identity, object pin, and
exact-snapshot materialization. Matrix latency is process/allocator-warm inside
one release test process. RSS launches each arm in a fresh direct child. UI
rendering and WASM compile/JIT are outside scope.

The explicit search/navigation contract tested is exact resident ID plus field
navigation and bounded entity lookup. No generic search, exact scalar-value
search, or full-text search claim is made.

## A0 and A1 — exact eager admission

A0 is current host read plus canonical admission followed by ordinary
`load_roproj` decode. A1 performs manifest dispatch, exact layout and lexical
canonical checks, version-owned strict decoding, semantic conversion,
`check_document`, and formula-limit validation while constructing one complete
`Document`. It does not retain the full source tree, re-encode the complete
tree, or decode the semantic document twice.

Both arms start from the same filesystem source. `physical_read_bytes` counts
the source bytes read from the host; A0's second decode is an in-memory parser
pass. `nesting_scan_bytes` and `json_parser_deserializer_bytes` are separate,
and every A0 record/AST/reference/dependency counter aggregates both logical
decodes. `HostReady` is the already-running test process and is outside the
timed interval; no shell/UI endpoint is claimed.

| Entities | Source | A0 p95 SemanticCurrent | A1 p95 SemanticCurrent | A0/A1 |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 0.60 MB | 29.3 ms | 15.5 ms | 1.90x |
| 4,000 | 2.41 MB | 113.3 ms | 59.4 ms | 1.91x |
| 16,000 | 9.63 MB | 583.1 ms | 304.7 ms | 1.91x |

This supports optimizing exact eager admission. It is not evidence for partial
semantic authority.

## C — Directory and Structural Index

Both C arms enforce the exact 18-file tree, strict JSON and recursive duplicate
rules, lexical canonical spelling, shard/order/identity rules, human-key laws,
schema/reference/required-field/type coverage, expression limits, and complete
source fingerprinting. Directory contains identities, human keys, schema
ownership, and byte locators. Structural adds presence/kinds, references,
formula dependency/reverse-dependency edges, and coverage facts.

Neither C result is SemanticCurrent. Scan latency therefore cannot be
interpreted as a time-to-semantic-result win.

### 64k realistic classes

| Shape | Source | A1 p95 | Directory p95 | Structural p95 | Directory/source | Structural/source |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Payload / Unicode / long record | 304.2 MB | 3.57 s | 3.34 s | 3.34 s | 0.052 | 0.070 |
| Reference heavy, 1.024M references | 85.1 MB | 1.99 s | 3.03 s | 2.81 s | 0.192 | 2.826 |
| Mixed realistic game data | 47.8 MB | 1.41 s | 1.64 s | 2.01 s | 0.318 | 1.377 |

### Structural size by shape at 16k

| Shape | Formula nodes / references / edges | Structural/source |
| --- | ---: | ---: |
| Payload / Unicode / long record | 0 / 0 / 0 | 0.070 |
| Reference heavy | 0 / 256k / 0 | 2.825 |
| Formula-edge heavy | 496k / 0 / 256k | 1.181 |
| Formula-AST heavy | 4.08M / 0 / 0 | 0.062 |
| Deep chain | 15,999 / 0 / 15,999 | 2.689 |
| Wide fan-out | 16k / 0 / 16k | 2.326 |
| Wide fan-in | 2.032M / 0 / 1.024M | 1.023 |
| Cross-shard cycle | 16k / 0 / 16k | 2.689 |
| Wide field presence | 0 / 0 / 0 | 1.782 |
| Mixed realistic game data | 48k / 16k / 16k | 1.377 |

The compact payload and constant-AST cases point to narrow payload/locator
optimization, not a universal Structural Index.

## B — progressive UX without partial authority

B publishes source-derived preview only as non-authoritative state. Semantic
Query, Propose, and Execute remain unavailable until exact A1 reaches
SemanticCurrent. Foreground measurements perform exact resident ID/field
navigation, not source I/O or generic search.

At 16k entities, 20 runs of 200 foreground batches × 256 exact operations
alternated `baseline_then_background` and `background_then_baseline`. The
foreground timer began only after the admission worker had completed at least
64 entity records and while that worker was still active:

- baseline per-run request p95: p50 `63 us`, p95 `4,161 us`;
- with background A1: p50 `63 us`, p95 `1,027 us`;
- foreground p95 ratio: p50 `0.866`, p95 `1.638`, max `1.683`; 5/20
  runs exceeded the `1.10` gate;
- order-stratified ratio p95 is `1.039` for baseline-first and `1.683` for
  background-first;
- background SemanticCurrent: p50 `270 ms`, p95 `2,293 ms`;
- cancellation: all 20 cancelled, with p95 `64` and maximum `79` completed
  records; cleanup/join latency p95 is `357 us` and maximum is `518 us`.

The worker was observed after at least 64 completed records; scheduler
overshoot reached 136 records in one interference row and is retained rather
than filtered. The large p95 tails are likewise preserved as recorded noisy
samples.

This capture does not establish a background-attributable regression: the
entire `>1.10` tail is confined to `background_then_baseline`, consistent with
a systematic second-arm/cache carryover effect. The gate is therefore
inconclusive, which is insufficient to advance Global Spine under the
preregistered no-regression requirement. B remains only a non-authoritative
progressive shell/source-preview observation; background scheduling would require
a separately controlled experiment. Correctness authority stays complete and
eager.

The controlled research reader is chunk-cancellable: it checks cancellation
between bounded read chunks and again through strict inspection, decode, work
counting, canonical rendering, and semantic conversion. It is not
allocation-bounded for an arbitrarily large single record, because the record
buffer continues growing until newline before the 64 KiB work-budget check.
Consequently, `RequiresForegroundExactAdmission` is a post-read typed decline,
not an early memory-bounded fallback. Ordinary exact A1 admits the same
canonical source and reaches SemanticCurrent, proving correctness recovery but
not bounded-memory background handling. B therefore fails #175's bounded and
memory-pressure-aware background-work requirement for this adversarial case.

## D — pinned source and bounded materialization

D uses a complete in-memory snapshot of the exact 18 source files, a matching
Structural Index, byte locators, record digests, and source fingerprint. This
is race-safe but retains all source bytes; it is not a low-memory paging result.

At 16k chain entities, p95 first bounded access resampling a fresh complete pin
and first materialization in every repetition was:

| Request | Closure | p95 | Proof |
| --- | ---: | ---: | --- |
| Random entity | 1 | 13.1 ms | exact source payload; semantic state not current |
| Near dependency | 2 | 13.5 ms | requires full admission for formula truth |
| Mid dependency | 8,001 | 94.2 ms | requires full admission for formula truth |
| Far dependency | 16,000 | 292 ms | requires full admission for formula truth |
| Cold reverse dependents | 15,999 | 392 ms | requires full admission |

Cross-cold cycles, source revision changes, and pinned revision-A versus live
revision-B behavior are covered by executable correctness tests. Unsupported
formula proof always returns `RequiresFullAdmission`; no bounded formula value
is published.

This supports bounded source navigation as a narrow technique. It does not
provide a complete semantic result and therefore does not satisfy C or D's
architecture gate.

## E — disposable sidecars

Both sidecars bind explicit format/algorithm identity, complete payload and
envelope integrity, and exact source identity. Because those checks are
self-reported by an untrusted cache, validated reuse also pins all 18 source
files, re-derives the complete exact Structural Index, and requires
fact-for-fact equality. Stale, corrupted, fabricated, unknown, incompatible,
or race-observed inputs fail closed to exact local admission. The JSON sidecar
has no SQLite/WAL/SHM footprint.

At 16k mixed entities:

| Case | Source | Sidecar | Reuse p95 | A1 p95 | Interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| E1 dirty filesystem, validated full open | 9.63 MB | 18.83 MB | 719 ms | 338 ms | Separate fresh processes; full open includes sidecar read and is 2.13x slower |
| E2 identity + decode only | 9.63 MB | 18.83 MB | 261 ms | 473 ms | Non-authoritative diagnostic; source-derived facts are not proven |
| E2 Git-object + pinned source-derived full open | 9.63 MB | 18.83 MB | 1,042 ms | 473 ms | Separate fresh processes independently pin the same commit/object bytes; exact full open is 2.20x slower |

E1 first build also costs a 422 ms scan plus 84 ms encode. The mixed sidecar is
`1.96x` source size. Integrity detects accidental corruption; it is not an
authentication mechanism against malicious replacement of both sidecar and
checksum.

## F — peak and eventual residency

Fresh-process p50 peak RSS at 16k mixed entities:

| Retained state | p50 peak RSS |
| --- | ---: |
| Baseline child | 2.0 MB |
| A1 complete `Document` | 30.4 MB |
| A0 complete `Document` | 66.7 MB |
| Directory | 48.9 MB |
| Structural Index | 71.8 MB |
| Structural + pinned source | 76.5 MB |
| Structural + pinned hot payload | 75.0 MB |
| Structural + complete `Document` | 79.4 MB |
| Structural + pinned source + complete `Document` | 91.2 MB |

The existing resident runtime separately measured `26.1 MB` for the synthetic
complete `Document` and `93.9 MB` for Document plus retained address,
calculation, dependency, reverse-dependency, and validation state. A new spine
would duplicate those structures unless a future production design proved
replacement rather than coexistence. This research does not prove such a
replacement.

## Correctness and adversarial pressure

The executable pressure matrix includes final-shard/final-record duplicate IDs
and keys, missing and wrong-schema references, final order and shard defects,
recursive duplicate/unknown members, malformed formulas, invalid and duplicate
human keys, empty titles, formula limits, far dependencies, cross-shard cycles,
revision races, corrupt/stale/unknown sidecars, Git payload mismatch, and
unsupported proof. Self-consistent fabricated sidecars are rejected by
complete comparison with the independently source-derived index. Reused
dirty-source state retains the pinned bytes that its index describes; the
generic Git opener fails closed unless the dedicated path independently
recomputes Git identity.

A1 preserves the current profile-before-semantic-validation precedence.
Lexical canonical proof retains number-token evidence rather than relying on
decoded `f64` equality. Directory/Structural scans cannot publish merely after
structural parsing: they run the required document-level coverage checks.

Focused validation at closure implementation HEAD
`3faece0fcbc8c7ef7e2734f404afb8d00c2a2ed4`:

- `cargo test -p tachiko-storage issue_175_ --locked`: 26 passed, 11 ignored
  measurement/internal-child entrypoints, 16 filtered out;

Prior unaffected validation retained by the evidence bundle:

- cross-crate admitted-output full-oracle test: 1 passed;
- storage `.roproj/v1` and host suites: 66 passed;
- formula complete-oracle suite: 10 passed;
- workspace validation-report suite: 19 passed; and
- workspace clippy with all targets and `-D warnings`: pass.

## Limitations

- Cold OS-page-cache evidence is unavailable. All latency tables say
  `os_cache_warm`.
- Matrix/A0/A1/B/C/D latency runs are process/allocator-warm. E1/E2 use fresh
  processes for setup, A1, and reuse; their internal and process-wall timings
  are separate columns.
- RSS has five repetitions per arm, enough to falsify the 40% reduction claim
  here but not to define a product memory SLA.
- A1 is a research implementation shared with the cross-crate oracle only
  through an opt-in test feature. It still performs strict syntax,
  duplicate-member, nesting, and DTO traversals and allocates temporary
  per-record canonical spelling evidence.
- Source preview is non-authoritative, capped at a 64 KiB record, cancellable,
  and exact-layout checked. The B foreground contract begins from an already
  resident exact `Document` and is not evidence for cold payload search.
- Background A1 polls cancellation inside a research-feature-only copy of the
  accepted semantic validator at schema, entity, field, and formula-node
  boundaries before SemanticCurrent. Entity records also poll between read
  chunks and check after each strict/decode/count/render/conversion phase, but
  the single-record buffer grows until newline without an allocation ceiling.
  The research fast path returns typed `RequiresForegroundExactAdmission`
  only after the 64 KiB post-read work-budget check. The same canonical source
  succeeds through ordinary exact A1 fallback and reaches SemanticCurrent,
  proving correctness recovery but not early memory-bounded fallback. Ordinary
  production validation remains exactly on its pre-experiment implementation
  path.
- D pins the complete source snapshot, so its timing cannot be interpreted as
  a source-RSS reduction.
- No UI render or WASM compile/JIT timing is included. E1/E2 raw rows disclose
  process wall time, while their primary internal timers exclude startup.
- No scalar-value or full-text search implementation was tested.

## Evidence and reproduction

The bundle [manifest](evidence/issue-175/manifest.json) records exact commands,
environment, source/measurement commits, cache state, percentile definition,
row-context inheritance, and SHA-256 for every CSV. The
[fixture manifest](evidence/issue-175/fixture-manifest.csv) binds seed,
revision, and source hashes. Raw and derived tables live under
[`docs/research/evidence/issue-175/`](evidence/issue-175/).

The large fixtures are generated deterministically by the ignored release
tests in
`crates/storage/src/roproj/v1/issue_175_research.rs`; no generated corpus is
committed. Retained-runtime RSS is in
`crates/workspace-engine/tests/issue_175_residency.rs`.

## Provisional interpretation withheld from #174

The previously drafted A/B/C/D recommendation remains withdrawn while the
Project Steward HOLD is active. These corrected observations are not a new
architecture publication, do not change Accepted authority, and authorize no
ADR, `.roproj/v2`, public readiness/cache protocol, production successor, or
Designer work. Delivery returns to Project Steward only after exact-head
validation and review convergence.
