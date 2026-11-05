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
const records = [], captures = [], timingCells = [];
const result = { completed: false, records, environment: { node: process.version, platform: process.platform, arch: process.arch } };
try {
  for (const caseValue of manifest.accepted.filter(c => ["320x2-mixed", "1000x16-mixed", "8406x3-text"].includes(c.name))) {
    for (const sourceFormat of ["csv", "xlsx"]) for (const carrier of ["opaque", "canonical"]) {
      let saved;
      for (const phase of ["saved", "reopened"]) {
        const start = performance.now();
        const laps = {};
        const lap = name => { laps[name] = +(performance.now() - start).toFixed(1); };
        const browser = await chromium.launch({ headless: true });
        lap("launch");
        const version = browser.version();
        let observed, workerUrls;
        try {
          const page = await browser.newPage();
          workerUrls = [];
          page.on("worker", worker => workerUrls.push(worker.url()));
          await page.goto(origin);
          lap("pageLoad");
          observed = await page.evaluate(async ({ origin, caseValue, sourceFormat, carrier, phase, saved, rejected }) => {
            const check = (ok, label) => { if (!ok) throw Error(label); };
            const same = (a, b, label) => check(JSON.stringify(a) === JSON.stringify(b), label);
            const ms = before => +(performance.now() - before).toFixed(1);
            const initStart = performance.now();
            const kit = await import(`${origin}/kit/experimental-client.js`);
            const client = kit.createExperimentalDesignerClient();
            const timings = { init: ms(initStart), import: 0, inspect: 0, open: 0, tableReads: [], snapshots: [], rejects: [] };
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
              same(table.columns.map(c => c.required), caseValue.kinds.map(() => false), "imported optional declarations");
              same(metadata.sheets[0].columns.map(c => c.name), caseValue.headers, "headers");
              same(table.columns.map(c => c.id), metadata.sheets[0].columns.map(c => c.field_id), "column identity/order");
              same(table.rows.map(r => r.id), metadata.sheets[0].rows.map(r => r.entity_id), "row identity/order");
              table.rows.forEach((row, r) => {
                const blanks = table.columns.filter((_, c) => values[r][c] === null).map(column => column.id);
                same(row.blank_fields, blanks.length === 0 ? undefined : blanks, "exact ordered absent declared fields");
                const present = table.columns.flatMap((column, c) => values[r][c] === null ? []
                  : [{ target: { entity: row.id, field: column.id }, stored: values[r][c], c }]);
                same(row.fields.map(field => field.target), present.map(cell => cell.target), "exact ordered present subset");
                row.fields.forEach((field, index) => {
                  same(field.blank, undefined, "present scalar has no blank marker");
                  same(field.stored, present[index].stored, `cell ${r},${present[index].c}`);
                  same(field.formula, null, "no formulas"); same(field.diagnostics, [], "no diagnostics");
                });
              });
            };
            // queryTable refuses above 1,000 rows; page the complete table.
            const table = async () => {
              let queries = 0;
              const actual = await timed("query", async () => {
                const revision = (await client.observeOccurrence()).revision;
                const first = await client.queryTableWindow(revision, collection, {}, 0, 64);
                queries += 1;
                const rows = [...first.rows];
                while (rows.length < first.total_rows) {
                  const page = await client.queryTableWindow(revision, collection, {}, rows.length, 64);
                  queries += 1;
                  if (page.rows.length === 0) break;
                  rows.push(...page.rows);
                }
                return { revision: first.revision, collection: first.collection, columns: first.columns, rows };
              });
              timings.tableReads.push({ queries, ms: +durations.query.at(-1).toFixed(1) });
              verify(actual); return actual;
            };
            const exported = async () => {
              const revision = (await client.observeOccurrence()).revision;
              const output = {}, parts = {};
              for (const format of ["csv", "xlsx"]) {
                const before = performance.now();
                output[format] = byteList((await client.exportSpreadsheet(revision, metadata, format, metadata.sheets[0].schema_id)).bytes);
                parts[format] = ms(before);
              }
              return { output, parts };
            };
            const snapshot = async () => {
              const before = performance.now();
              const occurrence = await client.observeOccurrence();
              let mark = performance.now();
              const tree = await client.exportCanonicalTree(occurrence.revision);
              const canonical = ms(mark);
              const tableRead = await table();
              mark = performance.now();
              const project = byteList((await client.exportProject(occurrence.revision)).bytes);
              const projectMs = ms(mark);
              const { output, parts } = await exported();
              timings.snapshots.push({ ms: ms(before), canonical, project: projectMs, csv: parts.csv, xlsx: parts.xlsx });
              return { occurrence, table: tableRead, opaque: project,
                files: tree.files.map(f => ({ path: f.path, bytes: byteList(f.bytes) })), exports: output };
            };
            const refusal = async (name, action, before) => {
              const started = performance.now();
              let failure = false;
              try { await action(); } catch { failure = true; }
              check(failure, "expected atomic refusal"); same(await snapshot(), before, "resident/occurrence/history/export preserved");
              timings.rejects.push({ name, ms: ms(started) });
            };
            // Read the revision at each publication, rather than depending on client cache.
            const history = async type => {
              const revision = (await client.observeOccurrence()).revision;
              return timed(type, () => client.trackerCommand({ type, expected_revision: revision }));
            };
            try {
              if (phase === "saved") {
                const importStart = performance.now();
                const bytes = await (await fetch(`${origin}/fixtures/${caseValue.name}.${sourceFormat}`)).arrayBuffer();
                const imported = await client.importSpreadsheet(bytes, sourceFormat, { delimiter: ",", header: true }, { column_types: [caseValue.kinds], extra_columns: [[]] });
                timings.import = ms(importStart);
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
                    await refusal(`${bad.name} inspect`, () => client.inspectSpreadsheet(bytes.slice(0), sourceFormat, { delimiter: ",", header: true }), before);
                  await refusal(`${bad.name} import`, () => client.importSpreadsheet(bytes, sourceFormat, { delimiter: ",", header: true }, { column_types: [bad.kinds], extra_columns: [[]] }), before);
                }
                const badMetadata = { ...metadata, version: 2 };
                await refusal("openImportedProject version 2", () => client.openImportedProject(transfer(before), badMetadata), before);
                await history("redo"); values.at(-1)[0] = lastFinal; await table();
              } else {
                metadata = saved.metadata;
                const bytes = transfer(saved);
                let mark = performance.now();
                const inspection = await client.inspectImportedProject(bytes.slice(0), metadata);
                timings.inspect = ms(mark);
                verify(inspection.table);
                mark = performance.now();
                const opened = await client.openImportedProject(bytes.slice(0), metadata);
                timings.open = ms(mark);
                collection = opened.bootstrap.default_collection; verify(opened.table);
                check((await client.observeOccurrence()).scope !== saved.scope, "fresh browser/Worker occurrence");
                const resaved = await snapshot();
                same(carrier === "opaque" ? resaved.opaque : resaved.files, carrier === "opaque" ? saved.opaque : saved.files, "same carrier bytes after fresh reopen");
              }
              const final = await snapshot();
              return { saved: { metadata, opaque: final.opaque, files: final.files, values, scope: final.occurrence.scope }, exports: final.exports, durations, timings };
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
        } finally { await browser.close(); lap("close"); }
        lap("total");
        const { timings: cellTimings } = observed;
        const summary = values => values.reduce((sum, value) => sum + value, 0).toFixed(1);
        console.log(`timing ${caseValue.name} ${sourceFormat} ${carrier} ${phase} total=${laps.total}ms launch=${laps.launch} init=${cellTimings.init} import=${cellTimings.import} tableReads=${cellTimings.tableReads.length}/${summary(cellTimings.tableReads.map(read => read.ms))}ms snapshots=${cellTimings.snapshots.length}/${summary(cellTimings.snapshots.map(entry => entry.ms))}ms rejects=${summary(cellTimings.rejects.map(entry => entry.ms))}ms close=${laps.close}`);
        records.push({ case: caseValue.name, sourceFormat, carrier, phase, workerUrls,
          browser: version, durations: observed.durations, milliseconds: laps.total, outcome: "PASS" });
        timingCells.push({ case: caseValue.name, sourceFormat, carrier, phase, node: laps,
          page: { ...cellTimings, edit: observed.durations.edit, undo: observed.durations.undo, redo: observed.durations.redo } });
      }
    }
  }
  result.completed = true;
} catch (error) {
  result.failure = String(error); process.exitCode = 1;
} finally {
  await writeFile(resolve(destination, "captures.json"), JSON.stringify(captures));
  await writeFile(resolve(destination, "result.json"), JSON.stringify(result, null, 2));
  await writeFile(resolve(destination, "timings.json"), JSON.stringify(timingCells, null, 2));
  console.log(JSON.stringify(result));
  await new Promise(done => server.close(done));
}
