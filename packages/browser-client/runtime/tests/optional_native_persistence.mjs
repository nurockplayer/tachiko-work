// Replay native host receipts and opaque bytes through the actual wasm32 ABI.
// Usage: node optional_native_persistence.mjs <designer_runtime.wasm> <capture...>
// Requires all sole/portable and imported structural case combinations emitted by the Rust test.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { basename } from "node:path";

const [wasmPath, ...captures] = process.argv.slice(2);
assert.ok(wasmPath, "WASM path and all native capture cases required");
const expectedCases = [
  "sole-false-portable-false", "sole-false-portable-true",
  "sole-true-portable-false", "sole-true-portable-true",
  ...["csv", "xlsx"].flatMap(source =>
    ["rename", "add_blank", "add_fill", "remove", "insert", "insert_blank", "delete"].map(operation => `${source}-${operation}`)),
  "refusals", "formulas", "cell-compatibility-csv", "cell-compatibility-xlsx",
  "explicit-name-csv", "explicit-name-xlsx",
  "foreign-current", "header-history-collision", "untouched-compatibility", "scalar-compatibility",
  "formula-output-policy", "csv-output-scope", "presentation-output-policy", "style-inventory-policy", "references-headers-policy", "closed-grammar-policy", "boolean-output-policy", "precision-budget-policy",
].sort();
const cases = new Map();
for (const path of captures) {
  const name = expectedCases.find(name => basename(path).endsWith(`.${name}.json`) || basename(path) === `${name}.json`);
  assert.ok(name, `${path}: unrecognized native capture case name`);
  assert.ok(!cases.has(name), `${path}: duplicate native capture case ${name}`);
  cases.set(name, path);
}
assert.deepEqual([...cases.keys()].sort(), expectedCases, "native capture inventory must include all expected cases");
const loaded = [];
for (const [name, path] of cases) {
  const records = JSON.parse(await readFile(path, "utf8"));
  assert.ok(Array.isArray(records) && records.length > 0,
    `${path}: native capture ${name} must contain a nonempty records array`);
  loaded.push({ name, path, records });
}
const wasm = await readFile(wasmPath);
const module = await WebAssembly.compile(wasm);
const exported = new Map(WebAssembly.Module.exports(module).map(({ name, kind }) => [name, kind]));
assert.equal(exported.get("memory"), "memory", "WASM lacks required memory export");
for (const name of [
  "request_reserve", "project_reserve", "response_ptr", "response_len", "project_ptr", "project_len",
  "request_run", "spreadsheet_run", "project_export_v3", "project_open", "portable_ro_export", "portable_ro_open",
]) {
  assert.equal(exported.get(`tachiko_designer_${name}`), "function",
    `WASM lacks required function export tachiko_designer_${name}`);
}
let count = 0;
for (const { name, path, records } of loaded) {
  const { exports: e } = await WebAssembly.instantiate(module, {});
  const put = (arena, bytes) => {
    const data = Uint8Array.from(bytes);
    const ptr = e[`tachiko_designer_${arena}_reserve`](data.length);
    new Uint8Array(e.memory.buffer, ptr, data.length).set(data);
  };
  const take = (arena) => new Uint8Array(e.memory.buffer,
    e[`tachiko_designer_${arena}_ptr`](), e[`tachiko_designer_${arena}_len`]()).slice();
  for (const [index, record] of records.entries()) {
    put("request", record.request);
    put("project", record.project);
    e[`tachiko_designer_${record.operation}`]();
    assert.deepEqual(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(take("response"))),
      record.reply, `${path}: receipt ${index} (${record.operation})`);
    if (record.operation === "spreadsheet_run" && record.reply.status === "error") {
      assert.equal(take("project").length, 0,
        `${path}: refused spreadsheet ${index} must expose no partial output bytes`);
    }
    if (record.bytes !== null) {
      assert.deepEqual(take("project"), Uint8Array.from(record.bytes),
        `${path}: bytes ${index} (${record.operation})`);
    }
    count++;
  }
  console.log(JSON.stringify({ case: name, capture: path, records: records.length }));
}
console.log(JSON.stringify({ records: count, wasm_sha256: createHash("sha256").update(wasm).digest("hex") }));
