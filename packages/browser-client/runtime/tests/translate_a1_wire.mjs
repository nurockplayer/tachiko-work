// Exercise the actual ordinary WASM ABI, with live native-table geometry.
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
const {instance: {exports: wasm}} = await WebAssembly.instantiate(await readFile(process.argv[2]), {});
const encoder = new TextEncoder();
const decoder = new TextDecoder();
function request(input) {
  const bytes = encoder.encode(JSON.stringify(input));
  const pointer = wasm.tachiko_designer_request_reserve(bytes.length);
  new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
  wasm.tachiko_designer_request_run();
  return JSON.parse(decoder.decode(new Uint8Array(wasm.memory.buffer, wasm.tachiko_designer_response_ptr(), wasm.tachiko_designer_response_len())));
}
function ok(input) {
  const reply = request(input);
  assert.equal(reply.status, "ok", JSON.stringify(reply));
  return reply.response.payload;
}
const opened = ok({type: "new_table", occurrence_id: "00000000-0000-4000-8000-000000000001", name: "Items",
  columns: ["name", "qty", "price", "total"].map((name, i) => ({name, field_type: i === 0 ? "text" : "number"}))});
const collection = opened.table.collection.id;
let revision = opened.table.revision;
for (let i = 0; i < 3; i++) {
  const publication = ok({type: "insert_row", expected_revision: revision, collection, initializers: []});
  revision = publication.resulting_revision;
}
const table = ok({type: "query_table", collection});
revision = table.revision;
const columns = table.columns.map(column => column.id);
const rows = table.rows.map(row => row.id);
const input = {type: "translate_a1_formula", expected_revision: revision, collection_id: collection,
  row_entity_id: rows[1], target_field_id: columns[3], text: "B3*C3", geometry: {header_row: 1, rows, columns}};
const first = ok(input);
assert.equal(first.source, `[.${table.columns[1].key}]*[.${table.columns[2].key}]`);
assert.deepEqual(first.notes, []);
assert.deepEqual(ok(input), first);
const refusal = request({...input, text: "SUM(B2:B3)"});
assert.equal(refusal.error.code, "unsupported_range", JSON.stringify(refusal));
assert.ok(refusal.error.span.end > refusal.error.span.start);
assert.equal(ok({type: "query_table", collection}).revision, revision);
console.log("translate_a1 actual WASM ABI: 2 acceptance cases and purity passed");
