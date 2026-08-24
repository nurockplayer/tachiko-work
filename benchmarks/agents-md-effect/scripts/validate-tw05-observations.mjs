#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

function usage() {
  console.error(
    "usage: node validate-tw05-observations.mjs --contract /abs/contract.json " +
      "--observations /abs/observations.json --adapter-file /abs/adapter",
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

function fail(message) {
  throw new Error(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function exactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} keys mismatch: expected ${wanted}, got ${actual}`);
  }
}

const args = parseArgs(process.argv.slice(2));
for (const key of ["contract", "observations", "adapter-file"]) {
  if (!args.has(key)) usage();
}

const contractBytes = await readFile(resolve(args.get("contract")));
const observations = JSON.parse(await readFile(resolve(args.get("observations")), "utf8"));
const adapterBytes = await readFile(resolve(args.get("adapter-file")));
const expectedContractSha256 =
  "053c95696dbc1e017fa7849c04c7a67bc87a1bfe3a5e705202cdc1a3cce663d8";
if (sha256(contractBytes) !== expectedContractSha256) fail("TW-05 contract hash mismatch");

const expected = [
  {step: "open", revision: 0},
  {step: "overview", entity_count: 2, formula_count: 2},
  {step: "calculate", first_product: 2, second_product: 4},
  {step: "set_first_base", revision: 1, first_product: 22},
  {
    step: "stale_set_first_base",
    typed_stale_revision_error: true,
    actual_revision: 1,
    state_unchanged: true,
  },
  {step: "snapshot", revision: 1, first_base: 11, first_product: 22},
];

exactKeys(observations, ["contract_sha256", "adapter", "native", "wasm"], "root");
if (observations.contract_sha256 !== expectedContractSha256) {
  fail("observation contract_sha256 mismatch");
}
exactKeys(observations.adapter, ["sha256", "behavior_implemented_by_adapter"], "adapter");
if (observations.adapter.sha256 !== sha256(adapterBytes)) fail("adapter SHA-256 mismatch");
if (observations.adapter.behavior_implemented_by_adapter !== false) {
  fail("adapter may translate names/types only; it may not implement behavior");
}
exactKeys(observations.native, ["execution", "observations"], "native");
exactKeys(observations.wasm, ["execution", "worker_boundary", "observations"], "wasm");
const equal = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const assertions = {};
for (let index = 0; index < expected.length; index += 1) {
  assertions[`native_step_${index}`] = equal(
    observations.native.observations?.[index],
    expected[index],
  );
}
assertions.native_execution = observations.native.execution === "native_process";
assertions.wasm_execution = observations.wasm.execution === "real_wasm32";
assertions.worker_boundary = observations.wasm.worker_boundary === "typescript_worker";
assertions.native_wasm_observations_equal = equal(
  observations.native.observations,
  observations.wasm.observations,
);

console.log(
  JSON.stringify({
    contract_id: "TW-05-resident-parity-v1",
    contract_sha256: expectedContractSha256,
    adapter_sha256: observations.adapter.sha256,
    assertions,
  }),
);
