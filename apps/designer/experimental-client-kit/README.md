# Tachiko experimental Designer client kit

This vendorable browser kit is the bounded `designer-client-kit/v0` experiment.
It is **not** a stable or public SDK, an npm package, a stable wire protocol, or
a compatibility promise. Every exported TypeScript shape, revision spelling,
Worker detail, filename, and browser requirement may change or disappear.

Keep this directory intact. `experimental-client.js` is the only intended
entry; its neighboring Worker modules and `designer_runtime.wasm` are private
runtime assets. Do not import those assets directly or call the raw WASM ABI.

```ts
import {
  createExperimentalDesignerClient,
  projectTransferFromFiles,
} from "./vendor/tachiko/experimental-client.js";

const client = createExperimentalDesignerClient();
const bytes = await projectTransferFromFiles(directoryInput.files!);
const opened = await client.openProject(bytes);
const table = await client.queryTable(opened.bootstrap.default_collection);

// Use stable targets from a projection and pin every edit to its revision.
const field = table.rows[0]?.fields.find(
  (candidate) => candidate.editable_scalar === "number",
);
if (field) {
  const publication = await client.editNumber(table.revision, field.target, "3");
  const refreshed = await client.queryFields(publication.resulting_revision, [
    field.target,
    ...publication.affected_calculations,
  ]);
  console.log(refreshed); // calculation and diagnostic projections are authoritative
  const exported = await client.exportProject(publication.resulting_revision);
  console.log(exported.bytes);
}

await client.closeProject();
await client.close();
```

`projectTransferFromFiles` packages browser-selected path/byte records only;
the Rust runtime remains the sole parser and authority for `.roproj` meaning.
The frontend may retain disposable revision-keyed projections and ordinary UI
state, but it must not calculate formulas, validate candidates, invent revision
semantics, or mirror an authoritative document.

See the repository's first-contact guide for the Product Gap walkthrough:
<https://github.com/nurockplayer/tachiko-work/blob/main/docs/engineering/experimental-designer-client-kit.md>.
