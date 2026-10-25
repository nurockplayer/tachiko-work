// Slice-1 native/WASM replay from HANDBACK-551, with fail-closed input guards.
// Usage: node optional_native_fields.mjs <designer_runtime.wasm> <all eight captures>
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { basename } from "node:path";

const [wasmPath, ...captures] = process.argv.slice(2);
assert.ok(wasmPath, "WASM path and all eight native capture cases required");
const expectedCases = ["native", "capacity", "formula", "rowgate",
  "editability-boolean", "editability-date", "editability-text", "requiredness"];
const cases = new Map();
for (const path of captures) {
  const match = basename(path).match(/\.json(?:\.(capacity|formula|rowgate|editability-(?:boolean|date|text)|requiredness))?$/);
  assert.ok(match, `${path}: unrecognized native capture case name`);
  const name = match[1] ?? "native";
  assert.ok(!cases.has(name), `${path}: duplicate native capture case ${name}`);
  cases.set(name, path);
}
assert.deepEqual([...cases.keys()].sort(), [...expectedCases].sort(),
  "native capture inventory must include all eight expected cases");
const loaded = [];
for (const [name, path] of cases) {
  const transcript = JSON.parse(await readFile(path, "utf8"));
  const records = Array.isArray(transcript) ? transcript : transcript?.records;
  assert.ok(Array.isArray(records) && records.length > 0,
    `${path}: native capture ${name} must contain a nonempty records array`);
  if (!Array.isArray(transcript)) {
    assert.ok(Array.isArray(transcript.project) && transcript.project.length > 0,
      `${path}: native capture must contain its initial project`);
    assert.ok(typeof transcript.occurrence_id === "string" && transcript.occurrence_id.length > 0,
      `${path}: native capture must contain its occurrence ID`);
  }
  loaded.push({ name, path, transcript, records });
}
const wasm = await readFile(wasmPath);
const module = await WebAssembly.compile(wasm);
const exported = new Map(WebAssembly.Module.exports(module).map(({ name, kind }) => [name, kind]));
assert.equal(exported.get("memory"), "memory", "WASM lacks required memory export");
for (const name of ["request_reserve", "project_reserve", "response_ptr", "response_len", "request_run", "project_open"]) {
  assert.equal(exported.get(`tachiko_designer_${name}`), "function",
    `WASM lacks required function export tachiko_designer_${name}`);
}
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const wasmSha256 = createHash("sha256").update(wasm).digest("hex");
let count = 0;
for (const { name, path, transcript, records } of loaded) {
  // Each capture starts in a fresh instance, just as the original one-file tool.
  const { exports: e } = await WebAssembly.instantiate(module, {});
  const put = (arena, bytes) => {
    const pointer = e[`tachiko_designer_${arena}_reserve`](bytes.length);
    assert.notEqual(pointer, 0);
    new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes);
  };
  const reply = () => JSON.parse(decoder.decode(new Uint8Array(e.memory.buffer,
    e.tachiko_designer_response_ptr(), e.tachiko_designer_response_len()).slice()));
  if (!Array.isArray(transcript)) {
    put("project", Uint8Array.from(transcript.project));
    put("request", encoder.encode(transcript.occurrence_id));
    e.tachiko_designer_project_open();
    const opened = reply();
    assert.equal(opened.status, "ok", `${path}: initial open: ${JSON.stringify(opened)}`);
  }
  let accepted = 0, refused = 0;
  for (const [index, record] of records.entries()) {
    put("request", encoder.encode(JSON.stringify(record.request)));
    e.tachiko_designer_request_run();
    const actual = reply();
    assert.deepEqual(actual, record.reply,
      `${path}: native/WASM mismatch at record ${index}: ${JSON.stringify(record.request)}`);
    actual.status === "ok" ? accepted++ : refused++;
  }
  count += records.length;
  console.log(JSON.stringify({ case: name, capture: path, records: records.length, accepted, refused,
    request_types: [...new Set(records.map(record => record.request.type))].sort() }));
}
console.log(JSON.stringify({ records: count, wasm_sha256: wasmSha256 }));
