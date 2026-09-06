# open-sheet export acceptance seed

Owner: [Issue #337](https://github.com/nurockplayer/tachiko-work/issues/337),
first bounded child of [#335](https://github.com/nurockplayer/tachiko-work/issues/335).
**Execution authorization follows the live Issue.** Repository Accepted
authority and the live Steward readiness decision outrank this harness.
`export.mjs` is a deliberate unimplemented seed, not an exporter. Continue
this branch for one delivery PR when Ready; never merge the failing seed alone.

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
executable to run `native_ci.py` under the existing push CI. No workflow file
is changed. This stdlib-only bridge uses the runner's Python 3 directly; it
installs no Python dependency and does not require uv on the hosted image.

```sh
cargo test -p tachiko-cli --test open_sheet_acceptance --locked
```

The native subset adds schema/scalar query evidence and tests the exporter
process. It intentionally does not select the public-package probe or Office
case; these remain separate mandatory delivery gates, not skipped passes.
The default process is the checked-in `export.mjs`, which currently reports
`PROBE_NOT_IMPLEMENTED`. After Ready, implement that entry point (and local
modules) rather than mocking the test reader or changing expected outcomes.

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

## Actual seed evidence, 2026-09-07 JST

Local checks: 8 oracle self-checks PASS; Python syntax and Node syntax PASS.
The first local runtime attempt ran zero tests because TACHIKO_BIN was absent;
that initial environment failure was not used as behavioral RED.

The later [hosted native run](https://github.com/nurockplayer/tachiko-work/actions/runs/34063666072/job/101568604416)
executed at exact code seed `5566d30bf12fc5c1620298551d8267e069ecdf8d`:

- 3 native query/admission/edit/rename preflights PASS; two are also repeated
  successfully by the export class (5 passing test executions in total).
- All 13 exporter test methods reached the unimplemented process and FAIL as
  expected. Including operator/layout subcases, unittest reports 18 methods
  run and 19 failure records. There are no setup errors in this hosted run.
- Rust compilation, formatting, docs consistency, repository tooling,
  dependency-layer check and Clippy PASS before the intended test failure.
  The existing 62 CLI tests also PASS. Cargo's overall test step FAILS;
  downstream steps blocked behind it are not claimed green.

This is native boundary and missing-exporter RED evidence, not proof that
open-sheet runs or XLSX calculations match an office engine. Public-package
execution, real Office recalculation, production adapter/unit tests and fresh
independent implementation review remain mandatory and unverified. The current
README update changes no executable test or fixture from the tested code seed.

The initial local container lacks Rust/Cargo/pnpm and cannot resolve github.com;
GitHub connector source/branch access and existing hosted CI supplied the native
execution evidence. No production dependency, semantic engine, SDK, UI, storage
or CI workflow changes are included. The live Issue records the Steward's
readiness decision and the bounded remaining dependency/Office evidence plan.
