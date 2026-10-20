// Replay native host receipts and opaque bytes through the actual wasm32 ABI.
// Usage: node optional_native_persistence.mjs <designer_runtime.wasm> <capture...>
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

const [wasmPath, ...captures] = process.argv.slice(2);
assert.ok(wasmPath && captures.length, "WASM path and at least one native capture required");
const wasm = await readFile(wasmPath);
let count = 0;
for (const path of captures) {
  const records = JSON.parse(await readFile(path, "utf8"));
  const { instance: { exports: e } } = await WebAssembly.instantiate(wasm, {});
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
    if (record.bytes !== null) {
      assert.deepEqual(take("project"), Uint8Array.from(record.bytes),
        `${path}: bytes ${index} (${record.operation})`);
    }
    count++;
  }
  console.log(JSON.stringify({ capture: path, records: records.length }));
}
console.log(JSON.stringify({ records: count, wasm_sha256: createHash("sha256").update(wasm).digest("hex") }));
