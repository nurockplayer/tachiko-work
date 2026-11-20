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

// Nothing is open yet, so the bridge advertises the ordinary request limit.
assert.equal(wasm.tachiko_designer_request_limit(), 65_536);

// A host asks for the working-size profile when the occurrence is created.
const first = "00000000-0000-4000-8000-000000000901";
const opened = ok({
  type: "new_table", occurrence_id: first, name: "Wire",
  columns: [{name: "name", field_type: "text"}, {name: "qty", field_type: "number"}],
  profile: "working_size",
});
assert.equal(opened.window.total_rows, 0);
assert.equal(wasm.tachiko_designer_request_limit(), 67_108_864);
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
assert.equal(imported.metadata.version, 2);
assert.equal("rows" in imported.metadata.sheets[0], false);
assert.ok(imported.metadata.sheets[0].columns.every(column => !("width" in column)));
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

// A paste over 64 KiB travels through the working-size request arena.
const pasteCollection = reopenedImport.response.payload.bootstrap.default_collection;
const pasteColumns = reopenedImport.response.payload.window.columns;
const pasteRows = Array.from({length: 3_000}, (_, row) => [
  `pasted-${String(row).padStart(4, "0")}`,
  `note-${String(row).padStart(4, "0")}`,
]);
const paste = {
  type: "paste_cells",
  expected_revision: reopenedImport.response.payload.bootstrap.revision,
  collection: pasteCollection,
  start_field: pasteColumns[0].id,
  rows: pasteRows,
};
const pasteBytes = encoder.encode(JSON.stringify(paste));
assert.ok(pasteBytes.length > 65_536, `paste request is only ${pasteBytes.length} bytes`);
put("request", pasteBytes);
wasm.tachiko_designer_request_run();
const pasted = reply();
assert.equal(pasted.status, "ok", JSON.stringify(pasted));
assert.equal(pasted.response.payload.many, true, JSON.stringify(pasted));
const pastedWindow = ok({
  type: "query_table_window",
  expected_revision: pasted.response.payload.resulting_revision,
  collection: pasteCollection,
  start: 0,
  count: 5,
});
assert.equal(pastedWindow.total_rows, 3_002);
assert.equal(pastedWindow.rows.length, 5);
assert.equal(pastedWindow.rows[0].fields[0].stored.value, "Alpha");
assert.equal(pastedWindow.rows[2].fields[0].stored.value, "pasted-0000");

// A host that never asks keeps the ordinary reply and profile.
const ordinary = ok({
  type: "new_table", occurrence_id: "00000000-0000-4000-8000-000000000905", name: "Ordinary",
  columns: [{name: "qty", field_type: "number"}],
});
assert.equal(ordinary.table.collection.id.length > 0, true);
assert.equal(wasm.tachiko_designer_request_limit(), 65_536);
console.log("working_size actual WASM ABI: profile create, save, inspect, open, both portable routes, the spreadsheet import route and a large paste passed");

// #839: the same compact selection works over the actual wasm32 request ABI.
for (const [rowCount, working] of [[32, false], [10_000, true]]) {
  const source = encoder.encode(`Left,Right\n${"2,4\n".repeat(rowCount)}`);
  const loaded = spreadsheet({
    type: "import", format: "csv", csv_options: {delimiter: ",", header: true},
    selection: {column_types: [["number", "number"]], extra_columns: [[]]},
    occurrence_id: "00000000-0000-4000-8000-000000000839", install: true,
    ...(working ? {profile: "working_size"} : {}),
  }, source);
  assert.equal(loaded.status, "ok", JSON.stringify(loaded));
  const initial = loaded.response.payload.opened;
  const revision = initial.bootstrap.revision;
  const collection = initial.bootstrap.default_collection;
  const firstWindow = ok({type: "query_table_window", expected_revision: revision,
    collection, start: 0, count: 32});
  const lastWindow = ok({type: "query_table_window", expected_revision: revision,
    collection, start: rowCount - 1, count: 1});
  const fields = firstWindow.columns.map(column => column.id);
  const targets = firstWindow.rows.flatMap(row => fields.map(field => ({entity: row.id, field})));
  const perCell = {type: "query_selection_statistics", expected_revision: revision, targets};
  assert.ok(encoder.encode(JSON.stringify(perCell)).length < 65_536);
  const expected = ok(perCell);
  const rangeRequest = {type: "query_selection_statistics", expected_revision: revision,
    range: {collection, first: firstWindow.rows[31].id, last: firstWindow.rows[0].id, fields}};
  assert.deepEqual(ok(rangeRequest), expected);
  rangeRequest.range.first = lastWindow.rows[0].id;
  assert.ok(encoder.encode(JSON.stringify(rangeRequest)).length < 1_024);
  const all = ok(rangeRequest);
  assert.equal(all.count, rowCount * 2);
  assert.equal(all.numerical_count, rowCount * 2);
  assert.equal(all.sum, rowCount * 6);
  assert.equal(all.average, 3);
  if (!working) {
    assert.equal(wasm.tachiko_designer_request_limit(), 65_536);
    const oversized = {...perCell, targets: Array.from({length: 2_000}, () => targets[0])};
    assert.ok(encoder.encode(JSON.stringify(oversized)).length > 65_536);
    assert.equal(request(oversized).error.code, "request_too_large");
  }
}

// Stored-order anchors include a newly inserted row even when its id sorts last.
const rangeTable = ok({type: "new_table",
  occurrence_id: "00000000-0000-4000-8000-000000000840",
  name: "Range order", columns: [{name: "Amount", field_type: "number"}]});
const rangeCollection = rangeTable.bootstrap.default_collection;
const amountField = rangeTable.table.columns[0].id;
let rangeRevision = rangeTable.bootstrap.revision;
for (const value of ["10", "20"]) {
  rangeRevision = ok({type: "insert_row", expected_revision: rangeRevision,
    collection: rangeCollection,
    initializers: [{field: amountField, input: {kind: "number", input: value}}]}).resulting_revision;
}
const beforeInsert = ok({type: "query_table_window", expected_revision: rangeRevision,
  collection: rangeCollection, start: 0, count: 2});
rangeRevision = ok({type: "insert_row", expected_revision: rangeRevision,
  collection: rangeCollection, position: {before: beforeInsert.rows[1].id},
  initializers: [{field: amountField, input: {kind: "number", input: "99"}}]}).resulting_revision;
const rangeWindow = ok({type: "query_table_window", expected_revision: rangeRevision,
  collection: rangeCollection, start: 0, count: 3});
assert.equal(rangeWindow.rows[0].id, beforeInsert.rows[0].id);
assert.equal(rangeWindow.rows[2].id, beforeInsert.rows[1].id);
const storedRange = ok({type: "query_selection_statistics", expected_revision: rangeRevision,
  range: {collection: rangeCollection, first: beforeInsert.rows[1].id,
    last: beforeInsert.rows[0].id, fields: [amountField]}});
assert.equal(storedRange.count, 3);
assert.equal(storedRange.sum, 129);
assert.deepEqual(storedRange, ok({type: "query_selection_statistics", expected_revision: rangeRevision,
  targets: rangeWindow.rows.map(row => ({entity: row.id, field: amountField}))}));
