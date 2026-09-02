# Experimental Designer client kit: first contact

Status: bounded Issue #232 experiment. This is not a stable/public SDK, npm
package, wire protocol, plugin ABI, or compatibility promise. The kit may
change or disappear after the first external-frontend pilot.

The browser frontend receives **projections**: derived semantic facts intended
for rendering. Every edit carries an **expected revision**, so a stale UI cannot
silently overwrite a newer authoritative state. The resident Rust **runtime is
the authority**: frontend code never parses semantic meaning, recalculates
formulas, validates candidates, or owns canonical state.

## Export and vendor the kit

From a clean Tachiko Work checkout with Rust, `wasm32-unknown-unknown`, and pnpm
11.25.0 installed:

```sh
bash scripts/export-experimental-designer-client.sh \
  /path/to/external-ui/vendor/tachiko
```

The destination must be absent or empty. The command compiles and copies one
self-contained browser kit:

```text
vendor/tachiko/
├── experimental-client.js       # intended ESM entry
├── experimental-client.d.ts     # TypeScript surface
├── experimental-client.worker.js
├── designer_runtime.wasm
├── host/                         # generated private support modules
├── runtime/                      # generated private support modules
├── package.json                  # ESM/artifact metadata; private, unpublished
└── README.md                     # instability and authority boundary
```

Keep the directory together and import only `experimental-client.js`. Do not
call the Worker support modules or WASM exports directly.

## Open Product Gap and render the first table

Use the repository-owned sample at
[`dogfood/product-gaps.roproj`](../../dogfood/product-gaps.roproj/). A browser
directory input supplies its opaque path/byte records; `projectTransferFromFiles`
does not parse `.roproj`, and Rust performs canonical admission.

```html
<input id="project" type="file" webkitdirectory />
```

```ts
import {
  DesignerRuntimeError,
  createExperimentalDesignerClient,
  projectTransferFromFiles,
} from "./vendor/tachiko/experimental-client.js";

const input = document.querySelector<HTMLInputElement>("#project")!;
const client = createExperimentalDesignerClient();
const transfer = await projectTransferFromFiles(input.files!);
const opened = await client.openProject(transfer);
const table = await client.queryTable(opened.bootstrap.default_collection);

for (const row of table.rows) {
  console.log(row.key, row.fields); // typed stored/calculated/diagnostic projections
}
```

`openProject` transfers its input buffer to the Worker. Copy the buffer first if
your host needs to retain it.

## Publish one revision-safe edit and observe authority

Targets come from projections and contain stable semantic IDs. Do not construct
targets from table indexes, paths, or JSON pointers.

```ts
const row = table.rows.find((candidate) => candidate.key === "designer_profile_bound")!;
const impactColumn = table.columns.find((column) => column.key === "impact")!;
const priorityColumn = table.columns.find((column) => column.key === "priority")!;
const impact = row.fields.find((field) => field.target.field === impactColumn.id)!;
const priority = row.fields.find((field) => field.target.field === priorityColumn.id)!;

const publication = await client.editNumber(table.revision, impact.target, "3");
const observed = await client.queryFields(publication.resulting_revision, [
  impact.target,
  priority.target,
]);

console.log(observed.fields); // priority calculates to 8; diagnostics come from Rust
const exported = await client.exportProject(publication.resulting_revision);
```

The same client exposes revision-pinned `editText` and `editBoolean`. A stale
call throws `DesignerRuntimeError` with `failure.code === "stale_revision"`;
query or export the reported current revision instead of rebasing the edit in
frontend code. Export also requires the exact current revision and returns
opaque canonical project bytes suitable for host-owned persistence or a later
`openProject` round trip.

Always tear down the occurrence and Worker when the UI is finished:

```ts
await client.closeProject();
await client.close();
```

For executable evidence, run:

```sh
bash scripts/experimental-designer-client-smoke.sh
```

That throwaway consumer exports the kit twice (proving deterministic contents),
imports no `apps/designer` source, opens Product Gap, queries its typed table,
publishes a Number edit, observes the priority calculation and diagnostic list,
proves a stale edit leaves exported canonical bytes unchanged, and reopens an
exact round trip.

Deeper authority: [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md),
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md),
the [Semantic API specification](../specs/semantic-api.md), and the
[frontend/runtime boundary](../architecture/frontend-backend-boundary.md).
