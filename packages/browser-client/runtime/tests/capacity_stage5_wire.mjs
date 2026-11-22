// Stage 5c actual wasm32 qualification; exact invocation is in capacity_stage5_bench.rs.
import assert from "node:assert/strict";
import {readFile, writeFile} from "node:fs/promises";
import {join} from "node:path";
import {performance} from "node:perf_hooks";

const [wasmPath, directory] = process.argv.slice(2);
const native = JSON.parse(await readFile(join(directory, "native.json"), "utf8"));
const source = await readFile(join(directory, "source.xlsx"));
assert.equal(source.length, native.source_bytes);
const {instance: {exports: wasm}} = await WebAssembly.instantiate(await readFile(wasmPath), {});
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function put(arena, bytes) {
  const pointer = wasm[`tachiko_designer_${arena}_reserve`](bytes.length);
  new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
}
function take(arena) {
  return new Uint8Array(wasm.memory.buffer,
    wasm[`tachiko_designer_${arena}_ptr`](), wasm[`tachiko_designer_${arena}_len`]()).slice();
}
function run(request, bytes) {
  // Match native: input encoding/copy and reply decoding/copy are outside the timer.
  put("request", encoder.encode(JSON.stringify(request)));
  put("project", bytes);
  const start = performance.now();
  wasm.tachiko_designer_spreadsheet_run();
  const elapsed = performance.now() - start;
  const reply = take("response");
  const value = JSON.parse(decoder.decode(reply));
  assert.equal(value.status, "ok", JSON.stringify(value));
  return {payload: value.response.payload, ms: elapsed, bytes: reply.length};
}

const samples = [];
for (let sample = 1; sample <= 3; sample++) {
  const inspected = run(native.inspect_request, source);
  assert.equal(inspected.payload.sheets[0].rows.length, native.rows);
  assert.equal(inspected.payload.sheets[0].columns.length, 30);
  assert.ok(!inspected.payload.ledger.some(finding => finding.blocking));
  const imported = run(native.import_request, source);
  const opened = imported.payload.opened;
  assert.equal(opened.profile, "working_size");
  assert.equal(opened.window.total_rows, native.rows);
  assert.equal(opened.window.columns.length, 30);
  const exported = run({type: "export", format: "xlsx",
    expected_revision: opened.bootstrap.revision,
    collection: opened.bootstrap.default_collection, metadata: imported.payload.metadata}, new Uint8Array());
  const xlsx = take("project");
  assert.equal(xlsx.length, exported.payload.byte_length);
  assert.deepEqual([...xlsx.slice(0, 2)], [0x50, 0x4b]);
  const record = {sample, inspect_ms: inspected.ms, import_ms: imported.ms,
    open_ms: inspected.ms + imported.ms, export_ms: exported.ms,
    reply_bytes: {inspect: inspected.bytes, import: imported.bytes, export: exported.bytes},
    xlsx_bytes: xlsx.length};
  console.log(`wasm ${JSON.stringify(record)}`);
  samples.push(record);
}
const summary = {};
for (const step of ["inspect_ms", "import_ms", "open_ms", "export_ms"]) {
  const times = samples.map(sample => sample[step]).sort((a, b) => a - b);
  summary[step] = {n: times.length, p50_ms: times[Math.floor((times.length - 1) / 2)], max_ms: times.at(-1)};
}
const report = {target: "wasm32-unknown-unknown", rows: native.rows, columns: 30,
  populated_cells: native.populated_cells, source_bytes: source.length, samples, summary};
await writeFile(join(directory, "wasm.json"), JSON.stringify(report, null, 2));
console.log(`wasm summary ${JSON.stringify(summary)}`);
for (const sample of samples) {
  // The existing 48 MiB inspection-preview exception remains; other replies are 16 MiB.
  assert.ok(sample.reply_bytes.inspect <= 48 * 1024 * 1024);
  assert.ok(sample.reply_bytes.import <= 16 * 1024 * 1024);
  assert.ok(sample.reply_bytes.export <= 16 * 1024 * 1024);
}
for (const step of ["open_ms", "export_ms"]) {
  for (const statistic of ["p50_ms", "max_ms"]) {
    assert.ok(summary[step][statistic] <= 2 * native.summary[step][statistic],
      `${step} ${statistic} exceeds 2x native; see wasm.json`);
  }
}
