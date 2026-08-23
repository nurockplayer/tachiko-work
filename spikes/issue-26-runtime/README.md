# Issue #26 Native/WASM Runtime Spike

Decision state: executable Provisional evidence, not a public SDK or wire
contract.

This standalone spike demonstrates one Rust-owned semantic `Document` behind a
TypeScript Worker and real `wasm32-unknown-unknown` module. It deliberately
lives outside the accepted eight-crate production workspace and calls
`tachiko-workspace-engine` for semantic queries, mutations, formula
calculation, diff, and merge.

The TypeScript client retains no document mirror. Whole-document values cross
the boundary only for explicit open/snapshot comparison and three-way merge
inputs. Normal commands return a revision plus affected calculated projections
and semantic-diff evidence.

## Run

```bash
bash scripts/issue-26-runtime-build.sh
cargo test --manifest-path spikes/issue-26-runtime/Cargo.toml --all-targets --locked
node --test \
  spikes/issue-26-runtime/test/worker-runtime.test.ts \
  spikes/issue-26-runtime/test/native-wasm-parity.test.ts
bash scripts/issue-26-portability-audit.sh
node spikes/issue-26-runtime/benchmark/runtime-benchmark.ts
```

The benchmark uses independent Worker/WASM instances for the resident and
whole-snapshot paths. Override the default 20 mutations with
`TACHIKO_SPIKE_ITERATIONS`.

## What is intentionally throwaway

- The raw four-function linear-memory ABI.
- Direct Serde encoding of workspace-internal semantic Rust structures.
- The JSON command/result spelling and string-only error transport.
- The synthetic project generator and benchmark result DTOs.

These mechanisms expose the pressure on a future adapter; they must not be
treated as a frozen external API. See
[`docs/research/2026-08-23-native-wasm-runtime-spike.md`](../../docs/research/2026-08-23-native-wasm-runtime-spike.md)
for the evidence and recommendation.

