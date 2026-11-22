// Run after building release WASM. The fixture is a small workbook with a
// stored 1.2 MiB image; see the remote reservation qualification command.
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";

const [wasmPath, imageWorkbookPath, malformedWorkbookPath] = process.argv.slice(2);
const module = await WebAssembly.compile(await readFile(wasmPath));
const request = new TextEncoder().encode(JSON.stringify({
  type: "inspect", format: "xlsx", csv_options: {delimiter: ",", header: true},
}));

async function inspect(bytes) {
  const {exports: wasm} = await WebAssembly.instantiate(module, {});
  function put(arena, value) {
    const pointer = wasm[`tachiko_designer_${arena}_reserve`](value.length);
    new Uint8Array(wasm.memory.buffer, pointer, value.length).set(value);
  }
  put("request", request);
  put("project", bytes);
  const before = wasm.memory.buffer.byteLength;
  // Input setup itself must never pre-grow the old 256 MiB reservation.
  assert.ok(before < 16 * 1024 * 1024, `input setup grew to ${before} bytes`);
  wasm.tachiko_designer_spreadsheet_run();
  const after = wasm.memory.buffer.byteLength;
  const reply = JSON.parse(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,
    wasm.tachiko_designer_response_ptr(), wasm.tachiko_designer_response_len())));
  assert.ok(after - before < 16 * 1024 * 1024,
    `unnecessary heap reservation: ${before} -> ${after}`);
  return reply;
}

const invalid = await inspect(new Uint8Array(1200 * 1024));
assert.equal(invalid.status, "error");
assert.equal(invalid.error.code, "invalid_project");
assert.match(invalid.error.message, /ZIP/);
console.log("invalid reserving-size input refuses without heap warmup: PASS");
assert.equal((await inspect(await readFile(imageWorkbookPath))).status, "ok");
console.log("small image-heavy workbook skips heap warmup: PASS");
assert.equal((await inspect(await readFile(malformedWorkbookPath))).status, "error");
console.log("malformed worksheet skips heap warmup: PASS");
