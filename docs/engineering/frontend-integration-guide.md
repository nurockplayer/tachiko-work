# Build an experimental Tachiko frontend

Status: onboarding guide for the Issue #231 external-frontend pilot. This is
implementation guidance, not a stable SDK, wire protocol, compatibility promise,
or normative Tachiko contract.

This guide is for a frontend engineer building a Tachiko UI in a separate GitHub
repository.

## What you are building

Build a useful graphical client over Tachiko's existing runtime.

Use React, Svelte, Vue, Solid, vanilla TypeScript, or another browser stack. A
spreadsheet-like editor, focused domain tool, or another presentation is welcome.
Do not copy the first-party Designer by default. Different interpretations are
useful pilot evidence.

```text
Tachiko projections
        ↓
your interaction and visual design
        ↓
a useful frontend
```

You are **not** being asked to build another Tachiko engine:

```text
Do not parse Tachiko storage
Do not rebuild formulas or validation
Do not invent revision behavior
```

## Three concepts are enough to start

```text
your frontend
  layout, selection, viewport, draft input
        |
        | typed queries + revision-pinned edits
        v
experimental client kit
        |
        v
Worker + Rust/WASM runtime
  values, formulas, validation, revisions, export
```

1. **Projection** is UI-facing data returned by Tachiko. Cache it by revision or
   discard it freely.
2. **Expected revision** protects an edit from overwriting newer state. A stale
   edit is rejected.
3. **Runtime authority** means formula results, diagnostics, semantic IDs,
   revisions, and canonical export come from Tachiko, not frontend code.

## Who owns what

| Your frontend owns | Tachiko owns |
| --- | --- |
| Layout and visual design | Canonical semantic values |
| Selection, focus, viewport, panels | Stable semantic identity |
| Temporary input and edit buffers | Formula calculation |
| Loading and error presentation | Validation and diagnostics |
| Disposable revision-keyed caches | Revision and stale-edit decisions |
| Local display-only sorting/grouping | Canonical `.roproj` admission and export |

You can reach the first table without reading Rust internals, the raw WASM ABI,
`.roproj` serialization rules, the formula engine, or the ADR collection.

## Current proven path

The pilot currently targets a browser app using ES modules, Web Workers, and
WebAssembly.

The repository proves Vite + Chromium. Other bundlers and browsers may work, but
are not yet pilot evidence. Keep every generated kit file together, and serve
`.wasm` with the correct WebAssembly MIME type.

The kit is vendored output, not an npm package. Generate it from the exact
Tachiko checkout you are testing.

## Quick start

### 1. Prepare your repository and export the kit

Keep your frontend outside `nurockplayer/tachiko-work`:

```text
my-tachiko-ui/
├── src/
├── samples/
└── vendor/
    └── tachiko/
```

From a clean Tachiko Work checkout with Rust, the
`wasm32-unknown-unknown` target, and pnpm 11.25.0 installed:

```sh
bash scripts/export-experimental-designer-client.sh \
  /path/to/my-tachiko-ui/vendor/tachiko
```

The destination must be absent or empty. Import only the intended entry:

```ts
import {
  createExperimentalDesignerClient,
  projectTransferFromFiles,
} from "../vendor/tachiko/experimental-client.js";
```

Do not import neighboring `runtime/` or `host/` modules directly.

### 2. Use the Product Gap sample

Start with:

```text
dogfood/product-gaps.roproj
```

It contains Text, Number, Boolean, and formula-backed fields in a domain unrelated
to the original Moonfall demo. Copy it into your repository or select it from a
local Tachiko checkout.

### 3. Open it and render the first table

The current browser path uses a directory input:

```html
<input id="project" type="file" webkitdirectory />
```

```ts
import {
  createExperimentalDesignerClient,
  projectTransferFromFiles,
  type TableProjection,
} from "../vendor/tachiko/experimental-client.js";

const input = document.querySelector<HTMLInputElement>("#project");
if (input === null) throw new Error("Project input is missing.");

const client = createExperimentalDesignerClient();
let currentTable: TableProjection | null = null;
let currentRevision: string | null = null;

input.addEventListener("change", () => {
  void openSelectedProject();
});

async function openSelectedProject(): Promise<void> {
  if (input.files === null) return;

  const transfer = await projectTransferFromFiles(input.files);
  const opened = await client.openProject(transfer);
  currentTable = opened.table;
  currentRevision = opened.table.revision;

  renderTable(opened.table);
}

function renderTable(table: TableProjection): void {
  console.log(table.collection.key, table.revision);
  console.table(table.rows.map((row) => ({ key: row.key, fields: row.fields })));

  // Replace this with your components and interaction design.
}
```

`openProject` already returns the initial table. No second query is needed for
the first useful screen.

### 4. Publish an edit and refresh from Tachiko

Use a field target returned by the projection. Never build a target from a row
number, label, JSON path, or DOM coordinate.

```ts
if (currentTable === null || currentRevision === null) {
  throw new Error("Open a project first.");
}

const row = currentTable.rows[0];
const field = row?.fields.find(
  (candidate) => candidate.editable_scalar === "number",
);
if (field === undefined) throw new Error("No editable Number field was found.");

const publication = await client.editNumber(
  currentRevision,
  field.target,
  "3",
);

const refreshed = await client.queryFields(publication.resulting_revision, [
  field.target,
  ...publication.affected_calculations,
]);
currentRevision = refreshed.revision;

console.log(refreshed.fields);
```

The client also exposes `editText` and `editBoolean`.

Do not recalculate dependent formulas in JavaScript. Patch your disposable UI
cache from `refreshed.fields`, or query the table again, and use
`refreshed.revision` for the next edit. Never keep editing with the old revision.

Export or close with:

```ts
if (currentRevision === null) throw new Error("Open a project first.");

const exported = await client.exportProject(currentRevision);
console.log(exported.bytes); // opaque canonical project bytes

await client.closeProject();
await client.close();
```

Do not edit exported bytes in frontend code.

## How to read a projection

```text
OpenedProjection
├── bootstrap
│   ├── title
│   ├── revision
│   ├── default_collection
│   └── collections[]
└── table
    ├── revision
    ├── collection
    ├── columns[]
    └── rows[]
        └── fields[]
            ├── target
            ├── stored
            ├── formula
            ├── calculated
            ├── diagnostics[]
            └── editable_scalar
```

- `stored` is direct input.
- `formula` describes formula-backed meaning.
- `calculated` is Tachiko's current formula result.
- `diagnostics` contains validation or calculation evidence.
- `editable_scalar` tells the pilot UI whether direct scalar editing is allowed.

Do not collapse these into one mutable cell value.

## API cheat sheet

| Call | Purpose |
| --- | --- |
| `openProject(bytes)` | Open a canonical project and receive bootstrap plus the first table |
| `queryTable(collection)` | Switch to another typed collection |
| `queryFields(revision, targets)` | Refresh selected values, calculations, and diagnostics |
| `editNumber(revision, target, input)` | Publish a Number edit |
| `editText(revision, target, value)` | Publish a Text edit |
| `editBoolean(revision, target, value)` | Publish a Boolean edit |
| `exportProject(revision)` | Export the exact current canonical project bytes |
| `closeProject()` | Destroy the current resident project occurrence |
| `close()` | Terminate the Worker |

The generated `experimental-client.d.ts` is the direct reference for current
TypeScript shapes. Every concrete shape remains experimental.

## The revision rule

```text
render R0
   ↓ edit with expected R0
Tachiko accepts and publishes R1
   ↓ query affected fields at R1
render R1
```

An edit still based on `R0` after the runtime reaches `R1` throws
`DesignerRuntimeError` with `failure.code === "stale_revision"`.

Do not silently replay it against the newer state. Refresh the required
projection, show the conflict, and let the user decide what to submit next.

## Pilot boundaries

Do not:

- import private source from `apps/designer`;
- call Worker support modules or raw WASM exports directly;
- parse or rewrite `.roproj` meaning in the frontend;
- implement a second formula evaluator or validator;
- maintain a JavaScript `Document` as competing canonical state;
- treat failed or stale edits as published;
- describe the kit as a stable or supported public SDK.

The current experiment is intentionally limited to:

- browser + Worker + WASM;
- exact canonical `.roproj/v1` directory selection;
- direct Text, Number, and Boolean edits;
- no schema or formula authoring contract;
- no npm or semantic-version compatibility promise;
- no network, cloud, authentication, or collaboration API;
- no autosave, same-path overwrite, or general persistence API;
- one resident project at a time per client;
- filenames, DTOs, revisions, and browser requirements that may change.

When a useful UI seems to require breaking a boundary, stop and report the
friction instead of hiding it behind a new abstraction.

## What counts as success

A first pilot repository should:

1. import only the generated kit;
2. open the Product Gap sample;
3. render a useful typed view;
4. publish one revision-safe scalar edit;
5. show formula or diagnostic evidence returned by Tachiko;
6. handle a stale or rejected edit honestly;
7. export or reopen the accepted state;
8. record what was confusing.

Visual polish is welcome, but the authority boundary is the actual experiment.

## Record friction

Add this report to your repository or Issue #231:

```md
## External frontend pilot report

- Repository:
- Framework / bundler / browser:
- Tachiko commit used:
- Time to first table:
- Time to first accepted edit:
- What worked without explanation:
- What required Tachiko-internal reading:
- Where I wanted to duplicate semantic logic:
- Missing or awkward client operation:
- Error or revision behavior that was unclear:
- One thing I would simplify:
- Would I continue on this boundary? Why?
```

Participants own their external repositories. During the current legal and
contribution gate, share links, screenshots, issue reports, and architecture
observations rather than opening an implementation or normative-specification PR
against `tachiko-work`.

Record pilot evidence in
[Issue #231](https://github.com/nurockplayer/tachiko-work/issues/231).

## Read next only when needed

- [Client-kit technical guide](experimental-designer-client-kit.md) covers exact
  export, generated assets, and the executable Product Gap walkthrough.
- [External-style smoke consumer](../../examples/experimental-designer-client/)
  is the smallest working reference.
- [Frontend/backend boundary](../architecture/frontend-backend-boundary.md)
  explains the accepted authority split.
- [Semantic API specification](../specs/semantic-api.md) explains deeper logical
  operation meaning without making this kit stable.
