// Exercise the exported production Worker/client/WASM stack in Node threads.
// Only the browser Worker event transport is adapted; semantic work stays in WASM.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { test } from "node:test";

if (!isMainThread) {
  globalThis.self = {
    addEventListener(type, listener) {
      if (type === "message") parentPort.on("message", data => listener({data}));
    },
    postMessage(data, transfer) { parentPort.postMessage(data, transfer); },
  };
  const {startDesignerWorker} = await import(pathToFileURL(resolve(workerData.kit, "runtime/worker-runtime.js")));
  const wasm = await readFile(resolve(workerData.kit, "designer_runtime.wasm"));
  startDesignerWorker(`data:application/wasm;base64,${wasm.toString("base64")}`);
} else {
  const kit = resolve(process.argv[2]);
  const {WorkerDesignerClient} = await import(pathToFileURL(resolve(kit, "runtime/worker-client.js")));
  function client() {
    return new WorkerDesignerClient(() => {
      const worker = new Worker(new URL(import.meta.url), {workerData: {kit}});
      return {
        addEventListener(type, listener) {
          worker.on(type, value => listener(type === "message" ? {data: value} : {message: value.message}));
        },
        postMessage(data, transfer) { worker.postMessage(data, transfer); },
        terminate() { void worker.terminate(); },
      };
    });
  }
  for (const history of [false, true]) {
    test(`last-name removal and ordinary edit; undo/redo=${history}`, async () => {
      const builder = client();
      let tree;
      try {
        await builder.newTable("Orders", [{name: "Item", field_type: "text"}, {name: "Other", field_type: "text"}]);
        tree = await builder.exportCanonicalTree("resident/0");
      } finally { await builder.close(); }
      for (const file of tree.files) {
        if (file.path !== "schemas.json") continue;
        const schemas = JSON.parse(new TextDecoder().decode(file.bytes));
        for (const schema of schemas) { delete schema.display_name; delete schema.fields[1].display_name; }
        file.bytes = new TextEncoder().encode(`${JSON.stringify(schemas, null, 2)}\n`).buffer;
      }
      const runtime = client();
      try {
        const opened = await runtime.openCanonicalTree(tree.files);
        const command = runtime.trackerCommand.bind(runtime);
        let publication = await command({type: "remove_column", expected_revision: "resident/0", collection: "orders", field: opened.table.columns[0].id});
        if (history) {
          publication = await command({type: "undo", expected_revision: publication.resulting_revision});
          publication = await command({type: "redo", expected_revision: publication.resulting_revision});
        }
        publication = await command({type: "insert_row", expected_revision: publication.resulting_revision, collection: "orders", initializers: [{field: opened.table.columns[1].id, input: {kind: "text", value: "Item"}}]});
        if (history) {
          publication = await command({type: "undo", expected_revision: publication.resulting_revision});
          await command({type: "redo", expected_revision: publication.resulting_revision});
        }
        const table = await runtime.queryTable("orders");
        assert.deepEqual(table.columns.map(column => column.display_name), ["other"]);
        assert.equal(table.rows.length, 1);
        assert.deepEqual(table.rows[0].fields[0].stored, {kind: "text", value: "Item"});
      } finally { await runtime.close(); }
    });
  }
  test("empty-table rename carries schema-field invalidation without requery", async () => {
    const runtime = client();
    try {
      const opened = await runtime.newTable("Orders", [{name: "Item", field_type: "text"}]);
      const publication = await runtime.trackerCommand({type: "rename_column", expected_revision: "resident/0", collection: "orders", field: opened.table.columns[0].id, name: "Item Name"});
      assert.deepEqual(publication.schema_fields, [{schema: opened.table.collection.id, field: opened.table.columns[0].id}]);
      assert.deepEqual(publication.entities, []);
      assert.deepEqual(publication.fields, []);
      assert.deepEqual(publication.affected_calculations, []);
    } finally { await runtime.close(); }
  });
}
