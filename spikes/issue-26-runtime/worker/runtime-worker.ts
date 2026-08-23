import { readFile } from "node:fs/promises";
import { parentPort, workerData } from "node:worker_threads";

import type { WorkerReply, WorkerRequest } from "./protocol.ts";
import { WasmRuntime } from "./wasm-runtime.ts";

if (parentPort === null) {
  throw new Error("runtime-worker must run inside a Worker");
}

const runtime = await WasmRuntime.instantiate(await readFile(workerData.wasmPath));

parentPort.on("message", (message: WorkerRequest) => {
  let reply: WorkerReply;
  try {
    reply = {
      id: message.id,
      ok: true,
      result: runtime.request(message.request),
    };
  } catch (error) {
    reply = {
      id: message.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
  parentPort.postMessage(reply);
});

