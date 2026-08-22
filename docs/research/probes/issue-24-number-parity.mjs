// Execute the wasm32-unknown-unknown build of issue-24-number-parity.rs.

import { readFile } from "node:fs/promises";

const wasmPath = process.argv[2];
if (!wasmPath) {
  throw new Error("usage: node issue-24-number-parity.mjs PROBE.wasm");
}

const bytes = await readFile(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes);
const count = Number(instance.exports.issue24_case_count());

for (let index = 0; index < count; index += 1) {
  const kind = Number(instance.exports.issue24_case_kind(index));
  const bits = BigInt.asUintN(
    64,
    instance.exports.issue24_case_bits(index),
  )
    .toString(16)
    .padStart(16, "0");
  console.log(`${index}:${kind}:${bits}`);
}
