import { describe, expect, it } from "vitest";

import { DesignerRuntimeError } from "../src/runtime/client.ts";
import { WorkerDesignerClient } from "../src/runtime/worker-client.ts";
import type {
  NativeBudgetExportPresentation,
  SpreadsheetExport,
} from "../src/runtime/interop-protocol.ts";
import type {
  FailureProjection,
  WorkerReply,
  WorkerRequest,
} from "../src/runtime/protocol.ts";

const presentation: NativeBudgetExportPresentation = {
  version: 1,
  active_view: "summary-view",
  views: [
    { id: "items-view", name: "Budget Items", collection_id: "budget_items" },
    { id: "summary-view", name: "Budget Summary", collection_id: "budget_summary" },
  ],
  collections: [
    {
      collection_id: "budget_items",
      rows: [{ entity_id: "rent", styles: [] }],
    },
    {
      collection_id: "budget_summary",
      rows: [{ entity_id: "monthly", styles: [] }],
    },
  ],
};

function transport() {
  let onMessage: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  const requests: WorkerRequest[] = [];
  const worker = {
    addEventListener(type: string, listener: unknown) {
      if (type === "message") onMessage = listener as typeof onMessage;
    },
    postMessage(request: WorkerRequest) {
      requests.push(request);
    },
    terminate() {},
  } as unknown as Worker;
  return {
    client: new WorkerDesignerClient(() => worker),
    request(): WorkerRequest {
      const request = requests[0];
      if (request === undefined) throw new Error("Missing request");
      return request;
    },
    reply(reply: WorkerReply) {
      if (onMessage === undefined) throw new Error("Missing message listener");
      onMessage({ data: reply } as MessageEvent<WorkerReply>);
    },
  };
}

describe("native Budget export private protocol", () => {
  it("routes the exact revision, presentation, format, bytes and ledger through the Worker client", async () => {
    const worker = transport();
    const exported: SpreadsheetExport = {
      revision: "resident/7",
      bytes: new Uint8Array([0x50, 0x4b, 0x03, 0x04]).buffer,
      ledger: [
        {
          category: "lossy_on_export",
          code: "native_budget_session_presentation",
          location: "Budget",
          message: "Charts are not exported.",
          blocking: false,
        },
      ],
    };
    const pending = worker.client.exportNativeBudgetSpreadsheet(
      "resident/7",
      presentation,
      "xlsx",
    );
    const request = worker.request();
    expect(request).toMatchObject({
      kind: "spreadsheet",
      operation: {
        type: "export_native_budget",
        expected_revision: "resident/7",
        presentation,
        format: "xlsx",
      },
    });
    if (request.kind !== "spreadsheet") throw new Error("Expected spreadsheet request");
    expect(request.bytes.byteLength).toBe(0);

    worker.reply({ id: request.id, status: "spreadsheet_exported", export: exported });
    await expect(pending).resolves.toEqual(exported);
  });

  it("preserves stale/refusal details as a structured runtime error", async () => {
    const worker = transport();
    const pending = worker.client.exportNativeBudgetSpreadsheet(
      "resident/6",
      presentation,
      "csv",
    );
    const request = worker.request();
    const failure: FailureProjection = {
      code: "stale_revision",
      message: "The requested revision is stale.",
      current_revision: "resident/7",
      diagnostics: [],
    };
    worker.reply({ id: request.id, status: "error", error: failure });
    await expect(pending).rejects.toBeInstanceOf(DesignerRuntimeError);
    await expect(pending).rejects.toMatchObject({ failure });
  });
});
