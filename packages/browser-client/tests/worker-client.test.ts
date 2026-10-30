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

describe("capacity refusal wire validation", () => {
  const failureBase = {
    code: "capacity_exceeded",
    message: "Capacity limit exceeded.",
    current_revision: "resident/4",
    diagnostics: [],
  };

  async function rejectWithFailure(failure: unknown): Promise<{rejected: Promise<unknown>}> {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: {id: number}) => {
        receive?.({data: {id: request.id, status: "error", error: failure}} as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    return {rejected: client.bootstrap()};
  }

  it("accepts a valid workbook capacity refusal", async () => {
    const {rejected} = await rejectWithFailure({
      ...failureBase, limit: "populated_cells", maximum: 100_000, attempted: 100_001,
    });
    await expect(rejected).rejects.toMatchObject({
      failure: {code: "capacity_exceeded", limit: "populated_cells", maximum: 100_000, attempted: 100_001},
    });
  });

  it.each([
    ["unknown limit", {...failureBase, limit: "wat", maximum: 1, attempted: 2}],
    ["negative maximum", {...failureBase, limit: "collections", maximum: -1, attempted: 0}],
    ["fractional maximum", {...failureBase, limit: "collections", maximum: 1.5, attempted: 2}],
    ["attempt at the limit", {...failureBase, limit: "collections", maximum: 1, attempted: 1}],
    ["nested capacity fields", {...failureBase, capacity: {limit: "collections", maximum: 1, attempted: 2}}],
    ["table limit without collection", {...failureBase, limit: "table_rows", maximum: 1, attempted: 2}],
    ["workbook limit with collection", {...failureBase, limit: "populated_cells", maximum: 1, attempted: 2, collection: "items"}],
  ])("rejects malformed capacity refusal: %s", async (_label, failure) => {
    const {rejected} = await rejectWithFailure(failure);
    await expect(rejected).rejects.toThrow("Invalid Worker capacity failure.");
  });

  it("accepts a per-table refusal with its collection", async () => {
    const {rejected} = await rejectWithFailure({
      ...failureBase, limit: "table_rows", maximum: 10, attempted: 11, collection: "items",
    });
    await expect(rejected).rejects.toMatchObject({
      failure: {code: "capacity_exceeded", limit: "table_rows", collection: "items"},
    });
  });
});

describe("capacity query wire validation", () => {
  const capacity = {
    revision: "resident/3",
    usage: {
      collections: 1, total_entities: 2, populated_cells: 3, formulas: 0,
      formula_ast_nodes: 0, dependency_edges: 0,
      tables: {items: {rows: 2, fields: 3}},
    },
    limits: {
      collections: 32, table_fields: 32, table_rows: 128, total_entities: 1024,
      populated_cells: 100_000, formulas: 32, formula_ast_nodes: 100_000, dependency_edges: 200_000,
    },
  };

  async function requestCapacity(payload: unknown, expectedRevision: string | null = "resident/3"): Promise<{result: Promise<unknown>; request: unknown}> {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    let capturedRequest: unknown;
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: unknown) => {
        capturedRequest = request;
        const message = request as {id: number};
        receive?.({data: {id: message.id, status: "ok", response: {type: "capacity", payload}}} as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    return {result: expectedRevision === null ? client.queryCapacity() : client.queryCapacity(expectedRevision), get request() { return capturedRequest; }};
  }

  it("sends the optional revision and accepts valid usage and limits", async () => {
    const {result, request} = await requestCapacity(capacity);
    await expect(result).resolves.toEqual(capacity);
    expect(request).toMatchObject({kind: "command", request: {type: "query_capacity", expected_revision: "resident/3"}});
  });

  it("omits an unspecified revision", async () => {
    const {result, request} = await requestCapacity(capacity, null);
    await expect(result).resolves.toEqual(capacity);
    expect(request).toMatchObject({kind: "command", request: {type: "query_capacity"}});
    expect((request as {request: object}).request).not.toHaveProperty("expected_revision");
  });

  it.each([
    ["missing revision", {...capacity, revision: undefined}],
    ["missing limit", {...capacity, limits: {...capacity.limits, table_rows: undefined}}],
    ["fractional usage", {...capacity, usage: {...capacity.usage, formulas: 0.5}}],
    ["invalid table count", {...capacity, usage: {...capacity.usage, tables: {items: {rows: -1, fields: 3}}}}],
  ])("rejects malformed capacity projection: %s", async (_label, payload) => {
    const {result} = await requestCapacity(payload);
    await expect(result).rejects.toThrow("Invalid Worker capacity projection.");
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


describe("selected v3 Worker export", () => {
  it("sends the exact revision through the selected opaque export request", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: { id: number; kind: string; expected_revision: string }) => {
        expect(request.kind).toBe("export_project_v3");
        expect(request.expected_revision).toBe("resident/17");
        receive?.({ data: { id: request.id, status: "project_exported",
          export: { revision: "resident/17", bytes } } } as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    expect(await client.exportProjectV3("resident/17")).toEqual({ revision: "resident/17", bytes });
  });
});

describe("duplicate collection column names", () => {
  it("sends the copy's column names with the duplicate request", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const duplicated = { publication: { revision: "resident/2" }, collection: { id: "table_1", key: "table_1", entity_count: 0 } };
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: { id: number; kind: string; request: Record<string, unknown> }) => {
        expect(request.kind).toBe("command");
        expect(request.request).toEqual({
          type: "duplicate_collection",
          expected_revision: "resident/1",
          collection: "items",
          name: "Copy",
          column_names: { column_1: "Amount" },
        });
        receive?.({ data: { id: request.id, status: "ok", response: { type: "duplicated", payload: duplicated } } } as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    await expect(client.duplicateCollection("resident/1", "items", "Copy", { column_1: "Amount" })).resolves.toEqual(duplicated);
  });
});
