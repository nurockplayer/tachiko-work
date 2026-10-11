// Work #507 producer qualification: exact kit, actual module Worker and full browser restart.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { chromium } from "@playwright/test";

const [kitPath, fixturePath, destination] = process.argv.slice(2);
assert(kitPath && fixturePath && destination);
const roots = { kit: resolve(kitPath), fixtures: resolve(fixturePath) };
await mkdir(destination);
const manifest = JSON.parse(await readFile(resolve(fixturePath, "cases.json"), "utf8"));
const server = createServer(async (request, response) => {
  try {
    const parts = new URL(request.url, "http://localhost").pathname.split("/").filter(Boolean);
    if (!parts.length) { response.end("<!doctype html><title>Work rectangle acceptance</title>"); return; }
    const root = roots[parts.shift()]; assert(root);
    const path = resolve(root, ...parts); assert(path.startsWith(root + sep));
    response.setHeader("Content-Type", path.endsWith(".wasm") ? "application/wasm" : path.endsWith(".js") ? "text/javascript" : "application/octet-stream");
    response.end(await readFile(path));
  } catch { response.statusCode = 404; response.end("missing"); }
});
await new Promise(done => server.listen(0, "127.0.0.1", done));
const origin = `http://127.0.0.1:${server.address().port}`;
const records = [], captures = [];
const result = { completed: false, records, environment: { node: process.version, platform: process.platform, arch: process.arch } };
try {
  for (const caseValue of manifest.accepted.filter(c => ["320x2-mixed", "1000x16-mixed", "8406x3-text"].includes(c.name))) {
    for (const sourceFormat of ["csv", "xlsx"]) for (const carrier of ["opaque", "canonical"]) {
      let saved;
      for (const phase of ["saved", "reopened"]) {
        const start = performance.now();
        const browser = await chromium.launch({ headless: true });
        try {
          const page = await browser.newPage();
          const workerUrls = [];
          page.on("worker", worker => workerUrls.push(worker.url()));
          await page.goto(origin);
          const observed = await page.evaluate(async ({ origin, caseValue, sourceFormat, carrier, phase, saved, rejected }) => {
            const check = (ok, label) => { if (!ok) throw Error(label); };
            const same = (a, b, label) => check(JSON.stringify(a) === JSON.stringify(b), label);
            const kit = await import(`${origin}/kit/experimental-client.js`);
            const client = kit.createExperimentalDesignerClient();
            const byteList = bytes => Array.from(new Uint8Array(bytes));
            const transfer = state => carrier === "opaque" ? Uint8Array.from(state.opaque).buffer
              : kit.projectTransferFromEntries(state.files.map(f => ({ path: f.path, bytes: Uint8Array.from(f.bytes).buffer })));
            const values = structuredClone(phase === "saved" ? caseValue.values : saved.values);
            let metadata, collection;
            const durations = { edit: [], undo: [], redo: [], query: [] };
            const timed = async (name, action) => {
              const before = performance.now();
              try { return await action(); } finally { durations[name].push(performance.now() - before); }
            };
            const verify = table => {
              same(table.rows.length, values.length, "all rows");
              same(table.collection.entity_count, values.length, "entity count");
              same(table.columns.map(c => c.field_type), caseValue.kinds, "confirmed types");
              same(metadata.sheets[0].columns.map(c => c.name), caseValue.headers, "headers");
              same(table.columns.map(c => c.id), metadata.sheets[0].columns.map(c => c.field_id), "column identity/order");
              same(table.rows.map(r => r.id), metadata.sheets[0].rows.map(r => r.entity_id), "row identity/order");
              table.rows.forEach((row, r) => {
                const present = table.columns.flatMap((column, c) => values[r][c] === null ? []
                  : [{ target: { entity: row.id, field: column.id }, stored: values[r][c], c }]);
                same(row.fields.map(field => field.target), present.map(cell => cell.target), "exact ordered present subset");
                row.fields.forEach((field, index) => {
                  same(field.stored, present[index].stored, `cell ${r},${present[index].c}`);
                  same(field.formula, null, "no formulas"); same(field.diagnostics, [], "no diagnostics");
                });
              });
            };
            const table = async () => {
              const actual = await timed("query", () => client.queryTable(collection)); verify(actual); return actual;
            };
            const exported = async () => {
              const revision = (await client.observeOccurrence()).revision;
              const output = {};
              for (const format of ["csv", "xlsx"]) output[format] = byteList((await client.exportSpreadsheet(revision, metadata, format, metadata.sheets[0].schema_id)).bytes);
              return output;
            };
            const snapshot = async () => {
              const occurrence = await client.observeOccurrence();
              const tree = await client.exportCanonicalTree(occurrence.revision);
              return { occurrence, table: await table(), opaque: byteList((await client.exportProject(occurrence.revision)).bytes),
                files: tree.files.map(f => ({ path: f.path, bytes: byteList(f.bytes) })), exports: await exported() };
            };
            const refusal = async (action, before) => {
              let failure = false;
              try { await action(); } catch { failure = true; }
              check(failure, "expected atomic refusal"); same(await snapshot(), before, "resident/occurrence/history/export preserved");
            };
            // Read the revision at each publication, rather than depending on client cache.
            const history = async type => {
              const revision = (await client.observeOccurrence()).revision;
              return timed(type, () => client.trackerCommand({ type, expected_revision: revision }));
            };
            try {
              if (phase === "saved") {
                const bytes = await (await fetch(`${origin}/fixtures/${caseValue.name}.${sourceFormat}`)).arrayBuffer();
                const imported = await client.importSpreadsheet(bytes, sourceFormat, { delimiter: ",", header: true }, { column_types: [caseValue.kinds], extra_columns: [[]] });
                metadata = imported.metadata; collection = imported.opened.bootstrap.default_collection; verify(imported.opened.table);
                for (const [r, c] of [[0, 0], [Math.floor(values.length / 2), caseValue.kinds.length - 1], [values.length - 1, 0]]) {
                  const actual = await table();
                  const target = actual.rows[r].fields.find(field => field.target.field === actual.columns[c].id)?.target;
                  check(target, "selected populated logical cell");
                  const original = values[r][c];
                  const value = caseValue.kinds[c] === "number" ? { kind: "number", value: 700.25 } : { kind: "text", value: '000-edited-雪,"first"\r\nkept' };
                  const revision = (await client.observeOccurrence()).revision;
                  await timed("edit", () => value.kind === "number" ? client.editNumber(revision, target, "700.25") : client.editText(revision, target, value.value));
                  values[r][c] = value; await table();
                  await history("undo"); values[r][c] = original; await table();
                  await history("redo"); values[r][c] = value; await table();
                }
                await history("undo");
                const lastOriginal = caseValue.values.at(-1)[0];
                const lastFinal = values.at(-1)[0]; values.at(-1)[0] = lastOriginal;
                const before = await snapshot();
                for (const bad of rejected) {
                  const bytes = await (await fetch(`${origin}/fixtures/${bad.name}.${sourceFormat}`)).arrayBuffer();
                  if (["320x2-mixed-expression", "320x2-mixed-nonfinite", "320x2-mixed-column"].includes(bad.name)
                      && (sourceFormat === "csv" || bad.name === "320x2-mixed-expression"))
                    await refusal(() => client.inspectSpreadsheet(bytes.slice(0), sourceFormat, { delimiter: ",", header: true }), before);
                  await refusal(() => client.importSpreadsheet(bytes, sourceFormat, { delimiter: ",", header: true }, { column_types: [bad.kinds], extra_columns: [[]] }), before);
                }
                const badMetadata = { ...metadata, version: 2 };
                await refusal(() => client.openImportedProject(transfer(before), badMetadata), before);
                await history("redo"); values.at(-1)[0] = lastFinal; await table();
              } else {
                metadata = saved.metadata;
                const bytes = transfer(saved);
                const inspection = await client.inspectImportedProject(bytes.slice(0), metadata); verify(inspection.table);
                const opened = await client.openImportedProject(bytes.slice(0), metadata);
                collection = opened.bootstrap.default_collection; verify(opened.table);
                check((await client.observeOccurrence()).scope !== saved.scope, "fresh browser/Worker occurrence");
                const resaved = await snapshot();
                same(carrier === "opaque" ? resaved.opaque : resaved.files, carrier === "opaque" ? saved.opaque : saved.files, "same carrier bytes after fresh reopen");
              }
              const final = await snapshot();
              return { saved: { metadata, opaque: final.opaque, files: final.files, values, scope: final.occurrence.scope }, exports: final.exports, durations };
            } finally { await client.close(); }
          }, { origin, caseValue, sourceFormat, carrier, phase, saved, rejected: manifest.refused });
          assert.equal(workerUrls.length, 1);
          assert(workerUrls[0].endsWith("/kit/experimental-client.worker.js"));
          saved = observed.saved;
          for (const format of ["csv", "xlsx"]) {
            const file = `${caseValue.name}-${sourceFormat}-${carrier}-${phase}.${format}`;
            await writeFile(resolve(destination, file), Uint8Array.from(observed.exports[format]), { flag: "wx" });
            captures.push({ case: caseValue.name, file, state: "edited", phase, sourceFormat, carrier });
          }
          records.push({ case: caseValue.name, sourceFormat, carrier, phase, workerUrls,
            browser: browser.version(), durations: observed.durations, milliseconds: performance.now() - start, outcome: "PASS" });
        } finally { await browser.close(); }
      }
    }
  }
  result.completed = true;
} catch (error) {
  result.failure = String(error); process.exitCode = 1;
} finally {
  await writeFile(resolve(destination, "captures.json"), JSON.stringify(captures));
  await writeFile(resolve(destination, "result.json"), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
  await new Promise(done => server.close(done));
}
