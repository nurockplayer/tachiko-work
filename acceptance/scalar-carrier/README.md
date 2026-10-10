# Work #503 scalar/carrier acceptance seed

Status: tests-only proposal against Work main
`a4fc7845456205c541c6f0c393235f0d75adb459`. No production source changes,
Ready declaration, qualified kit, or consumer acceptance are included.

The issue's expected outcomes and Accepted ADR-0027 fidelity boundary are the
oracle. This seed is author-prepared for independent adequacy and existing
Steward disposition, not falsely attributed to the retained Steward.

## Coverage

- `incompatible_scalar_carriers_are_blocking_before_empty_defaults`: numeric,
  default numeric, inline string, and string mismatch must yield blocking
  `scalar_mapping_rejected`; native semantic import must refuse.
- `compatible_scalar_carriers_and_numeric_blanks_keep_their_values`: valid inline
  Text, explicit/default numeric values, and legitimate numeric blanks remain.
- `formula_cache_cannot_hide_incompatible_scalar_carriers`: formula presence and
  ignored-cache recovery cannot downgrade incompatible carriers to evidence-only.
- `duplicate_or_mixed_scalar_carriers_remain_blocking`: existing structural
  protections remain. Existing formula-cache tests remain unchanged and required.
- `run.mjs`: actual raw Rust WASM imports a one-column resident, exports the seed
  XLSX, and exercises 21 carrier fixtures through Inspect and Import. Each refusal
  must preserve occurrence, current revision, exact table/project/CSV/XLSX bytes.
  Both history stacks are established before refusals, then Redo and two Undos
  must recover exact earlier snapshots. Canonical reopen and both exports must
  preserve the original resident. Positive imports use `install:false`.
  Compatible formulas separately assert retained A1 source in the preview,
  retained semantic formula and a Rust-calculated value of 3. A compatible
  inline-string cache remains Text in the preview, not an artificial Empty.

`fixtures.py` changes exactly A2 of the runtime-produced seed. All other ZIP
entries are byte-preserved. Original and mutated XML are retained in generated
fixtures; source SHA256 values are in the generated manifest. No production
parser or replacement semantic implementation is used in the oracle.

## Commands on a qualified toolchain

```sh
cargo test --manifest-path packages/browser-client/runtime/Cargo.toml --locked --test interop_adapter
node acceptance/scalar-carrier/run.mjs /absolute/exact/runtime.wasm /absolute/new/result-directory
```

Run the same seed on baseline first, then the admitted repaired exact candidate.
The native mismatch/formula tests and raw-WASM first mismatch should demonstrate
the behavioral failure on baseline. That is an expectation, **not an executed
RED result**. The runner records the supplied WASM SHA256; the executor must also
bind it to exact source/manifest provenance. A hash alone is not qualification.

The raw-WASM runner creates a new output directory and records `completed:false`
plus failure details on errors after that directory is created, including a
missing WASM file. It never overwrites an earlier run or reports
completed success after setup, missing export, malformed reply, or assertion
failure. Node and Python standard library suffice for the harness; no package
installation or dependency/lock change is needed.

## Current evidence limits

Rust/Cargo are unavailable in the current dot executor. Native compile/tests,
baseline RED, repaired GREEN, actual raw-WASM behavior, formatting/Clippy/MSRV,
browser/Worker and repository-wide gates are **UNRUN**. Node syntax, ZIP mutation
falsifiers and runner fail-closed control are harness checks only. They are not
runtime, producer, or Sheet qualification. Full issue acceptance stays open.

The source repair is intentionally not applied before the existing Ready gate.
The bounded proposed direction is a cell type/carrier structural check shared by
`cell_value` and `ignorable_formula_cache`, before missing-value defaults. No
numeric inference, alternate semantic model, Date guard, capacity rule, or
supported formula-cache change is proposed. One coherent Guarded review unit;
existing leads, #413, kit/lock and all release HOLDs remain unchanged.
