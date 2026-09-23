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
