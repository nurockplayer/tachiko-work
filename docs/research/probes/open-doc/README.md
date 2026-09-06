# open-doc report projection: acceptance seed

Owner: [Issue #336](https://github.com/nurockplayer/tachiko-work/issues/336).
This directory is research-only. `report.mjs` is deliberately unimplemented;
this branch must not merge while acceptance is RED. Live repository authority
and the Issue's readiness decision control implementation permission.

## Stage-0 decision

Use a **read-only, batch, exact-source snapshot** of the existing Moonfall
fixture, through the real native Tachiko CLI, followed by an ordinary open-doc
workspace with a synchronous JSON import. Start with HTML; PDF is not required.
No custom Vite plugin, public SDK, browser Worker, second calculation engine,
MCP server, interactive write-back, or upstream patch is needed for this slice.

#232 is completed, not a blocker. Its exported kit is a browser/Worker client;
that topology adds no necessary value to this fixed batch-report experiment.
This does not reopen or duplicate #232. #67/#163 retain their broader decision
and PPTX scope; this is a separately bounded downstream open-doc experiment.

Baseline inspected: Tachiko `3887612f65cc788efa0c23417e7ed16c8289dbb0`.
Upstream inspected: open-doc `68d5d063f332161efa71e907dcfa3e11f59dbb2e`,
`@open-document/core` version `0.6.0`. Inspection is not installation evidence.

## Local process/artifact contract (experimental, not a public API)

```sh
node docs/research/probes/open-doc/report.mjs \
  --source examples/game-balance/game-balance.ro \
  --out /an/absent/report-workspace \
  --tachiko /absolute/path/to/tachiko \
  --theme plain
```

The bounded source profile is a regular direct `.ro` file containing the
existing Moonfall stable schema/fields and original sword. Related canaries
exercise formula edits, an unrelated economy edit, a second sword, and rename.
This is not an arbitrary-dataset renderer or a `.roproj` ingestion project.
Use existing native representation conversions outside this boundary when
needed; never implement a canonical storage reader in JavaScript.

Successful generation creates `projection.json` and
`docs/moonfall/index.tsx` in the output workspace. The template imports JSON
synchronously and uses upstream `DataTable`/page/flow primitives. `plain` and
`compact` are two presentation-only layouts. Source text must remain inert data,
not interpolated executable TSX. With a given theme the TSX source is invariant
under data changes. Use a reviewed local template, no arbitrary external TSX.

`projection.json` has these required observations:

- `profile`: `tachiko.open-doc-research/v0`;
- `source`: exactly `{kind: "snapshot", documentId, sha256}`;
- `schemaId`: the existing weapons stable ID;
- `fields`: `{name, damage, dps}` containing the existing stable field IDs;
- `rows`: `{entityId, name, damage, dps}` records sorted by stable entity ID;
- `metric`: `{entityId, fieldId, value}` bound to the original sword's DPS;
- `validation`: the complete native `analyze validation` JSON, with
  `--source-state sha256:<the exact input-byte digest>`.

The three row properties are report-local projection keys, not semantic IDs.
`support.mjs` records the fixture IDs and obtains independent expected values
from the real CLI. Do not calculate DPS or validate source meaning in JS.
Discover current keys through `analyze document`; resolve each requested stable
subject to its current address in that same snapshot; verify each returned
`analyze field` subject. Human-readable keys are only address material.

Read source bytes once into a private immutable staging input and use that exact
input for every native read. Hash those bytes, not a pathname or Git HEAD. The
SHA-256 identifies the **input bytes**; it is not a semantic revision token,
Approval, live-currentness claim, or proof of who reviewed the data. The CLI's
`source_label` is caller-owned evidence, not revision/mutation authority.

Repeat the same input and template at different output locations without time,
randomness, or absolute staging paths leaking into projection/template bytes.
Every source change updates the snapshot stamp even if the displayed values are
unchanged. Regenerate the entire bounded report; no new dependency graph or
selective invalidation engine is authorized. Unchanged report values do not
establish that arbitrary natural-language claims remain true.

On malformed/semantically invalid input: exit nonzero, emit one JSON error on
stderr with `code: "SOURCE_REJECTED"`, and leave no output workspace. On an
existing output: `code: "OUTPUT_EXISTS"`, nonzero, no modifications. Other
setup/transport/query failures must also fail closed and must not be disguised
as valid/empty/zero values. Preserve the source on every outcome. Keep the
single-writer research host scope explicit; do not claim a new cross-process
atomic persistence protocol.

## Publication observations

Use open-doc **as published**, not a hand-written HTML substitute. The separate
publication suite calls upstream `checkLayout` and `exportDocument` directly,
then inspects the real exported HTML in Chromium with JavaScript disabled and
HTTP(S) requests blocked. No live Tachiko/server connection is needed by the
artifact. No remote assets are used, so this fixture exports one HTML file.

The authored template exposes small test/provenance anchors:

- one `data-tachiko-source-sha256` attribute containing the input digest;
- one `data-tachiko-metric` element whose text is the numeric DPS value;
- one `data-tachiko-validation="valid"` element for native valid evidence;
- for every table value, a `tbody` descendant with both
  `data-tachiko-entity-id` and `data-tachiko-field-id`, whose text is the value.

These are adapter metadata, not new semantic identity families. Existing
`DataTable` custom format callbacks can render the bound cell spans. A React
array-index key alone does **not** conflict with stable semantic identity and
is not grounds to demand an upstream `rowKey` change for read-only rendering.

Call `closeRenderSession()` between separate workspace roots. The inspected
upstream warm-session implementation compares `serverOrigin`; do not assume
that an undefined origin distinguishes two local roots. Record this as a
source-inspected lifecycle constraint, not a reproduced upstream bug report.

## Tests and evidence ownership

```sh
# Fixture preflight: real native CLI; no open-doc install needed.
cargo build -p tachiko-cli --locked
TACHIKO_BIN="$PWD/target/debug/tachiko" \
  node --test --test-name-pattern='^fixture:' \
  docs/research/probes/open-doc/acceptance.test.mjs

# Full projection acceptance; baseline must fail at PROBE_NOT_IMPLEMENTED.
cargo test -p tachiko-cli --test open_doc_acceptance --locked -- --nocapture

# Real renderer acceptance after delivery installs pinned local dependencies.
TACHIKO_BIN="$PWD/target/debug/tachiko" \
  node --test docs/research/probes/open-doc/publication.test.mjs
```

The Rust bridge intentionally includes projection acceptance in existing Cargo
CI without adding/weakening CI workflows or requiring Node packages for that
suite. The native fixture tests must pass before interpreting adapter failures
as behavioral RED. Missing compiler, Node, package, browser, invalid fixture,
or malformed oracle shape is **not** behavioral RED. No expected-failure or
skip annotations are authorized as a way to obtain final GREEN.

The delivery agent owns the minimal private pnpm package/lock needed for the
actual renderer: pin `@open-document/core@0.6.0`, use compatible React and
Playwright, the repository's pnpm version and supported Node, and document the
exact resolved versions. Keep dependencies local to this research directory.
Do not claim those packages are installed or verified from source inspection.
If this upstream version cannot be installed, return concrete resolution
failure evidence instead of silently substituting another version or a mock.

For rendering tests, generated workspaces live under ignored `.scratch/` so
normal ancestor dependency resolution can use the probe's local `node_modules`.
Ensure cleanup in all outcomes. New application logic should follow the repo's
TypeScript preference; the `.mjs` command can remain a thin launcher.

Acceptance mapping:

| Required behavior | Executable evidence |
| --- | --- |
| Native fixture/ID/diagnostic admission | Two `fixture:` cases |
| Real values, validation and source preservation | Exact projection case and per-invocation source comparison |
| Source change and formula authority | Relevant-change and changed-expression cases |
| Unrelated content versus new source snapshot | Unrelated-economy case |
| Stable identity versus display order/key | Duplicate + rename case |
| Presentation separation and repeatability | Theme and repeated-output cases |
| Inert source text | TSX invariance plus real exported Chinese/markup text |
| No plausible partial/empty report or overwrite | Invalid calculation, malformed input, existing-output cases |
| Actual pagination/export/offline content | Three `publication:` cases using real upstream ops + Chromium |

Steward owns these acceptance outcomes. Codex owns the prototype and its unit
tests. Unit tests are applicable and not waived. Read-only source review must
also verify native authority, snapshot staging, error cleanup, no hidden source
parser/calculator, fixed-template safety, and scope. Browser/packaging evidence
cannot be replaced by the native projection suite. Byte-identical HTML/PDF,
interactive editing and real-user adoption tests are not requirements of this
bounded research slice; visual inspection of one real HTML artifact and an
honest maintainer-facing proposal remain delivery evidence.
