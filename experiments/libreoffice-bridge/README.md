# LibreOffice collaboration qualification — Issue #355

**Research / tests-only.** Live Issue #355 owns scope and readiness. This is not
an implemented Calc-to-Tachiko bridge or a product-acceptance pass.

## Reproduce Stage A

Use an installed LibreOffice and its ABI-compatible Python with `import uno`.
A generic uv-managed Python does not automatically include pyuno. On the tested
Linux environment `/usr/bin/python3` provides the distribution's UNO binding:

```sh
uv run --offline --no-project --no-managed-python --python /usr/bin/python3 \
  python experiments/libreoffice-bridge/office_preflight.py \
  --office /usr/bin/soffice --output /tmp/lo355-new-run
```

The output directory must be absent and its parent must exist. On macOS, select
an actual Python/UNO combination and installed `soffice`; do not assume the Linux
paths work. Missing bindings or connection failure are environment failures,
not behavioral RED. This seed makes no macOS GUI/packaging claim.

Only synthetic documents authored inside the script are opened. The runner uses
a fresh profile, a unique local named pipe (no TCP listener), explicit
NEVER_EXECUTE / NO_UPDATE load settings, a 90-second Office-process watchdog,
and cleanup restricted to its own process group/temp profile. These precautions
are not an adversarial sandbox qualification. No unknown input path is accepted.

The output contains generated ODS/XLSX files, `office.log`, and full observations
in `evidence.json`. Keep these under scratch storage; the generator and compact
execution manifest are tracked, not binary samples. Re-running reconstructs the
corpus. ZIP metadata may differ; compare the bounded observations rather than
requiring byte-identical packages. File hashes identify artifacts from one run.

## Six executable checks

| Check | Independent expected observation | Limit |
| --- | --- | --- |
| Literal/type round trip | `00123` and `=1+1` remain Text; Unicode and 42.5 survive; true blank, empty formula result and zero remain distinguishable | TRUE/FALSE are formula-result probes, not proof of native Boolean import |
| Quarterly-plan round trip | ODS → XLSX → ODS preserves six live formula cells; changing tax 0.25 → 0.5 changes net 600/600/750 → 400/400/500; changed XLSX reopens with those results | Hand-derived #337 canary arithmetic, not general formula-tree equivalence or Excel certification |
| Date/percent/error round trip | Explicit 1899-12-30 origin; serial 46274 displays 2026-09-09; 0.25 displays 25.00%; divide-by-zero remains an error, not blank | No general Date/Currency/error migration authorization |
| ReadOnly API behavior | A logically read-only document can still be changed through UNO while the source file stays unchanged without saving | Proves why the flag alone is not an integrity boundary |
| Existing-output protection | A sentinel destination is refused and preserved | Test harness behavior, not a product transaction or race-safety proof |
| Read observation preservation | Repeated reads preserve observed cells, dirty state and source bytes | Synthetic single-session observation, not concurrent-save safety |

The first three checks observe the freshly authored workbook and reopen each
format transition. All previously generated source hashes are checked during
conversion; changed-input recalculation does not overwrite its source files.

The observations come from real UNO/Calc, not Python's formula evaluation. The
script does not parse or evaluate formulas, translate Office into `.ro`, or
modify existing Tachiko acceptance. It deliberately demonstrates that Calc's
cell type/display/value are different information channels.

## Actual evidence

`evidence-summary.json` records the tested script hash and six passing checks on
LibreOffice **25.2.3.2 520(Build:2)** / Linux x86_64 / distribution Python 3.13.5.
The upstream API documentation inspected on 2026-09-09 describes 26.8; that does
not mean this executable test ran on 26.8. Python compilation also passed.

Native Tachiko, Rust compilation, installed open-sheet execution, macOS GUI,
Ruff, full repository gates and independent final-head review were **not run in
this preparation environment**. Rust/cargo/tachiko/pnpm/Ruff were unavailable,
and a GitHub clone failed due to container DNS. Connected GitHub reads/writes
worked separately. These are disclosed environment limits, not red product
behavior. Historical #337/#341 passes are prior evidence, not reruns here.

## Stage B: existing native evidence, not a new implementation lane

From a normal complete checkout, follow CONTRIBUTING.md for pinned tools, then
reuse existing owners rather than adding another exporter or formula oracle:

```sh
pnpm --dir experiments/open-sheet-export install --frozen-lockfile
cargo test -p tachiko-cli --test open_sheet_acceptance --locked
cargo test -p tachiko-cli --test open_sheet_handoff_acceptance --locked
cargo test --manifest-path apps/designer/runtime/Cargo.toml \
  --test open_sheet_handoff_preflight --locked
cargo build -p tachiko-cli --locked
export TACHIKO_BIN="$(pwd)/target/debug/tachiko"
export REFERENCE_OFFICE_BIN=/absolute/path/to/installed/soffice
export OPEN_SHEET_POC_COMMAND='["node","experiments/open-sheet-export/export.mjs"]'
uv run --no-project --no-managed-python python \
  experiments/open-sheet-export/acceptance.py --mode office
```

Read the existing export/handoff READMEs for their complete requirements and
actual-saved-artifact checks. Their native/public-package/Office results are
separate evidence classes; the five commands above alone do not prove the new
Calc ingress or GUI. Record exact source/HEAD and binary/package provenance.

Next inspect the smallest saved-Calc-snapshot → explicit new candidate seam.
Report which existing Rust admission and read-only semantic query surfaces can
be exercised without an Office formula parser, automatic schema inference or
Core changes. Return the exact gap and source/mapping evidence to Steward.
**Do not create the production adapter or invent replacement acceptance.**
Steward owns the additional executable A1–A6 acceptance before implementation
readiness. Native preflight cannot promote its own assumptions into authority.

A future accepted candidate has newly assigned identities. Ordinary Office
files do not preserve Tachiko schema, history or authorization merely because
values look the same. Unsaved Calc edits, stale source hashes and unsupported
formulas need explicit rejection or a separate user-approved saved copy—not
silent fallback to cached values or a previous disk version.

## Source trail

- Live scope and references: Issue #355.
- Existing arithmetic/export: `../open-sheet-export/canary.json` and acceptance.
- Existing human handoff: `../open-sheet-handoff/`; #335 final disposition.
- UNO API: https://api.libreoffice.org/
- Load/save/ReadOnly caveat: https://api.libreoffice.org/docs/idl/ref/servicecom_1_1sun_1_1star_1_1document_1_1MediaDescriptor.html
- Add-on entry point: https://help.libreoffice.org/latest/en-US/text/shared/guide/integratinguno.html
- Licensing, not clearance: https://www.libreoffice.org/licenses/
