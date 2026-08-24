#!/usr/bin/env node

import {createHash} from "node:crypto";
import {readFile, realpath, writeFile} from "node:fs/promises";
import {spawn} from "node:child_process";
import readline from "node:readline";
import {dirname, resolve} from "node:path";
import {pathToFileURL, fileURLToPath} from "node:url";

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || values.has(key)) {
      throw new Error("invalid TW-05 adapter arguments");
    }
    values.set(key, value);
  }
  for (const key of ["--candidate-root", "--contract", "--output"]) {
    if (!values.has(key)) throw new Error(`missing ${key}`);
  }
  return values;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

class NativeClient {
  #child;
  #lines;

  constructor(executable) {
    this.#child = spawn(executable, [], {stdio: ["pipe", "pipe", "inherit"]});
    this.#lines = readline.createInterface({input: this.#child.stdout})[Symbol.asyncIterator]();
  }

  async wireRequest(request) {
    this.#child.stdin.write(`${JSON.stringify(request)}\n`);
    const line = await this.#lines.next();
    if (line.done) throw new Error("native driver ended before replying");
    const reply = JSON.parse(line.value);
    if (!reply.ok) throw new Error(reply.error);
    return reply.result;
  }

  async close() {
    this.#child.stdin.end();
    await new Promise((resolveExit, reject) => {
      this.#child.once("exit", (code) => {
        if (code === 0) resolveExit();
        else reject(new Error(`native driver exited with ${code}`));
      });
    });
  }
}

function calculatedValue(response, entity) {
  const record = response.result?.calculated?.find(
    (entry) =>
      entry.field?.entity === entity &&
      entry.field?.field === "synthetic-computed-field-id",
  );
  if (typeof record?.value !== "number") throw new Error("missing calculated projection");
  return record.value;
}

function patchedValue(response, entity, field) {
  const patch = response.result?.patches?.find(
    (entry) => entry.field?.entity === entity && entry.field?.field === field,
  );
  if (patch?.value?.type !== "number") throw new Error("missing numeric mutation patch");
  return patch.value.value;
}

function snapshotNumber(document, entity, field) {
  const value = document.entities?.[entity]?.fields?.[field];
  if (!["number"].includes(value?.type ?? value?.kind)) {
    throw new Error("missing numeric snapshot field");
  }
  return value.value;
}

async function runSequence(client) {
  const firstEntity = "synthetic-entity-000000";
  const secondEntity = "synthetic-entity-000001";
  const baseField = "synthetic-base-field-id";
  const computedField = "synthetic-computed-field-id";
  const opened = await client.wireRequest({type: "open_synthetic", entity_count: 2});
  const overview = await client.wireRequest({
    type: "execute",
    command: {type: "overview"},
  });
  const calculation = await client.wireRequest({
    type: "execute",
    command: {type: "calculate"},
  });
  const firstMutation = await client.wireRequest({
    type: "execute",
    expected_revision: 0,
    command: {
      type: "set_scalar",
      address: {entity: "entity_0000", field: "base"},
      input: "11",
    },
  });

  let staleError;
  let staleResult;
  try {
    staleResult = await client.wireRequest({
      type: "execute",
      expected_revision: 0,
      command: {
        type: "set_scalar",
        address: {entity: "entity_0000", field: "base"},
        input: "12",
      },
    });
  } catch (error) {
    staleError = String(error.message ?? error);
  }
  const afterStaleCalculation = await client.wireRequest({
    type: "execute",
    command: {type: "calculate"},
  });
  const snapshot = await client.wireRequest({type: "snapshot"});
  if (opened.type !== "opened" || overview.type !== "command" || calculation.type !== "command") {
    throw new Error("unexpected historical runtime response shape");
  }
  if (firstMutation.type !== "command" || snapshot.type !== "snapshot") {
    throw new Error("unexpected mutation/snapshot response shape");
  }
  const firstBase = snapshotNumber(snapshot.document, firstEntity, baseField);
  if (afterStaleCalculation.type !== "command") {
    throw new Error("unexpected after-stale calculation response shape");
  }
  const firstProduct = calculatedValue(afterStaleCalculation.response, firstEntity);
  const staleRejected = staleResult === undefined && /stale.*revision/i.test(staleError ?? "");
  const actualRevision = staleResult?.response?.revision ?? firstMutation.response.revision;
  return [
    {step: "open", revision: opened.revision},
    {
      step: "overview",
      entity_count: overview.response.result.entity_count,
      formula_count: overview.response.result.formula_count,
    },
    {
      step: "calculate",
      first_product: calculatedValue(calculation.response, firstEntity),
      second_product: calculatedValue(calculation.response, secondEntity),
    },
    {
      step: "set_first_base",
      revision: firstMutation.response.revision,
      first_product: patchedValue(firstMutation.response, firstEntity, computedField),
    },
    {
      step: "stale_set_first_base",
      typed_stale_revision_error: staleRejected,
      actual_revision: actualRevision,
      state_unchanged: firstBase === 11 && firstProduct === 22,
    },
    {step: "snapshot", revision: actualRevision, first_base: firstBase, first_product: firstProduct},
  ];
}

const args = parseArgs(process.argv.slice(2));
const candidateRoot = await realpath(resolve(args.get("--candidate-root")));
const contractBytes = await readFile(resolve(args.get("--contract")));
const adapterBytes = await readFile(fileURLToPath(import.meta.url));
const spikeRoot = resolve(candidateRoot, "spikes/issue-26-runtime");
const native = new NativeClient(resolve(spikeRoot, "target/release/native-driver"));
const runtimeClientModule = await import(
  pathToFileURL(resolve(spikeRoot, "worker/runtime-client.ts")).href
);
const wasm = await runtimeClientModule.RuntimeClient.spawn(
  resolve(spikeRoot, "target/wasm32-unknown-unknown/release/tachiko_issue_26_runtime_spike.wasm"),
);

let nativeObservations;
let wasmObservations;
try {
  [nativeObservations, wasmObservations] = await Promise.all([
    runSequence(native),
    runSequence(wasm),
  ]);
} finally {
  await Promise.all([native.close(), wasm.close()]);
}

const observations = {
  contract_sha256: sha256(contractBytes),
  adapter: {sha256: sha256(adapterBytes), behavior_implemented_by_adapter: false},
  native: {execution: "native_process", observations: nativeObservations},
  wasm: {
    execution: "real_wasm32",
    worker_boundary: "typescript_worker",
    observations: wasmObservations,
  },
};
await writeFile(resolve(args.get("--output")), `${JSON.stringify(observations)}\n`, {mode: 0o600});
console.log(JSON.stringify({
  adapter: "TW-05-historical-target-v1",
  native_wasm_equal: JSON.stringify(nativeObservations) === JSON.stringify(wasmObservations),
}));
