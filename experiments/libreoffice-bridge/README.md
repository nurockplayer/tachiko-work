# LibreOffice → Tachiko: saved literal-table analysis

Owner: **Issue #355**. The **live Issue alone controls readiness**. The qualified
seed deliberately returned `BRIDGE_NOT_IMPLEMENTED`; this branch now implements
the bounded bridge, but it is not complete or handback-ready until the required
real Office/native acceptance and fresh exact-head review have passed.

Source-inspected baseline: `8bba9b09cea3c011df383216ba3846ccd003dece`.
Stage-A ancestor: `8a883198bb53fe5559f4b5fa758d56d434f4c082` on
`chatgpt/issue-355-libreoffice-preflight`. The branch retains that ancestry.

## Research decision

The smallest useful next seam is a **local, read-only analysis of one explicitly
selected rectangle in a saved ODS/XLSX file**, not a Calc menu extension, generic
Office importer, formula translator, or another spreadsheet engine.

```text
explicit saved-file request + expected file hash + selection + mapping
  → one bounded byte capture, independently hashed
  → isolated LibreOffice opens a private copy
  → typed cell observations (not display-only values)
  → explicitly mapped, transient direct-ro/v2 candidate
  → existing Rust validate / analyze document / analyze validation
  → new analysis report + truthful fidelity ledger
```

No Core or production CLI command is needed for this experiment. Current
`crates/cli/src/main.rs` exposes all three native commands; `commands.rs` routes
them through `tachiko_storage` and `tachiko_workspace_engine`. The already
merged #341 experiment establishes a precedent for an explicit untrusted DTO
candidate admitted by Rust. Neither Python nor the report becomes semantic
validation authority. No Python formula evaluator or canonical `.roproj` codec
is permitted.

#337/#341's export, editable handoff and actual-saved-artifact Office evidence
remain with their existing owners; do not copy their exporters or reopen #335.
They are not this new ingress's test oracle. A5 (persistent handoff/re-export)
is **not activated** in this read-only slice. Ordinary repository regression
requirements still apply to the final delivery head.

## Fixed experiment profile

The fixture in `canary.py` declares a three-row inventory table with Text `code`,
Number `quantity`, and Number `unit_price`. It includes `00123`, literal `=1+1`,
Unicode, zero, a negative number and a fractional number. A negative quantity
is not automatically invalid: this profile invents no business rule.

The same bounded contract admits 1–100 explicitly mapped rows and 1–16 columns
in one rectangle, including one header row (height 2–101). Positions/dimensions
are non-Boolean integers, start at zero, and must fit Calc's actual sheet.
`mapping.key_header` locates the required Text source-key column. Every header
and data-row source key is non-empty, unique and mapped exactly once. Every
selected cell is covered; no trailing-row/column truncation or silent omission.
Mapping columns/rows must also have unique nonempty target IDs and keys. The
native storage reader remains the final authority over admitted identities.

`BASE_MAPPING` shows the disposable mapping structure. The caller explicitly
assigns new document/schema/field/entity IDs and required field types. IDs are
not derived from A1 locations, sheet names or file hashes; they do not recover
Office history. Reuse of fixed fixture IDs in tests is not reimport/merge or a
cross-project identity guarantee. There is no existing-project input/write path.

| Observed source | Rule |
| --- | --- |
| UNO `TEXT` → mapped Text | Preserve the text exactly; never interpret a leading `=` as a formula |
| UNO `VALUE` → mapped Number | Preserve the observed finite binary64 value, including zero; do not parse displayed strings or coerce Text/Boolean |
| Decimal number format `16` or `17` | Accept the stored number; formatting is presentation-only (`NUMBER`, optionally `DEFINED`) |
| Formula, even with a matching cached value | Reject; no cached-value substitution, parser or evaluator |
| Date/time/currency/percent/logical/other numeric format | Reject as outside this closed experiment, not a general Tachiko limitation |
| True blank, wrong mapped scalar type, nonfinite value | Reject rather than invent a default or repair it |
| Merged cell in the selection | Reject instead of guessing row/column ownership |
| Unknown/duplicate/missing header or row key | Reject the complete analysis |

“Exact scalars” is relative to the values observed by Calc **after its file
import**, not a promise about arbitrary original XML number spellings, metadata,
formula equivalence or lossless whole-workbook migration. Cells outside the
selected rectangle are explicitly unassessed; the report must not say the whole
workbook is validated or safe. Hidden/filter styling is not an exclusion rule:
all physical cells in the explicit selection are considered.

## Unsaved work and source binding

The CLI accepts only `source.kind = "saved-file"`. It analyzes the named file's
persisted bytes, **not the user's current Calc window**. To include unsaved edits,
the user must explicitly save/export a separate copy first. The command neither
attaches to an active Office process nor claims it can detect another process's
unsaved buffer. Requests describing an active document are rejected, never
silently replaced by the previous disk version.

Require a regular, non-symlink `.ods`/`.xlsx` file no larger than 2 MiB and a
lowercase 64-hex expected SHA-256. Capture source bytes once, compare their hash,
and load only a private copy of that captured buffer. Do not hash one file and
then ask UNO to open a moving original. Recheck the original before publication;
an observed mismatch/disappearance rejects `source_changed`. This is ordinary
concurrent-change detection, not an adversarial filesystem-lock/ABA guarantee.

Hash and selection are **source evidence**, never a resident revision, semantic
ID, approval or proof of trust. `analyze --source-state` receives the file hash as
an opaque observation label only. No distributed atomicity claim spans Office,
filesystem and Tachiko. When another process changes a source or wins the output
race, preserve that external result; never claim to roll it back.

## Two executable boundaries

### Native integration seam

```python
bridge.analyze_snapshot(observation, mapping, *, tachiko_bin: str) -> dict
```

This is a disposable internal integration/test boundary, not a supported public
JSON import API. The end-user command must obtain observations from real UNO;
it must not offer an option to substitute caller JSON for Office observation.
The exact observation example is `synthetic_observation()` in `canary.py`.
`source` contains saved-file kind, hash and selection; `rows` is the complete
header/data rectangle. Cell `kind` is UNO's EMPTY/TEXT/VALUE/FORMULA; Text has
`text`, Number has `value` and raw `number_format` flags; `merged` is Boolean.

Successful result:

```text
{status: "analyzed", source: <same observation source>,
 candidate: <explicit direct-ro/v2 DTO>,
 native: {document: <actual native JSON>, validation: <actual native JSON>},
 ledger: [{classification, code, ...}, ...]}
```

The transient candidate crosses real `tachiko validate`, `tachiko analyze
document`, and `tachiko analyze validation` over the same private candidate bytes.
Return the real JSON, not a Python imitation. Keep stdout JSON clean; native
errors/timeout/non-JSON output cannot become success. Temporary candidates are
removed on success/failure. Caller-owned observation/mapping objects are not
mutated. The only native operations in this slice are read/validation commands.

The accepted ledger includes:

| Classification | Required code |
| --- | --- |
| exact | `selected_scalars` |
| strengthening | `explicit_schema` |
| strengthening | `new_identity` |
| presentation | `presentation_not_imported` |
| unresolved | `outside_selection_unassessed` |

A rejected result is `{status: "rejected", code, ledger}` with a same-code ledger
entry and **no candidate/native success fields**. Codes: `invalid_snapshot`,
`invalid_mapping`, `mapping_mismatch`, `unsupported_cell`, `type_mismatch`,
`native_failure`. Selected unsupported content has an `unsupported` ledger entry
with its source locator; no partial success. Detailed diagnostic prose is free.

### Actual saved-file command

```sh
uv run --no-project --no-managed-python --python /path/to/pyuno-python \
  python experiments/libreoffice-bridge/bridge.py --request /absolute/request.json
```

The request is `{source: {kind, path, expected_sha256, selection}, mapping,
output}`. `output` is a new absent JSON file; the parent exists. Environment:
`TACHIKO_BIN` and `REFERENCE_OFFICE_BIN` identify the native CLI and installed
LibreOffice. The request must not select a remote UNO connection.

Exit 0 and return the full successful JSON on stdout and in the output file.
Other outcomes exit nonzero with the rejected JSON on stdout and no newly
published report. Additional host codes: `saved_file_required`, `invalid_input`,
`source_changed`, `invalid_output`. Reject existing/symlink output, source/output
aliasing and destination races. Stage output privately and publish no-replace;
do not expose a partial report or delete another writer's output. Keep all
existing Office files and Tachiko projects untouched.

Use a fresh profile/private local pipe, disable document macros and external
link updates, bound process timeouts and clean only owned processes/files.
`ReadOnly` is supplementary, not a security guarantee. Shared process-lifecycle
helpers may be extracted from the Stage-A harness without changing its oracle;
production must not import `canary.py`, `acceptance.py`, `office_acceptance.py`
or copy expected records. Do not add a production dependency on the tests.
This experiment only handles trusted synthetic inputs. Its load flags, private
profile and tests do not certify hostile-document isolation or network safety.

## Acceptance and commands

| Requirement | Executable evidence |
| --- | --- |
| Actual Office facts match the native fixture | 3 `OfficePreflight` tests: ODS/XLSX, dirty buffer versus disk, moved selection |
| Existing Rust admission/read/reopen seam | 3 `NativePreflight` tests, real Cargo-built CLI |
| A1: typed mapping and actual native authority | `NativeAcceptance` 01–03, 12–14; real forwarding trace + independent native comparison |
| A2/A3: honest mapping, no loss/coercion/partial result | `NativeAcceptance` 04–11; `OfficeAcceptance` 03–04, 08 |
| A4: source/output/other-project preservation | `OfficeAcceptance` 05–07, 09–11; native temp cleanup assertions |
| A6: actual saved file → implemented command → Rust | 11 `OfficeAcceptance` methods, real UNO and real CLI; no macOS GUI claim |
| A5: editable accepted handoff/re-export | Not activated; remains a separate evidence-triggered slice |

```sh
# Stage A, unchanged previous qualification
REFERENCE_OFFICE_BIN=/path/to/soffice
uv run --offline --no-project --no-managed-python --python /path/to/pyuno-python \
  python experiments/libreoffice-bridge/office_preflight.py \
  --office "$REFERENCE_OFFICE_BIN" --output /tmp/lo355-new-stage-a

# New oracle self-check (not engine evidence)
uv run --no-project --no-managed-python python \
  experiments/libreoffice-bridge/acceptance.py --mode oracle

# New real Office source qualification; no native dependency
export REFERENCE_OFFICE_BIN=/path/to/soffice
uv run --offline --no-project --no-managed-python --python /path/to/pyuno-python \
  python experiments/libreoffice-bridge/office_acceptance.py \
  --mode preflight --evidence /tmp/lo355-new-preflight.json

# Real native preflight + implemented native bridge acceptance under existing CI
cargo test -p tachiko-cli --test libreoffice_bridge_acceptance --locked

# Final actual end-to-end Office gate AFTER implementation
cargo build -p tachiko-cli --locked
export TACHIKO_BIN="$(pwd)/target/debug/tachiko"
export LIBREOFFICE_BRIDGE_HEAD="$(git rev-parse HEAD)"
uv run --no-project --no-managed-python --python /path/to/pyuno-python \
  python experiments/libreoffice-bridge/office_acceptance.py \
  --mode acceptance --evidence /tmp/lo355-new-final.json
```

The Cargo bridge deliberately follows the existing repository's stdlib Python
subprocess test pattern; there is no new Python dependency or CI workflow.
`cargo test` native mode does **not** run UNO or the saved-file command. The local
Office preflight does **not** run Rust. Both distinctions must remain visible.
Missing binaries, setup/compile errors or wrong fixture expectations are not RED.
A successful native subset alone is not completion of this Issue.

## Runnable synthetic command/report example

This is a disposable, local demonstration of the exact closed profile. It uses
the Steward-owned synthetic fixture helper to create a new ODS file in fresh
scratch space; the production bridge imports none of these helpers. Set `PYUNO`
to the ABI-compatible Python supplied with the selected LibreOffice installation
(not an arbitrary system Python), then run from the repository root:

```sh
export REFERENCE_OFFICE_BIN=/path/to/soffice
export PYUNO=/path/to/LibreOffice-compatible-python
export REPO_ROOT="$(pwd)"
export TACHIKO_BIN="$REPO_ROOT/target/debug/tachiko"
export EXAMPLE_DIR="$(mktemp -d)"
cd "$REPO_ROOT/experiments/libreoffice-bridge"

"$PYUNO" - <<'PY'
import hashlib, json, os
from pathlib import Path
from canary import BASE_MAPPING, create_inventory
from office_preflight import Office

root = Path(os.environ["EXAMPLE_DIR"])
source = root / "inventory.ods"
office = Office(os.environ["REFERENCE_OFFICE_BIN"], root)
try:
    selection = create_inventory(office, source)
finally:
    office.close()
request = {
    "source": {"kind": "saved-file", "path": str(source),
               "expected_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
               "selection": selection},
    "mapping": BASE_MAPPING,
    "output": str(root / "report.json"),
}
(root / "request.json").write_text(json.dumps(request) + "\n", encoding="utf-8")
PY

"$PYUNO" bridge.py --request "$EXAMPLE_DIR/request.json" \
  > "$EXAMPLE_DIR/stdout.json"
cmp "$EXAMPLE_DIR/stdout.json" "$EXAMPLE_DIR/report.json"
shasum -a 256 "$EXAMPLE_DIR"/inventory.ods "$EXAMPLE_DIR"/report.json
```

On a correctly provisioned host, the command exits zero, the two JSON files are
byte-identical, and the report has `status: "analyzed"`, actual native document
and validation JSON, and the five required ledger entries. This is a synthetic
example only; it neither validates the whole workbook nor establishes Office
interoperability outside this profile.

Use Ruff for implementation lint/format and the live repository's final-head
checks. Add delivery-agent unit tests independently of this acceptance. Actual
Office/native end-to-end evidence and fresh independent Guarded review are
required before handback; neither can be substituted by the preflight or by
matching cached values. No failing seed merge, skipped-test green, self-merge,
upstream contact, extension publication or partnership claim.

## Evidence and finite verdict

Historical Stage-A results remain in `evidence-summary.json`; they are not a
new native result. The qualified seed recorded Stage A 6/6 PASS, real Office
preflight 3/3 PASS, oracle self-checks 3/3 PASS, and hosted native qualification.
At implementation time the delivery agent reproduced the Steward native bridge
acceptance against a real Cargo-built `tachiko` (17 tests: 3 preflights plus 14
bridge methods), added delivery-owned unit coverage, and ran Ruff and Python
compilation. Those native results do not substitute for actual Office evidence.

The current macOS host has LibreOffice 26.8.0.3 installed but no runnable
ABI-compatible `pyuno` interpreter: its system Python cannot import `uno`.
Therefore the new real Office preflight and full saved-file command acceptance
are **not run here** and must remain recorded as incomplete rather than green.
The live PR/handoff, not this static document, records the exact current SHA,
hosted CI, review state, and any later Office report hashes.

**Finite verdict:** the closed literal-table seam is useful only when a saved
file, explicit mapping, and real UNO observation can be verified end to end.
It adds a small Python/UNO maintenance surface and version-sensitive runtime
prerequisite; it does not justify a general Office importer, active-document
integration, persistent handoff, or product adoption. Any broader profile needs
new evidence and Steward authority.

## Official source trail

- XCell channels and the Text→getValue() zero trap: https://api.libreoffice.org/docs/idl/ref/interfacecom_1_1sun_1_1star_1_1table_1_1XCell.html
- NumberFormat constants: https://api.libreoffice.org/docs/idl/ref/NumberFormat_8idl.html
- Logical ReadOnly / macro / update load options: https://api.libreoffice.org/docs/idl/ref/servicecom_1_1sun_1_1star_1_1document_1_1MediaDescriptor.html
- Save-copy versus changing document location: https://api.libreoffice.org/docs/idl/ref/interfacecom_1_1sun_1_1star_1_1frame_1_1XStorable.html
- Loaded-document modified state: https://api.libreoffice.org/docs/idl/ref/interfacecom_1_1sun_1_1star_1_1util_1_1XModifiable.html

Docs inspected on 2026-09-10 describe the 26.8 API reference, not the installed
25.2 binary qualification. Accepted repository authority remains controlling;
this closed profile creates no public SDK, source format or permanent limit.
