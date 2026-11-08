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
            const bytes = new Uint8Array(event.data.bytes);
            const reply = event.data.profile === "working_size"
              ? runtime.inspectProjectWorkingSize(bytes)
              : runtime.inspectProject(bytes);
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "open_project": {
            const bytes = new Uint8Array(event.data.bytes);
            const reply = event.data.profile === "working_size"
              ? runtime.openProjectWorkingSize(bytes, event.data.occurrence_id)
              : runtime.openProject(bytes, event.data.occurrence_id);
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "open_local_document": {
            const bytes = new Uint8Array(event.data.bytes);
            const reply = event.data.profile === "working_size"
              ? runtime.openLocalDocumentWorkingSize(bytes, event.data.occurrence_id)
              : runtime.openLocalDocument(bytes, event.data.occurrence_id);
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "export_project":
          case "export_project_v3": {
            const reply = event.data.kind === "export_project_v3"
              ? runtime.exportProjectV3(event.data.expected_revision)
              : runtime.exportProject(event.data.expected_revision);
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
          case "export_canonical_tree": {
            const reply = runtime.exportCanonicalTree(event.data.expected_revision);
            if (reply.status === "error") {
              scope.postMessage({ id: event.data.id, ...reply });
            } else {
              scope.postMessage(
                { id: event.data.id, status: "canonical_tree_exported", export: reply.export },
                reply.export.files.map(file => file.bytes),
              );
            }
            break;
          }
          case "export_portable_ro": {
            const reply = runtime.exportPortableRo(event.data.expected_revision);
            if (reply.status === "error") {
              scope.postMessage({ id: event.data.id, ...reply });
            } else {
              scope.postMessage(
                { id: event.data.id, status: "portable_ro_exported", export: reply.export },
                [reply.export.bytes],
              );
            }
            break;
          }
          case "verify_portable_ro": {
            const reply = runtime.verifyPortableRo(new Uint8Array(event.data.bytes));
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "open_portable_ro": {
            const bytes = new Uint8Array(event.data.bytes);
            const reply = event.data.profile === "working_size"
              ? runtime.openPortableRoWorkingSize(bytes, event.data.occurrence_id)
              : runtime.openPortableRo(bytes, event.data.occurrence_id);
            scope.postMessage({ id: event.data.id, ...reply });
            break;
          }
          case "observe_occurrence": {
            const reply = runtime.observeOccurrence();
            scope.postMessage({ id: event.data.id, ...reply });
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
