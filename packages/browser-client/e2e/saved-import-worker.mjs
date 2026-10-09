// UNRUN proposal. Actual exported kit, native module Worker and whole-browser
// restart; no mock Worker/runtime or weakened capacity performance oracle.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { arch, platform, release } from 'node:os';
import { resolve, sep } from 'node:path';
import { chromium } from '@playwright/test';
const args = process.argv.slice(2);
assert(args.length >= 3 && args.length <= 5, 'KIT FIXTURES NEW_CAPTURE [MANIFEST [--mvp]] required');
const [kitArg, fixturesArg, captureArg, manifestArg = 'saved-import-manifest.json', modeArg] = args;
assert(manifestArg !== '--mvp', '--mvp must follow an explicit manifest argument');
assert(modeArg === undefined || modeArg === '--mvp', 'unknown or misplaced argument');
const mvp = modeArg === '--mvp';
assert(kitArg && fixturesArg && captureArg, 'KIT FIXTURES NEW_CAPTURE required');
const roots = {kit: resolve(kitArg), fixtures: resolve(fixturesArg), capture: resolve(captureArg)};
await mkdir(roots.capture, {recursive: false});
const fixture = JSON.parse(await readFile(`${roots.fixtures}/${manifestArg}`, 'utf8'));
assert([66, 8406].includes(fixture.rows.length));
if (mvp) assert.equal(fixture.rows.length, 8406, '--mvp requires exactly 8406 fixture rows');
assert.equal(fixture.headers.length, 3);
const sourceStem = fixture.rows.length === 66 ? 'saved-import-66' : 'source-8406';
const server = createServer(async (request, response) => {
  try {
    const parts = new URL(request.url, 'http://localhost').pathname.split('/').filter(Boolean);
    if (!parts.length) { response.end('<!doctype html><title>Saved import regression</title>'); return; }
    const root = roots[parts.shift()]; assert(root);
    const path = resolve(root, ...parts); assert(path.startsWith(root + sep));
    response.setHeader('Content-Type', path.endsWith('.wasm') ? 'application/wasm' : path.endsWith('.js') ? 'text/javascript' : 'application/octet-stream');
    response.end(await readFile(path));
  } catch { response.statusCode = 404; response.end('missing'); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;
const receipt = [];
const environment = {node: process.version, os: `${platform()} ${release()}`, architecture: arch()};
try {
  for (const format of ['csv', 'xlsx']) for (const carrier of ['canonical', 'opaque']) {
    let saved;
    for (const phase of ['save', 'restart-reopen']) {
      const phaseStarted = Date.now();
      const browser = await chromium.launch({headless: true, ...(process.env.CAPACITY_CHROMIUM ? {executablePath: process.env.CAPACITY_CHROMIUM} : {})});
      let phaseReceipt;
      try {
        const page = await browser.newPage();
        const workerUrls = [];
        page.on('worker', worker => workerUrls.push(worker.url()));
        await page.goto(origin);
        const result = await page.evaluate(async ({origin, fixture, format, carrier, phase, saved, sourceStem, mvp}) => {
          const check = (ok, message) => { if (!ok) throw Error(message); };
          const same = (a, b, label) => check(JSON.stringify(a) === JSON.stringify(b), label);
          const fail = async action => { let rejected = false; try { await action(); } catch { rejected = true; } check(rejected, 'required atomic refusal'); };
          const kit = await import(`${origin}/kit/experimental-client.js`);
          const client = kit.createExperimentalDesignerClient();
          // The absent capability cannot become inspect→ordinary-open fallback.
          // This is the real kit capability check; legacy-client refusal also
          // needs its separate consumer/unit boundary coverage.
          check(typeof client.openImportedProject === 'function', 'required openImportedProject capability absent');
          check(typeof client.inspectImportedProject === 'function', 'required inspection capability absent');
          let ordinaryOpenCalls = 0;
          const ordinaryOpen = client.openProject.bind(client);
          client.openProject = (...args) => { ordinaryOpenCalls++; return ordinaryOpen(...args); };
          let expected = structuredClone(phase === 'save' ? fixture.rows : saved.expected);
          let metadata, collection;
          const captures = [];
          const editProofs = [];
          const durationsMs = {edit: [], undo: [], redo: [], query: []};
          let captureSequence = 0;
          const timed = async (key, action) => {
            const start = performance.now();
            try { return await action(); }
            finally { durationsMs[key].push(performance.now() - start); }
          };
          const currentTable = () => timed('query', () => client.queryTable(collection));
          const checkTable = table => {
            same(table.rows.length, expected.length, 'all rows');
            same(table.columns.length, 3, 'three columns');
            same(metadata.sheets[0].columns.map(c => c.name), fixture.headers, 'source header fidelity');
            same(table.collection.id, metadata.sheets[0].schema_id, 'collection mapping');
            same(table.columns.map(c => c.field_type), ['text', 'text', 'text'], 'Text types');
            same(table.columns.map(c => c.id), metadata.sheets[0].columns.map(c => c.field_id), 'column mappings');
            same(table.rows.map(r => r.id), metadata.sheets[0].rows.map(r => r.entity_id), 'all row mappings');
            table.rows.forEach((r, i) => {
              same(r.fields.length, 3, 'three fields per row');
              r.fields.forEach((f, j) => {
              same(f.target, {entity: r.id, field: table.columns[j].id}, 'stable target');
              same(f.stored, {kind: 'text', value: expected[i][j]}, `cell ${i},${j}`);
              same(f.formula, null, 'no formula'); same(f.diagnostics, [], 'no diagnostics');
              });
            });
          };
          const byteList = bytes => Array.from(new Uint8Array(bytes));
          const exports = async label => {
            const captureId = captureSequence++;
            const revision = (await client.observeOccurrence()).revision;
            const output = {};
            for (const f of ['csv', 'xlsx']) {
              const value = await client.exportSpreadsheet(revision, metadata, f, metadata.sheets[0].schema_id);
              output[f] = byteList(value.bytes);
              captures.push({name: `${format}-${carrier}-${phase}-${captureId}-${label}.${f}`, bytes: output[f], expected: structuredClone(expected)});
            }
            return output;
          };
          const snapshot = async () => {
            const occurrence = await client.observeOccurrence();
            const table = await currentTable(); checkTable(table);
            const opaque = await client.exportProject(occurrence.revision);
            const tree = await client.exportCanonicalTree(occurrence.revision);
            return {occurrence, table, opaque: byteList(opaque.bytes), files: tree.files.map(f => ({path: f.path, bytes: byteList(f.bytes)})), exports: await exports('snapshot')};
          };
          const openSaved = async s => {
            metadata = structuredClone(s.metadata);
            const bytes = carrier === 'opaque' ? Uint8Array.from(s.opaque).buffer : kit.projectTransferFromEntries(s.files.map(f => ({path: f.path, bytes: Uint8Array.from(f.bytes).buffer})));
            const inspection = await client.inspectImportedProject(bytes.slice(0), metadata);
            checkTable(inspection.table);
            // Inspection is evidence only; the installation independently admits
            // the same selected bytes/metadata through the required new route.
            const opened = await client.openImportedProject(bytes.slice(0), metadata);
            collection = opened.bootstrap.default_collection; checkTable(opened.table);
            same((await client.observeOccurrence()).revision, 'resident/0', 'fresh revision');
            return bytes;
          };
          try {
            if (phase === 'save') {
              const bytes = await (await fetch(`${origin}/fixtures/${sourceStem}.${format}`)).arrayBuffer();
              const imported = await client.importSpreadsheet(bytes, format, {delimiter: ',', header: true}, {column_types: [['text','text','text']], extra_columns: [[]]});
              metadata = imported.metadata; collection = imported.opened.bootstrap.default_collection;
              checkTable(imported.opened.table);
            } else {
              await openSaved(saved);
            }
            if (mvp && phase === 'save') {
              const fixedEdits = [[0, 0, '000-edited-雪,"first"\r\nkept'], [4203, 1, 'https://example.invalid/edited-middle?x=0007'], [8405, 2, '0001700008405999']];
              for (const [row, column, value] of fixedEdits) {
                const target = (await currentTable()).rows[row].fields[column].target;
                const original = expected[row][column];
                const editBefore = await client.observeOccurrence();
                await timed('edit', () => client.editText(editBefore.revision, target, value));
                expected[row][column] = value; checkTable(await currentTable());
                const editOccurrence = await client.observeOccurrence();
                await timed('undo', () => client.trackerCommand({type: 'undo', expected_revision: editOccurrence.revision}));
                expected[row][column] = original; checkTable(await currentTable());
                const undoOccurrence = await client.observeOccurrence();
                await timed('redo', () => client.trackerCommand({type: 'redo', expected_revision: undoOccurrence.revision}));
                expected[row][column] = value; checkTable(await currentTable());
                const redoOccurrence = await client.observeOccurrence();
                editProofs.push({row, column, target, original, value, actions:['edit','undo','redo'], revisions:[editBefore.revision, editOccurrence.revision, undoOccurrence.revision, redoOccurrence.revision]});
              }
            }
            const first = await snapshot();
            const oldScope = first.occurrence.scope;
            if (phase === 'restart-reopen') check(oldScope !== saved.scope, 'whole-browser/Worker fresh occurrence');
            const last = (await currentTable()).rows.at(-1).fields[0].target;
            const original = expected.at(-1)[0];
            const states = [original, 'history-one雪&<>', 'history-two雪&<>'];
            for (const text of states.slice(1)) {
              await client.editText((await client.observeOccurrence()).revision, last, text);
              expected.at(-1)[0] = text; checkTable(await currentTable());
            }
            await client.trackerCommand({type: 'undo', expected_revision: (await client.observeOccurrence()).revision});
            expected.at(-1)[0] = states[1]; checkTable(await currentTable());
            const before = await snapshot(); // Both Undo and Redo now populated.
            const savedBytes = carrier === 'opaque' ? Uint8Array.from(before.opaque).buffer : kit.projectTransferFromEntries(before.files.map(f => ({path: f.path, bytes: Uint8Array.from(f.bytes).buffer})));
            const bad = structuredClone(metadata); bad.version = 2;
            await fail(() => client.inspectImportedProject(savedBytes.slice(0), bad));
            same(await snapshot(), before, 'invalid inspection preserves every resident/export field');
            await fail(() => client.openImportedProject(savedBytes.slice(0), bad));
            same(await snapshot(), before, 'invalid metadata preserves every resident/export field');
            if (mvp) for (const file of [`malformed.${format}`, 'source-8407.' + format]) {
              const response = await fetch(`${origin}/fixtures/${file}`);
              check(response.ok, `required refusal fixture unavailable: ${file}`);
              const source = await response.arrayBuffer();
              await fail(() => client.importSpreadsheet(source, format, {delimiter: ',', header: true}, {column_types:[['text','text','text']], extra_columns:[[]]}));
              same(await snapshot(), before, `${file} refusal preserves resident/revision/history/exports`);
            }
            if (expected.length === 66) {
              const all = await currentTable();
              await fail(async () => {
              const preview = await client.previewCleanup(before.occurrence.revision, {kind:'deduplicate', entities:all.rows.map(r=>r.id), key_fields:all.columns.map(c=>c.id)});
              await client.commitCleanup(preview.revision, preview.preview_id);
            });
              same(await snapshot(), before, '66→65 refusal preserves complete resident/exports');
            }
            for (const [redo, index] of [[true,2],[false,1],[false,0],[true,1],[true,2]]) {
              await client.trackerCommand({type: redo ? 'redo' : 'undo', expected_revision: (await client.observeOccurrence()).revision});
              expected.at(-1)[0] = states[index]; checkTable(await currentTable());
            }
            const final = await snapshot();
            same(ordinaryOpenCalls, 0, 'no ordinary-open fallback');
            return {saved:{metadata, opaque:final.opaque, files:final.files, expected, scope:final.occurrence.scope}, captures, editProofs, durationsMs};
          } finally { await client.close(); }
        }, {origin, fixture, format, carrier, phase, saved, sourceStem, mvp});
        assert.equal(workerUrls.length, 1, 'one actual Worker per browser phase');
        assert(workerUrls[0].endsWith('/kit/experimental-client.worker.js'));
        saved = result.saved;
        await writeFile(`${roots.capture}/${format}-${carrier}-${phase}-saved.json`, JSON.stringify(saved));
        for (const capture of result.captures) {
          // Unique stage IDs preserve every export and its independently held
          // expected matrix, including legitimate changes between edit states.
          const output = `${roots.capture}/${capture.name}`;
          const bytes = Uint8Array.from(capture.bytes);
          try { assert.deepEqual(await readFile(output), Buffer.from(bytes)); }
          catch (error) { if (error.code !== 'ENOENT') throw error; await writeFile(output, bytes); }
          await writeFile(`${output}.expected.json`, JSON.stringify(capture.expected));
        }
        phaseReceipt = {format, carrier, phase, mvp, fixtureRows:fixture.rows.length, fixtureRecipe:fixture.recipe, workerUrls, browser:browser.version(), environment, durationsMs:result.durationsMs, editProofs:result.editProofs, rss:{status:'UNVERIFIED', reason:'CDP observed 7 Chromium processes while /proc observed 9; platform and process coverage differ, so whole-browser RSS is not established.'}, outcome:'PASS'};
      } finally {
        await browser.close();
        if (phaseReceipt) phaseReceipt.phaseElapsedMs = Date.now() - phaseStarted;
      }
      receipt.push(phaseReceipt);
    }
  }
  await writeFile(`${roots.capture}/worker-result.json`, JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify(receipt));
} finally { await new Promise(done => server.close(done)); }
