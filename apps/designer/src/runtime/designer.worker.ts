/// <reference lib="webworker" />

import type { FailureProjection, WorkerRequest } from "./protocol.ts";
import { createDesignerWasmBridge } from "./wasm-bridge.ts";

const scope = self as DedicatedWorkerGlobalScope;
const bridge = createDesignerWasmBridge("/designer_runtime.wasm");

scope.addEventListener("message", (event: MessageEvent<WorkerRequest>) => {
  void (async () => {
    try {
      const runtime = await bridge;
      const reply = runtime.request(event.data.request);
      scope.postMessage({ id: event.data.id, ...reply });
    } catch (error) {
      const failure: FailureProjection = {
        code: "worker_failure",
        message: error instanceof Error ? error.message : String(error),
        current_revision: "unavailable",
        diagnostics: [],
      };
      scope.postMessage({ id: event.data.id, status: "error", error: failure });
    }
  })();
});
