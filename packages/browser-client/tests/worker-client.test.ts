import { afterEach, describe, expect, it, vi } from "vitest";

import { createExperimentalDesignerClient } from "../src/experimental-client.ts";
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

  it("accepts an import projection-bytes refusal", async () => {
    const {rejected} = await rejectWithFailure({
      ...failureBase, limit: "projection_bytes", maximum: 65_536, attempted: 65_537,
      message: "Workbook is too large to open: it needs 65537 bytes and the limit is 65536. The workbook was not imported.",
    });
    await expect(rejected).rejects.toMatchObject({
      failure: {code: "capacity_exceeded", limit: "projection_bytes", maximum: 65_536, attempted: 65_537},
    });
  });

  it("accepts a working-size saved-bytes refusal", async () => {
    const {rejected} = await rejectWithFailure({
      ...failureBase, limit: "saved_bytes", maximum: 66_060_288, attempted: 66_257_664,
      message: "The edit would make the project too large to save.",
    });
    await expect(rejected).rejects.toMatchObject({
      failure: {code: "capacity_exceeded", limit: "saved_bytes", maximum: 66_060_288, attempted: 66_257_664},
    });
  });

  it("accepts a valid workbook capacity refusal", async () => {
    const {rejected} = await rejectWithFailure({
      ...failureBase, limit: "populated_cells", maximum: 100_000, attempted: 100_001,
    });
    await expect(rejected).rejects.toMatchObject({
      failure: {code: "capacity_exceeded", limit: "populated_cells", maximum: 100_000, attempted: 100_001},
    });
  });

  it.each([
    {format: "xlsx", limit: "collections", attempted: 33, maximum: 32},
    {format: "xlsx", limit: "table_fields", collection: "Sales", attempted: 33, maximum: 32},
    {format: "csv", limit: "table_fields", collection: "Imported table", attempted: 33, maximum: 32},
    {format: "xlsx", limit: "table_rows", collection: "Sales", attempted: 129, maximum: 128},
    {format: "xlsx", limit: "total_entities", attempted: 1025, maximum: 1024},
    {format: "xlsx", limit: "formulas", attempted: 33, maximum: 32},
  ] as const)("accepts $format import refusal for $limit", async ({format, ...capacity}) => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    vi.stubGlobal("crypto", {randomUUID: () => "00000000-0000-4000-8000-000000000638"});
    const failure = {
      ...failureBase, ...capacity,
      message: `Workbook has at least ${capacity.attempted} items; the maximum is ${capacity.maximum}. The workbook was not imported.`,
    };
    let receive: ((event: MessageEvent) => void) | undefined;
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: {id: number; kind: string; operation: {type: string}}) => {
        expect(request.kind).toBe("spreadsheet");
        expect(["inspect", "import"]).toContain(request.operation.type);
        receive?.({data: {id: request.id, status: "error", error: failure}} as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    const options = {delimiter: "," as const, header: true};
    await expect(client.inspectSpreadsheet(new ArrayBuffer(1), format, options)).rejects.toMatchObject({failure});
    await expect(client.importSpreadsheet(new ArrayBuffer(1), format, options, {column_types: [], extra_columns: []})).rejects.toMatchObject({failure});
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
      populated_cells: 100_000, formulas: 256, formula_ast_nodes: 100_000, dependency_edges: 200_000,
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
  it("sends add and delete table requests", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const requests: Record<string, unknown>[] = [];
    const publication = { resulting_revision: "resident/1" };
    const duplicated = { publication, collection: { id: "new", key: "new", entity_count: 0 } };
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => { if (type === "message") receive = callback; },
      postMessage: (request: { id: number; request: Record<string, unknown> }) => {
        requests.push(request.request);
        const response = request.request.type === "add_table" ? { type: "duplicated", payload: duplicated } : { type: "published", payload: publication };
        receive?.({ data: { id: request.id, status: "ok", response } } as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    const columns = [{name: "Name", field_type: "text", required: true}];
    await expect(client.addTable("resident/0", "New", columns)).resolves.toEqual(duplicated);
    await expect(client.deleteTable("resident/1", "new")).resolves.toEqual(publication);
    expect(requests).toEqual([
      {type: "add_table", expected_revision: "resident/0", name: "New", columns},
      {type: "delete_table", expected_revision: "resident/1", collection: "new"},
    ]);
  });

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

describe("A1 translation transport", () => {
  it("round-trips same-row translation and the partial-range refusal", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => { if (type === "message") receive = callback; },
      postMessage: (request: {id: number; kind: string; request: {type: string; text: string; geometry: unknown}}) => {
        expect(request.kind).toBe("command");
        expect(request.request.type).toBe("translate_a1_formula");
        expect(request.request.geometry).toEqual({header_row: 1, rows: ["r1", "r2", "r3"], columns: ["name", "qty", "price", "total"]});
        const reply = request.request.text === "B3*C3"
          ? {status: "ok", response: {type: "a1_translation", payload: {revision: "resident/0", source: "[.qty]*[.price]", notes: []}}}
          : {status: "error", error: {code: "unsupported_range", message: "Unsupported range", current_revision: "resident/0", diagnostics: [], span: {start: 0, end: 10}}};
        receive?.({data: {id: request.id, ...reply}} as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    const input = {expectedRevision: "resident/0", collectionId: "Items", rowEntityId: "r2", targetFieldId: "total",
      geometry: {headerRow: 1, rows: ["r1", "r2", "r3"], columns: ["name", "qty", "price", "total"]}};
    await expect(client.translateA1Formula({...input, text: "B3*C3"})).resolves.toEqual({revision: "resident/0", source: "[.qty]*[.price]", notes: []});
    await expect(client.translateA1Formula({...input, text: "SUM(B2:B3)"})).rejects.toMatchObject({failure: {code: "unsupported_range", span: {start: 0, end: 10}}});
  });
});

describe("working-size Worker transport", () => {
  const openedWindow = {
    profile: "working_size",
    bootstrap: { title: "Working", revision: "resident/0", default_collection: "items", collections: [] },
    window: {
      revision: "resident/0",
      collection: { id: "items", key: "items", entity_count: 0 },
      columns: [], rows: [], total_rows: 0, unfiltered_rows: 0, start: 0,
    },
  };
  const opened = {
    bootstrap: openedWindow.bootstrap,
    table: { revision: "resident/0", collection: { id: "items", key: "items", entity_count: 0 }, columns: [], rows: [] },
  };
  type Sent = { id: number; kind: string; [key: string]: unknown };

  async function transport(response: Record<string, unknown>) {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    vi.stubGlobal("crypto", { randomUUID: () => "123e4567-e89b-42d3-a456-426614174000" });
    let receive: ((event: MessageEvent) => void) | undefined;
    const sent: Sent[] = [];
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: Sent) => {
        sent.push(request);
        receive?.({ data: { id: request.id, ...response } } as MessageEvent);
      },
    } as unknown as Worker;
    return { client: new WorkerDesignerClient(() => worker), sent };
  }

  const windowReply = { status: "ok", response: { type: "opened_window", payload: openedWindow } };
  const openedReply = { status: "ok", response: { type: "opened", payload: opened } };

  it("sends the profile on every working-size route and returns the window", async () => {
    const { client, sent } = await transport(windowReply);
    await expect(client.newTableWorkingSize("Working", [])).resolves.toEqual(openedWindow);
    await expect(client.inspectProjectWorkingSize(new Uint8Array([1]).buffer)).resolves.toEqual(openedWindow);
    await expect(client.openProjectWorkingSize(new Uint8Array([1]).buffer)).resolves.toEqual(openedWindow);
    await expect(client.openCanonicalTreeWorkingSize([
      { path: "manifest.json", bytes: new Uint8Array([1]).buffer },
    ])).resolves.toEqual(openedWindow);
    await expect(client.openLocalDocumentWorkingSize(new Uint8Array([1]).buffer)).resolves.toEqual(openedWindow);
    await expect(client.openPortableRoWorkingSize(new Uint8Array([1]).buffer)).resolves.toEqual(openedWindow);
    expect(sent).toHaveLength(6);
    expect(sent[0]).toMatchObject({ kind: "command", request: { type: "new_table", profile: "working_size" } });
    expect(sent[1]).toMatchObject({ kind: "inspect_project", profile: "working_size" });
    expect(sent[2]).toMatchObject({ kind: "open_project", profile: "working_size" });
    expect(sent[3]).toMatchObject({ kind: "open_project", profile: "working_size" });
    expect(sent[4]).toMatchObject({ kind: "open_local_document", profile: "working_size" });
    expect(sent[5]).toMatchObject({ kind: "open_portable_ro", profile: "working_size" });
  });

  it("leaves every ordinary route without a profile field", async () => {
    const { client, sent } = await transport(openedReply);
    await client.newTable("Ordinary", []);
    await client.inspectProject(new Uint8Array([1]).buffer);
    await client.openProject(new Uint8Array([1]).buffer);
    await client.openCanonicalTree([{ path: "manifest.json", bytes: new Uint8Array([1]).buffer }]);
    await client.openLocalDocument(new Uint8Array([1]).buffer);
    await client.openPortableRo(new Uint8Array([1]).buffer);
    expect(sent).toHaveLength(6);
    expect(sent[0]!.request).not.toHaveProperty("profile");
    for (const request of sent.slice(1)) expect(request).not.toHaveProperty("profile");
  });

  it("refuses a reply of another type", async () => {
    const { client } = await transport(openedReply);
    await expect(client.newTableWorkingSize("Working", [])).rejects.toThrow("Expected 'opened_window' response, received 'opened'.");
  });

  it("sends the profile on the imported working-size spreadsheet routes", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    vi.stubGlobal("crypto", { randomUUID: () => "123e4567-e89b-42d3-a456-426614174000" });
    const importedWindow = { opened: openedWindow, metadata: { version: 1, sheets: [] }, ledger: [] };
    const imported = { opened, metadata: { version: 1, sheets: [] }, ledger: [] };
    let receive: ((event: MessageEvent) => void) | undefined;
    const sent: Sent[] = [];
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => {
        if (type === "message") receive = callback;
      },
      postMessage: (request: Sent) => {
        sent.push(request);
        const profile = (request.operation as {profile?: string}).profile;
        const base = (request.operation as {type: string}).type;
        const response = profile === "working_size"
          ? base === "import"
            ? { type: "imported_window", payload: importedWindow }
            : { type: "opened_window", payload: openedWindow }
          : base === "import"
            ? { type: "imported", payload: imported }
            : { type: "opened", payload: opened };
        receive?.({ data: { id: request.id, status: "ok", response } } as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    const validate = vi.fn();
    const options = { delimiter: "," as const, header: true };
    const selection = { column_types: [], extra_columns: [] };
    const metadata = { version: 1, sheets: [] };

    await expect(client.importSpreadsheetWorkingSize(new Uint8Array([1]).buffer, "csv", options, selection, validate))
      .resolves.toEqual(importedWindow);
    expect(validate).toHaveBeenCalledWith(importedWindow);
    expect(sent[0]).toMatchObject({ kind: "spreadsheet", operation: { type: "import", install: false, profile: "working_size" } });
    expect(sent[1]).toMatchObject({ kind: "spreadsheet", operation: { type: "import", install: true, profile: "working_size" } });

    await expect(client.inspectImportedProjectWorkingSize(new Uint8Array([1]).buffer, metadata)).resolves.toEqual(openedWindow);
    await expect(client.openImportedProjectWorkingSize(new Uint8Array([1]).buffer, metadata)).resolves.toEqual(openedWindow);
    expect(sent[2]).toMatchObject({ kind: "spreadsheet", operation: { type: "inspect_project", metadata, profile: "working_size" } });
    expect(sent[3]).toMatchObject({ kind: "spreadsheet", operation: { type: "open_project", metadata, profile: "working_size" } });

    // An existing method still sends no profile key.
    await client.importSpreadsheet(new Uint8Array([1]).buffer, "csv", options, selection);
    expect(sent[4]!.operation).not.toHaveProperty("profile");
  });

  it("refuses an imported working-size reply of another type", async () => {
    const { client } = await transport(openedReply);
    const options = { delimiter: "," as const, header: true };
    await expect(client.importSpreadsheetWorkingSize(
      new Uint8Array([1]).buffer, "csv", options, { column_types: [], extra_columns: [] },
    )).rejects.toThrow("Expected 'imported_window' response, received 'opened'.");
    await expect(client.inspectImportedProjectWorkingSize(new Uint8Array([1]).buffer, { version: 1, sheets: [] }))
      .rejects.toThrow("Expected 'opened_window' response, received 'opened'.");
  });

  it("refuses a window with broken row presence", async () => {
    const column = { id: "c1", key: "c1", display_name: "c1", field_type: "text", required: false };
    const { client } = await transport({ status: "ok", response: {
      type: "opened_window",
      payload: { ...openedWindow, window: { ...openedWindow.window, columns: [column], rows: [{ id: "r1", fields: [] }] } },
    } });
    await expect(client.openProjectWorkingSize(new Uint8Array([1]).buffer)).rejects.toThrow("Invalid Worker sparse row presence.");
  });
});

describe("Published changed-cell wire validation", () => {
  it("refuses a changed cell with bad presence", async () => {
    let receive: ((event: MessageEvent) => void) | undefined;
    const requests: { id: number }[] = [];
    vi.stubGlobal("Worker", class {
      addEventListener(type: string, callback: (event: MessageEvent) => void) {
        if (type === "message") receive = callback;
      }
      postMessage(request: { id: number }) { requests.push(structuredClone(request)); }
      terminate() {}
    });
    const client = createExperimentalDesignerClient();
    const target = {entity: "row", field: "note"};
    const pending = client.clearField("resident/0", target);
    const publication = {
      base_revision: "resident/0",
      resulting_revision: "resident/1",
      schemas: [],
      schema_fields: [],
      entities: [],
      fields: [target],
      affected_calculations: [],
      changed: [{
        target,
        address: "items.row.note",
        stored: null,
        formula: null,
        calculated: null,
        diagnostics: [],
        editable_scalar: null,
      }],
    };
    receive!(new MessageEvent("message", {
      data: {id: requests.at(-1)!.id, status: "ok", response: {type: "published", payload: publication}},
    }));
    await expect(pending).rejects.toThrow("non-blank field payload");
    client.close();
  });

  it("refuses a duplicated collection's changed cell with bad presence", async () => {
    let receive: ((event: MessageEvent) => void) | undefined;
    const requests: { id: number }[] = [];
    vi.stubGlobal("Worker", class {
      addEventListener(type: string, callback: (event: MessageEvent) => void) {
        if (type === "message") receive = callback;
      }
      postMessage(request: { id: number }) { requests.push(structuredClone(request)); }
      terminate() {}
    });
    const client = createExperimentalDesignerClient();
    const target = {entity: "row", field: "note"};
    const pending = client.duplicateCollection!("resident/0", "items", "Copy");
    const publication = {
      base_revision: "resident/0",
      resulting_revision: "resident/1",
      schemas: [],
      schema_fields: [],
      entities: [],
      fields: [target],
      affected_calculations: [],
      changed: [{
        target,
        address: "items.row.note",
        stored: null,
        formula: null,
        calculated: null,
        diagnostics: [],
        editable_scalar: null,
      }],
    };
    const collection = {id: "copy", key: "copy", display_name: "Copy"};
    receive!(new MessageEvent("message", {
      data: {id: requests.at(-1)!.id, status: "ok", response: {type: "duplicated", payload: {publication, collection}}},
    }));
    await expect(pending).rejects.toThrow("non-blank field payload");
    client.close();
  });
});


describe("table rename transport", () => {
  it("sends renameTable as a tracker command", async () => {
    const { WorkerDesignerClient } = await import("../src/runtime/worker-client.ts");
    let receive: ((event: MessageEvent) => void) | undefined;
    const publication = { base_revision: "resident/1", resulting_revision: "resident/2", schemas: [], schema_fields: [], entities: [], fields: [], affected_calculations: [] };
    const worker = {
      addEventListener: (type: string, callback: (event: MessageEvent) => void) => { if (type === "message") receive = callback; },
      postMessage: (request: {id: number; kind: string; request: Record<string, unknown>}) => {
        expect(request.kind).toBe("command");
        expect(request.request).toEqual({type: "rename_table", expected_revision: "resident/1", collection: "items", name: "New name"});
        receive?.({data: {id: request.id, status: "ok", response: {type: "published", payload: publication}}} as MessageEvent);
      },
    } as unknown as Worker;
    const client = new WorkerDesignerClient(() => worker);
    await expect(client.renameTable("resident/1", "items", "New name")).resolves.toEqual(publication);
  });
});
