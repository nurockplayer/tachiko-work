# Work #503 scalar/carrier acceptance seed

Status: bounded repair admitted by [the #503 Ready disposition](https://github.com/nurockplayer/tachiko-work/issues/503#issuecomment-6096139133),
against main `a4fc7845456205c541c6f0c393235f0d75adb459`. Final repair qualification
is pending; no qualified kit or consumer acceptance is included.

The issue's expected outcomes and Accepted ADR-0027 fidelity boundary are the
oracle. The independently reviewed author-prepared seed was adopted in that disposition;
it is not falsely attributed as originally authored by the Steward.

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
The admitted baseline produced native mismatch/formula failures and a raw-WASM
first-mismatch failure as recorded below. The runner records the supplied WASM
SHA256; the executor must also
bind it to exact source/manifest provenance. A hash alone is not qualification.

The raw-WASM runner creates a new output directory and records `completed:false`
plus failure details on errors after that directory is created, including a
missing WASM file. It never overwrites an earlier run or reports
completed success after setup, missing export, malformed reply, or assertion
failure. Node and Python standard library suffice for the harness; no package
installation or dependency/lock change is needed.

## Current evidence limits

[Hosted baseline run 38040384393](https://github.com/nurockplayer/tachiko-work/actions/runs/38040384393)
executed seed `47c858381e157de65475f9daaaad1abe6755054b` on unchanged production:
43 native adapter tests passed and two mismatch/formula assertions failed.
Actual raw WASM built and failed the first `number-inline.xlsx` Inspect assertion
for missing blocking `scalar_mapping_rejected`. `completed:false` means the
remaining twenty fixtures and subsequent refusal/history/reopen checks did not run.
The original full browser-client gate separately stopped on test formatting;
its hosted rustfmt diff is applied mechanically without changing assertions.

The artifact's ZIP SHA256 is
`fb0b6a6f632d1fcde5ba4378a73a3c97ad243b61fadff3a3ed854b5cd0e9997c`;
WASM SHA256 is
`b7588bc5295d97539eb47eaf0a2377e50f0eb096a2e1f8c76e5b60d5722dbaab`.
Full source/tree/toolchain evidence and the immutable private backup are recorded
in [the seed receipt](https://github.com/nurockplayer/tachiko-work/issues/503#issuecomment-6095905411).
Rust/Cargo remain unavailable in the dot executor. Repaired native/WASM GREEN,
full browser/actual-Worker, final repository checks and independent deep review
are pending. Local harness checks are not runtime or Sheet qualification.

The admitted repair shares structural type/carrier rejection between scalar
conversion and formula-cache recovery. It preserves compatible caches and blanks.
No numeric inference, alternate semantic model, Date guard, capacity rule, kit/lock
or consumer pin change is included. Full #503 acceptance stays open until the
complete exact-source native/WASM and applicable repository gates pass.
