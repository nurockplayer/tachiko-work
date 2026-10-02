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

The experimental client offers `exportProjectV3(expectedRevision)` to explicitly
select an opaque canonical `.roproj/v3` project transfer. It supports Date and
saved grouped-sum definitions together, with fresh admission and projection
budgets checked before export. Use the existing `inspectProject` and
`openProject` methods for these bytes. A successful v3 open requires explicit
v3 export for subsequent saves; ordinary export refuses that occurrence.

The origin marker belongs to the live occurrence, survives edits and history
operations, and resets on successful New, Import, legacy open or Close. Keep a
v3-capable reader available when rolling back a producer that has emitted v3
artifacts. Sheet host save/restart qualification remains a separate consumer gate.
