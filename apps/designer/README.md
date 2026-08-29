# Tachiko Designer vertical slice

This is the first production graphical projection over the resident Rust
workspace runtime. It intentionally supports one narrow product path: browse a
typed collection, edit a directly stored Number, publish against the expected
revision, and selectively refresh affected fields and dependent formulas.

The Worker retains the Rust-authoritative workspace occurrence. Browser state
contains only bounded, revision-keyed projections and edit buffers. The
app-local JSON DTOs and raw WASM ABI are private delivery mechanics, not a
public SDK, stable wire protocol, canonical document model, or storage format.

## Development

Use pnpm exclusively:

```sh
pnpm --dir apps/designer install --frozen-lockfile
pnpm --dir apps/designer dev
pnpm --dir apps/designer lint
pnpm --dir apps/designer typecheck
pnpm --dir apps/designer test
pnpm --dir apps/designer build
pnpm --dir apps/designer exec playwright install chromium
pnpm --dir apps/designer test:browser
```

`build` compiles the private Rust adapter for `wasm32-unknown-unknown` before
Vite assembles the application. `dev` performs the same runtime build before
starting Vite, so a clean checkout never serves without its Worker artifact.
Persistence, schema/entity authoring, formula
authoring, and public transport/SDK stabilization are outside this slice.
