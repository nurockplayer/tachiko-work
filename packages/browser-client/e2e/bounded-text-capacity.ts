// Standalone real Chromium/moduleWorker qualification, run using Node 24's
// TypeScript stripping. No injected runtime, mocked Worker, or modified kit.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { mkdir, open, readFile, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { chromium } from "@playwright/test";
import type { ExperimentalDesignerClient } from "../src/experimental-client.ts";
import type { ImportedProjection, InteropMetadata, SpreadsheetFormat } from "../src/runtime/interop-protocol.ts";
import type { TableProjection } from "../src/runtime/protocol.ts";

type Manifest = { rows: string[][]; edits: [number, number, string][]; promoted_opened_rows: string[][] };
const [kitArg, fixtureArg, captureArg] = process.argv.slice(2);
assert(kitArg && fixtureArg && captureArg, "KIT FIXTURES NEW_CAPTURE required");
const roots = { kit: resolve(kitArg), fixtures: resolve(fixtureArg), capture: resolve(captureArg) };
await mkdir(roots.capture, { recursive: false });
const manifest = JSON.parse(await readFile(`${roots.fixtures}/manifest.json`, "utf8")) as Manifest;
assert.equal(manifest.rows.length, 8406);
const sourceHashes = new Map<string, string>();
for (const format of ["csv", "xlsx"]) {
  const bytes = await readFile(`${roots.fixtures}/source-8406.${format}`);
  sourceHashes.set(format, createHash("sha256").update(bytes).digest("hex"));
}
const server = createServer(async (request, response) => {
  try {
    const parts = new URL(request.url ?? "/", "http://localhost").pathname.split("/").filter(Boolean);
    if (parts.length === 0) { response.end("<!doctype html><title>Capacity acceptance</title>"); return; }
    const root = roots[parts.shift() as keyof typeof roots];
    assert(root);
    const path = resolve(root, ...parts);
    assert(path.startsWith(root + sep));
    const body = await readFile(path);
    response.setHeader("Content-Type", path.endsWith(".wasm") ? "application/wasm" : path.endsWith(".js") ? "text/javascript" : "application/octet-stream");
    response.end(body);
  } catch { response.statusCode = 404; response.end("missing"); }
});
await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
const address = server.address();
assert(address && typeof address !== "string");
const origin = `http://127.0.0.1:${address.port}`;
const receipts: unknown[] = [];

try {
  for (const format of ["csv", "xlsx"] as const) {
    // Closing the whole browser after import forces fresh host/Worker/WASM state.
    for (const mode of ["import", "reopen"] as const) {
      const browser = await chromium.launch({ headless: true, ...(process.env.CAPACITY_CHROMIUM ? { executablePath: process.env.CAPACITY_CHROMIUM } : {}) });
      const browserSession = await browser.newBrowserCDPSession();
      const samples: { at: number; rss: number }[] = [];
      let pendingSample: Promise<void> | undefined;
      let sampleError: unknown;
      const sample = () => {
        if (pendingSample) return pendingSample;
        pendingSample = (async () => {
          const { processInfo } = await browserSession.send("SystemInfo.getProcessInfo");
          const memory = await Promise.all(processInfo.map(async ({ id }) => {
            try {
              const status = await readFile(`/proc/${id}/status`, "utf8");
              return Number(status.match(/^VmRSS:\s+(\d+) kB$/m)?.[1] ?? 0) * 1024;
            } catch { return 0; }
          }));
          samples.push({ at: Date.now(), rss: memory.reduce((a, b) => a + b, 0) });
        })().finally(() => { pendingSample = undefined; });
        return pendingSample;
      };
      await sample();
      const sampler = setInterval(() => { void sample().catch((error: unknown) => { sampleError = error; }); }, 100);
      try {
        const page = await browser.newPage();
        // CDP pauses the actual exported-kit Worker before startup. The observer
        // retains only its exported Memory; it does not replace Rust/bridge calls.
        // Artifact bytes stay unchanged. Measurements disclose this instrumentation.
        const pageSession = await page.context().newCDPSession(page);
        let commandId = 0;
        const pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
        const workerSessions: string[] = [];
        const memorySamples: { stage: string; at: number; bytes: number; worker: number }[] = [];
        let observerError: unknown;
        pageSession.on("Target.receivedMessageFromTarget", ({ message }) => {
          const reply = JSON.parse(message) as { id?: number; result?: unknown; error?: { message: string } };
          if (reply.id === undefined) return;
          const request = pending.get(reply.id);
          if (!request) return;
          pending.delete(reply.id);
          if (reply.error) request.reject(new Error(reply.error.message)); else request.resolve(reply.result);
        });
        const workerCommand = (sessionId: string, method: string, params = {}): Promise<unknown> => {
          const id = ++commandId;
          return new Promise((resolve, reject) => {
            pending.set(id, { resolve, reject });
            void pageSession.send("Target.sendMessageToTarget", { sessionId, message: JSON.stringify({ id, method, params }) }).catch(reject);
          });
        };
        pageSession.on("Target.attachedToTarget", ({ sessionId, targetInfo }) => {
          if (targetInfo.type !== "worker") return;
          workerSessions.push(sessionId);
          void (async () => {
            await workerCommand(sessionId, "Runtime.evaluate", { expression: `(() => {
              const instantiate = WebAssembly.instantiateStreaming;
              WebAssembly.instantiateStreaming = async function (...args) {
                const result = await instantiate.apply(this, args);
                globalThis.__capacityObservedMemory = result.instance.exports.memory;
                return result;
              };
            })()` });
            await workerCommand(sessionId, "Runtime.runIfWaitingForDebugger");
          })().catch((error: unknown) => { observerError = error; });
        });
        await pageSession.send("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: true, flatten: false });
        await page.exposeBinding("capacityCheckpoint", async (_source, stage: string) => {
          assert.equal(observerError, undefined);
          assert(workerSessions.length >= 1 && workerSessions.length <= (mode === "import" ? 2 : 1));
          const observed = await workerCommand(workerSessions.at(-1)!, "Runtime.evaluate", { expression: "globalThis.__capacityObservedMemory.buffer.byteLength", returnByValue: true }) as { result: { value?: number }; exceptionDetails?: unknown };
          assert.equal(observed.exceptionDetails, undefined);
          assert(typeof observed.result.value === "number" && observed.result.value > 0);
          memorySamples.push({ stage, at: Date.now(), bytes: observed.result.value, worker: workerSessions.length });
          await sample();
        });
        const workerUrls: string[] = [];
        const workerTerminations: Promise<void>[] = [];
        page.on("worker", (worker) => {
          workerUrls.push(worker.url());
          workerTerminations.push(new Promise<void>((done) => worker.once("close", done)));
        });
        await page.exposeBinding("capacityWorkerTerminated", async () => {
          assert.equal(workerTerminations.length, 1, "preflight Worker must terminate before replacement");
          await new Promise<void>((done, reject) => {
            const timeout = setTimeout(() => reject(new Error("Worker termination timeout")), 1000);
            void workerTerminations[0]!.then(() => { clearTimeout(timeout); done(); });
          });
        });
        await page.goto(origin);
        const result = await page.evaluate(async ({ origin, format, mode, oracle }) => {
          const check: (condition: unknown, message: string) => asserts condition = (condition, message) => {
            if (!condition) throw new Error(message);
          };
          const same = (actual: unknown, expected: unknown, label: string) => check(JSON.stringify(actual) === JSON.stringify(expected), label);
          const { createExperimentalDesignerClient } = await import(`${origin}/kit/experimental-client.js`) as {
            createExperimentalDesignerClient: () => ExperimentalDesignerClient;
          };
          const timings: Record<string, number[]> = {};
          const timed = async <T>(name: string, action: () => Promise<T>, expected?: { target: { entity: string; field: string }; value: string }) => {
            const before = expected ? await client.observeOccurrence() : undefined;
            const start = performance.now();
            try {
              const result = await action();
              if (["edit", "undo", "redo"].includes(name)) {
                const publication = result as { base_revision: string; resulting_revision: string; fields: { entity: string; field: string }[] };
                check(expected, "selective response expected oracle");
                const after = await client.observeOccurrence();
                same(after.scope, before!.scope, "publication occurrence");
                same(publication.base_revision, before!.revision, "publication base");
                same(after.revision, publication.resulting_revision, "publication resident revision");
                same(after.revision, `resident/${Number(before!.revision.split("/")[1]) + 1}`, "revision increments once");
                same(publication.fields, [expected.target], "publication changed target");
                const selected = await client.queryFields(publication.resulting_revision, publication.fields);
                same(selected.revision, publication.resulting_revision, "selective response revision");
                same(selected.fields.length, publication.fields.length, "selective response count");
                same(selected.fields[0]!.target, expected.target, "selective target");
                same(selected.fields[0]!.stored, { kind: "text", value: expected.value }, "selective Text value");
                same(selected.fields[0]!.formula, null, "selective formula absence");
                same(selected.fields[0]!.diagnostics, [], "selective diagnostics");
              }
              return result;
            }
            finally { (timings[name] ??= []).push(performance.now() - start); }
          };
          const heartbeat: number[] = [];
          let lastBeat = performance.now();
          const pulse = setInterval(() => {
            const now = performance.now(); heartbeat.push(now - lastBeat); lastBeat = now;
          }, 16);
          let client = createExperimentalDesignerClient();
          let metadata: InteropMetadata;
          let collection = "";
          let expected = structuredClone(oracle.rows);
          const bytes = async (path: string) => (await fetch(`${origin}/${path}`)).arrayBuffer();
          const digest = async (input: ArrayBuffer) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", input))).join(",");
          const checkTable = (table: TableProjection, expectedRows = expected) => {
            same(table.rows.length, expectedRows.length, "row count");
            same(table.collection.entity_count, expectedRows.length, "entity count");
            same(table.columns.map((c) => c.field_type), ["text", "text", "text"], "Text schema");
            const sheet = metadata.sheets[0]!;
            same(sheet.columns.map((column) => column.name), ["account_id", "profile_url", "unix_timestamp"], "header labels");
            same(table.collection.id, sheet.schema_id, "collection identity");
            same(table.collection.key, collection, "query collection key");
            same(table.columns.map((c) => c.id), sheet.columns.map((c) => c.field_id), "field identities");
            same(table.rows.map((r) => r.id), sheet.rows.map((r) => r.entity_id), "row identities/order");
            table.rows.forEach((row, r) => {
              same(row.fields.length, 3, "field count");
              row.fields.forEach((field, c) => {
                same(field.target, { entity: row.id, field: table.columns[c]!.id }, "target identity");
                same(field.stored, { kind: "text", value: expectedRows[r]![c] }, `cell ${r},${c}`);
                same(field.formula, null, "no formulas"); same(field.diagnostics, [], "no diagnostics");
              });
            });
          };
          const options = { delimiter: ",", header: true };
          const selection = { column_types: [["text", "text", "text"]] as ["text", "text", "text"][], extra_columns: [[]] };
          let saved: ArrayBuffer;
          let originalScope: string;
          const artifacts: { name: string; bytes: Uint8Array }[] = [];
          const smallProjections: { recipe: string; cells: number; imported: number; opened: number; reopened: number; workerRestarted: boolean }[] = [];
          const checkpoint = (stage: string) => (globalThis as unknown as { capacityCheckpoint(stage: string): Promise<void> }).capacityCheckpoint(stage);
          const smallClosure = async (imported: ImportedProjection, rows: string[][], restart: boolean) => {
            metadata = imported.metadata;
            collection = imported.opened.bootstrap.default_collection;
            checkTable(imported.opened.table, rows);
            const size = (value: unknown) => new TextEncoder().encode(JSON.stringify(value)).length;
            // The actual public payload alone exceeds the old complete reply limit.
            // Imported promotion does not imply Opened promotion on fresh reopen.
            const importedBytes = size(imported), openedBytes = size(imported.opened);
            check(importedBytes > 65_536, "small promoted Imported payload");
            if (restart) check(openedBytes > 65_536, "small promoted Opened payload");
            const recipe = restart ? "promoted-opened" : "small";
            const smallSaved = (await timed(`${recipe}_save`, () => client.exportProject(imported.opened.bootstrap.revision))).bytes;
            const before = await client.observeOccurrence();
            if (restart) await checkpoint("promoted-opened-before-termination");
            await client.closeProject();
            if (restart) {
              await client.close();
              await (globalThis as unknown as { capacityWorkerTerminated(): Promise<void> }).capacityWorkerTerminated();
              client = createExperimentalDesignerClient();
            }
            const reopened = await timed(`${recipe}_reopen`, () => client.openProject(smallSaved));
            checkTable(reopened.table, rows);
            const current = await client.observeOccurrence();
            check(current.scope !== before.scope, "small fresh occurrence");
            same(current.revision, "resident/0", "small fresh revision");
            if (restart) check(size(reopened) > 65_536, "fresh Worker promoted Opened payload");
            const smallMetadata = structuredClone(metadata);
            smallMetadata.sheets[0]!.name = "Capacity\tname\n雪&\r";
            const exported = await timed(`${recipe}_export_xlsx`, () => client.exportSpreadsheet!(current.revision, smallMetadata, "xlsx", smallMetadata.sheets[0]!.schema_id));
            artifacts.push({ name: `${recipe}-capacity-from-${format}.xlsx`, bytes: new Uint8Array(exported.bytes) });
            const reimported = await timed(`${recipe}_reimport`, () => client.importSpreadsheet!(exported.bytes, "xlsx", options, selection));
            metadata = reimported.metadata;
            collection = reimported.opened.bootstrap.default_collection;
            same(metadata.sheets[0]!.name, smallMetadata.sheets[0]!.name, "small worksheet-name fidelity");
            checkTable(reimported.opened.table, rows);
            smallProjections.push({ recipe, cells: rows.length * 3, imported: importedBytes, opened: openedBytes, reopened: size(reopened), workerRestarted: restart });
          };
          try {
            if (mode === "import") {
              const promotedInput = await bytes(`fixtures/promoted-opened.${format}`);
              await smallClosure(await timed("promoted-opened_import", () => client.importSpreadsheet!(promotedInput, format, options, selection)), oracle.promoted_opened_rows, true);
              for (const count of [8, 64, 65, 128, 129, 1024, 1025, 8406]) {
                const input = await bytes(`fixtures/source-${count}.${format}`);
                const inspected = await timed("inspect", () => client.inspectSpreadsheet!(input, format, options));
                same(inspected.sheets[0]!.rows.length, count, "inspected count");
                check(!inspected.ledger.some((finding) => finding.blocking), "blocking ledger");
                const imported = await timed("import", () => client.importSpreadsheet!(input, format, options, selection));
                metadata = imported.metadata;
                collection = imported.opened.bootstrap.default_collection;
                checkTable(imported.opened.table, expected.slice(0, count));
                if (count === 64) {
                  await smallClosure(imported, expected.slice(0, count), false);
                }
              }
              originalScope = (await client.observeOccurrence()).scope;
              await checkpoint("imported");
              for (const [r, c, value] of oracle.edits) {
                const before = await client.queryTable(collection);
                const target = before.rows[r]!.fields[c]!.target;
                const published = await timed("edit", () => client.editText(before.revision, target, value), { target, value });
                same(published.base_revision, before.revision, "base revision");
                check(published.resulting_revision !== before.revision, "revision advances");
                const undone = await timed("undo", () => client.trackerCommand!({ type: "undo", expected_revision: published.resulting_revision }), { target, value: expected[r]![c]! });
                checkTable(await client.queryTable(collection));
                await timed("redo", () => client.trackerCommand!({ type: "redo", expected_revision: undone.resulting_revision }), { target, value });
                expected[r]![c] = value;
                checkTable(await client.queryTable(collection));
              }
              const target = (await client.queryTable(collection)).rows[0]!.fields[0]!.target;
              let revision = (await client.observeOccurrence()).revision;
              for (let i = 0; i < 128; i++) {
                const value = i === 127 ? expected[0]![0]! : `history-${String(i).padStart(3, "0")}`;
                const published = await timed("edit", () => client.editText(revision, target, value), { target, value });
                same(published.base_revision, revision, "history base revision");
                check(published.resulting_revision !== revision, "history advances revision");
                revision = published.resulting_revision;
                if (i === 63 || i === 127) await checkpoint(`history-${i + 1}`);
              }
              checkTable(await client.queryTable(collection));
              same((await client.observeOccurrence()).scope, originalScope, "edits preserve occurrence");
              saved = (await timed("save", () => client.exportProject(revision))).bytes;
              const savedHash = await digest(saved);
              const exportHashes: Record<string, string> = {};
              for (const output of ["csv", "xlsx"] as const) exportHashes[output] = await digest((await client.exportSpreadsheet!(revision, metadata!, output, metadata!.sheets[0]!.schema_id)).bytes);
              const preserve = async (action: () => Promise<unknown>) => {
                const before = await client.observeOccurrence();
                let rejected = false;
                try { await action(); } catch { rejected = true; }
                check(rejected, "invalid operation must reject");
                same(await client.observeOccurrence(), before, "rejection preserves occurrence/revision");
                checkTable(await client.queryTable(collection));
                same(await digest((await client.exportProject(revision)).bytes), savedHash, "rejection preserves canonical bytes");
                for (const output of ["csv", "xlsx"] as const) same(await digest((await client.exportSpreadsheet!(revision, metadata!, output, metadata!.sheets[0]!.schema_id)).bytes), exportHashes[output], "rejection preserves spreadsheet export bytes");
                const undone = await timed("undo", () => client.trackerCommand!({ type: "undo", expected_revision: revision }), { target, value: "history-126" });
                const prior = structuredClone(expected); prior[0]![0] = "history-126";
                checkTable(await client.queryTable(collection), prior);
                const redone = await timed("redo", () => client.trackerCommand!({ type: "redo", expected_revision: undone.resulting_revision }), { target, value: expected[0]![0]! });
                revision = redone.resulting_revision;
                checkTable(await client.queryTable(collection));
              };
              for (const file of [`source-8407.${format}`, `malformed.${format}`]) {
                const invalid = await bytes(`fixtures/${file}`);
                await preserve(() => client.inspectSpreadsheet!(invalid, format, options));
                await preserve(() => client.importSpreadsheet!(invalid, format, options, selection));
              }
              await preserve(() => client.openProject(saved.slice(0, saved.byteLength - 1)));
              await preserve(() => client.editText("resident/0", target, "stale must fail"));
              await preserve(() => client.queryFields("resident/0", [target]));
              checkTable((await timed("inspect_project", () => client.inspectImportedProject!(saved, metadata!))).table);
            } else {
              metadata = await (await fetch(`${origin}/capture/${format}-metadata.json`)).json() as InteropMetadata;
              collection = await (await fetch(`${origin}/capture/${format}-collection.txt`)).text();
              saved = await bytes(`capture/${format}-saved.project`);
              for (const [r, c, value] of oracle.edits) expected[r]![c] = value;
              const previous = await (await fetch(`${origin}/capture/${format}-scope.json`)).json() as { scope: string };
              const scopes = new Set([previous.scope]);
              for (let cycle = 0; cycle < 10; cycle++) {
                const opened = await timed("reopen", () => client.openProject(saved));
                checkTable(opened.table);
                const occurrence = await client.observeOccurrence();
                same(occurrence.revision, "resident/0", "fresh revision");
                check(!scopes.has(occurrence.scope), "fresh occurrence"); scopes.add(occurrence.scope);
                same(await digest((await timed("resave", () => client.exportProject("resident/0"))).bytes), await digest(saved), "fresh save equality");
                if (cycle === 0 || cycle === 4 || cycle === 9) await checkpoint(`reopen-${cycle + 1}`);
                if (cycle < 9) await client.closeProject();
              }
              originalScope = (await client.observeOccurrence()).scope;
            }
            const revision = (await client.observeOccurrence()).revision;
            for (const output of ["csv", "xlsx"] as SpreadsheetFormat[]) {
              const exported = await timed(`export_${output}`, () => client.exportSpreadsheet!(revision, metadata!, output, metadata!.sheets[0]!.schema_id));
              check(!exported.ledger.some((finding) => finding.blocking), "export blocking ledger");
              artifacts.push({ name: `${format}-${mode}-export.${output}`, bytes: new Uint8Array(exported.bytes) });
            }
            await checkpoint("exported");
            return { timings, heartbeat, artifacts, smallProjections, metadata: JSON.stringify(metadata!), collection, saved: new Uint8Array(saved), scope: originalScope, cellCount: expected.length * 3 };
          } finally { clearInterval(pulse); await client.closeProject(); await client.close(); }
        }, { origin, format, mode, oracle: manifest });
        for (const artifact of result.artifacts) await writeFile(`${roots.capture}/${artifact.name}`, Buffer.from(artifact.bytes), { flag: "wx" });
        if (mode === "import") {
          const handle = await open(`${roots.capture}/${format}-saved.project`, "wx");
          try { await handle.writeFile(Buffer.from(result.saved)); await handle.sync(); } finally { await handle.close(); }
          await writeFile(`${roots.capture}/${format}-metadata.json`, result.metadata, { flag: "wx" });
          await writeFile(`${roots.capture}/${format}-collection.txt`, result.collection, { flag: "wx" });
          await writeFile(`${roots.capture}/${format}-scope.json`, JSON.stringify({ scope: result.scope }), { flag: "wx" });
        }
        assert.equal(workerUrls.length, mode === "import" ? 2 : 1, "promotion preflight restarts once; main journey retains one Worker");
        assert(workerUrls.every((url) => url === `${origin}/kit/experimental-client.worker.js`));
        clearInterval(sampler);
        await pendingSample;
        await sample();
        assert.equal(sampleError, undefined, "memory sampling must succeed");
        assert.equal(observerError, undefined);
        assert.equal(memorySamples.length, mode === "import" ? 5 : 4);
        assert.deepEqual(memorySamples.map(({ stage }) => stage), mode === "import" ? ["promoted-opened-before-termination", "imported", "history-64", "history-128", "exported"] : ["reopen-1", "reopen-5", "reopen-10", "exported"]);
        assert(memorySamples.slice(-4).every(({ worker }) => worker === workerUrls.length), "all main journey checkpoints retain the same Worker");
        assert.deepEqual(mode === "import" ? ["edit", "undo", "redo"].map((name) => result.timings[name]?.length) : ["reopen", "resave"].map((name) => result.timings[name]?.length), mode === "import" ? [131, 10, 10] : [10, 10], "complete unchanged main operation counts");
        const receipt = { format, mode, browser: browser.version(), workerUrls, smallProjections: result.smallProjections, timings: result.timings, heartbeat: result.heartbeat, memory: samples, wasmMemory: memorySamples, instrumentation: "CDP startup observer retains actual Worker exported WebAssembly.Memory; no artifact rewrite", cells: result.cellCount };
        receipts.push(receipt);
        await writeFile(`${roots.capture}/${format}-${mode}-receipt.json`, JSON.stringify(receipt, null, 2), { flag: "wx" });
        assert(memorySamples.every(({ bytes }) => bytes <= 256 * 1024 * 1024), "actual Worker WASM linear-memory budget");
        const percentile = (values: number[], fraction: number) => values.toSorted((a, b) => a - b)[Math.ceil(values.length * fraction) - 1]!;
        for (const [name, values] of Object.entries(result.timings)) {
          assert(values.length > 0);
          assert(Math.max(...values) <= (["edit", "undo", "redo"].includes(name) ? 1000 : 10_000), `${name} max budget`);
          if (["edit", "undo", "redo"].includes(name)) assert(percentile(values, .95) <= 500, `${name} p95 budget`);
        }
        assert(result.heartbeat.length > 0);
        assert(percentile(result.heartbeat, .95) <= 100 && Math.max(...result.heartbeat) <= 250, "main-thread heartbeat budget");
        assert(samples.length >= 2 && samples.every(({ rss }) => rss > 0), "real Linux process memory evidence");
        assert(Math.max(...samples.map(({ rss }) => rss)) - samples[0]!.rss <= 512 * 1024 * 1024, "process memory increase budget");
      } finally { clearInterval(sampler); await pendingSample; await browser.close(); }
    }
    assert.equal(createHash("sha256").update(await readFile(`${roots.fixtures}/source-8406.${format}`)).digest("hex"), sourceHashes.get(format), "source preserved");
  }
  await writeFile(`${roots.capture}/worker-complete.json`, JSON.stringify({ receipts: receipts.length, cells: 25218, modes: ["import", "fresh-browser-reopen"] }), { flag: "wx" });
} finally { server.close(); }
