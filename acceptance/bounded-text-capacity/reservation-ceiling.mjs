// Run with a fresh WASM instance so other boundary probes cannot mask growth.
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";

const [wasmPath, workbookPath] = process.argv.slice(2);
const {instance: {exports: wasm}} = await WebAssembly.instantiate(await readFile(wasmPath), {});
const source = await readFile(workbookPath);
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const budget = 256 * 1024 * 1024;
let peak = wasm.memory.buffer.byteLength;
const common = {format: "xlsx", csv_options: {delimiter: ",", header: true}};

function run(request) {
  for (const [arena, bytes] of [["request", encoder.encode(JSON.stringify(request))], ["project", source]]) {
    const pointer = wasm[`tachiko_designer_${arena}_reserve`](bytes.length);
    new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
  }
  wasm.tachiko_designer_spreadsheet_run();
  const memory = wasm.memory.buffer.byteLength;
  peak = Math.max(peak, memory);
  assert.ok(memory <= budget, `${request.type} linear memory ${memory} exceeds ${budget} bytes`);
  const reply = JSON.parse(decoder.decode(new Uint8Array(wasm.memory.buffer,
    wasm.tachiko_designer_response_ptr(), wasm.tachiko_designer_response_len())));
  assert.equal(reply.status, "ok", JSON.stringify(reply).slice(0, 900));
  console.log(`${request.type} linear memory: ${memory} bytes`);
  return reply.response.payload;
}

const preview = run({type: "inspect", ...common});
assert.equal(preview.sheets[0].rows.length, 8406);
assert.ok(!preview.ledger.some(finding => finding.blocking));
assert.ok(wasm.memory.buffer.byteLength >= 248 * 1024 * 1024,
  `reservation did not reach the ceiling: ${wasm.memory.buffer.byteLength} bytes`);
const imported = run({type: "import", ...common,
  selection: {column_types: [["text", "text", "text"]], extra_columns: [[]]},
  occurrence_id: "00000000-0000-4000-8000-000000000933", install: true});
assert.equal(imported.opened.table.rows.length, 8406);
console.log(`peak linear memory: ${peak} bytes (${(peak / 1024 / 1024).toFixed(2)} MiB)`);
console.log("admitted XLSX inspect and import stay within 256 MiB: PASS");
