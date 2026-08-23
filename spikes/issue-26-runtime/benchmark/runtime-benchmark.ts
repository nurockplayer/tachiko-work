import assert from "node:assert/strict";
import { stat } from "node:fs/promises";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

import type {
  RuntimeCommand,
  SemanticDocument,
  WireRequest,
  WireResult,
} from "../worker/protocol.ts";
import { RuntimeClient } from "../worker/runtime-client.ts";

const here = path.dirname(fileURLToPath(import.meta.url));
const spikeRoot = path.resolve(here, "..");
const wasmPath = path.join(
  spikeRoot,
  "target/wasm32-unknown-unknown/release/tachiko_issue_26_runtime_spike.wasm",
);
const entityCounts = [10, 100, 1000];
const mutationIterations = Number.parseInt(
  process.env.TACHIKO_SPIKE_ITERATIONS ?? "20",
  10,
);

function encodedBytes(value: unknown): number {
  return Buffer.byteLength(JSON.stringify(value));
}

function median(values: number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function round(value: number): number {
  return Number(value.toFixed(3));
}

function mutation(index: number): RuntimeCommand {
  return {
    type: "set_scalar",
    address: { entity: "entity_0000", field: "base" },
    input: index % 2 === 0 ? "11" : "12",
  };
}

async function timedRequest(
  client: RuntimeClient,
  request: WireRequest,
): Promise<{ duration: number; result: WireResult }> {
  const started = performance.now();
  const result = await client.wireRequest(request);
  return { duration: performance.now() - started, result };
}

function serializationMedian(document: SemanticDocument): number {
  const samples = [];
  for (let index = 0; index < 20; index += 1) {
    const started = performance.now();
    JSON.stringify(document);
    samples.push(performance.now() - started);
  }
  return median(samples);
}

async function measureScale(entityCount: number) {
  const residentClient = await RuntimeClient.spawn(wasmPath);
  const statelessClient = await RuntimeClient.spawn(wasmPath);
  try {
    const initial = await residentClient.generateSynthetic(entityCount);
    const snapshotBytes = encodedBytes(initial);

    const openRequest: WireRequest = { type: "open", document: initial };
    const opened = await timedRequest(residentClient, openRequest);
    if (opened.result.type !== "opened") {
      throw new Error(`expected opened result, received ${opened.result.type}`);
    }

    const residentDurations: number[] = [];
    let residentRequestBytes = 0;
    let residentResponseBytes = 0;
    for (let index = 0; index < mutationIterations; index += 1) {
      const request: WireRequest = {
        type: "execute",
        command: mutation(index),
      };
      residentRequestBytes += encodedBytes(request);
      const sample = await timedRequest(residentClient, request);
      residentDurations.push(sample.duration);
      residentResponseBytes += encodedBytes(sample.result);
      if (sample.result.type !== "command") {
        throw new Error(`expected command result, received ${sample.result.type}`);
      }
    }
    const residentDocument = await residentClient.snapshot();

    const calculationDurations: number[] = [];
    let calculationResultBytes = 0;
    for (let index = 0; index < 5; index += 1) {
      const sample = await timedRequest(residentClient, {
        type: "execute",
        command: { type: "calculate" },
      });
      calculationDurations.push(sample.duration);
      calculationResultBytes = encodedBytes(sample.result);
    }

    let statelessDocument = initial;
    const statelessDurations: number[] = [];
    let statelessRequestBytes = 0;
    let statelessResponseBytes = 0;
    for (let index = 0; index < mutationIterations; index += 1) {
      const request: WireRequest = {
        type: "execute_snapshot",
        document: statelessDocument,
        command: mutation(index),
      };
      statelessRequestBytes += encodedBytes(request);
      const sample = await timedRequest(statelessClient, request);
      statelessDurations.push(sample.duration);
      statelessResponseBytes += encodedBytes(sample.result);
      if (sample.result.type !== "snapshot_execution") {
        throw new Error(
          `expected snapshot execution, received ${sample.result.type}`,
        );
      }
      statelessDocument = sample.result.document;
    }
    assert.deepEqual(residentDocument, statelessDocument);

    const statelessTotal = statelessDurations.reduce(
      (sum, value) => sum + value,
      0,
    );
    const residentTotal = residentDurations.reduce(
      (sum, value) => sum + value,
      0,
    );
    return {
      entities: entityCount,
      formulas: entityCount,
      numeric_fields: entityCount * 3,
      snapshot_bytes: snapshotBytes,
      main_thread_json_stringify_median_ms: round(serializationMedian(initial)),
      resident_initial_open_ms: round(opened.duration),
      resident_initial_open_request_json_bytes_estimate: encodedBytes(openRequest),
      repeated_mutations: {
        iterations: mutationIterations,
        whole_snapshot: {
          total_ms: round(statelessTotal),
          median_ms: round(median(statelessDurations)),
          request_json_bytes_estimate_total: statelessRequestBytes,
          response_json_bytes_estimate_total: statelessResponseBytes,
        },
        resident_commands: {
          total_ms: round(residentTotal),
          median_ms: round(median(residentDurations)),
          request_json_bytes_estimate_total: residentRequestBytes,
          response_json_bytes_estimate_total: residentResponseBytes,
        },
        elapsed_speedup: round(statelessTotal / residentTotal),
        request_json_size_estimate_reduction: round(
          statelessRequestBytes / residentRequestBytes,
        ),
        response_json_size_estimate_reduction: round(
          statelessResponseBytes / residentResponseBytes,
        ),
      },
      full_calculation_query: {
        median_ms: round(median(calculationDurations)),
        result_json_bytes_estimate: calculationResultBytes,
      },
    };
  } finally {
    await Promise.all([residentClient.close(), statelessClient.close()]);
  }
}

const measurements = [];
for (const entityCount of entityCounts) {
  measurements.push(await measureScale(entityCount));
}
const wasm = await stat(wasmPath);
process.stdout.write(
  `${JSON.stringify(
    {
      environment: {
        node: process.version,
        platform: `${process.platform}-${process.arch}`,
        wasm_release_bytes: wasm.size,
        mutation_iterations: mutationIterations,
      },
      measurements,
    },
    null,
    2,
  )}\n`,
);
