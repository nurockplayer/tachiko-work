#!/usr/bin/env node

import {createHash} from "node:crypto";
import {readFile} from "node:fs/promises";
import {resolve} from "node:path";

function usage() {
  console.error(
    "usage: node validate-tw09-stable-facts.mjs --contract /abs/contract.json " +
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
const contract = JSON.parse(contractBytes.toString("utf8"));
const observations = JSON.parse(await readFile(resolve(args.get("observations")), "utf8"));
const adapterBytes = await readFile(resolve(args.get("adapter-file")));
const expectedContractSha256 =
  "38a0a4922c7c9d43c50d0690f5ccca7751ab2a70859a542f6beae99540ab8f98";
if (sha256(contractBytes) !== expectedContractSha256) fail("TW-09 contract hash mismatch");

exactKeys(observations, ["contract_sha256", "adapter", "observations"], "root");
if (observations.contract_sha256 !== expectedContractSha256) {
  fail("observation contract_sha256 mismatch");
}
exactKeys(observations.adapter, ["sha256", "behavior_implemented_by_adapter"], "adapter");
if (observations.adapter.sha256 !== sha256(adapterBytes)) fail("adapter SHA-256 mismatch");
if (observations.adapter.behavior_implemented_by_adapter !== false) {
  fail("adapter may normalize names/types only; it may not implement behavior");
}

const equal = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const actual = observations.observations ?? {};
const expected = contract.expected_observations;
const assertions = {
  machine_code: actual.machine_fact?.code === expected.machine_fact.code,
  machine_classification_severity_provider:
    actual.machine_fact?.classification === expected.machine_fact.classification &&
    actual.machine_fact?.severity === expected.machine_fact.severity &&
    actual.machine_fact?.provider === expected.machine_fact.provider,
  machine_subjects: equal(actual.machine_fact?.subjects, expected.machine_fact.subjects),
  machine_related_subjects: equal(
    actual.machine_fact?.related_subjects,
    expected.machine_fact.related_subjects,
  ),
  machine_facts: equal(actual.machine_fact?.facts, expected.machine_fact.facts),
  stable_machine_code_order: equal(
    actual.stable_order?.machine_codes,
    expected.stable_order.machine_codes,
  ),
  presentation_invariance: equal(
    actual.presentation_invariance,
    expected.presentation_invariance,
  ),
  renamed_duplicate: equal(actual.renamed_duplicate, expected.renamed_duplicate),
};

console.log(
  JSON.stringify({
    contract_id: "TW-09-stable-diagnostic-facts-v1",
    contract_sha256: expectedContractSha256,
    adapter_sha256: observations.adapter.sha256,
    assertions,
  }),
);
