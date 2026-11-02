import { describe, expect, it, vi } from "vitest";

import {
  createExperimentalDesignerClient,
  openCanonicalTreeFromEntries,
  type ExperimentalDesignerClient,
  type FailureCode,
} from "../src/experimental-client.ts";
import { DesignerRuntimeError } from "../src/runtime/client.ts";
import type { PublicationProjection, TrackerCommand, WorkerReply, WorkerRequest } from "../src/runtime/protocol.ts";

const canonicalPaths = [
  "manifest.json",
  "schemas.json",
  ..."0123456789abcdef".split("").map((digit) => `entities/${digit}.jsonl`),
];

function entries(): { path: string; bytes: ArrayBuffer }[] {
  return canonicalPaths.map((path) => ({path, bytes: new ArrayBuffer(0)}));
}

function opened(revision: string) {
  return {
    bootstrap: {
      title: "fixture",
      revision,
      default_collection: "entities",
      collections: [],
      keyed_grouped_sum_definition_ids: [],
    },
    table: {revision, collection: {id: "entities", key: "entities", entity_count: 0}, columns: [], rows: []},
  };
}

describe("public canonical-entry preflight", () => {
  it("dispatches one public open for a complete canonical set", async () => {
    let resident = "resident/0";
    const openProject = vi.fn(async () => {
      resident = "resident/1";
      return opened(resident);
    });
    const client = {openProject} as unknown as Pick<ExperimentalDesignerClient, "openProject">;

    const result = await openCanonicalTreeFromEntries(client, entries());

    expect(openProject).toHaveBeenCalledTimes(1);
    expect(result.bootstrap.revision).toBe("resident/1");
    expect(resident).toBe("resident/1");
  });

  it.each([
    ["duplicate", () => { const value = entries(); value[17] = value[0]!; return value; }],
    ["unsafe path", () => { const value = entries(); value[0] = {path: "../manifest.json", bytes: new ArrayBuffer(0)}; return value; }],
    ["over file count", () => [...entries(), {path: "definitions.json", bytes: new ArrayBuffer(0)}, {path: "entities/extra.jsonl", bytes: new ArrayBuffer(0)}]],
    ["over transfer size", () => { const value = entries(); value[17] = {path: "entities/f.jsonl", bytes: new ArrayBuffer(64 * 1024 * 1024)}; return value; }],
  ])("rejects %s before dispatch and leaves the resident occurrence unchanged", async (_name, makeEntries) => {
    let resident = "resident/0";
    const openProject = vi.fn(async () => {
      resident = "unexpected";
      return opened(resident);
    });
    const client = {openProject} as unknown as Pick<ExperimentalDesignerClient, "openProject">;

    await expect(openCanonicalTreeFromEntries(client, makeEntries())).rejects.toThrow();

    expect(openProject).not.toHaveBeenCalled();
    expect(resident).toBe("resident/0");
  });
});


it("exposes explicitly selected v3 export on the experimental entry", () => {
  vi.stubGlobal("Worker", class {
    addEventListener() {}
    postMessage() {}
    terminate() {}
  });
  try {
    const client = createExperimentalDesignerClient();
    expect(client.exportProjectV3).toBeTypeOf("function");
    client.close();
  } finally {
    vi.unstubAllGlobals();
  }
});

it("sends edit_cells through the public client and preserves publication and refusal replies", async () => {
  let receive: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  const requests: WorkerRequest[] = [];
  vi.stubGlobal("Worker", class {
    addEventListener(type: string, callback: (event: MessageEvent<WorkerReply>) => void) {
      if (type === "message") receive = callback;
    }
    postMessage(request: WorkerRequest) { requests.push(structuredClone(request)); }
    terminate() {}
  });
  const client = createExperimentalDesignerClient();
  try {
    const command = {
      type: "edit_cells",
      expected_revision: "resident/7",
      edits: [
        {target: {entity: "row-1", field: "name"}, input: {kind: "text", value: "updated"}},
        {target: {entity: "row-2", field: "amount"}, input: {kind: "number", input: "42"}},
      ],
    } satisfies TrackerCommand;
    const publication: PublicationProjection = {
      base_revision: "resident/7", resulting_revision: "resident/8",
      schemas: [], schema_fields: [], entities: ["row-1", "row-2"],
      fields: command.edits.map(edit => edit.target), affected_calculations: [],
    };
    const accepted = client.trackerCommand!(command);
    expect(requests).toEqual([{id: 1, kind: "command", request: command}]);
    receive!(new MessageEvent<WorkerReply>("message", {
      data: {id: 1, status: "ok", response: {type: "published", payload: publication}},
    }));
    await expect(accepted).resolves.toEqual(publication);

    const invalid: TrackerCommand = {
      ...command, expected_revision: "resident/8",
      edits: [...command.edits, command.edits[0]!],
    };
    const rejected = client.trackerCommand!(invalid);
    expect(requests[1]).toEqual({id: 2, kind: "command", request: invalid});
    const failure = {code: "invalid_tracker_operation", message: "duplicate cell targets are unsupported",
      current_revision: "resident/8", diagnostics: []};
    receive!(new MessageEvent<WorkerReply>("message", {
      data: {id: 2, status: "error", error: failure},
    }));
    await expect(rejected).rejects.toBeInstanceOf(DesignerRuntimeError);
    await expect(rejected).rejects.toMatchObject({failure});
  } finally {
    client.close();
    vi.unstubAllGlobals();
  }
});

it.each(["csv", "xlsx"] as const)("exports native tables as %s through the public Worker method", async format => {
  let receive: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  const requests: WorkerRequest[] = [];
  vi.stubGlobal("Worker", class {
    addEventListener(type: string, callback: (event: MessageEvent<WorkerReply>) => void) {
      if (type === "message") receive = callback;
    }
    postMessage(request: WorkerRequest) { requests.push(structuredClone(request)); }
    terminate() {}
  });
  const client = createExperimentalDesignerClient();
  try {
    const pending = client.exportNativeTablesSpreadsheet("resident/7", "sales", format);
    expect(requests).toEqual([{
      id: 1, kind: "spreadsheet", bytes: new ArrayBuffer(0),
      operation: {type: "export_native_tables", expected_revision: "resident/7", collection: "sales", format},
    }]);
    const result = {revision: "resident/7", bytes: new Uint8Array([1, 2, 3]).buffer,
      ledger: [{category: "lossy_on_export" as const, code: "native_tables_session_history_excluded", location: "workbook", message: "Session history excluded.", blocking: false}]};
    receive!(new MessageEvent<WorkerReply>("message", {
      data: {id: 1, status: "spreadsheet_exported", export: result},
    }));
    await expect(pending).resolves.toEqual(result);

    const stale = client.exportNativeTablesSpreadsheet("resident/6", "sales", format);
    const failure = {code: "stale_revision", message: "Stale revision", current_revision: "resident/7", diagnostics: []};
    receive!(new MessageEvent<WorkerReply>("message", {data: {id: 2, status: "error", error: failure}}));
    await expect(stale).rejects.toMatchObject({failure});
  } finally {
    client.close();
    vi.unstubAllGlobals();
  }
});

it("preserves required_values_missing diagnostics through the public Worker client", async () => {
  let receive: ((event: MessageEvent<WorkerReply>) => void) | undefined;
  const requests: WorkerRequest[] = [];
  vi.stubGlobal("Worker", class {
    addEventListener(type: string, callback: (event: MessageEvent<WorkerReply>) => void) {
      if (type === "message") receive = callback;
    }
    postMessage(request: WorkerRequest) { requests.push(structuredClone(request)); }
    terminate() {}
  });
  const client = createExperimentalDesignerClient();
  try {
    const pending = client.setColumnRequiredness("resident/7", "items", "note", true);
    expect(requests).toEqual([{id: 1, kind: "command", request: {
      type: "set_column_requiredness", expected_revision: "resident/7", collection: "items", field: "note", required: true,
    }}]);
    const failure = {
      code: "required_values_missing", message: "3 rows have no value in this column.",
      current_revision: "resident/7", diagnostics: [{code: "required_values_missing",
        message: "3 rows have no value in this column.", path: "note", subjects: [],
        related_subjects: [], facts: [{name: "blank_count", value: "3"}]}],
    };
    const expectedCode: FailureCode = "required_values_missing";
    expect(failure.code).toBe(expectedCode);
    receive!(new MessageEvent<WorkerReply>("message", {
      data: {id: 1, status: "error", error: failure},
    }));
    await expect(pending).rejects.toMatchObject({failure});
  } finally {
    client.close();
    vi.unstubAllGlobals();
  }
});
