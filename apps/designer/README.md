# Tachiko Designer vertical slice

This is the first production graphical projection and persistence composition
over the resident Rust workspace runtime. It intentionally supports one narrow
product path: open one canonical Moonfall-shaped `.roproj/v1`, browse a typed
collection, edit a directly stored Number, publish against the expected
revision, selectively refresh affected fields and dependent formulas, Save As
to a new browser-local destination, then destroy and reopen the occurrence.

The Worker retains the Rust-authoritative workspace occurrence. Browser state
contains only bounded, revision-keyed projections and edit buffers. The
app-local JSON DTOs and raw WASM ABI are private delivery mechanics, not a
public SDK, stable wire protocol, canonical document model, or storage format.

Open sends only selected path/byte records through a separately bounded private
project arena. Rust `tachiko-storage` performs exact canonical admission and the
workspace performs complete validation before a fresh occurrence replaces the
current one. The browser host supplies a fresh cryptographic occurrence token
for every demo or Open, so document-scope authority is not reused across Worker
or page lifetimes. Rejected admission leaves the current occurrence unchanged;
if projection transfer fails after successful replacement, the unusable new
occurrence is closed and the failure remains visible. If teardown cannot be
confirmed, the client is stopped and the UI fails closed until reload.

Save As captures one exact `ResidentWorkspaceSession::export_snapshot()`
revision and encodes it through the existing canonical `.roproj/v1` codec.
IndexedDB commits the opaque complete tree as one create-only record, so an
existing project name is never overwritten and the UI marks only the confirmed
revision durable. Browser projects survive Worker teardown and page reload, but
remain browser-origin data and can be removed by clearing site data. Same-name
replacement, autosave, recovery/history, cloud persistence, and distribution
remain outside this slice.

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
Schema/entity authoring, formula authoring, overwrite/update-in-place,
autosave/recovery, and public transport/storage/SDK stabilization are outside
this slice.
