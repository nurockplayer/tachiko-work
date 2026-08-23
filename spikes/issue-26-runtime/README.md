# Issue #26 Native/WASM Runtime Spike

Decision state: executable Provisional evidence, not a public SDK or wire
contract.

This standalone spike demonstrates one Rust-owned semantic `Document` behind a
TypeScript Node Worker and real `wasm32-unknown-unknown` module. It deliberately
lives outside the accepted eight-crate production workspace and calls
`tachiko-workspace-engine` for semantic queries, mutations, formula
calculation, diff, and merge.

The TypeScript client retains no document mirror. Whole-document values cross
the boundary only for explicit open/snapshot comparison and three-way merge
inputs. Normal commands return a revision plus typed affected-value patches and
semantic-diff evidence. The Worker message shape and WASM loader use
browser-standard primitives, but an actual browser Web Worker is not executed
by this spike.

## Run

```bash
bash scripts/issue-26-runtime-build.sh
cargo test --manifest-path spikes/issue-26-runtime/Cargo.toml --all-targets --locked
pnpm --dir spikes/issue-26-runtime exec node --test \
  test/worker-runtime.test.ts \
  test/native-wasm-parity.test.ts
bash scripts/issue-26-portability-audit.sh
pnpm --dir spikes/issue-26-runtime exec node benchmark/runtime-benchmark.ts
```

The benchmark uses independent Worker/WASM instances for the resident and
whole-snapshot paths. Its byte counts are JSON payload-size estimates, not
instrumented structured-clone traffic. Override the default 20 mutations with
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

## Integration note

The recorded benchmark values come from the original spike baseline. After PR
#92 hardened formula/workspace production boundaries, this branch was updated
only to trigger merge-time CI against the resulting current `main`; the
measurements remain historical topology evidence rather than a new performance
run.
