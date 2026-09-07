# open-sheet human-handoff experiment — Steward acceptance

Owner: Issue #341; parent #335. **Tests-only until the live Issue says Ready.**
No adapter implementation, public SDK, production adoption or upstream contact
is authorized by publishing this seed. Continue the same branch for one eventual
Draft delivery PR; do not merge a failing acceptance seed on its own.

## Decision and fixed demonstration

Source baseline: Tachiko `02f09391327564875dd09c56c582b7374443379a`;
open-sheet live main `6ab72ddb50bb87dad11cc321d87b5e9bcc3c5689`.
Reuse the already-proven isolated package, lockfile and provenance in
`../open-sheet-export`; do not add another package, tarball or application runtime
dependency. `model.mjs` is repository-owned trusted authoring through public
Workbook/Sheet/Table/col/ref/compile calls, equivalent to the JSX authoring API.
No arbitrary user module, TSX, script or source-string evaluation is accepted.

One quarterly plan: tax 0.25; January revenue/cost 1200/400, February 1600/800,
March 2000/1000. Gross = revenue - cost. Net = gross * (1 - tax).
Base net = 600/600/750. Human changes tax to 0.5; net = 400/400/500, with gross
800/800/1000 unchanged. Independent expectations reuse #337's fixed canary,
never results from the new converter or an evaluator under test.

## Ingress contract — disposable, not a new interchange standard

`proposeHandoff(compiledWorkbook, mapping)` in `handoff.mjs` returns
`{status: "proposal", document, ledger}` or throws a structured rejection.
The document is a direct-ro/v2 **candidate**, not canonical authority or an
accepted workspace. The function is side-effect-free and does not evaluate
formulas or mutate caller-owned objects. Both sync and Promise returns work.

`mapping.json` fixes two blocks, schemas, required Text/Number fields and fresh
Tachiko identities for this isolated demonstration. The grid's exact Text labels
January/February/March locate rows only during this handoff. Public registry
anchors locate cells; exact symbolic references map those cells to assigned
entity/field targets. Duplicate/missing/unknown row labels and duplicate target
IDs fail. Source row/column/sheet order, sheet spelling and layout origin are
not identity. Do not claim these IDs recover upstream history, and never use
this mapping to merge/reimport into an existing project.

Only finite numeric literals, direct scalar Text/Number, binary + - * / and
symbolic references to explicitly mapped numeric data cells/key-value entries
are translated. No string-to-number coercion, default missing values, A1/raw
formula parser, ranges, functions, spill, dates, external refs or generic
spreadsheet import. Unclaimed semantic/formula content fails, not drops.
Headers/key-value labels and formatting are presentation only; they do not
create types or constraints. Formula/value role collisions and unsupported
metadata affecting meaning must fail rather than be inferred away.

Required ledger classifications/codes:

| Classification | Code | Meaning |
| --- | --- | --- |
| exact | closed_formula_profile | Syntactic translation within the selected arithmetic profile, subject to Rust admission |
| strengthening | explicit_schema | Required fields/types are explicitly proposed, not inferred from formatting |
| strengthening | new_identity | IDs are newly assigned by this mapping, not historical source IDs |
| presentation | presentation_not_semantics | Header/layout/style is not native semantic authority |

Rejections throw `{code, ledger}`; ledger contains an `unsupported` entry with
the same code; no partial document. Codes: `unsupported_value`,
`unsupported_formula`, `unmapped_source`, `invalid_mapping`. Cycles and other
native semantic failures belong to Rust validation/admission; the converter
must not add a second graph evaluator to detect them.

## Native and public-package acceptance

```sh
pnpm --dir experiments/open-sheet-export install --frozen-lockfile
cargo test -p tachiko-cli --test open_sheet_handoff_acceptance --locked
cargo test --manifest-path apps/designer/runtime/Cargo.toml \
  --test open_sheet_handoff_preflight --locked
```

The first bridge executes `acceptance.test.mjs` with the real built CLI. It
proves public compilation, independent native canary admission and conversion
behavior (including changed numbers/formulas, layout changes and rejection).
The second test proves this exact canary fits the existing human runtime,
changes only the intended field, returns exactly the three affected net
calculations, rejects stale/invalid input, preserves exported bytes on rejection,
and reopens equal canonical bytes under a fresh occurrence.

Existing hosted CI already installs the pinned open-sheet consumer and runs
both owning test suites; no workflow change is needed. Missing tools/packages,
compile failures or fixture defects are not behavioral RED. The deliberate
`HANDOFF_NOT_IMPLEMENTED` exception identifies the currently missing ingress.

## Human-facing surface and persistence

Use one tiny experiment-local browser client over the exported #232 kit only,
not the main Designer UI, app-private imports, raw WASM, new server API or new
semantic engine. A local trusted preparation command may compile the fixed
model, propose its mapping and invoke existing Rust CLI validation/materialization
into a new scratch canonical directory. Keep the original source immutable.
Generate/export the kit through `scripts/export-experimental-designer-client.sh`.
Serve only the generated scratch demo on loopback, not arbitrary filesystem or
mutation endpoints. Browser packaging may use the exported `projectTransferFromFiles`
helper over opaque canonical file bytes; never reimplement their semantic codec.

Preview shows what is exact, what is strengthened/newly identified, what is
presentation-only, and unsupported findings. Accept opens exactly that frozen
candidate through the kit; Cancel leaves no accepted occurrence and no editable
work. An existing accepted occurrence must never be silently replaced.

After acceptance, ordinary controls edit tax via `editNumber` with the queried
revision. Render current values, validation and a **last accepted change/impact**
card using `PublicationProjection.fields/affected_calculations` and pinned
before/after `queryFields` results. Never calculate net or dependency membership
in the UI. An earlier draft retains its original revision and fails stale;
invalid numeric text must reach the Rust command and fail `invalid_number`.
Rejected edits do not alter canonical bytes or replace the last successful card.

Save the exact `exportProject` bytes to an experimental local file; reopen those
bytes through `openProject` in a fresh client/occurrence. This is a bounded use
of the existing private host transfer, not a new supported file format. Reopened
values/IDs/formulas survive; resident revision strings can restart and session
history need not survive. Do not label this durable audit history, collaborative
merge or a complete long-lived history product.

The real saved bytes must feed the #337 exporter and new Office evidence, not
a regenerated original canary. Host-only unpacking of the pinned private
path/byte transfer is allowed in test/support code; Rust must still validate
canonical files. No JavaScript/Python semantic storage codec is authorized.

## Browser and actual-saved-artifact acceptance

Implement a trusted `prepare-demo.mjs --output <new directory>` inside this
experiment. It compiles `model.mjs`, creates the proposal, validates/materializes
its frozen candidate with the CLI, exports the existing client kit into
`vendor/tachiko`, and stages one small HTML/TypeScript demo plus opaque candidate
files. It never publishes into a live project. It must reject existing output
and clean failed staging. Keep generated assets, node_modules and evidence out
of Git. No third-party dependency upgrade or new package is needed.

After that entry point exists, the repeatable delivery sequence is:

```sh
cargo build -p tachiko-cli --locked
export TACHIKO_BIN="$(pwd)/target/debug/tachiko"
pnpm --dir apps/designer install --frozen-lockfile
pnpm --dir apps/designer exec playwright install chromium
# Choose a new absent absolute directory under an existing scratch parent.
node experiments/open-sheet-handoff/prepare-demo.mjs --output /tmp/issue341-demo
# Run this static loopback host in a separate terminal; stop it after testing.
pnpm --dir apps/designer exec vite /tmp/issue341-demo \
  --host 127.0.0.1 --port 4179 --strictPort
```

```sh
export HANDOFF_HEAD="$(git rev-parse HEAD)"
export HANDOFF_DEMO_URL=http://127.0.0.1:4179
export HANDOFF_ARTIFACT_DIR=/tmp/issue341-browser-evidence
node --test experiments/open-sheet-handoff/browser-acceptance.mjs
export REFERENCE_OFFICE_BIN=/absolute/path/to/real/soffice
uv run --no-project --no-managed-python python \
  experiments/open-sheet-handoff/artifact_acceptance.py \
  --artifacts "$HANDOFF_ARTIFACT_DIR" --output /tmp/issue341-office-evidence --mode office
```

The browser test supplies the precise accessible controls/test IDs. These are
bounded demo affordances, not new production design-system obligations. It drives
consent/cancel, editing, impact, stale/invalid rejection, download and reopen.
A separate real public client independently admits the downloaded bytes; a fake
UI success message is insufficient. Input is textual so invalid numeric text
can reach Rust instead of being silently blocked by an HTML number control.

The artifact test checks those exact before/after hashes against browser evidence
and the current Git HEAD, extracts only the bounded private path/byte carrier
into fresh scratch directories, and asks native Rust to validate and inspect all
16 expected fields and six formulas. It passes these real projects to the
unchanged #337 exporter, checks live uncached formula structure, and recalculates
both outputs in fresh Office profiles. `--mode structural` is useful locally but
is explicitly NOT the Office gate. No old sample.xlsx or regenerated canary can
replace actual browser output. The carrier's six stdlib self-tests run with:

```sh
uv run --no-project --no-managed-python python \
  experiments/open-sheet-handoff/artifact_acceptance.py --self-check
```

### Requirement-to-evidence map

| Gate | Steward-owned evidence |
| --- | --- |
| Real public source and independent native model | Two `preflight:` cases in `acceptance.test.mjs` |
| Exact ingress and truthful strengthening/rejection | Eleven `handoff:` cases in the same suite |
| Existing runtime supports edit/currentness/impact/reopen | `open_sheet_handoff_preflight.rs` |
| Human consent, editing, change display and saved state | `browser-acceptance.mjs`, actual downloads and screenshots |
| Changed saved model remains the export source | `artifact_acceptance.py`, native queries and independent OOXML |
| Real downstream arithmetic, not cached answers | Same artifact test with `--mode office` and recorded binary/version |
| Partner value and honest limitation claims | Fresh reviewer inspects the running demo/screenshots and records whether a reader can identify input change and three impacts; maintainer interest is NOT inferred |

### Evidence ownership, checkpoints and stopping rules

Terra owns environment setup and all external gates on its local or hosted test
runner after implementation: existing pinned public-package install, exported
kit, Chromium and real LibreOffice. Reuse available validation binaries first;
installation of Chromium/LibreOffice from trusted standard distribution channels
is authorized for this test environment only. Record versions and complete
commands. Missing tools remain blocked, not skipped passes. This is not permission
to add production dependencies, change global security settings or contact upstream.

Material checkpoints: (1) reproduce native/public preflight and ingress RED;
(2) implement the closed converter with unit tests and turn ingress green;
(3) run actual browser/save/reopen and saved-artifact Office gates; (4) fresh
read-only Sol final-head review, repair actionable findings, then Steward handback.
No routine pause between successful in-scope checkpoints. A real contradiction
with Accepted semantics/kit/storage, a required wider profile/dependency, or an
unavailable required environment after bounded recovery returns to Steward.
Never alter an oracle to manufacture a passing implementation.

Guarded: identity/type/ingress, publication/currentness, persistence and export
correctness. One coherent disposable vertical slice, normally under 15 files /
2,000 handwritten lines; report a forecast overrun before expanding scope.
Core/formula/storage meaning, public kit contracts, first-party launch surfaces,
and #337 acceptance stay unchanged. One eventual implementation PR is justified
because isolated fragments cannot prove the handoff value end to end.

Terra High coordinates; Luna Medium may implement bounded adapter/UI modules and
unit tests; fresh independent Sol High reviews the exact final material head.
Endpoint: one green reviewed Draft PR, truthful Office/manual evidence, and a
finite verdict handed back to Steward. No self-merge, production adoption,
package publication, upstream PR or maintainer outreach. Passing this demo does
not authorize generic import/sync, durable audit history or adoption under #335.

## Evidence status at seed publication

Real hosted run `34097937190`, exact seed `3d6ec690efe126916c39a2183f478e2ee6a0ec40`:
public compiler and independent native preflights PASS (2); all eleven converter
cases reach `HANDOFF_NOT_IMPLEMENTED` and FAIL as intended, with zero skips.
The old #337 exporter regression and 62 CLI tests also PASS. Overall CI is not
green: the intended new Cargo acceptance failure blocks downstream steps.
An initial runtime formatting issue and subsequent redundant-closure Clippy issue
were seed defects, NOT behavioral RED; both are corrected on this branch and
must be requalified. The live Issue carries the exact-head qualification result.

Local Node syntax checks and six private-carrier oracle self-checks PASS; Python
compilation PASS. The local container has Node 22 but no Rust/pnpm/GitHub DNS;
local syntax checks are not Node-24 repository runtime qualification. Browser,
actual-saved-artifact and Office tests have not run before the implementation
exists. Their explicit owner/checkpoint plan above is not execution evidence.
