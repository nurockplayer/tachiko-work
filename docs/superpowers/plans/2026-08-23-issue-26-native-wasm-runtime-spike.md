# Issue #26 Native/WASM Runtime Spike Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce executable evidence that one Rust-owned `Document` can serve native and Worker-hosted WASM clients through `tachiko-workspace-engine` without moving a full document across the boundary for every operation.

**Architecture:** Keep the accepted eight-crate production graph unchanged. A standalone spike crate wraps the existing snapshot-style workspace-engine in a resident `Document` owner, exposes the same command execution through a native driver and a minimal raw WASM byte-buffer ABI, and maps provisional results into spike-only JSON DTOs. TypeScript owns Worker transport plus revisioned projections only; persistence remains an external host responsibility.

**Tech Stack:** Rust 2024/MSRV 1.85, `wasm32-unknown-unknown`, Serde JSON, Node.js 24 TypeScript type stripping, Node `worker_threads`, Cargo release builds.

**Spec:** GitHub Issue #26, ADR-0015 through ADR-0019, and `docs/architecture/rust-crate-architecture.md`.

## Global Constraints

- `workspace-engine != filesystem`, browser storage, or Tauri host.
- React/frontend state is never the canonical semantic document.
- Do not add a ninth production workspace crate or change the ADR-0016 dependency graph.
- The spike DTO/ABI is Provisional evidence, not a public SDK or diagnostic contract.
- Do not decide #23 diagnostics details, #41 `.roproj`, or #43 portable package design.
- Every mutation uses workspace-engine policy and publishes a new resident state only after the existing immutable preview succeeds.

---

### Task 1: Resident Runtime and Snapshot Comparison Seam

**Files:**
- Create: `spikes/issue-26-runtime/Cargo.toml`
- Create: `spikes/issue-26-runtime/src/lib.rs`
- Create: `spikes/issue-26-runtime/tests/resident_runtime.rs`

**Interfaces:**
- Consumes: `tachiko-workspace-engine::{overview, calculate_fields, set_scalar, compare_documents, Document}`.
- Produces: `ResidentRuntime::open`, `ResidentRuntime::execute`, `execute_snapshot`, and `synthetic_document`.

- [x] **Step 1: Write failing resident-runtime tests**

Test that opening a deterministic synthetic document, querying overview/calculation, mutating a scalar, observing formula impact/diff, and taking a snapshot all use one revisioned Rust-owned state. Test the stateless snapshot seam with the same command and assert byte-equivalent semantic results.

- [x] **Step 2: Run the test and verify RED**

Run: `cargo test --manifest-path spikes/issue-26-runtime/Cargo.toml --test resident_runtime --locked`

Expected: compile failure because the spike crate/runtime interfaces do not exist.

- [x] **Step 3: Implement the minimal resident wrapper**

Define spike-only `Command` and `Response` Serde types. Implement immutable workspace-engine preview application followed by resident replacement, return revision/result/diff projections rather than the document, and keep explicit snapshot import/export only for load/persistence comparison.

- [x] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test --manifest-path spikes/issue-26-runtime/Cargo.toml --all-targets --locked`

Expected: all resident and snapshot equivalence tests pass.

### Task 2: Raw WASM ABI and TypeScript Worker Boundary

**Files:**
- Create: `spikes/issue-26-runtime/src/wasm.rs`
- Create: `spikes/issue-26-runtime/src/bin/native-driver.rs`
- Create: `spikes/issue-26-runtime/worker/protocol.ts`
- Create: `spikes/issue-26-runtime/worker/wasm-runtime.ts`
- Create: `spikes/issue-26-runtime/worker/runtime-worker.ts`
- Create: `spikes/issue-26-runtime/worker/runtime-client.ts`
- Create: `spikes/issue-26-runtime/test/worker-runtime.test.ts`

**Interfaces:**
- Consumes: UTF-8 JSON request bytes and the resident/snapshot functions from Task 1.
- Produces: alloc/dealloc plus request/response WASM exports, a Worker RPC protocol, and a revision/projection-only TypeScript client.

- [x] **Step 1: Write a failing Worker integration test**

The test loads the real `.wasm`, opens a synthetic project, runs overview, calculation, mutation, and snapshot requests through a worker, and asserts the formula result and diff evidence.

- [x] **Step 2: Run the integration test and verify RED**

Run: `pnpm --dir spikes/issue-26-runtime exec node --test test/worker-runtime.test.ts`

Expected: module/build failure because the worker/WASM artifacts do not exist.

- [x] **Step 3: Implement the minimal raw ABI and Worker client**

Keep one copied output buffer valid until the next call. Decode/encode only at the adapter, copy WASM memory immediately in TypeScript, and never retain a document mirror in `RuntimeClient`.

- [x] **Step 4: Build and run the Worker test**

Run: `bash scripts/issue-26-runtime-build.sh`

Run: `pnpm --dir spikes/issue-26-runtime exec node --test test/worker-runtime.test.ts`

Expected: the real Worker → WASM → workspace-engine path passes.

### Task 3: Native/WASM Differential Corpus

**Files:**
- Create: `spikes/issue-26-runtime/fixtures/parity-commands.json`
- Create: `spikes/issue-26-runtime/test/native-wasm-parity.test.ts`

**Interfaces:**
- Consumes: one deterministic command corpus.
- Produces: exact decoded native and WASM semantic values for overview, formula calculation, mutation/diff, snapshot execution, and a merge probe where the existing snapshot API remains usable.

- [x] **Step 1: Write the failing differential test**

Execute the corpus with the native driver and Worker/WASM runtime, normalize only elapsed timing fields (none are semantic), and compare every record exactly.

- [x] **Step 2: Run and verify RED**

Run: `pnpm --dir spikes/issue-26-runtime exec node --test test/native-wasm-parity.test.ts`

Expected: failure until both drivers implement the shared corpus protocol.

- [x] **Step 3: Implement shared deterministic execution**

Use the same Rust `Command`/`Response` implementation for both targets. Keep merge as explicit three-snapshot evidence rather than inventing resident branch/history semantics.

- [x] **Step 4: Run and verify GREEN**

Run: `pnpm --dir spikes/issue-26-runtime exec node --test test/native-wasm-parity.test.ts`

Expected: identical decoded deterministic semantic values from native and WASM
records. Transport envelope bytes are not part of the comparison.

### Task 4: Portability Audit and Performance Evidence

**Files:**
- Create: `scripts/issue-26-runtime-build.sh`
- Create: `scripts/issue-26-portability-audit.sh`
- Create: `spikes/issue-26-runtime/benchmark/runtime-benchmark.ts`
- Create: `spikes/issue-26-runtime/README.md`
- Create: `docs/research/2026-08-23-native-wasm-runtime-spike.md`

**Interfaces:**
- Consumes: 10/100/1000-entity synthetic projects and release WASM/native artifacts.
- Produces: portable target build results, dependency/host-capability audit, JSON measurement output, and an evidence-backed architecture recommendation.

- [x] **Step 1: Add executable portability checks**

Compile semantic-core, formula, diff, merge, workspace-engine, and provider-free AI separately for `wasm32-unknown-unknown`; scan their production sources and dependency graph for filesystem, path, clock, randomness, locale/environment, thread, network, and native-only leakage.

- [x] **Step 2: Add boundary benchmark**

Measure initial snapshot bytes/time, repeated stateless whole-document mutation roundtrips, resident command mutations, compact mutation-result bytes, and full calculation-query payload bytes for each project size. Report medians and totals, not single-call microbenchmarks.

- [x] **Step 3: Run the evidence suite**

Run: `bash scripts/issue-26-portability-audit.sh`

Run: `pnpm --dir spikes/issue-26-runtime exec node benchmark/runtime-benchmark.ts`

Expected: machine-readable measurements for all scales and a zero-exit portability audit.

- [x] **Step 4: Record findings without freezing the public API**

Document recommended topology, resident ownership, JS/WASM messages, frontend projection/ephemeral state, host persistence, concrete current API friction, decided/open Issue #26 questions, and focused follow-up amendments/tickets.

### Task 5: Full Verification, Review, Commit, and Focused PR

**Files:**
- Verify all files from Tasks 1-4.

**Interfaces:**
- Produces: a reviewed focused branch with reproducible evidence and no production semantic changes.

- [x] **Step 1: Run formatting, lint, tests, portability, parity, and benchmarks fresh**

Run the root workspace gates plus the standalone spike tests and Worker differential suite.

- [x] **Step 2: Review `origin/main...HEAD`**

Inspect for duplicated semantic policy, a second canonical state, host leakage, unstable DTO claims, diagnostics-contract drift, unsafe ABI lifetime bugs, and benchmark methodology errors.

- [ ] **Step 3: Commit and open an unmerged focused PR**

Use `rtk` for every git/gh command. The PR reports exact evidence, explicit open questions, and links Issue #26 without claiming to close architectural work that remains open.
