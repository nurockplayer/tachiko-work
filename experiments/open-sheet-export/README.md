# open-sheet export acceptance experiment

Owner: [Issue #337](https://github.com/nurockplayer/tachiko-work/issues/337),
first bounded child of [#335](https://github.com/nurockplayer/tachiko-work/issues/335).
**Execution authorization follows the live Issue.** Repository Accepted
authority and the live Steward readiness decision outrank this harness.
`export.mjs` is the disposable prototype entry point. Continue this branch for
one delivery PR only; never treat this experiment as a production dependency
or a replacement for Tachiko's native semantic engine.

## Selected seam and scope

Use existing Rust CLI semantic queries over one captured immutable canonical
snapshot, then translate their typed formula facts to the public open-sheet
`CompiledWorkbook` / `XlsxWriter` surface. Do not parse Tachiko formula source
or canonical storage in the TypeScript adapter. The current Designer client
kit exposes formula text rather than a complete bound AST; expanding the kit
is not a prerequisite of this first experiment. `tachiko formula inspect`
already returns the bound `expression`, calculation and `source_revision`.
The native CI preflight also demonstrates how existing semantic inspection
supplies scalar values and schema/field identity without a second semantic reader.

Read all facts from the same captured source. Do not issue independent reads
against a changing live directory. A CLI source token is not a resident-session
revision or permission token. The adapter never calculates formulas, publishes
semantic state, or treats Excel/open-sheet coercion as Tachiko authority.

The output has live formulas but no formula caches (`cacheValues: false`).
Native stable IDs, type/validation guarantees, revision/history and native
round-trip fidelity are not preserved by ordinary XLSX; disclose this through
the fidelity ledger. Reverse import, arbitrary TSX execution, SDK extraction,
charts, PDF/HTML and partner-side runtime demonstrations remain outside this
slice. The existing shipped Designer exporters are not replaced here.

## Pinned inputs

- Tachiko baseline: `3887612f65cc788efa0c23417e7ed16c8289dbb0`.
- open-sheet source: `6ab72ddb50bb87dad11cc321d87b5e9bcc3c5689`.
- That source's expected core manifest version is `0.2.0`. This is not a claim
  that the corresponding npm tarball contains the same source.
- `open-sheet-provenance.json` records the exact upstream checkout, source
  lockfile and manifest digests, plus the built public-package tarball digest.
  `package.json` consumes that tarball only through a local file dependency;
  `pnpm-lock.yaml` makes that isolated install reproducible. No `src/*` import
  or registry substitution is used.
- `canary.json` contains direct-ro/v2 source and disposable presentation
  requests. Rust validates and materializes the source as canonical `.roproj`.
  The fixture does not hand-write an alternate canonical project codec.

The quarterly plan has tax `0.25`, revenues `1200/1600/2000` and costs
`400/800/1000`. Gross values are `800/800/1000`; net values are
`600/600/750`. Changing tax to `0.5` gives `400/400/500`. The integers and
binary fractions deliberately admit exact assertions. This does not certify
all binary64 edge cases or arbitrary Excel numerical behavior.

## Preflight commands

Use the toolchain required by the live repository (currently Node >=24 and
pnpm 11.25.0). The local Node 22 syntax check is not runtime qualification.
Build existing source, not a new exporter:

```sh
cargo build -p tachiko-cli --locked
export TACHIKO_BIN="$(pwd)/target/debug/tachiko"
```

Use a clean scratch checkout of the exact open-sheet commit. Follow its own
AGENTS/build instructions with pnpm, build its public package, and install or
link that built package into an otherwise empty scratch consumer. Record the
checkout HEAD, manifest, lockfile/installation provenance and actual command
log. Do not silently substitute an npm version or import `src/*` paths.

```sh
export OPEN_SHEET_CONSUMER_DIR=/absolute/path/to/scratch-consumer
uv run --no-project --no-managed-python python \
  experiments/open-sheet-export/acceptance.py --mode self-check
uv run --no-project --no-managed-python python \
  experiments/open-sheet-export/acceptance.py --mode preflight
```

The public-package canary is also runnable separately:

```sh
node experiments/open-sheet-export/public-seam-probe.mjs \
  --output /absolute/path/to/absent-probe.xlsx
```

`preflight` has three tests: Rust fixture/query admission, real Rust edit/rename,
and public upstream writer invocation. The probe does not call open-sheet's
evaluator. A missing executable/package/seam is an environment failure, not a
behavioral RED. While the Issue is not Ready, return the results to the Steward.
After a live Ready decision, public-package preflight is the first implementation
checkpoint: failure blocks further adapter work until reconciled. It never
authorizes a production dependency, wider scope or upstream changes.

## Native CI bridge and behavioral RED

`crates/cli/tests/open_sheet_acceptance.rs` uses Cargo's real built `tachiko`
executable to run `native_ci.py` under the existing push CI. The existing
quality workflow installs this experiment's frozen pnpm dependency graph before
the Cargo bridge runs. This is the minimal CI wiring required to resolve the
checked-in public-package tarball on a clean hosted runner. The bridge itself
uses the runner's Python 3 directly; it installs no Python dependency and does
not require uv on the hosted image.

```sh
pnpm --dir experiments/open-sheet-export install --frozen-lockfile
cargo test -p tachiko-cli --test open_sheet_acceptance --locked
```

The native subset adds schema/scalar query evidence and tests the exporter
process. It intentionally does not select the public-package probe or Office
case; these remain separate mandatory delivery gates, not skipped passes. The
checked-in process captures an immutable source snapshot, queries only that
snapshot through the Rust CLI, projects the returned typed facts through public
open-sheet APIs, and writes an uncached XLSX. It does not parse `.roproj`,
interpret Tachiko expressions, or calculate formulas.

## Disposable exporter boundary, after Ready only

Set `OPEN_SHEET_POC_COMMAND` to a JSON argv array for the future experiment's
entry point. The tests append `--request <request.json>`. For example:

```sh
export OPEN_SHEET_POC_COMMAND='["node","/absolute/path/to/experiments/open-sheet-export/export.mjs"]'
uv run --no-project --no-managed-python python \
  experiments/open-sheet-export/acceptance.py --mode acceptance
```

The request carries `source` (canonical project directory),
`expected_source_revision` (opaque CLI query token), `projection` (one layout
from the fixture), and `output` (new `.xlsx` destination). These names are an
experiment-local process contract, not a new public SDK or stable protocol.
Each sheet names a schema, ordered stable row/field IDs, zero-based
`header_row` and `left_column`, and per-field Excel number formats. Data begins
immediately after the header. Headers use current field keys and bold text.
Each selected target has one destination; missing dependencies and duplicates
reject rather than guessing. No A1 address is an authoring identity.

Success exits zero, writes the absent destination, and emits only JSON on
stdout:

```json
{"status":"exported","source_revision":"<exact input revision>","writer":{"name":"open-sheet","source_commit":"6ab72ddb50bb87dad11cc321d87b5e9bcc3c5689"},"ledger":[{"code":"semantic_metadata_not_exported"}]}
```

Rejection exits nonzero, creates no artifact, preserves any existing output,
and emits `{"status":"rejected","code":"...","ledger":[...]}`. Expected
codes are `stale_source_revision`, `unprojected_reference`,
`ambiguous_projection`, `output_exists`, `invalid_output`, and
`unsupported_value`. The latter must also appear as a ledger finding. Ledger
entries may contain additional useful structured detail. Human diagnostics
belong on stderr. Source files must remain byte-identical throughout export.
The final review must check actual writer invocation and snapshot acquisition;
a self-reported writer/revision string does not prove either implementation.

## Reference engine, after exporter acceptance

```sh
export REFERENCE_OFFICE_BIN=/absolute/path/to/libreoffice
uv run --no-project --no-managed-python python \
  experiments/open-sheet-export/acceptance.py --mode office
```

This mode includes the preceding runtime/export tests and runs LibreOffice in
fresh isolated profiles. Before recalculation it verifies that exported live
formulas contain no cached numeric answers. It then checks all six actual
formula results at both tax rates. Merely reading our cached answers, launching
the executable, skipping the engine, or loading XML is not recalculation proof.
LibreOffice evidence is not Microsoft Excel certification. A different office
engine requires a recorded bounded harness reconciliation, not a fake binary.

## Requirement-to-test map

| Requirement | Executable evidence |
| --- | --- |
| Valid canonical source and authoritative bound facts | `RuntimePreflight.test_fixture_reaches_real_rust_formula_boundary` |
| Existing safe mutation and identity-preserving rename | `test_existing_rust_edit_and_rename_preserve_bound_targets` |
| Public upstream writer, no evaluator/cache dependency | `test_public_upstream_writer_without_evaluator` |
| Cross-sheet formulas, styles, moved rows/columns/sheets | `test_live_formulas_and_formats_in_base_and_moved_layout` |
| Real changed source, same bound targets | `test_changed_input_and_rename_reexport_from_current_rust_source` |
| Stale/missing/ambiguous/unsupported input rejection | Four corresponding `test_*rejected*` / unsupported-value cases |
| No overwrite or source-directory pollution | Destination-preservation and inside-source rejection cases |
| Formula shape, not just equal current values | `test_all_current_formula_operators_preserve_expression_shape`; `verify_book` |
| Text/Boolean fidelity, growth, repeatable observable export | Literal-text, Boolean, grown-source and repeated-snapshot cases |
| Actual Office arithmetic without trusting export caches | `OfficeAcceptance.test_uncached_exports_recalculate_in_real_office_engine` |

The test-only Excel arithmetic reader compares syntax trees against the known
fixture, resolving exported addresses back to stable targets. It does not
evaluate formulas, require a particular serializer whitespace style, or supply
production semantics. Byte-identical ZIP output is not asserted: observable
workbook content, formulas and formats are. Core numeric normalization and
broader compatibility remain under their existing owners.

## Current implementation evidence, 2026-09-07 JST

The immutable upstream checkout at
`6ab72ddb50bb87dad11cc321d87b5e9bcc3c5689` was built with its declared
`pnpm@10.33.2`, producing public `@open-sheet/core@0.2.0`. The built tarball
SHA-256 is `db25607ba5a429c50a7520890312618201af33bc0cc4dc0d1ecff084dd475fc6`.
The complete recorded provenance is in `open-sheet-provenance.json`; the
adapter imports only `@open-sheet/core` and `@open-sheet/core/node`.

`sample.xlsx` is a reproducible base-canary export from this adapter, with
SHA-256 `dd7efe4bdbf83e41ea114be98120cd8915af7a05cea618d1b59fb23e374477af`.
The independent OOXML observer validates its sheets, live formula shapes,
formats and absent formula caches. It is an inspectable sample, not Office
recalculation evidence.

With `TACHIKO_BIN` set to a fresh `cargo build -p tachiko-cli --locked` output
and `OPEN_SHEET_CONSUMER_DIR` set to this frozen experiment install, these
checks pass on the implementation candidate:

- `acceptance.py --mode self-check`: 8 checks.
- `acceptance.py --mode preflight`: 3 checks against the independent scratch
  public-package consumer.
- `pnpm --dir experiments/open-sheet-export test`: 2 adapter unit tests.
- `acceptance.py --mode acceptance`: 16 runtime/export checks, including
  formula shape, styles, source preservation, changed-source export and every
  required rejection path.
- `cargo test -p tachiko-cli --test open_sheet_acceptance --locked`: the
  existing Cargo CI bridge passes.
- `cargo fmt --all -- --check`, `node scripts/workspace-dependency-check.mjs`,
  and `bash scripts/docs-consistency-check.sh` pass.

These are implementation and contract checks, not Office-calculation proof.
No `libreoffice` or `soffice` executable is available in the current execution
environment, so `acceptance.py --mode office` remains unverified. That gate
requires a real LibreOffice executable and fresh profile evidence; XML/cached
value inspection, a launch-only check, or a substitute engine would not close
it. The live Issue requires this limitation to remain explicit at Steward
handback.
