#!/usr/bin/env node

import {readFile, writeFile} from "node:fs/promises";
import {resolve} from "node:path";

function usage() {
  console.error(
    "usage: node collect-portable-observations.mjs --native /abs/native.out " +
      "--wasm /abs/wasm.out --output /abs/observations.json",
  );
  process.exit(2);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) usage();
    values.set(key.slice(2), value);
  }
  return values;
}

function parseRecords(text, label) {
  const records = text
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const match = line.match(/^(\d+)\|(\d+)\|([0-9a-f]{16})\|(\d+)$/);
      if (!match) throw new Error(`${label} contains a malformed record: ${line}`);
      return {
        index: Number(match[1]),
        class: Number(match[2]),
        bits: match[3],
        auxiliary: match[4],
      };
    });
  if (records.length === 0) throw new Error(`${label} contains no records`);
  if (new Set(records.map((record) => record.index)).size !== records.length) {
    throw new Error(`${label} contains duplicate indexes`);
  }
  return records;
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["native", "wasm", "output"]) {
  if (!args.has(key)) usage();
}

const native = parseRecords(await readFile(resolve(args.get("native")), "utf8"), "native");
const wasm = parseRecords(await readFile(resolve(args.get("wasm")), "utf8"), "wasm");
const observation = {
  contract_id: "tachiko-portable-observations-v1",
  native,
  wasm,
  native_wasm_byte_equal: JSON.stringify(native) === JSON.stringify(wasm),
};
await writeFile(resolve(args.get("output")), `${JSON.stringify(observation, null, 2)}\n`, {
  mode: 0o600,
});
console.log(JSON.stringify(observation));
