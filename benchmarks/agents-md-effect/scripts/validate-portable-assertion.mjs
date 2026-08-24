#!/usr/bin/env node

import {readFile} from "node:fs/promises";
import {resolve} from "node:path";

function usage() {
  console.error(
    "usage: node validate-portable-assertion.mjs --oracle-lock /abs/oracle-lock.json " +
      "--case TW-03 --assertion tw-03.portable.records-10-20 " +
      "--observations /abs/portable-observations.json",
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

const args = parseArgs(process.argv.slice(2));
for (const key of ["oracle-lock", "case", "assertion", "observations"]) {
  if (!args.has(key)) usage();
}

const lock = JSON.parse(await readFile(resolve(args.get("oracle-lock")), "utf8"));
const caseEntry = lock.cases.find((entry) => entry.id === args.get("case"));
const assertion = caseEntry?.assertions.find((entry) => entry.id === args.get("assertion"));
if (!assertion || assertion.selector?.kind !== "portable_record_set") {
  throw new Error("locked portable assertion not found");
}
const observations = JSON.parse(
  await readFile(resolve(args.get("observations")), "utf8"),
);
if (
  observations.contract_id !== "tachiko-portable-observations-v1" ||
  !Array.isArray(observations.native) ||
  !Array.isArray(observations.wasm)
) {
  throw new Error("portable observation artifact has an invalid structure");
}

const indexes = new Set(assertion.selector.indexes);
const selected = (records) => records.filter((record) => indexes.has(record.index));
const native = selected(observations.native);
const wasm = selected(observations.wasm);
const expected = assertion.selector.expected_records;
const reasons = [];
if (JSON.stringify(native) !== JSON.stringify(expected)) {
  reasons.push("native selected records differ from the lock");
}
if (JSON.stringify(wasm) !== JSON.stringify(expected)) {
  reasons.push("WASM selected records differ from the lock");
}
if (JSON.stringify(native) !== JSON.stringify(wasm)) {
  reasons.push("selected native/WASM records differ");
}
if (
  [...native, ...wasm].some((record) => record.class === assertion.selector.reject_class)
) {
  reasons.push(`selected record uses rejected class ${assertion.selector.reject_class}`);
}

console.log(
  JSON.stringify({
    protocol_id: lock.protocol_id,
    case_id: caseEntry.id,
    assertion_id: assertion.id,
    pass: reasons.length === 0,
    reasons,
    selected_native: native,
    selected_wasm: wasm,
  }),
);
