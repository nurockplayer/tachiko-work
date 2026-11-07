// Exercise the working-size profile over the actual wasm32 ABI.
// Usage: node working_size_wire.mjs <designer_runtime.wasm>
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";

const {instance: {exports: wasm}} = await WebAssembly.instantiate(await readFile(process.argv[2]), {});
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function put(arena, bytes) {
  const data = Uint8Array.from(bytes);
  const pointer = wasm[`tachiko_designer_${arena}_reserve`](data.length);
  new Uint8Array(wasm.memory.buffer, pointer, data.length).set(data);
}
function take(arena) {
  return new Uint8Array(wasm.memory.buffer,
    wasm[`tachiko_designer_${arena}_ptr`](), wasm[`tachiko_designer_${arena}_len`]()).slice();
}
function reply() {
  return JSON.parse(decoder.decode(take("response")));
}
function request(input) {
  put("request", encoder.encode(JSON.stringify(input)));
  wasm.tachiko_designer_request_run();
  return reply();
}
function ok(input) {
  const result = request(input);
  assert.equal(result.status, "ok", JSON.stringify(result));
  return result.response.payload;
}
function openedWindow(result) {
  assert.equal(result.status, "ok", JSON.stringify(result));
  assert.equal(result.response.type, "opened_window", JSON.stringify(result));
  assert.equal(result.response.payload.profile, "working_size");
  assert.equal(result.response.payload.table, undefined, "a working-size reply carries no whole table");
  return result.response.payload;
}
function reopen(operation, bytes, occurrence) {
  put("project", bytes);
  put("request", encoder.encode(occurrence));
  wasm[`tachiko_designer_${operation}`]();
  return openedWindow(reply());
}

// A host asks for the working-size profile when the occurrence is created.
const first = "00000000-0000-4000-8000-000000000901";
const opened = ok({
  type: "new_table", occurrence_id: first, name: "Wire",
  columns: [{name: "name", field_type: "text"}, {name: "qty", field_type: "number"}],
  profile: "working_size",
});
assert.equal(opened.window.total_rows, 0);
const collection = opened.bootstrap.default_collection;
const inserted = ok({
  type: "insert_row", expected_revision: opened.bootstrap.revision, collection,
  initializers: opened.window.columns.map((column, index) => ({
    field: column.id, input: index === 0 ? {kind: "text", value: "wire"} : {kind: "number", input: "2"},
  })),
});

// The saved v3 bundle reopens at working size through inspect and open.
put("request", encoder.encode(inserted.resulting_revision));
wasm.tachiko_designer_project_export_v3();
const exported = reply();
assert.equal(exported.status, "ok", JSON.stringify(exported));
const bundle = take("project");

const inspected = reopen("project_inspect_working_size", bundle, "");
assert.equal(inspected.window.total_rows, 1);
const reopened = reopen("project_open_working_size", bundle, "00000000-0000-4000-8000-000000000902");
assert.equal(reopened.window.total_rows, 1);
assert.equal(reopened.window.rows.length, 1);

// The portable package travels through both working-size open routes.
put("request", encoder.encode(reopened.bootstrap.revision));
wasm.tachiko_designer_portable_ro_export();
const portableReply = reply();
assert.equal(portableReply.status, "ok", JSON.stringify(portableReply));
const portable = take("project");
assert.deepEqual([...portable.slice(0, 2)], [0x50, 0x4b]);

const fromPortable = reopen("portable_ro_open_working_size", portable, "00000000-0000-4000-8000-000000000903");
assert.equal(fromPortable.window.total_rows, 1);
const fromLocal = reopen("project_open_local_document_working_size", portable, "00000000-0000-4000-8000-000000000904");
assert.equal(fromLocal.window.total_rows, 1);

// The spreadsheet bridge imports with the profile, replies one window, and the
// saved import reopens at working size through inspect and open.
function spreadsheet(input, bytes) {
  put("request", encoder.encode(JSON.stringify(input)));
  put("project", bytes);
  wasm.tachiko_designer_spreadsheet_run();
  return reply();
}
const csvBytes = encoder.encode("Name,Note\nAlpha,first\nBeta,second\n");
const importedReply = spreadsheet({
  type: "import", format: "csv", csv_options: {delimiter: ",", header: true},
  selection: {column_types: [["text", "text"]], extra_columns: [[]]},
  occurrence_id: "00000000-0000-4000-8000-000000000906",
  install: true, profile: "working_size",
}, csvBytes);
assert.equal(importedReply.status, "ok", JSON.stringify(importedReply));
assert.equal(importedReply.response.type, "imported_window", JSON.stringify(importedReply));
const imported = importedReply.response.payload;
assert.equal(imported.opened.profile, "working_size");
assert.equal(imported.opened.window.total_rows, 2);
assert.equal(imported.opened.table, undefined, "an imported working-size reply carries no whole table");

put("request", encoder.encode(imported.opened.bootstrap.revision));
wasm.tachiko_designer_project_export_v3();
const importedExport = reply();
assert.equal(importedExport.status, "ok", JSON.stringify(importedExport));
const importedBundle = take("project");

const inspectedImport = spreadsheet({
  type: "inspect_project", metadata: imported.metadata, profile: "working_size",
}, importedBundle);
assert.equal(inspectedImport.status, "ok", JSON.stringify(inspectedImport));
assert.equal(inspectedImport.response.type, "opened_window");
assert.equal(inspectedImport.response.payload.window.total_rows, 2);

const reopenedImport = spreadsheet({
  type: "open_project",
  occurrence_id: "00000000-0000-4000-8000-000000000907",
  metadata: imported.metadata, profile: "working_size",
}, importedBundle);
assert.equal(reopenedImport.status, "ok", JSON.stringify(reopenedImport));
assert.equal(reopenedImport.response.type, "opened_window");
assert.equal(reopenedImport.response.payload.profile, "working_size");
assert.equal(reopenedImport.response.payload.window.total_rows, 2);

// A host that never asks keeps the ordinary reply and profile.
const ordinary = ok({
  type: "new_table", occurrence_id: "00000000-0000-4000-8000-000000000905", name: "Ordinary",
  columns: [{name: "qty", field_type: "number"}],
});
assert.equal(ordinary.table.collection.id.length > 0, true);
console.log("working_size actual WASM ABI: profile create, save, inspect, open, both portable routes and the spreadsheet import route passed");
