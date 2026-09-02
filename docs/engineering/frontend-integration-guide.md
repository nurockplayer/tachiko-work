# Build an experimental Tachiko frontend

Status: onboarding guide for the Issue #231 external-frontend pilot. This is
implementation guidance, not a stable SDK, wire protocol, compatibility promise,
or normative Tachiko contract.

This guide is for a frontend engineer who wants to build a Tachiko UI in a
separate GitHub repository.

## Your assignment

Build a useful graphical client over Tachiko's existing runtime.

You may use React, Svelte, Vue, Solid, vanilla TypeScript, or another browser
stack. You may design a spreadsheet-like editor, a focused domain tool, a mobile-
shaped interface, or another presentation that makes sense to you.

You do **not** need to copy the first-party Designer. Different interpretations
are useful evidence for this pilot.

Your job is:

```text
Tachiko projections
        ↓
your interaction and visual design
        ↓
a useful frontend
```

Your job is not:

```text
parse Tachiko storage
+ rebuild formulas
+ rebuild validation
+ invent revision rules
= a second Tachiko engine
```

## The 30-second mental model

```text
your frontend
  layout, selection, viewport, draft input, visual state
        |
        | typed queries and revision-pinned edits
        v
experimental Designer client kit
        |
        v
Worker + Rust/WASM resident runtime
  canonical values, formulas, validation, revisions, export
```

The frontend renders **projections** returned by Tachiko. A projection is a
UI-facing view of semantic facts. It is safe to cache by revision and safe to
throw away.

Every edit supplies an **expected revision**. Tachiko rejects a stale edit rather
than silently overwriting a newer state.

The Rust **runtime is authoritative**. Formula results, validation findings,
semantic identity, revisions, and canonical export come from Tachiko, not from
frontend code.

## What each side owns

| Your frontend owns | Tachiko owns |
| --- | --- |
| Layout and visual design | Canonical semantic values |
| Selection, focus, viewport, open panels | Stable semantic identity |
| Temporary text and edit buffers | Formula calculation |
| Loading, error, and interaction presentation | Validation and diagnostics |
| Disposable revision-keyed projection caches | Revision and stale-edit decisions |
| Local sorting or grouping used only for display | Canonical `.roproj` admission and export |

A local UI state may look newer than the last runtime response, but it does not
become semantic truth until Tachiko accepts and publishes the edit.

## You do not need to learn these first

You can reach the first table without understanding:

- Rust crate internals;
- the raw WASM memory arena or ABI;
- `.roproj` serialization rules;
- the formula evaluator;
- the validation engine;
- the complete Semantic API specification;
- repository ADR history.

Those materials remain available for deeper work, but they are not an onboarding
tax.

## Current proven environment

The pilot currently targets a browser frontend using ES modules, Web Workers,
and WebAssembly.

The repository smoke consumer proves the flow with Vite and Chromium. Other
bundlers and browsers may work, but they are not yet pilot evidence. A server
must serve `.wasm` with an appropriate WebAssembly MIME type and keep the
exported kit files together so relative Worker and runtime URLs continue to
resolve.

The kit is vendored source and runtime output, not an npm package. Regenerate it
from the exact Tachiko checkout you intend to test.

## Fast path to the first table

### 1. Create your own repository

Keep your frontend outside `nurockplayer/tachiko-work`.

A simple shape is enough:

```text
my-tachiko-ui/
├── src/
├── public/
├── samples/
└── vendor/
    └── tachiko/
```

Use your preferred framework and repository conventions. The pilot does not
require a particular component library or visual design.

### 2. Export the client kit

From a clean Tachiko Work checkout with Rust, the
`wasm32-unknown-unknown` target, and pnpm 11.25.0 installed:

```sh
bash scripts/export-experimental-designer-client.sh \
  /path/to/my-tachiko-ui/vendor/tachiko
```

The destination must be absent or empty. Keep the generated directory intact
and import only its intended entry:

```ts
import {
  createExperimentalDesignerClient,
  projectTransferFromFiles,
} from "../vendor/tachiko/experimental-client.js";
```

Do not import neighboring `runtime/` or `host/` modules directly. They are
private support files that may change without notice.

### 3. Copy the sample project

The first pilot sample is:

```text
dogfood/product-gaps.roproj
```

It contains Text, Number, Boolean, and formula-backed fields in a domain unrelated
to the original Moonfall demo.

You may copy it into your repository for the pilot, or select it directly from a
local Tachiko checkout.

### 4. Open it and render the returned table

Use a browser directory input for the current `.roproj/v1` path:

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

input.addEventListener("change", () => {
  void openSelectedProject();
});

async function openSelectedProject(): Promise<void> {
  if (input.files === null) return;

  const transfer = await projectTransferFromFiles(input.files);
  const opened = await client.openProject(transfer);

  renderTable(opened.table);
}

function renderTable(table: TableProjection): void {
  console.log(table.collection.key, table.revision);
  console.table(
    table.rows.map((row) => ({
      key: row.key,
      fields: row.fields,
    })),
  );

  // Replace this function with your components and interaction design.
}
```

`openProject` already returns an initial `table`. You do not need another query
to render the first useful screen.

### 5. Publish one edit

Use a field target returned by the projection. Do not construct a target from a
row number, label, JSON path, or DOM coordinate.

```ts
const table = opened.table;
const row = table.rows[0];
const field = row?.fields.find(
  (candidate) => candidate.editable_scalar === "number",
);

if (field === undefined) throw new Error("No editable Number field was found.");

const publication = await client.editNumber(
  table.revision,
  field.target,
  "3",
);

const refreshed = await client.queryFields(publication.resulting_revision, [
  field.target,
  ...publication.affected_calculations,
]);

console.log(refreshed.fields);
```

The same client exposes `editText` and `editBoolean`.

Do not recalculate dependent formulas in JavaScript. Use the publication's
affected targets and query Tachiko for the new calculated and diagnostic
projections.

### 6. Export or close

Export requires the exact current revision:

```ts
const exported = await client.exportProject(publication.resulting_revision);
console.log(exported.bytes);
```

The bytes are an opaque canonical project bundle for host-owned persistence or a
later `openProject` round trip. Do not edit the bytes in frontend code.

Clean up both the project occurrence and Worker:

```ts
await client.closeProject();
await client.close();
```

## Read the projection without learning the engine

The first response is shaped approximately like this:

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

The distinctions matter:

- `stored` is directly stored input;
- `formula` describes formula-backed meaning;
- `calculated` is Tachiko's current formula outcome;
- `diagnostics` is Tachiko's validation/calculation evidence;
- `editable_scalar` tells the current pilot UI whether direct scalar editing is
  supported.

Do not collapse all five into one mutable cell value.

## Client API cheat sheet

| Call | Use it for |
| --- | --- |
| `openProject(bytes)` | Admit one canonical project and receive bootstrap plus the first table |
| `queryTable(collection)` | Switch to another typed collection |
| `queryFields(revision, targets)` | Refresh selected stored, calculated, and diagnostic projections |
| `editNumber(revision, target, input)` | Publish a Number edit through Tachiko authority |
| `editText(revision, target, value)` | Publish a Text edit through Tachiko authority |
| `editBoolean(revision, target, value)` | Publish a Boolean edit through Tachiko authority |
| `exportProject(revision)` | Export the exact current canonical project bytes |
| `closeProject()` | Destroy the current resident project occurrence |
| `close()` | Terminate the client Worker |

The generated `experimental-client.d.ts` is the most direct reference for the
current concrete TypeScript shapes. Those shapes are experimental and may change.

## The revision rule

Treat the revision as part of every edit, not as display decoration:

```text
render revision R0
        |
        | edit target using expected R0
        v
Tachiko accepts
        |
        v
publication says resulting revision R1
        |
        | query affected fields at R1
        v
render revision R1
```

When another accepted edit has already moved the runtime to `R1`, an edit still
based on `R0` fails with `DesignerRuntimeError` and
`failure.code === "stale_revision"`.

Do not silently replay that edit against the newer state. Show the conflict,
refresh the required projection, and let the user decide what to submit next.

## Things that will invalidate the experiment

Do not:

- import private Tachiko application source from `apps/designer`;
- copy only selected Worker/WASM internals and call them directly;
- parse or rewrite `.roproj` semantic content in the frontend;
- implement a second formula evaluator or validator;
- maintain a JavaScript `Document` as a competing canonical edit model;
- derive stable targets from names, table positions, or storage paths;
- treat a failed or stale edit as published;
- describe the current kit as a stable or supported public SDK.

If the only practical way forward appears to require one of these, stop and
record the friction. That is valuable architecture evidence for Issue #231.

## Known pilot limits

The current experiment is intentionally narrow:

- browser + Worker + WASM only;
- an exact canonical `.roproj/v1` directory is opened through a browser directory
  selection;
- direct Text, Number, and Boolean edits only;
- no schema or formula authoring UI contract;
- no npm publication or semantic-version compatibility promise;
- no network, cloud, authentication, or collaboration client contract;
- no autosave, same-path overwrite, or generalized persistence API;
- one resident project occurrence per client instance;
- the exported filenames, DTOs, revision spelling, and browser requirements may
  change or disappear.

Do not spend pilot time hiding these limits behind a new abstraction. Report the
ones that materially block your UI.

## What counts as a successful pilot client

A first external repository should demonstrate:

1. it imports only the generated kit;
2. it opens the Product Gap sample;
3. it renders a useful typed view;
4. it publishes at least one revision-safe Text, Number, or Boolean edit;
5. it displays formula or diagnostic evidence returned by Tachiko;
6. it handles a stale or rejected edit without claiming success;
7. it exports or reopens the accepted state;
8. it records where onboarding or the client boundary was confusing.

Visual polish is welcome, but it is not a substitute for proving the authority
boundary.

## Record friction while it is fresh

Please capture this small report in your repository or in Issue #231:

```md
## External frontend pilot report

- Repository:
- Framework / bundler / browser:
- Tachiko commit used:
- Time to first table:
- Time to first accepted edit:
- What worked without explanation:
- What required reading Tachiko internals:
- Where I wanted to duplicate semantic logic:
- Missing or awkward client operation:
- Error or revision behavior that was unclear:
- One thing I would simplify:
- Would I continue building on this boundary? Why?
```

Questions that sound basic are especially useful. If rendering one table requires
learning an internal concept, the boundary or documentation may be wrong.

## Where the work lives

Participants own and operate their external repositories. During the current
legal/contribution gate, share repository links, screenshots, issue reports, and
architecture observations rather than opening an implementation or normative-
specification PR against `tachiko-work`.

Record pilot evidence in
[Issue #231](https://github.com/nurockplayer/tachiko-work/issues/231).

## Read next only when needed

- [Experimental client-kit technical guide](experimental-designer-client-kit.md)
  covers exact export, assets, and the executable Product Gap walkthrough.
- [External-style smoke consumer](../../examples/experimental-designer-client/)
  is the smallest working reference implementation.
- [Frontend/backend boundary](../architecture/frontend-backend-boundary.md)
  explains the accepted authority split.
- [Semantic API specification](../specs/semantic-api.md) explains deeper logical
  operation meaning without making the current kit stable.
- [Issue #231](https://github.com/nurockplayer/tachiko-work/issues/231) owns the
  pilot and its evidence.
