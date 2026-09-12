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

describe("Designer canonical and portable storage bridge", () => {
  it("keeps canonical tree entries distinct from genuine portable .ro bytes and fails closed on rejected verification", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const requests: Array<{ id: number; kind: string; bytes?: ArrayBuffer }> = [];
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => { if (type === "message") receive = callback; },
      postMessage: (request: { id: number; kind: string; bytes?: ArrayBuffer }) => {
        requests.push(request);
        if (request.kind === "export_canonical_tree") {
          receive?.({ data: { id: request.id, status: "canonical_tree_exported", export: { revision: "resident/0", files: [{ path: "manifest.json", bytes: new ArrayBuffer(1) }] } } } as MessageEvent);
        } else if (request.kind === "export_portable_ro") {
          receive?.({ data: { id: request.id, status: "portable_ro_exported", export: { revision: "resident/0", bytes: new Uint8Array([0x50, 0x4b, 3, 4]).buffer } } } as MessageEvent);
        } else if (request.kind === "verify_portable_ro") {
          receive?.({ data: { id: request.id, status: "ok", response: { type: "portable_ro_verified", payload: { accepted: false } } } } as MessageEvent);
        }
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);

    await expect(client.exportCanonicalTree("resident/0")).resolves.toMatchObject({
      revision: "resident/0",
      files: [{ path: "manifest.json" }],
    });
    const portable = await client.exportPortableRo("resident/0");
    expect(portable).toMatchObject({
      revision: "resident/0",
    });
    expect(portable.bytes).toBeInstanceOf(ArrayBuffer);
    await expect(client.verifyPortableRo(new ArrayBuffer(1))).rejects.toThrow("Expected portable .ro verification, received 'ok'.");
    expect(requests.map(request => request.kind)).toEqual(["export_canonical_tree", "export_portable_ro", "verify_portable_ro"]);
  });

  it("uses fresh host identity for portable open and reads the runtime occurrence projection", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    vi.stubGlobal("crypto", { randomUUID: () => "123e4567-e89b-42d3-a456-426614174000" });
    let receive: ((event: MessageEvent) => void) | undefined;
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => { if (type === "message") receive = callback; },
      postMessage: (request: { id: number; kind: string; occurrence_id?: string }) => {
        if (request.kind === "open_portable_ro") {
          expect(request.occurrence_id).toBe("123e4567-e89b-42d3-a456-426614174000");
          receive?.({ data: { id: request.id, status: "ok", response: { type: "opened", payload: { bootstrap: { title: "Opened", revision: "resident/0", default_collection: "items", collections: [] }, table: { revision: "resident/0", collection: { id: "items", key: "items", entity_count: 0 }, columns: [], rows: [] } } } } } as MessageEvent);
        } else if (request.kind === "observe_occurrence") {
          receive?.({ data: { id: request.id, status: "ok", response: { type: "occurrence_observed", payload: { scope: "designer-occurrence/123e4567-e89b-42d3-a456-426614174000/doc", revision: "resident/0" } } } } as MessageEvent);
        }
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    await expect(client.openPortableRo(new ArrayBuffer(1))).resolves.toMatchObject({ bootstrap: { title: "Opened" } });
    await expect(client.observeOccurrence()).resolves.toEqual({
      scope: "designer-occurrence/123e4567-e89b-42d3-a456-426614174000/doc",
      revision: "resident/0",
    });
  });
});
