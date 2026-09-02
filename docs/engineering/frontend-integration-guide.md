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
Tachiko semantic runtime
        ↓
typed frontend projections
        ↓
your interaction + visual design
        ↓
a useful Tachiko client
```

You are not being asked to parse Tachiko storage, rebuild formulas or validation,
or invent revision behavior. That would create a second engine.

## Three concepts are enough to start

1. **Projection** is UI-facing data returned by Tachiko. Render it; do not turn it
   into a competing canonical document model.
2. **Expected revision** protects an edit from overwriting newer state. A stale
   edit is rejected rather than silently replayed.
3. **Runtime authority** means formula results, diagnostics, semantic IDs,
   revisions, project admission, and canonical export come from Tachiko.

That is enough context for a frontend engineer to begin. Rust internals, the raw
WASM ABI, `.roproj` serialization, formula-engine internals, and the ADR archive
are not prerequisites.

## Who owns what

| Your frontend owns | Tachiko owns |
| --- | --- |
| Layout and visual design | Canonical semantic values |
| Selection, focus, viewport, panels | Stable semantic identity |
| Temporary edit buffers | Formula calculation |
| Loading and error presentation | Validation and diagnostics |
| Local display sorting/grouping | Revision and stale-edit decisions |
| Host persistence UX | Canonical project admission/export bytes |

A good boundary feels boring to the frontend developer: ask Tachiko for typed
facts, present them well, send edits back with the revision they came from.

## Current proven path

The current pilot path is deliberately narrow:

- browser application;
- ES modules;
- Web Worker;
- Rust compiled to WebAssembly;
- Vite + Chromium as the repository-proven combination;
- a vendored experimental client kit generated from the Tachiko Work checkout.

Other frameworks and bundlers are welcome experiments. Treat failures there as
pilot evidence, not as permission to bypass the client boundary.

## Start in your own repository

Keep the frontend outside `nurockplayer/tachiko-work`.

For example:

```text
my-tachiko-ui/
├── src/
├── samples/
└── vendor/
    └── tachiko/
```

From the Tachiko Work checkout, export the current client kit:

```sh
bash scripts/export-experimental-designer-client.sh \
  /path/to/my-tachiko-ui/vendor/tachiko
```

The kit is generated output, not an npm package. Keep it together and import only
its intended entry point, `experimental-client.js`. Do not reach into neighboring
private Worker, runtime, host, or raw WASM modules.

For exact TypeScript names, generated files, and the executable wiring path, use
[Experimental Designer client kit: first contact](experimental-designer-client-kit.md).

## Use the Product Gap sample first

Begin with the repository-owned sample:

```text
dogfood/product-gaps.roproj
```

It contains ordinary typed fields plus formula-backed fields in a domain unrelated
to Moonfall. It exists to prove that the frontend boundary is not tied to one
demo domain.

You may copy it into an external repository only as a local pilot fixture. It is
product evidence, not a public template or delivery source.

## First useful milestone

Your first pass only needs to prove this sequence:

```text
open Product Gap
      ↓
render one typed collection
      ↓
edit one Text / Number / Boolean field
      ↓
receive the accepted new revision
      ↓
refresh from Tachiko
      ↓
show formula / diagnostic evidence
      ↓
persist or export without silent data loss
```

Do not optimize early. A full table refresh after an accepted edit is fine for
the first pilot. Selective projection refresh can come later.

The smallest repository-owned working consumer is
[`examples/experimental-designer-client/`](../../examples/experimental-designer-client/).
Use it to confirm the integration seam, not as a UI design template.

## Editing rules

When publishing edits:

- use semantic field targets returned by Tachiko, not row numbers, labels, JSON
  paths, or DOM coordinates;
- send the revision the UI actually rendered;
- treat a rejected or stale edit as rejected;
- refresh from Tachiko after acceptance;
- never recalculate dependent formulas in JavaScript;
- never silently replay a stale edit against newer state.

The frontend may cache projections, but the cache is disposable and
revision-keyed. Tachiko remains the source of semantic truth.

## Persistence rules

Canonical export bytes come from Tachiko, but returning bytes is not the same as
a durable save. The frontend host decides where they are persisted.

A real client therefore needs to distinguish:

```text
resident revision     what Tachiko currently holds
rendered revision     what the UI currently shows
durable revision      what the host has successfully saved
```

Do not tell the user work is saved until host persistence actually succeeds.
Do not discard accepted resident changes without saving them or asking first.
Do not let open, edit, save, and close operations race one another.

## Pilot boundaries

Do not:

- import private source from `apps/designer`;
- call private Worker support modules or raw WASM exports directly;
- parse or rewrite `.roproj` semantic meaning in frontend code;
- implement a second formula evaluator or validator;
- maintain a JavaScript document as competing canonical state;
- describe the current kit as a stable or supported public SDK.

The experiment currently does **not** promise:

- schema or formula authoring;
- npm distribution or semantic-version compatibility;
- cloud, authentication, or collaboration APIs;
- autosave or same-path overwrite;
- a general persistence API;
- stable DTO names, filenames, or browser requirements.

If a useful UI appears to require crossing one of these boundaries, report the
friction instead of hiding it behind a frontend workaround.

## What counts as success

A first external frontend repository should:

1. import only the generated kit;
2. open the Product Gap sample;
3. render a useful typed view;
4. publish one revision-safe scalar edit;
5. show formula or diagnostic evidence returned by Tachiko;
6. handle a stale or rejected edit honestly;
7. persist or reopen accepted state without silent data loss;
8. record what was confusing.

Visual polish is welcome, but the architectural experiment is whether a frontend
engineer can build something useful without learning Tachiko's engine internals.

## Record friction

Add a short report to your repository or Issue #231:

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

Participants own their external repositories. Share links, screenshots, issue
reports, and architecture observations back to
[Issue #231](https://github.com/nurockplayer/tachiko-work/issues/231).

## Read next only when needed

- [Client-kit technical guide](experimental-designer-client-kit.md) for exact
  export, generated assets, API shapes, and executable wiring.
- [External-style smoke consumer](../../examples/experimental-designer-client/)
  for the smallest working integration.
- [Frontend/backend boundary](../architecture/frontend-backend-boundary.md) for
  the accepted authority split.
- [Semantic API specification](../specs/semantic-api.md) for deeper logical
  operation meaning without making this kit stable.
