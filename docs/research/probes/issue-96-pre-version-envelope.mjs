// Execute the wasm32-unknown-unknown build of the Issue #96 research probe.

import { readFile } from "node:fs/promises";

const wasmPath = process.argv[2];
if (!wasmPath) {
  throw new Error("usage: node issue-96-pre-version-envelope.mjs PROBE.wasm");
}

const bytes = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes);
const count = Number(instance.exports.issue96_case_count());

for (let index = 0; index < count; index += 1) {
  const resultClass = Number(instance.exports.issue96_case_class(index));
  const inputBytes = Number(instance.exports.issue96_case_bytes(index));
  console.log(`${index}|${resultClass}|${inputBytes}`);
}
