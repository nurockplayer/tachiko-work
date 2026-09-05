/// <reference lib="webworker" />

import type { FailureProjection, WorkerRequest } from "./protocol.ts";
import { createDesignerWasmBridge } from "./wasm-bridge.ts";

export function startDesignerWorker(wasmUrl: string): void {
  const scope = self as DedicatedWorkerGlobalScope;
  const bridge = createDesignerWasmBridge(wasmUrl);

  scope.addEventListener("message", (event: MessageEvent<WorkerRequest>) => {
    void (async () => {
      try {
        const runtime = await bridge;
        switch (event.data.kind) {
          case "spreadsheet": {
            const reply = runtime.spreadsheet(event.data.operation, new Uint8Array(event.data.bytes));
            if (reply.status === "spreadsheet_exported") scope.postMessage({id: event.data.id, ...reply}, [reply.export.bytes]);
            else scope.postMessage({id: event.data.id, ...reply});
            break;
          }
          case "command": {
            const reply = runtime.request(event.data.request);
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "inspect_project": {
            const reply = runtime.inspectProject(new Uint8Array(event.data.bytes));
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "open_project": {
            const reply = runtime.openProject(
              new Uint8Array(event.data.bytes),
              event.data.occurrence_id,
            );
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "export_project": {
            const reply = runtime.exportProject(event.data.expected_revision);
            if (reply.status === "error") {
              scope.postMessage({ id: event.data.id, ...reply });
            } else {
              scope.postMessage(
                { id: event.data.id, status: "project_exported", export: reply.export },
                [reply.export.bytes],
              );
            }
            break;
          }
          case "close_project":
            runtime.closeProject();
            scope.postMessage({ id: event.data.id, status: "closed" });
            break;
        }
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
}
