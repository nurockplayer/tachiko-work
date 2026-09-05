import { afterEach, describe, expect, it, vi } from "vitest";

import { freshOccurrenceId } from "../src/runtime/worker-client.ts";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Designer occurrence identity", () => {
  it("uses the platform UUID generator when available", () => {
    vi.stubGlobal("crypto", {
      randomUUID: () => "123e4567-e89b-42d3-a456-426614174000",
    });

    expect(freshOccurrenceId()).toBe("123e4567-e89b-42d3-a456-426614174000");
  });

  it("builds a lowercase canonical UUID v4 from cryptographic bytes", () => {
    vi.stubGlobal("crypto", {
      getRandomValues: (bytes: Uint8Array) => {
        bytes.set(Array.from({ length: 16 }, (_, index) => index));
        return bytes;
      },
    });

    expect(freshOccurrenceId()).toBe("00010203-0405-4607-8809-0a0b0c0d0e0f");
  });
});

describe("Designer project inspection", () => {
  it("uses the read-only worker operation and retains bytes for the later open", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const opened = {
      bootstrap: { title: "Candidate", revision: "resident/0", default_collection: "items", collections: [] },
      table: { revision: "resident/0", collection: { id: "items", key: "items", entity_count: 0 }, columns: [], rows: [] },
    };
    const postMessage = vi.fn((request: { id: number; kind: string; bytes: ArrayBuffer }, transfer: Transferable[]) => {
      expect(request.kind).toBe("inspect_project");
      expect(transfer).toEqual([request.bytes]);
      const transferred = structuredClone(request, { transfer });
      expect(new Uint8Array(transferred.bytes)).toEqual(new Uint8Array([1, 2, 3]));
      receive?.({ data: { id: request.id, status: "ok", response: { type: "opened", payload: opened } } } as MessageEvent);
    });
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => { if (type === "message") receive = callback; },
      postMessage,
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    await expect(client.inspectProject(bytes)).resolves.toEqual(opened);
    expect(new Uint8Array(bytes)).toEqual(new Uint8Array([1, 2, 3]));
    expect(postMessage).toHaveBeenCalledOnce();
  });
});
