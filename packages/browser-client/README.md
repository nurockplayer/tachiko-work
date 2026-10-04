# Experimental browser client producer

This package builds and qualifies Tachiko Work's experimental browser client
and standalone Rust/WASM runtime. It is a technical producer boundary, not a
first-party application, stable SDK, published package, or compatibility
promise.

From the repository root, install the pinned toolchain and run its checks:

```sh
pnpm --dir packages/browser-client install --frozen-lockfile
pnpm --dir packages/browser-client exec playwright install chromium
bash scripts/browser-client-check.sh
```

The browser check runs the external consumer workflow from
`examples/experimental-designer-client` against an exported kit. The producer
also retains direct runtime, source identity, manifest, notices, no-clobber,
and strict consumer canaries. Its emitted `designer_runtime.wasm` filename and
Rust crate ABI identity remain compatibility bytes.

The producer supports the private [bounded Text capacity profile](../../acceptance/bounded-text-capacity/CONTRACT.md):
one headered table with three Text fields and at most 8,406 rows, plain styles,
no column widths, formulas, constraints or saved calculation definitions.
Admission proves finite complete projections, exact Text preservation and both
CSV/XLSX export closure before installing a candidate or publishing an edit.
The existing generic and native Tracker profiles retain their own limits.
Capacity Text edit requests retain proposal evidence only for that request; the
existing 64-operation session Undo horizon is unchanged.

The ordinary Designer request ABI checks 64 KiB before parsing. The spreadsheet
ABI caps its arena at 4 MiB, then uses a bounded operation-kind discriminator;
above 64 KiB it admits only existing Export/InspectProject operations before
decoding their complete payload. Other controls and selective field replies
remain at 64 KiB. Capacity artifacts require a capacity-capable reader; downgrading
the producer cannot make the old pin support them. Producer qualification and
the separate Sheet import/edit/durable save/restart/reopen qualification remain
distinct, and this change does not select v3 or remove the Date + definition guard.
