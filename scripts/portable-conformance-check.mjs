import { readFile } from "node:fs/promises";

const [modulePath] = process.argv.slice(2);
if (!modulePath) {
  throw new Error("usage: node portable-conformance-check.mjs <module.wasm>");
}

const bytes = await readFile(modulePath);
const { instance } = await WebAssembly.instantiate(bytes, {});
const {
  tachiko_case_count: caseCount,
  tachiko_case_class: caseClass,
  tachiko_case_bits: caseBits,
  tachiko_case_auxiliary: caseAuxiliary,
} = instance.exports;

for (let index = 0; index < caseCount(); index += 1) {
  const bits = BigInt.asUintN(64, caseBits(index))
    .toString(16)
    .padStart(16, "0");
  const auxiliary = BigInt.asUintN(64, caseAuxiliary(index));
  console.log(`${index}|${caseClass(index)}|${bits}|${auxiliary}`);
}
